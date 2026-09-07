//! Pressing a button, and what comes out of the devices.
//!
//! Nothing between the two ends is stood in for: the profiles are the ones the
//! device loads, so a test that passes here is a statement about the profile
//! as much as about the emulator.

use std::path::{Path, PathBuf};

use evdev::{EventType, KeyCode};
use console_input_gamepad::capture::captured;
use console_input_gamepad::devices::{Devices, Has};
use console_input_gamepad::go::{Held, LegionGo};
use console_input_gamepad::profile::Profile;
use console_input_gamepad::router::every_profile;
use console_input_gamepad::world::{World, Written};

fn ok<T>(answer: Result<T, console_core_never::Never>) -> T {
    let Ok(value) = answer;

    value
}

fn root() -> PathBuf {
    {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
}

fn go(profile: &str) -> LegionGo<World, Held> {
    let devices = ok(Devices::new(captured().expect("the capture"), ok(World::of(captured().expect("the capture")))));
    LegionGo::new(every_profile(&root()).expect("the profiles"), devices, Held::default(), profile)
        .expect("a pad")
}

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
    let devices = ok(Devices::new(captured().expect("the capture"), ok(World::of(captured().expect("the capture")))));
    LegionGo::new([(stem.to_string(), profile)].into(), devices, Held::default(), stem)
        .expect("a pad")
}

fn keys(go: &LegionGo<World, Held>, role: &str) -> Vec<(u16, i32)> {
    ok(go.devices.sink.of_kind(role, EventType::KEY, None))
        .iter()
        .map(|written| (written.code, written.value))
        .collect()
}

#[test]
fn a_press_becomes_what_the_profile_says_it_is() {
    let mut pad = go(console_input_gamepad::router::NAME);
    pad.press("a").expect("a");
    assert_eq!(keys(&pad, "pad"), [(KeyCode::BTN_SOUTH.0, 1), (KeyCode::BTN_SOUTH.0, 0)]);
    assert!(keys(&pad, "mouse").is_empty(), "the click is not the profile's to send");
}

#[test]
fn the_same_press_means_something_else_under_another_profile() {
    let mut routed = go(console_input_gamepad::router::NAME);
    routed.press("right-paddle-top").expect("a paddle");
    assert_eq!(keys(&routed, "keyboard"), [(KeyCode::KEY_F15.0, 1), (KeyCode::KEY_F15.0, 0)]);
    assert!(keys(&routed, "pad").is_empty(), "a routed button does not also reach the pad");

    let mut passed = go("game");
    passed.press("right-paddle-top").expect("a paddle");
    assert!(keys(&passed, "keyboard").is_empty(), "nothing is translated under Game Mode's");
}

#[test]
fn a_button_with_no_mapping_reaches_the_pad_as_itself() {
    let mut pad = go("game");
    pad.press("a").expect("a");
    assert_eq!(keys(&pad, "pad"), [(KeyCode::BTN_SOUTH.0, 1), (KeyCode::BTN_SOUTH.0, 0)]);
}

#[test]
fn nothing_reaches_a_device_the_profile_does_not_publish() {
    let mut pad = go_of(WITHOUT_A_MOUSE, "spare");
    assert_eq!(ok(pad.profile()).publishes("mouse"), Ok(Has::No));
    pad.press("a").expect("a");
    assert!(keys(&pad, "mouse").is_empty());
}

#[test]
fn holding_is_held_until_it_is_let_go() {
    let mut pad = go(console_input_gamepad::router::NAME);
    pad.hold("l1").expect("l1");
    assert_eq!(ok(pad.holding()), ["l1"]);
    pad.release_all().expect("let go");
    assert!(ok(pad.holding()).is_empty());
    let said = keys(&pad, "keyboard");
    assert_eq!(said.len() % 2, 0, "what went down came up");
}

#[test]
fn holding_a_trigger_pulls_it_all_the_way() {
    let mut pad = go("game");
    pad.hold("l2").expect("l2");
    let pulled = ok(pad.devices.sink.of_kind("pad", EventType::ABSOLUTE, Some(2)));
    let range = pad.devices.axis("pad", 2).expect("ABS_Z");
    assert_eq!(pulled.last().map(|written| written.value), Some(range.max));
}

#[test]
fn a_stick_is_one_frame_of_two_numbers() {
    let mut pad = go("game");
    pad.stick("left-stick", 1.0, -1.0).expect("a push");
    let span = ok(pad.devices.axis("pad", 0).expect("ABS_X").span());
    let pushed = ok(pad.devices.sink.of_kind("pad", EventType::ABSOLUTE, None));
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
    assert_eq!(ok(pad.profile()).publishes("xbox-elite"), Ok(Has::No));
    pad.stick("left-stick", 1.0, 0.0).expect("a push");
    assert!(ok(pad.devices.sink.of_kind("pad", EventType::ABSOLUTE, None)).is_empty());
}

#[test]
fn the_touchpad_is_not_in_the_profile_loop_at_all() {
    let mut pad = go(console_input_gamepad::router::NAME);
    ok(pad.tap(300, 400));
    let touched = ok(pad.devices.sink.written("touchpad"));
    assert_eq!(touched.first().map(|w| (w.kind, w.code, w.value)), Some((EventType::KEY, KeyCode::BTN_TOUCH.0, 1)));
    assert!(touched.iter().any(|w| w.kind == EventType::ABSOLUTE && w.value == 300));
}

#[test]
fn a_drag_reports_every_step_of_the_way() {
    let mut pad = go(console_input_gamepad::router::NAME);
    ok(pad.drag((0, 0), (80, 0), 8, 0.0));
    let along: Vec<i32> = ok(pad.devices.sink.of_kind("touchpad", EventType::ABSOLUTE, Some(0)))
        .iter()
        .map(|written| written.value)
        .collect();
    assert_eq!(along, [0, 10, 20, 30, 40, 50, 60, 70, 80]);
}

#[test]
fn a_profile_nothing_has_says_which_there_are() {
    let mut pad = go(console_input_gamepad::router::NAME);
    let fault = pad.load_profile("gaming").expect_err("no such profile");
    assert!(fault.contains("router") && fault.contains("gaming"), "{fault}");
}

#[test]
fn a_button_nothing_is_called_says_so_rather_than_pressing_something_else() {
    let mut pad = go(console_input_gamepad::router::NAME);
    assert!(pad.press("triangle").is_err());
}

#[test]
fn a_capture_written_again_is_the_file_that_is_kept() {
    let held = console_input_gamepad::capture::CAPTURED;
    let read: Vec<console_input_gamepad::capture::Descriptor> =
        serde_json::from_str(held).expect("the capture reads");
    let written = serde_json::to_string_pretty(&read).expect("the capture writes");
    assert_eq!(format!("{written}\n"), held);
}
