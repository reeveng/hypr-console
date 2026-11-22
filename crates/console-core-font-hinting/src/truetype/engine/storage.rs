//! Managing the storage area.
//!
//! Implements 2 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#managing-the-storage-area>

use super::{Engine, OpResult};

impl Engine<'_> {
  pub(super) fn op_rs(&mut self) -> OpResult {
    let location = self.value_stack.pop_index()?;
    let maybe_value = self.storage.get(location);
    let value = match self.graphics.is_pedantic {
      crate::Pedantry::Strict => maybe_value?,
      crate::Pedantry::Lenient => match maybe_value {
        Ok(value) => value,
        Err(_outside_storage) => 0,
      },
    };
    self.value_stack.push(value)
  }

  pub(super) fn op_ws(&mut self) -> OpResult {
    let value = self.value_stack.pop()?;
    let location = self.value_stack.pop_index()?;
    let result = self.storage.set(location, value);

    match self.graphics.is_pedantic {
      crate::Pedantry::Strict => result,
      crate::Pedantry::Lenient => Ok(()),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::super::{HintErrorKind, MockEngine};

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
      engine.op_ws().required()?;
    }

    for i in 0..8 {
      engine.value_stack.push(i).required()?;
      engine.op_rs().required()?;

      {
        let queried_value = engine.value_stack.pop().required()?;
        assert_eq!(queried_value, i32::wrapping_mul(i, 2));
      }
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
    engine.op_ws().required()?;
    engine.value_stack.push(oob_index).required()?;
    engine.op_rs().required()?;

    engine.graphics.is_pedantic = crate::Pedantry::Strict;
    engine.value_stack.push(oob_index).required()?;
    engine.value_stack.push(0).required()?;
    assert_eq!(
      engine.op_ws(),
      Err(HintErrorKind::InvalidStorageIndex({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(oob_index);
        migrated_value
      }))
    );
    engine.value_stack.push(oob_index).required()?;
    assert_eq!(
      engine.op_rs(),
      Err(HintErrorKind::InvalidStorageIndex({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(oob_index);
        migrated_value
      }))
    );

    Ok(())
  }
}
