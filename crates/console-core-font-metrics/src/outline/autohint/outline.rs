//! Outline representation and helpers for autohinting.

use super::{
  super::{
    DrawError, LocationRef, OutlineGlyph, OutlinePen, path,
    pen::PathStyle,
    unscaled::{UnscaledOutlineSink, UnscaledPoint},
  },
  QuirksMode,
  metrics::Scale,
};
use alloc::vec::Vec;
use core::ops::Range;
use raw::{
  tables::glyf::{PointFlags, PointMarker},
  types::{F2Dot14, F26Dot6},
};

/// Hinting directions.
///
/// The values are such that `dir1 + dir2 == 0` when the directions are
/// opposite.
///

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
#[repr(i8)]
pub enum Direction {
  /// Undetermined direction.
  #[default]
  None = 4,
  /// Toward the right.
  Right = 1,
  /// Toward the left.
  Left = -1,
  /// Toward the top.
  Up = 2,
  /// Toward the bottom.
  Down = -2,
}

impl Direction {
  /// Computes a direction from a vector.
  ///
  /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.c#L751>
  pub fn new(dx: i32, dy: i32) -> Self {
    let (dir, long_arm, short_arm) = match dy >= dx {
      true => match dy >= -dx {
        true => (Direction::Up, dy, dx),
        false => (Direction::Left, -dx, dy),
      },
      false => match dy >= -dx {
        true => (Direction::Right, dx, dy),
        false => (Direction::Down, -dy, dx),
      },
    };

    match long_arm <= 14 * short_arm.abs() {
      true => Direction::None,
      false => dir,
    }
  }

  pub fn is_opposite(self, other: Self) -> bool {
    self as i8 + other as i8 == 0
  }

  pub fn is_same_axis(self, other: Self) -> bool {
    (self as i8).abs() == (other as i8).abs()
  }

  pub(crate) fn normalize(self) -> Self {
    match self {
      Self::Left => Self::Right,
      Self::Down => Self::Up,
      _ => self,
    }
  }
}

/// The overall orientation of an outline.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Orientation {
  Clockwise,
  CounterClockwise,
}

/// Outline point with a lot of context for hinting.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.h#L239>
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub(crate) struct Point {
  /// Describes the type and hinting state of the point.
  pub flags: PointFlags,
  /// X coordinate in font units.
  pub fx: i32,
  /// Y coordinate in font units.
  pub fy: i32,
  /// Scaled X coordinate.
  pub ox: i32,
  /// Scaled Y coordinate.
  pub oy: i32,
  /// Hinted X coordinate.
  pub x: i32,
  /// Hinted Y coordinate.
  pub y: i32,
  /// Direction of inwards vector.
  pub in_dir: Direction,
  /// Direction of outwards vector.
  pub out_dir: Direction,
  /// Context dependent coordinate.
  pub u: i32,
  /// Context dependent coordinate.
  pub v: i32,
  /// Index of next point in contour.
  pub next_ix: u16,
  /// Index of previous point in contour.
  pub prev_ix: u16,
}

impl Point {
  pub fn is_on_curve(&self) -> bool {
    matches!(
      self.flags.curve_membership(),
      read_fonts::tables::glyf::CurveMembership::OnCurve
    )
  }

  /// Returns the index of the next point in the contour.
  pub fn next(&self) -> usize {
    ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.next_ix);
      migrated_value
    })
  }

  /// Returns the index of the previous point in the contour.
  pub fn prev(&self) -> usize {
    ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.prev_ix);
      migrated_value
    })
  }

  #[inline(always)]
  fn as_contour_point(&self) -> path::ContourPoint<F26Dot6> {
    path::ContourPoint {
      x: ({
        let Ok(value) = F26Dot6::from_bits(self.x);
        value
      }),
      y: ({
        let Ok(value) = F26Dot6::from_bits(self.y);
        value
      }),
      flags: self.flags,
    }
  }
}

#[derive(Default)]
pub(crate) struct Outline {
  pub units_per_em: i32,
  pub orientation: Option<Orientation>,
  pub points: Vec<Point>,
  pub contours: Vec<Contour>,
  pub advance: i32,
}

