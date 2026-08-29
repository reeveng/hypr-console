//! An InputPlumber profile, read as what each button turns into.
//!
//! A profile is the whole of what a button means: the compositor is not in the
//! loop and the daemons only see what came out of here. So a change to what
//! the device does is a change to one of these files, and anything that wants
//! to know what the device does, the guide and the tests included, reads them
//! rather than being told twice.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use evdev::KeyCode;
use serde::Deserialize;

use crate::vocabulary;

/// One thing a press turns into.
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
    pub fn said(self) -> &'static str {
        match self {
            Kind::Key => "key",
            Kind::MouseButton => "mouse-button",
            Kind::MouseMotion => "mouse-motion",
            Kind::GamepadButton => "gamepad-button",
            Kind::GamepadAxis => "gamepad-axis",
            Kind::GamepadTrigger => "gamepad-trigger",
        }
    }

    /// Which target device a profile has to list before this can reach it.
    ///
    /// InputPlumber builds the targets a profile names and destroys the rest,
    /// so a mapping that sends a pad button from a profile with no pad in it
    /// sends it nowhere.
    pub fn needs(self) -> &'static str {
        match self {
            Kind::Key => "keyboard",
            Kind::MouseButton | Kind::MouseMotion => "mouse",
            Kind::GamepadButton | Kind::GamepadAxis | Kind::GamepadTrigger => "xbox-elite",
        }
    }
}

/// A key, a mouse button, a pad button: one end of a mapping.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Target {
    pub kind: Kind,
    pub name: String,
}

impl Target {
    /// The kernel code this arrives as, where there is one.
    pub fn code(&self) -> Option<KeyCode> {
        match self.kind {
            Kind::Key => vocabulary::key_code(&self.name).ok(),
            Kind::MouseButton => vocabulary::mouse_code(&self.name),
            Kind::GamepadButton => vocabulary::gamepad_code(&self.name),
            _ => None,
        }
    }
}

/// What was pressed, pushed or pulled.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Button(String),
    Axis { name: String, direction: Option<String>, deadzone: Option<f64> },
    Trigger { name: String, deadzone: Option<f64> },
}

impl Source {
    /// What is written on the button, where the source is one.
    pub fn button(&self) -> Option<&str> {
        match self {
            Source::Button(name) => Some(vocabulary::spoken_for(name)),
            _ => None,
        }
    }
}

/// One entry: what was pressed, what it becomes, and what it is called.
#[derive(Debug, Clone, PartialEq)]
pub struct Mapping {
    pub label: String,
    pub source: Source,
    pub targets: Vec<Target>,
}

impl Mapping {
    /// The half of the label after the dash: what it does, in words.
    ///
    /// Every mapping is named "Button - what it does", and the guide on the
    /// device prints exactly this. A mapping that says nothing about what it
    /// does reads as nothing rather than being quietly dropped.
    pub fn does(&self) -> &str {
        self.label.split_once(" - ").map_or("", |(_, does)| does).trim()
    }
}

/// What the pad is, while this profile is loaded.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub path: PathBuf,
    pub name: String,
    pub description: String,
    pub target_devices: Vec<String>,
    pub mappings: Vec<Mapping>,
}

impl Profile {
    /// One profile, out of the file it is written in.
    pub fn read(path: &Path, yaml: &str) -> Result<Self, String> {
        let raw: Raw = serde_yaml_ng::from_str(yaml)
            .map_err(|fault| format!("{} does not parse: {fault}", path.display()))?;
        let stem = path.file_stem().map_or(String::new(), |s| s.to_string_lossy().to_string());
        Ok(Profile {
            path: path.to_path_buf(),
            name: raw.name.unwrap_or(stem),
            description: raw.description.unwrap_or_default().trim().to_string(),
            target_devices: raw.target_devices.unwrap_or_default(),
            mappings: raw.mapping.unwrap_or_default().iter().filter_map(read_mapping).collect(),
        })
    }

    /// Whether an event can reach a device at all here.
    pub fn publishes(&self, target_device: &str) -> bool {
        self.target_devices.iter().any(|named| named == target_device)
    }

    /// Every mapping a named button has here, usually none or one.
    pub fn for_button(&self, spoken: &str) -> Result<Vec<&Mapping>, String> {
        let name = vocabulary::button_name(spoken)?;
        Ok(self
            .mappings
            .iter()
            .filter(|mapping| matches!(&mapping.source, Source::Button(said) if said == name))
            .collect())
    }

    /// What pressing that button turns into here.
    pub fn targets_of(&self, spoken: &str) -> Result<Vec<&Target>, String> {
        Ok(self.for_button(spoken)?.iter().flat_map(|mapping| &mapping.targets).collect())
    }

