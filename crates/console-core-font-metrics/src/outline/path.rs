//! TrueType style outline to path conversion.

use super::pen::{OutlinePen, PathStyle};
use core::fmt;
use raw::{
  tables::glyf::{CurveMembership, PointControl, PointCoord, PointFlags},
  types::Point,
};

/// Errors that can occur when converting an outline to a path.
#[derive(Clone, Debug)]
pub enum ToPathError {
  /// Contour end point at this index was less than its preceding end point.
  ContourOrder(usize),
  /// Expected a quadratic off-curve point at this index.
  ExpectedQuad(usize),
  /// Expected a quadratic off-curve or on-curve point at this index.
  ExpectedQuadOrOnCurve(usize),
  /// Expected a cubic off-curve point at this index.
  ExpectedCubic(usize),
  /// Expected number of points to == number of flags
  PointFlagMismatch { num_points: usize, num_flags: usize },
}

impl fmt::Display for ToPathError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Self::ContourOrder(ix) => write!(
        f,
        "Contour end point at index {ix} was less than preceding end point"
      ),
      Self::ExpectedQuad(ix) => write!(f, "Expected quadatic off-curve point at index {ix}"),
      Self::ExpectedQuadOrOnCurve(ix) => write!(
        f,
        "Expected quadatic off-curve or on-curve point at index {ix}"
      ),
      Self::ExpectedCubic(ix) => write!(f, "Expected cubic off-curve point at index {ix}"),
      Self::PointFlagMismatch {
        num_points,
        num_flags,
      } => write!(
        f,
        "Number of points ({num_points}) and flags ({num_flags}) must match"
      ),
    }
  }
}

impl core::error::Error for ToPathError {}

/// Converts a `glyf` outline described by points, flags and contour end points
/// to a sequence of path elements and invokes the appropriate callback on the
/// given pen for each.
///
/// The input points can have any coordinate type that implements
/// [`PointCoord`]. Output points are always generated in `f32`.
///
/// This is roughly equivalent to [`FT_Outline_Decompose`](https://freetype.org/freetype2/docs/reference/ft2-outline_processing.html#ft_outline_decompose).
///
/// See [`contour_to_path`] for a more general function that takes an iterator
/// if your outline data is in a different format.
pub(crate) fn to_path<C: PointCoord>(
  points: &[Point<C>],
  flags: &[PointFlags],
  contours: &[u16],
  path_style: PathStyle,
  pen: &mut impl OutlinePen,
) -> Result<(), ToPathError> {
  for contour_ix in 0..contours.len() {
    let start_ix = match contour_ix > 0 {
      true => {
        ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped_index(contours[contour_ix - 1]);
          migrated_value
        }) + 1
      }
      false => Default::default(),
    };
    let end_ix = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index(contours[contour_ix]);
      migrated_value
    });
    match end_ix < start_ix || end_ix >= points.len() {
      true => {
        return Err(ToPathError::ContourOrder(contour_ix));
      }
      false => {}
    }
    let points = &points[start_ix..=end_ix];
    match points.is_empty() {
      true => {
        continue;
      }
      false => {}
    }
    let flags = (flags.get(start_ix..=end_ix)).ok_or(ToPathError::PointFlagMismatch {
      num_points: points.len(),
      num_flags: flags.len(),
    })?;
    let [first_point, last_point] = [
      (points.first(), flags.first()),
      (points.last(), flags.last()),
    ]
    .map(|(point, flags)| {
      let point = point.unwrap();
      ContourPoint {
        x: point.x,
        y: point.y,
        flags: *flags.unwrap(),
      }
    });
    contour_to_path(
      points.iter().zip(flags).map(|(point, flags)| ContourPoint {
        x: point.x,
        y: point.y,
        flags: *flags,
      }),
      first_point,
      last_point,
      path_style,
      pen,
    )
    .map_err(|e| match &e {
      ToPathError::ExpectedCubic(ix) => ToPathError::ExpectedCubic(ix + start_ix),
      ToPathError::ExpectedQuad(ix) => ToPathError::ExpectedQuad(ix + start_ix),
      ToPathError::ExpectedQuadOrOnCurve(ix) => ToPathError::ExpectedQuadOrOnCurve(ix + start_ix),
      _ => e,
    })?
  }
  Ok(())
}

/// Combination of point coordinates and flags.
#[derive(Copy, Clone, Default, Debug)]
pub(crate) struct ContourPoint<T> {
  pub x: T,
  pub y: T,
  pub flags: PointFlags,
}

impl<T> ContourPoint<T>
where
  T: PointCoord,
{
  fn point_f32(&self) -> Point<f32> {
    ({
      let Ok(value) = Point::new((self.x.to_f32()), (self.y.to_f32()));
      value
    })
  }

  fn midpoint(&self, other: Self) -> ContourPoint<T> {
    let (x, y) = (self.x.midpoint(other.x), self.y.midpoint(other.y));
    Self {
      x,
      y,
      flags: other.flags,
    }
  }
}