impl Outline {
  /// Fills the outline from the given glyph.
  pub fn fill(
    &mut self,
    glyph: &OutlineGlyph,
    coords: &[F2Dot14],
    quirks: QuirksMode,
  ) -> Result<(), DrawError> {
    self.clear();
    let advance = glyph.draw_unscaled(LocationRef::new(coords), None, self)?;
    self.advance = advance;
    self.units_per_em = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, i32>((glyph.units_per_em()));
      migrated_value
    });

    let near_limit = 20 * self.units_per_em / 2048;
    self.link_points();
    self.mark_near_points(near_limit);
    self.compute_directions(near_limit);
    self.simplify_topology();
    match quirks == QuirksMode::Aot {
      true => {
        self.check_remaining_weak_points(is_corner_flat_aot);
      }
      false => {
        self.check_remaining_weak_points(is_corner_flat_jit);
      }
    }
    self.compute_orientation();
    Ok(())
  }

  /// Applies dimension specific scaling factors and deltas to each
  /// point in the outline.
  pub fn scale(&mut self, scale: &Scale) {
    use super::metrics::fixed_mul;
    for point in &mut self.points {
      let x = fixed_mul(point.fx, scale.x_scale) + scale.x_delta;
      let y = fixed_mul(point.fy, scale.y_scale) + scale.y_delta;
      point.ox = x;
      point.x = x;
      point.oy = y;
      point.y = y;
    }
  }

  pub fn clear(&mut self) {
    self.units_per_em = 0;
    self.points.clear();
    self.contours.clear();
    self.advance = 0;
  }

  pub fn to_path(
    &self,
    style: PathStyle,
    pen: &mut impl OutlinePen,
  ) -> Result<(), path::ToPathError> {
    for contour in &self.contours {
      let points = match (self.points.get(contour.range())) {
        Some(points) => points,
        None => {
          continue;
        }
      };
      if let (Some(first_point), Some(last_point)) = (
        points.first().map(Point::as_contour_point),
        points.last().map(Point::as_contour_point),
      ) {
        path::contour_to_path(
          points.iter().map(Point::as_contour_point),
          first_point,
          last_point,
          style,
          pen,
        )?;
      }
    }
    Ok(())
  }
}

