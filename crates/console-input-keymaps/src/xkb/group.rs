use console_core_never::Never;
use console_input_keymaps_derive::CloneWithDelta;

#[derive(Copy, Clone, Debug, CloneWithDelta, PartialEq)]
pub(crate) struct GroupIdx(u32);

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(crate) struct GroupMask(pub(crate) u32);

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum GroupChange {
  Absolute(GroupIdx),
  Rel(i32),
}

impl Default for GroupChange {
  fn default() -> Self {
    Self::Rel(0)
  }
}

impl GroupIdx {
  pub(crate) const ONE: GroupIdx = GroupIdx(1);

  pub(crate) const fn new(group: u32) -> Result<Option<Self>, Never> {
    Ok(match group {
      1..=u32::BITS => Some(Self(group)),
      _ => None,
    })
  }

  pub(crate) const fn raw(self) -> Result<u32, Never> {
    Ok(self.0)
  }
}
