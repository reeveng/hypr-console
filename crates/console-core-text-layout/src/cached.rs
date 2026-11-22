//! A cache keeps its storage when its contents become stale.
//!
//! An unused cache starts with an empty value of its own type. Taking that
//! storage leaves another empty value behind, so access after rebuilding does
//! not depend on an impossible empty variant being rejected with a panic.

use console_core_never::Never;
use core::fmt::Debug;
use core::mem;

#[derive(Clone, Debug)]
pub enum Cached<T: Clone + Debug> {
  Unused(T),
  Used(T),
}

impl<T: Clone + Debug + Default> Default for Cached<T> {
  fn default() -> Self {
    Self::Unused(T::default())
  }
}

impl<T: Clone + Debug + Default> Cached<T> {
  pub const fn get(&self) -> Result<Option<&T>, Never> {
    Ok(match self {
      Self::Unused(_) => None,
      Self::Used(value) => Some(value),
    })
  }

  pub fn get_mut(&mut self) -> Result<Option<&mut T>, Never> {
    Ok(match self {
      Self::Unused(_) => None,
      Self::Used(value) => Some(value),
    })
  }

  pub(crate) const fn value(&self) -> Result<&T, Never> {
    Ok(match self {
      Self::Unused(value) | Self::Used(value) => value,
    })
  }

  pub fn take_unused(&mut self) -> Result<Option<T>, Never> {
    Ok(match mem::take(self) {
      Self::Unused(value) => Some(value),
      Self::Used(value) => {
        *self = Self::Used(value);

        None
      }
    })
  }

  pub fn take_used(&mut self) -> Result<Option<T>, Never> {
    Ok(match mem::take(self) {
      Self::Used(value) => Some(value),
      Self::Unused(value) => {
        *self = Self::Unused(value);

        None
      }
    })
  }

  pub fn set_unused(&mut self) -> Result<(), Never> {
    *self = match mem::take(self) {
      Self::Unused(value) | Self::Used(value) => Self::Unused(value),
    };

    Ok(())
  }

  pub fn set_used(&mut self, value: T) -> Result<(), Never> {
    *self = Self::Used(value);

    Ok(())
  }
}
