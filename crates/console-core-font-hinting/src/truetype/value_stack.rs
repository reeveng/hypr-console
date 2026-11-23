//! Value stack for the TrueType interpreter.

use super::error::HintErrorKind;
use crate::Validation;
use HintErrorKind::{ValueStackOverflow, ValueStackUnderflow};
use console_core_never::Never;
use raw::types::F26Dot6;
use read_fonts::tables::glyf::bytecode::InlineOperands;

pub struct ValueStack<'a> {
  values: &'a mut [i32],
  len: u64,
  validation: Validation,
}

impl<'a> ValueStack<'a> {
  pub fn new(values: &'a mut [i32], validation: Validation) -> Result<Self, Never> {
    Ok(Self {
      values,
      len: 0,
      validation,
    })
  }

  #[cfg(test)]
  pub fn set_validation(&mut self, validation: Validation) -> Result<(), Never> {
    self.validation = validation;
    Ok(())
  }

  pub fn len(&self) -> Result<u64, Never> {
    Ok(self.len)
  }

  #[cfg(test)]
  pub fn values(&self) -> Result<&[i32], HintErrorKind> {
    self
      .values
      .get({
        let Ok(value) = console_core_number_conversion::indices_to(..self.len);
        value
      })
      .ok_or(ValueStackOverflow)
  }

  pub fn push(&mut self, value: i32) -> Result<(), HintErrorKind> {
    let target = self
      .values
      .get_mut({
        let Ok(value_2) = console_core_number_conversion::index(self.len);
        value_2
      })
      .ok_or(ValueStackOverflow)?;
    *target = value;
    self.len = self.len.saturating_add(1);
    Ok(())
  }

  pub fn push_inline_operands(&mut self, operands: &InlineOperands) -> Result<(), HintErrorKind> {
    let end = self
      .len
      .checked_add({
        let Ok(length) = console_core_number_conversion::index(operands.len());

        {
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(length);
          value
        }
      })
      .ok_or(ValueStackOverflow)?;
    let destination = self
      .values
      .get_mut({
        let Ok(value) = console_core_number_conversion::indices(self.len..end);
        value
      })
      .ok_or(ValueStackOverflow)?;

    for (target, value) in destination.iter_mut().zip(operands.values()) {
      *target = value;
    }

    self.len = end;
    Ok(())
  }

  pub fn peek(&self) -> Result<Option<i32>, Never> {
    Ok(match self.len.checked_sub(1) {
      Some(index) => self
        .values
        .get({
          let Ok(value) = console_core_number_conversion::index(index);
          value
        })
        .copied(),
      None => None,
    })
  }

  pub fn pop(&mut self) -> Result<i32, HintErrorKind> {
    let Ok(top) = self.peek();

    match top {
      Some(value) => {
        self.len = self.len.saturating_sub(1);
        Ok(value)
      }
      None => match self.validation {
        Validation::Pedantic => Err(ValueStackUnderflow),
        Validation::Compatible => Ok(0),
      },
    }
  }

  pub fn pop_f26dot6(&mut self) -> Result<F26Dot6, HintErrorKind> {
    let value = self.pop()?;
    Ok({
      let Ok(value_2) = F26Dot6::from_bits(value);
      value_2
    })
  }

  pub fn pop_index(&mut self) -> Result<u64, HintErrorKind> {
    let value = self.pop()?;
    u64::try_from(value).map_err(|_not_an_index| HintErrorKind::InvalidStackValue(value))
  }

  pub fn pop_count_checked(&mut self) -> Result<u64, HintErrorKind> {
    let value = self.pop()?;

    match ::core::primitive::bool::then_some(
      value < 0 && matches!(self.validation, Validation::Pedantic),
      (),
    ) {
      ::core::option::Option::Some(()) => Err(HintErrorKind::InvalidStackValue(value)),
      ::core::option::Option::None => {
        u64::try_from(value.max(0)).map_err(|_too_wide| HintErrorKind::InvalidStackValue(value))
      }
    }
  }

  pub fn apply_unary(
    &mut self,
    mut operation: impl FnMut(i32) -> Result<i32, HintErrorKind>,
  ) -> Result<(), HintErrorKind> {
    let input = self.pop()?;
    let output = operation(input)?;
    self.push(output)
  }

  pub fn apply_binary(
    &mut self,
    mut operation: impl FnMut(i32, i32) -> Result<i32, HintErrorKind>,
  ) -> Result<(), HintErrorKind> {
    let right = self.pop()?;
    let left = self.pop()?;
    let output = operation(left, right)?;
    self.push(output)
  }

