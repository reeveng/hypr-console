//! What comes out of the machine, as pactl reports it.
//!
//! A row for the speakers and a row for each thing playing through them, so a
//! video can be turned down without turning down the game.

use std::collections::BTreeMap;

use console_core_never::Never;
use serde::de::{Deserialize, DeserializeOwned, Deserializer, Error};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Default)]
pub struct Channel {
  pub value_percent: String,
}

#[derive(Debug, Clone, Default)]
pub struct Thing {
  pub index: i64,
  pub name: String,
  pub mute: bool,
  pub volume: BTreeMap<String, Channel>,
  pub properties: BTreeMap<String, serde_json::Value>,
}

impl Thing {
  pub fn level(&self) -> Result<i32, Never> {
    let channel = match self.volume.values().next() {
      Some(channel) => channel,
      None => return Ok(0),
    };

    let level = match channel.value_percent.trim_end_matches('%').parse::<i32>() {
      Ok(level) => level,
      Err(_not_a_number) => return Ok(0),
    };

    Ok(level)
  }

  pub fn display_name(&self) -> Result<String, Never> {
    for key in ["application.name", "media.name", "node.name"] {
      match self.properties.get(key).and_then(|value| value.as_str()) {
        Some(said) => match ::core::primitive::bool::then_some(said.is_empty(), ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => return Ok(said.to_string()),
        },
        None => {}
      }
    }

    Ok("Something".to_string())
  }
}

const MAX_PACTL_BYTES: u64 = 8 * 1024 * 1024;

fn field<T: DeserializeOwned + Default>(
  fields: &mut Map<String, Value>,
  name: &str,
) -> Result<T, serde_json::Error> {
  match fields.remove(name) {
    Some(value) => serde_json::from_value(value),
    None => Ok(T::default()),
  }
}

impl<'de> Deserialize<'de> for Channel {
  fn deserialize<D: Deserializer<'de>>(source: D) -> Result<Self, D::Error> {
    let mut fields = Map::deserialize(source)?;
    let value_percent = field(&mut fields, "value_percent").map_err(D::Error::custom)?;

    Ok(Channel { value_percent })
  }
}

impl<'de> Deserialize<'de> for Thing {
  fn deserialize<D: Deserializer<'de>>(source: D) -> Result<Self, D::Error> {
    let mut fields = Map::deserialize(source)?;
    let index = field(&mut fields, "index").map_err(D::Error::custom)?;
    let name = field(&mut fields, "name").map_err(D::Error::custom)?;
    let mute = field(&mut fields, "mute").map_err(D::Error::custom)?;
    let volume = field(&mut fields, "volume").map_err(D::Error::custom)?;
    let properties = field(&mut fields, "properties").map_err(D::Error::custom)?;

    Ok(Thing {
      index,
      name,
      mute,
      volume,
      properties,
    })
  }
}

pub fn read(json: &str) -> Result<Vec<Thing>, Never> {
  let Ok(length) = console_core_number_conversion::fitted::<_, u64>(json.len());

  match length.cmp(&MAX_PACTL_BYTES) {
    std::cmp::Ordering::Greater => {
      console_core_logging::warn!("pactl output exceeded {MAX_PACTL_BYTES} bytes");
      return Ok(Vec::new());
    }
    std::cmp::Ordering::Less | std::cmp::Ordering::Equal => {}
  }

  Ok(match serde_json::from_str::<Vec<Thing>>(json) {
    Ok(things) => things,
    Err(fault) => {
      console_core_logging::warn!("pactl output could not be decoded: {fault}");
      Vec::new()
    }
  })
}

pub fn speakers(sinks: &[Thing], default: &str) -> Result<Option<Thing>, Never> {
  Ok(
    sinks
      .iter()
      .find(|sink| sink.name == default)
      .or_else(|| sinks.first())
      .cloned(),
  )
}

pub fn one(things: &[Thing], index: i64) -> Result<Option<&Thing>, Never> {
  Ok(things.iter().find(|thing| thing.index == index))
}

#[cfg(test)]
mod tests {
  use super::*;

  const SAID: &str = r#"[
      {"index": 43, "name": "alsa_output.pci", "mute": false, "driver": "PipeWire",
       "volume": {"front-left": {"value_percent": "40%", "db": "-23.88 dB"},
                  "front-right": {"value_percent": "40%"}},
       "properties": {"node.name": "alsa_output.pci"}},
      {"index": 44, "name": "bluez", "mute": true, "volume": {},
       "properties": {"application.name": "Firefox", "media.name": "a video"}}
    ]"#;

  const TWO: &str = "pactl said two things";

  fn both() -> Result<Option<(Thing, Thing)>, Never> {
    let Ok(things) = read(SAID);

    Ok(match things.as_slice() {
      [sink, stream] => Some((sink.clone(), stream.clone())),
      _other => None,
    })
  }

  #[test]
  fn a_volume_reported_per_channel_is_one_number() -> Result<(), &'static str> {
    let Ok(both) = both();
    let (sink, _stream) = both.ok_or(TWO)?;

    assert_eq!(sink.level(), Ok(40));

    Ok(())
  }

  #[test]
  fn a_thing_saying_nothing_about_its_volume_is_at_nothing() -> Result<(), &'static str> {
    let Ok(both) = both();
    let (_sink, stream) = both.ok_or(TWO)?;

    assert_eq!(stream.level(), Ok(0));

    Ok(())
  }

  #[test]
  fn a_stream_is_called_what_its_own_application_calls_it() -> Result<(), &'static str> {
    let Ok(both) = both();
    let (sink, stream) = both.ok_or(TWO)?;

    assert_eq!(stream.display_name(), Ok("Firefox".to_string()));
    assert_eq!(sink.display_name(), Ok("alsa_output.pci".to_string()));

    Ok(())
  }

  #[test]
  fn a_stream_that_names_itself_nothing_is_still_a_row() {
    assert_eq!(Thing::default().display_name(), Ok("Something".to_string()));
  }

  #[test]
  fn the_speakers_are_the_default_sink_where_there_is_one() {
    let Ok(sinks) = read(SAID);
    let Ok(chosen) = speakers(&sinks, "bluez");
    let Ok(first) = speakers(&sinks, "gone");
    let Ok(none) = speakers(&[], "gone");

    assert_eq!(chosen.map(|sink| sink.index), Some(44));
    assert_eq!(first.map(|sink| sink.index), Some(43), "the first there is");
    assert!(none.is_none());
  }

  #[test]
  fn nothing_pactl_says_is_ever_a_reason_to_fail() {
    let Ok(empty) = read("");
    let Ok(refused) = read("Connection refused");

    assert!(empty.is_empty());
    assert!(refused.is_empty());
  }
}
