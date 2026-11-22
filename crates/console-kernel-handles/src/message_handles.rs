//! Received handle values are stored inline up to the channel protocol limit.

use core::{array, fmt, iter, ops::Deref, slice};

use console_core_number_conversion::index;

use super::{HandleError, HandleValue, MESSAGE_HANDLES_LIMIT};

#[cfg_attr(
  dylint_lib = "explicit011_no_as_cast",
  expect(
    explicit011_no_as_cast,
    reason = "the protocol's 64-handle limit fits in a usize on every target"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "array lengths and the standard array iterator's const parameter are native positions"
  )
)]
const CAPACITY: usize = MESSAGE_HANDLES_LIMIT as usize;

#[derive(Clone)]
pub struct MessageHandles {
  values: [HandleValue; CAPACITY],
  len: u64,
}

impl MessageHandles {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this preserves the public const constructor and Default's infallible inline representation"
    )
  )]
  pub const fn new() -> Self {
    Self {
      values: [HandleValue(0); CAPACITY],
      len: 0,
    }
  }

  pub fn try_push(&mut self, handle: HandleValue) -> Result<(), HandleError> {
    let Ok(position) = index(self.len);
    let slot = self
      .values
      .get_mut(position)
      .ok_or(HandleError::MessageTooLarge)?;

    *slot = handle;
    self.len = self.len.saturating_add(1);

    Ok(())
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this preserves the public slice interface and supplies the infallible Deref view"
    )
  )]
  pub fn as_slice(&self) -> &[HandleValue] {
    let Ok(length) = index(self.len);

    match self.values.get(..length) {
      Some(held) => held,
      None => &[],
    }
  }
}

impl Default for MessageHandles {
  fn default() -> Self {
    Self::new()
  }
}

impl fmt::Debug for MessageHandles {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.as_slice().fmt(formatter)
  }
}

impl PartialEq for MessageHandles {
  fn eq(&self, other: &Self) -> bool {
    self.as_slice() == other.as_slice()
  }
}

impl Eq for MessageHandles {}

impl Deref for MessageHandles {
  type Target = [HandleValue];

  fn deref(&self) -> &Self::Target {
    self.as_slice()
  }
}

impl<'a> IntoIterator for &'a MessageHandles {
  type Item = &'a HandleValue;
  type IntoIter = slice::Iter<'a, HandleValue>;

  fn into_iter(self) -> Self::IntoIter {
    self.as_slice().iter()
  }
}

impl IntoIterator for MessageHandles {
  type Item = HandleValue;
  type IntoIter = iter::Take<array::IntoIter<HandleValue, CAPACITY>>;

  fn into_iter(self) -> Self::IntoIter {
    let Ok(length) = index(self.len);

    self.values.into_iter().take(length)
  }
}
