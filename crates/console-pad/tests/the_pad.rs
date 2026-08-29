//! Pressing a button, and what comes out of the devices.
//!
//! Nothing between the two ends is stood in for: the profiles are the ones the
//! device loads, so a test that passes here is a statement about the profile
//! as much as about the emulator.

use std::path::{Path, PathBuf};

use evdev::{EventType, KeyCode};
use console_pad::capture::captured;
use console_pad::devices::Devices;
use console_pad::go::{Held, LegionGo};
use console_pad::profile::{Profile, load_all};
use console_pad::world::{World, Written};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the repository")
}

fn go(profile: &str) -> LegionGo<World, Held> {
    let devices = Devices::new(captured(), World::of(captured()));
    LegionGo::new(load_all(&root()).expect("the profiles"), devices, Held::default(), profile)
        .expect("a pad")
}

/// A profile that publishes only a keyboard, which none of the shipped ones do.
const WITHOUT_A_MOUSE: &str = "
name: Spare
target_devices:
  - keyboard
mapping:
  - name: A - click
    source_event:
      gamepad:
        button: South
    target_events:
      - mouse:
          button: Left
";

fn go_of(yaml: &str, stem: &str) -> LegionGo<World, Held> {
    let path = PathBuf::from(format!("{stem}.yaml"));
    let profile = Profile::read(&path, yaml).expect("a profile");
    let devices = Devices::new(captured(), World::of(captured()));
    LegionGo::new([(stem.to_string(), profile)].into(), devices, Held::default(), stem)
        .expect("a pad")
}

fn keys(go: &LegionGo<World, Held>, role: &str) -> Vec<(u16, i32)> {
    go.devices
        .sink
        .of_kind(role, EventType::KEY, None)
        .iter()
        .map(|written| (written.code, written.value))
        .collect()
}

#[test]
fn a_press_becomes_what_the_profile_says_it_is() {
    let mut pad = go("desktop");
    pad.press("a").expect("a");
    assert_eq!(keys(&pad, "mouse"), [(KeyCode::BTN_LEFT.0, 1), (KeyCode::BTN_LEFT.0, 0)]);
    assert!(keys(&pad, "pad").is_empty(), "a mapped button does not also reach the pad");
}

#[test]
fn the_same_press_means_something_else_under_another_profile() {
    let mut chooser = go("tabs");
    chooser.press("a").expect("a");
    let under_a_chooser = keys(&chooser, "keyboard");
    assert!(!under_a_chooser.is_empty(), "a does something while a chooser is up");
    assert!(keys(&chooser, "mouse").is_empty(), "and it is not a click");
}

/// What an empty profile means. `keyboard.yaml` has no mappings at all and is
/// documented as passing everything through.
#[test]
fn a_button_with_no_mapping_reaches_the_pad_as_itself() {
    let mut pad = go("keyboard");
    pad.press("a").expect("a");
    assert_eq!(keys(&pad, "pad"), [(KeyCode::BTN_SOUTH.0, 1), (KeyCode::BTN_SOUTH.0, 0)]);
}

/// InputPlumber builds the targets a profile names and destroys the rest, so a
/// press that would reach a device the profile does not publish reaches
/// nothing at all.
///
/// Written here rather than read out of the checkout, because every profile
/// this desktop ships publishes all three and a rule cannot be shown by a case
/// that never happens.
#[test]
fn nothing_reaches_a_device_the_profile_does_not_publish() {
    let mut pad = go_of(WITHOUT_A_MOUSE, "spare");
    assert!(!pad.profile().publishes("mouse"));
    pad.press("a").expect("a");
    assert!(keys(&pad, "mouse").is_empty());
}

#[test]
fn holding_is_held_until_it_is_let_go() {
    let mut pad = go("desktop");
    pad.hold("l1").expect("l1");
    assert_eq!(pad.holding(), ["l1"]);
    pad.release_all().expect("let go");
    assert!(pad.holding().is_empty());
    let said = keys(&pad, "keyboard");
    assert_eq!(said.len() % 2, 0, "what went down came up");
}

