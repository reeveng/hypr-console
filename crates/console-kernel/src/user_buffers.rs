//! Validation and fixed-size copy-in for the prototype's two-page user window.
//! The window borrows consecutive leaf entries below a user-writable path.
//! Copy-in stages bytes through the caller's fault-aware loader and leaves
//! the destination untouched until the whole read succeeds.

use crate::frame_mappings::{Access, FrameAccess, user_access};

const PAGE: u64 = 4096;
const USER_END: u64 = 1 << 47;

#[derive(Debug, PartialEq, Eq)]
pub enum BufferError {
  Range,
  Permission,
  Length,
  MemoryAccess,
}

pub struct UserRange {
  pub start: u64,
  pub length: u64,
}

pub struct UserWindow<'table> {
  base: u64,
  entries: &'table [u64; 2],
}

pub struct UserBuffer<'table> {
  start: u64,
  end: u64,
  _window: &'table UserWindow<'table>,
}

impl<'table> UserWindow<'table> {
  pub fn new(base: u64, entries: &'table [u64; 2]) -> Result<Self, BufferError> {
    match ::core::primitive::bool::then_some(
      base == 0
        || base & PAGE.saturating_sub(1) != 0
        || base > USER_END.saturating_sub(PAGE.saturating_mul(2)),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Err(BufferError::Range);
      }
      ::core::option::Option::None => {}
    }

    Ok(Self { base, entries })
  }

  pub fn authorize(
    &self,
    range: UserRange,
    access: FrameAccess,
  ) -> Result<UserBuffer<'_>, BufferError> {
    let UserRange { start, length } = range;
    let end = start.checked_add(length).ok_or(BufferError::Range)?;
    let middle = self.base.saturating_add(PAGE);

    match ::core::primitive::bool::then_some(
      length == 0 || start < self.base || end > middle.saturating_add(PAGE),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Err(BufferError::Range);
      }
      ::core::option::Option::None => {}
    }

    let [first, second] = *self.entries;
    let Ok(low) = user_access(first, access);
    let Ok(high) = user_access(second, access);

    match ::core::primitive::bool::then_some(
      (start < middle && low == Access::Denied) || (end > middle && high == Access::Denied),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Err(BufferError::Permission);
      }
      ::core::option::Option::None => {}
    }

    Ok(UserBuffer {
      start,
      end,
      _window: self,
    })
  }
}

impl UserBuffer<'_> {
  pub fn copy_in<const N: usize>(
    &self,
    mut load: impl FnMut(u64) -> Result<u8, BufferError>,
    destination: &mut [u8; N],
  ) -> Result<(), BufferError> {
    match ::core::primitive::bool::then_some(
      u64::try_from(N) != Ok(self.end.saturating_sub(self.start)),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Err(BufferError::Length);
      }
      ::core::option::Option::None => {}
    }

    let mut staged = [0; N];

    for (byte, address) in staged.iter_mut().zip(self.start..self.end) {
      let checked_value = load(address)?;
      *byte = checked_value;
    }

    *destination = staged;
    Ok(())
  }
}

#[cfg(kani)]
#[path = "../proofs/user_buffers.rs"]
mod proofs;
