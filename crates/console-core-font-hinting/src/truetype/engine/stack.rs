//! Managing the stack and pushing data onto the interpreter stack.
//!
//! Implements 26 instructions.
//!
//! See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#managing-the-stack>
//! and <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions#pushing-data-onto-the-interpreter-stack>

use read_fonts::tables::glyf::bytecode::InlineOperands;

use super::{Engine, OpResult};

impl Engine<'_> {
  pub(super) fn op_dup(&mut self) -> OpResult {
    self.value_stack.dup()
  }

  pub(super) fn op_pop(&mut self) -> OpResult {
    self.value_stack.pop()?;
    Ok(())
  }

  pub(super) fn op_clear(&mut self) -> OpResult {
    let Ok(()) = self.value_stack.clear();
    Ok(())
  }

  pub(super) fn op_swap(&mut self) -> OpResult {
    self.value_stack.swap()
  }

  pub(super) fn op_depth(&mut self) -> OpResult {
    let Ok(depth) = self.value_stack.len();
    let Ok(depth_value) = console_core_number_conversion::fitted::<_, i32>(depth);
    self.value_stack.push(depth_value)
  }

  pub(super) fn op_cindex(&mut self) -> OpResult {
    self.value_stack.copy_index()
  }

  pub(super) fn op_mindex(&mut self) -> OpResult {
    self.value_stack.move_index()
  }

  pub(super) fn op_roll(&mut self) -> OpResult {
    self.value_stack.roll()
  }

  pub(super) fn op_push(&mut self, operands: &InlineOperands) -> OpResult {
    self.value_stack.push_inline_operands(operands)
  }
}

#[cfg(test)]
mod tests {
  use super::super::MockEngine;
  use read_fonts::tables::glyf::bytecode::MockInlineOperands;

  #[test]
  fn stack_ops() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut mock = MockEngine::default();
    let Ok(mut engine) = mock.engine();
    let byte_args = MockInlineOperands::from_bytes(&[2, 4, 6, 8]);
    let word_args = MockInlineOperands::from_words(&[-2000, 4000, -6000, 8000]);
    let initial_stack = byte_args
      .operands()
      .values()
      .chain(word_args.operands().values())
      .collect::<Vec<_>>();

    engine.op_push(&byte_args.operands()).required()?;
    engine.op_push(&word_args.operands()).required()?;
    let required = engine.value_stack.values().required()?;
    assert_eq!(required, initial_stack);

    engine.op_depth().required()?;
    let Ok(expected_depth) =
      console_core_number_conversion::wrapped::<_, i32>(initial_stack.len());
    assert_eq!(engine.value_stack.pop(), Ok(expected_depth));

    engine.op_pop().required()?;
    engine.op_pop().required()?;
    let (expected, _) = initial_stack.split_last_chunk::<2>().required()?;
    let after_pop = engine.value_stack.values().required()?;
    assert_eq!(after_pop, expected);

    engine.op_swap().required()?;
    let swapped = engine.value_stack.values().required()?;
    let swapped_tail = swapped.get(4..).required()?;
    assert_eq!(swapped_tail, &[4000, -2000]);

    engine.op_roll().required()?;
    let rolled = engine.value_stack.values().required()?;
    let rolled_tail = rolled.get(3..).required()?;
    assert_eq!(rolled_tail, &[4000, -2000, 8]);

    engine.value_stack.push(4).required()?;
    engine.op_cindex().required()?;
    assert_eq!(engine.value_stack.peek(), Ok(Some(6)));

    engine.value_stack.push(3).required()?;
    engine.op_mindex().required()?;
    assert_eq!(engine.value_stack.peek(), Ok(Some(-2000)));

    Ok(())
  }
}
