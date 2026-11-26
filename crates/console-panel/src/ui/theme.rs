//! Palette roles for Console chrome and custom content.

use console_core_color::Oklch;
use console_core_color::palette::{self, PaletteError, Wearing};
use console_core_never::Never;

const BUNDLED: &str = include_str!("../../../../files/usr/local/lib/console/palette.sh");

pub fn current() -> Result<Wearing, PaletteError> {
  match Wearing::worn() {
    Ok(colors) => Ok(colors),
    Err(fault) => {
      console_core_logging::debug!("Using the bundled Console palette: {fault}");

      bundled()
    }
  }
}

pub(crate) fn bundled() -> Result<Wearing, PaletteError> {
  let Ok(colors) = palette::read(BUNDLED);
  Wearing::out_of(&colors)
}

pub enum Ink {
  Text,
  Secondary,
  Disabled,
  Accent,
  Error,
  OnAccent,
}

impl Ink {
  pub fn color(self, theme: &Wearing) -> Result<Oklch, Never> {
    Ok(match self {
      Self::Text => theme.text,
      Self::Secondary => theme.soft,
      Self::Disabled => theme.edge,
      Self::Accent => theme.pink,
      Self::Error => theme.coral,
      Self::OnAccent => theme.night,
    })
  }
}

pub enum Surface {
  Window,
  Content,
  Recessed,
  Tinted,
}

impl Surface {
  pub fn color(self, theme: &Wearing) -> Result<Oklch, Never> {
    Ok(match self {
      Self::Window => theme.panel,
      Self::Content => theme.ground,
      Self::Recessed => theme.night,
      Self::Tinted => theme.fill,
    })
  }
}