  pub fn clear(&mut self) -> Result<(), Never> {
    self.len = 0;
    Ok(())
  }

  pub fn dup(&mut self) -> Result<(), HintErrorKind> {
    let Ok(top) = self.peek();

    match top {
      Some(value) => self.push(value),
      None => match self.validation {
        Validation::Pedantic => Err(ValueStackUnderflow),
        Validation::Compatible => self.push(0),
      },
    }
  }

  pub fn swap(&mut self) -> Result<(), HintErrorKind> {
    let right = self.pop()?;
    let left = self.pop()?;
    self.push(right)?;
    self.push(left)
  }

  pub fn copy_index(&mut self) -> Result<(), HintErrorKind> {
    let top = self.len.checked_sub(1).ok_or(ValueStackUnderflow)?;
    let value = self
      .values
      .get({
        let Ok(value) = console_core_number_conversion::index(top);
        value
      })
      .copied()
      .ok_or(ValueStackUnderflow)?;
    let offset = u64::try_from(value).map_err(|_not_an_index| ValueStackUnderflow)?;
    let source = top.checked_sub(offset).ok_or(ValueStackUnderflow)?;
    let value_3 = self
      .values
      .get({
        let Ok(value_2) = console_core_number_conversion::index(source);
        value_2
      })
      .copied()
      .ok_or(ValueStackUnderflow)?;
    let target = self
      .values
      .get_mut({
        let Ok(value_4) = console_core_number_conversion::index(top);
        value_4
      })
      .ok_or(ValueStackUnderflow)?;
    *target = value_3;
    Ok(())
  }

  pub fn move_index(&mut self) -> Result<(), HintErrorKind> {
    let top = self.len.checked_sub(1).ok_or(ValueStackUnderflow)?;
    let offset_value = self
      .values
      .get({
        let Ok(value) = console_core_number_conversion::index(top);
        value
      })
      .copied()
      .ok_or(ValueStackUnderflow)?;
    let offset = u64::try_from(offset_value).map_err(|_not_an_index| ValueStackUnderflow)?;
    let source = top.checked_sub(offset).ok_or(ValueStackUnderflow)?;
    let target_index = top.checked_sub(1).ok_or(ValueStackUnderflow)?;
    let value = self
      .values
      .get({
        let Ok(value) = console_core_number_conversion::index(source);
        value
      })
      .copied()
      .ok_or(ValueStackUnderflow)?;
    let window = self
      .values
      .get_mut({
        let Ok(value_2) = console_core_number_conversion::indices(source..self.len);
        value_2
      })
      .ok_or(ValueStackUnderflow)?;
    let mut items = window.iter_mut();
    let mut previous = items.next().ok_or(ValueStackUnderflow)?;

    for next in items {
      *previous = *next;
      previous = next;
    }

    let target = self
      .values
      .get_mut({
        let Ok(value_3) = console_core_number_conversion::index(target_index);
        value_3
      })
      .ok_or(ValueStackUnderflow)?;
    *target = value;
    self.len = top;
    Ok(())
  }

  pub fn roll(&mut self) -> Result<(), HintErrorKind> {
    let last = self.pop()?;
    let middle = self.pop()?;
    let first = self.pop()?;
    self.push(middle)?;
    self.push(last)?;
    self.push(first)?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::{HintErrorKind, ValueStack};
  use read_fonts::tables::glyf::bytecode::MockInlineOperands;

  fn filled_stack(values: &mut [i32]) -> Result<ValueStack<'_>, console_core_never::Never> {
    let Ok(len) = console_core_number_conversion::fitted::<_, u64>(values.len());
    Ok(ValueStack {
      values,
      len,
      validation: crate::Validation::Pedantic,
    })
  }

  #[test]
  fn push() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut buffer = [0; 4];
    let Ok(mut stack) = ValueStack::new(&mut buffer, crate::Validation::Pedantic);

    for i in 0..4 {
      stack.push(i).required()?;
      assert_eq!(stack.peek(), Ok(Some(i)));
    }

    assert!(matches!(
      stack.push(0),
      Err(HintErrorKind::ValueStackOverflow)
    ));

