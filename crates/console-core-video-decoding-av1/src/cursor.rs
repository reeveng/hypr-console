#![forbid(unsafe_code)]

use console_core_number_conversion::{fixed_index, index};

use crate::src::error::{Rav1dError, Rav1dResult};

pub(crate) struct CursorMut<'a, T> {
  storage: &'a mut [T],
  position: u64,
}

impl<'a, T> CursorMut<'a, T> {
  pub(crate) fn new(storage: &'a mut [T], position: u64) -> Rav1dResult<Self> {
    let Ok(length) = fixed_index(storage.len());

    match (position <= length).then_some(()) {
      Some(()) => Ok(Self { storage, position }),
      None => Err(Rav1dError::EINVAL),
    }
  }

  fn relative_position(&self, displacement: i64) -> Rav1dResult<u64> {
    self.position.checked_add_signed(displacement).ok_or(Rav1dError::ERANGE)
  }

  pub(crate) fn reborrow(&mut self, displacement: i64) -> Rav1dResult<CursorMut<'_, T>> {
    let position = self.relative_position(displacement)?;
    CursorMut::new(self.storage, position)
  }

  pub(crate) fn advance(&mut self, amount: u64) -> Rav1dResult {
    let position = self.position.checked_add(amount).ok_or(Rav1dError::ERANGE)?;
    let Ok(length) = fixed_index(self.storage.len());

    match (position <= length).then_some(()) {
      Some(()) => {
        self.position = position;
        Ok(())
      }
      None => Err(Rav1dError::EINVAL),
    }
  }

  fn element_position(&self, displacement: i64) -> Rav1dResult<u64> {
    let position = self.relative_position(displacement)?;
    let Ok(length) = fixed_index(self.storage.len());

    match (position < length).then_some(()) {
      Some(()) => Ok(position),
      None => Err(Rav1dError::EINVAL),
    }
  }

  pub(crate) fn get(&self, displacement: i64) -> Rav1dResult<T>
  where
    T: Copy,
  {
    let position = self.element_position(displacement)?;
    let Ok(native_position) = index(position);
    self.storage.get(native_position).copied().ok_or(Rav1dError::EINVAL)
  }

  pub(crate) fn set(&mut self, displacement: i64, value: T) -> Rav1dResult {
    let position = self.element_position(displacement)?;
    let Ok(native_position) = index(position);
    let element = self.storage.get_mut(native_position).ok_or(Rav1dError::EINVAL)?;
    *element = value;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reborrow_moves_independently_and_preserves_checked_storage() -> Rav1dResult {
    let mut samples = [0, 1, 2, 3];
    let mut cursor = CursorMut::new(&mut samples, 2)?;
    let predecessor = cursor.get(-1)?;
    assert_eq!(predecessor, 1);
    assert!(matches!(cursor.get(-3), Err(Rav1dError::ERANGE)));
    assert!(matches!(cursor.set(2, 8), Err(Rav1dError::EINVAL)));

    let mut earlier = cursor.reborrow(-1)?;
    earlier.set(-1, 7)?;
    earlier.advance(1)?;
    let shared_current = earlier.get(0)?;
    let updated = cursor.get(-2)?;
    let original_current = cursor.get(0)?;
    assert_eq!(shared_current, 2);
    assert_eq!(updated, 7);
    assert_eq!(original_current, 2);

    cursor.advance(2)?;
    assert!(matches!(cursor.advance(1), Err(Rav1dError::EINVAL)));
    assert!(matches!(cursor.get(0), Err(Rav1dError::EINVAL)));
    let final_sample = cursor.get(-1)?;
    assert_eq!(final_sample, 3);
    Ok(())
  }
}
