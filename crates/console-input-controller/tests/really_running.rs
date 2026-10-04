//! The same daemon, against devices the kernel really made.
//!
//! These need to be able to make an input device, which means /dev/uinput. They
//! say so and stop where that is not open, rather than failing, because
//! everything they prove about what the daemon decides is proved in the fast
//! tier too. What only these can prove is that the emulator's devices are the
//! ones the daemon goes looking for, and that what it writes is a real pointer
//! moving.

mod live;

use console_core_geometry::Point;
use console_core_external_programs::Program;
use console_input_event_devices::{AbsoluteAxisCode, EventType, KeyCode, RelativeAxisCode};

use live::{Failure, READS, or_skip};

#[test]
fn the_daemon_finds_all_three_devices() -> Result<(), Failure> {
    let found = or_skip()?;
    let running = match found {
        Some(running) => running,
        None => return Ok(()),
    };
    let Ok(said) = running.said();

    for wanted in READS {
        assert!(said.contains(wanted), "it did not say it had found the {wanted}: {said}");
    }

    Ok(())
}

#[test]
fn the_right_stick_really_turns_a_wheel() -> Result<(), Failure> {
    let found = or_skip()?;
    let mut running = match found {
        Some(running) => running,
        None => return Ok(()),
    };

    running.go.thumbstick("right-stick", Point { x: 0.0, y: -1.0 })?;
    let turned = running.total((EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0), 1.0)?;

    running.go.center("right-stick")?;

    assert!(turned > 0, "the wheel did not turn");

    Ok(())
}

#[test]
fn a_finger_on_the_pad_really_moves_a_pointer() -> Result<(), Failure> {
    let found = or_skip()?;
    let mut running = match found {
        Some(running) => running,
        None => return Ok(()),
    };
    let Ok(()) = running.go.drag(Point { x: 200, y: 300 }, Point { x: 500, y: 300 }, 6, 0.12);
    let events = running.events(0.4)?;
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
        moved.iter().all(|(code, _)| *code == RelativeAxisCode::REL_X.0 || *code == RelativeAxisCode::REL_Y.0),
        "a finger turned something that is not the pointer"
    );

    Ok(())
}

#[test]
fn a_tap_is_really_a_click() -> Result<(), Failure> {
    let found = or_skip()?;
    let mut running = match found {
        Some(running) => running,
        None => return Ok(()),
    };
    let Ok(()) = running.go.tap(Point { x: 500, y: 500 });
    let events = running.events(0.4)?;
    let clicked: Vec<(u16, i32)> = events
        .iter()
        .filter(|event| event.kind == EventType::KEY)
        .map(|event| (event.code, event.value))
        .collect();

    assert_eq!(clicked, [(KeyCode::BTN_LEFT.0, 1), (KeyCode::BTN_LEFT.0, 0)]);

    Ok(())
}

#[test]
fn a_shoulder_really_reaches_the_compositor() -> Result<(), Failure> {
    let found = or_skip()?;
    let mut running = match found {
        Some(running) => running,
        None => return Ok(()),
    };

    running.go.press("r1")?;
    let ran = running.ran()?;
    let Ok(hyprctl) = Program::Hyprctl.name();

    assert_eq!(ran, [[hyprctl, "dispatch", r#"hl.dsp.focus({workspace = "+1"})"#]]);

    Ok(())
}

#[test]
fn a_paddle_really_opens_the_menu() -> Result<(), Failure> {
    let found = or_skip()?;
    let mut running = match found {
        Some(running) => running,
        None => return Ok(()),
    };

    running.go.press("left-paddle-top")?;
    let names = running.names()?;

    assert_eq!(names, ["launcher"]);

    Ok(())
}

#[test]
fn the_emulator_publishes_what_the_capture_says() -> Result<(), Failure> {
    let found = or_skip()?;
    let running = match found {
        Some(running) => running,
        None => return Ok(()),
    };
    let Ok(every) = console_input_event_devices::Device::every();
    let pad = every
        .into_iter()
        .find(|device| device.name.as_deref() == Some("Microsoft X-Box One Elite 2 pad"))
        .ok_or("a pad")?;
    let axes = pad.absolute()?;
    let (_, stick) = axes.into_iter().find(|(code, _)| *code == AbsoluteAxisCode::ABS_RX).ok_or("a right stick")?;

    assert_eq!((stick.minimum, stick.maximum), (-32768, 32767));
    assert_eq!(pad.physical_path, None, "a pad with a physical location is a real one");

    drop(running);

    Ok(())
}
