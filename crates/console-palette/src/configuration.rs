//! What `theme/palette.toml` says, and nothing about what is done with it.

use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;

use console_core_atomic_writes::MAX_TEXT_BYTES;
use console_core_color::Hue;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use indexmap::IndexMap;
use serde::de::{Error, IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Default)]
pub struct PaletteText(String);

impl<'de> Deserialize<'de> for PaletteText {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let text = String::deserialize(deserializer)?;
    let Ok(length) = fitted::<_, u64>(text.len());

    match (length <= u64::from(MAX_TEXT_BYTES)).then_some(()) {
      Some(()) => Ok(Self(text)),
      None => Err(D::Error::custom("palette text exceeds the file size limit")),
    }
  }
}

impl Deref for PaletteText {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl fmt::Display for PaletteText {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(self)
  }
}

#[derive(Debug)]
pub struct PaletteItems<T>(Vec<T>);

impl<T> Default for PaletteItems<T> {
  fn default() -> Self {
    Self(Vec::new())
  }
}

impl<T> Deref for PaletteItems<T> {
  type Target = [T];

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for PaletteItems<T> {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    deserializer.deserialize_seq(PaletteItemsVisitor(PhantomData))
  }
}

struct PaletteItemsVisitor<T>(PhantomData<fn() -> T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for PaletteItemsVisitor<T> {
  type Value = PaletteItems<T>;

  fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str("a palette array within the file size limit")
  }

  fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
    let mut items = Vec::new();

    for _ in 0..MAX_TEXT_BYTES {
      let next = sequence.next_element()?;

      let item = match next {
        Some(item) => item,
        None => return Ok(PaletteItems(items)),
      };
      items.push(item);
    }

    let checked_value = sequence.next_element::<IgnoredAny>()?;

    match checked_value {
      None => Ok(PaletteItems(items)),
      Some(_) => Err(A::Error::custom("palette array exceeds the file size limit")),
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this type names the dimensionless WCAG contrast ratio"
  )
)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Deserialize)]
#[serde(transparent, deny_unknown_fields)]
pub struct Ratio(pub f64);

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this type records signed APCA lightness contrast in contrast points"
  )
)]
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(transparent, deny_unknown_fields)]
pub struct LightnessContrast(pub f64);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
  pub metadata: Metadata,
  pub color: IndexMap<String, Color>,
  pub terminal: Terminal,
  #[serde(default, rename = "pair")]
  pub pairs: PaletteItems<Pair>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
  pub name: PaletteText,
  pub about: PaletteText,
  #[serde(rename = "neutral_hue")]
  _neutral_hue: Option<f64>,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "chroma and lightness are the dimensionless coordinates passed to Oklch"
  )
)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Color {
  pub hue: PaletteHue,
  pub chroma: f64,
  #[serde(default)]
  pub lightness: f64,
  pub least: Option<Least>,
  #[serde(default)]
  pub spent: PaletteText,
}

#[derive(Debug)]
pub struct PaletteHue(pub Hue);

impl<'de> Deserialize<'de> for PaletteHue {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    f64::deserialize(deserializer).map(|degrees| Self(Hue(degrees)))
  }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Least {
  #[serde(default)]
  pub on: PaletteItems<String>,
  pub ratio: Option<Ratio>,
  pub lightness_contrast: Option<LightnessContrast>,
  #[serde(default)]
  pub carries: PaletteItems<String>,
  pub carries_ratio: Option<Ratio>,
  pub carries_lightness_contrast: Option<LightnessContrast>,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "bright_lift is a dimensionless change to Oklch lightness"
  )
)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terminal {
  pub background: PaletteText,
  pub foreground: PaletteText,
  pub cursor: PaletteText,
  pub selection: PaletteText,
  pub bright_lift: f64,
  pub normal: IndexMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
  pub front: Names,
  pub back: PaletteItems<String>,
  pub ratio: Ratio,
  pub lightness_contrast: Option<LightnessContrast>,
  #[serde(default)]
  pub kind: PaletteText,
  #[serde(rename = "where")]
  pub where_: PaletteText,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Names {
  One(String),
  Many(Vec<String>),
}

impl Names {
  pub fn each(&self) -> Result<&[String], Never> {
    Ok(match self {
      Names::One(name) => std::slice::from_ref(name),
      Names::Many(names) => names,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn unknown_keys_in_palette_tables_are_rejected() {
    let color = toml::from_str::<Color>("hue = 30\nchroma = 0.1\nligthness = 0.3");

    assert!(matches!(color, Err(_unknown_key)));
  }
}
