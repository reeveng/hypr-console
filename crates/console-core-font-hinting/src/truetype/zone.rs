//! Glyph zones.

use console_core_never::Never;
use read_fonts::{
  tables::glyf::{PointFlags, PointMarker},
  types::{F26Dot6, Point},
};

use super::{
  error::HintErrorKind,
  graphics::{CoordAxis, GraphicsState},
  math,
};

use super::graphics::Touch;

struct ShiftRegion {
  start: u64,
  end: u64,
  reference: u64,
}

struct InterpolationRegion {
  start: u64,
  end: u64,
  first_reference: u64,
  second_reference: u64,
}

use HintErrorKind::{InvalidPointIndex, InvalidPointRange};

#[derive(Copy, Clone, PartialEq, Default, Debug)]
#[repr(u8)]
pub enum ZonePointer {
  Twilight = 0,
  #[default]
  Glyph = 3,
}

pub enum PointTouch {
  Touched,
  Untouched,
}


impl TryFrom<i32> for ZonePointer {
  type Error = HintErrorKind;

  fn try_from(value: i32) -> Result<Self, Self::Error> {
    match value {
      0 => Ok(Self::Twilight),
      1 => Ok(Self::Glyph),
      _ => Err(HintErrorKind::InvalidZoneIndex(value)),
    }
  }
}

#[derive(Default, Debug)]
pub struct Zone<'a> {
  pub unscaled: &'a [Point<i32>],

  pub original: &'a mut [Point<F26Dot6>],

  pub points: &'a mut [Point<F26Dot6>],
  pub flags: &'a mut [PointFlags],
  pub contours: &'a [u16],
}

impl<'a> Zone<'a> {
  pub fn new(
    unscaled: &'a [Point<i32>],
    original: &'a mut [Point<F26Dot6>],
    points: &'a mut [Point<F26Dot6>],
    flags: &'a mut [PointFlags],
    contours: &'a [u16],
  ) -> Result<Self, console_core_never::Never> {
    Ok(Self {
      unscaled,
      original,
      points,
      flags,
      contours,
    })
  }

  pub fn point(&self, index: u64) -> Result<Point<F26Dot6>, HintErrorKind> {
    self
      .points
      .get({
        let Ok(value) = console_core_number_conversion::index(index);
        value
      })
      .copied()
      .ok_or(InvalidPointIndex(index))
  }

  pub fn point_bits(&self, index: u64) -> Result<Point<i32>, HintErrorKind> {
    let point = self.point(index)?;
    let Ok(bits) = point.map(|coordinate| {
      let Ok(bits) = coordinate.to_bits();
      bits
    });
    Ok(bits)
  }

  pub fn point_mut(&mut self, index: u64) -> Result<&mut Point<F26Dot6>, HintErrorKind> {
    self
      .points
      .get_mut({
        let Ok(value) = console_core_number_conversion::index(index);
        value
      })
      .ok_or(InvalidPointIndex(index))
  }

  pub fn original(&self, index: u64) -> Result<Point<F26Dot6>, HintErrorKind> {
    self
      .original
      .get({
        let Ok(value) = console_core_number_conversion::index(index);
        value
      })
      .copied()
      .ok_or(InvalidPointIndex(index))
  }

  pub fn original_mut(&mut self, index: u64) -> Result<&mut Point<F26Dot6>, HintErrorKind> {
    self
      .original
      .get_mut({
        let Ok(value) = console_core_number_conversion::index(index);
        value
      })
      .ok_or(InvalidPointIndex(index))
  }

  pub fn unscaled(&self, index: u64) -> Result<Point<i32>, console_core_never::Never> {
    Ok(
      match self
        .unscaled
        .get({
          let Ok(value) = console_core_number_conversion::index(index);
          value
        })
        .copied()
      {
        Some(point) => point,
        None => Point { x: 0, y: 0 },
      },
    )
  }

  pub fn contour(&self, index: u64) -> Result<u16, HintErrorKind> {
    self
      .contours
      .get({
        let Ok(value) = console_core_number_conversion::index(index);
        value
      })
      .copied()
      .ok_or(HintErrorKind::InvalidContourIndex(index))
  }

  pub fn touch(&mut self, index: u64, axis: CoordAxis) -> Result<(), HintErrorKind> {
    let flag = self
      .flags
      .get_mut({
        let Ok(value) = console_core_number_conversion::index(index);
        value
      })
      .ok_or(InvalidPointIndex(index))?;
    flag.set_marker({
      let Ok(value) = axis.touched_marker();
      value
    });
    Ok(())
  }

  pub fn untouch(&mut self, index: u64, axis: CoordAxis) -> Result<(), HintErrorKind> {
    let flag = self
      .flags
      .get_mut({
        let Ok(value) = console_core_number_conversion::index(index);
        value
      })
      .ok_or(InvalidPointIndex(index))?;
    flag.clear_marker({
      let Ok(value) = axis.touched_marker();
      value
    });
    Ok(())
  }

  pub fn point_touch(&self, index: u64, axis: CoordAxis) -> Result<PointTouch, HintErrorKind> {
    let flag = self
      .flags
      .get({
        let Ok(value) = console_core_number_conversion::index(index);
        value
      })
      .ok_or(InvalidPointIndex(index))?;
    let Ok(marker) = axis.touched_marker();

    Ok(match flag.marker_presence(marker) {
      read_fonts::Presence::Present => PointTouch::Touched,
      read_fonts::Presence::Absent => PointTouch::Untouched,
    })
  }

