//! Logical functions.
//!
//! Implements 11 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#logical-functions>

use super::{Engine, F26Dot6, OpResult};

impl Engine<'_> {
  pub(super) fn op_lt(&mut self) -> OpResult {
    self.value_stack.apply_binary(|a, b| Ok(i32::from(a < b)))
  }

  pub(super) fn op_lteq(&mut self) -> OpResult {
    self.value_stack.apply_binary(|a, b| Ok(i32::from(a <= b)))
  }

  pub(super) fn op_gt(&mut self) -> OpResult {
    self.value_stack.apply_binary(|a, b| Ok(i32::from(a > b)))
  }

  pub(super) fn op_gteq(&mut self) -> OpResult {
    self.value_stack.apply_binary(|a, b| Ok(i32::from(a >= b)))
  }

  pub(super) fn op_eq(&mut self) -> OpResult {
    self.value_stack.apply_binary(|a, b| Ok(i32::from(a == b)))
  }

  pub(super) fn op_neq(&mut self) -> OpResult {
    self.value_stack.apply_binary(|a, b| Ok(i32::from(a != b)))
  }

  pub(super) fn op_odd(&mut self) -> OpResult {
    let round_state = self.graphics.round_state;
    self.value_stack.apply_unary(|e1| {
      let rounded = round_state.round({
        let Ok(value) = F26Dot6::from_bits(e1);
        value
      })?;
      Ok(i32::from(
        {
          let Ok(value) = rounded.to_bits();
          value
        } & 127
          == 64,
      ))
    })
  }

  pub(super) fn op_even(&mut self) -> OpResult {
    let round_state = self.graphics.round_state;
    self.value_stack.apply_unary(|e1| {
      let rounded = round_state.round({
        let Ok(value) = F26Dot6::from_bits(e1);
        value
      })?;
      Ok(i32::from(
        {
          let Ok(value) = rounded.to_bits();
          value
        } & 127
          == 0,
      ))
    })
  }

  pub(super) fn op_and(&mut self) -> OpResult {
    self
      .value_stack
      .apply_binary(|a, b| Ok(i32::from(a != 0 && b != 0)))
  }

  pub(super) fn op_or(&mut self) -> OpResult {
    self
      .value_stack
      .apply_binary(|a, b| Ok(i32::from(a != 0 || b != 0)))
  }

  pub(super) fn op_not(&mut self) -> OpResult {
    self.value_stack.apply_unary(|e| Ok(i32::from(e == 0)))
  }
}

#[cfg(test)]
mod tests {
  use super::super::MockEngine;

  #[test]
  fn compare_ops() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    for a in -10..=10 {
      for b in -10..=10 {
        let input = &[a, b];
        engine.test_exec(input, a < b, |engine_2| {
          engine_2.op_lt().required()?;
          Ok(())
        })?;
        engine.test_exec(input, a <= b, |engine_3| {
          engine_3.op_lteq().required()?;
          Ok(())
        })?;
        engine.test_exec(input, a > b, |engine_4| {
          engine_4.op_gt().required()?;
          Ok(())
        })?;
        engine.test_exec(input, a >= b, |engine_5| {
          engine_5.op_gteq().required()?;
          Ok(())
        })?;
        engine.test_exec(input, a == b, |engine_6| {
          engine_6.op_eq().required()?;
          Ok(())
        })?;
        engine.test_exec(input, a != b, |engine_7| {
          engine_7.op_neq().required()?;
          Ok(())
        })?;
      }
    }

    Ok(())
  }

  #[test]
  fn parity_ops() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    let cases = [
      (0, true),
      (64, false),
      (128, true),
      (192, false),
      (256, true),
      (57, false),
      (-128, true),
    ];
    for (input, is_even) in cases {
      engine.test_exec(&[input], is_even, |engine_2| {
        engine_2.op_even().required()?;
        Ok(())
      })?;
    }

    for (input, is_even) in cases {
      engine.test_exec(&[input], !is_even, |engine_3| {
        engine_3.op_odd().required()?;
        Ok(())
      })?;
    }

    Ok(())
  }

  #[test]
  fn not_op() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    engine.test_exec(&[0], 1, |engine_2| {
      engine_2.op_not().required()?;
      Ok(())
    })?;
    engine.test_exec(&[234234], 0, |engine_3| {
      engine_3.op_not().required()?;
      Ok(())
    })?;

    Ok(())
  }

  #[test]
  fn and_or_ops() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();

    for a in -10..=10 {
      for b in -10..=10 {
        let input = &[a, b];
        let a_2 = a != 0;
        let b_2 = b != 0;
        engine.test_exec(input, a_2 && b_2, |engine_2| {
          engine_2.op_and().required()?;
          Ok(())
        })?;
        engine.test_exec(input, a_2 || b_2, |engine_3| {
          engine_3.op_or().required()?;
          Ok(())
        })?;
      }
    }

    Ok(())
  }
}
