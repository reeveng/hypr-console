//! Per-picture overrides for the automatic wallpaper rotation.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use console_core_atomic_writes as writes;
use console_core_never::Never;
use console_core_places::Base;
use console_weather::conditions::Weather;

use crate::choose::{self, Outside, Picture, Turn};
use crate::moon::Moon;
use crate::place::{self, Name};
use crate::sun::{Season, Sky};

#[derive(Debug, Clone, Copy)]
pub enum Category {
  Time,
  Weather,
  Season,
  Moon,
}

impl Category {
  pub const EVERY: [Self; 4] = [Self::Time, Self::Weather, Self::Season, Self::Moon];

  pub const fn label(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Time => "Time of day",
      Self::Weather => "Weather",
      Self::Season => "Season",
      Self::Moon => "Moon phase",
    })
  }

  pub fn options(self) -> Result<Vec<&'static str>, Never> {
    match self {
      Self::Time => [
        Sky::Dawn,
        Sky::Sunrise,
        Sky::Day,
        Sky::Sunset,
        Sky::Dusk,
        Sky::Night,
      ]
      .into_iter()
      .map(Sky::word)
      .collect(),
      Self::Weather => Weather::EVERY.into_iter().map(Weather::word).collect(),
      Self::Season => Season::EVERY.into_iter().map(Season::word).collect(),
      Self::Moon => Moon::EVERY.into_iter().map(Moon::word).collect(),
    }
  }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rule {
  pub automatic: bool,
  sky: Tags,
  weather: Tags,
  season: Tags,
  moon: Tags,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
struct Tags(Vec<String>);

impl<'de> serde::Deserialize<'de> for Tags {
  fn deserialize<D: serde::Deserializer<'de>>(reader: D) -> Result<Self, D::Error> {
    reader.deserialize_seq(ReadTags)
  }
}

struct ReadTags;

impl<'de> serde::de::Visitor<'de> for ReadTags {
  type Value = Tags;

  fn expecting(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    to.write_str("at most 64 wallpaper conditions, each at most 256 bytes")
  }

  fn visit_seq<S: serde::de::SeqAccess<'de>>(self, mut reader: S) -> Result<Tags, S::Error> {
    let mut tags = Vec::new();

    for _ in 0..64 {
      let next = reader.next_element::<String>()?;

      match next {
        Some(tag) => match tag.len().cmp(&256) {
          std::cmp::Ordering::Less | std::cmp::Ordering::Equal => tags.push(tag),
          std::cmp::Ordering::Greater => {
            return Err(serde::de::Error::custom(
              "wallpaper condition exceeds 256 bytes",
            ));
          }
        },
        None => return Ok(Tags(tags)),
      }
    }

    let excess = reader.next_element::<serde::de::IgnoredAny>()?;

    match excess {
      Some(_) => Err(serde::de::Error::custom(
        "wallpaper rule has more than 64 conditions in a category",
      )),
      None => Ok(Tags(tags)),
    }
  }
}

impl Rule {
  pub fn bundled(picture: &Picture) -> Result<Self, Never> {
    Ok(Self {
      automatic: true,
      sky: Tags(picture.sky.clone()),
      weather: Tags(picture.weather.clone()),
      season: Tags(picture.season.clone()),
      moon: Tags(picture.moon.clone()),
    })
  }

  pub fn tags(&self, category: Category) -> Result<&[String], Never> {
    Ok(match category {
      Category::Time => &self.sky.0,
      Category::Weather => &self.weather.0,
      Category::Season => &self.season.0,
      Category::Moon => &self.moon.0,
    })
  }

