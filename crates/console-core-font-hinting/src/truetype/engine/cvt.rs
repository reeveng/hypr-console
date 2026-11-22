//! Managing the control value table.
//!
//! Implements 3 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#managing-the-control-value-table>

use super::{super::math, Engine, F26Dot6, OpResult};

impl Engine<'_> {
  pub(super) fn op_wcvtp(&mut self) -> OpResult {
    let value = self.value_stack.pop_f26dot6()?;
    let location = self.value_stack.pop_index()?;
    let result = self.cvt.set(location, value);

    match self.graphics.is_pedantic {
      crate::Pedantry::Strict => result,
      crate::Pedantry::Lenient => Ok(()),
    }
  }

  pub(super) fn op_wcvtf(&mut self) -> OpResult {
    let value = self.value_stack.pop()?;
    let location = self.value_stack.pop_index()?;
    let Ok(scaled_value) = math::mul(math::Product {
      left: value,
      right: self.graphics.scale,
    });
    let result = self.cvt.set(location, {
      let Ok(value_2) = F26Dot6::from_bits(scaled_value);
      value_2
    });

    match self.graphics.is_pedantic {
      crate::Pedantry::Strict => result,
      crate::Pedantry::Lenient => Ok(()),
    }
  }

  pub(super) fn op_rcvt(&mut self) -> OpResult {
    let Ok(location) = console_core_number_conversion::wrapped::<_, u64>({
      let answer = self.value_stack.pop()?;
      answer
    });
    let maybe_value = self.cvt.get(location);
    let value = match self.graphics.is_pedantic {
      crate::Pedantry::Strict => maybe_value?,
      crate::Pedantry::Lenient => match maybe_value {
        Ok(value) => value,
        Err(_outside_control_values) => F26Dot6::ZERO,
      },
    };
    self.value_stack.push({
      let Ok(value_2) = value.to_bits();
      value_2
    })
  }
}

#[cfg(test)]
mod tests {
  use super::super::{super::math, HintErrorKind, MockEngine};

  #[test]
  fn write_read() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    for i in 0..8 {
      engine.value_stack.push(i).required()?;
      engine
        .value_stack
        .push(i32::wrapping_mul(i, 2))
        .required()?;
      engine.op_wcvtp().required()?;
    }

    for i in 0..8 {
      engine.value_stack.push(i).required()?;
      engine.op_rcvt().required()?;

      {
        let queried_value = engine.value_stack.pop().required()?;
        assert_eq!(queried_value, i32::wrapping_mul(i, 2));
      }
    }

    Ok(())
  }

  #[test]
  fn write_scaled_read() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    let scale = 64;
    engine.graphics.scale = scale;

    for i in 0..8 {
      engine.value_stack.push(i).required()?;
      engine
        .value_stack
        .push(i32::wrapping_mul(i, 2))
        .required()?;

      engine.op_wcvtf().required()?;
    }

    for i in 0..8 {
      engine.value_stack.push(i).required()?;
      engine.op_rcvt().required()?;
      let value = engine.value_stack.pop().required()?;
      let Ok(expected) = math::mul(math::Product {
        left: i32::wrapping_mul(i, 2),
        right: scale,
      });
      assert_eq!(value, expected);
    }

    Ok(())
  }

  #[test]
  fn pedantry() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    let oob_index = 1000;

    engine.graphics.is_pedantic = crate::Pedantry::Lenient;
    engine.value_stack.push(oob_index).required()?;
    engine.value_stack.push(0).required()?;
    engine.op_wcvtp().required()?;
    engine.value_stack.push(oob_index).required()?;
    engine.value_stack.push(0).required()?;
    engine.op_wcvtf().required()?;
    engine.value_stack.push(oob_index).required()?;
    engine.op_rcvt().required()?;

    engine.graphics.is_pedantic = crate::Pedantry::Strict;
    engine.value_stack.push(oob_index).required()?;
    engine.value_stack.push(0).required()?;
    assert_eq!(
      engine.op_wcvtp(),
      Err(HintErrorKind::InvalidCvtIndex({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(oob_index);
        migrated_value
      }))
    );
    engine.value_stack.push(oob_index).required()?;
    engine.value_stack.push(0).required()?;
    assert_eq!(
      engine.op_wcvtf(),
      Err(HintErrorKind::InvalidCvtIndex({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(oob_index);
        migrated_value
      }))
    );
    engine.value_stack.push(oob_index).required()?;
    assert_eq!(
      engine.op_rcvt(),
      Err(HintErrorKind::InvalidCvtIndex({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(oob_index);
        migrated_value
      }))
    );

    Ok(())
  }
}