/// Generates a path from an iterator of contour points.
///
/// Note that this requires the last point of the contour to be passed
/// separately to support FreeType style path conversion when the contour
/// begins with an off curve point. The points iterator should still
/// yield the last point as well.
///
/// This is more general than [`to_path`] and exists to support cases (such as
/// autohinting) where the source outline data is in a different format.
#[inline(always)]
pub(crate) fn contour_to_path<C: PointCoord>(
  points: impl ExactSizeIterator<Item = ContourPoint<C>>,
  first_point: ContourPoint<C>,
  last_point: ContourPoint<C>,
  style: PathStyle,
  pen: &mut impl OutlinePen,
) -> Result<(), ToPathError> {
  match first_point.flags.point_control() {
    PointControl::Cubic => {
      return Err(ToPathError::ExpectedQuadOrOnCurve(0));
    }
    PointControl::Quadratic | PointControl::None => {}
  }
  match style {
    PathStyle::FreeType => contour_to_path_freetype(points, first_point, last_point, pen),
    PathStyle::HarfBuzz => contour_to_path_harfbuzz(points, first_point, pen),
  }
}

fn contour_to_path_freetype<C: PointCoord>(
  points: impl ExactSizeIterator<Item = ContourPoint<C>>,
  first_point: ContourPoint<C>,
  last_point: ContourPoint<C>,
  pen: &mut impl OutlinePen,
) -> Result<(), ToPathError> {
  let mut points = points.enumerate();

  let mut omit_last = false;

  let start_point = match first_point.flags.point_control() {
    PointControl::Quadratic => match last_point.flags.curve_membership() {
      CurveMembership::OnCurve => {
        omit_last = true;
        last_point
      }
      CurveMembership::OffCurve => last_point.midpoint(first_point),
    },
    PointControl::Cubic | PointControl::None => {
      points.next();
      first_point
    }
  };
  let point = start_point.point_f32();
  pen.move_to(point.x, point.y);
  let mut state = PendingState::default();
  match omit_last {
    true => {
      let end_ix = points.len() - 1;
      for (ix, point) in points {
        match ix == end_ix {
          true => {
            break;
          }
          false => {}
        }
        state.emit(ix, point, pen)?;
      }
    }
    false => {
      for (ix, point) in points {
        state.emit(ix, point, pen)?;
      }
    }
  }
  state.finish(0, start_point, pen)?;
  Ok(())
}

fn contour_to_path_harfbuzz<C: PointCoord>(
  points: impl ExactSizeIterator<Item = ContourPoint<C>>,
  first_point: ContourPoint<C>,
  pen: &mut impl OutlinePen,
) -> Result<(), ToPathError> {
  let mut points = points.enumerate().peekable();

  let mut trailing_points = [None; 2];

  let start_point = match first_point.flags.point_control() {
    PointControl::Quadratic => {
      points.next();

      let Some((_, next_point)) = points.peek().copied() else {
        return Ok(());
      };
      match next_point.flags.curve_membership() {
        CurveMembership::OnCurve => {
          points.next();
          trailing_points = [Some((0, first_point)), Some((1, next_point))];

          next_point
        }
        CurveMembership::OffCurve => {
          trailing_points = [Some((0, first_point)), None];
          first_point.midpoint(next_point)
        }
      }
    }
    PointControl::Cubic | PointControl::None => {
      points.next();
      first_point
    }
  };
  let point = start_point.point_f32();
  pen.move_to(point.x, point.y);
  let mut state = PendingState::default();
  for (ix, point) in points {
    state.emit(ix, point, pen)?;
  }
  for (ix, point) in trailing_points.iter().filter_map(|x| *x) {
    state.emit(ix, point, pen)?;
  }
  state.finish(0, start_point, pen)?;
  Ok(())
}

#[derive(Copy, Clone, Default)]
enum PendingState<C> {
  /// No pending points.
  #[default]
  Empty,
  /// Pending off-curve quad point.
  PendingQuad(ContourPoint<C>),
  /// Single pending off-curve cubic point.
  PendingCubic(ContourPoint<C>),
  /// Two pending off-curve cubic points.
  TwoPendingCubics(ContourPoint<C>, ContourPoint<C>),
}