  pub fn flip_on_curve(&mut self, index: u64) -> Result<(), HintErrorKind> {
    let flag = self
      .flags
      .get_mut({
        let Ok(value) = console_core_number_conversion::index(index);
        value
      })
      .ok_or(InvalidPointIndex(index))?;
    flag.flip_on_curve();
    Ok(())
  }

  pub fn set_on_curve(
    &mut self,
    range: core::ops::Range<u64>,
    curve: crate::CurveKind,
  ) -> Result<(), HintErrorKind> {
    let core::ops::Range { start, end } = range;
    let flags = self
      .flags
      .get_mut({
        let Ok(value) = console_core_number_conversion::indices(start..end);
        value
      })
      .ok_or(InvalidPointRange(start, end))?;

    match curve {
      crate::CurveKind::On => {
        for flag in flags {
          flag.set_on_curve();
        }
      }
      crate::CurveKind::Off => {
        for flag in flags {
          flag.clear_on_curve();
        }
      }
    }

    Ok(())
  }

  pub fn iup(&mut self, axis: CoordAxis) -> Result<(), HintErrorKind> {
    let mut first_point = 0;
    let Ok(point_count) = console_core_number_conversion::fitted::<_, u64>(self.points.len());

    for &contour_end in self.contours {
      let declared_end = u64::from(contour_end);
      let end_point = match point_count {
        0 => u64::MAX,
        _ => declared_end.min(point_count.wrapping_sub(1)),
      };

      match end_point.cmp(&first_point) {
        core::cmp::Ordering::Less => continue,
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {}
      }

      self.iup_contour(axis, first_point..=end_point)?;
      first_point = end_point.wrapping_add(1);
    }

    Ok(())
  }

  fn iup_contour(
    &mut self,
    axis: CoordAxis,
    points: core::ops::RangeInclusive<u64>,
  ) -> Result<(), HintErrorKind> {
    let first_point = *points.start();
    let end_point = *points.end();
    let mut touched = None;

    for point in points {
      let touch = self.point_touch(point, axis)?;

      match touch {
        PointTouch::Touched => {
          touched = Some(point);
          break;
        }
        PointTouch::Untouched => {}
      }
    }

    let first_touched = match touched {
      Some(point) => point,
      None => return Ok(()),
    };
    let mut previous_touched = first_touched;

    for point in first_touched.wrapping_add(1)..=end_point {
      let touch = self.point_touch(point, axis)?;

      match touch {
        PointTouch::Touched => {
          self.iup_interpolate(
            axis,
            InterpolationRegion {
              start: previous_touched.wrapping_add(1),
              end: point.wrapping_sub(1),
              first_reference: previous_touched,
              second_reference: point,
            },
          )?;
          previous_touched = point;
        }
        PointTouch::Untouched => {}
      }
    }

    match (previous_touched == first_touched).then_some(()) {
      Some(()) => {
        return self.iup_shift(
          axis,
          ShiftRegion {
            start: first_point,
            end: end_point,
            reference: first_touched,
          },
        );
      }
      None => {}
    }

    self.iup_interpolate(
      axis,
      InterpolationRegion {
        start: previous_touched.wrapping_add(1),
        end: end_point,
        first_reference: previous_touched,
        second_reference: first_touched,
      },
    )?;

    match first_touched {
      0 => {}
      _ => {
        self.iup_interpolate(
          axis,
          InterpolationRegion {
            start: first_point,
            end: first_touched.wrapping_sub(1),
            first_reference: previous_touched,
            second_reference: first_touched,
          },
        )?;
      }
    }

    Ok(())
  }

