//! Tracking function call state.

use console_core_never::Never;

use super::{definition::Definition, error::HintErrorKind, program::Program};

#[cfg(test)]
const MAX_DEPTH: u64 = 32;

#[derive(Copy, Clone, Default)]
pub struct CallRecord {
  pub caller_program: Program,
  pub return_pc: u64,
  pub current_count: u32,
  pub definition: Definition,
}

#[derive(Default)]
pub struct CallStack {
  records: [CallRecord; 32],
  len: u64,
}

impl CallStack {
  pub fn clear(&mut self) -> Result<(), Never> {
    self.len = 0;
    Ok(())
  }

  pub fn push(&mut self, record: CallRecord) -> Result<(), HintErrorKind> {
    let top = self
      .records
      .get_mut({
        let Ok(value) = console_core_number_conversion::index(self.len);
        value
      })
      .ok_or(HintErrorKind::CallStackOverflow)?;
    *top = record;
    self.len = self.len.saturating_add(1);
    Ok(())
  }

  pub fn pop(&mut self) -> Result<CallRecord, HintErrorKind> {
    let top = self
      .len
      .checked_sub(1)
      .ok_or(HintErrorKind::CallStackUnderflow)?;
    let record = self
      .records
      .get({
        let Ok(value) = console_core_number_conversion::index(top);
        value
      })
      .copied()
      .ok_or(HintErrorKind::CallStackUnderflow)?;
    self.len = top;
    Ok(record)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn stack_overflow() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut stack = CallStack::default();

    for i in 0..MAX_DEPTH {
      let Ok(record) = record_with_key(i);
      stack.push(record).required()?;
    }

    assert!(matches!(
      stack.push(CallRecord::default()),
      Err(HintErrorKind::CallStackOverflow)
    ));

    Ok(())
  }

  #[test]
  fn stack_underflow() {
    assert!(matches!(
      CallStack::default().pop(),
      Err(HintErrorKind::CallStackUnderflow)
    ));
  }

  #[test]
  fn stack_push_pop() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;

    let mut stack = CallStack::default();

    for i in 0..MAX_DEPTH {
      let Ok(record) = record_with_key(i);
      stack.push(record).required()?;
    }

    for i in (0..MAX_DEPTH).rev() {
      {
        let queried_value = stack.pop().required()?;
        let Ok(key) = queried_value.definition.key();
        let Ok(expected_key) = console_core_number_conversion::fitted::<_, i32>(i);
        assert_eq!(key, expected_key);
      }
    }

    Ok(())
  }

  fn record_with_key(key: u64) -> Result<CallRecord, Never> {
    let Ok(key_2) = console_core_number_conversion::fitted::<_, i32>(key);
    let Ok(definition) = Definition::new(Program::Font, 0..0, key_2);
    Ok(CallRecord {
      caller_program: Program::Glyph,
      return_pc: 0,
      current_count: 1,
      definition,
    })
  }
}
