use crate::modifier::hidden::ModifierIndex;
use crate::modifier::hidden::ModifierMask;
use console_core_never::Never;
use std::fmt::Debug;
use std::fmt::Formatter;

pub(crate) mod hidden {
  use console_input_keymaps_derive::CloneWithDelta;

  #[derive(Copy, Clone, Eq, PartialEq, Hash, Default, CloneWithDelta)]
  pub struct ModifierMask(pub u32);

  #[derive(Copy, Clone, Eq, PartialEq, Hash, CloneWithDelta, Ord, PartialOrd)]
  pub(crate) struct ModifierIndex(pub(super) u32);
}

impl ModifierIndex {
  pub(crate) const SHIFT: Self = Self(0);
  pub(crate) const LOCK: Self = Self(1);
  pub(crate) const CONTROL: Self = Self(2);

  pub(crate) const fn new(index: u32) -> Result<Option<Self>, Never> {
    Ok(match index {
      u32::BITS.. => None,
      _ => Some(Self(index)),
    })
  }

  pub(crate) const fn raw(self) -> Result<u32, Never> {
    Ok(self.0)
  }

  pub(crate) const fn to_mask(self) -> Result<ModifierMask, Never> {
    Ok(ModifierMask(1 << self.0))
  }
}

impl ModifierMask {
  pub(crate) const NONE: Self = Self(0);
  pub(crate) const SHIFT: Self = match ModifierIndex::SHIFT.to_mask() {
    Ok(v) => v,
  };
  pub(crate) const LOCK: Self = match ModifierIndex::LOCK.to_mask() {
    Ok(v) => v,
  };

  pub(crate) const fn contains(self, other: ModifierMask) -> Result<Presence, Never> {
    Ok(match other.0 & !self.0 {
      0 => Presence::Present,
      _ => Presence::Absent,
    })
  }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum Presence {
  Present,
  Absent,
}

bitmask!(ModifierMask);

impl Debug for ModifierIndex {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(f)
  }
}