  pub fn toggle(&mut self, category: Category, tag: &str) -> Result<(), Never> {
    let tags = match category {
      Category::Time => &mut self.sky.0,
      Category::Weather => &mut self.weather.0,
      Category::Season => &mut self.season.0,
      Category::Moon => &mut self.moon.0,
    };
    let ::core::result::Result::Ok(prior) =
      ::console_core_number_conversion::fixed_index(tags.len());
    tags.retain(|entry| !entry.trim().eq_ignore_ascii_case(tag));

    let Ok(remaining) = ::console_core_number_conversion::fixed_index(tags.len());

    match remaining.cmp(&prior) {
      std::cmp::Ordering::Equal => tags.push(tag.to_string()),
      std::cmp::Ordering::Less | std::cmp::Ordering::Greater => {}
    }

    self.automatic = true;
    Ok(())
  }

  fn specificity(&self, outside: &Outside) -> Result<Option<u32>, Never> {
    match bool::then_some(self.automatic, ()) {
      None => Ok(None),
      Some(()) => choose::specificity(
        [&self.sky.0, &self.weather.0, &self.season.0, &self.moon.0],
        outside,
      ),
    }
  }
}

pub type Rules = BTreeMap<String, Rule>;

pub fn directory() -> Result<Option<PathBuf>, Never> {
  let Ok(at) = Base::Configuration.ours();

  Ok(at.map(|at_2| at_2.join("wallpaper-rules")))
}

fn path(name: &str) -> io::Result<PathBuf> {
  let checked = Name::new(name).map_err(io::Error::other)?;
  let Ok(at) = directory();
  let directory =
    at.ok_or_else(|| io::Error::other("Console's configuration directory is unavailable"))?;

  let mut file = directory.join(&checked);
  file.as_mut_os_string().push(".toml");
  Ok(file)
}

const MAX_RULE_BYTES: u32 = 16 * 1024;

fn parse(at: &Path) -> io::Result<Rule> {
  let bytes = writes::read_prefix(at, MAX_RULE_BYTES.saturating_add(1))?;
  let Ok(length) = console_core_number_conversion::fitted::<_, u32>(bytes.len());

  match length.cmp(&MAX_RULE_BYTES) {
    std::cmp::Ordering::Less | std::cmp::Ordering::Equal => {}
    std::cmp::Ordering::Greater => {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "wallpaper rule exceeds 16 KiB",
      ));
    }
  }

  let text = String::from_utf8(bytes).map_err(io::Error::other)?;
  toml::from_str(&text).map_err(io::Error::other)
}

fn read(at: &Path) -> Result<Option<Rule>, Never> {
  Ok(match parse(at) {
    Ok(rule) => Some(rule),
    Err(fault) => match bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
      Some(()) => None,
      None => {
        console_core_logging::warn!("reading {}: {fault}", at.display());
        None
      }
    },
  })
}

pub fn load(name: &str) -> Result<Option<Rule>, Never> {
  match path(name) {
    Ok(at) => read(&at),
    Err(fault) => {
      console_core_logging::warn!("could not locate wallpaper rule for {name}: {fault}");
      Ok(None)
    }
  }
}

pub fn save(name: &str, rule: &Rule) -> io::Result<()> {
  let at = path(name)?;
  let written = toml::to_string(rule).map_err(io::Error::other)?;

  writes::whole_with_folders(&at, written.as_bytes()).map_err(io::Error::other)
}

pub fn reset(name: &str) -> io::Result<()> {
  let at = path(name)?;

  match std::fs::remove_file(at) {
    Ok(()) => Ok(()),
    Err(fault) => match bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
      Some(()) => Ok(()),
      None => Err(fault),
    },
  }
}

