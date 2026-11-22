#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct IntRect {
  pub(crate) x0: u32,
  pub(crate) y0: u32,
  pub(crate) x1: u32,
  pub(crate) y1: u32,
}

impl IntRect {
  pub(crate) fn width(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(u32::wrapping_sub(self.x1, self.x0))
  }

  pub(crate) fn height(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(u32::wrapping_sub(self.y1, self.y0))
  }

  pub(crate) fn intersect(
    &self,
    other: Self,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let disjoint =
      self.x1 < other.x0 || other.x1 < self.x0 || self.y1 < other.y0 || other.y1 < self.y0;

    Ok(match disjoint.then_some(()) {
      Some(()) => Self {
        x0: 0,
        y0: 0,
        x1: 0,
        y1: 0,
      },
      None => Self {
        x0: self.x0.max(other.x0),
        y0: self.y0.max(other.y0),
        x1: self.x1.min(other.x1),
        y1: self.y1.min(other.y1),
      },
    })
  }
}
