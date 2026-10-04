//! Pressing a button, and what comes out of the devices.
//!
//! Nothing between the two ends is stood in for: the profiles are the ones the
//! device loads, so a test that passes here is a statement about the profile
//! as much as about the emulator.

use console_core_geometry::Point;
use console_core_never::Never;
use std::collections::BTreeMap;
use std::error::Error;
use std::path::PathBuf;

use console_input_event_devices::{EventType, KeyCode};
use console_input_gamepad::GamepadError;
use console_input_gamepad::capture::load_capture;
use console_input_gamepad::devices::{Devices, Has};
use console_input_gamepad::go::LegionGo;
use console_waiting::clock::TestClock;
use console_input_gamepad::profile::Profile;
use console_input_gamepad::router::every_profile;
use console_input_gamepad::world::{World, Written};

type Pad = LegionGo<World, TestClock>;

fn under(profiles: BTreeMap<String, Profile>, profile: &str) -> Result<Pad, GamepadError> {
    let seen = load_capture()?;
    let descriptors = load_capture()?;
    let Ok(world) = World::of(seen);
    let Ok(devices) = Devices::new(descriptors, world);

    LegionGo::new(profiles, devices, TestClock::default(), profile)
}

fn go(profile: &str) -> Result<Pad, Box<dyn Error>> {
    let root = console_repository::root()?;
    let profiles = every_profile(&root)?;
    let pad = under(profiles, profile)?;

    Ok(pad)
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

fn spare() -> Result<Pad, GamepadError> {
    let profile = Profile::read(&PathBuf::from("spare.yaml"), WITHOUT_A_MOUSE)?;

    under(BTreeMap::from([("spare".to_string(), profile)]), "spare")
}

fn keys(go: &Pad, role: &str) -> Result<Vec<(u16, i32)>, Never> {
    let Ok(written) = go.devices.sink.of_kind(role, EventType::KEY, None);

    Ok(written.iter().map(|written| (written.code, written.value)).collect())
}

#[test]
fn a_press_becomes_what_the_profile_says_it_is() -> Result<(), Box<dyn Error>> {
    let mut pad = go(console_input_gamepad::router::NAME)?;

    pad.press("a")?;

    let Ok(on_the_pad) = keys(&pad, "pad");
    let Ok(on_the_mouse) = keys(&pad, "mouse");

    assert_eq!(on_the_pad, [(KeyCode::BTN_SOUTH.0, 1), (KeyCode::BTN_SOUTH.0, 0)]);
    assert!(on_the_mouse.is_empty(), "the click is not the profile's to send");

    Ok(())
}

#[test]
fn the_same_press_means_something_else_under_another_profile() -> Result<(), Box<dyn Error>> {
    let mut routed = go(console_input_gamepad::router::NAME)?;

    routed.press("right-paddle-top")?;

    let Ok(typed) = keys(&routed, "keyboard");
    let Ok(on_the_pad) = keys(&routed, "pad");

    assert_eq!(typed, [(KeyCode::KEY_F15.0, 1), (KeyCode::KEY_F15.0, 0)]);
    assert!(on_the_pad.is_empty(), "a routed button does not also reach the pad");

    let mut passed = go("game")?;

    passed.press("right-paddle-top")?;

    let Ok(typed) = keys(&passed, "keyboard");

    assert!(typed.is_empty(), "nothing is translated under Game Mode's");

    Ok(())
}

#[test]
fn a_button_with_no_mapping_reaches_the_pad_as_itself() -> Result<(), Box<dyn Error>> {
    let mut pad = go("game")?;

    pad.press("a")?;

    assert_eq!(keys(&pad, "pad"), Ok(vec![(KeyCode::BTN_SOUTH.0, 1), (KeyCode::BTN_SOUTH.0, 0)]));

    Ok(())
}

#[test]
fn nothing_reaches_a_device_the_profile_does_not_publish() -> Result<(), GamepadError> {
    let mut pad = spare()?;
    let Ok(profile) = pad.profile();

    assert_eq!(profile.publishes("mouse"), Ok(Has::No));

    pad.press("a")?;

    assert_eq!(keys(&pad, "mouse"), Ok(Vec::new()));

    Ok(())
}

#[test]
fn holding_is_held_until_it_is_let_go() -> Result<(), Box<dyn Error>> {
    let mut pad = go(console_input_gamepad::router::NAME)?;

    pad.hold("l1")?;

    assert_eq!(pad.held_buttons(), Ok(vec!["l1"]));

    pad.release_all()?;

    let Ok(holding) = pad.held_buttons();
    let Ok(said) = keys(&pad, "keyboard");

    assert!(holding.is_empty());
    assert!(said.len().is_multiple_of(2), "what went down came up");

    Ok(())
}

#[test]
fn holding_a_trigger_pulls_it_all_the_way() -> Result<(), Box<dyn Error>> {
    let mut pad = go("game")?;

    pad.hold("l2")?;

    let Ok(pulled) = pad.devices.sink.of_kind("pad", EventType::ABSOLUTE, Some(2));
    let range = pad.devices.axis("pad", 2)?;

    assert_eq!(pulled.last().map(|written| written.value), Some(range.maximum));

    Ok(())
}

#[test]
fn a_stick_is_one_frame_of_two_numbers() -> Result<(), Box<dyn Error>> {
    let mut pad = go("game")?;

    pad.thumbstick("left-stick", Point { x: 1.0, y: -1.0 })?;

    let axis = pad.devices.axis("pad", 0)?;
    let Ok(span) = axis.span();
    let Ok(pushed) = pad.devices.sink.of_kind("pad", EventType::ABSOLUTE, None);

    assert_eq!(
        pushed,
        [
            Written { kind: EventType::ABSOLUTE, code: 0, value: span },
            Written { kind: EventType::ABSOLUTE, code: 1, value: span.saturating_neg() },
        ]
    );

    Ok(())
}

#[test]
fn a_stick_only_moves_where_the_profile_publishes_a_pad() -> Result<(), GamepadError> {
    let mut pad = spare()?;
    let Ok(profile) = pad.profile();

    assert_eq!(profile.publishes("xbox-elite"), Ok(Has::No));

    pad.thumbstick("left-stick", Point { x: 1.0, y: 0.0 })?;

    assert_eq!(pad.devices.sink.of_kind("pad", EventType::ABSOLUTE, None), Ok(Vec::new()));

    Ok(())
}

#[test]
fn the_touchpad_is_not_in_the_profile_loop_at_all() -> Result<(), Box<dyn Error>> {
    let mut pad = go(console_input_gamepad::router::NAME)?;
    let Ok(()) = pad.tap(Point { x: 300, y: 400 });
    let Ok(touched) = pad.devices.sink.writes_by("touchpad");

    assert_eq!(touched.first().map(|w| (w.kind, w.code, w.value)), Some((EventType::KEY, KeyCode::BTN_TOUCH.0, 1)));
    assert!(touched.iter().any(|w| w.kind == EventType::ABSOLUTE && w.value == 300));

    Ok(())
}

#[test]
fn a_drag_reports_every_step_of_the_way() -> Result<(), Box<dyn Error>> {
    let mut pad = go(console_input_gamepad::router::NAME)?;
    let Ok(()) = pad.drag(Point { x: 0, y: 0 }, Point { x: 80, y: 0 }, 8, 0.0);
    let Ok(written) = pad.devices.sink.of_kind("touchpad", EventType::ABSOLUTE, Some(0));
    let along: Vec<i32> = written.iter().map(|written| written.value).collect();

    assert_eq!(along, [0, 10, 20, 30, 40, 50, 60, 70, 80]);

    Ok(())
}

#[test]
fn a_profile_nothing_has_says_which_there_are() -> Result<(), Box<dyn Error>> {
    let mut pad = go(console_input_gamepad::router::NAME)?;

    match pad.load_profile("gaming") {
        Ok(()) => Err(Box::from("a profile nothing has was loaded")),
        Err(fault) => {
            let said = fault.to_string();

            assert!(said.contains("router") && said.contains("gaming"), "{said}");

            Ok(())
        }
    }
}

#[test]
fn a_button_nothing_is_called_says_so_rather_than_pressing_something_else() -> Result<(), Box<dyn Error>> {
    let mut pad = go(console_input_gamepad::router::NAME)?;

    assert!(matches!(pad.press("triangle"), Err(GamepadError::NoSuchButton(_))));

    Ok(())
}

#[test]
fn a_capture_written_again_is_the_file_that_is_kept() -> Result<(), serde_json::Error> {
    let held = console_input_gamepad::capture::CAPTURED;
    let read: Vec<console_input_gamepad::capture::Descriptor> = serde_json::from_str(held)?;
    let written = serde_json::to_string_pretty(&read)?;

    assert_eq!(format!("{written}\n"), held);

    Ok(())
}
