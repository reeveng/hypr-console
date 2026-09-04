//! The home screen: whose buttons are whose, and whether the bar can be
//! pressed while it is drawn.
//!
//! Both of these are one fault, found on the device after everything about it
//! had been asked from the wrong side and answered yes. The home screen asked
//! the compositor for the keyboard exclusively, which is the only way a layer
//! drawn under everything can have it -- and Hyprland answers an exclusive
//! layer by handing it every pointer and every touch on the screen, wherever
//! they land, because that is what a lock screen needs. So a finger on the
//! launcher, the keyboard, the music or the sound reached the home screen
//! instead, which opened whatever the highlight happened to be standing on.
//!
//! Nothing caught it, and the reason is worth keeping. Every check that
//! touched this asked about plumbing: the bar's surface is there, at the right
//! size, on the right layer, with the right modules, and the daemon binds the
//! buttons it says it binds. Every one of those answers was yes. None of them
//! is a finger. `PRESSABLE` is the one that presses, and it is the shape any
//! check of a surface that answers a touch should have.

use console_stage::checking::{Body, Check, Done, cannot, empty, same, seen};
use console_stage::device::{Device, Seen};
use console_stage::here::{Here, TURNS};

/// The home screen, drawn, with nothing over it.
const THE_HOME_SCREEN: &str = r#"{"eDP-1":{"levels":{
    "0":[{"namespace":"awww-daemon","h":1600},{"namespace":"console-home","h":1562}],
    "2":[{"namespace":"waybar","h":38},{"namespace":"updating","h":2}]}}}"#;

pub const WHOSE_BUTTONS: Check = Check {
    name: "260-the-home-screens-buttons",
    about: "A is the pointer's until the d-pad wakes the home screen, and the \
            home screen's after.",
    feature: "home",
    since: "2026-09-03",
    bodies: &[Body::Here(whose_here), Body::Device(whose_there)],
};

pub const PRESSABLE: Check = Check {
    name: "270-the-bar-answers-a-finger",
    about: "A tap on the bar opens what the bar says it opens, with the home \
            screen drawn under it.",
    feature: "home",
    since: "2026-09-03",
    bodies: &[Body::Device(pressable_there)],
};

/// Asleep, A is the pointer's button, and the home screen is told nothing.
///
/// This is the half that can be asked with no machine. The stage models the
/// waking, because whether the highlight is up is not something the compositor
/// answers and a check that assumed it was awake would be asking the question
/// from the side that was already wrong.
fn whose_here(stage: &mut Here) -> Done {
    stage.showing(THE_HOME_SCREEN)?;

    stage.press("a")?;
    stage.settle(TURNS);
    seen(stage.sent(evdev::EventType::KEY, evdev::KeyCode::BTN_LEFT.0, 1), || {
        "A on a sleeping home screen was not the pointer's button".to_string()
    })?;
    empty(stage.told(), || format!("the home screen was told {:?} while asleep", stage.told()))?;

    // The d-pad is the home screen's whether it is awake or not, and the first
    // press of it is the waking.
    stage.fresh();
    stage.press("dpad-right")?;
    stage.settle(TURNS);
    same(&stage.told(), &[console_door::Said::Right].as_slice(), || {
        format!("the d-pad said {:?} to the home screen", stage.told())
    })?;

    // And now A has changed hands. Nothing about the screen changed -- the
    // same layers are up -- which is why this cannot be read off the
    // compositor and is read off what the home screen says about itself.
    stage.fresh();
    stage.press("a")?;
    stage.settle(TURNS);
    same(&stage.told(), &[console_door::Said::Pressing, console_door::Said::Released].as_slice(), || {
        format!("A on an awake home screen said {:?}", stage.told())
    })?;
    empty(&stage.written, || "A was still the pointer's button".to_string())?;

    // B puts the highlight away, and A goes back to the pointer with it.
    stage.fresh();
    stage.press("b")?;
    stage.settle(TURNS);
    stage.press("a")?;
    stage.settle(TURNS);
    seen(stage.sent(evdev::EventType::KEY, evdev::KeyCode::BTN_LEFT.0, 1), || {
        "B did not give A back to the pointer".to_string()
    })
}

/// On the machine, asked of the note the home screen leaves about itself.
///
/// Not of what A opened, which was the first thing this tried and is the wrong
/// question: A asleep is the pointer's button, so what it opens is whatever
/// the pointer happens to be over -- the bar, if somebody left it there -- and
/// a check that expects nothing to open fails on a device that is working
/// correctly.
///
/// Whether the home screen is holding a highlight is the thing that decides
/// whose A it is, and the home screen says so out loud because the daemon has
/// to read it. So that is what is asked.
fn whose_there(stage: &mut Device) -> Done {
    stage.exec_cmd("put-away");
    stage.settle(1.2);

    same(&stage.home_awake(), &Seen::NotYet, || {
        "the home screen was holding a highlight with nobody having asked for one".to_string()
    })?;

    // The d-pad is the home screen's whether it is awake or not, and the first
    // press of it is the waking.
    stage.press("dpad-right");
    stage.settle(1.0);
    same(&stage.home_awake(), &Seen::Yes, || {
        "the d-pad did not wake the home screen".to_string()
    })?;

    // And now Y is the home screen's, which it is not while it is asleep.
    stage.press("y");
    stage.settle(1.4);
    let up = stage.menus();
    seen(on_screen(&up, "home-place"), || format!("Y on an awake home screen opened {up:?}"))?;

    // Out of the card, and then out of the highlight.
    stage.press("b");
    stage.settle(1.2);
    stage.press("b");
    stage.settle(1.0);
    same(&stage.home_awake(), &Seen::NotYet, || {
        "B did not put the highlight away".to_string()
    })
}

/// The one that presses a pixel.
///
/// A finger on the launcher icon in the bar, with the home screen drawn under
/// it, and the launcher had better open. This is the check the fault would
/// have had to get past, and none of the ones that existed asked anything a
/// finger could have answered: the bar's surface was there, at the right size,
/// on the right layer, with the right modules, and every tap on it went
/// somewhere else.
///
/// Where the icon is is asked of the compositor rather than written down. The
/// bar's icons are as tall as the bar and about as wide, and the launcher is
/// the first of them from the left edge, so half a bar in and half a bar down
/// is the middle of it -- which stays true when the bar changes height, and a
/// number written down here would not.
fn pressable_there(stage: &mut Device) -> Done {
    stage.exec_cmd("put-away");
    stage.settle(1.0);

    let Some((left, top, _, tall)) = stage.layer("waybar") else {
        return cannot("the bar is not on the screen to be pressed");
    };

    let at = (left + tall / 2, top + tall / 2);

    let before = stage.menus();
    empty(&before, || format!("something was already up: {before:?}"))?;

    stage.touch(at)?;
    stage.settle(1.6);

    let up = stage.menus();
    seen(on_screen(&up, "launcher"), || {
        format!("a finger on the launcher icon at {at:?} opened {up:?}")
    })?;

    // And again, which is what says the first one was not a fluke of whatever
    // happened to have the focus.
    stage.exec_cmd("put-away");
    stage.settle(1.0);
    stage.touch(at)?;
    stage.settle(1.6);

    let again = stage.menus();
    seen(on_screen(&again, "launcher"), || {
        format!("the second finger on the launcher icon opened {again:?}")
    })?;

    stage.exec_cmd("put-away");
    stage.settle(0.8);
    Ok(())
}

fn on_screen(up: &[String], namespace: &str) -> Seen {
    match up.iter().any(|name| name == namespace) {
        true => Seen::Yes,
        false => Seen::NotYet,
    }
}
