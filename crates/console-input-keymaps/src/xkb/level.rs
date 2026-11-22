use console_core_never::Never;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub(crate) struct Level(u32);

impl Level {
  pub(crate) const ONE: Self = Self(1);

  pub(crate) const fn new(level: u32) -> Result<Option<Level>, Never> {
    Ok(match level {
      1.. => Some(Self(level)),
      _ => None,
    })
  }

  pub(crate) const fn raw(self) -> Result<u32, Never> {
    Ok(self.0)
  }
}