  fn iup_shift(&mut self, axis: CoordAxis, region: ShiftRegion) -> Result<(), HintErrorKind> {
    let ShiftRegion {
      start,
      end,
      reference,
    } = region;

    match ::core::primitive::bool::then_some(start > end || start > reference || reference > end, ()) {
      ::core::option::Option::Some(()) => return Ok(()),
      ::core::option::Option::None => {}
    }

    let current = self.point(reference)?;
    let original = self.original(reference)?;
    let Ok(current_2) = scaled_coordinate(current, axis);
    let Ok(original_2) = scaled_coordinate(original, axis);
    let delta = current_2 - original_2;

    match ::core::primitive::bool::then_some(delta != F26Dot6::ZERO, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(range) = console_core_number_conversion::indices_inclusive(start..=end);
        let points = self
          .points
          .get_mut(range)
          .ok_or(InvalidPointRange(start, end.wrapping_add(1)))?;
        let Ok(offset) = console_core_number_conversion::index(reference.wrapping_sub(start));
        let (first, second) = points.split_at_mut(offset);
        let second_2 = second
          .get_mut(1..)
          .ok_or(InvalidPointIndex(reference.wrapping_sub(start)))?;

        for point in first.iter_mut().chain(second_2) {
          let Ok(current_3) = scaled_coordinate(*point, axis);
          let Ok(()) = set_scaled_coordinate(point, axis, current_3 + delta);
        }
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  fn iup_interpolate(
    &mut self,
    axis: CoordAxis,
    region: InterpolationRegion,
  ) -> Result<(), HintErrorKind> {
    let InterpolationRegion {
      start,
      end,
      first_reference: mut ref1,
      second_reference: mut ref2,
    } = region;
    let Ok(max_points) = console_core_number_conversion::fitted::<_, u64>(self.points.len());

    match ::core::primitive::bool::then_some(start > end || ref1 >= max_points || ref2 >= max_points, ()) {
      ::core::option::Option::Some(()) => return Ok(()),
      ::core::option::Option::None => {}
    }

    let Ok(unscaled1) = self.unscaled(ref1);
    let Ok(unscaled2) = self.unscaled(ref2);
    let Ok(mut unscaled1_2) = unscaled_coordinate(unscaled1, axis);
    let Ok(mut unscaled2_2) = unscaled_coordinate(unscaled2, axis);

    match ::core::primitive::bool::then_some(unscaled1_2 > unscaled2_2, ()) {
      ::core::option::Option::Some(()) => {
        core::mem::swap(&mut unscaled1_2, &mut unscaled2_2);
        core::mem::swap(&mut ref1, &mut ref2);
      }
      ::core::option::Option::None => {}
    }

    let original1 = self.original(ref1)?;
    let original2 = self.original(ref2)?;
    let current1 = self.point(ref1)?;
    let current2 = self.point(ref2)?;
    let Ok(original1_2) = scaled_coordinate(original1, axis);
    let Ok(original2_2) = scaled_coordinate(original2, axis);
    let Ok(current1_2) = scaled_coordinate(current1, axis);
    let Ok(current2_2) = scaled_coordinate(current2, axis);
    let Ok(interpolation) = CoordinateInterpolation::new(CoordinateAnchors {
      original: [original1_2, original2_2],
      current: [current1_2, current2_2],
      unscaled: [unscaled1_2, unscaled2_2],
    });
    let Ok(range) = console_core_number_conversion::indices_inclusive(start..=end);
    let invalid_range = InvalidPointRange(start, end.wrapping_add(1));
    let originals = self
      .original
      .get(range.clone())
      .ok_or(invalid_range.clone())?;
    let unscaled = self
      .unscaled
      .get(range.clone())
      .ok_or(invalid_range.clone())?;
    let points = self.points.get_mut(range).ok_or(invalid_range)?;

    for ((original, unscaled_2), point) in originals.iter().zip(unscaled).zip(points) {
      let Ok(original_2) = scaled_coordinate(*original, axis);
      let Ok(unscaled_3) = unscaled_coordinate(*unscaled_2, axis);
      let Ok(coordinate) = interpolation.apply(CoordinateSample {
        original: original_2,
        unscaled: unscaled_3,
      });
      let Ok(()) = set_scaled_coordinate(point, axis, coordinate);
    }

    Ok(())
  }
}

impl<'a> GraphicsState<'a> {
  pub fn point_bounds<const N: usize>(
    &self,
    pairs: [(ZonePointer, u64); N],
  ) -> Result<crate::PointBounds, Never> {
    for (zp, index) in pairs {
      match ::core::primitive::bool::then_some(index
        > ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            {
              let Ok(value) = self.zone(zp);
              value
            }
            .points
            .len(),
          );
          value
        }), ()) {
        ::core::option::Option::Some(()) => {
          return Ok(crate::PointBounds::Outside);
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(crate::PointBounds::Within)
  }

  #[inline(always)]
  pub fn zone(&self, pointer: ZonePointer) -> Result<&Zone<'a>, console_core_never::Never> {
    let [twilight, glyph] = &self.zones;

    Ok(match pointer {
      ZonePointer::Twilight => twilight,
      ZonePointer::Glyph => glyph,
    })
  }

  #[inline(always)]
  pub fn zone_mut(
    &mut self,
    pointer: ZonePointer,
  ) -> Result<&mut Zone<'a>, console_core_never::Never> {
    let [twilight, glyph] = &mut self.zones;

    Ok(match pointer {
      ZonePointer::Twilight => twilight,
      ZonePointer::Glyph => glyph,
    })
  }

  #[inline(always)]
  pub fn zp0(&self) -> Result<&Zone<'a>, console_core_never::Never> {
    Ok({
      let Ok(value) = self.zone(self.zp0);
      value
    })
  }

  #[inline(always)]
  pub fn zp0_mut(&mut self) -> Result<&mut Zone<'a>, console_core_never::Never> {
    Ok({
      let Ok(value) = self.zone_mut(self.zp0);
      value
    })
  }

  #[inline(always)]
  pub fn zp1(&self) -> Result<&Zone<'_>, console_core_never::Never> {
    Ok({
      let Ok(value) = self.zone(self.zp1);
      value
    })
  }

  #[inline(always)]
  pub fn zp1_mut(&mut self) -> Result<&mut Zone<'a>, console_core_never::Never> {
    Ok({
      let Ok(value) = self.zone_mut(self.zp1);
      value
    })
  }

  #[inline(always)]
  pub fn zp2(&self) -> Result<&Zone<'_>, console_core_never::Never> {
    Ok({
      let Ok(value) = self.zone(self.zp2);
      value
    })
  }

  #[inline(always)]
  pub fn zp2_mut(&mut self) -> Result<&mut Zone<'a>, console_core_never::Never> {
    Ok({
      let Ok(value) = self.zone_mut(self.zp2);
      value
    })
  }
}

fn displacement_on_axis(ratio: math::ProductRatio) -> Result<F26Dot6, console_core_never::Never> {
  let Ok(bits) = math::mul_div(ratio);
  F26Dot6::from_bits(bits)
}

