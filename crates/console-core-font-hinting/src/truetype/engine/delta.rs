//! Managing delta exceptions.
//!
//! Implements 6 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#managing-exceptions>

use super::{
  super::{graphics::CoordAxis, zone::PointTouch},
  Engine, F26Dot6, OpResult,
};
use read_fonts::tables::glyf::bytecode::Opcode;

impl Engine<'_> {
  pub(super) fn op_deltap(&mut self, opcode: Opcode) -> OpResult {
    let gs = &mut self.graphics;
    let Ok(ppem) = console_core_number_conversion::wrapped::<_, u32>(gs.ppem);
    let Ok(point_count) = console_core_number_conversion::fitted::<_, u64>(
      {
        let Ok(value) = gs.zp0();
        value
      }
      .points
      .len(),
    );
    let n = self.value_stack.pop_count_checked()?;

    let Ok(stack_len) = self.value_stack.len();
    let n_2 = n.min(stack_len.wrapping_div(2));
    let bias = u32::wrapping_add(
      match ::core::primitive::bool::then_some(opcode == Opcode::DELTAP2, ()) {
        ::core::option::Option::Some(()) => 16,
        ::core::option::Option::None => {
          match ::core::primitive::bool::then_some(opcode == Opcode::DELTAP3, ()) {
            ::core::option::Option::Some(()) => 32,
            ::core::option::Option::None => 0,
          }
        }
      },
      {
        let Ok(value) = console_core_number_conversion::wrapped::<_, u32>(gs.delta_base);
        value
      },
    );
    let back_compat = gs.backward_compatibility;
    let did_iup = matches!(gs.did_iup_x, crate::InterpolationProgress::Applied)
      && matches!(gs.did_iup_y, crate::InterpolationProgress::Applied);

    for _ in 0..n_2 {
      let point_ix = self.value_stack.pop_index()?;
      let mut b = self.value_stack.pop()?;

      match ::core::primitive::bool::then_some(
        point_ix
          >= ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(point_count);
            value
          }),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      let mut c = u32::wrapping_shr(
        ({
          let Ok(value) = console_core_number_conversion::wrapped::<_, u32>(b);
          value
        } & 0xF0),
        4_u32,
      );
      c = u32::wrapping_add(c, bias);

      match ::core::primitive::bool::then_some(ppem == c, ()) {
        ::core::option::Option::Some(()) => {
          b = i32::wrapping_sub(b & 0xF, 8);

          match ::core::primitive::bool::then_some(b >= 0, ()) {
            ::core::option::Option::Some(()) => {
              b = i32::wrapping_add(b, 1);
            }
            ::core::option::Option::None => {}
          }

          b = i32::wrapping_mul(
            b,
            i32::wrapping_shl(1, {
              let Ok(value) =
                console_core_number_conversion::wrapped::<_, u32>(i32::wrapping_sub(6, {
                  let Ok(value) =
                    console_core_number_conversion::wrapped::<_, i32>(gs.delta_shift);
                  value
                }));
              value
            }),
          );
          let Ok(distance) = F26Dot6::from_bits(b);

          let composite_vertical =
            matches!(gs.glyph_kind, crate::GlyphKind::Composite) && gs.freedom_vector.y != 0;
          let should_move = match (back_compat, did_iup, composite_vertical) {
            (crate::BackwardCompatibility::Disabled, _, _) => true,
            (crate::BackwardCompatibility::Enabled, true, _) => false,
            (crate::BackwardCompatibility::Enabled, false, true) => true,
            (crate::BackwardCompatibility::Enabled, false, false) => {
              let Ok(zone) = gs.zp0();
              let touch = zone.point_touch(point_ix, CoordAxis::Y)?;

              matches!(touch, PointTouch::Touched)
            }
          };

          match should_move.then_some(()) {
            Some(()) => gs.move_point(gs.zp0, point_ix, distance)?,
            None => {}
          }
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(())
  }

  pub(super) fn op_deltac(&mut self, opcode: Opcode) -> OpResult {
    let gs = &mut self.graphics;
    let Ok(ppem) = console_core_number_conversion::wrapped::<_, u32>(gs.ppem);
    let n = self.value_stack.pop_count_checked()?;

    let Ok(stack_len) = self.value_stack.len();
    let n_2 = n.min(stack_len.wrapping_div(2));
    let bias = u32::wrapping_add(
      match ::core::primitive::bool::then_some(opcode == Opcode::DELTAC2, ()) {
        ::core::option::Option::Some(()) => 16,
        ::core::option::Option::None => {
          match ::core::primitive::bool::then_some(opcode == Opcode::DELTAC3, ()) {
            ::core::option::Option::Some(()) => 32,
            ::core::option::Option::None => 0,
          }
        }
      },
      {
        let Ok(value) = console_core_number_conversion::wrapped::<_, u32>(gs.delta_base);
        value
      },
    );

    for _ in 0..n_2 {
      let cvt_ix = self.value_stack.pop_index()?;
      let mut b = self.value_stack.pop()?;
      let mut c = u32::wrapping_shr(
        ({
          let Ok(value) = console_core_number_conversion::wrapped::<_, u32>(b);
          value
        } & 0xF0),
        4_u32,
      );
      c = u32::wrapping_add(c, bias);

      match ::core::primitive::bool::then_some(ppem == c, ()) {
        ::core::option::Option::Some(()) => {
          b = i32::wrapping_sub(b & 0xF, 8);

          match ::core::primitive::bool::then_some(b >= 0, ()) {
            ::core::option::Option::Some(()) => {
              b = i32::wrapping_add(b, 1);
            }
            ::core::option::Option::None => {}
          }

          b = i32::wrapping_mul(
            b,
            i32::wrapping_shl(1, {
              let Ok(value) =
                console_core_number_conversion::wrapped::<_, u32>(i32::wrapping_sub(6, {
                  let Ok(value) =
                    console_core_number_conversion::wrapped::<_, i32>(gs.delta_shift);
                  value
                }));
              value
            }),
          );
          let cvt_val = self.cvt.get(cvt_ix)?;
          self.cvt.set(
            cvt_ix,
            cvt_val + {
              let Ok(value) = F26Dot6::from_bits(b);
              value
            },
          )?;
        }
        ::core::option::Option::None => {}
      }
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::super::{super::zone::ZonePointer, HintErrorKind, MockEngine};
  use raw::{tables::glyf::bytecode::Opcode, types::Point};

  #[test]
  fn deltap() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.zp0 = ZonePointer::Glyph;
    let raw_ppem = 16;
    let raw_adjustment = 7;

    for (point_ix, (ppem_bias, opcode)) in [
      (0, Opcode::DELTAP1),
      (16, Opcode::DELTAP2),
      (32, Opcode::DELTAP3),
    ]
    .iter()
    .enumerate()
    {
      let ppem = i32::wrapping_add(raw_ppem, *(ppem_bias));
      engine.graphics.ppem = ppem;

      let packed_ppem = i32::wrapping_sub(raw_ppem, {
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, i32>(engine.graphics.delta_base);
        migrated_value
      });
      engine
        .value_stack
        .push(
          i32::wrapping_shl(packed_ppem, {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(4);
            migrated_value
          }) | raw_adjustment,
        )
        .required()?;

      engine
        .value_stack
        .push({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(point_ix);
          migrated_value
        })
        .required()?;

      engine.value_stack.push(1).required()?;
      engine.op_deltap(*opcode).required()?;
      let point = (*match &engine.graphics.zones {
        [_, element, ..] => element,
      })
      .point({
        let Ok(value) = console_core_number_conversion::fitted::<_, u64>(point_ix);
        value
      })
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
          let Ok(value) = Point::new(-8, 0);
          value
        })
      );
    }

    Ok(())
  }

  #[test]
  fn deltac() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    let raw_ppem = 16;
    let raw_adjustment = 7;

    for (cvt_ix, (ppem_bias, opcode)) in [
      (0, Opcode::DELTAC1),
      (16, Opcode::DELTAC2),
      (32, Opcode::DELTAC3),
    ]
    .iter()
    .enumerate()
    {
      let ppem = i32::wrapping_add(raw_ppem, *(ppem_bias));
      engine.graphics.ppem = ppem;

      let packed_ppem = i32::wrapping_sub(raw_ppem, {
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, i32>(engine.graphics.delta_base);
        migrated_value
      });
      engine
        .value_stack
        .push(
          i32::wrapping_shl(packed_ppem, {
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(4);
            migrated_value
          }) | raw_adjustment,
        )
        .required()?;

      engine
        .value_stack
        .push({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(cvt_ix);
          migrated_value
        })
        .required()?;

      engine.value_stack.push(1).required()?;
      engine.op_deltac(*opcode).required()?;
      let value = engine
        .cvt
        .get({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(cvt_ix);
          value
        })
        .required()?;
      assert_eq!(
        ({
          let Ok(value_2) = value.to_bits();
          value_2
        }),
        -8
      );
    }

    Ok(())
  }

  #[test]
  fn deltap_negative_count() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    let stack = [0, 0, -1];

    for value in &stack {
      engine.value_stack.push(*value).required()?;
    }

    engine.op_deltap(Opcode::DELTAP3).required()?;

    let Ok(()) = engine
      .value_stack
      .set_validation(crate::Validation::Pedantic);

    for value in &stack {
      engine.value_stack.push(*value).required()?;
    }

    assert!(matches!(
      engine.op_deltap(Opcode::DELTAP3),
      Err(HintErrorKind::InvalidStackValue(-1))
    ));

    Ok(())
  }

  #[test]
  fn deltac_negative_count() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    let stack = [0, 0, -1];

    for value in &stack {
      engine.value_stack.push(*value).required()?;
    }

    engine.op_deltac(Opcode::DELTAC3).required()?;

    let Ok(()) = engine
      .value_stack
      .set_validation(crate::Validation::Pedantic);

    for value in &stack {
      engine.value_stack.push(*value).required()?;
    }

    assert!(matches!(
      engine.op_deltac(Opcode::DELTAC3),
      Err(HintErrorKind::InvalidStackValue(-1))
    ));

    Ok(())
  }
}
