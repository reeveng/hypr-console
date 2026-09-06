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

use console_never::Never;
use evdev::{AbsoluteAxisCode, KeyCode};

pub const BUTTON: &str = "Gamepad:Button:";
pub const AXIS: &str = "Gamepad:Axis:";
pub const TRIGGER: &str = "Gamepad:Trigger:";

pub fn button_of(capability: &str) -> Result<Option<&str>, Never> {
    Ok(capability.strip_prefix(BUTTON))
}

pub fn capability_of(button: &str) -> Result<String, Never> {
    Ok(format!("{BUTTON}{button}"))
}

pub const BUTTONS: [(&str, &str); 23] = [
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
    ("quick-access-2", "QuickAccess2"),
    ("right-paddle-3", "RightPaddle3"),
];

pub const AXES: [(&str, &str); 2] = [("left-stick", "LeftStick"), ("right-stick", "RightStick")];

pub const TRIGGERS: [(&str, &str); 2] = [("l2", "LeftTrigger"), ("r2", "RightTrigger")];

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

pub const AXIS_CODES: [(&str, (AbsoluteAxisCode, AbsoluteAxisCode)); 2] = [
    ("LeftStick", (AbsoluteAxisCode::ABS_X, AbsoluteAxisCode::ABS_Y)),
    ("RightStick", (AbsoluteAxisCode::ABS_RX, AbsoluteAxisCode::ABS_RY)),
];

pub const TRIGGER_CODES: [(&str, AbsoluteAxisCode); 2] = [
    ("LeftTrigger", AbsoluteAxisCode::ABS_Z),
    ("RightTrigger", AbsoluteAxisCode::ABS_RZ),
];

pub const HAT_CODES: [(&str, (AbsoluteAxisCode, i32)); 4] = [
    ("DPadUp", (AbsoluteAxisCode::ABS_HAT0Y, -1)),
    ("DPadDown", (AbsoluteAxisCode::ABS_HAT0Y, 1)),
    ("DPadLeft", (AbsoluteAxisCode::ABS_HAT0X, -1)),
    ("DPadRight", (AbsoluteAxisCode::ABS_HAT0X, 1)),
];

pub fn hat_code(name: &str) -> Result<Option<(AbsoluteAxisCode, i32)>, Never> {
    found(&HAT_CODES, name)
}

pub const TRIGGER_BUTTONS: [(&str, KeyCode); 2] =
    [("LeftTrigger", KeyCode::BTN_TL2), ("RightTrigger", KeyCode::BTN_TR2)];

fn found<'a, T: Copy>(table: &'a [(&'a str, T)], name: &str) -> Result<Option<T>, Never> {
    Ok(table.iter().find(|(said, _)| *said == name).map(|(_, what)| *what))
}

pub fn gamepad_code(name: &str) -> Result<Option<KeyCode>, Never> {
    found(&GAMEPAD_CODES, name)
}

pub fn mouse_code(name: &str) -> Result<Option<KeyCode>, Never> {
    found(&MOUSE_CODES, name)
}

pub fn axis_codes(name: &str) -> Result<Option<(AbsoluteAxisCode, AbsoluteAxisCode)>, Never> {
    found(&AXIS_CODES, name)
}

pub fn trigger_code(name: &str) -> Result<Option<AbsoluteAxisCode>, Never> {
    found(&TRIGGER_CODES, name)
}

pub fn trigger_button(name: &str) -> Result<Option<KeyCode>, Never> {
    found(&TRIGGER_BUTTONS, name)
}

pub fn axis_named(spoken: &str) -> Result<&str, Never> {
    let named = found(&AXES, spoken)?;

    Ok(named.unwrap_or(spoken))
}

pub fn trigger_named(spoken: &str) -> Result<&str, Never> {
    let named = found(&TRIGGERS, spoken)?;

    Ok(named.unwrap_or(spoken))
}

pub fn is_trigger(spoken: &str) -> Result<Names, Never> {
    let trigger = found(&TRIGGERS, spoken)?;

    Ok(match trigger.is_some() {
        true => Names::ATrigger,
        false => Names::AButton,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Names {
    ATrigger,
    AButton,
}

pub fn key_code(name: &str) -> Result<KeyCode, String> {
    let tail = name.strip_prefix("Key").ok_or_else(|| format!("not a key name: {name:?}"))?;
    KeyCode::from_str(&format!("KEY_{}", tail.to_uppercase()))
        .map_err(|_| format!("no such key: {name:?}"))
}

pub fn button_name(spoken: &str) -> Result<&'static str, String> {
    let Ok(found) = found(&BUTTONS, spoken);

    found.ok_or_else(|| {
        let mut every: Vec<&str> = BUTTONS.iter().map(|(said, _)| *said).collect();
        every.sort_unstable();
        format!("no button called {spoken:?}; try one of {}", every.join(", "))
    })
}

pub fn spoken_for(profile_name: &str) -> Result<&str, Never> {
    Ok(BUTTONS
        .iter()
        .find(|(_, name)| *name == profile_name)
        .map_or(profile_name, |(spoken, _)| *spoken))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_face_buttons_are_not_where_their_names_suggest() {
        assert_eq!(gamepad_code(button_name("x").expect("x")), Ok(Some(KeyCode::BTN_NORTH)));
        assert_eq!(gamepad_code(button_name("y").expect("y")), Ok(Some(KeyCode::BTN_WEST)));
    }

    #[test]
    fn the_dpad_is_a_hat() {
        assert_eq!(gamepad_code("DPadUp"), Ok(None));
        assert_eq!(hat_code("DPadUp"), Ok(Some((AbsoluteAxisCode::ABS_HAT0Y, -1))));
        assert_eq!(hat_code("DPadRight"), Ok(Some((AbsoluteAxisCode::ABS_HAT0X, 1))));
    }

    #[test]
    fn only_a_button_is_a_button() {
        assert_eq!(button_of("Gamepad:Button:South"), Ok(Some("South")));
        assert_eq!(button_of("Gamepad:Axis:LeftStick"), Ok(None));
        assert_eq!(capability_of("South"), Ok("Gamepad:Button:South".to_string()));
    }

    #[test]
    fn a_button_crosses_both_ways() {
        assert_eq!(button_name("legion-right"), Ok("QuickAccess"));
        assert_eq!(spoken_for("QuickAccess"), Ok("legion-right"));
    }

    #[test]
    fn a_name_nothing_calls_a_button_says_what_the_buttons_are() {
        let fault = button_name("triangle").expect_err("no such button");
        assert!(fault.contains("triangle") && fault.contains("dpad-up"), "{fault}");
    }

    #[test]
    fn a_profile_name_nothing_speaks_for_is_left_as_it_is() {
        assert_eq!(spoken_for("LeftPaddle3"), Ok("LeftPaddle3"));
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
