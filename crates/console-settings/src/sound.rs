//! What comes out of the machine, as pactl reports it.
//!
//! A row for the speakers and a row for each thing playing through them, so a
//! video can be turned down without turning down the game.

use std::collections::BTreeMap;

use console_core_never::Never;
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Channel {
    #[serde(default)]
    pub value_percent: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Thing {
    #[serde(default)]
    pub index: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub volume: BTreeMap<String, Channel>,
    #[serde(default)]
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
            Err(_) => return Ok(0),
        };

        Ok(level)
    }

    pub fn said(&self) -> Result<String, Never> {
        for key in ["application.name", "media.name", "node.name"] {
            match self.properties.get(key).and_then(|value| value.as_str()) {
                Some(said) => match said.is_empty() {
                    true => {},
                    false => return Ok(said.to_string()),
                },
                None => {},
            }
        }

        Ok("Something".to_string())
    }
}

pub fn read(json: &str) -> Result<Vec<Thing>, Never> {
    Ok(match serde_json::from_str(json) {
        Ok(things) => things,

        Err(fault) => {
            eprintln!("console: pactl said something this does not know: {fault}");
            Vec::new()
        }
    })
}

pub fn speakers(sinks: &[Thing], default: &str) -> Result<Option<Thing>, Never> {
    Ok(sinks.iter().find(|sink| sink.name == default).or_else(|| sinks.first()).cloned())
}

pub fn one(things: &[Thing], index: i64) -> Result<Option<&Thing>, Never> {
    Ok(things.iter().find(|thing| thing.index == index))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(json: &str) -> Vec<Thing> {
        let Ok(things) = super::read(json);

        things
    }

    fn level(thing: &Thing) -> i32 {
        let Ok(level) = thing.level();

        level
    }

    fn said(thing: &Thing) -> String {
        let Ok(said) = thing.said();

        said
    }

    fn speakers(sinks: &[Thing], default: &str) -> Option<Thing> {
        let Ok(speakers) = super::speakers(sinks, default);

        speakers
    }

    const SAID: &str = r#"[
      {"index": 43, "name": "alsa_output.pci", "mute": false,
       "volume": {"front-left": {"value_percent": "40%"},
                  "front-right": {"value_percent": "40%"}},
       "properties": {"node.name": "alsa_output.pci"}},
      {"index": 44, "name": "bluez", "mute": true, "volume": {},
       "properties": {"application.name": "Firefox", "media.name": "a video"}}
    ]"#;

    #[test]
    fn a_volume_reported_per_channel_is_one_number() {
        assert_eq!(level(&read(SAID)[0]), 40);
    }

    #[test]
    fn a_thing_saying_nothing_about_its_volume_is_at_nothing() {
        assert_eq!(level(&read(SAID)[1]), 0);
    }

    #[test]
    fn a_stream_is_called_what_its_own_application_calls_it() {
        assert_eq!(said(&read(SAID)[1]), "Firefox");
        assert_eq!(said(&read(SAID)[0]), "alsa_output.pci");
    }

    #[test]
    fn a_stream_that_names_itself_nothing_is_still_a_row() {
        assert_eq!(said(&Thing::default()), "Something");
    }

    #[test]
    fn the_speakers_are_the_default_sink_where_there_is_one() {
        let sinks = read(SAID);
        assert_eq!(speakers(&sinks, "bluez").expect("a sink").index, 44);
        assert_eq!(speakers(&sinks, "gone").expect("a sink").index, 43, "the first there is");
        assert!(speakers(&[], "gone").is_none());
    }

    #[test]
    fn nothing_pactl_says_is_ever_a_reason_to_fail() {
        assert!(read("").is_empty());
        assert!(read("Connection refused").is_empty());
    }
}