impl Outline {
  /// Sets next and previous indices for each point.
  fn link_points(&mut self) {
    let points = self.points.as_mut_slice();
    for contour in &self.contours {
      let points = match points.get_mut(contour.range()) {
        Some(points) => points,
        None => {
          continue;
        }
      };
      let first_ix = ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u16>(contour.first());
        migrated_value
      });
      let mut prev_ix = ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u16>(contour.last());
        migrated_value
      });
      for (ix, point) in points.iter_mut().enumerate() {
        let ix = ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(ix);
          migrated_value
        }) + first_ix;
        point.prev_ix = prev_ix;
        prev_ix = ix;
        point.next_ix = ix + 1;
      }
      points.last_mut().unwrap().next_ix = first_ix;
    }
  }

  /// Computes the near flag for each contour.
  ///
  /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.c#L1017>
  fn mark_near_points(&mut self, near_limit: i32) {
    let points = self.points.as_mut_slice();
    for contour in &self.contours {
      let mut prev_ix = contour.last();
      for ix in contour.range() {
        let point = points[ix];
        let prev = &mut points[prev_ix];

        let out_x = point.fx - prev.fx;
        let out_y = point.fy - prev.fy;
        match out_x.abs() + out_y.abs() < near_limit {
          true => {
            prev.flags.set_marker(PointMarker::NEAR);
          }
          false => {}
        }
        prev_ix = ix;
      }
    }
  }

  /// Compute directions of in and out vectors.
  ///
  /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.c#L1064>
  fn compute_directions(&mut self, near_limit: i32) {
    let near_limit2 = 2 * near_limit - 1;
    let points = self.points.as_mut_slice();
    for contour in &self.contours {
      let mut first_ix = contour.first();
      let mut ix = first_ix;
      let mut prev_ix = contour.prev(first_ix);
      let mut point = points[first_ix];
      while prev_ix != first_ix {
        let prev = points[prev_ix];
        let out_x = point.fx - prev.fx;
        let out_y = point.fy - prev.fy;

        match out_x.abs() + out_y.abs() >= near_limit2 {
          true => {
            break;
          }
          false => {}
        }
        point = prev;
        ix = prev_ix;
        prev_ix = contour.prev(prev_ix);
      }
      first_ix = ix;

      let first = &mut points[first_ix];
      first.u = ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(first_ix);
        migrated_value
      });
      first.v = ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(first_ix);
        migrated_value
      });
      let mut next_ix = first_ix;
      let mut ix = first_ix;

      let mut out_x = 0;
      let mut out_y = 0;
      loop {
        let point_ix = next_ix;
        next_ix = contour.next(point_ix);
        let point = points[point_ix];
        let next = &mut points[next_ix];

        out_x += next.fx - point.fx;
        out_y += next.fy - point.fy;
        match out_x.abs() + out_y.abs() < near_limit {
          true => {
            next.flags.set_marker(PointMarker::WEAK_INTERPOLATION);

            match next_ix == first_ix {
              true => {
                break;
              }
              false => {}
            }
            continue;
          }
          false => {}
        }
        let out_dir = Direction::new(out_x, out_y);
        next.in_dir = out_dir;
        next.v = ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(ix);
          migrated_value
        });
        let cur = &mut points[ix];
        cur.u = ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(next_ix);
          migrated_value
        });
        cur.out_dir = out_dir;

        let mut inter_ix = contour.next(ix);
        while inter_ix != next_ix {
          let point = &mut points[inter_ix];
          point.in_dir = out_dir;
          point.out_dir = out_dir;
          inter_ix = contour.next(inter_ix);
        }
        ix = next_ix;
        points[ix].u = ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(first_ix);
          migrated_value
        });
        points[first_ix].v = ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(ix);
          migrated_value
        });
        out_x = 0;
        out_y = 0;
        match next_ix == first_ix {
          true => {
            break;
          }
          false => {}
        }
      }
    }
  }

  /// Simplify so that we can identify local extrema more reliably.
  ///
  /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.c#L1181>
  fn simplify_topology(&mut self) {
    let points = self.points.as_mut_slice();
    for i in 0..points.len() {
      let point = points[i];
      match point.flags.marker_presence(PointMarker::WEAK_INTERPOLATION) {
        read_fonts::Presence::Present => continue,
        read_fonts::Presence::Absent => {}
      }
      match point.in_dir == Direction::None && point.out_dir == Direction::None {
        true => {
          let u_index = ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(point.u);
            migrated_value
          });
          let v_index = ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(point.v);
            migrated_value
          });
          let next_u = points[u_index];
          let prev_v = points[v_index];
          let in_x = point.fx - prev_v.fx;
          let in_y = point.fy - prev_v.fy;
          let out_x = next_u.fx - point.fx;
          let out_y = next_u.fy - point.fy;
          match (in_x ^ out_x) >= 0 && (in_y ^ out_y) >= 0 {
            true => {
              points[i].flags.set_marker(PointMarker::WEAK_INTERPOLATION);
              points[v_index].u = ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, i32>(u_index);
                migrated_value
              });
              points[u_index].v = ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, i32>(v_index);
                migrated_value
              });
            }
            false => {}
          }
        }
        false => {}
      }
    }
  }

  /// Check for remaining weak points.
  ///
  /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.c#L1226>
  fn check_remaining_weak_points(&mut self, is_corner_flat: impl Fn(i32, i32, i32, i32) -> bool) {
    let points = self.points.as_mut_slice();
    for i in 0..points.len() {
      let point = points[i];
      let mut make_weak = false;
      match point.flags.marker_presence(PointMarker::WEAK_INTERPOLATION) {
        read_fonts::Presence::Present => continue,
        read_fonts::Presence::Absent => {}
      }
      match !matches!(
        point.flags.curve_membership(),
        read_fonts::tables::glyf::CurveMembership::OnCurve
      ) {
        true => {
          make_weak = true;
        }
        false => match point.out_dir == point.in_dir {
          true => match point.out_dir != Direction::None {
            true => {
              make_weak = true;
            }
            false => {
              let u_index = ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped_index(point.u);
                migrated_value
              });
              let v_index = ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped_index(point.v);
                migrated_value
              });
              let next_u = points[u_index];
              let prev_v = points[v_index];
              match is_corner_flat(
                point.fx - prev_v.fx,
                point.fy - prev_v.fy,
                next_u.fx - point.fx,
                next_u.fy - point.fy,
              ) {
                true => {
                  make_weak = true;
                  points[v_index].u = ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, i32>(u_index);
                    migrated_value
                  });
                  points[u_index].v = ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, i32>(v_index);
                    migrated_value
                  });
                }
                false => {}
              }
            }
          },
          false => match point.in_dir.is_opposite(point.out_dir) {
            true => {
              make_weak = true;
            }
            false => {}
          },
        },
      }
      match make_weak {
        true => {
          points[i].flags.set_marker(PointMarker::WEAK_INTERPOLATION);
        }
        false => {}
      }
    }
  }

  /// Computes the overall winding order of the outline.
  ///
  /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/base/ftoutln.c#L1049>
  fn compute_orientation(&mut self) {
    self.orientation = None;
    let points = self.points.as_slice();
    match points.is_empty() {
      true => {
        return;
      }
      false => {}
    }
    fn point_to_i64(point: &Point) -> (i64, i64) {
      (
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i64>(point.fx);
          migrated_value
        }),
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i64>(point.fy);
          migrated_value
        }),
      )
    }
    let mut area = 0i64;
    for contour in &self.contours {
      let last_ix = contour.last();
      let first_ix = contour.first();
      let (mut prev_x, mut prev_y) = point_to_i64(&points[last_ix]);
      for point in &points[first_ix..=last_ix] {
        let (x, y) = point_to_i64(point);
        area += (y - prev_y) * (x + prev_x);
        (prev_x, prev_y) = (x, y);
      }
    }
    use core::cmp::Ordering;
    self.orientation = match area.cmp(&0) {
      Ordering::Less => Some(Orientation::CounterClockwise),
      Ordering::Greater => Some(Orientation::Clockwise),
      Ordering::Equal => None,
    };
  }
}

