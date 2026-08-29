//! The names of the things on the front of the machine, and what they are.
//!
//! Two vocabularies meet here. One is the Legion Go's, the names a person uses
//! for what their thumbs are on, and the names InputPlumber's profiles are
//! written in. The other is the kernel's, the codes that come out of a device.
//! Everything that has to cross between them crosses here, once, so that a
//! button called X in a profile and a button called X in a test are the same
//! button.
//!
//! The face buttons are the trap. On this device the one labelled X is
//! BTN_NORTH and the one labelled Y is BTN_WEST, which is not what either name
//! suggests and not what most pads do. It is written down here rather than
//! remembered.

use std::str::FromStr;

use evdev::{AbsoluteAxisCode, KeyCode};

/// What a person calls it, and what InputPlumber's profiles call it.
///
/// The left column is what you would say out loud. The right is the `button:`
/// name in a profile, which is the only name InputPlumber answers to.
pub const BUTTONS: [(&str, &str); 21] = [
    ("a", "South"),
    ("b", "East"),
    ("x", "North"),
    ("y", "West"),
    ("dpad-up", "DPadUp"),
    ("dpad-down", "DPadDown"),
    ("dpad-left", "DPadLeft"),
    ("dpad-right", "DPadRight"),
    ("l1", "LeftBumper"),
    ("r1", "RightBumper"),
    ("l3", "LeftStick"),
    ("r3", "RightStick"),
    ("menu", "Start"),
    ("view", "Select"),
    ("legion-left", "Guide"),
    ("legion-right", "QuickAccess"),
    ("keyboard", "Keyboard"),
    ("left-paddle-top", "LeftPaddle1"),
    ("left-paddle-bottom", "LeftPaddle2"),
    ("right-paddle-top", "RightPaddle1"),
    ("right-paddle-bottom", "RightPaddle2"),
];

/// The two analogue sticks, under the names a profile uses.
pub const AXES: [(&str, &str); 2] = [("left-stick", "LeftStick"), ("right-stick", "RightStick")];

/// The two triggers, under the names a profile uses.
pub const TRIGGERS: [(&str, &str); 2] = [("l2", "LeftTrigger"), ("r2", "RightTrigger")];

/// What a profile can send, and the code it arrives as.
///
/// Only what the profiles actually target is here. A name a profile does not
/// use is a name nothing has confirmed, and a guess in this table would be a
/// guess about which button a thumb is on.
pub const GAMEPAD_CODES: [(&str, KeyCode); 11] = [
    ("South", KeyCode::BTN_SOUTH),
    ("East", KeyCode::BTN_EAST),
    ("North", KeyCode::BTN_NORTH),
    ("West", KeyCode::BTN_WEST),
    ("Start", KeyCode::BTN_START),
    ("Select", KeyCode::BTN_SELECT),
    ("Guide", KeyCode::BTN_MODE),
    ("LeftBumper", KeyCode::BTN_TL),
    ("RightBumper", KeyCode::BTN_TR),
    ("LeftStick", KeyCode::BTN_THUMBL),
    ("RightStick", KeyCode::BTN_THUMBR),
];

pub const MOUSE_CODES: [(&str, KeyCode); 3] = [
    ("Left", KeyCode::BTN_LEFT),
    ("Right", KeyCode::BTN_RIGHT),
    ("Middle", KeyCode::BTN_MIDDLE),
];

/// A stick, as the pair of axes it arrives on.
pub const AXIS_CODES: [(&str, (AbsoluteAxisCode, AbsoluteAxisCode)); 2] = [
    ("LeftStick", (AbsoluteAxisCode::ABS_X, AbsoluteAxisCode::ABS_Y)),
    ("RightStick", (AbsoluteAxisCode::ABS_RX, AbsoluteAxisCode::ABS_RY)),
];

/// A trigger, as the axis it arrives on.
pub const TRIGGER_CODES: [(&str, AbsoluteAxisCode); 2] = [
    ("LeftTrigger", AbsoluteAxisCode::ABS_Z),
    ("RightTrigger", AbsoluteAxisCode::ABS_RZ),
];

/// Both triggers also report as a button when they are pulled far enough,
/// which is how the daemon learns that L2 is being held.
pub const TRIGGER_BUTTONS: [(&str, KeyCode); 2] =
    [("LeftTrigger", KeyCode::BTN_TL2), ("RightTrigger", KeyCode::BTN_TR2)];

