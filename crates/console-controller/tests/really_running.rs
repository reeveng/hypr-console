//! The same daemon, against devices the kernel really made.
//!
//! These need to be able to make an input device, which means /dev/uinput. They
//! say so and stop where that is not open, rather than failing, because
//! everything they prove about what the daemon decides is proved in the fast
//! tier too. What only these can prove is that the emulator's devices are the
//! ones the daemon goes looking for, and that what it writes is a real pointer
//! moving.

mod live;

use evdev::{AbsoluteAxisCode, EventType, KeyCode, RelativeAxisCode};

use live::{READS, or_skip};

#[test]
fn the_daemon_finds_all_three_devices() {
    let Some(running) = or_skip() else { return };
    let said = running.said();
    for wanted in READS {
        assert!(said.contains(wanted), "it did not say it had found the {wanted}: {said}");
    }
}

#[test]
fn the_right_stick_really_turns_a_wheel() {
    let Some(mut running) = or_skip() else { return };
    running.go.stick("right-stick", 0.0, -1.0).expect("a stick");
    let turned = running.total(EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0, 1.0);
    running.go.centre("right-stick").expect("a stick");
    assert!(turned > 0, "the wheel did not turn");
}

#[test]
fn a_finger_on_the_pad_really_moves_a_pointer() {
    let Some(mut running) = or_skip() else { return };
    running.go.drag((200, 300), (500, 300), 6, 0.12);
    let moved: Vec<(u16, i32)> = running
        .events(0.4)
        .iter()
        .filter(|event| event.event_type() == EventType::RELATIVE)
        .map(|event| (event.code(), event.value()))
        .collect();
    let across: i32 =
        moved.iter().filter(|(code, _)| *code == RelativeAxisCode::REL_X.0).map(|(_, v)| v).sum();
    assert!(across > 0, "the pointer did not move");
    assert!(
        moved.iter().all(|(code, _)| {
            [RelativeAxisCode::REL_X.0, RelativeAxisCode::REL_Y.0].contains(code)
        }),
        "a finger turned something that is not the pointer"
    );
}

#[test]
fn a_tap_is_really_a_click() {
    let Some(mut running) = or_skip() else { return };
    running.go.tap(500, 500);
    let clicked: Vec<(u16, i32)> = running
        .events(0.4)
        .iter()
        .filter(|event| event.event_type() == EventType::KEY)
        .map(|event| (event.code(), event.value()))
        .collect();
    assert_eq!(clicked, [(KeyCode::BTN_LEFT.0, 1), (KeyCode::BTN_LEFT.0, 0)]);
}

#[test]
fn a_shoulder_really_reaches_the_compositor() {
    let Some(mut running) = or_skip() else { return };
    running.go.press("r1").expect("a shoulder");
    running.settle();
    assert_eq!(
        running.commands(),
        [["hyprctl", "dispatch", r#"hl.dsp.focus({workspace = "+1"})"#]]
    );
}

#[test]
fn a_paddle_really_opens_the_menu() {
    let Some(mut running) = or_skip() else { return };
    running.go.press("left-paddle-top").expect("a paddle");
    running.settle();
    assert_eq!(running.names(), ["launcher"]);
}

/// The one thing the fast tier cannot check: that a device built from the
/// capture is the device the daemon is looking for, down to the axes.
#[test]
fn the_emulator_publishes_what_the_capture_says() {
    let Some(running) = or_skip() else { return };
    let pad = evdev::enumerate()
        .map(|(_, device)| device)
        .find(|device| device.name() == Some("Microsoft X-Box One Elite 2 pad"))
        .expect("a pad");
    let axis = pad.get_absinfo().expect("its axes").find(|(code, _)| *code == AbsoluteAxisCode::ABS_RX);
    let (_, stick) = axis.expect("a right stick");
    assert_eq!((stick.minimum(), stick.maximum()), (-32768, 32767));
    assert_eq!(pad.physical_path(), None, "a pad with a physical location is a real one");
    drop(running);
}