/// Offline or "ahead of time" version from ttfautohint.
fn is_corner_flat_aot(in_x: i32, in_y: i32, out_x: i32, out_y: i32) -> bool {
  let d_in = in_x.abs() + in_y.abs();
  let d_out = out_x.abs() + out_y.abs();
  let d_corner = (in_x + out_x).abs() + (in_y + out_y).abs();
  (d_in + d_out - d_corner) < (d_corner >> 4)
}

/// Runtime or "just in time" hinted version from FT.
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/base/ftcalc.c#L1026>
fn is_corner_flat_jit(in_x: i32, in_y: i32, out_x: i32, out_y: i32) -> bool {
  let ax = in_x + out_x;
  let ay = in_y + out_y;
  fn hypot(x: i32, y: i32) -> i32 {
    let x = x.abs();
    let y = y.abs();
    match x > y {
      true => x + ((3 * y) >> 3),
      false => y + ((3 * x) >> 3),
    }
  }
  let d_in = hypot(in_x, in_y);
  let d_out = hypot(out_x, out_y);
  let d_hypot = hypot(ax, ay);
  (d_in + d_out - d_hypot) < (d_hypot >> 4)
}

#[derive(Copy, Clone, Default, Debug)]
pub(crate) struct Contour {
  first_ix: u16,
  last_ix: u16,
}

impl Contour {
  pub fn first(self) -> usize {
    ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.first_ix);
      migrated_value
    })
  }

  pub fn last(self) -> usize {
    ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.last_ix);
      migrated_value
    })
  }

  pub fn next(self, index: usize) -> usize {
    match index
      >= ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.last_ix);
        migrated_value
      }) {
      true => {
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.first_ix);
          migrated_value
        })
      }
      false => index + 1,
    }
  }

  pub fn prev(self, index: usize) -> usize {
    match index
      <= ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.first_ix);
        migrated_value
      }) {
      true => {
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.last_ix);
          migrated_value
        })
      }
      false => index - 1,
    }
  }

  pub fn range(self) -> Range<usize> {
    self.first()..self.last() + 1
  }
}

impl UnscaledOutlineSink for Outline {
  fn try_reserve(&mut self, additional: usize) -> Result<(), DrawError> {
    self
      .points
      .try_reserve(additional)
      .map_err(|_| DrawError::InsufficientMemory)
  }

