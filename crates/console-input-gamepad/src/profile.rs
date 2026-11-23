//! An InputPlumber profile, read as what each button turns into.
//!
//! A profile is the whole of what a button means: the compositor is not in the
//! loop and the daemons only see what came out of here. So a change to what
//! the device does is a change to one of these files, and anything that wants
//! to know what the device does, the guide and the tests included, reads them
//! rather than being told twice.
//!
//! Reading one is behind `read`, which the handheld does not build. The device
//! only ever *writes* profiles -- `router::Router::yaml` composes the one it
//! wears, by hand -- and everything that reads one back is a check, a test or
//! the emulator, all of which run on a laptop. What the feature costs when it
//! is on is a YAML parser and the C library it was transliterated from, and
//! that is a strange thing to compile on a handheld for a program that never
//! calls it.
//!
//! The reader supports the v1 InputPlumber profile fields and models gamepad
//! sources and keyboard, mouse and gamepad targets. Other v1 event types are
//! accepted so that a profile can use them alongside the mappings the emulator
//! exercises.
//! The file size and decoded field lengths are limited because YAML aliases
//! can repeat the same input.

#[cfg(feature = "read")]
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[cfg(feature = "read")]
use console_core_bounded_reads::{ReadError, text as read_text};
use console_core_never::Never;
#[cfg(feature = "read")]
use console_core_number_conversion::fixed_index;
use console_core_words::Words;
use console_input_event_devices::KeyCode;
#[cfg(feature = "read")]
use serde::Deserialize;
#[cfg(feature = "read")]
use serde::de::IgnoredAny;

