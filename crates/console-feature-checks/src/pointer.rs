//! The right stick and the touchpad, which are the pointer.

use evdev::{EventType, KeyCode, RelativeAxisCode};
use console_test_stages::checking::{Body, Check, Done, cannot, less_than, more_than, same, seen};
use console_test_stages::device::Device;
use console_test_stages::here::{Here, TURNS};

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

    let Ok(()) = stage.settle(HELD);
    let Ok(up) = stage.wrote(EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0);

    more_than(up, 0, || "the wheel did not turn".to_string())?;

    stage.stick("right-stick", 0.0, 1.0)?;

    let Ok(()) = stage.settle(HELD);
    let Ok(back) = stage.wrote(EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0);

    less_than(back, up, || "pushing the other way did not turn it back".to_string())
}

fn scroll_there(_stage: &mut Device) -> Done {
    cannot("nothing on the device can see a page scroll")
}

fn touch_here(stage: &mut Here) -> Done {
    let Ok(()) = stage.drag((200, 300), (500, 300));
    let Ok(()) = stage.settle(TURNS);
    let Ok(across) = stage.wrote(EventType::RELATIVE, RelativeAxisCode::REL_X.0);

    more_than(across, 0, || "the pointer did not move".to_string())?;

    let Ok(down) = stage.wrote(EventType::RELATIVE, RelativeAxisCode::REL_Y.0);

    same(&down, &0, || "it moved the other way too".to_string())?;

    let Ok(()) = stage.tap(400, 400);
    let Ok(()) = stage.settle(TURNS);
    let Ok(pressed) = stage.sent(EventType::KEY, KeyCode::BTN_LEFT.0, 1);

    seen(pressed, || "a tap did not click".to_string())?;

    let Ok(let_go) = stage.sent(EventType::KEY, KeyCode::BTN_LEFT.0, 0);

    seen(let_go, || "the click was never let go".to_string())
}

fn touch_there(stage: &mut Device) -> Done {
    stage.tap(512, 512)
}