impl GraphicsState<'_> {
  pub(crate) fn move_original(
    &mut self,
    zone: ZonePointer,
    point_ix: u64,
    distance: F26Dot6,
  ) -> Result<(), HintErrorKind> {
    let fv = self.freedom_vector;
    let fdotp = self.fdotp;
    let axis = self.freedom_axis;
    let Ok(target) = self.zone_mut(zone);
    let point = target.original_mut(point_ix)?;

    match axis {
      CoordAxis::X => point.x += distance,
      CoordAxis::Y => point.y += distance,
      CoordAxis::Both => {
        let Ok(bits) = distance.to_bits();

        match fv.x {
          0 => {}
          _ => {
            let Ok(delta) = displacement_on_axis(math::ProductRatio {
              value: bits,
              multiplier: fv.x,
              divisor: fdotp,
            });
            point.x += delta;
          }
        }

        match fv.y {
          0 => {}
          _ => {
            let Ok(delta) = displacement_on_axis(math::ProductRatio {
              value: bits,
              multiplier: fv.y,
              divisor: fdotp,
            });
            point.y += delta;
          }
        }
      }
    }

    Ok(())
  }

  pub(crate) fn move_point(
    &mut self,
    zone: ZonePointer,
    point_ix: u64,
    distance: F26Dot6,
  ) -> Result<(), HintErrorKind> {
    let compatible = matches!(
      self.backward_compatibility,
      crate::BackwardCompatibility::Enabled
    );
    let compatible_after_interpolation = compatible
      && matches!(self.did_iup_x, crate::InterpolationProgress::Applied)
      && matches!(self.did_iup_y, crate::InterpolationProgress::Applied);
    let [twilight, glyph] = &mut self.zones;
    let target = match zone {
      ZonePointer::Twilight => twilight,
      ZonePointer::Glyph => glyph,
    };
    let point = target.point_mut(point_ix)?;

    match self.freedom_axis {
      CoordAxis::X => {
        match (!compatible).then_some(()) {
          Some(()) => point.x += distance,
          None => {}
        }

        target.touch(point_ix, CoordAxis::X)?;
      }
      CoordAxis::Y => {
        match (!compatible_after_interpolation).then_some(()) {
          Some(()) => point.y += distance,
          None => {}
        }

        target.touch(point_ix, CoordAxis::Y)?;
      }
      CoordAxis::Both => {
        let fv = self.freedom_vector;
        let Ok(bits) = distance.to_bits();

        match (fv.x, compatible) {
          (0, _) | (_, true) => {}
          (multiplier, false) => {
            let Ok(delta) = displacement_on_axis(math::ProductRatio {
              value: bits, multiplier, divisor: self.fdotp,
            });
            point.x += delta;
          }
        }

        match fv.x {
          0 => {}
          _ => target.touch(point_ix, CoordAxis::X)?,
        }

        match (fv.y, compatible_after_interpolation) {
          (0, _) | (_, true) => {}
          (multiplier, false) => {
            let current = target.point_mut(point_ix)?;
            let Ok(delta) = displacement_on_axis(math::ProductRatio {
              value: bits, multiplier, divisor: self.fdotp,
            });
            current.y += delta;
          }
        }

        match fv.y {
          0 => {}
          _ => target.touch(point_ix, CoordAxis::Y)?,
        }
      }
    }

    Ok(())
  }

  pub(crate) fn move_zp2_point(
    &mut self,
    point_ix: u64,
    dx: F26Dot6,
    dy: F26Dot6,
    touch: Touch,
  ) -> Result<(), HintErrorKind> {
    let back_compat = self.backward_compatibility;
    let back_compat_and_did_iup = matches!(back_compat, crate::BackwardCompatibility::Enabled)
      && matches!(self.did_iup_x, crate::InterpolationProgress::Applied)
      && matches!(self.did_iup_y, crate::InterpolationProgress::Applied);
    let fv = self.freedom_vector;
    let Ok(zone) = self.zp2_mut();

    match ::core::primitive::bool::then_some(fv.x != 0, ()) {
      ::core::option::Option::Some(()) => {
        match ::core::primitive::bool::then_some(!matches!(back_compat, crate::BackwardCompatibility::Enabled), ()) {
          ::core::option::Option::Some(()) => {
            {
              let answer = zone.point_mut(point_ix)?;
              answer
            }
            .x += dx;
          }
          ::core::option::Option::None => {}
        }

        match touch {
          crate::truetype::graphics::Touch::Mark => {
            zone.touch(point_ix, CoordAxis::X)?;
          }
          crate::truetype::graphics::Touch::Keep => {}
        }
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(fv.y != 0, ()) {
      ::core::option::Option::Some(()) => {
        match ::core::primitive::bool::then_some(!back_compat_and_did_iup, ()) {
          ::core::option::Option::Some(()) => {
            {
              let answer = zone.point_mut(point_ix)?;
              answer
            }
            .y += dy;
          }
          ::core::option::Option::None => {}
        }

        match touch {
          crate::truetype::graphics::Touch::Mark => {
            zone.touch(point_ix, CoordAxis::Y)?;
          }
          crate::truetype::graphics::Touch::Keep => {}
        }
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  pub(crate) fn point_displacement(
    &mut self,
    opcode: u8,
  ) -> Result<PointDisplacement, HintErrorKind> {
    let (zone, point_ix) = match ::core::primitive::bool::then_some((opcode & 1) != 0, ()) {
      ::core::option::Option::Some(()) => (self.zp0, self.rp1),
      ::core::option::Option::None => (self.zp1, self.rp2),
    };
    let Ok(zone_data) = self.zone(zone);
    let point = zone_data.point(point_ix)?;
    let original_point = zone_data.original(point_ix)?;
    let Ok(distance) = self.project(point, original_point);
    let fv = self.freedom_vector;
    let Ok(dx) = F26Dot6::from_bits({
      let Ok(math_result_5) = math::mul_div(math::ProductRatio {
        value: {
          let Ok(value) = distance.to_bits();
          value
        },
        multiplier: fv.x,
        divisor: self.fdotp,
      });
      math_result_5
    });
    let Ok(dy) = F26Dot6::from_bits({
      let Ok(math_result_6) = math::mul_div(math::ProductRatio {
        value: {
          let Ok(value) = distance.to_bits();
          value
        },
        multiplier: fv.y,
        divisor: self.fdotp,
      });
      math_result_6
    });
    Ok(PointDisplacement {
      zone,
      point_ix,
      dx: (dx),
      dy: (dy),
    })
  }
}

#[derive(PartialEq, Debug)]
pub(crate) struct PointDisplacement {
  pub zone: ZonePointer,
  pub point_ix: u64,
  pub dx: F26Dot6,
  pub dy: F26Dot6,
}

impl CoordAxis {
  fn touched_marker(self) -> Result<PointMarker, console_core_never::Never> {
    Ok(match self {
      CoordAxis::Both => PointMarker::TOUCHED,
      CoordAxis::X => PointMarker::TOUCHED_X,
      CoordAxis::Y => PointMarker::TOUCHED_Y,
    })
  }
}

fn scaled_coordinate(point: Point<F26Dot6>, axis: CoordAxis) -> Result<F26Dot6, Never> {
  Ok(match ::core::primitive::bool::then_some(axis == CoordAxis::X, ()) {
    ::core::option::Option::Some(()) => point.x,
    ::core::option::Option::None => point.y,
  })
}

fn unscaled_coordinate(point: Point<i32>, axis: CoordAxis) -> Result<i32, Never> {
  Ok(match ::core::primitive::bool::then_some(axis == CoordAxis::X, ()) {
    ::core::option::Option::Some(()) => point.x,
    ::core::option::Option::None => point.y,
  })
}

fn set_scaled_coordinate(
  point: &mut Point<F26Dot6>,
  axis: CoordAxis,
  coordinate: F26Dot6,
) -> Result<(), Never> {
  match ::core::primitive::bool::then_some(axis == CoordAxis::X, ()) {
    ::core::option::Option::Some(()) => point.x = coordinate,
    ::core::option::Option::None => point.y = coordinate,
  }

  Ok(())
}

struct CoordinateAnchors {
  original: [F26Dot6; 2],
  current: [F26Dot6; 2],
  unscaled: [i32; 2],
}

struct CoordinateSample {
  original: F26Dot6,
  unscaled: i32,
}

enum CoordinateScale {
  Constant,
  Scaled(i32),
}

struct CoordinateInterpolation {
  anchors: CoordinateAnchors,
  scale: CoordinateScale,
}

impl CoordinateInterpolation {
  fn new(anchors: CoordinateAnchors) -> Result<Self, Never> {
    let [current1, current2] = anchors.current;
    let [unscaled1, unscaled2] = anchors.unscaled;
    let scale = match ::core::primitive::bool::then_some(current1 == current2 || unscaled1 == unscaled2, ()) {
      ::core::option::Option::Some(()) => CoordinateScale::Constant,
      ::core::option::Option::None => {
        let Ok(numerator) = (current2 - current1).to_bits();
        let denominator = unscaled2.wrapping_sub(unscaled1);
        let Ok(scale) = math::div(math::Ratio {
          numerator,
          denominator,
        });
        CoordinateScale::Scaled(scale)
      }
    };
    Ok(Self { anchors, scale })
  }

  fn apply(&self, sample: CoordinateSample) -> Result<F26Dot6, Never> {
    let [original1, original2] = self.anchors.original;
    let [current1, current2] = self.anchors.current;
    Ok(match ::core::primitive::bool::then_some(sample.original <= original1, ()) {
      ::core::option::Option::Some(()) => sample.original + (current1 - original1),
      ::core::option::Option::None => match ::core::primitive::bool::then_some(sample.original >= original2, ()) {
        ::core::option::Option::Some(()) => sample.original + (current2 - original2),
        ::core::option::Option::None => match self.scale {
          CoordinateScale::Constant => current1,
          CoordinateScale::Scaled(scale) => {
            let Ok(delta) = math::mul(math::Product {
              left: {
                let [unscaled1, _unscaled2] = self.anchors.unscaled;
                sample.unscaled.wrapping_sub(unscaled1)
              },
              right: scale,
            });
            let Ok(delta_2) = F26Dot6::from_bits(delta);
            current1 + delta_2
          }
        },
      },
    })
  }
}

#[cfg(test)]
mod tests {
  use super::{CoordAxis, GraphicsState, PointDisplacement, Zone, ZonePointer, math};
  use raw::{
    tables::glyf::{PointFlags, PointMarker},
    types::{F26Dot6, Point},
  };

  #[test]
  fn flip_on_curve_point() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let on_curve = PointFlags::on_curve();
    let off_curve = PointFlags::off_curve_quad();
    let mut zone = Zone {
      unscaled: &mut [],
      original: &mut [],
      points: &mut [],
      contours: &[],
      flags: &mut [on_curve, off_curve, off_curve, on_curve],
    };

    for i in 0..4 {
      zone.flip_on_curve(i).required()?;
    }

    assert_eq!(zone.flags, &[off_curve, on_curve, on_curve, off_curve]);

    Ok(())
  }

  #[test]
  fn set_on_curve_regions() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let on_curve = PointFlags::on_curve();
    let off_curve = PointFlags::off_curve_quad();
    let mut zone = Zone {
      unscaled: &mut [],
      original: &mut [],
      points: &mut [],
      contours: &[],
      flags: &mut [on_curve, off_curve, off_curve, on_curve],
    };
    zone
      .set_on_curve(
        0..2,
        match ::core::primitive::bool::then_some(true, ()) {
          ::core::option::Option::Some(()) => crate::CurveKind::On,
          ::core::option::Option::None => crate::CurveKind::Off,
        },
      )
      .required()?;
    zone
      .set_on_curve(
        2..4,
        match ::core::primitive::bool::then_some(false, ()) {
          ::core::option::Option::Some(()) => crate::CurveKind::On,
          ::core::option::Option::None => crate::CurveKind::Off,
        },
      )
      .required()?;
    assert_eq!(zone.flags, &[on_curve, on_curve, off_curve, off_curve]);

    Ok(())
  }

  #[test]
  fn iup_shift() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let [untouched, touched] = {
      let Ok(migrated_value) = point_markers();
      migrated_value
    };

    let mut original = {
      let Ok(migrated_value) = f26dot6_points([(0, 0), (10, 10), (20, 20)]);
      migrated_value
    };
    let mut points = {
      let Ok(migrated_value) = f26dot6_points([(-5, -20), (10, 10), (20, 20)]);
      migrated_value
    };
    let mut zone = Zone {
      unscaled: &mut [],
      original: &mut original,
      points: &mut points,
      contours: &[3],
      flags: &mut [touched, untouched, untouched],
    };
    zone.iup(CoordAxis::X).required()?;
    assert_eq!(
      zone.points,
      &({
        let Ok(migrated_value) = f26dot6_points([(-5, -20), (5, 10), (15, 20)]);
        migrated_value
      }),
    );
    zone.iup(CoordAxis::Y).required()?;
    assert_eq!(
      zone.points,
      &({
        let Ok(migrated_value) = f26dot6_points([(-5, -20), (5, -10), (15, 0)]);
        migrated_value
      }),
    );

    Ok(())
  }

  #[test]
  fn iup_interpolate() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let [untouched, touched] = {
      let Ok(migrated_value) = point_markers();
      migrated_value
    };

    let mut original = {
      let Ok(migrated_value) = f26dot6_points([(0, 0), (10, 10), (20, 20)]);
      migrated_value
    };
    let mut points = {
      let Ok(migrated_value) = f26dot6_points([(-5, -20), (10, 10), (27, 56)]);
      migrated_value
    };
    let mut zone = Zone {
      unscaled: &mut [
        ({
          let Ok(value) = Point::new(0, 0);
          value
        }),
        ({
          let Ok(value) = Point::new(500, 500);
          value
        }),
        ({
          let Ok(value) = Point::new(1000, 1000);
          value
        }),
      ],
      original: &mut original,
      points: &mut points,
      contours: &[3],
      flags: &mut [touched, untouched, touched],
    };
    zone.iup(CoordAxis::X).required()?;
    assert_eq!(
      zone.points,
      &({
        let Ok(migrated_value) = f26dot6_points([(-5, -20), (11, 10), (27, 56)]);
        migrated_value
      }),
    );
    zone.iup(CoordAxis::Y).required()?;
    assert_eq!(
      zone.points,
      &({
        let Ok(migrated_value) = f26dot6_points([(-5, -20), (11, 18), (27, 56)]);
        migrated_value
      }),
    );

    Ok(())
  }

  #[test]
  fn move_point_x() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = {
      let Ok(migrated_value) = MockGraphicsState::new();
      migrated_value
    };
    let mut gs = {
      let Ok(migrated_value) = mock.graphics_state(Point { x: 100, y: 0 });
      migrated_value
    };
    let point_ix = 0;
    let orig_x = {
      let queried_value = (*match &gs.zones {
        [_, element, ..] => element,
      })
      .point(point_ix)
      .required()?;
      queried_value.x
    };
    let Ok(dx) = F26Dot6::from_bits(10);

    gs.move_point(ZonePointer::Glyph, 0, dx).required()?;
    let required = (*match &gs.zones {
      [_, element, ..] => element,
    })
    .point(point_ix)
    .required()?;
    assert_eq!(orig_x, required.x);

    gs.backward_compatibility = crate::BackwardCompatibility::Disabled;
    gs.move_point(ZonePointer::Glyph, 0, dx).required()?;
    let new_x = {
      let queried_value = (*match &gs.zones {
        [_, element, ..] => element,
      })
      .point(point_ix)
      .required()?;
      queried_value.x
    };
    assert_ne!(orig_x, new_x);

    assert_eq!(new_x, orig_x + dx);
    Ok(())
  }

  #[test]
  fn move_point_y() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = {
      let Ok(migrated_value) = MockGraphicsState::new();
      migrated_value
    };
    let mut gs = {
      let Ok(migrated_value) = mock.graphics_state(Point { x: 0, y: 100 });
      migrated_value
    };
    let point_ix = 0;
    let orig_y = {
      let queried_value = (*match &gs.zones {
        [_, element, ..] => element,
      })
      .point(point_ix)
      .required()?;
      queried_value.y
    };
    let Ok(dy) = F26Dot6::from_bits(10);

    gs.did_iup_x = crate::InterpolationProgress::Applied;
    gs.did_iup_y = crate::InterpolationProgress::Applied;
    gs.move_point(ZonePointer::Glyph, 0, dy).required()?;
    let required = (*match &gs.zones {
      [_, element, ..] => element,
    })
    .point(point_ix)
    .required()?;
    assert_eq!(orig_y, required.y);

    gs.did_iup_x = crate::InterpolationProgress::Pending;
    gs.did_iup_y = crate::InterpolationProgress::Pending;
    gs.move_point(ZonePointer::Glyph, 0, dy).required()?;
    let new_y = {
      let queried_value = (*match &gs.zones {
        [_, element, ..] => element,
      })
      .point(point_ix)
      .required()?;
      queried_value.y
    };
    assert_ne!(orig_y, new_y);

    assert_eq!(new_y, orig_y + dy);
    Ok(())
  }

  #[test]
  fn move_point_x_and_y() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = {
      let Ok(migrated_value) = MockGraphicsState::new();
      migrated_value
    };
    let mut gs = {
      let Ok(migrated_value) = mock.graphics_state(Point { x: 100, y: 50 });
      migrated_value
    };
    let point_ix = 0;
    let orig_point = (*match &gs.zones {
      [_, element, ..] => element,
    })
    .point(point_ix)
    .required()?;
    let Ok(dist) = F26Dot6::from_bits(10);

    gs.did_iup_x = crate::InterpolationProgress::Applied;
    gs.did_iup_y = crate::InterpolationProgress::Applied;
    gs.move_point(ZonePointer::Glyph, 0, dist).required()?;
    let required = (*match &gs.zones {
      [_, element, ..] => element,
    })
    .point(point_ix)
    .required()?;
    assert_eq!(orig_point, required);

    gs.backward_compatibility = crate::BackwardCompatibility::Disabled;
    gs.did_iup_x = crate::InterpolationProgress::Pending;
    gs.did_iup_y = crate::InterpolationProgress::Pending;
    gs.move_point(ZonePointer::Glyph, 0, dist).required()?;
    let point = (*match &gs.zones {
      [_, element, ..] => element,
    })
    .point(point_ix)
    .required()?;
    assert_eq!(
      ({
        let Ok(mapped) = point.map(|coordinate| {
          let Ok(bits) = coordinate.to_bits();
          bits
        });
        mapped
      }),
      ({
        let Ok(value) = Point::new(4, -16);
        value
      })
    );

    Ok(())
  }

  #[test]
  fn move_original_x() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = {
      let Ok(migrated_value) = MockGraphicsState::new();
      migrated_value
    };
    let mut gs = {
      let Ok(migrated_value) = mock.graphics_state(Point { x: 100, y: 0 });
      migrated_value
    };
    let point_ix = 0;
    let orig_x = {
      let queried_value = (*match &gs.zones {
        [_, element, ..] => element,
      })
      .original(point_ix)
      .required()?;
      queried_value.x
    };
    let Ok(dx) = F26Dot6::from_bits(10);
    gs.move_original(ZonePointer::Glyph, 0, dx).required()?;
    let new_x = {
      let queried_value = (*match &gs.zones {
        [_, element, ..] => element,
      })
      .original(point_ix)
      .required()?;
      queried_value.x
    };

    assert_eq!(new_x, orig_x + dx);
    Ok(())
  }

  #[test]
  fn move_original_y() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = {
      let Ok(migrated_value) = MockGraphicsState::new();
      migrated_value
    };
    let mut gs = {
      let Ok(migrated_value) = mock.graphics_state(Point { x: 0, y: 100 });
      migrated_value
    };
    let point_ix = 0;
    let orig_y = {
      let queried_value = (*match &gs.zones {
        [_, element, ..] => element,
      })
      .original(point_ix)
      .required()?;
      queried_value.y
    };
    let Ok(dy) = F26Dot6::from_bits(10);
    gs.move_original(ZonePointer::Glyph, 0, dy).required()?;
    let new_y = {
      let queried_value = (*match &gs.zones {
        [_, element, ..] => element,
      })
      .original(point_ix)
      .required()?;
      queried_value.y
    };

    assert_eq!(new_y, orig_y + dy);
    Ok(())
  }

  #[test]
  fn move_original_x_and_y() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = {
      let Ok(migrated_value) = MockGraphicsState::new();
      migrated_value
    };
    let mut gs = {
      let Ok(migrated_value) = mock.graphics_state(Point { x: 100, y: 50 });
      migrated_value
    };
    let point_ix = 0;
    let Ok(dist) = F26Dot6::from_bits(10);
    gs.move_original(ZonePointer::Glyph, 0, dist).required()?;
    let point = (*match &gs.zones {
      [_, element, ..] => element,
    })
    .original(point_ix)
    .required()?;
    assert_eq!(
      ({
        let Ok(mapped) = point.map(|coordinate| {
          let Ok(bits) = coordinate.to_bits();
          bits
        });
        mapped
      }),
      ({
        let Ok(value) = Point::new(9, 4);
        value
      })
    );

    Ok(())
  }

  #[test]
  fn move_zp2_point() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = {
      let Ok(migrated_value) = MockGraphicsState::new();
      migrated_value
    };
    let mut gs = {
      let Ok(migrated_value) = mock.graphics_state(Point { x: 100, y: 50 });
      migrated_value
    };
    gs.zp2 = ZonePointer::Glyph;
    let point_ix = 0;
    let orig_point = (*match &gs.zones {
      [_, element, ..] => element,
    })
    .point(point_ix)
    .required()?;
    let Ok(dx) = F26Dot6::from_bits(10);
    let Ok(dy) = F26Dot6::from_bits(-10);

    gs.did_iup_x = crate::InterpolationProgress::Applied;
    gs.did_iup_y = crate::InterpolationProgress::Applied;
    gs.move_zp2_point(
      point_ix,
      dx,
      dy,
      match ::core::primitive::bool::then_some(false, ()) {
        ::core::option::Option::Some(()) => crate::truetype::graphics::Touch::Mark,
        ::core::option::Option::None => crate::truetype::graphics::Touch::Keep,
      },
    )
    .required()?;
    let required = (*match &gs.zones {
      [_, element, ..] => element,
    })
    .point(point_ix)
    .required()?;
    assert_eq!(orig_point, required);

    gs.backward_compatibility = crate::BackwardCompatibility::Disabled;
    gs.did_iup_x = crate::InterpolationProgress::Pending;
    gs.did_iup_y = crate::InterpolationProgress::Pending;
    gs.move_zp2_point(
      point_ix,
      dx,
      dy,
      match ::core::primitive::bool::then_some(false, ()) {
        ::core::option::Option::Some(()) => crate::truetype::graphics::Touch::Mark,
        ::core::option::Option::None => crate::truetype::graphics::Touch::Keep,
      },
    )
    .required()?;
    let point = (*match &gs.zones {
      [_, element, ..] => element,
    })
    .point(point_ix)
    .required()?;
    assert_eq!(
      point,
      orig_point
        + ({
          let Ok(value) = Point::new(dx, dy);
          value
        })
    );

    Ok(())
  }

  #[test]
  fn point_displacement() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = {
      let Ok(migrated_value) = MockGraphicsState::new();
      migrated_value
    };
    let mut gs = {
      let Ok(migrated_value) = mock.graphics_state(Point { x: 100, y: 50 });
      migrated_value
    };
    gs.zp0 = ZonePointer::Glyph;
    gs.rp1 = 0;

    {
      let queried_value = gs.point_displacement(1).required()?;
      assert_eq!(
        queried_value,
        PointDisplacement {
          zone: ZonePointer::Glyph,
          point_ix: 0,
          dx: ({
            let Ok(value) = F26Dot6::from_f64(-0.1875);
            value
          }),
          dy: ({
            let Ok(value) = F26Dot6::from_f64(-0.09375);
            value
          }),
        }
      );
    }

    gs.rp2 = 2;

    {
      let queried_value = gs.point_displacement(0).required()?;
      assert_eq!(
        queried_value,
        PointDisplacement {
          zone: ZonePointer::Glyph,
          point_ix: 2,
          dx: ({
            let Ok(value) = F26Dot6::from_f64(0.390625);
            value
          }),
          dy: ({
            let Ok(value) = F26Dot6::from_f64(0.203125);
            value
          }),
        }
      );
    }

    Ok(())
  }

  struct MockGraphicsState {
    points: [Point<F26Dot6>; 3],
    original: [Point<F26Dot6>; 3],
    contours: [u16; 1],
    flags: [PointFlags; 3],
  }

  impl MockGraphicsState {
    fn new() -> core::result::Result<Self, console_core_never::Never> {
      Ok(Self {
        points: ({
          let Ok(migrated_value) = f26dot6_points([(-5, -20), (10, 10), (20, 20)]);
          migrated_value
        }),
        original: ({
          let Ok(migrated_value) = f26dot6_points([(0, 0), (10, 10), (20, -42)]);
          migrated_value
        }),
        flags: [PointFlags::default(); 3],
        contours: [3],
      })
    }

    fn graphics_state(
      &mut self,
      freedom: Point<i32>,
    ) -> core::result::Result<GraphicsState<'_>, console_core_never::Never> {
      let glyph = Zone {
        unscaled: &mut [],
        original: &mut self.original,
        points: &mut self.points,
        contours: &self.contours,
        flags: &mut self.flags,
      };
      let v = {
        let Ok(math_result_7) = math::normalize14(freedom);
        math_result_7
      };
      let mut gs = GraphicsState {
        zones: [Zone::default(), glyph],
        freedom_vector: v,
        proj_vector: v,
        zp0: ZonePointer::Glyph,
        ..Default::default()
      };
      let ::core::result::Result::Ok(()) = gs.update_projection_state();

      Ok(gs)
    }
  }

  fn point_markers() -> core::result::Result<[PointFlags; 2], console_core_never::Never> {
    Ok({
      let untouched = PointFlags::default();
      let mut touched = untouched;
      touched.set_marker(PointMarker::TOUCHED);
      [untouched, touched]
    })
  }

  fn f26dot6_points<const N: usize>(
    points: [(i32, i32); N],
  ) -> core::result::Result<[Point<F26Dot6>; N], console_core_never::Never> {
    Ok(points.map(|point| {
      let Ok(value) = Point::new(
        {
          let Ok(value) = F26Dot6::from_bits(point.0);
          value
        },
        {
          let Ok(value) = F26Dot6::from_bits(point.1);
          value
        },
      );
      value
    }))
  }
}