/// The one lookup every table here is read with.
fn found<'a, T: Copy>(table: &'a [(&'a str, T)], name: &str) -> Option<T> {
    table.iter().find(|(said, _)| *said == name).map(|(_, what)| *what)
}

pub fn gamepad_code(name: &str) -> Option<KeyCode> {
    found(&GAMEPAD_CODES, name)
}

pub fn mouse_code(name: &str) -> Option<KeyCode> {
    found(&MOUSE_CODES, name)
}

pub fn axis_codes(name: &str) -> Option<(AbsoluteAxisCode, AbsoluteAxisCode)> {
    found(&AXIS_CODES, name)
}

pub fn trigger_code(name: &str) -> Option<AbsoluteAxisCode> {
    found(&TRIGGER_CODES, name)
}

pub fn trigger_button(name: &str) -> Option<KeyCode> {
    found(&TRIGGER_BUTTONS, name)
}

/// The profile's name for a stick, if that is what was said.
pub fn axis_named(spoken: &str) -> &str {
    found(&AXES, spoken).unwrap_or(spoken)
}

/// The profile's name for a trigger, if that is what was said.
pub fn trigger_named(spoken: &str) -> &str {
    found(&TRIGGERS, spoken).unwrap_or(spoken)
}

/// Whether what was said is a trigger rather than a button.
pub fn is_trigger(spoken: &str) -> bool {
    found(&TRIGGERS, spoken).is_some()
}

/// `KeyPageUp` as the kernel's KEY_PAGEUP.
///
/// InputPlumber writes a key as Key followed by its name in the shape a person
/// would write it. The kernel writes the same name in capitals. That is the
/// whole of the difference, for every key any profile here sends.
pub fn key_code(name: &str) -> Result<KeyCode, String> {
    let tail = name.strip_prefix("Key").ok_or_else(|| format!("not a key name: {name:?}"))?;
    KeyCode::from_str(&format!("KEY_{}", tail.to_uppercase()))
        .map_err(|_| format!("no such key: {name:?}"))
}

/// `x` as `North`, which is what a profile calls it.
pub fn button_name(spoken: &str) -> Result<&'static str, String> {
    found(&BUTTONS, spoken).ok_or_else(|| {
        let mut every: Vec<&str> = BUTTONS.iter().map(|(said, _)| *said).collect();
        every.sort_unstable();
        format!("no button called {spoken:?}; try one of {}", every.join(", "))
    })
}

/// `North` as `x`, which is what is written on the button.
pub fn spoken_for(profile_name: &str) -> &str {
    BUTTONS
        .iter()
        .find(|(_, name)| *name == profile_name)
        .map_or(profile_name, |(spoken, _)| *spoken)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_face_buttons_are_not_where_their_names_suggest() {
        // The one thing in this file worth a test: X is north and Y is west.
        assert_eq!(gamepad_code(button_name("x").expect("x")), Some(KeyCode::BTN_NORTH));
        assert_eq!(gamepad_code(button_name("y").expect("y")), Some(KeyCode::BTN_WEST));
    }

    #[test]
    fn a_button_crosses_both_ways() {
        assert_eq!(button_name("legion-right"), Ok("QuickAccess"));
        assert_eq!(spoken_for("QuickAccess"), "legion-right");
    }

    #[test]
    fn a_name_nothing_calls_a_button_says_what_the_buttons_are() {
        let fault = button_name("triangle").expect_err("no such button");
        assert!(fault.contains("triangle") && fault.contains("dpad-up"), "{fault}");
    }

    #[test]
    fn a_profile_name_nothing_speaks_for_is_left_as_it_is() {
        assert_eq!(spoken_for("LeftPaddle3"), "LeftPaddle3");
    }

    #[test]
    fn a_key_is_the_same_name_in_capitals() {
        assert_eq!(key_code("KeyPageUp"), Ok(KeyCode::KEY_PAGEUP));
        assert_eq!(key_code("KeyF13"), Ok(KeyCode::KEY_F13));
    }

    #[test]
    fn something_that_is_not_a_key_says_so_rather_than_guessing() {
        assert!(key_code("South").is_err());
        assert!(key_code("KeyNotAKey").is_err());
    }
}
