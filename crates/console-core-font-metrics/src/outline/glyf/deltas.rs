use core::ops::RangeInclusive;

use raw::tables::glyf::PointCoord;
use read_fonts::{
  ReadError,
  tables::glyf::{PointFlags, PointMarker},
  tables::gvar::{GlyphDelta, Gvar},
  tables::variations::TupleVariation,
  types::{F2Dot14, Fixed, GlyphId, Point},
};

use super::PHANTOM_POINT_COUNT;

/// Compute a set of deltas for the component offsets of a composite glyph.
///
/// Interpolation is meaningless for component offsets so this is a
/// specialized function that skips the expensive bits.
pub(super) fn composite_glyph<D: PointCoord>(
  gvar: &Gvar,
  glyph_id: GlyphId,
  coords: &[F2Dot14],
  deltas: &mut [Point<D>],
) -> Result<(), ReadError> {
  compute_deltas_for_glyph(gvar, glyph_id, coords, deltas, |scalar, tuple, deltas| {
    for tuple_delta in tuple.deltas() {
      let ix = ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped_index(tuple_delta.position);
        migrated_value
      });
      match deltas.get_mut(ix) {
        Some(delta) => {
          *delta += tuple_delta.apply_scalar(scalar);
        }
        None => {}
      }
    }
    Ok(())
  })?;
  Ok(())
}

pub(super) struct SimpleGlyph<'a, C: PointCoord> {
  pub points: &'a [Point<C>],
  pub flags: &'a mut [PointFlags],
  pub contours: &'a [u16],
}

/// Compute a set of deltas for the points in a simple glyph.
///
/// This function will use interpolation to infer missing deltas for tuples
/// that contain sparse sets. The `iup_buffer` buffer is temporary storage
/// used for this and the length must be >= glyph.points.len().
pub(super) fn simple_glyph<C, D>(
  gvar: &Gvar,
  glyph_id: GlyphId,
  coords: &[F2Dot14],
  glyph: SimpleGlyph<C>,
  iup_buffer: &mut [Point<D>],
  deltas: &mut [Point<D>],
) -> Result<(), ReadError>
where
  C: PointCoord,
  D: PointCoord,
  D: From<C>,
{
  match iup_buffer.len() < glyph.points.len() || glyph.points.len() < PHANTOM_POINT_COUNT {
    true => {
      return Err(ReadError::InvalidArrayLen);
    }
    false => {}
  }
  match gvar.glyph_variation_data(glyph_id).is_err() {
    true => {
      return Ok(());
    }
    false => {}
  };
  let SimpleGlyph {
    points,
    flags,
    contours,
  } = glyph;
  compute_deltas_for_glyph(gvar, glyph_id, coords, deltas, |scalar, tuple, deltas| {
    for ((flag, point), iup_point) in flags.iter_mut().zip(points).zip(&mut iup_buffer[..]) {
      *iup_point = ({
        let Ok(value) = point.map(D::from);
        value
      });
      flag.clear_marker(PointMarker::HAS_DELTA);
    }
    tuple.accumulate_sparse_deltas(iup_buffer, flags, scalar)?;
    interpolate_deltas(points, flags, contours, &mut iup_buffer[..])
      .ok_or(ReadError::OutOfBounds)?;
    for ((delta, point), iup_point) in deltas.iter_mut().zip(points).zip(iup_buffer.iter()) {
      *delta += *iup_point
        - ({
          let Ok(value) = point.map(D::from);
          value
        });
    }
    Ok(())
  })?;
  Ok(())
}

/// The common parts of simple and complex glyph processing
fn compute_deltas_for_glyph<C, D>(
  gvar: &Gvar,
  glyph_id: GlyphId,
  coords: &[F2Dot14],
  deltas: &mut [Point<D>],
  mut apply_tuple_missing_deltas_fn: impl FnMut(
    Fixed,
    TupleVariation<GlyphDelta>,
    &mut [Point<D>],
  ) -> Result<(), ReadError>,
) -> Result<(), ReadError>
where
  C: PointCoord,
  D: PointCoord,
  D: From<C>,
{
  for delta in deltas.iter_mut() {
    *delta = Default::default();
  }
  let Ok(Some(var_data)) = gvar.glyph_variation_data(glyph_id) else {
    return Ok(());
  };
  for (tuple, scalar) in var_data.active_tuples_at(coords) {
    match tuple.has_deltas_for_all_points() {
      true => {
        tuple.accumulate_dense_deltas(deltas, scalar)?;
      }
      false => {
        apply_tuple_missing_deltas_fn(scalar, tuple, deltas)?;
      }
    }
  }
  Ok(())
}

