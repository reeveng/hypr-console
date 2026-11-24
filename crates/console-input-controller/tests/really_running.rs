//! The same daemon, against devices the kernel really made.
//!
//! The test VM supplies its own writable /dev/uinput. These checks fail when
//! the daemon cannot find the emulated devices or publish its virtual pointer.

mod live;

use std::time::Duration;

use console_core_geometry::Point;
use console_input_event_devices::{AbsoluteAxisCode, EventType, KeyCode, RelativeAxisCode};
use console_input_gamepad::go::DragSeconds;

use live::{Failure, READS, required};

#[test]
#[ignore = "run just test e2e-live in a disposable NixOS VM"]
fn the_daemon_finds_all_three_devices() -> Result<(), Failure> {
  let running = required()?;
  let Ok(said) = running.said();

  for wanted in READS {
    assert!(
      said.contains(wanted),
      "it did not say it had found the {wanted}: {said}"
    );
  }

  Ok(())
}

#[test]
#[ignore = "run just test e2e-live in a disposable NixOS VM"]
fn the_right_stick_really_turns_a_wheel() -> Result<(), Failure> {
  let mut running = required()?;

  let Ok(go) = running.go();
  go.thumbstick("right-stick", Point { x: 0.0, y: -1.0 })?;
  let turned = running.total(
    (EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0),
    Duration::from_secs(1),
  )?;

  let Ok(steered) = running.go();
  steered.center("right-stick")?;

  assert!(turned > 0, "the wheel did not turn");

  Ok(())
}

#[test]
#[ignore = "run just test e2e-live in a disposable NixOS VM"]
fn a_finger_on_the_pad_really_moves_a_pointer() -> Result<(), Failure> {
  let mut running = required()?;
  let Ok(go) = running.go();
  let Ok(()) = go.drag(
    Point { x: 200, y: 300 },
    Point { x: 500, y: 300 },
    6,
    DragSeconds(0.12),
  );
  let events = running.events(Duration::from_millis(400))?;
  let moved: Vec<(u16, i32)> = events
    .iter()
    .filter(|event| event.kind == EventType::RELATIVE)
    .map(|event| (event.code, event.value))
    .collect();
  let across = moved
    .iter()
    .filter(|(code, _)| *code == RelativeAxisCode::REL_X.0)
    .fold(0_i32, |sum, (_, value)| sum.saturating_add(*value));

  assert!(across > 0, "the pointer did not move");
  assert!(
    moved
      .iter()
      .all(|(code, _)| *code == RelativeAxisCode::REL_X.0 || *code == RelativeAxisCode::REL_Y.0),
    "a finger turned something that is not the pointer"
  );

  Ok(())
}

#[test]
#[ignore = "run just test e2e-live in a disposable NixOS VM"]
fn a_tap_is_really_a_click() -> Result<(), Failure> {
  let mut running = required()?;
  let Ok(go) = running.go();
  let Ok(()) = go.tap(Point { x: 500, y: 500 });
  let events = running.events(Duration::from_millis(400))?;
  let clicked: Vec<(u16, i32)> = events
    .iter()
    .filter(|event| event.kind == EventType::KEY)
    .map(|event| (event.code, event.value))
    .collect();

  assert_eq!(
    clicked,
    [(KeyCode::BTN_LEFT.0, 1), (KeyCode::BTN_LEFT.0, 0)]
  );

  Ok(())
}

#[test]
#[ignore = "run just test e2e-live in a disposable NixOS VM"]
fn a_shoulder_really_reaches_the_compositor() -> Result<(), Failure> {
  let mut running = required()?;

  let Ok(go) = running.go();
  go.press("r1")?;
  let ran = running.ran()?;

  assert_eq!(
    ran,
    [["hyprctl", "dispatch", r#"hl.dsp.focus({workspace = "+1"})"#]]
  );

  Ok(())
}

#[test]
#[ignore = "run just test e2e-live in a disposable NixOS VM"]
fn a_paddle_really_opens_the_menu() -> Result<(), Failure> {
  let mut running = required()?;

  let Ok(go) = running.go();
  go.press("left-paddle-top")?;
  let names = running.names()?;

  assert_eq!(names, ["launcher"]);

  Ok(())
}

#[test]
#[ignore = "run just test e2e-live in a disposable NixOS VM"]
fn the_emulator_publishes_what_the_capture_says() -> Result<(), Failure> {
  let running = required()?;
  let Ok(every) = console_input_event_devices::Device::every();
  let pad = every
    .into_iter()
    .find(|device| device.name.as_deref() == Some("Microsoft X-Box One Elite 2 pad"))
    .ok_or("a pad")?;
  let axes = pad.absolute()?;
  let (_, stick) = axes
    .into_iter()
    .find(|(code, _)| *code == AbsoluteAxisCode::ABS_RX)
    .ok_or("a right stick")?;

  assert_eq!((stick.minimum, stick.maximum), (-32768, 32767));
  assert_eq!(
    pad.physical_path, None,
    "a pad with a physical location is a real one"
  );

  drop(running);

  Ok(())
}
