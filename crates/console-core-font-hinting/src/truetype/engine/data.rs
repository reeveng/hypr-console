//! Reading and writing data.
//!
//! Implements 7 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#reading-and-writing-data>

use super::{
  super::{math, zone::ZonePointer},
  Engine, OpResult,
};

impl Engine<'_> {
  pub(super) fn op_gc(&mut self, opcode: u8) -> OpResult {
    let p = self.value_stack.pop_index()?;
    let gs = &mut self.graphics;

    match ::core::primitive::bool::then_some(!matches!(gs.is_pedantic, crate::Pedantry::Strict)
      && !(gs.point_bounds([(gs.zp2, p)]) == Ok(crate::PointBounds::Within)), ())
    {
      ::core::option::Option::Some(()) => {
        self.value_stack.push(0)?;
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let value = match ::core::primitive::bool::then_some((opcode & 1) != 0, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(value) = gs.dual_project(
          {
            let answer = {
              let Ok(value) = gs.zp2();
              value
            }
            .original(p)?;
            answer
          },
          Default::default(),
        );
        value
      }
      ::core::option::Option::None => {
        let Ok(value) = gs.project(
          {
            let answer = {
              let Ok(value) = gs.zp2();
              value
            }
            .point(p)?;
            answer
          },
          Default::default(),
        );
        value
      }
    };
    self.value_stack.push({
      let Ok(value_2) = value.to_bits();
      value_2
    })?;
    Ok(())
  }

  pub(super) fn op_scfs(&mut self) -> OpResult {
    let value = self.value_stack.pop_f26dot6()?;
    let p = self.value_stack.pop_index()?;
    let gs = &mut self.graphics;
    let Ok(projection) = gs.project(
      {
        let answer = {
          let Ok(value_2) = gs.zp2();
          value_2
        }
        .point(p)?;
        answer
      },
      Default::default(),
    );
    gs.move_point(gs.zp2, p, {
      let Ok(value_3) = value.wrapping_sub(projection);
      value_3
    })?;

    match gs.zp2 {
      ZonePointer::Twilight => {
        let Ok(twilight) = gs.zone_mut(ZonePointer::Twilight);
        *{
          let answer = twilight.original_mut(p)?;
          answer
        } = {
          let answer = twilight.point(p)?;
          answer
        };
      }
      ZonePointer::Glyph => {}
    }

    Ok(())
  }

  pub(super) fn op_md(&mut self, opcode: u8) -> OpResult {
    let p1 = self.value_stack.pop_index()?;
    let p2 = self.value_stack.pop_index()?;
    let gs = &self.graphics;

    match ::core::primitive::bool::then_some(!matches!(gs.is_pedantic, crate::Pedantry::Strict)
      && !(gs.point_bounds([(gs.zp0, p2), (gs.zp1, p1)]) == Ok(crate::PointBounds::Within)), ())
    {
      ::core::option::Option::Some(()) => {
        self.value_stack.push(0)?;
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(zp0) = gs.zp0();
    let Ok(zp1) = gs.zp1();
    let current = opcode & 1 != 0;
    let twilight =
      matches!(gs.zp0, ZonePointer::Twilight) || matches!(gs.zp1, ZonePointer::Twilight);

    let distance = match (current, twilight) {
      (true, _) => {
        let point = zp0.point(p2)?;
        let reference = zp1.point(p1)?;
        let Ok(projected) = gs.project(point, reference);
        let Ok(bits) = projected.to_bits();
        bits
      }
      (false, true) => {
        let point = zp0.original(p2)?;
        let reference = zp1.original(p1)?;
        let Ok(projected) = gs.dual_project(point, reference);
        let Ok(bits) = projected.to_bits();
        bits
      }
      (false, false) => {
        let Ok(point) = zp0.unscaled(p2);
        let Ok(reference) = zp1.unscaled(p1);
        let Ok(projected) = gs.dual_project_unscaled(point, reference);
        let Ok(scale) = gs.unscaled_to_pixels();
        let Ok(bits) = math::mul(math::Product {
          left: projected,
          right: scale,
        });
        bits
      }
    };

    self.value_stack.push(distance)
  }

  pub(super) fn op_mppem(&mut self) -> OpResult {
    self.value_stack.push(self.graphics.ppem)
  }

  pub(super) fn op_mps(&mut self) -> OpResult {
    self
      .value_stack
      .push(i32::wrapping_mul(self.graphics.ppem, 64))
  }
}

#[cfg(test)]
mod tests {
  use super::super::{super::zone::ZonePointer, Engine, MockEngine, math};
  use raw::types::F26Dot6;

  #[test]
  fn measure_ppem_and_point_size() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    let ppem = 20;
    engine.graphics.ppem = ppem;
    engine.op_mppem().required()?;

    {
      let queried_value = engine.value_stack.pop().required()?;
      assert_eq!(queried_value, ppem);
    }

    engine.op_mps().required()?;

    {
      let queried_value = engine.value_stack.pop().required()?;
      assert_eq!(queried_value, i32::wrapping_mul(ppem, 64));
    }

    Ok(())
  }

  #[test]
  fn gc() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });

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
    engine.op_gc(0).required()?;

    {
      let queried_value = engine.value_stack.pop().required()?;
      assert_eq!(queried_value, 4);
    }

    let point_2 = (*match &mut engine.graphics.zones {
      [_, element, ..] => element,
    })
    .original_mut(1)
    .required()?;
    point_2.x = {
      let Ok(value) = F26Dot6::from_bits(-64);
      value
    };
    point_2.y = {
      let Ok(value) = F26Dot6::from_bits(521);
      value
    };
    engine.value_stack.push(1).required()?;
    engine.op_gc(1).required()?;

    {
      let queried_value = engine.value_stack.pop().required()?;
      assert_eq!(queried_value, 176);
    }

    Ok(())
  }

  #[test]
  fn scfs() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });

    engine.graphics.backward_compatibility = crate::BackwardCompatibility::Disabled;
    engine.graphics.did_iup_x = crate::InterpolationProgress::Applied;
    engine.graphics.did_iup_y = crate::InterpolationProgress::Applied;

    engine.graphics.zp2 = ZonePointer::Twilight;
    let point = (*match &mut engine.graphics.zones {
      [element, ..] => element,
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

    {
      let queried_value = (*match &engine.graphics.zones {
        [element, ..] => element,
      })
      .point(1)
      .required()?;
      let required = (*match &engine.graphics.zones {
        [element, ..] => element,
      })
      .original(1)
      .required()?;
      assert_ne!(queried_value, required);
    }

    engine.value_stack.push(1).required()?;

    engine.value_stack.push(42).required()?;

    engine.op_scfs().required()?;
    let point_2 = (*match &engine.graphics.zones {
      [element, ..] => element,
    })
    .point(1)
    .required()?;
    assert_eq!(
      ({
        let Ok(value) = point_2.x.to_bits();
        value
      }),
      166
    );
    assert_eq!(
      ({
        let Ok(value) = point_2.y.to_bits();
        value
      }),
      -239
    );

    let required = (*match &engine.graphics.zones {
      [element, ..] => element,
    })
    .original(1)
    .required()?;
    assert_eq!(point_2, required);

    Ok(())
  }

  #[test]
  fn md_scaled() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });

    let Ok(zone) = engine.graphics.zone_mut(ZonePointer::Glyph);
    let point1 = zone.point_mut(1).required()?;
    point1.x = {
      let Ok(value) = F26Dot6::from_bits(132);
      value
    };
    point1.y = {
      let Ok(value) = F26Dot6::from_bits(-256);
      value
    };
    let point2 = zone.point_mut(3).required()?;
    point2.x = {
      let Ok(value) = F26Dot6::from_bits(-64);
      value
    };
    point2.y = {
      let Ok(value) = F26Dot6::from_bits(100);
      value
    };

    engine.value_stack.push(1).required()?;
    engine.value_stack.push(3).required()?;
    engine.op_md(1).required()?;

    {
      let queried_value = engine.value_stack.pop().required()?;
      assert_eq!(queried_value, 16);
    }

    Ok(())
  }

  #[test]
  fn md_unscaled() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });

    engine.graphics.scale = 375912;
    engine.value_stack.push(1).required()?;
    engine.value_stack.push(3).required()?;
    engine.op_md(0).required()?;

    {
      let queried_value = engine.value_stack.pop().required()?;
      assert_eq!(queried_value, 11);
    }

    Ok(())
  }

  #[test]
  fn md_twilight() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    ({
      let Ok(migrated_value) = set_test_vectors(&mut engine);
      migrated_value
    });

    engine.graphics.zp0 = ZonePointer::Twilight;
    engine.graphics.zp1 = ZonePointer::Twilight;

    let Ok(zone) = engine.graphics.zone_mut(ZonePointer::Twilight);
    let point1 = zone.original_mut(1).required()?;
    point1.x = {
      let Ok(value) = F26Dot6::from_bits(132);
      value
    };
    point1.y = {
      let Ok(value) = F26Dot6::from_bits(-256);
      value
    };
    let point2 = zone.original_mut(3).required()?;
    point2.x = {
      let Ok(value) = F26Dot6::from_bits(-64);
      value
    };
    point2.y = {
      let Ok(value) = F26Dot6::from_bits(100);
      value
    };

    engine.value_stack.push(1).required()?;
    engine.value_stack.push(3).required()?;
    engine.op_md(0).required()?;

    {
      let queried_value = engine.value_stack.pop().required()?;
      assert_eq!(queried_value, 16);
    }

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
}
