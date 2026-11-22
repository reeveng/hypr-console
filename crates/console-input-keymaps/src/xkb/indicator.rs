use console_core_never::Never;
use console_input_keymaps_derive::CloneWithDelta;

#[derive(Copy, Clone, Debug, CloneWithDelta, Eq, PartialEq)]
pub(crate) struct IndicatorIdx(u32);

impl IndicatorIdx {
  pub(crate) const ONE: Self = Self(1);

  pub(crate) const fn new(idx: u32) -> Result<Option<Self>, Never> {
    Ok(match idx {
      1..=u32::BITS => Some(Self(idx)),
      _ => None,
    })
  }

  pub(crate) const fn raw(self) -> Result<u32, Never> {
    Ok(self.0)
  }
}