/// Saying "hold l2" is what a person means, so it is what it does: a trigger
/// is an axis, and holding one is pulling it all the way.
#[test]
fn holding_a_trigger_pulls_it_all_the_way() {
    let mut pad = go("keyboard");
    pad.hold("l2").expect("l2");
    let pulled = pad.devices.sink.of_kind("pad", EventType::ABSOLUTE, Some(2));
    let range = pad.devices.axis("pad", 2).expect("ABS_Z");
    assert_eq!(pulled.last().map(|written| written.value), Some(range.max));
}

#[test]
fn a_stick_is_one_frame_of_two_numbers() {
    let mut pad = go("keyboard");
    pad.stick("left-stick", 1.0, -1.0).expect("a push");
    let span = pad.devices.axis("pad", 0).expect("ABS_X").span();
    let pushed = pad.devices.sink.of_kind("pad", EventType::ABSOLUTE, None);
    assert_eq!(
        pushed,
        [
            Written { kind: EventType::ABSOLUTE, code: 0, value: span },
            Written { kind: EventType::ABSOLUTE, code: 1, value: -span },
        ]
    );
}

#[test]
fn a_stick_only_moves_where_the_profile_publishes_a_pad() {
    let mut pad = go_of(WITHOUT_A_MOUSE, "spare");
    assert!(!pad.profile().publishes("xbox-elite"));
    pad.stick("left-stick", 1.0, 0.0).expect("a push");
    assert!(pad.devices.sink.of_kind("pad", EventType::ABSOLUTE, None).is_empty());
}

/// InputPlumber cannot translate the touchpad and the compositor makes it
/// absolute, so on the device it is left alone and read directly. It is left
/// alone here too: no profile is in the way.
#[test]
fn the_touchpad_is_not_in_the_profile_loop_at_all() {
    let mut pad = go("tabs");
    pad.tap(300, 400);
    let touched = pad.devices.sink.written("touchpad");
    assert_eq!(touched.first().map(|w| (w.kind, w.code, w.value)), Some((EventType::KEY, KeyCode::BTN_TOUCH.0, 1)));
    assert!(touched.iter().any(|w| w.kind == EventType::ABSOLUTE && w.value == 300));
}

#[test]
fn a_drag_reports_every_step_of_the_way() {
    let mut pad = go("desktop");
    pad.drag((0, 0), (80, 0), 8, 0.0);
    let along: Vec<i32> = pad
        .devices
        .sink
        .of_kind("touchpad", EventType::ABSOLUTE, Some(0))
        .iter()
        .map(|written| written.value)
        .collect();
    assert_eq!(along, [0, 10, 20, 30, 40, 50, 60, 70, 80]);
}

#[test]
fn a_profile_nothing_has_says_which_there_are() {
    let mut pad = go("desktop");
    let fault = pad.load_profile("gaming").expect_err("no such profile");
    assert!(fault.contains("desktop") && fault.contains("gaming"), "{fault}");
}

#[test]
fn a_button_nothing_is_called_says_so_rather_than_pressing_something_else() {
    let mut pad = go("desktop");
    assert!(pad.press("triangle").is_err());
}

/// What the capture writes is what the capture reads.
///
/// The fixture in the tree was written by `capture-devices` on the device. If
/// writing it again here does not come out as the same bytes, then a capture
/// taken tomorrow is a diff of key order rather than a diff of the machine, and
/// nobody would be able to tell the two apart.
#[test]
fn a_capture_written_again_is_the_file_that_is_kept() {
    let held = console_pad::capture::CAPTURED;
    let read: Vec<console_pad::capture::Descriptor> =
        serde_json::from_str(held).expect("the capture reads");
    let written = serde_json::to_string_pretty(&read).expect("the capture writes");
    assert_eq!(format!("{written}\n"), held);
}
