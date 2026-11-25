//! Window border colors from Console's selected palette.

use console_core_color::palette::{self, PaletteError, WearingError};
use console_core_never::Never;

use crate::{DispatchResult, Request};

pub struct Colors {
  active: String,
  inactive: String,
}

impl Colors {
  pub fn current() -> Result<Self, WearingError> {
    let colors = palette::spent()?;
    let color = |role: &'static str| colors.get(role).cloned().ok_or(PaletteError::Absent(role));
    let active = color("pink")?;
    let inactive = color("edge")?;

    Ok(Self { active, inactive })
  }

  #[must_use]
  pub fn hex_pair(&self) -> Result<String, Never> {
    let Self { active, inactive } = self;

    Ok(format!("{active} {inactive}"))
  }

  pub fn apply(&self) -> Result<DispatchResult, Never> {
    let Self { active, inactive } = self;

    crate::request(
      Request::Script,
      &format!(
        r#"hl.config({{ general = {{ ["col.active_border"] = "rgba({active}ff)", ["col.inactive_border"] = "rgba({inactive}ff)" }} }})"#
      ),
    )
  }
}
