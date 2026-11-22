use console_core_never::Never;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub(crate) struct RadioGroup(u32);

impl RadioGroup {
  pub(crate) const fn new(radio_group: u32) -> Result<Option<RadioGroup>, Never> {
    Ok(match radio_group {
      1..=u32::BITS => Some(Self(radio_group)),
      _ => None,
    })
  }

  pub(crate) const fn raw(self) -> Result<u32, Never> {
    Ok(self.0)
  }

  pub(crate) const fn to_mask(self) -> Result<u32, Never> {
    Ok(1 << (self.0 - 1))
  }
}