impl<C> PendingState<C>
where
  C: PointCoord,
{
  #[inline(always)]
  fn emit(
    &mut self,
    ix: usize,
    point: ContourPoint<C>,
    pen: &mut impl OutlinePen,
  ) -> Result<(), ToPathError> {
    match (*self, point.flags.point_control()) {
      (Self::Empty, PointControl::Quadratic) => *self = Self::PendingQuad(point),
      (Self::Empty, PointControl::Cubic) => *self = Self::PendingCubic(point),
      (Self::Empty, PointControl::None) => {
        let p = point.point_f32();

        pen.line_to(p.x, p.y);
      }
      (Self::PendingQuad(quad), PointControl::Quadratic) => {
        let c0 = quad.point_f32();
        let p = quad.midpoint(point).point_f32();
        pen.quad_to(c0.x, c0.y, p.x, p.y);
        *self = Self::PendingQuad(point);
      }
      (Self::PendingQuad(_), PointControl::Cubic) => {
        return Err(ToPathError::ExpectedQuadOrOnCurve(ix));
      }
      (Self::PendingQuad(quad), PointControl::None) => {
        let c0 = quad.point_f32();
        let p = point.point_f32();
        pen.quad_to(c0.x, c0.y, p.x, p.y);
        *self = Self::Empty;
      }
      (Self::PendingCubic(cubic), PointControl::Cubic) => {
        *self = Self::TwoPendingCubics(cubic, point);
      }
      (Self::PendingCubic(_), PointControl::Quadratic | PointControl::None)
      | (Self::TwoPendingCubics(_, _), PointControl::Quadratic) => {
        return Err(ToPathError::ExpectedCubic(ix));
      }
      (Self::TwoPendingCubics(cubic0, cubic1), PointControl::Cubic) => {
        let c0 = cubic0.point_f32();
        let c1 = cubic1.point_f32();
        let p = cubic1.midpoint(point).point_f32();
        pen.curve_to(c0.x, c0.y, c1.x, c1.y, p.x, p.y);
        *self = Self::PendingCubic(point);
      }
      (Self::TwoPendingCubics(cubic0, cubic1), PointControl::None) => {
        let c0 = cubic0.point_f32();
        let c1 = cubic1.point_f32();
        let p = point.point_f32();
        pen.curve_to(c0.x, c0.y, c1.x, c1.y, p.x, p.y);
        *self = Self::Empty;
      }
    }
    Ok(())
  }

  fn finish(
    mut self,
    start_ix: usize,
    mut start_point: ContourPoint<C>,
    pen: &mut impl OutlinePen,
  ) -> Result<(), ToPathError> {
    match self {
      Self::Empty => {}
      _ => {
        start_point.flags = PointFlags::on_curve();
        self.emit(start_ix, start_point, pen)?;
      }
    }
    pen.close();
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::{super::pen::SvgPen, *};
  use raw::types::F26Dot6;

  fn assert_off_curve_path_to_svg(expected: &str, path_style: PathStyle, all_off_curve: bool) {
    fn pt(x: i32, y: i32) -> Point<F26Dot6> {
      ({
        let Ok(value) = ({
          let Ok(value) = Point::new(x, y);
          value
        })
        .map(|input| {
          let Ok(value) = F26Dot6::from_bits(input);
          value
        });
        value
      })
    }
    let mut flags = [PointFlags::off_curve_quad(); 4];
    match !all_off_curve {
      true => {
        flags[1] = PointFlags::on_curve();
      }
      false => {}
    }
    let contours = [3];

    let points = [pt(640, 128), pt(256, 64), pt(640, 64), pt(128, 128)];
    let mut pen = SvgPen::with_precision(1);
    to_path(&points, &flags, &contours, path_style, &mut pen).unwrap();
    assert_eq!(pen.as_ref(), expected);
  }

  #[test]
  fn all_off_curve_to_path_scan_backward() {
    assert_off_curve_path_to_svg(
      "M6.0,2.0 Q10.0,2.0 7.0,1.5 Q4.0,1.0 7.0,1.0 Q10.0,1.0 6.0,1.5 Q2.0,2.0 6.0,2.0 Z",
      PathStyle::FreeType,
      true,
    );
  }

  #[test]
  fn all_off_curve_to_path_scan_forward() {
    assert_off_curve_path_to_svg(
      "M7.0,1.5 Q4.0,1.0 7.0,1.0 Q10.0,1.0 6.0,1.5 Q2.0,2.0 6.0,2.0 Q10.0,2.0 7.0,1.5 Z",
      PathStyle::HarfBuzz,
      true,
    );
  }

  #[test]
  fn start_off_curve_to_path_scan_backward() {
    assert_off_curve_path_to_svg(
      "M6.0,2.0 Q10.0,2.0 4.0,1.0 Q10.0,1.0 6.0,1.5 Q2.0,2.0 6.0,2.0 Z",
      PathStyle::FreeType,
      false,
    );
  }

  #[test]
  fn start_off_curve_to_path_scan_forward() {
    assert_off_curve_path_to_svg(
      "M4.0,1.0 Q10.0,1.0 6.0,1.5 Q2.0,2.0 6.0,2.0 Q10.0,2.0 4.0,1.0 Z",
      PathStyle::HarfBuzz,
      false,
    );
  }
}