use crate::GamepadError;
#[cfg(feature = "read")]
use crate::bounded::{List, Text};
use crate::devices::Has;
use crate::vocabulary;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Words)]
pub enum Kind {
  #[words(said = "key", needs = "keyboard")]
  Key,
  #[words(said = "mouse-button", needs = "mouse")]
  MouseButton,
  #[words(said = "mouse-motion", needs = "mouse")]
  MouseMotion,
  #[words(said = "gamepad-button", needs = "xbox-elite")]
  GamepadButton,
  #[words(said = "gamepad-axis", needs = "xbox-elite")]
  GamepadAxis,
  #[words(said = "gamepad-trigger", needs = "xbox-elite")]
  GamepadTrigger,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Target {
  pub kind: Kind,
  pub name: String,
}

impl Target {
  pub fn code(&self) -> Result<KeyCode, GamepadError> {
    match self.kind {
      Kind::Key => vocabulary::key_code(&self.name),
      Kind::MouseButton => {
        let Ok(code) = vocabulary::mouse_code(&self.name);

        code.ok_or_else(|| GamepadError::NoMouseButton(self.name.clone()))
      }
      Kind::GamepadButton => {
        let Ok(code) = vocabulary::gamepad_code(&self.name);

        code.ok_or_else(|| GamepadError::NoPadButton(self.name.clone()))
      }
      Kind::MouseMotion | Kind::GamepadAxis | Kind::GamepadTrigger => {
        Err(GamepadError::NotOneCode(self.kind, self.name.clone()))
      }
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
  Button(String),
  Axis {
    name: String,
    direction: Option<String>,
    deadzone: Option<f64>,
  },
  Trigger {
    name: String,
    deadzone: Option<f64>,
  },
}

impl Source {
  pub fn button(&self) -> Result<Option<&str>, Never> {
    match self {
      Source::Button(name) => {
        let spoken = vocabulary::spoken_for(name)?;

        Ok(Some(spoken))
      }
      Source::Axis { .. } | Source::Trigger { .. } => Ok(None),
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mapping {
  pub label: String,
  pub source: Source,
  pub targets: Vec<Target>,
}

impl Mapping {
  pub fn does(&self) -> Result<&str, Never> {
    Ok(
      self
        .label
        .split_once(" - ")
        .map_or("", |(_, does)| does)
        .trim(),
    )
  }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Profile {
  pub path: PathBuf,
  pub name: String,
  pub description: String,
  pub target_devices: Vec<String>,
  pub mappings: Vec<Mapping>,
}

#[cfg(feature = "read")]
impl Profile {
  pub fn read(path: &Path, yaml: &str) -> Result<Self, GamepadError> {
    let Ok(bytes) = fixed_index(yaml.len());

    match bytes.cmp(&u64::from(MAX_PROFILE_BYTES)) {
      std::cmp::Ordering::Greater => {
        let fault = ReadError::TooLong {
          limit: MAX_PROFILE_BYTES,
        };
        return Err(GamepadError::Read(
          path.to_path_buf(),
          std::io::Error::from(fault),
        ));
      }
      std::cmp::Ordering::Equal | std::cmp::Ordering::Less => {}
    }

    let raw: Raw = serde_yaml_ng::from_str(yaml)
      .map_err(|fault| GamepadError::Parse(path.to_path_buf(), fault))?;

    let mut mappings = Vec::new();

    let told = match raw.mapping {
      Some(ref told) => told.0.as_slice(),
      None => &[],
    };

    for raw_2 in told {
      let Ok(mapping) = read_mapping(raw_2);

      match mapping {
        Some(mapping_2) => mappings.push(mapping_2),
        None => {}
      }
    }

    Ok(Profile {
      path: path.to_path_buf(),
      name: match raw.name {
        Some(name) => name.0,
        None => stem_at(path)?,
      },
      description: match raw.description {
        Some(ref said) => said.0.trim().to_string(),
        None => String::new(),
      },
      target_devices: match raw.target_devices {
        Some(named) => named.0.into_iter().map(|name| name.0).collect(),
        None => Vec::new(),
      },
      mappings,
    })
  }
}

impl Profile {
  pub fn publishes(&self, target_device: &str) -> Result<Has, Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        self
          .target_devices
          .iter()
          .any(|named| named == target_device),
        (),
      ) {
        ::core::option::Option::Some(()) => Has::Yes,
        ::core::option::Option::None => Has::No,
      },
    )
  }

  pub fn for_button(&self, spoken: &str) -> Result<Vec<&Mapping>, GamepadError> {
    let name = vocabulary::button_name(spoken)?;
    Ok(
      self
        .mappings
        .iter()
        .filter(|mapping| matches!(&mapping.source, Source::Button(said) if said == name))
        .collect(),
    )
  }

  pub fn targets_of(&self, spoken: &str) -> Result<Vec<&Target>, GamepadError> {
    let mappings = self.for_button(spoken)?;

    Ok(
      mappings
        .iter()
        .flat_map(|mapping| &mapping.targets)
        .collect(),
    )
  }

  pub fn stem(&self) -> Result<String, GamepadError> {
    stem_at(&self.path)
  }
}

fn stem_at(path: &Path) -> Result<String, GamepadError> {
  match path.file_stem() {
    Some(stem) => stem
      .to_str()
      .map(str::to_owned)
      .ok_or_else(|| GamepadError::InvalidProfileName(path.to_path_buf())),
    None => Ok(String::new()),
  }
}

#[cfg(feature = "read")]
pub const PROFILE_DIR: &str = "files/etc/inputplumber/profiles";

#[cfg(feature = "read")]
const MAX_PROFILE_BYTES: u32 = 512 * 1024 - 1;

#[cfg(feature = "read")]
const MAX_DESCRIPTION_BYTES: u32 = 64 * 1024;
#[cfg(feature = "read")]
const MAX_LABEL_BYTES: u32 = 4096;
#[cfg(feature = "read")]
const MAX_TOKEN_BYTES: u32 = 256;
#[cfg(feature = "read")]
const MAX_TARGET_DEVICES: u32 = 64;
#[cfg(feature = "read")]
const MAX_MAPPINGS: u32 = 1024;
#[cfg(feature = "read")]
const MAX_TARGET_EVENTS: u32 = 32;

#[cfg(feature = "read")]
pub fn load_all(root: &Path) -> Result<BTreeMap<String, Profile>, GamepadError> {
  let holding = root.join(PROFILE_DIR);
  let listed =
    std::fs::read_dir(&holding).map_err(|fault| GamepadError::Read(holding.clone(), fault))?;
  let mut found: Vec<PathBuf> = listed
    .filter_map(|entry| match entry {
      Ok(entry_2) => Some(entry_2.path()),
      Err(_unreadable_entry) => None,
    })
    .filter(|path| path.extension().is_some_and(|kind| kind == "yaml"))
    .collect();
  found.sort();
  found
    .iter()
    .map(|path| {
      let file =
        std::fs::File::open(path).map_err(|fault| GamepadError::Read(path.clone(), fault))?;
      let yaml = read_text(file, MAX_PROFILE_BYTES)
        .map_err(|fault| GamepadError::Read(path.clone(), std::io::Error::from(fault)))?;
      let profile = Profile::read(path, &yaml)?;
      let stem = profile.stem()?;

      Ok((stem, profile))
    })
    .collect()
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
  #[serde(rename = "version")]
  _version: Option<IgnoredAny>,
  #[serde(rename = "kind")]
  _kind: Option<IgnoredAny>,
  name: Option<Text<MAX_LABEL_BYTES>>,
  description: Option<Text<MAX_DESCRIPTION_BYTES>>,
  target_devices: Option<List<Text<MAX_TOKEN_BYTES>, MAX_TARGET_DEVICES>>,
  mapping: Option<List<RawMapping, MAX_MAPPINGS>>,
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMapping {
  name: Option<Text<MAX_LABEL_BYTES>>,
  source_event: Option<RawSourceEvent>,
  target_events: Option<List<RawTargetEvent, MAX_TARGET_EVENTS>>,
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSourceEvent {
  gamepad: Option<RawGamepad>,
  #[serde(rename = "keyboard")]
  _keyboard: Option<IgnoredAny>,
  #[serde(rename = "mouse")]
  _mouse: Option<IgnoredAny>,
  #[serde(rename = "touchpad")]
  _touchpad: Option<IgnoredAny>,
  #[serde(rename = "touchscreen")]
  _touchscreen: Option<IgnoredAny>,
  #[serde(rename = "dbus")]
  _dbus: Option<IgnoredAny>,
  #[serde(rename = "gyroscope")]
  _gyroscope: Option<IgnoredAny>,
  #[serde(rename = "accelerometer")]
  _accelerometer: Option<IgnoredAny>,
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGamepad {
  button: Option<Text<MAX_TOKEN_BYTES>>,
  axis: Option<RawAxis>,
  trigger: Option<RawTrigger>,
  #[serde(rename = "gyro")]
  _gyro: Option<IgnoredAny>,
  #[serde(rename = "accelerometer")]
  _accelerometer: Option<IgnoredAny>,
  #[serde(rename = "dial")]
  _dial: Option<IgnoredAny>,
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAxis {
  name: Text<MAX_TOKEN_BYTES>,
  direction: Option<Text<MAX_TOKEN_BYTES>>,
  deadzone: Option<f64>,
  #[serde(rename = "quadratic_scaling")]
  _quadratic_scaling: Option<IgnoredAny>,
  #[serde(rename = "invert")]
  _invert: Option<IgnoredAny>,
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTrigger {
  name: Text<MAX_TOKEN_BYTES>,
  deadzone: Option<f64>,
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTargetEvent {
  keyboard: Option<Text<MAX_TOKEN_BYTES>>,
  mouse: Option<RawMouse>,
  gamepad: Option<RawGamepad>,
  #[serde(rename = "touchpad")]
  _touchpad: Option<IgnoredAny>,
  #[serde(rename = "touchscreen")]
  _touchscreen: Option<IgnoredAny>,
  #[serde(rename = "dbus")]
  _dbus: Option<IgnoredAny>,
  #[serde(rename = "gyroscope")]
  _gyroscope: Option<IgnoredAny>,
  #[serde(rename = "accelerometer")]
  _accelerometer: Option<IgnoredAny>,
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMouse {
  button: Option<Text<MAX_TOKEN_BYTES>>,
  motion: Option<IgnoredAny>,
  #[serde(rename = "wheel")]
  _wheel: Option<IgnoredAny>,
}

#[cfg(feature = "read")]
fn read_mapping(raw: &RawMapping) -> Result<Option<Mapping>, Never> {
  let source_event = match raw.source_event.as_ref() {
    Some(source_event) => source_event,
    None => return Ok(None),
  };

  let gamepad = match source_event.gamepad.as_ref() {
    Some(gamepad) => gamepad,
    None => return Ok(None),
  };

  let source = match (&gamepad.button, &gamepad.axis, &gamepad.trigger) {
    (Some(button), _, _) => Source::Button(button.0.clone()),
    (_, Some(axis), _) => Source::Axis {
      name: axis.name.0.clone(),
      direction: axis.direction.as_ref().map(|direction| direction.0.clone()),
      deadzone: axis.deadzone,
    },
    (_, _, Some(trigger)) => Source::Trigger {
      name: trigger.name.0.clone(),
      deadzone: trigger.deadzone,
    },
    _ => return Ok(None),
  };

  let mut targets = Vec::new();

  let told = match raw.target_events.as_ref() {
    Some(told) => told.0.as_slice(),
    None => &[],
  };

  for raw_2 in told {
    let target = read_target(raw_2)?;

    match target {
      Some(target_2) => targets.push(target_2),
      None => {}
    }
  }

  let label = match raw.name.as_ref() {
    Some(label) => label.0.clone(),
    None => String::new(),
  };

  Ok(Some(Mapping {
    label,
    source,
    targets,
  }))
}

#[cfg(feature = "read")]
fn read_target(raw: &RawTargetEvent) -> Result<Option<Target>, Never> {
  let named = |kind, name: &str| {
    Some(Target {
      kind,
      name: name.to_string(),
    })
  };

  Ok(match (&raw.keyboard, &raw.mouse, &raw.gamepad) {
    (Some(key), _, _) => named(Kind::Key, &key.0),
    (
      _,
      Some(RawMouse {
        button: Some(button),
        ..
      }),
      _,
    ) => named(Kind::MouseButton, &button.0),
    (
      _,
      Some(RawMouse {
        motion: Some(_), ..
      }),
      _,
    ) => named(Kind::MouseMotion, "Motion"),
    (
      _,
      _,
      Some(RawGamepad {
        button: Some(button),
        ..
      }),
    ) => named(Kind::GamepadButton, &button.0),
    (
      _,
      _,
      Some(RawGamepad {
        axis: Some(axis), ..
      }),
    ) => named(Kind::GamepadAxis, &axis.name.0),
    (
      _,
      _,
      Some(RawGamepad {
        trigger: Some(trigger),
        ..
      }),
    ) => named(Kind::GamepadTrigger, &trigger.name.0),
    _ => None,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  const SAID: &str = "
name: Desktop
description: >
  One controller map for the whole desktop.
target_devices:
  - mouse
  - keyboard
  - xbox-elite
mapping:
  - name: A - click
    source_event:
      gamepad:
        button: South
    target_events:
      - mouse:
          button: Left
  - name: Left stick - move the pointer
    source_event:
      gamepad:
        axis:
          name: LeftStick
    target_events:
      - mouse:
          motion:
            speed_pps: 900
  - name: R2 - forward a page
    source_event:
      gamepad:
        trigger:
          name: RightTrigger
          deadzone: 0.3
    target_events:
      - keyboard: KeyPageDown
";

  fn read() -> Result<Profile, GamepadError> {
    Profile::read(Path::new("desktop.yaml"), SAID)
  }

  #[test]
  fn a_button_says_what_it_turns_into() -> Result<(), GamepadError> {
    let profile = read()?;
    let a = profile.targets_of("a")?;
    let b = profile.targets_of("b")?;

    assert_eq!(
      a,
      [&Target {
        kind: Kind::MouseButton,
        name: "Left".to_string()
      }]
    );
    assert!(b.is_empty());

    Ok(())
  }

  #[test]
  fn a_profile_can_mix_gamepad_mappings_with_other_v1_events() -> Result<(), GamepadError> {
    let yaml = "
version: 1
kind: DeviceProfile
mapping:
  - name: A
    source_event:
      gamepad: {button: South}
    target_events:
      - dbus: ui_accept
      - mouse: {wheel: {direction: up}}
      - keyboard: KeyEnter
  - name: Touch
    source_event:
      touchpad: {name: LeftPad, touch: {button: Touch}}
    target_events:
      - keyboard: KeySpace
  - name: Motion
    source_event:
      gyroscope: {name: Gyro1}
    target_events:
      - mouse: {motion: {speed_pps: 800}}
";
    let path = Path::new("desktop.yaml");
    let profile = Profile::read(path, yaml)?;
    let targets = profile.targets_of("a")?;

    assert_eq!(profile.name, "desktop");
    assert_eq!(profile.mappings.len(), 1);
    assert_eq!(
      targets,
      [&Target {
        kind: Kind::Key,
        name: "KeyEnter".to_owned(),
      }]
    );
    let misspelled = yaml.replace("keyboard: KeyEnter", "keybord: KeyEnter");

    assert!(matches!(
      Profile::read(path, &misspelled),
      Err(GamepadError::Parse(_, _))
    ));

    Ok(())
  }

  #[test]
  fn a_profile_name_can_fall_back_only_to_a_utf8_stem() -> Result<(), GamepadError> {
    use std::os::unix::ffi::OsStrExt;

    let path = Path::new(std::ffi::OsStr::from_bytes(b"bad\xff.yaml"));
    let named = Profile::read(path, "name: Game")?;

    assert_eq!(named.name, "Game");
    assert!(matches!(
      named.stem(),
      Err(GamepadError::InvalidProfileName(_))
    ));
    assert!(matches!(
      Profile::read(path, "{}"),
      Err(GamepadError::InvalidProfileName(_))
    ));

    Ok(())
  }

  #[test]
  fn a_mapping_says_what_it_does_in_words() -> Result<(), GamepadError> {
    let profile = read()?;
    let does: Vec<&str> = profile
      .mappings
      .iter()
      .take(2)
      .map(|mapping| {
        let Ok(does) = mapping.does();

        does
      })
      .collect();

    assert_eq!(does, ["click", "move the pointer"]);

    Ok(())
  }

  #[test]
  fn a_label_that_does_not_finish_the_sentence_says_nothing() {
    let mapping = Mapping {
      label: "A".to_string(),
      source: Source::Button("South".to_string()),
      targets: vec![],
    };
    assert_eq!(mapping.does(), Ok(""));
  }

  #[test]
  fn what_the_profile_does_not_publish_cannot_be_reached() -> Result<(), GamepadError> {
    let profile = read()?;

    assert_eq!(profile.publishes("mouse"), Ok(Has::Yes));
    assert_eq!(profile.publishes("touchpad"), Ok(Has::No));

    Ok(())
  }

  #[test]
  fn a_stick_and_a_trigger_are_read_as_what_they_are() -> Result<(), GamepadError> {
    let profile = read()?;
    let stick = profile
      .mappings
      .get(1)
      .ok_or(GamepadError::NotFound("a second mapping"))?;
    let trigger = profile
      .mappings
      .get(2)
      .ok_or(GamepadError::NotFound("a third mapping"))?;

    assert_eq!(
      stick.source,
      Source::Axis {
        name: "LeftStick".to_string(),
        direction: None,
        deadzone: None
      }
    );
    assert_eq!(
      trigger.source,
      Source::Trigger {
        name: "RightTrigger".to_string(),
        deadzone: Some(0.3)
      }
    );
    assert_eq!(stick.source.button(), Ok(None));

    Ok(())
  }

  #[test]
  fn a_folded_description_arrives_as_one_line() -> Result<(), GamepadError> {
    let profile = read()?;

    assert_eq!(
      profile.description,
      "One controller map for the whole desktop."
    );

    Ok(())
  }

  #[test]
  fn every_kind_knows_which_device_it_needs() {
    assert_eq!(Kind::Key.needs(), Ok("keyboard"));
    assert_eq!(Kind::MouseMotion.needs(), Ok("mouse"));
    assert_eq!(Kind::GamepadTrigger.needs(), Ok("xbox-elite"));
  }
}