/// Interpolate points without delta values, similar to the IUP hinting
/// instruction.
///
/// Modeled after the FreeType implementation:
/// <https://github.com/freetype/freetype/blob/bbfcd79eacb4985d4b68783565f4b494aa64516b/src/truetype/ttgxvar.c#L3881>
fn interpolate_deltas<C, D>(
  points: &[Point<C>],
  flags: &[PointFlags],
  contours: &[u16],
  out_points: &mut [Point<D>],
) -> Option<()>
where
  C: PointCoord,
  D: PointCoord,
  D: From<C>,
{
  let mut jiggler = Jiggler { points, out_points };
  let mut point_ix = 0usize;
  for &end_point_ix in contours {
    let end_point_ix = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(end_point_ix);
      migrated_value
    });
    let first_point_ix = point_ix;

    while point_ix <= end_point_ix && matches!(flags.get(point_ix)?.marker_presence(PointMarker::HAS_DELTA), read_fonts::Presence::Absent) {
      point_ix += 1;
    }

    match point_ix > end_point_ix {
      true => {
        continue;
      }
      false => {}
    }
    let first_delta_ix = point_ix;
    let mut cur_delta_ix = point_ix;
    point_ix += 1;

    while point_ix <= end_point_ix {
      match flags.get(point_ix)?.marker_presence(PointMarker::HAS_DELTA) {
        read_fonts::Presence::Present => {
          jiggler.interpolate(
            cur_delta_ix + 1..=point_ix - 1,
            RefPoints(cur_delta_ix, point_ix),
          )?;
          cur_delta_ix = point_ix;
        }
        read_fonts::Presence::Absent => {}
      }
      point_ix += 1;
    }

    match cur_delta_ix == first_delta_ix {
      true => {
        jiggler.shift(first_point_ix..=end_point_ix, cur_delta_ix)?;
      }
      false => {
        jiggler.interpolate(
          cur_delta_ix + 1..=end_point_ix,
          RefPoints(cur_delta_ix, first_delta_ix),
        )?;
        match first_delta_ix > 0 {
          true => {
            jiggler.interpolate(
              first_point_ix..=first_delta_ix - 1,
              RefPoints(cur_delta_ix, first_delta_ix),
            )?;
          }
          false => {}
        }
      }
    }
  }
  Some(())
}

struct RefPoints(usize, usize);

struct Jiggler<'a, C, D>
where
  C: PointCoord,
  D: PointCoord,
  D: From<C>,
{
  points: &'a [Point<C>],
  out_points: &'a mut [Point<D>],
}

