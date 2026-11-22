//! Bounds for the persisted wallpaper table and selection.

use std::fmt;
use std::marker::PhantomData;

use console_core_never::Never;
use serde::Deserialize;
use serde::de::{self, Error as _, IgnoredAny, SeqAccess, Visitor};

use crate::choose::{Picture, Set, Wanted};
use crate::grade::Grade;
use crate::place::Name;
use crate::render::Stir;

const TEXT_BYTES: u32 = 16_384;
const TAGS: u32 = 64;
const PICTURES: u32 = 1_024;

#[derive(Default)]
struct Text(String);

impl<'de> Deserialize<'de> for Text {
  fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let value = String::deserialize(deserializer)?;
    let length = u32::try_from(value.len()).map_err(D::Error::custom)?;

    match (length <= TEXT_BYTES).then_some(()) {
      Some(()) => Ok(Self(value)),
      None => Err(D::Error::custom("wallpaper text exceeds 16384 bytes")),
    }
  }
}

struct Bounded<T, const LIMIT: u32>(Vec<T>);

impl<T, const LIMIT: u32> Default for Bounded<T, LIMIT> {
  fn default() -> Self {
    Self(Vec::new())
  }
}

impl<'de, T: Deserialize<'de>, const LIMIT: u32> Deserialize<'de> for Bounded<T, LIMIT> {
  fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    deserializer
      .deserialize_seq(Entries::<T, LIMIT>(PhantomData))
      .map(Self)
  }
}

struct Entries<T, const LIMIT: u32>(PhantomData<T>);

impl<'de, T: Deserialize<'de>, const LIMIT: u32> Visitor<'de> for Entries<T, LIMIT> {
  type Value = Vec<T>;

  fn expecting(&self, into: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(into, "at most {LIMIT} wallpaper entries")
  }

  fn visit_seq<A: SeqAccess<'de>>(self, mut source: A) -> Result<Self::Value, A::Error> {
    let mut read = Vec::new();

    for _ in 0..LIMIT {
      let item = source.next_element()?;

      match item {
        Some(value) => read.push(value),
        None => return Ok(read),
      }
    }

    let extra = source.next_element::<IgnoredAny>()?;

    match extra {
      Some(_extra) => Err(A::Error::custom(format!(
        "more than {LIMIT} wallpaper entries"
      ))),
      None => Ok(read),
    }
  }
}

type Tags = Bounded<Text, TAGS>;

fn strings(tags: Tags) -> Result<Vec<String>, Never> {
  Ok(tags.0.into_iter().map(|Text(value)| value).collect())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PictureFields {
  name: Text,
  says: Text,
  #[serde(default)]
  by: Text,
  #[serde(default)]
  from: Text,
  #[serde(default)]
  sha256: Text,
  #[serde(default)]
  grade: Option<Grade>,
  #[serde(default)]
  sky: Tags,
  #[serde(default)]
  weather: Tags,
  #[serde(default)]
  season: Tags,
  #[serde(default)]
  moon: Tags,
}

impl<'de> Deserialize<'de> for Picture {
  fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let fields = PictureFields::deserialize(deserializer)?;
    let _checked = Name::new(&fields.name.0).map_err(D::Error::custom)?;
    let Ok(sky) = strings(fields.sky);
    let Ok(weather) = strings(fields.weather);
    let Ok(season) = strings(fields.season);
    let Ok(moon) = strings(fields.moon);

    Ok(Self {
      name: fields.name.0,
      says: fields.says.0,
      by: fields.by.0,
      from: fields.from.0,
      sha256: fields.sha256.0,
      grade: fields.grade,
      sky,
      weather,
      season,
      moon,
    })
  }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetFields {
  #[serde(default)]
  stir: Stir,
  #[serde(default, rename = "picture")]
  pictures: Bounded<Picture, PICTURES>,
}

impl<'de> Deserialize<'de> for Set {
  fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let fields = SetFields::deserialize(deserializer)?;

    Ok(Self {
      stir: fields.stir,
      pictures: fields.pictures.0,
    })
  }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WantedFields {
  follow: bool,
  picture: Text,
}

impl<'de> Deserialize<'de> for Wanted {
  fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let fields = WantedFields::deserialize(deserializer)?;

    match fields.picture.0.as_str() {
      "" => {}
      name => {
        let _checked = Name::new(name).map_err(D::Error::custom)?;
      }
    }

    Ok(Self {
      follow: fields.follow,
      picture: fields.picture.0,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_table_refuses_unknown_fields_and_names_that_climb() {
    let unknown = toml::from_str::<Set>("[stir]\nextra = 1\n");
    let wrong_name =
      toml::from_str::<Set>("[[picture]]\nname = '../outside'\nsays = 'Outside'\n");

    assert!(matches!(unknown, Err(_unknown)));
    assert!(matches!(wrong_name, Err(_invalid)));
  }

  #[test]
  fn a_list_of_weather_tags_is_bounded_during_decoding() {
    let mut table = String::from("[[picture]]\nname = 'river'\nsays = 'River'\nweather = [");

    for _ in 0..=TAGS {
      table.push_str("'rain',");
    }

    table.push(']');
    let read = toml::from_str::<Set>(&table);

    assert!(matches!(read, Err(_too_many_tags)));
  }
}
