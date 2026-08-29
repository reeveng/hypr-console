//! The right stick and the touchpad, which are the pointer.

use evdev::{EventType, KeyCode, RelativeAxisCode};
use console_stage::checking::{Body, Check, Done, cannot, ought};
use console_stage::device::Device;
use console_stage::here::{Here, TURNS};

/// How many turns of the loop a stick is held for. Long enough that a wheel
/// notch is arithmetic rather than a rounding.
const HELD: usize = 12;

pub const SCROLL: Check = Check {
    name: "120-scrolling",
    about: "The right stick turns the wheel, and how far it is pushed is how fast.",
    feature: "scroll",
    since: "2026-08-24",
    bodies: &[Body::Here(scroll_here), Body::Device(scroll_there)],
};

pub const TOUCHPAD: Check = Check {
    name: "130-the-touchpad",
    about: "A finger on the pad moves the pointer, and a quick touch is a click.",
    feature: "touchpad",
    since: "2026-08-27",
    bodies: &[Body::Here(touch_here), Body::Device(touch_there)],
};

fn scroll_here(stage: &mut Here) -> Done {
    stage.stick("right-stick", 0.0, -1.0)?;
    stage.settle(HELD);
    let up = stage.wrote(EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0);
    ought(up > 0, || "the wheel did not turn".to_string())?;

    stage.stick("right-stick", 0.0, 1.0)?;
    stage.settle(HELD);
    let back = stage.wrote(EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0);
    ought(back < up, || "pushing the other way did not turn it back".to_string())
}

/// Not asked on the machine. What the wheel did is a thing the window under the
/// pointer knows and nothing else can be asked, so this is a check the emulator
/// answers and the device cannot.
fn scroll_there(_stage: &mut Device) -> Done {
    cannot("nothing on the device can see a page scroll")
}

fn touch_here(stage: &mut Here) -> Done {
    stage.drag((200, 300), (500, 300));
    stage.settle(TURNS);
    ought(stage.wrote(EventType::RELATIVE, RelativeAxisCode::REL_X.0) > 0, || {
        "the pointer did not move".to_string()
    })?;
    ought(stage.wrote(EventType::RELATIVE, RelativeAxisCode::REL_Y.0) == 0, || {
        "it moved the other way too".to_string()
    })?;

    stage.tap(400, 400);
    stage.settle(TURNS);
    ought(stage.sent(EventType::KEY, KeyCode::BTN_LEFT.0, 1), || {
        "a tap did not click".to_string()
    })?;
    ought(stage.sent(EventType::KEY, KeyCode::BTN_LEFT.0, 0), || {
        "the click was never let go".to_string()
    })
}

/// InputPlumber cannot send touch: asked to translate it, it answers
/// "Translation not implemented" and drops the event, which is the whole reason
/// the daemon reads the pad directly. So there is no way to press this one from
/// here, and the pointer is where the emulator has to be believed.
fn touch_there(stage: &mut Device) -> Done {
    stage.tap(512, 512)
}