pub fn installed() -> Result<Rules, Never> {
  let Ok(directory) = directory();
  let mut rules = Rules::new();

  let here = match directory {
    Some(matched_directory) => matched_directory,
    ::core::option::Option::None => return Ok(rules),
  };

  let entries = match std::fs::read_dir(&here) {
    Ok(entries) => entries,
    Err(fault) => {
      match bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
        Some(()) => {}
        None => console_core_logging::warn!("reading {}: {fault}", here.display()),
      }

      return Ok(rules);
    }
  };

  for entry in entries {
    let entry_3 = match entry {
      Ok(entry_2) => entry_2,
      Err(fault) => {
        console_core_logging::warn!("reading {}: {fault}", here.display());
        continue;
      }
    };
    let file = entry_3.file_name();

    let name = {
      let optional_name = file
        .to_str()
        .and_then(|file_2| file_2.strip_suffix(".toml"));

      match optional_name {
        Some(matched_name) => matched_name,
        ::core::option::Option::None => continue,
      }
    };

    let Ok(picture) = place::picture(name);

    match picture {
      ::core::option::Option::Some(_) => {
        let Ok(rule) = read(&entry_3.path());

        match rule {
          Some(rule_2) => {
            rules.insert(name.to_string(), rule_2);
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {}
    }
  }

  Ok(rules)
}

pub fn pick<'a>(
  rules: &'a Rules,
  bundled: &'a [Picture],
  outside: &Outside,
  turn: Turn,
) -> Result<Option<&'a str>, Never> {
  let configured = rules.iter().filter_map(|(name, rule)| {
    let Ok(matched) = rule.specificity(outside);
    matched.map(|score| (name.as_str(), score))
  });
  let defaults = bundled
    .iter()
    .filter(|picture| !rules.contains_key(&picture.name));
  let Ok(rotation) = choose::rotate(configured, turn);

  match rotation {
    Some(chosen) => Ok(Some(chosen)),
    None => {
      let Ok(fallback) = choose::choose_from(defaults.clone(), outside, turn);

      Ok(
        fallback
          .or_else(|| defaults.into_iter().next())
          .map(|picture| picture.name.as_str()),
      )
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::choose::Set;

  #[test]
  fn configured_pictures_match_across_categories_and_rotate_within_the_set()
  -> Result<(), toml::de::Error> {
    let outside = Outside {
      sky: Sky::Night,
      season: Season::Winter,
      moon: Moon::New,
      weather: Some(Weather::Rain),
    };
    let mut rule = Rule::default();
    let Ok(()) = rule.toggle(Category::Time, "night");
    let Ok(()) = rule.toggle(Category::Weather, "rain");
    let Ok(()) = rule.toggle(Category::Weather, "storm");
    let mut rules = Rules::from([
      ("one".to_string(), rule.clone()),
      ("two".to_string(), rule.clone()),
    ]);
    let Ok(()) = rule.toggle(Category::Weather, "rain");
    rules.insert("only-storm".to_string(), rule);
    let bundled: Set = toml::from_str("[[picture]]\nname = 'default'\nsays = 'Default'")?;

    assert_eq!(
      pick(&rules, &bundled.pictures, &outside, Turn(0)),
      Ok(Some("one"))
    );
    assert_eq!(
      pick(&rules, &bundled.pictures, &outside, Turn(1)),
      Ok(Some("two"))
    );

    let daytime = Outside {
      sky: Sky::Day,
      ..outside
    };
    assert_eq!(
      pick(&rules, &bundled.pictures, &daytime, Turn(0)),
      Ok(Some("default"))
    );
    let offline = Outside {
      weather: None,
      ..outside
    };
    assert_eq!(
      pick(&rules, &bundled.pictures, &offline, Turn(0)),
      Ok(Some("default"))
    );

    rules.insert("default".to_string(), Rule::default());
    assert_eq!(pick(&rules, &bundled.pictures, &daytime, Turn(0)), Ok(None));

    Ok(())
  }

  #[test]
  fn edited_rules_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let mut rule = Rule::default();
    let Ok(()) = rule.toggle(Category::Time, "day");
    let Ok(()) = rule.toggle(Category::Season, "spring");
    rule.automatic = false;

    let held = toml::to_string(&rule)?;

    let restored = toml::from_str::<Rule>(&held)?;
    assert_eq!(restored, rule);
    assert!(matches!(path("../pictures"), Err(fault) if fault.kind() == io::ErrorKind::Other));

    Ok(())
  }
}