    /// The word `controller-profile` takes for this one.
    pub fn stem(&self) -> String {
        self.path.file_stem().map_or(String::new(), |s| s.to_string_lossy().to_string())
    }
}

/// Where the profiles live in a checkout.
pub const PROFILE_DIR: &str = "files/etc/inputplumber/profiles";

/// Every profile in a checkout, by the word `controller-profile` takes.
pub fn load_all(root: &Path) -> Result<BTreeMap<String, Profile>, String> {
    let holding = root.join(PROFILE_DIR);
    let mut found: Vec<PathBuf> = std::fs::read_dir(&holding)
        .map_err(|fault| format!("{} could not be read: {fault}", holding.display()))?
        .filter_map(|entry| entry.ok().map(|found| found.path()))
        .filter(|path| path.extension().is_some_and(|kind| kind == "yaml"))
        .collect();
    found.sort();
    found
        .iter()
        .map(|path| {
            let yaml = std::fs::read_to_string(path)
                .map_err(|fault| format!("{} could not be read: {fault}", path.display()))?;
            let profile = Profile::read(path, &yaml)?;
            Ok((profile.stem(), profile))
        })
        .collect()
}

// -------------------------------------------------------------- as it is written

#[derive(Deserialize)]
struct Raw {
    name: Option<String>,
    description: Option<String>,
    target_devices: Option<Vec<String>>,
    mapping: Option<Vec<RawMapping>>,
}

#[derive(Deserialize)]
struct RawMapping {
    name: Option<String>,
    source_event: Option<RawSourceEvent>,
    target_events: Option<Vec<RawTargetEvent>>,
}

#[derive(Deserialize)]
struct RawSourceEvent {
    gamepad: Option<RawGamepadSource>,
}

#[derive(Deserialize)]
struct RawGamepadSource {
    button: Option<String>,
    axis: Option<RawAxis>,
    trigger: Option<RawTrigger>,
}

#[derive(Deserialize)]
struct RawAxis {
    name: String,
    direction: Option<String>,
    deadzone: Option<f64>,
}

#[derive(Deserialize)]
struct RawTrigger {
    name: String,
    deadzone: Option<f64>,
}

#[derive(Deserialize)]
struct RawTargetEvent {
    keyboard: Option<String>,
    mouse: Option<RawMouse>,
    gamepad: Option<RawGamepadTarget>,
}

#[derive(Deserialize)]
struct RawMouse {
    button: Option<String>,
    motion: Option<serde_yaml_ng::Value>,
}

#[derive(Deserialize)]
struct RawGamepadTarget {
    button: Option<String>,
    axis: Option<RawAxis>,
    trigger: Option<RawTrigger>,
}

/// One entry, if it says what was pressed. An entry that names no source is
/// not a mapping and is dropped, the way InputPlumber drops it.
fn read_mapping(raw: &RawMapping) -> Option<Mapping> {
    let gamepad = raw.source_event.as_ref()?.gamepad.as_ref()?;
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
        _ => return None,
    };
    Some(Mapping {
        label: raw.name.clone().unwrap_or_default(),
        source,
        targets: raw.target_events.as_deref().unwrap_or_default().iter().filter_map(read_target).collect(),
    })
}

fn read_target(raw: &RawTargetEvent) -> Option<Target> {
    let named = |kind, name: &str| Some(Target { kind, name: name.to_string() });
    match (&raw.keyboard, &raw.mouse, &raw.gamepad) {
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
    }
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
        assert_eq!(read().mappings[0].does(), "click");
        assert_eq!(read().mappings[1].does(), "move the pointer");
    }

    #[test]
    fn a_label_that_does_not_finish_the_sentence_says_nothing() {
        let mapping = Mapping {
            label: "A".to_string(),
            source: Source::Button("South".to_string()),
            targets: vec![],
        };
        assert_eq!(mapping.does(), "");
    }

    #[test]
    fn what_the_profile_does_not_publish_cannot_be_reached() {
        let profile = read();
        assert!(profile.publishes("mouse"));
        assert!(!profile.publishes("touchpad"));
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
        assert_eq!(profile.mappings[1].source.button(), None);
    }

    #[test]
    fn a_folded_description_arrives_as_one_line() {
        assert_eq!(read().description, "One controller map for the whole desktop.");
    }

    #[test]
    fn every_kind_knows_which_device_it_needs() {
        assert_eq!(Kind::Key.needs(), "keyboard");
        assert_eq!(Kind::MouseMotion.needs(), "mouse");
        assert_eq!(Kind::GamepadTrigger.needs(), "xbox-elite");
    }
}
