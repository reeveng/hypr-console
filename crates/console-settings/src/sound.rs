//! What comes out of the machine, as pactl reports it.
//!
//! A row for the speakers and a row for each thing playing through them, so a
//! video can be turned down without turning down the game.

use std::collections::BTreeMap;

use serde::Deserialize;

/// One channel's share of a volume.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Channel {
    #[serde(default)]
    pub value_percent: String,
}

/// A sink or a stream, as pactl describes it.
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
    /// One number for a volume that is reported per channel.
    pub fn level(&self) -> i32 {
        self.volume
            .values()
            .next()
            .map(|channel| channel.value_percent.trim_end_matches('%'))
            .and_then(|said| said.parse().ok())
            .unwrap_or(0)
    }

    /// What to call a stream, in the words its own application uses.
    pub fn said(&self) -> String {
        for key in ["application.name", "media.name", "node.name"] {
            if let Some(said) = self.properties.get(key).and_then(|value| value.as_str())
                && !said.is_empty()
            {
                return said.to_string();
            }
        }
        "Something".to_string()
    }
}

/// Everything of one kind, or nothing if pactl said something else.
pub fn read(json: &str) -> Vec<Thing> {
    serde_json::from_str(json).unwrap_or_default()
}

/// The speakers: whichever sink is the default, or the first there is.
pub fn speakers(sinks: &[Thing], default: &str) -> Option<Thing> {
    sinks
        .iter()
        .find(|sink| sink.name == default)
        .or_else(|| sinks.first())
        .cloned()
}

/// One of them by the number pactl knows it as.
pub fn one(things: &[Thing], index: i64) -> Option<&Thing> {
    things.iter().find(|thing| thing.index == index)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(read(SAID)[0].level(), 40);
    }

    /// A stream with no volume at all is not a stream at half.
    #[test]
    fn a_thing_saying_nothing_about_its_volume_is_at_nothing() {
        assert_eq!(read(SAID)[1].level(), 0);
    }

    #[test]
    fn a_stream_is_called_what_its_own_application_calls_it() {
        assert_eq!(read(SAID)[1].said(), "Firefox");
        assert_eq!(read(SAID)[0].said(), "alsa_output.pci");
    }

    #[test]
    fn a_stream_that_names_itself_nothing_is_still_a_row() {
        assert_eq!(Thing::default().said(), "Something");
    }

    #[test]
    fn the_speakers_are_the_default_sink_where_there_is_one() {
        let sinks = read(SAID);
        assert_eq!(speakers(&sinks, "bluez").expect("a sink").index, 44);
        assert_eq!(speakers(&sinks, "gone").expect("a sink").index, 43, "the first there is");
        assert!(speakers(&[], "gone").is_none());
    }

    /// pactl answers with an error on stderr and nothing on stdout when there
    /// is no sound server. A panel with no Sound tab is worse than one saying
    /// nothing is playing.
    #[test]
    fn nothing_pactl_says_is_ever_a_reason_to_fail() {
        assert!(read("").is_empty());
        assert!(read("Connection refused").is_empty());
    }
}