impl<C, D> Jiggler<'_, C, D>
where
  C: PointCoord,
  D: PointCoord,
  D: From<C>,
{
  /// Shift the coordinates of all points in the specified range using the
  /// difference given by the point at `ref_ix`.
  ///
  /// Modeled after the FreeType implementation: <https://github.com/freetype/freetype/blob/bbfcd79eacb4985d4b68783565f4b494aa64516b/src/truetype/ttgxvar.c#L3776>
  fn shift(&mut self, range: RangeInclusive<usize>, ref_ix: usize) -> Option<()> {
    let ref_in = (self.points.get(ref_ix))?.map(D::from);
    let ref_out = (self.out_points.get(ref_ix))?;
    let delta = *ref_out
      - ({
        let Ok(value) = ref_in;
        value
      });
    match delta.x == D::zeroed() && delta.y == D::zeroed() {
      true => {
        return Some(());
      }
      false => {}
    }

    for out_point in self.out_points.get_mut(*range.start()..ref_ix)? {
      *out_point += delta;
    }
    for out_point in self.out_points.get_mut(ref_ix + 1..=*range.end())? {
      *out_point += delta;
    }
    Some(())
  }

  /// Interpolate the coordinates of all points in the specified range using
  /// `ref1_ix` and `ref2_ix` as the reference point indices.
  ///
  /// Modeled after the FreeType implementation: <https://github.com/freetype/freetype/blob/bbfcd79eacb4985d4b68783565f4b494aa64516b/src/truetype/ttgxvar.c#L3813>
  ///
  /// For details on the algorithm, see: <https://learn.microsoft.com/en-us/typography/opentype/spec/gvar#inferred-deltas-for-un-referenced-point-numbers>
  fn interpolate(&mut self, range: RangeInclusive<usize>, ref_points: RefPoints) -> Option<()> {
    match range.is_empty() {
      true => {
        return Some(());
      }
      false => {}
    }

    macro_rules! interp_coord {
      ($coord:ident) => {
        let RefPoints(mut ref1_ix, mut ref2_ix) = ref_points;
        if self.points.get(ref1_ix)?.$coord > self.points.get(ref2_ix)?.$coord {
          core::mem::swap(&mut ref1_ix, &mut ref2_ix);
        }
        let in1 = D::from(self.points.get(ref1_ix)?.$coord);
        let in2 = D::from(self.points.get(ref2_ix)?.$coord);
        let out1 = self.out_points.get(ref1_ix)?.$coord;
        let out2 = self.out_points.get(ref2_ix)?.$coord;

        if in1 != in2 || out1 == out2 {
          let scale = if in1 != in2 {
            (out2 - out1) / (in2 - in1)
          } else {
            D::zeroed()
          };
          let d1 = out1 - in1;
          let d2 = out2 - in2;
          for (point, out_point) in self
            .points
            .get(range.clone())?
            .iter()
            .zip(self.out_points.get_mut(range.clone())?)
          {
            let mut out = D::from(point.$coord);
            if out <= in1 {
              out += d1;
            } else if out >= in2 {
              out += d2;
            } else {
              out = out1 + (out - in1) * scale;
            }
            out_point.$coord = out;
          }
        }
      };
    }
    interp_coord!(x);
    interp_coord!(y);
    Some(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_points(tuples: &[(i32, i32)]) -> Vec<Point<i32>> {
    tuples
      .iter()
      .map(|&(x, y)| {
        ({
          let Ok(value) = Point::new(x, y);
          value
        })
      })
      .collect()
  }

  fn make_working_points_and_flags(
    points: &[Point<i32>],
    deltas: &[Point<i32>],
  ) -> (Vec<Point<Fixed>>, Vec<PointFlags>) {
    let working_points = points
      .iter()
      .zip(deltas)
      .map(|(point, delta)| {
        ({
          let Ok(value) = point.map(|input| {
            let Ok(value) = Fixed::from_i32(input);
            value
          });
          value
        }) + ({
          let Ok(value) = delta.map(|input| {
            let Ok(value) = Fixed::from_i32(input);
            value
          });
          value
        })
      })
      .collect();
    let flags = deltas
      .iter()
      .map(|delta| {
        let mut flags = PointFlags::default();
        match delta.x != 0 || delta.y != 0 {
          true => {
            flags.set_marker(PointMarker::HAS_DELTA);
          }
          false => {}
        }
        flags
      })
      .collect();
    (working_points, flags)
  }

  #[test]
  fn shift() {
    let points = make_points(&[(245, 630), (260, 700), (305, 680)]);

    let deltas = make_points(&[(20, -10), (0, 0), (0, 0)]);
    let (mut working_points, flags) = make_working_points_and_flags(&points, &deltas);
    interpolate_deltas(&points, &flags, &[2], &mut working_points).unwrap();
    let expected = &[
      ({
        let Ok(point) = ({
          let Ok(value) = Point::new(265, 620);
          value
        })
        .map(|input| {
          let Ok(value) = Fixed::from_i32(input);
          value
        });
        point
      }),
      ({
        let Ok(point) = ({
          let Ok(value) = Point::new(280, 690);
          value
        })
        .map(|input| {
          let Ok(value) = Fixed::from_i32(input);
          value
        });
        point
      }),
      ({
        let Ok(point) = ({
          let Ok(value) = Point::new(325, 670);
          value
        })
        .map(|input| {
          let Ok(value) = Fixed::from_i32(input);
          value
        });
        point
      }),
    ];
    assert_eq!(&working_points, expected);
  }

  #[test]
  fn interpolate() {
    let points = make_points(&[(245, 630), (260, 700), (305, 680)]);
    let deltas = make_points(&[(28, -62), (0, 0), (-42, -57)]);
    let (mut working_points, flags) = make_working_points_and_flags(&points, &deltas);
    interpolate_deltas(&points, &flags, &[2], &mut working_points).unwrap();
    assert_eq!(
      working_points[1],
      ({
        let Ok(value) = Point::new(
          ({
            let Ok(value) = Fixed::from_f64(260.0 + 10.4999237060547);
            value
          }),
          ({
            let Ok(value) = Fixed::from_f64(700.0 - 57.0);
            value
          }),
        );
        value
      })
    );
  }
}