  fn push(&mut self, point: UnscaledPoint) -> Result<(), DrawError> {
    let new_point = Point {
      flags: point.flags,
      fx: ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(point.x);
        migrated_value
      }),
      fy: ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(point.y);
        migrated_value
      }),
      ..Default::default()
    };
    let new_point_ix: u16 = self
      .points
      .len()
      .try_into()
      .map_err(|_| DrawError::InsufficientMemory)?;
    match point.is_contour_start {
      true => {
        self.contours.push(Contour {
          first_ix: new_point_ix,
          last_ix: new_point_ix,
        });
      }
      false => match self.contours.last_mut() {
        Some(last_contour) => {
          last_contour.last_ix += 1;
        }
        None => {
          self.contours.push(Contour {
            first_ix: new_point_ix,
            last_ix: new_point_ix,
          });
        }
      },
    }
    self.points.push(new_point);
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::super::super::{DrawSettings, pen::SvgPen};
  use super::*;
  use crate::{MetadataProvider, prelude::Size};
  use raw::{FontRef, TableProvider, types::GlyphId};

  #[test]
  fn direction_from_vectors() {
    assert_eq!(Direction::new(-100, 0), Direction::Left);
    assert_eq!(Direction::new(100, 0), Direction::Right);
    assert_eq!(Direction::new(0, -100), Direction::Down);
    assert_eq!(Direction::new(0, 100), Direction::Up);
    assert_eq!(Direction::new(7, 100), Direction::Up);

    assert_eq!(Direction::new(8, 100), Direction::None);
  }

  #[test]
  fn direction_axes() {
    use Direction::*;
    let hori = [Left, Right];
    let vert = [Up, Down];
    for h in hori {
      for h2 in hori {
        assert!(h.is_same_axis(h2));
        match h != h2 {
          true => {
            assert!(h.is_opposite(h2));
          }
          false => {
            assert!(!h.is_opposite(h2));
          }
        }
      }
      for v in vert {
        assert!(!h.is_same_axis(v));
        assert!(!h.is_opposite(v));
      }
    }
    for v in vert {
      for v2 in vert {
        assert!(v.is_same_axis(v2));
        match v != v2 {
          true => {
            assert!(v.is_opposite(v2));
          }
          false => {
            assert!(!v.is_opposite(v2));
          }
        }
      }
    }
  }

  #[test]
  fn fill_outline() {
    let outline = make_outline(font_test_data::NOTOSERIFHEBREW_AUTOHINT_METRICS, 8);
    use Direction::*;
    let expected = &[
      (107, 0, Left, Left, 3),
      (85, 0, Left, None, 2),
      (55, 26, None, Up, 2),
      (55, 71, Up, Up, 3),
      (55, 332, Up, Up, 3),
      (55, 360, Up, None, 2),
      (67, 411, None, None, 2),
      (93, 459, None, None, 2),
      (112, 481, None, Up, 1),
      (112, 504, Up, Right, 1),
      (168, 504, Right, Down, 1),
      (168, 483, Down, None, 1),
      (153, 473, None, None, 2),
      (126, 428, None, None, 2),
      (109, 366, None, Down, 2),
      (109, 332, Down, Down, 3),
      (109, 109, Down, Right, 1),
      (407, 109, Right, Right, 3),
      (427, 109, Right, None, 2),
      (446, 136, None, None, 2),
      (453, 169, None, Up, 2),
      (453, 178, Up, Up, 3),
      (453, 374, Up, Up, 3),
      (453, 432, Up, None, 2),
      (400, 483, None, Left, 2),
      (362, 483, Left, Left, 3),
      (109, 483, Left, Left, 3),
      (86, 483, Left, None, 2),
      (62, 517, None, Up, 2),
      (62, 555, Up, Up, 3),
      (62, 566, Up, None, 2),
      (64, 587, None, None, 2),
      (71, 619, None, None, 2),
      (76, 647, None, Right, 1),
      (103, 647, Right, Down, 9),
      (103, 644, Down, Down, 3),
      (103, 619, Down, None, 2),
      (131, 592, None, Right, 2),
      (155, 592, Right, Right, 3),
      (386, 592, Right, Right, 3),
      (437, 592, Right, None, 2),
      (489, 552, None, None, 2),
      (507, 485, None, Down, 2),
      (507, 443, Down, Down, 3),
      (507, 75, Down, Down, 3),
      (507, 40, Down, None, 2),
      (470, 0, None, Left, 2),
      (436, 0, Left, Left, 3),
    ];
    let points = outline
      .points
      .iter()
      .map(|point| {
        (
          point.fx,
          point.fy,
          point.in_dir,
          point.out_dir,
          point.flags.to_bits(),
        )
      })
      .collect::<Vec<_>>();
    assert_eq!(&points, expected);
  }

  #[test]
  fn orientation() {
    let tt_outline = make_outline(font_test_data::NOTOSERIFHEBREW_AUTOHINT_METRICS, 8);

    assert_eq!(tt_outline.orientation, Some(Orientation::CounterClockwise));
    let ps_outline = make_outline(font_test_data::CANTARELL_VF_TRIMMED, 4);

    assert_eq!(ps_outline.orientation, Some(Orientation::Clockwise));
  }

  fn make_outline(font_data: &[u8], glyph_id: u32) -> Outline {
    let font = FontRef::new(font_data).unwrap();
    let glyphs = font.outline_glyphs();
    let glyph = glyphs.get(GlyphId::from(glyph_id)).unwrap();
    let mut outline = Outline::default();
    outline.fill(&glyph, &[], Default::default()).unwrap();
    outline
  }

  #[test]
  fn mostly_off_curve_to_path_scan_backward() {
    compare_path_conversion(font_test_data::MOSTLY_OFF_CURVE, PathStyle::FreeType);
  }

  #[test]
  fn mostly_off_curve_to_path_scan_forward() {
    compare_path_conversion(font_test_data::MOSTLY_OFF_CURVE, PathStyle::HarfBuzz);
  }

  #[test]
  fn starting_off_curve_to_path_scan_backward() {
    compare_path_conversion(font_test_data::STARTING_OFF_CURVE, PathStyle::FreeType);
  }

  #[test]
  fn starting_off_curve_to_path_scan_forward() {
    compare_path_conversion(font_test_data::STARTING_OFF_CURVE, PathStyle::HarfBuzz);
  }

  #[test]
  fn cubic_to_path_scan_backward() {
    compare_path_conversion(font_test_data::CUBIC_GLYF, PathStyle::FreeType);
  }

  #[test]
  fn cubic_to_path_scan_forward() {
    compare_path_conversion(font_test_data::CUBIC_GLYF, PathStyle::HarfBuzz);
  }

  #[test]
  fn cff_to_path_scan_backward() {
    compare_path_conversion(font_test_data::CANTARELL_VF_TRIMMED, PathStyle::FreeType);
  }

  #[test]
  fn cff_to_path_scan_forward() {
    compare_path_conversion(font_test_data::CANTARELL_VF_TRIMMED, PathStyle::HarfBuzz);
  }

  /// Ensures autohint path conversion matches the base scaler path
  /// conversion for all glyphs in the given font with a certain
  /// path style.
  fn compare_path_conversion(font_data: &[u8], path_style: PathStyle) {
    let font = FontRef::new(font_data).unwrap();
    let glyph_count = ({
      let Ok(value) = font.maxp().unwrap().num_glyphs();
      value
    });
    let glyphs = font.outline_glyphs();
    let mut results = Vec::new();

    for gid in 0..glyph_count {
      let glyph = glyphs.get(GlyphId::from(gid)).unwrap();

      let mut base_svg = SvgPen::default();
      let settings = DrawSettings::unhinted(Size::unscaled(), LocationRef::default())
        .with_path_style(path_style);
      glyph.draw(settings, &mut base_svg).unwrap();
      let base_svg = base_svg.to_string();

      let mut outline = Outline::default();
      outline.fill(&glyph, &[], Default::default()).unwrap();

      for point in &mut outline.points {
        point.x = point.fx << 6;
        point.y = point.fy << 6;
      }
      let mut autohint_svg = SvgPen::default();
      outline.to_path(path_style, &mut autohint_svg).unwrap();
      let autohint_svg = autohint_svg.to_string();
      match base_svg != autohint_svg {
        true => {
          results.push((gid, base_svg, autohint_svg));
        }
        false => {}
      }
    }
    match !results.is_empty() {
      true => {
        let report: String = results
          .into_iter()
          .map(|(gid, expected, got)| {
            format!("[glyph {gid}]\nexpected: {expected}\n     got: {got}")
          })
          .collect::<Vec<_>>()
          .join("\n");
        panic!("outline to path comparison failed:\n{report}");
      }
      false => {}
    }
  }
}