    Ok(())
  }

  #[test]
  fn push_args() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut buffer = [0; 32];
    let Ok(mut stack) = ValueStack::new(&mut buffer, crate::Validation::Pedantic);
    let values = [-5, 2, 2845, 92, -26, 42, i16::MIN, i16::MAX];
    let mock_args = MockInlineOperands::from_words(&values);
    stack
      .push_inline_operands(&mock_args.operands())
      .required()?;
    let mut popped = vec![];

    for _ in &values {
      let required = stack.pop().required()?;
      popped.push(required);
    }

    assert!(
      values
        .iter()
        .rev()
        .map(|x| i32::from(*x))
        .eq(popped.iter().copied())
    );

    Ok(())
  }

  #[test]
  fn pop() {
    let mut buffer = [0, 1, 2, 3];
    let Ok(mut stack) = filled_stack(&mut buffer);

    for i in (0..4).rev() {
      assert_eq!(stack.pop(), Ok(i));
    }

    assert!(matches!(
      stack.pop(),
      Err(HintErrorKind::ValueStackUnderflow)
    ));
  }

  #[test]
  fn dup() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut buffer = [1, 2, 3, 0];
    let Ok(mut stack) = filled_stack(&mut buffer);

    stack.pop().required()?;
    stack.dup().required()?;
    let required = stack.values().required()?;
    assert_eq!(required, &[1, 2, 3, 3]);

    Ok(())
  }

  #[test]
  fn swap() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut buffer = [1, 2, 3];
    let Ok(mut stack) = filled_stack(&mut buffer);
    stack.swap().required()?;
    let required = stack.values().required()?;
    assert_eq!(required, &[1, 3, 2]);

    Ok(())
  }

  #[test]
  fn copy_index() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut buffer = [4, 10, 2, 1, 3];
    let Ok(mut stack) = filled_stack(&mut buffer);
    stack.copy_index().required()?;
    let required = stack.values().required()?;
    assert_eq!(required, &[4, 10, 2, 1, 10]);

    Ok(())
  }

  #[test]
  fn move_index() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut buffer = [4, 10, 2, 1, 3];
    let Ok(mut stack) = filled_stack(&mut buffer);
    stack.move_index().required()?;
    let required = stack.values().required()?;
    assert_eq!(required, &[4, 2, 1, 10]);

    Ok(())
  }

  #[test]
  fn roll() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut buffer = [1, 2, 3];
    let Ok(mut stack) = filled_stack(&mut buffer);
    stack.roll().required()?;
    let required = stack.values().required()?;
    assert_eq!(required, &[2, 3, 1]);

    Ok(())
  }

  #[test]
  fn unnop() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut buffer = [42];
    let Ok(mut stack) = filled_stack(&mut buffer);
    stack.apply_unary(|a| Ok((a).wrapping_neg())).required()?;
    assert_eq!(stack.peek(), Ok(Some(-42)));
    stack.apply_unary(|a| Ok(!a)).required()?;
    assert_eq!(stack.peek(), Ok(Some(!-42)));

    Ok(())
  }

  #[test]
  fn binop() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut buffer = [0; 32];
    let Ok(mut stack) = ValueStack::new(&mut buffer, crate::Validation::Pedantic);

    for value in 1..=5 {
      stack.push(value).required()?;
    }

    stack
      .apply_binary(|a, b| Ok(i32::wrapping_add(a, b)))
      .required()?;
    assert_eq!(stack.peek(), Ok(Some(9)));
    stack
      .apply_binary(|a, b| Ok(i32::wrapping_mul(a, b)))
      .required()?;
    assert_eq!(stack.peek(), Ok(Some(27)));
    stack
      .apply_binary(|a, b| Ok(i32::wrapping_sub(a, b)))
      .required()?;
    assert_eq!(stack.peek(), Ok(Some(-25)));
    stack
      .apply_binary(|a, b| a.checked_div(b).ok_or(HintErrorKind::DivideByZero))
      .required()?;
    assert_eq!(stack.peek(), Ok(Some(0)));

    Ok(())
  }

  #[test]
  fn move_index_avoid_overflow() {
    let mut buffer = [0];
    let Ok(mut stack) = filled_stack(&mut buffer);

    assert_eq!(stack.move_index(), Err(HintErrorKind::ValueStackUnderflow));
  }

  #[test]
  fn pop_count_checked() {
    let mut buffer = [-1, 128, -1, 128];
    let Ok(mut stack) = filled_stack(&mut buffer);
    let Ok(()) = stack.set_validation(crate::Validation::Pedantic);
    assert_eq!(stack.pop_count_checked(), Ok(128));

    assert!(matches!(
      stack.pop_count_checked(),
      Err(HintErrorKind::InvalidStackValue(-1))
    ));
    let Ok(()) = stack.set_validation(crate::Validation::Compatible);
    assert_eq!(stack.pop_count_checked(), Ok(128));

    assert_eq!(stack.pop_count_checked(), Ok(0));
  }
}
