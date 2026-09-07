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
//! Not hand-rolled instead, and this is the part to argue with rather than the
//! feature: InputPlumber parses these same files, and a reader of our own that
//! accepted a subset of what it accepts would be the read-it-twice fault this
//! header opens by refusing. A profile would pass here and mean something else
//! on the machine. Behind a feature it is the same parser or nothing.

#[cfg(feature = "read")]
use std::collections::BTreeMap;
#[cfg(feature = "read")]
use std::path::Path;
use std::path::PathBuf;

use console_core_never::Never;
use evdev::KeyCode;
#[cfg(feature = "read")]
use serde::Deserialize;

use crate::devices::Has;
use crate::vocabulary;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Key,
    MouseButton,
    MouseMotion,
    GamepadButton,
    GamepadAxis,
    GamepadTrigger,
}

impl Kind {
    pub fn said(self) -> Result<&'static str, Never> {
        Ok(match self {
            Kind::Key => "key",
            Kind::MouseButton => "mouse-button",
            Kind::MouseMotion => "mouse-motion",
            Kind::GamepadButton => "gamepad-button",
            Kind::GamepadAxis => "gamepad-axis",
            Kind::GamepadTrigger => "gamepad-trigger",
        })
    }

    pub fn needs(self) -> Result<&'static str, Never> {
        Ok(match self {
            Kind::Key => "keyboard",
            Kind::MouseButton | Kind::MouseMotion => "mouse",
            Kind::GamepadButton | Kind::GamepadAxis | Kind::GamepadTrigger => "xbox-elite",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Target {
    pub kind: Kind,
    pub name: String,
}

impl Target {
    pub fn code(&self) -> Result<KeyCode, String> {
        match self.kind {
            Kind::Key => vocabulary::key_code(&self.name),
            Kind::MouseButton => {
                let Ok(code) = vocabulary::mouse_code(&self.name);

                code.ok_or_else(|| format!("no mouse button called {:?}", self.name))
            }
            Kind::GamepadButton => {
                let Ok(code) = vocabulary::gamepad_code(&self.name);

                code.ok_or_else(|| format!("no pad button called {:?}", self.name))
            }
            Kind::MouseMotion | Kind::GamepadAxis | Kind::GamepadTrigger => Err(format!(
                "{:?} does not arrive as one code: {:?}",
                self.kind, self.name
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Button(String),
    Axis { name: String, direction: Option<String>, deadzone: Option<f64> },
    Trigger { name: String, deadzone: Option<f64> },
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
        Ok(self.label.split_once(" - ").map_or("", |(_, does)| does).trim())
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
    pub fn read(path: &Path, yaml: &str) -> Result<Self, String> {
        let raw: Raw = serde_yaml_ng::from_str(yaml)
            .map_err(|fault| format!("{} does not parse: {fault}", path.display()))?;
        let stem = path.file_stem().map_or(String::new(), |s| s.to_string_lossy().to_string());

        let mut mappings = Vec::new();

        for raw in &raw.mapping.unwrap_or_default() {
            let Ok(mapping) = read_mapping(raw);

            match mapping {
                Some(mapping) => mappings.push(mapping),
                None => {},
            }
        }

        Ok(Profile {
            path: path.to_path_buf(),
            name: raw.name.unwrap_or(stem),
            description: raw.description.unwrap_or_default().trim().to_string(),
            target_devices: raw.target_devices.unwrap_or_default(),
            mappings,
        })
    }
}

impl Profile {
    pub fn publishes(&self, target_device: &str) -> Result<Has, Never> {
        Ok(match self.target_devices.iter().any(|named| named == target_device) {
            true => Has::Yes,
            false => Has::No,
        })
    }

    pub fn for_button(&self, spoken: &str) -> Result<Vec<&Mapping>, String> {
        let name = vocabulary::button_name(spoken)?;
        Ok(self
            .mappings
            .iter()
            .filter(|mapping| matches!(&mapping.source, Source::Button(said) if said == name))
            .collect())
    }

    pub fn targets_of(&self, spoken: &str) -> Result<Vec<&Target>, String> {
        let mappings = self.for_button(spoken)?;

        Ok(mappings.iter().flat_map(|mapping| &mapping.targets).collect())
    }

    pub fn stem(&self) -> Result<String, Never> {
        Ok(self.path.file_stem().map_or(String::new(), |s| s.to_string_lossy().to_string()))
    }
}

#[cfg(feature = "read")]
pub const PROFILE_DIR: &str = "files/etc/inputplumber/profiles";

#[cfg(feature = "read")]
pub fn load_all(root: &Path) -> Result<BTreeMap<String, Profile>, String> {
    let holding = root.join(PROFILE_DIR);
    let listed = std::fs::read_dir(&holding)
        .map_err(|fault| format!("{} could not be read: {fault}", holding.display()))?;
    let mut found: Vec<PathBuf> = listed
        .filter_map(|entry| match entry {
            Ok(e) => Some(e.path()),
            Err(_) => None,
        })
        .filter(|path| path.extension().is_some_and(|kind| kind == "yaml"))
        .collect();
    found.sort();
    found
        .iter()
        .map(|path| {
            let yaml = std::fs::read_to_string(path)
                .map_err(|fault| format!("{} could not be read: {fault}", path.display()))?;
            let profile = Profile::read(path, &yaml)?;

            let Ok(stem) = profile.stem();

            Ok((stem, profile))
        })
        .collect()
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
struct Raw {
    name: Option<String>,
    description: Option<String>,
    target_devices: Option<Vec<String>>,
    mapping: Option<Vec<RawMapping>>,
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
struct RawMapping {
    name: Option<String>,
    source_event: Option<RawSourceEvent>,
    target_events: Option<Vec<RawTargetEvent>>,
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
struct RawSourceEvent {
    gamepad: Option<RawGamepadSource>,
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
struct RawGamepadSource {
    button: Option<String>,
    axis: Option<RawAxis>,
    trigger: Option<RawTrigger>,
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
struct RawAxis {
    name: String,
    direction: Option<String>,
    deadzone: Option<f64>,
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
struct RawTrigger {
    name: String,
    deadzone: Option<f64>,
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
struct RawTargetEvent {
    keyboard: Option<String>,
    mouse: Option<RawMouse>,
    gamepad: Option<RawGamepadTarget>,
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
struct RawMouse {
    button: Option<String>,
    motion: Option<serde_yaml_ng::Value>,
}

#[cfg(feature = "read")]
#[derive(Deserialize)]
struct RawGamepadTarget {
    button: Option<String>,
    axis: Option<RawAxis>,
    trigger: Option<RawTrigger>,
}

#[cfg(feature = "read")]
fn read_mapping(raw: &RawMapping) -> Result<Option<Mapping>, Never> {
    let Some(source_event) = raw.source_event.as_ref() else { return Ok(None) };

    let Some(gamepad) = source_event.gamepad.as_ref() else { return Ok(None) };

    let source = match (&gamepad.button, &gamepad.axis, &gamepad.trigger) {
        (Some(button), _, _) => Source::Button(button.clone()),
        (_, Some(axis), _) => Source::Axis {
            name: axis.name.clone(),
            direction: axis.direction.clone(),
            deadzone: axis.deadzone,
        },
        (_, _, Some(trigger)) => {
            Source::Trigger { name: trigger.name.clone(), deadzone: trigger.deadzone }
        }
        _ => return Ok(None),
    };

    let mut targets = Vec::new();

    for raw in raw.target_events.as_deref().unwrap_or_default() {
        let target = read_target(raw)?;

        match target {
            Some(target) => targets.push(target),
            None => {},
        }
    }

    Ok(Some(Mapping { label: raw.name.clone().unwrap_or_default(), source, targets }))
}

#[cfg(feature = "read")]
fn read_target(raw: &RawTargetEvent) -> Result<Option<Target>, Never> {
    let named = |kind, name: &str| Some(Target { kind, name: name.to_string() });

    Ok(match (&raw.keyboard, &raw.mouse, &raw.gamepad) {
        (Some(key), _, _) => named(Kind::Key, key),
        (_, Some(RawMouse { button: Some(button), .. }), _) => named(Kind::MouseButton, button),
        (_, Some(RawMouse { motion: Some(_), .. }), _) => named(Kind::MouseMotion, "Motion"),
        (_, _, Some(RawGamepadTarget { button: Some(button), .. })) => {
            named(Kind::GamepadButton, button)
        }
        (_, _, Some(RawGamepadTarget { axis: Some(axis), .. })) => named(Kind::GamepadAxis, &axis.name),
        (_, _, Some(RawGamepadTarget { trigger: Some(trigger), .. })) => {
            named(Kind::GamepadTrigger, &trigger.name)
        }
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

    fn read() -> Profile {
        Profile::read(Path::new("desktop.yaml"), SAID).expect("a profile")
    }

    #[test]
    fn a_button_says_what_it_turns_into() {
        let profile = read();
        assert_eq!(
            profile.targets_of("a").expect("a"),
            [&Target { kind: Kind::MouseButton, name: "Left".to_string() }]
        );
        assert_eq!(profile.targets_of("b").expect("b"), [] as [&Target; 0]);
    }

    #[test]
    fn a_mapping_says_what_it_does_in_words() {
        assert_eq!(read().mappings[0].does(), Ok("click"));
        assert_eq!(read().mappings[1].does(), Ok("move the pointer"));
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
    fn what_the_profile_does_not_publish_cannot_be_reached() {
        let profile = read();
        assert_eq!(profile.publishes("mouse"), Ok(Has::Yes));
        assert_eq!(profile.publishes("touchpad"), Ok(Has::No));
    }

    #[test]
    fn a_stick_and_a_trigger_are_read_as_what_they_are() {
        let profile = read();
        assert_eq!(
            profile.mappings[1].source,
            Source::Axis { name: "LeftStick".to_string(), direction: None, deadzone: None }
        );
        assert_eq!(
            profile.mappings[2].source,
            Source::Trigger { name: "RightTrigger".to_string(), deadzone: Some(0.3) }
        );
        assert_eq!(profile.mappings[1].source.button(), Ok(None));
    }

    #[test]
    fn a_folded_description_arrives_as_one_line() {
        assert_eq!(read().description, "One controller map for the whole desktop.");
    }

    #[test]
    fn every_kind_knows_which_device_it_needs() {
        assert_eq!(Kind::Key.needs(), Ok("keyboard"));
        assert_eq!(Kind::MouseMotion.needs(), Ok("mouse"));
        assert_eq!(Kind::GamepadTrigger.needs(), Ok("xbox-elite"));
    }
}
