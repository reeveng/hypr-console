//! Hinting targets and the independent rendering flags carried by smooth modes.

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Target {
  Mono,

  Smooth {
    mode: SmoothMode,

    symmetric_rendering: SymmetricRendering,

    linear_metrics: LinearMetrics,
  },
}

impl Default for Target {
  fn default() -> Self {
    Target::from(SmoothMode::Normal)
  }
}

#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum SmoothMode {
  #[default]
  Normal,

  Light,

  Lcd,

  VerticalLcd,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SymmetricRendering {
  Enabled,
  Disabled,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LinearMetrics {
  Preserve,
  Hint,
}

impl From<SmoothMode> for Target {
  fn from(value: SmoothMode) -> Self {
    Self::Smooth {
      mode: value,
      symmetric_rendering: SymmetricRendering::Enabled,
      linear_metrics: LinearMetrics::Hint,
    }
  }
}
