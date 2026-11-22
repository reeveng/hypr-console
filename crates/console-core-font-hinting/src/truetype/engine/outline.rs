//! Managing outlines.
//!
//! Implements 87 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#managing-outlines>

use super::{
  super::{
    graphics::CoordAxis,
    zone::{PointDisplacement, PointTouch, ZonePointer},
  },
  Engine, F26Dot6, HintErrorKind, OpResult, math,
};

impl Engine<'_> {
  pub(super) fn op_flippt(&mut self) -> OpResult {
    let count = {
      let Ok(value) =
        console_core_number_conversion::wrapped::<_, u64>(self.graphics.loop_counter);
      value
    };
    self.graphics.loop_counter = 1;

    match ::core::primitive::bool::then_some(matches!(
      self.graphics.backward_compatibility,
      crate::BackwardCompatibility::Enabled
    ) && matches!(
      self.graphics.did_iup_x,
      crate::InterpolationProgress::Applied
    ) && matches!(
      self.graphics.did_iup_y,
      crate::InterpolationProgress::Applied
    ), ()) {
      ::core::option::Option::Some(()) => {
        for _ in 0..count {
          self.value_stack.pop()?;
        }

        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(zone) = self.graphics.zone_mut(ZonePointer::Glyph);

    for _ in 0..count {
      let p = self.value_stack.pop_index()?;
      zone.flip_on_curve(p)?;
    }

    Ok(())
  }

  pub(super) fn op_fliprgon(&mut self) -> OpResult {
    self.set_on_curve_for_range(match ::core::primitive::bool::then_some(true, ()) {
      ::core::option::Option::Some(()) => crate::CurveKind::On,
      ::core::option::Option::None => crate::CurveKind::Off,
    })
  }

  pub(super) fn op_fliprgoff(&mut self) -> OpResult {
    self.set_on_curve_for_range(match ::core::primitive::bool::then_some(false, ()) {
      ::core::option::Option::Some(()) => crate::CurveKind::On,
      ::core::option::Option::None => crate::CurveKind::Off,
    })
  }

  pub(super) fn op_shp(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let PointDisplacement { dx, dy, .. } = gs.point_displacement(opcode)?;
    let count = gs.loop_counter;
    gs.loop_counter = 1;

    for _ in 0..count {
      let p = self.value_stack.pop_index()?;
      gs.move_zp2_point(
        p,
        dx,
        dy,
        match ::core::primitive::bool::then_some(true, ()) {
          ::core::option::Option::Some(()) => crate::truetype::graphics::Touch::Mark,
          ::core::option::Option::None => crate::truetype::graphics::Touch::Keep,
        },
      )?;
    }

    Ok(())
  }

  pub(super) fn op_shc(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let contour_ix = self.value_stack.pop_index()?;

    match ::core::primitive::bool::then_some(!matches!(gs.is_pedantic, crate::Pedantry::Strict)
      && contour_ix
        >= ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            {
              let Ok(value) = gs.zp2();
              value
            }
            .contours
            .len(),
          );
          value
        }), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let point_disp = gs.point_displacement(opcode)?;
    let start = match ::core::primitive::bool::then_some(contour_ix != 0, ()) {
      ::core::option::Option::Some(()) => u64::wrapping_add(
        {
          let Ok(value) = console_core_number_conversion::wrapped::<_, u64>({
            let answer = {
              let Ok(value) = gs.zp2();
              value
            }
            .contour(u64::wrapping_sub(contour_ix, 1))?;
            answer
          });
          value
        },
        1,
      ),
      ::core::option::Option::None => 0,
    };
    let Ok(end) = console_core_number_conversion::fitted::<_, u64>(
      match gs.zp2 {
        ZonePointer::Twilight => {
          let Ok(value) = gs.zp2();
          value
        }
        .points
        .len(),
        ZonePointer::Glyph => {
          let Ok(value) = console_core_number_conversion::index(u64::wrapping_add(
            {
              let Ok(value) = console_core_number_conversion::wrapped::<_, u64>({
                let answer = {
                  let Ok(value) = gs.zp2();
                  value
                }
                .contour(contour_ix)?;
                answer
              });
              value
            },
            1,
          ));
          value
        }
      },
    );

    for i in start..({
      let Ok(value) = console_core_number_conversion::fitted::<_, u64>(end);
      value
    }) {
      match ::core::primitive::bool::then_some(point_disp.zone != gs.zp2 || point_disp.point_ix != i, ()) {
        ::core::option::Option::Some(()) => {
          gs.move_zp2_point(
            i,
            point_disp.dx,
            point_disp.dy,
            match ::core::primitive::bool::then_some(true, ()) {
              ::core::option::Option::Some(()) => crate::truetype::graphics::Touch::Mark,
              ::core::option::Option::None => crate::truetype::graphics::Touch::Keep,
            },
          )?;
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(())
  }

  pub(super) fn op_shz(&mut self, opcode: u8) -> OpResult {
    let zone_index = self.value_stack.pop()?;
    let _validated_zone = ZonePointer::try_from(zone_index)?;
    let gs = &mut self.graphics;
    let point_disp = gs.point_displacement(opcode)?;
    let Ok(zp2) = gs.zp2();
    let end = match gs.zp2 {
      ZonePointer::Twilight => {
        let Ok(point_count) = console_core_number_conversion::fitted::<_, u64>(zp2.points.len());
        point_count
      }
      ZonePointer::Glyph => match zp2.contours.last() {
        Some(&last_point) => u64::from(last_point).wrapping_add(1),
        None => 0,
      },
    };

    for i in 0..end {
      match ::core::primitive::bool::then_some(point_disp.zone != gs.zp2
        || i
          != ({
            let Ok(value) = console_core_number_conversion::index(point_disp.point_ix);

            {
              let Ok(value_2) = console_core_number_conversion::fitted::<_, u64>(value);
              value_2
            }
          }), ()) {
        ::core::option::Option::Some(()) => {
          gs.move_zp2_point(
            {
              let Ok(value) = console_core_number_conversion::fitted::<_, u64>(i);
              value
            },
            point_disp.dx,
            point_disp.dy,
            match ::core::primitive::bool::then_some(false, ()) {
              ::core::option::Option::Some(()) => crate::truetype::graphics::Touch::Mark,
              ::core::option::Option::None => crate::truetype::graphics::Touch::Keep,
            },
          )?;
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(())
  }

  pub(super) fn op_shpix(&mut self) -> OpResult {
    let gs = &mut self.graphics;
    let in_twilight = matches!(gs.zp0, crate::truetype::zone::ZonePointer::Twilight)
      || matches!(gs.zp1, crate::truetype::zone::ZonePointer::Twilight)
      || matches!(gs.zp2, crate::truetype::zone::ZonePointer::Twilight);
    let amount = self.value_stack.pop()?;
    let Ok(dx) = F26Dot6::from_bits({
      let Ok(math_result_1) = math::mul14(math::Product {
        left: amount,
        right: gs.freedom_vector.x,
      });
      math_result_1
    });
    let Ok(dy) = F26Dot6::from_bits({
      let Ok(math_result_2) = math::mul14(math::Product {
        left: amount,
        right: gs.freedom_vector.y,
      });
      math_result_2
    });
    let count = gs.loop_counter;
    gs.loop_counter = 1;
    let did_iup = matches!(gs.did_iup_x, crate::InterpolationProgress::Applied)
      && matches!(gs.did_iup_y, crate::InterpolationProgress::Applied);

    let composite_vertical =
      matches!(gs.glyph_kind, crate::GlyphKind::Composite) && gs.freedom_vector.y != 0;

    for _ in 0..count {
      let point = self.value_stack.pop_index()?;
      let should_move = match (gs.backward_compatibility, in_twilight, did_iup, composite_vertical) {
        (crate::BackwardCompatibility::Disabled, _, _, _) => true,
        (crate::BackwardCompatibility::Enabled, true, _, _) => true,
        (crate::BackwardCompatibility::Enabled, false, true, _) => false,
        (crate::BackwardCompatibility::Enabled, false, false, true) => true,
        (crate::BackwardCompatibility::Enabled, false, false, false) => {
          let Ok(zone) = gs.zp2();
          let touch = zone.point_touch(point, CoordAxis::Y)?;

          matches!(touch, PointTouch::Touched)
        }
      };

      match should_move.then_some(()) {
        Some(()) => gs.move_zp2_point(point, dx, dy, crate::truetype::graphics::Touch::Mark)?,
        None => {}
      }
    }

    Ok(())
  }

  pub(super) fn op_msirp(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let distance = self.value_stack.pop_f26dot6()?;
    let point_ix = self.value_stack.pop_index()?;

    match ::core::primitive::bool::then_some(!matches!(gs.is_pedantic, crate::Pedantry::Strict)
      && !(gs.point_bounds([(gs.zp1, point_ix), (gs.zp0, gs.rp0)])
        == Ok(crate::PointBounds::Within)), ())
    {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    match gs.zp1 {
      ZonePointer::Twilight => {
        *{
          let answer = {
            let Ok(value) = gs.zp1_mut();
            value
          }
          .point_mut(point_ix)?;
          answer
        } = {
          let answer = {
            let Ok(value) = gs.zp0();
            value
          }
          .original(gs.rp0)?;
          answer
        };
        gs.move_original(gs.zp1, point_ix, distance)?;
        *{
          let answer = {
            let Ok(value) = gs.zp1_mut();
            value
          }
          .point_mut(point_ix)?;
          answer
        } = {
          let answer = {
            let Ok(value) = gs.zp1();
            value
          }
          .original(point_ix)?;
          answer
        };
      }
      ZonePointer::Glyph => {}
    }

    let Ok(d) = gs.project(
      {
        let answer = {
          let Ok(value) = gs.zp1();
          value
        }
        .point(point_ix)?;
        answer
      },
      {
        let answer = {
          let Ok(value) = gs.zp0();
          value
        }
        .point(gs.rp0)?;
        answer
      },
    );
    gs.move_point(gs.zp1, point_ix, {
      let Ok(value) = distance.wrapping_sub(d);
      value
    })?;
    gs.rp1 = gs.rp0;
    gs.rp2 = point_ix;

    match ::core::primitive::bool::then_some((opcode & 1) != 0, ()) {
      ::core::option::Option::Some(()) => {
        gs.rp0 = point_ix;
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  pub(super) fn op_mdap(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let p = self.value_stack.pop_index()?;

    match ::core::primitive::bool::then_some(!matches!(gs.is_pedantic, crate::Pedantry::Strict)
      && !(gs.point_bounds([(gs.zp0, p)]) == Ok(crate::PointBounds::Within)), ())
    {
      ::core::option::Option::Some(()) => {
        gs.rp0 = p;
        gs.rp1 = p;
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let distance = match ::core::primitive::bool::then_some((opcode & 1) != 0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(cur_dist) = gs.project(
          {
            let answer = {
              let Ok(value) = gs.zp0();
              value
            }
            .point(p)?;
            answer
          },
          Default::default(),
        );
        let rounded = gs.round(cur_dist)?;

        {
          let Ok(distance) = rounded.wrapping_sub(cur_dist);
          distance
        }
      }
      ::core::option::Option::None => F26Dot6::ZERO,
    };
    gs.move_point(gs.zp0, p, distance)?;
    gs.rp0 = p;
    gs.rp1 = p;
    Ok(())
  }

  pub(super) fn op_miap(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let cvt_entry = self.value_stack.pop_index()?;
    let point_ix = self.value_stack.pop_index()?;
    let mut distance = self.cvt.get(cvt_entry)?;

    match gs.zp0 {
      ZonePointer::Twilight => {
        let fv = gs.freedom_vector;
        let Ok(z) = gs.zp0_mut();
        let original_point = z.original_mut(point_ix)?;
        original_point.x = {
          let Ok(value) = F26Dot6::from_bits({
            let Ok(math_result_3) = math::mul14(math::Product {
              left: {
                let Ok(value) = distance.to_bits();
                value
              },
              right: fv.x,
            });
            math_result_3
          });
          value
        };
        original_point.y = {
          let Ok(value) = F26Dot6::from_bits({
            let Ok(math_result_4) = math::mul14(math::Product {
              left: {
                let Ok(value) = distance.to_bits();
                value
              },
              right: fv.y,
            });
            math_result_4
          });
          value
        };
        *{
          let answer = z.point_mut(point_ix)?;
          answer
        } = *original_point;
      }
      ZonePointer::Glyph => {}
    }

    let Ok(original_distance) = gs.project(
      {
        let answer = {
          let Ok(value) = gs.zp0();
          value
        }
        .point(point_ix)?;
        answer
      },
      Default::default(),
    );

    match ::core::primitive::bool::then_some((opcode & 1) != 0, ()) {
      ::core::option::Option::Some(()) => {
        let delta = {
          let Ok(value) = distance.wrapping_sub(original_distance);
          value
        }
        .abs();

        match ::core::primitive::bool::then_some({
          let Ok(value) = delta;
          value
        } > gs.control_value_cutin, ())
        {
          ::core::option::Option::Some(()) => {
            distance = original_distance;
          }
          ::core::option::Option::None => {}
        }

        let rounded = gs.round(distance)?;
        distance = rounded;
      }
      ::core::option::Option::None => {}
    }

    gs.move_point(gs.zp0, point_ix, {
      let Ok(value) = distance.wrapping_sub(original_distance);
      value
    })?;
    gs.rp0 = point_ix;
    gs.rp1 = point_ix;
    Ok(())
  }

  pub(super) fn op_mdrp(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let p = self.value_stack.pop_index()?;

    match ::core::primitive::bool::then_some(!matches!(gs.is_pedantic, crate::Pedantry::Strict)
      && !(gs.point_bounds([(gs.zp1, p), (gs.zp0, gs.rp0)]) == Ok(crate::PointBounds::Within)), ())
    {
      ::core::option::Option::Some(()) => {
        gs.rp1 = gs.rp0;
        gs.rp2 = p;

        match ::core::primitive::bool::then_some((opcode & 16) != 0, ()) {
          ::core::option::Option::Some(()) => {
            gs.rp0 = p;
          }
          ::core::option::Option::None => {}
        }

        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let mut original_distance =
      match ::core::primitive::bool::then_some(matches!(gs.zp0, crate::truetype::zone::ZonePointer::Twilight)
        || matches!(gs.zp1, crate::truetype::zone::ZonePointer::Twilight), ())
      {
        ::core::option::Option::Some(()) => {
          let Ok(value) = gs.dual_project(
            {
              let answer = {
                let Ok(value) = gs.zp1();
                value
              }
              .original(p)?;
              answer
            },
            {
              let answer = {
                let Ok(value) = gs.zp0();
                value
              }
              .original(gs.rp0)?;
              answer
            },
          );
          value
        }
        ::core::option::Option::None => {
          let Ok(v1) = {
            let Ok(value) = gs.zp1();
            value
          }
          .unscaled(p);
          let Ok(v2) = {
            let Ok(value) = gs.zp0();
            value
          }
          .unscaled(gs.rp0);
          let Ok(dist) = gs.dual_project_unscaled(v1, v2);

          {
            let Ok(value) = F26Dot6::from_bits({
              let Ok(math_result_5) = math::mul(math::Product {
                left: dist,
                right: {
                  let Ok(value) = gs.unscaled_to_pixels();
                  value
                },
              });
              math_result_5
            });
            value
          }
        }
      };
    let cutin = gs.single_width_cutin;
    let value = gs.single_width;

    match ::core::primitive::bool::then_some(cutin > F26Dot6::ZERO
      && original_distance < value + cutin
      && original_distance > value - cutin, ())
    {
      ::core::option::Option::Some(()) => {
        original_distance = match ::core::primitive::bool::then_some(original_distance >= F26Dot6::ZERO, ()) {
          ::core::option::Option::Some(()) => value,
          ::core::option::Option::None => -value,
        };
      }
      ::core::option::Option::None => {}
    }

    let rounding = match ::core::primitive::bool::then_some((opcode & 4) != 0, ()) {
      ::core::option::Option::Some(()) => gs.round(original_distance),
      ::core::option::Option::None => Ok(original_distance),
    };
    let mut distance = rounding?;

    match ::core::primitive::bool::then_some((opcode & 8) != 0, ()) {
      ::core::option::Option::Some(()) => {
        let min_distance = gs.min_distance;

        match ::core::primitive::bool::then_some(original_distance >= F26Dot6::ZERO, ()) {
          ::core::option::Option::Some(()) => match ::core::primitive::bool::then_some(distance < min_distance, ()) {
            ::core::option::Option::Some(()) => {
              distance = min_distance;
            }
            ::core::option::Option::None => {}
          },
          ::core::option::Option::None => match ::core::primitive::bool::then_some(distance > -min_distance, ()) {
            ::core::option::Option::Some(()) => {
              distance = -min_distance;
            }
            ::core::option::Option::None => {}
          },
        }
      }
      ::core::option::Option::None => {}
    }

    original_distance = {
      let Ok(value_4) = gs.project(
        {
          let answer = {
            let Ok(value_2) = gs.zp1();
            value_2
          }
          .point(p)?;
          answer
        },
        {
          let answer = {
            let Ok(value_3) = gs.zp0();
            value_3
          }
          .point(gs.rp0)?;
          answer
        },
      );
      value_4
    };
    gs.move_point(gs.zp1, p, {
      let Ok(value_5) = distance.wrapping_sub(original_distance);
      value_5
    })?;
    gs.rp1 = gs.rp0;
    gs.rp2 = p;

    match ::core::primitive::bool::then_some((opcode & 16) != 0, ()) {
      ::core::option::Option::Some(()) => {
        gs.rp0 = p;
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  pub(super) fn op_mirp(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let cvt_operand = self.value_stack.pop()?;
    let Ok(n) = console_core_number_conversion::wrapped::<_, u64>(cvt_operand.wrapping_add(1));
    let p = self.value_stack.pop_index()?;

    let Ok(cvt_len) = self.cvt.len();

    match ::core::primitive::bool::then_some(!matches!(gs.is_pedantic, crate::Pedantry::Strict)
      && (!(gs.point_bounds([(gs.zp1, p), (gs.zp0, gs.rp0)]) == Ok(crate::PointBounds::Within))
        || (n > cvt_len)), ())
    {
      ::core::option::Option::Some(()) => {
        gs.rp1 = gs.rp0;

        match ::core::primitive::bool::then_some((opcode & 16) != 0, ()) {
          ::core::option::Option::Some(()) => {
            gs.rp0 = p;
          }
          ::core::option::Option::None => {}
        }

        gs.rp2 = p;
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let mut cvt_distance = match ::core::primitive::bool::then_some(n == 0, ()) {
      ::core::option::Option::Some(()) => F26Dot6::ZERO,
      ::core::option::Option::None => self.cvt.get(u64::wrapping_sub(n, 1))?,
    };

    let Ok(width_difference) = cvt_distance.wrapping_sub(gs.single_width);
    let Ok(abs_difference) = width_difference.abs();

    match (abs_difference < gs.single_width_cutin).then_some(()) {
      Some(()) => {
        cvt_distance = match cvt_distance.cmp(&F26Dot6::ZERO) {
          core::cmp::Ordering::Less => -gs.single_width,
          core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => gs.single_width,
        };
      }
      None => {}
    }

    match gs.zp1 {
      ZonePointer::Twilight => {
        let fv = gs.freedom_vector;
        let point = {
          let Ok(d) = cvt_distance.to_bits();
          let p2 = {
            let Ok(value_4) = gs.zp0();
            value_4
          }
          .original(gs.rp0)?;
          let p1 = {
            let Ok(value_5) = gs.zp1_mut();
            value_5
          }
          .original_mut(p)?;
          p1.x = p2.x + {
            let Ok(value_6) = F26Dot6::from_bits({
              let Ok(math_result_6) = math::mul(math::Product {
                left: (d),
                right: fv.x,
              });
              math_result_6
            });
            value_6
          };
          p1.y = p2.y + {
            let Ok(value_7) = F26Dot6::from_bits({
              let Ok(math_result_7) = math::mul(math::Product {
                left: (d),
                right: fv.y,
              });
              math_result_7
            });
            value_7
          };
          *p1
        };
        *{
          let answer = {
            let Ok(value_8) = gs.zp1_mut();
            value_8
          }
          .point_mut(p)?;
          answer
        } = point;
      }
      ZonePointer::Glyph => {}
    }

    let Ok(point_zone) = gs.zp1();
    let Ok(reference_zone) = gs.zp0();
    let original_point = point_zone.original(p)?;
    let original_reference = reference_zone.original(gs.rp0)?;
    let Ok(original_distance) = gs.dual_project(original_point, original_reference);
    let current_point = point_zone.point(p)?;
    let current_reference = reference_zone.point(gs.rp0)?;
    let Ok(current_distance) = gs.project(current_point, current_reference);
    let Ok(original_bits) = original_distance.to_bits();
    let Ok(cvt_bits) = cvt_distance.to_bits();

    match (gs.auto_flip, (original_bits ^ cvt_bits) < 0) {
      (crate::AutomaticFlip::Enabled, true) => cvt_distance = -cvt_distance,
      (crate::AutomaticFlip::Enabled, false) | (crate::AutomaticFlip::Disabled, _) => {}
    }

    match (opcode & 4, gs.zp0 == gs.zp1) {
      (0, _) | (_, false) => {}
      (_, true) => {
        let Ok(difference) = cvt_distance.wrapping_sub(original_distance);
        let Ok(width_delta) = difference.abs();
        cvt_distance = match width_delta.cmp(&gs.control_value_cutin) {
          core::cmp::Ordering::Greater => original_distance,
          core::cmp::Ordering::Less | core::cmp::Ordering::Equal => cvt_distance,
        };
      }
    }

    let rounding = match opcode & 4 {
      0 => Ok(cvt_distance),
      _ => gs.round(cvt_distance),
    };
    let rounded = rounding?;
    let distance = match opcode & 8 {
      0 => rounded,
      _ => match original_distance.cmp(&F26Dot6::ZERO) {
        core::cmp::Ordering::Less => rounded.min(-gs.min_distance),
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => rounded.max(gs.min_distance),
      },
    };
    let Ok(displacement) = distance.wrapping_sub(current_distance);
    gs.move_point(gs.zp1, p, displacement)?;
    gs.rp1 = gs.rp0;

    match ::core::primitive::bool::then_some((opcode & 16) != 0, ()) {
      ::core::option::Option::Some(()) => {
        gs.rp0 = p;
      }
      ::core::option::Option::None => {}
    }

    gs.rp2 = p;
    Ok(())
  }

  pub(super) fn op_alignrp(&mut self) -> OpResult {
    let gs = &mut self.graphics;
    let count = gs.loop_counter;
    gs.loop_counter = 1;

    for _ in 0..count {
      let p = self.value_stack.pop_index()?;
      let Ok(distance) = gs.project(
        {
          let answer = {
            let Ok(value) = gs.zp1();
            value
          }
          .point(p)?;
          answer
        },
        {
          let answer = {
            let Ok(value) = gs.zp0();
            value
          }
          .point(gs.rp0)?;
          answer
        },
      );
      gs.move_point(gs.zp1, p, -distance)?;
    }

    Ok(())
  }

  pub(super) fn op_isect(&mut self) -> OpResult {
    let gs = &mut self.graphics;
    let b1 = self.value_stack.pop_index()?;
    let b0 = self.value_stack.pop_index()?;
    let a1 = self.value_stack.pop_index()?;
    let a0 = self.value_stack.pop_index()?;
    let point_ix = self.value_stack.pop_index()?;
    let Ok(zone) = gs.zp1();
    let pa0 = zone.point_bits(a0)?;
    let pa1 = zone.point_bits(a1)?;
    let Ok(zone_2) = gs.zp0();
    let pb0 = zone_2.point_bits(b0)?;
    let pb1 = zone_2.point_bits(b1)?;
    let dbx = i32::wrapping_sub(pb1.x, pb0.x);
    let dby = i32::wrapping_sub(pb1.y, pb0.y);
    let dax = i32::wrapping_sub(pa1.x, pa0.x);
    let day = i32::wrapping_sub(pa1.y, pa0.y);
    let dx = i32::wrapping_sub(pb0.x, pa0.x);
    let dy = i32::wrapping_sub(pb0.y, pa0.y);
    let discriminant = i32::wrapping_add(
      {
        let Ok(intersection_product_1) = math::mul_div(math::ProductRatio {
          value: dax,
          multiplier: dby.wrapping_neg(),
          divisor: 0x40,
        });
        intersection_product_1
      },
      {
        let Ok(intersection_product_2) = math::mul_div(math::ProductRatio {
          value: day,
          multiplier: dbx,
          divisor: 0x40,
        });
        intersection_product_2
      },
    );
    let dotproduct = i32::wrapping_add(
      {
        let Ok(intersection_product_3) = math::mul_div(math::ProductRatio {
          value: dax,
          multiplier: dbx,
          divisor: 0x40,
        });
        intersection_product_3
      },
      {
        let Ok(intersection_product_4) = math::mul_div(math::ProductRatio {
          value: day,
          multiplier: dby,
          divisor: 0x40,
        });
        intersection_product_4
      },
    );

    match ::core::primitive::bool::then_some(discriminant.wrapping_abs().wrapping_mul(19) > dotproduct.abs(), ()) {
      ::core::option::Option::Some(()) => {
        let v = i32::wrapping_add(
          {
            let Ok(intersection_product_5) = math::mul_div(math::ProductRatio {
              value: dx,
              multiplier: dby.wrapping_neg(),
              divisor: 0x40,
            });
            intersection_product_5
          },
          {
            let Ok(intersection_product_6) = math::mul_div(math::ProductRatio {
              value: dy,
              multiplier: dbx,
              divisor: 0x40,
            });
            intersection_product_6
          },
        );
        let Ok(x) = math::mul_div(math::ProductRatio {
          value: v,
          multiplier: dax,
          divisor: discriminant,
        });
        let Ok(y) = math::mul_div(math::ProductRatio {
          value: v,
          multiplier: day,
          divisor: discriminant,
        });
        let point = {
          let Ok(value) = gs.zp2_mut();
          value
        }
        .point_mut(point_ix)?;
        point.x = {
          let Ok(value) = F26Dot6::from_bits(i32::wrapping_add(pa0.x, x));
          value
        };
        point.y = {
          let Ok(value) = F26Dot6::from_bits(i32::wrapping_add(pa0.y, y));
          value
        };
      }
      ::core::option::Option::None => {
        let point = {
          let Ok(value) = gs.zp2_mut();
          value
        }
        .point_mut(point_ix)?;
        point.x = {
          let Ok(value) = F26Dot6::from_bits(i32::wrapping_div(
            i32::wrapping_add(
              i32::wrapping_add(i32::wrapping_add(pa0.x, pa1.x), pb0.x),
              pb1.x,
            ),
            4,
          ));
          value
        };
        point.y = {
          let Ok(value) = F26Dot6::from_bits(i32::wrapping_div(
            i32::wrapping_add(
              i32::wrapping_add(i32::wrapping_add(pa0.y, pa1.y), pb0.y),
              pb1.y,
            ),
            4,
          ));
          value
        };
      }
    }

    {
      let Ok(value) = gs.zp2_mut();
      value
    }
    .touch(point_ix, CoordAxis::Both)?;
    Ok(())
  }

  pub(super) fn op_alignpts(&mut self) -> OpResult {
    let p2 = self.value_stack.pop_index()?;
    let p1 = self.value_stack.pop_index()?;
    let gs = &mut self.graphics;
    let Ok(distance) = F26Dot6::from_bits(i32::wrapping_div(
      {
        let Ok(value) = {
          let Ok(value) = gs.project(
            {
              let answer = {
                let Ok(value) = gs.zp0();
                value
              }
              .point(p2)?;
              answer
            },
            {
              let answer = {
                let Ok(value) = gs.zp1();
                value
              }
              .point(p1)?;
              answer
            },
          );
          value
        }
        .to_bits();
        value
      },
      2,
    ));
    gs.move_point(gs.zp1, p1, distance)?;
    gs.move_point(gs.zp0, p2, -(distance))?;
    Ok(())
  }

  pub(super) fn op_ip(&mut self) -> OpResult {
    let gs = &mut self.graphics;
    let count = gs.loop_counter;
    gs.loop_counter = 1;

    match ::core::primitive::bool::then_some(!matches!(gs.is_pedantic, crate::Pedantry::Strict)
      && !(gs.point_bounds([(gs.zp0, gs.rp1), (gs.zp1, gs.rp2)])
        == Ok(crate::PointBounds::Within)), ())
    {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let in_twilight = matches!(gs.zp0, crate::truetype::zone::ZonePointer::Twilight)
      || matches!(gs.zp1, crate::truetype::zone::ZonePointer::Twilight)
      || matches!(gs.zp2, crate::truetype::zone::ZonePointer::Twilight);
    let orus_base = match ::core::primitive::bool::then_some(in_twilight, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value) = gs.zp0();
        value
      }
      .original(gs.rp1)?,
      ::core::option::Option::None => {
        let Ok(value) = {
          let Ok(value) = {
            let Ok(value) = gs.zp0();
            value
          }
          .unscaled(gs.rp1);
          value
        }
        .map(|bits| {
          let Ok(coordinate) = F26Dot6::from_bits(bits);
          coordinate
        });
        value
      }
    };
    let cur_base = {
      let Ok(value) = gs.zp0();
      value
    }
    .point(gs.rp1)?;
    let old_range = match ::core::primitive::bool::then_some(in_twilight, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value) = gs.dual_project(
          {
            let answer = {
              let Ok(value) = gs.zp1();
              value
            }
            .original(gs.rp2)?;
            answer
          },
          orus_base,
        );
        value
      }
      ::core::option::Option::None => {
        let Ok(value) = gs.dual_project(
          {
            let Ok(value) = {
              let Ok(value) = {
                let Ok(value) = gs.zp1();
                value
              }
              .unscaled(gs.rp2);
              value
            }
            .map(|bits| {
              let Ok(coordinate) = F26Dot6::from_bits(bits);
              coordinate
            });
            value
          },
          orus_base,
        );
        value
      }
    };
    let Ok(cur_range) = gs.project(
      {
        let answer = {
          let Ok(value) = gs.zp1();
          value
        }
        .point(gs.rp2)?;
        answer
      },
      cur_base,
    );

    for _ in 0..count {
      let point = self.value_stack.pop_index()?;

      match ::core::primitive::bool::then_some(!matches!(gs.is_pedantic, crate::Pedantry::Strict)
        && !(gs.point_bounds([(gs.zp2, point)]) == Ok(crate::PointBounds::Within)), ())
      {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      let Ok(zone) = gs.zp2();
      let original_point = match ::core::primitive::bool::then_some(in_twilight, ()) {
        ::core::option::Option::Some(()) => zone.original(point)?,
        ::core::option::Option::None => {
          let Ok(unscaled) = zone.unscaled(point);
          let Ok(fixed) = unscaled.map(|bits| {
            let Ok(coordinate) = F26Dot6::from_bits(bits);
            coordinate
          });
          fixed
        }
      };
      let current_point = zone.point(point)?;
      let Ok(original_distance) = gs.dual_project(original_point, orus_base);
      let Ok(cur_distance) = gs.project(current_point, cur_base);

      let new_distance = match (original_distance == F26Dot6::ZERO, old_range == F26Dot6::ZERO) {
        (true, _) => F26Dot6::ZERO,
        (false, true) => original_distance,
        (false, false) => {
          let Ok(value) = original_distance.to_bits();
          let Ok(multiplier) = cur_range.to_bits();
          let Ok(divisor) = old_range.to_bits();
          let Ok(bits) = math::mul_div(math::ProductRatio { value, multiplier, divisor });
          let Ok(distance) = F26Dot6::from_bits(bits);
          distance
        }
      };
      gs.move_point(gs.zp2, point, {
        let Ok(value) = new_distance.wrapping_sub(cur_distance);
        value
      })?;
    }

    Ok(())
  }

  pub(super) fn op_iup(&mut self, opcode: u8) -> OpResult {
    let gs = &mut self.graphics;
    let axis = match ::core::primitive::bool::then_some((opcode & 1) != 0, ()) {
      ::core::option::Option::Some(()) => CoordAxis::X,
      ::core::option::Option::None => CoordAxis::Y,
    };
    let mut run = true;

    match gs.backward_compatibility {
      crate::BackwardCompatibility::Enabled => {
        match ::core::primitive::bool::then_some(matches!(gs.did_iup_x, crate::InterpolationProgress::Applied)
          && matches!(gs.did_iup_y, crate::InterpolationProgress::Applied), ())
        {
          ::core::option::Option::Some(()) => {
            run = false;
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(axis == CoordAxis::X, ()) {
          ::core::option::Option::Some(()) => {
            gs.did_iup_x = crate::InterpolationProgress::Applied;
          }
          ::core::option::Option::None => {
            gs.did_iup_y = crate::InterpolationProgress::Applied;
          }
        }
      }
      crate::BackwardCompatibility::Disabled => {}
    }

    match ::core::primitive::bool::then_some(run, ()) {
      ::core::option::Option::Some(()) => {
        {
          let Ok(value) = gs.zone_mut(ZonePointer::Glyph);
          value
        }
        .iup(axis)?;
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  pub(super) fn op_utp(&mut self) -> OpResult {
    let p = self.value_stack.pop_index()?;
    let coord_axis = match (
      self.graphics.freedom_vector.x != 0,
      self.graphics.freedom_vector.y != 0,
    ) {
      (true, true) => Some(CoordAxis::Both),
      (true, false) => Some(CoordAxis::X),
      (false, true) => Some(CoordAxis::Y),
      (false, false) => None,
    };

    match coord_axis {
      Some(coord_axis_2) => {
        {
          let Ok(value) = self.graphics.zp0_mut();
          value
        }
        .untouch(p, coord_axis_2)?;
      }
      None => {}
    }

    Ok(())
  }

  fn set_on_curve_for_range(&mut self, curve: crate::CurveKind) -> OpResult {
    let high_point = self.value_stack.pop_index()?;
    let low_point = self.value_stack.pop_index()?;

    let high_point_2 = high_point
      .checked_add(1)
      .ok_or(HintErrorKind::InvalidPointIndex(high_point))?;

    match ::core::primitive::bool::then_some(matches!(
      self.graphics.backward_compatibility,
      crate::BackwardCompatibility::Enabled
    ) && matches!(
      self.graphics.did_iup_x,
      crate::InterpolationProgress::Applied
    ) && matches!(
      self.graphics.did_iup_y,
      crate::InterpolationProgress::Applied
    ), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    {
      let Ok(value) = self.graphics.zone_mut(ZonePointer::Glyph);
      value
    }
    .set_on_curve(low_point..high_point_2, curve)
  }
}

#[cfg(test)]
mod tests {
  use super::{super::MockEngine, CoordAxis, Engine, ZonePointer, math};
  use raw::{
    tables::glyf::{PointMarker, bytecode::Opcode},
    types::{F26Dot6, Point},
  };

  #[test]
  fn flip_point() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    let count = 5;

    engine.value_stack.push(count).required()?;
    engine.op_sloop().required()?;

    for i in (1..=9).step_by(2) {
      engine.value_stack.push(i).required()?;
    }

    assert_eq!(
      engine.value_stack.len(),
      Ok({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(count);
        migrated_value
      })
    );

    engine.op_flippt().required()?;
    let flags = &(*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .flags;

    for (i, flag) in { let checked_value = flags.get(..10).required()?; checked_value }.iter().enumerate() {
      assert_eq!(matches!(flag.curve_membership(), raw::tables::glyf::CurveMembership::OnCurve), i & 1 != 0);
    }

    Ok(())
  }

  #[test]
  fn state_prevents_flip_point() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    let count = 5;

    engine.value_stack.push(count).required()?;
    engine.op_sloop().required()?;

    for i in (1..=9).step_by(2) {
      engine.value_stack.push(i).required()?;
    }

    assert_eq!(
      engine.value_stack.len(),
      Ok({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(count);
        migrated_value
      })
    );

    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Enabled;
    engine.graphics.did_iup_x = crate::InterpolationProgress::Applied;
    engine.graphics.did_iup_y = crate::InterpolationProgress::Applied;

    engine.op_flippt().required()?;
    let flags = &(*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .flags;

    let visible_flags = flags.get(..10).required()?;

    for flag in visible_flags {
      assert!(!matches!(flag.curve_membership(), raw::tables::glyf::CurveMembership::OnCurve));
    }

    Ok(())
  }

  #[test]
  fn flip_range_on_off() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    engine.value_stack.push(10).required()?;
    engine.value_stack.push(20).required()?;
    engine.op_fliprgon().required()?;

    for (i, flag) in (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .flags
    .iter()
    .enumerate()
    {
      assert_eq!(matches!(flag.curve_membership(), raw::tables::glyf::CurveMembership::OnCurve), (10..=20).contains(&i));
    }

    engine.value_stack.push(12).required()?;
    engine.value_stack.push(15).required()?;
    engine.op_fliprgoff().required()?;

    for (i, flag) in (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .flags
    .iter()
    .enumerate()
    {
      assert_eq!(
        matches!(flag.curve_membership(), raw::tables::glyf::CurveMembership::OnCurve),
        (10..=11).contains(&i) || (16..=20).contains(&i)
      );
    }

    Ok(())
  }

  #[test]
  fn state_prevents_flip_range_on_off() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Enabled;
    engine.graphics.did_iup_x = crate::InterpolationProgress::Applied;
    engine.graphics.did_iup_y = crate::InterpolationProgress::Applied;

    engine.value_stack.push(10).required()?;
    engine.value_stack.push(20).required()?;
    engine.op_fliprgon().required()?;

    for flag in (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .flags
    .iter()
    {
      assert!(!matches!(flag.curve_membership(), raw::tables::glyf::CurveMembership::OnCurve));
    }

    for flag in (*match &mut engine.graphics.zones {
      [_, element, ..] => element,
    })
    .flags
    .iter_mut()
    {
      flag.set_on_curve();
    }

    engine.value_stack.push(12).required()?;
    engine.value_stack.push(15).required()?;
    engine.op_fliprgoff().required()?;

    for flag in (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .flags
    .iter()
    {
      assert!(matches!(flag.curve_membership(), raw::tables::glyf::CurveMembership::OnCurve));
    }

    Ok(())
  }

  #[test]
  fn untouch_point() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    let Ok(glyph) = engine.graphics.zone_mut(ZonePointer::Glyph);
    let Ok(point_count) = console_core_number_conversion::fitted::<_, u64>(glyph.points.len());

    for index in 0..point_count {
      glyph.touch(index, CoordAxis::Both).required()?;
    }

    let untouch = |interpreter: &mut Engine,
                   point_ix: u64,
                   fx,
                   fy,
                   marker|
     -> Result<(), crate::TestFailure> {
      let Ok(point_index) = console_core_number_conversion::index(point_ix);
      let Ok(zone) = interpreter.graphics.zp0();
      let flag = zone.flags.get(point_index).required()?;
      assert_eq!(flag.marker_presence(marker), raw::Presence::Present);

      interpreter.graphics.freedom_vector.x = fx;
      interpreter.graphics.freedom_vector.y = fy;
      let Ok(stack_index) = console_core_number_conversion::wrapped::<_, i32>(point_ix);
      interpreter.value_stack.push(stack_index).required()?;
      interpreter.op_utp().required()?;
      let Ok(updated_zone) = interpreter.graphics.zp0();
      let flag_after = updated_zone.flags.get(point_index).required()?;
      assert_eq!(flag_after.marker_presence(marker), raw::Presence::Absent);
        Ok(())
      };

    untouch(&mut engine, 0, 1, 0, PointMarker::TOUCHED_X)?;

    untouch(&mut engine, 1, 0, 1, PointMarker::TOUCHED_Y)?;

    untouch(&mut engine, 2, 1, 1, PointMarker::TOUCHED)?;

    Ok(())
  }

  #[test]
  fn shp() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });
    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.zp0 = ZonePointer::Glyph;
    engine.graphics.zp2 = ZonePointer::Glyph;
    engine.graphics.rp2 = 1;
    let point = (*match &mut engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point_mut(1)
    .required()?;
    point.x = {
      let Ok(value) = F26Dot6::from_bits(132);
      value
    };
    point.y = {
      let Ok(value) = F26Dot6::from_bits(-256);
      value
    };
    engine.value_stack.push(1).required()?;
    engine.op_shp(0).required()?;
    let point_2 = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(1)
    .required()?;
    assert_eq!(
      ({
        let Ok(mapped) = point_2.map(|coordinate| {
          let Ok(bits) = coordinate.to_bits();
          bits
        });
        mapped
      }),
      ({
        let Ok(value) = Point::new(136, -254);
        value
      })
    );

    Ok(())
  }

  #[test]
  fn shc() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });
    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.zp0 = ZonePointer::Glyph;
    engine.graphics.zp2 = ZonePointer::Glyph;
    engine.graphics.rp2 = 1;
    let point = (*match &mut engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point_mut(1)
    .required()?;
    point.x = {
      let Ok(value) = F26Dot6::from_bits(132);
      value
    };
    point.y = {
      let Ok(value) = F26Dot6::from_bits(-256);
      value
    };
    engine.value_stack.push(0).required()?;
    engine.op_shc(0).required()?;
    let points = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .points
    .iter()
    .map(|p| {
      let Ok(mapped) = p.map(|coordinate| {
        let Ok(bits) = coordinate.to_bits();
        bits
      });
      mapped
    })
    .take(3)
    .collect::<Vec<_>>();
    assert_eq!(
      points,
      &[
        ({
          let Ok(value) = Point::new(4, 2);
          value
        }),
        ({
          let Ok(value) = Point::new(132, -256);
          value
        }),
        ({
          let Ok(value) = Point::new(4, 2);
          value
        }),
      ]
    );

    Ok(())
  }

  #[test]
  fn shz() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });
    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.zp0 = ZonePointer::Glyph;
    engine.graphics.zp2 = ZonePointer::Glyph;
    engine.graphics.rp2 = 1;
    let point = (*match &mut engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point_mut(1)
    .required()?;
    point.x = {
      let Ok(value) = F26Dot6::from_bits(132);
      value
    };
    point.y = {
      let Ok(value) = F26Dot6::from_bits(-256);
      value
    };
    engine.value_stack.push(0).required()?;
    engine.op_shz(0).required()?;
    let points = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .points
    .iter()
    .map(|p| {
      let Ok(mapped) = p.map(|coordinate| {
        let Ok(bits) = coordinate.to_bits();
        bits
      });
      mapped
    })
    .take(3)
    .collect::<Vec<_>>();
    assert_eq!(
      points,
      &[
        ({
          let Ok(value) = Point::new(4, 2);
          value
        }),
        ({
          let Ok(value) = Point::new(132, -256);
          value
        }),
        ({
          let Ok(value) = Point::new(4, 2);
          value
        }),
      ]
    );

    Ok(())
  }

  #[test]
  fn shpix() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });
    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.zp2 = ZonePointer::Glyph;
    let point = (*match &mut engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point_mut(1)
    .required()?;
    point.x = {
      let Ok(value) = F26Dot6::from_bits(132);
      value
    };
    point.y = {
      let Ok(value) = F26Dot6::from_bits(-256);
      value
    };

    engine.value_stack.push(1).required()?;

    engine.value_stack.push(42).required()?;
    engine.op_shpix().required()?;
    let point_2 = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(1)
    .required()?;
    assert_eq!(
      ({
        let Ok(mapped) = point_2.map(|coordinate| {
          let Ok(bits) = coordinate.to_bits();
          bits
        });
        mapped
      }),
      ({
        let Ok(value) = Point::new(170, -237);
        value
      })
    );

    Ok(())
  }

  #[test]
  fn msirp() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });
    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.zp0 = ZonePointer::Glyph;
    engine.graphics.zp1 = ZonePointer::Glyph;
    let point = (*match &mut engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point_mut(1)
    .required()?;
    point.x = {
      let Ok(value) = F26Dot6::from_bits(132);
      value
    };
    point.y = {
      let Ok(value) = F26Dot6::from_bits(-256);
      value
    };

    engine.value_stack.push(1).required()?;

    engine.value_stack.push(-42).required()?;
    engine.op_msirp(0).required()?;
    let point_2 = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(1)
    .required()?;
    assert_eq!(
      ({
        let Ok(mapped) = point_2.map(|coordinate| {
          let Ok(bits) = coordinate.to_bits();
          bits
        });
        mapped
      }),
      ({
        let Ok(value) = Point::new(91, -277);
        value
      })
    );
    assert_eq!(engine.graphics.rp0, 0);

    engine.value_stack.push(4).required()?;
    engine.value_stack.push(0).required()?;
    engine.op_msirp(1).required()?;
    assert_eq!(engine.graphics.rp0, 4);

    Ok(())
  }

  #[test]
  fn mdap() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });
    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.zp0 = ZonePointer::Glyph;

    engine.set_glyph_point(1, (132, -256))?;
    engine.value_stack.push(1).required()?;
    engine.op_mdap(1).required()?;
    let point = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(1)
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
        let Ok(value) = Point::new(128, -258);
        value
      })
    );

    engine.set_glyph_point(2, (132, -256))?;
    engine.value_stack.push(2).required()?;
    engine.op_mdap(0).required()?;
    let point_2 = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(2)
    .required()?;
    assert_eq!(
      ({
        let Ok(mapped) = point_2.map(|coordinate| {
          let Ok(bits) = coordinate.to_bits();
          bits
        });
        mapped
      }),
      ({
        let Ok(value) = Point::new(132, -256);
        value
      })
    );

    Ok(())
  }

  #[test]
  fn miap() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });
    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.zp0 = ZonePointer::Glyph;

    engine
      .cvt
      .set(1, {
        let Ok(value) = F26Dot6::from_f64(0.75);
        value
      })
      .required()?;

    engine.set_glyph_point(1, (132, -256))?;
    engine.value_stack.push(1).required()?;
    engine.value_stack.push(1).required()?;
    engine.op_miap(1).required()?;
    let point = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(1)
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
        let Ok(value) = Point::new(186, -229);
        value
      })
    );

    engine.set_glyph_point(2, (132, -256))?;
    engine.value_stack.push(2).required()?;
    engine.value_stack.push(1).required()?;
    engine.op_miap(0).required()?;
    let point_2 = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(2)
    .required()?;
    assert_eq!(
      ({
        let Ok(mapped) = point_2.map(|coordinate| {
          let Ok(bits) = coordinate.to_bits();
          bits
        });
        mapped
      }),
      ({
        let Ok(value) = Point::new(171, -236);
        value
      })
    );

    Ok(())
  }

  #[test]
  fn mdrp_rp0() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    engine.graphics.rp0 = 0;

    engine.value_stack.push(1).required()?;
    let Ok(opcode_mdrp00000) = Opcode::MDRP00000.to_byte();
    engine.op_mdrp(opcode_mdrp00000).required()?;
    assert_eq!(engine.graphics.rp0, 0);

    engine.value_stack.push(1).required()?;
    let Ok(opcode_mdrp10000) = Opcode::MDRP10000.to_byte();
    engine.op_mdrp(opcode_mdrp10000).required()?;
    assert_eq!(engine.graphics.rp0, 1);

    Ok(())
  }

  #[test]
  fn mdrp_mindist() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });
    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.zp0 = ZonePointer::Glyph;

    engine.set_glyph_point(1, (132, -256))?;
    engine.value_stack.push(1).required()?;
    let Ok(opcode_mdrp00000) = Opcode::MDRP00000.to_byte();
    engine.op_mdrp(opcode_mdrp00000).required()?;
    let point = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(1)
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
        let Ok(value) = Point::new(128, -258);
        value
      })
    );

    engine.set_glyph_point(2, (132, -256))?;
    engine.value_stack.push(2).required()?;
    let Ok(opcode_mdrp01000) = Opcode::MDRP01000.to_byte();
    engine.op_mdrp(opcode_mdrp01000).required()?;
    let point_2 = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(2)
    .required()?;
    assert_eq!(
      ({
        let Ok(mapped) = point_2.map(|coordinate| {
          let Ok(bits) = coordinate.to_bits();
          bits
        });
        mapped
      }),
      ({
        let Ok(value) = Point::new(186, -229);
        value
      })
    );

    Ok(())
  }

  #[test]
  fn mdrp_round() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });
    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.zp0 = ZonePointer::Glyph;
    engine.op_rthg().required()?;

    engine.set_glyph_point(1, (132, -231))?;
    engine.value_stack.push(1).required()?;
    let Ok(opcode_mdrp00000) = Opcode::MDRP00000.to_byte();
    engine.op_mdrp(opcode_mdrp00000).required()?;
    let point = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(1)
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
        let Ok(value) = Point::new(119, -238);
        value
      })
    );

    engine.set_glyph_point(2, (132, -231))?;
    engine.value_stack.push(2).required()?;
    let Ok(opcode_mdrp00100) = Opcode::MDRP00100.to_byte();
    engine.op_mdrp(opcode_mdrp00100).required()?;
    let point_2 = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(2)
    .required()?;
    assert_eq!(
      ({
        let Ok(mapped) = point_2.map(|coordinate| {
          let Ok(bits) = coordinate.to_bits();
          bits
        });
        mapped
      }),
      ({
        let Ok(value) = Point::new(147, -223);
        value
      })
    );

    Ok(())
  }

  #[test]
  fn mirp_rp0() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    engine.graphics.rp0 = 0;

    engine.value_stack.push(1).required()?;
    engine.value_stack.push(1).required()?;
    let Ok(opcode_mirp00000) = Opcode::MIRP00000.to_byte();
    engine.op_mirp(opcode_mirp00000).required()?;
    assert_eq!(engine.graphics.rp0, 0);

    engine.value_stack.push(1).required()?;
    engine.value_stack.push(1).required()?;
    let Ok(opcode_mirp10000) = Opcode::MIRP10000.to_byte();
    engine.op_mirp(opcode_mirp10000).required()?;
    assert_eq!(engine.graphics.rp0, 1);

    Ok(())
  }

  #[test]
  fn mirp_mindist() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });
    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.zp0 = ZonePointer::Glyph;

    engine
      .cvt
      .set(1, {
        let Ok(value) = F26Dot6::from_f64(0.75);
        value
      })
      .required()?;

    engine.set_glyph_point(1, (132, -256))?;
    engine.value_stack.push(1).required()?;
    engine.value_stack.push(1).required()?;
    let Ok(opcode_mirp00000) = Opcode::MIRP00000.to_byte();
    engine.op_mirp(opcode_mirp00000).required()?;
    let point = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(1)
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
        let Ok(value) = Point::new(171, -236);
        value
      })
    );

    engine.set_glyph_point(2, (132, -256))?;
    engine.value_stack.push(2).required()?;
    engine.value_stack.push(1).required()?;
    let Ok(opcode_mirp01000) = Opcode::MIRP01000.to_byte();
    engine.op_mirp(opcode_mirp01000).required()?;
    let point_2 = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(2)
    .required()?;
    assert_eq!(
      ({
        let Ok(mapped) = point_2.map(|coordinate| {
          let Ok(bits) = coordinate.to_bits();
          bits
        });
        mapped
      }),
      ({
        let Ok(value) = Point::new(186, -229);
        value
      })
    );

    Ok(())
  }

  #[test]
  fn mirp_round() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });
    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.zp0 = ZonePointer::Glyph;

    engine
      .cvt
      .set(1, {
        let Ok(value) = F26Dot6::from_f64(0.75);
        value
      })
      .required()?;
    engine.op_rthg().required()?;

    engine.set_glyph_point(1, (132, -231))?;
    engine.value_stack.push(1).required()?;
    engine.value_stack.push(1).required()?;
    let Ok(opcode_mirp00000) = Opcode::MIRP00000.to_byte();
    engine.op_mirp(opcode_mirp00000).required()?;
    let point = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(1)
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
        let Ok(value) = Point::new(162, -216);
        value
      })
    );

    engine.set_glyph_point(2, (132, -231))?;
    engine.value_stack.push(2).required()?;
    engine.value_stack.push(1).required()?;
    let Ok(opcode_mirp00100) = Opcode::MIRP00100.to_byte();
    engine.op_mirp(opcode_mirp00100).required()?;
    let point_2 = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(2)
    .required()?;
    assert_eq!(
      ({
        let Ok(mapped) = point_2.map(|coordinate| {
          let Ok(bits) = coordinate.to_bits();
          bits
        });
        mapped
      }),
      ({
        let Ok(value) = Point::new(147, -223);
        value
      })
    );

    Ok(())
  }

  #[test]
  fn alignrp() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });
    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.zp0 = ZonePointer::Glyph;
    engine.graphics.zp1 = ZonePointer::Glyph;
    engine.graphics.rp0 = 0;
    engine.set_glyph_point(0, (132, -231))?;
    engine.set_glyph_point(1, (-72, 109))?;
    engine.value_stack.push(1).required()?;
    engine.op_alignrp().required()?;
    let point = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(1)
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
        let Ok(value) = Point::new(-45, 122);
        value
      })
    );

    Ok(())
  }

  #[test]
  fn isect() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    engine.graphics.zp0 = ZonePointer::Glyph;
    engine.graphics.zp1 = ZonePointer::Glyph;
    engine.graphics.rp0 = 0;

    engine.set_glyph_point(0, (0, 0))?;
    engine.set_glyph_point(1, (100, 100))?;

    engine.set_glyph_point(2, (0, 100))?;
    engine.set_glyph_point(3, (100, 0))?;

    for ix in [4, 0, 1, 2, 3] {
      engine.value_stack.push(ix).required()?;
    }

    engine.op_isect().required()?;
    let point = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(4)
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
        let Ok(value) = Point::new(50, 50);
        value
      })
    );

    Ok(())
  }

  #[test]
  fn alignpts() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });
    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.zp0 = ZonePointer::Glyph;
    engine.graphics.zp1 = ZonePointer::Glyph;
    engine.set_glyph_point(0, (132, -231))?;
    engine.set_glyph_point(1, (-72, 109))?;
    engine.value_stack.push(0).required()?;
    engine.value_stack.push(1).required()?;
    engine.op_alignpts().required()?;
    let p1 = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(0)
    .required()?;
    let p2 = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(1)
    .required()?;
    assert_eq!(
      ({
        let Ok(mapped) = p1.map(|coordinate| {
          let Ok(bits) = coordinate.to_bits();
          bits
        });
        mapped
      }),
      ({
        let Ok(value) = Point::new(119, -238);
        value
      })
    );
    assert_eq!(
      ({
        let Ok(mapped) = p2.map(|coordinate| {
          let Ok(bits) = coordinate.to_bits();
          bits
        });
        mapped
      }),
      ({
        let Ok(value) = Point::new(-59, 116);
        value
      })
    );

    Ok(())
  }

  #[test]
  fn ip() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });
    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.zp0 = ZonePointer::Glyph;
    engine.graphics.zp1 = ZonePointer::Glyph;
    engine.graphics.zp2 = ZonePointer::Glyph;
    engine.graphics.rp1 = 2;
    engine.graphics.rp2 = 3;
    engine.set_glyph_point(2, (72, -109))?;
    engine.set_glyph_point(1, (132, -231))?;
    engine.value_stack.push(1).required()?;
    engine.op_ip().required()?;
    let point = (*match &engine.graphics.zones {
      [_, element, ..] => element,
    })
    .point(1)
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
        let Ok(value) = Point::new(147, -223);
        value
      })
    );

    Ok(())
  }

  #[test]
  fn iup_flags() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    assert!(!matches!(
      engine.graphics.did_iup_x,
      crate::InterpolationProgress::Applied
    ));
    assert!(!matches!(
      engine.graphics.did_iup_y,
      crate::InterpolationProgress::Applied
    ));

    engine.op_iup(0).required()?;
    assert!(!matches!(
      engine.graphics.did_iup_x,
      crate::InterpolationProgress::Applied
    ));
    assert!(matches!(
      engine.graphics.did_iup_y,
      crate::InterpolationProgress::Applied
    ));

    engine.op_iup(1).required()?;
    assert!(matches!(
      engine.graphics.did_iup_x,
      crate::InterpolationProgress::Applied
    ));
    assert!(matches!(
      engine.graphics.did_iup_y,
      crate::InterpolationProgress::Applied
    ));

    Ok(())
  }

  #[test]
  fn flip_region_avoid_overflow() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    engine.value_stack.push(1).required()?;
    engine.value_stack.push(-1).required()?;

    assert_eq!(
      engine.set_on_curve_for_range(crate::CurveKind::On),
      Err(super::HintErrorKind::InvalidStackValue(-1))
    );

    Ok(())
  }

  fn set_test_vectors(
    engine: &mut Engine,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(v) = math::normalize14(read_fonts::types::Point { x: 100, y: 50 });
    engine.graphics.proj_vector = v;
    engine.graphics.dual_proj_vector = v;
    engine.graphics.freedom_vector = v;
    let ::core::result::Result::Ok(()) = engine.graphics.update_projection_state();

    Ok(())
  }

  impl Engine<'_> {
    fn set_glyph_point(
      &mut self,
      point_ix: u64,
      xy: (i32, i32),
    ) -> Result<(), crate::TestFailure> {
      use crate::test_support::Required;
      let Ok(zone) = self.graphics.zone_mut(ZonePointer::Glyph);
      let point = zone.point_mut(point_ix).required()?;
      point.x = {
        let Ok(value) = F26Dot6::from_bits(xy.0);
        value
      };
      point.y = {
        let Ok(value) = F26Dot6::from_bits(xy.1);
        value
      };
      Ok(())
    }
  }
}
