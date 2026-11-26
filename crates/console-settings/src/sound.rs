//! What comes out of the machine, as pactl reports it.
//!
//! The current output and the applications playing audio. The processor's
//! internal nodes are hidden from the device and application controls.

use std::collections::{BTreeMap, BTreeSet};

use console_core_never::Never;
use serde::de::{Deserialize, DeserializeOwned, Deserializer, Error};
use serde_json::{Map, Value};

use crate::level::Muted;

#[derive(Debug, Clone, Default)]
pub struct Channel {
  pub value_percent: String,
}

#[derive(Debug, Clone, Default)]
pub struct Thing {
  pub index: i64,
  pub name: String,
  pub mute: Muted,
  pub volume: BTreeMap<String, Channel>,
  pub properties: BTreeMap<String, serde_json::Value>,
  description: String,
  port: Option<String>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum App {
  Identified(&'static str, String),
  Stream(i64),
}

impl App {
  #[must_use]
  pub(crate) fn leveling_key(&self) -> core::result::Result<Option<String>, Never> {
    let (field, name) = match self {
      Self::Identified(field @ ("application.id" | "application.name"), name) => (field, name),
      Self::Identified(_, _) | Self::Stream(_) => return Ok(None),
    };

    match { ::core::primitive::bool::then_some(name.len() > 512, ()) } {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let mut key = format!("app.{field}.");

    let Ok(()) = console_core_bytes::hex::append(
      &mut key,
      name.as_bytes(),
      console_core_bytes::hex::Case::Lower,
    );

    Ok(Some(key))
  }
}

#[derive(Clone)]
pub(crate) enum VolumeTarget {
  Output(i64),
  App(App),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetMatch {
  Selected,
  Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioNode {
  Application,
  Internal,
}

impl VolumeTarget {
  #[must_use]
  pub(crate) fn kind(&self) -> core::result::Result<&'static str, Never> {
    Ok(match self {
      Self::Output(_) => "sink",
      Self::App(_) => "sink-input",
    })
  }

  #[must_use]
  pub(crate) fn matches(&self, item: &Thing) -> Result<TargetMatch, Never> {
    let selected = match self {
      Self::Output(index) => item.index == *index,
      Self::App(app) => {
        let Ok(node) = item.node();

        match node {
          AudioNode::Application => {
            let Ok(identity) = item.app();
            identity == *app
          }
          AudioNode::Internal => false,
        }
      }
    };

    Ok(match selected.then_some(()) {
      Some(()) => TargetMatch::Selected,
      None => TargetMatch::Other,
    })
  }
}

pub(crate) const AUTOMATIC_LEVELING: &str = "console.balanced-playback.automatic-leveling";
pub(crate) const NIGHT_MODE: &str = "console.balanced-playback.night-mode";
pub(crate) const APP_LEVELING: &str = "console-balanced-playback";

#[must_use]
pub(crate) fn bypassed_apps(
  report: &str,
) -> core::result::Result<Option<BTreeSet<String>>, Never> {
  let mut lines = report.lines();

  let header = match lines.next() {
    Some(header) => header,
    None => return Ok(None),
  };

  match {
    ::core::primitive::bool::then_some(
      !header.starts_with("Found \"console-balanced-playback\" metadata "),
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  Ok(Some(
    lines
      .filter_map(|line| {
        let split_once = line.split_once("key:'")?;
        let fields = split_once.1;
        let (key, fields_2) = fields.split_once("' value:'")?;
        fields_2.starts_with("false'").then(|| key.to_owned())
      })
      .collect(),
  ))
}

#[must_use]
pub(crate) fn automatic_leveling(
  report: &str,
) -> core::result::Result<Option<console_core_controls::Switch>, Never> {
  use console_core_controls::Switch;

  let value = match report.strip_prefix("Value: ") {
    Some(value) => value,
    None => return Ok(None),
  };
  let next = match value.split_whitespace().next() {
    Some(next) => next,
    None => return Ok(None),
  };

  Ok(match next {
    "true" => Some(Switch::On),
    "false" => Some(Switch::Off),
    _ => None,
  })
}

impl Thing {
  pub fn level(&self) -> Result<i32, Never> {
    Ok(
      match self
        .volume
        .values()
        .filter_map(|channel| {
          match {
            let checked_value_2 = channel.value_percent.strip_suffix('%')?;
            checked_value_2
          }
          .parse::<i32>()
          {
            ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
            ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None,
          }
        })
        .max()
      {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0,
      },
    )
  }

  #[must_use]
  pub fn output_name(&self) -> Result<String, Never> {
    let label = [&self.description, &self.name]
      .into_iter()
      .map(String::as_str)
      .find(|label| !label.trim().is_empty());
    let description = match label {
      Some(named) => named,
      None => "Audio Output",
    };

    Ok(
      match self
        .port
        .as_deref()
        .filter(|port| !description.ends_with(port))
      {
        Some(port) => format!("{description} ({port})"),
        None => description.to_owned(),
      },
    )
  }

  #[must_use]
  pub fn node(&self) -> Result<AudioNode, Never> {
    let internal = self.properties.contains_key("console.balance.output")
      || self.properties.contains_key("console.balance.preview");

    Ok(match internal.then_some(()) {
      Some(()) => AudioNode::Internal,
      None => AudioNode::Application,
    })
  }

  #[must_use]
  pub(crate) fn app(&self) -> core::result::Result<App, Never> {
    for property in [
      "application.id",
      "application.name",
      "media.name",
      "node.name",
    ] {
      match self.properties.get(property).and_then(Value::as_str) {
        Some("") | None => {}
        Some(value) => return Ok(App::Identified(property, value.to_owned())),
      }
    }

    Ok(App::Stream(self.index))
  }

  pub fn display_name(&self) -> Result<String, Never> {
    for key in [
      "application.name",
      "application.id",
      "media.name",
      "node.name",
    ] {
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
    let reported_mute: bool = field(&mut fields, "mute").map_err(D::Error::custom)?;
    let volume = field(&mut fields, "volume").map_err(D::Error::custom)?;
    let properties = field(&mut fields, "properties").map_err(D::Error::custom)?;
    let description = field(&mut fields, "description").map_err(D::Error::custom)?;
    let ports = fields.get("ports").and_then(Value::as_array);
    let port = fields
      .get("active_port")
      .and_then(Value::as_str)
      .and_then(|active| {
        let ports_2 = ports?;
        ports_2
          .iter()
          .find(|port| port.get("name").and_then(Value::as_str) == Some(active))
      })
      .and_then(|port| port.get("description").and_then(Value::as_str))
      .filter(|description_2| !description_2.trim().is_empty())
      .map(str::to_owned);

    Ok(Thing {
      index,
      name,
      mute: Muted::from(reported_mute),
      volume,
      properties,
      description,
      port,
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

#[must_use]
pub fn outputs(sinks: &[Thing]) -> Result<impl Iterator<Item = &Thing>, Never> {
  Ok(sinks.iter().filter(|sink| {
    !sink.name.is_empty()
      && sink.name != "auto_null"
      && !sink.properties.contains_key("wireplumber.is-fallback")
      && !sink.properties.contains_key("console.balance.sink")
  }))
}

#[must_use]
pub fn selected(outputs: &[&Thing], default: &str) -> Result<u32, Never> {
  match outputs.iter().position(|sink| sink.name == default.trim()) {
    Some(position) => console_core_number_conversion::fitted(position),
    None => Ok(0),
  }
}

#[must_use]
pub(crate) fn applications(
  playing: &[Thing],
) -> core::result::Result<BTreeMap<App, Vec<&Thing>>, Never> {
  let mut apps = BTreeMap::<App, Vec<&Thing>>::new();

  for stream in playing
    .iter()
    .filter(|stream| stream.node() == Ok(AudioNode::Application))
  {
    apps
      .entry({
        let Ok(explicit_value) = stream.app();
        explicit_value
      })
      .or_default()
      .push(stream);
  }

  Ok(apps)
}

#[must_use]
pub(crate) fn loudest(streams: &[&Thing]) -> core::result::Result<i32, Never> {
  Ok(
    match streams
      .iter()
      .map(|stream| {
        let Ok(level) = stream.level();
        level
      })
      .max()
    {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => 0,
    },
  )
}

pub(crate) const DEFAULT_APP_VOLUME: i32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamVolume {
  Adjusted,
  Default,
}

#[must_use]
pub(crate) fn adjusted(stream: &Thing) -> Result<StreamVolume, Never> {
  let different = stream
    .volume
    .values()
    .any(|channel| channel.value_percent != "100%");

  Ok(match different.then_some(()) {
    Some(()) => StreamVolume::Adjusted,
    None => StreamVolume::Default,
  })
}

pub(crate) struct VolumeStep {
  pub from: i32,
  pub to: i32,
}

#[must_use]
pub(crate) fn volume_argument(step: VolumeStep) -> Result<String, Never> {
  let VolumeStep { from, to } = step;

  Ok(match (from, to) {
    (1.., 1..) => {
      let decibels = 60.0 * (f64::from(to) / f64::from(from)).log10();
      format!("{decibels:+.6}dB")
    }
    _ => format!("{to}%"),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  const SAID: &str = r#"[
      {"index": 43, "name": "alsa_output.pci", "mute": false, "driver": "PipeWire",
       "volume": {"front-left": {"value_percent": "25%", "db": "-36.12 dB"},
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
  fn connected_outputs_have_their_own_names_and_hide_internal_nodes() {
    let Ok(sinks) = read(
      r#"[
      {"name":"auto_null"},
      {"name":"console.balance.3","properties":{"console.balance.sink":"3"}},
      {"name":"alsa_output.pci","description":"Built-in Audio","active_port":"analog-output-headphones",
        "ports":[{"name":"analog-output-headphones","description":"Headphones"}]},
      {"name":"bluez_output","description":"Wireless Headphones"}
    ]"#,
    );
    let Ok(available) = outputs(&sinks);
    let outputs: Vec<_> = available.collect();
    let labels: Vec<_> = outputs
      .iter()
      .map(|sink| {
        let Ok(label) = sink.output_name();
        label
      })
      .collect();

    assert_eq!(
      labels,
      ["Built-in Audio (Headphones)", "Wireless Headphones"]
    );
    assert_eq!(selected(&outputs, "bluez_output\n"), Ok(1));
    assert_eq!(selected(&outputs, "unplugged"), Ok(0));
  }

  #[test]
  fn a_volume_change_scales_channels_together_until_silence() {
    assert_eq!(
      ({
        let Ok(explicit_value) = volume_argument(VolumeStep { from: 40, to: 80 });
        explicit_value
      }),
      "+18.061800dB"
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = volume_argument(VolumeStep { from: 80, to: 40 });
        explicit_value
      }),
      "-18.061800dB"
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = volume_argument(VolumeStep { from: 5, to: 0 });
        explicit_value
      }),
      "0%"
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = volume_argument(VolumeStep { from: 0, to: 5 });
        explicit_value
      }),
      "5%"
    );
  }

  #[test]
  fn the_automatic_setting_reads_the_effective_value() {
    use console_core_controls::Switch;

    assert_eq!(
      ({
        let Ok(explicit_value) = automatic_leveling("Value: true");
        explicit_value
      }),
      Some(Switch::On)
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = automatic_leveling("Value: false (Saved: true)");
        explicit_value
      }),
      Some(Switch::Off)
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = automatic_leveling("Setting not found");
        explicit_value
      }),
      None
    );
  }

  #[test]
  fn app_overrides_use_the_stable_identity_and_only_list_bypasses() {
    let Ok(key) = App::Identified("application.id", "Player 🎵".to_string()).leveling_key();
    assert_eq!(
      key.as_deref(),
      Some("app.application.id.506c6179657220f09f8eb5")
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = App::Stream(4).leveling_key();
        explicit_value
      }),
      None
    );
    let report = "Found \"console-balanced-playback\" metadata 43\n\
      update: id:0 key:'app.application.name.61' value:'false' type:'Spa:String:JSON'\n\
      update: id:0 key:'app.application.name.62' value:'true' type:'Spa:String:JSON'\n";
    assert_eq!(
      ({
        let Ok(explicit_value) = bypassed_apps(report);
        explicit_value
      }),
      Some(BTreeSet::from(["app.application.name.61".to_owned()]))
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = bypassed_apps("");
        explicit_value
      }),
      None
    );
  }

  #[test]
  fn nothing_pactl_says_is_ever_a_reason_to_fail() {
    let Ok(empty) = read("");
    let Ok(refused) = read("Connection refused");

    assert!(empty.is_empty());
    assert!(refused.is_empty());
  }
}
