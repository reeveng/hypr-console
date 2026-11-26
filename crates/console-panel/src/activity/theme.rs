//! Use the Console palette installed beside the rest of the desktop.

use std::collections::BTreeMap;
use std::{fs, io};

use console_core_color::Oklch;
use console_core_color::palette::{self, PaletteError, Wearing, WearingError};

use super::Result;

const SYSTEM: &str = "/usr/local/lib/console/palette.sh";
const BUNDLED: &str = include_str!("../../../../files/usr/local/lib/console/palette.sh");

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
  pub chrome: Wearing,
  pub ground: Oklch,
  pub surface: Oklch,
  pub lifted: Oklch,
  pub rule: Oklch,
  pub ink: Oklch,
  pub quiet: Oklch,
  pub accent: Oklch,
  pub bars: Oklch,
  pub lanes: [Oklch; 5],
}

impl Palette {
  fn from_spent(spent: &BTreeMap<String, String>) -> std::result::Result<Self, PaletteError> {
    let wearing = Wearing::out_of(spent)?;
    let lanes = [
      ("mauve", wearing.pink),
      ("sky", wearing.soft),
      ("coral", wearing.coral),
      ("mint", wearing.edge),
      ("peach", wearing.text),
    ]
    .map(
      |(name, alternative)| match palette::find_color(spent, name) {
        Ok(color) => color,
        Err(_missing_or_invalid_lane_color) => alternative,
      },
    );
    Ok(Self {
      chrome: wearing,
      ground: wearing.panel,
      surface: wearing.ground,
      lifted: wearing.fill,
      rule: wearing.edge,
      ink: wearing.text,
      quiet: wearing.soft,
      accent: wearing.pink,
      bars: wearing.fill,
      lanes,
    })
  }

  pub fn current() -> Result<Option<Self>> {
    let spent = match palette::spent() {
      Ok(spent) => spent,
      Err(fault) => {
        let missing = match &fault {
          WearingError::Unread { why, .. } => why.kind() == io::ErrorKind::NotFound,
          WearingError::Lost(_) | WearingError::Palette(_) | WearingError::Theme(_) => false,
        };

        match bool::then_some(missing, ()) {
          Some(()) => {}
          None => return Err(Box::new(fault)),
        }

        let text = match fs::read_to_string(SYSTEM) {
          Ok(text) => text,
          Err(fault_2) => match ::core::primitive::bool::then_some(
            fault_2.kind() == io::ErrorKind::NotFound,
            (),
          ) {
            ::core::option::Option::Some(()) => return Ok(None),
            ::core::option::Option::None => return Err(Box::new(fault_2)),
          },
        };
        let Ok(spent) = palette::read(&text);
        spent
      }
    };
    let palette = Self::from_spent(&spent)?;

    Ok(Some(palette))
  }

  pub fn bundled() -> Result<Self> {
    let Ok(spent) = palette::read(BUNDLED);
    let palette = Self::from_spent(&spent)?;

    Ok(palette)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn uses_configured_console_colors_for_charts_and_surfaces() -> Result<()> {
    let Ok(mut spent) = palette::read(BUNDLED);

    for (key, rgb) in [
      ("panel", "112233"),
      ("pink", "A98765"),
      ("fill", "556677"),
      ("sky", "887766"),
    ] {
      spent.insert(String::from(key), String::from(rgb));
    }

    let palette = Palette::from_spent(&spent)?;
    let find_color = palette::find_color(&spent, "panel")?;
    assert_eq!(palette.ground, find_color);
    let pink = palette::find_color(&spent, "pink")?;
    let fill = palette::find_color(&spent, "fill")?;
    let sky = palette::find_color(&spent, "sky")?;
    let [_mauve, lane_sky, _coral, _mint, _peach] = palette.lanes;
    assert_eq!(palette.accent, pink);
    assert_eq!(palette.bars, fill);
    assert_eq!(lane_sky, sky);
    Ok(())
  }
}
