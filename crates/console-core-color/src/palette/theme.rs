use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

use console_core_atomic_writes::{self as writes, Stored};
use console_core_never::Never;
use console_core_places::Base;

use crate::{
  Floor, Hue, Oklch, ParseError, Rgba, Short, darkest_clearing, hexcode, lightest_clearing,
  to_oklch,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
  #[default]
  Blossom,
  Ocean,
  Forest,
  Slate,
  Custom(Rgba),
}

impl Theme {
  pub const EVERY: [Self; 4] = [Self::Blossom, Self::Ocean, Self::Forest, Self::Slate];

  pub const fn name(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Blossom => "Blossom",
      Self::Ocean => "Ocean",
      Self::Forest => "Forest",
      Self::Slate => "Slate",
      Self::Custom(_) => "Custom",
    })
  }

  pub fn custom(entered: &str) -> Result<Self, ParseError> {
    Rgba::of(entered.trim()).map(Self::Custom)
  }

  pub fn custom_color(self) -> Result<Option<String>, Never> {
    Ok(match self {
      Self::Custom(color) => {
        let Ok(code) = digits(color);
        Some(format!("#{code}"))
      }
      Self::Blossom | Self::Ocean | Self::Forest | Self::Slate => None,
    })
  }

  fn read(choice: &str) -> Result<Self, Never> {
    match choice.as_bytes() {
      [b'#', ..] => {
        return Ok(match Self::custom(choice) {
          Ok(custom) => custom,
          Err(_invalid_color) => Self::Blossom,
        });
      }
      _ => {}
    }

    let named = Self::EVERY.into_iter().find(|theme| {
      let Ok(name) = theme.name();
      name == choice
    });

    Ok(match named {
      Some(theme) => theme,
      None => Self::Blossom,
    })
  }

  pub(super) fn apply(self, colors: &mut BTreeMap<String, String>) -> Result<(), Short> {
    let (neutral, accent, chroma_scale) = match self {
      Self::Blossom => return Ok(()),
      Self::Ocean => (246.0, 226.0, 1.0),
      Self::Forest => (158.0, 143.0, 1.0),
      Self::Slate => (255.0, 255.0, 0.3),
      Self::Custom(color) => {
        let Ok(color_2) = Oklch::of(color);
        let scale = (color_2.chroma / 0.1).clamp(0.0, 1.0);

        (color_2.hue.0, color_2.hue.0, scale)
      }
    };

    let hue_groups: [(&[&str], f64); 2] = [
      (
        &["night", "ground", "panel", "edge", "text", "soft"],
        neutral,
      ),
      (&["fill", "pink", "rose"], accent),
    ];

    for (roles, hue) in hue_groups {
      for role in roles {
        {
          match colors.get_mut(*role) {
            Some(code) => {
              let Ok(mut color) = to_oklch(code);
              color.hue = Hue(hue);
              color.chroma *= chroma_scale;
              let Ok(adjusted) = hexcode(color);
              *code = adjusted;
            }
            ::core::option::Option::None => {}
          };
        }
      }
    }

    fit_contrast(colors)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ColorRole {
  key: &'static str,
  label: &'static str,
}

impl ColorRole {
  pub const EVERY: [Self; 17] = [
    Self {
      key: "night",
      label: "Deep background",
    },
    Self {
      key: "ground",
      label: "Bar background",
    },
    Self {
      key: "panel",
      label: "Panel background",
    },
    Self {
      key: "text",
      label: "Text",
    },
    Self {
      key: "soft",
      label: "Secondary text",
    },
    Self {
      key: "edge",
      label: "Borders",
    },
    Self {
      key: "pink",
      label: "Accent",
    },
    Self {
      key: "rose",
      label: "Selection",
    },
    Self {
      key: "fill",
      label: "Progress fill",
    },
    Self {
      key: "mauve",
      label: "Sound",
    },
    Self {
      key: "lilac",
      label: "Bluetooth",
    },
    Self {
      key: "sky",
      label: "Links",
    },
    Self {
      key: "mint",
      label: "Network",
    },
    Self {
      key: "leaf",
      label: "Success",
    },
    Self {
      key: "butter",
      label: "Warnings",
    },
    Self {
      key: "peach",
      label: "Numbers",
    },
    Self {
      key: "coral",
      label: "Errors",
    },
  ];

  pub const fn key(self) -> Result<&'static str, Never> {
    Ok(self.key)
  }

  pub const fn label(self) -> Result<&'static str, Never> {
    Ok(self.label)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overrides {
  Active,
  None,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ThemeSettings {
  pub base: Theme,
  colors: BTreeMap<ColorRole, Rgba>,
}

impl ThemeSettings {
  pub fn color(&self, role: ColorRole) -> Result<Option<Rgba>, Never> {
    Ok(self.colors.get(&role).copied())
  }

  pub fn set_color(&mut self, role: ColorRole, color: Option<Rgba>) -> Result<(), Never> {
    match color {
      Some(color_2) => {
        let _ = self.colors.insert(role, color_2);
      }
      None => {
        let _ = self.colors.remove(&role);
      }
    }

    Ok(())
  }

  pub fn overrides(&self) -> Result<Overrides, Never> {
    Ok(match self.colors.first_key_value() {
      Some(_) => Overrides::Active,
      None => Overrides::None,
    })
  }

  pub fn reset_colors(&mut self) -> Result<(), Never> {
    self.colors.clear();

    Ok(())
  }

  fn path() -> Result<Option<PathBuf>, Never> {
    let Ok(at) = Base::Configuration.ours();

    Ok(at.map(|at_2| at_2.join("color-theme")))
  }

  pub fn current() -> Result<Self, Never> {
    let Ok(configured) = Self::path();

    let at = match configured {
      Some(matched_at) => matched_at,
      ::core::option::Option::None => return Ok(Self::default()),
    };

    let Ok(stored) = writes::read(&at);

    match stored {
      Stored::Text(written) => Self::read(&written),
      Stored::Absent => Ok(Self::default()),
      Stored::Failed(fault) => {
        console_core_logging::warn!("reading {}: {fault}", at.display());
        Ok(Self::default())
      }
    }
  }

  pub fn save(&self) -> io::Result<()> {
    let Ok(at) = Self::path();
    let at_2 =
      at.ok_or_else(|| io::Error::other("Console's configuration directory is unavailable"))?;

    let Ok(written) = self.serialize();
    writes::whole_with_folders(&at_2, written.as_bytes()).map_err(io::Error::other)
  }

  fn read(written: &str) -> Result<Self, Never> {
    let written_2 = written.trim_start();
    let (choice, overrides) = match written_2.split_once('\n') {
      Some(sections) => sections,
      None => (written_2, ""),
    };
    let Ok(overrides_2) = super::read(overrides);
    let colors = ColorRole::EVERY
      .into_iter()
      .filter_map(|role| {
        let code = overrides_2.get(role.key)?;

        match Rgba::of(code) {
          Ok(color) => Some((role, color)),
          Err(_invalid_color) => None,
        }
      })
      .collect();
    let Ok(base) = Theme::read(choice.trim());

    Ok(Self { base, colors })
  }

  fn serialize(&self) -> Result<String, Never> {
    let Ok(name) = self.base.name();
    let Ok(custom) = self.base.custom_color();
    let choice = match custom.as_deref() {
      Some(code) => code,
      None => name,
    };
    let mut written = format!("{choice}\n");

    for (role, color) in &self.colors {
      let Ok(code) = digits(*color);
      written.push_str(role.key);
      written.push('=');
      written.push_str(&code);
      written.push('\n');
    }

    Ok(written)
  }

  pub(super) fn apply(&self, colors: &mut BTreeMap<String, String>) -> Result<(), Short> {
    self.base.apply(colors)?;

    for (role, color) in &self.colors {
      let Ok(code) = digits(*color);
      let _ = colors.insert(role.key.to_string(), code);
    }

    Ok(())
  }
}

fn digits(color: Rgba) -> Result<String, Never> {
  let Rgba {
    red, green, blue, ..
  } = color;

  console_core_bytes::hex::encode(&[red, green, blue], console_core_bytes::hex::Case::Lower)
}

fn fit_contrast(colors: &mut BTreeMap<String, String>) -> Result<(), Short> {
  let ::core::result::Result::Ok(surfaces) = shades(colors, &["panel", "ground", "night"]);
  let ::core::result::Result::Ok(edge_surfaces) = shades(colors, &["panel", "ground"]);

  let contrast_groups: [(&[&str], &[String], Floor); 3] = [
    (
      &["edge"],
      &edge_surfaces,
      Floor {
        ratio: 3.0,
        lightness_contrast: 30.0,
      },
    ),
    (
      &["text"],
      &surfaces,
      Floor {
        ratio: 10.0,
        lightness_contrast: 90.0,
      },
    ),
    (
      &[
        "soft", "pink", "rose", "mauve", "lilac", "sky", "mint", "leaf", "butter", "peach",
        "coral",
      ],
      &surfaces,
      Floor {
        ratio: 7.0,
        lightness_contrast: 75.0,
      },
    ),
  ];

  for (roles, against, floor) in contrast_groups {
    for role in roles {
      {
        match colors.get_mut(*role) {
          Some(code) => {
            let Ok(color) = to_oklch(code);
            let lightness = lightest_clearing(color, against, floor)?;
            let Ok(adjusted) = color.at(lightness).and_then(hexcode);
            *code = adjusted;
          }
          ::core::option::Option::None => {}
        };
      }
    }
  }

  let ::core::result::Result::Ok(text) = shades(colors, &["text"]);

  match colors.get_mut("fill") {
    Some(code) => {
      let Ok(color) = to_oklch(code);
      let lightness = darkest_clearing(
        color,
        &text,
        Floor {
          ratio: 7.0,
          lightness_contrast: 0.0,
        },
      )?;
      let Ok(adjusted) = color.at(lightness).and_then(hexcode);
      *code = adjusted;
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

#[must_use]
fn shades(
  colors: &BTreeMap<String, String>,
  roles: &[&str],
) -> ::core::result::Result<Vec<String>, ::console_core_never::Never> {
  ::core::result::Result::Ok(
    roles
      .iter()
      .filter_map(|role| colors.get(*role).cloned())
      .collect(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::palette::{Wearing, read};
  use crate::{Ansi, Clears, Ground};

  const SHIPPED: &str = include_str!("../../../../files/usr/local/lib/console/palette.sh");

  #[test]
  fn themes_keep_the_console_readable() -> Result<(), Box<dyn std::error::Error>> {
    let Ok(default) = read(SHIPPED);
    let red = Theme::custom("#ff0000")?;
    let green = Theme::custom("#00ff00")?;
    let blue = Theme::custom("#0000ff")?;
    let black = Theme::custom("#000000")?;

    for theme in Theme::EVERY.into_iter().chain([red, green, blue, black]) {
      let mut colors = default.clone();
      theme.apply(&mut colors)?;
      Wearing::out_of(&colors)?;

      match theme {
        Theme::Blossom => {}
        Theme::Ocean | Theme::Forest | Theme::Slate | Theme::Custom(_) => {
          assert_ne!(colors.get("pink"), default.get("pink"));
        }
      }

      for (ink, ground, ratio, lightness_contrast) in [
        ("text", "panel", 7.0, 90.0),
        ("text", "fill", 7.0, 75.0),
        ("soft", "panel", 7.0, 75.0),
        ("pink", "panel", 7.0, 75.0),
        ("night", "pink", 7.0, 75.0),
        ("edge", "panel", 3.0, 30.0),
      ] {
        let floor = Floor {
          ratio,
          lightness_contrast,
        };
        let ink_code = colors.get(ink).ok_or("missing ink")?;
        let ground_code = colors.get(ground).ok_or("missing background")?;
        let Ok(clears) = floor.cleared_by(Ansi(ink_code), Ground(ground_code));
        assert_eq!(clears, Clears::Yes, "{theme:?}, {ink} on {ground}");
      }
    }

    assert_eq!(Theme::custom("#xyz"), Err(ParseError::Length(3)));
    let mixed_case = Theme::custom(" 6aABcF ")?;
    let Ok(code) = mixed_case.custom_color();
    assert_eq!(code.as_deref(), Some("#6aabcf"));

    Ok(())
  }

  #[test]
  fn every_color_in_the_native_palette_can_be_edited() {
    let Ok(default) = read(SHIPPED);
    let mut roles = ColorRole::EVERY.map(|role| role.key);
    roles.sort_unstable();
    assert!(roles.into_iter().eq(default.keys().map(String::as_str)));
  }

  #[test]
  fn individual_colors_are_exact_and_survive_a_base_change()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(shipped) = read(SHIPPED);
    let Ok(mut settings) = ThemeSettings::read("Ocean\ntext=112233\npink=F50849\n");
    settings.base = Theme::Forest;
    let mut forest = shipped.clone();
    settings.base.apply(&mut forest)?;
    let mut customized = shipped.clone();
    settings.apply(&mut customized)?;

    assert_eq!(customized.get("text").map(String::as_str), Some("112233"));
    assert_eq!(customized.get("pink").map(String::as_str), Some("f50849"));
    assert_eq!(customized.get("panel"), forest.get("panel"));
    let Ok(serialized) = settings.serialize();
    assert_eq!(ThemeSettings::read(&serialized), Ok(settings.clone()));

    let text = ColorRole::EVERY
      .into_iter()
      .find(|role| role.key == "text")
      .ok_or("missing text role")?;
    let Ok(()) = settings.set_color(text, None);
    let mut partial = shipped.clone();
    settings.apply(&mut partial)?;
    assert_eq!(partial.get("text"), forest.get("text"));
    assert_eq!(partial.get("pink").map(String::as_str), Some("f50849"));

    let Ok(()) = settings.reset_colors();
    let mut reset = shipped;
    settings.apply(&mut reset)?;
    assert_eq!(reset, forest);

    Ok(())
  }

  #[test]
  fn preset_and_custom_hue_files_still_load() -> Result<(), ParseError> {
    let Ok(preset) = ThemeSettings::read("\n Slate\n");
    assert_eq!(preset.base, Theme::Slate);
    assert_eq!(preset.overrides(), Ok(Overrides::None));
    let Ok(custom) = ThemeSettings::read("#6820cc\n");
    let written_custom = Theme::custom("#6820cc")?;
    assert_eq!(custom.base, written_custom);
    assert_eq!(custom.overrides(), Ok(Overrides::None));
    let Ok(malformed) = ThemeSettings::read("Forest\ntext=bogus!\nunknown=333333\n");
    assert_eq!(malformed.base, Theme::Forest);
    assert_eq!(malformed.overrides(), Ok(Overrides::None));

    Ok(())
  }
}
