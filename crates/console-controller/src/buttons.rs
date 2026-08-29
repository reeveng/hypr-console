//! Button to action.
//!
//! The daemon is paused while the on-screen keyboard is up, so these never
//! compete with it for the same press.
//!
//! One button, one thing. Nothing here appears twice: a button that shares its
//! job with another button is a button that could have been doing something
//! nothing else does.

use evdev::KeyCode;

use crate::doing::Doing;

/// The back of the device, and Legion right.
///
/// Each arrives as a key of its own on the keyboard InputPlumber publishes, so
/// which button was pressed is a fact rather than a guess about button codes.
///
/// The paddles mean the same thing in every profile: the left one opens, the
/// right one closes, and what is open or closed is worked out here where the
/// screen can be seen. Written into the profiles instead, they meant one thing
/// while a chooser was up and another while it was not, and the pad changes
/// profile a beat after the screen changes: pressed in that beat, the left
/// paddle reached nothing and the right paddle closed the window behind the
/// menu that had just opened.
pub const KEYS: [(KeyCode, &[&str]); 5] = [
    (KeyCode::KEY_F13, &["launcher", "--keep"]),
    (KeyCode::KEY_F14, &["dictate"]),
    (KeyCode::KEY_F15, &["put-away"]),
    (KeyCode::KEY_F16, &["/usr/local/bin/console-screenshot"]),
    (KeyCode::KEY_F17, &["settings-panel"]),
];

/// Held with L2, a button does its second thing.
///
/// Brightness lives here rather than on a button of its own: the front of the
/// device is for what you reach for without thinking, and how bright the
/// screen is is not that.
pub const CARRIED_KEYS: [(KeyCode, &[&str]); 2] = [
    (KeyCode::KEY_LEFT, &["/usr/local/bin/console-brightness", "down"]),
    (KeyCode::KEY_RIGHT, &["/usr/local/bin/console-brightness", "up"]),
];

/// The front of the device.
///
/// Legion left leaves the desktop for Game Mode, which is the one thing here
/// that is not a part of this desktop at all, so it has a button of its own
/// and no second job. View opens the browser, which used to be on the bottom
/// left paddle and gave it up: a paddle is where a hand already is while it is
/// speaking, and opening a browser is a thing done once and then let go of.
pub const BUTTONS: [(KeyCode, &[&str]); 3] = [
    (KeyCode::BTN_MODE, &["game-mode"]),
    (KeyCode::BTN_SELECT, &["/usr/local/bin/console-browser"]),
    (KeyCode::BTN_START, &["/usr/local/bin/console-buttons", "--menu"]),
];

/// The shoulders, and which way each one goes.
pub const SHOULDERS: [(KeyCode, &str); 2] =
    [(KeyCode::BTN_TL, "-1"), (KeyCode::BTN_TR, "+1")];

fn found(table: &[(KeyCode, &'static [&'static str])], code: u16) -> Option<Doing> {
    table.iter().find(|(key, _)| key.0 == code).map(|(_, argv)| Doing::run(argv))
}

/// What a key on the published keyboard does, held with L2 or not.
///
/// The carried table is asked first, so a key that has a second job does that
/// one and not both.
pub fn on_keyboard(code: u16, carrying: bool) -> Option<Doing> {
    match carrying {
        true => found(&CARRIED_KEYS, code).or_else(|| found(&KEYS, code)),
        false => found(&KEYS, code),
    }
}

/// What a button on the pad does.
pub fn on_pad(code: u16, carrying: bool) -> Option<Doing> {
    SHOULDERS
        .iter()
        .find(|(key, _)| key.0 == code)
        .map(|(_, where_)| Doing::workspace(where_, carrying))
        .or_else(|| found(&BUTTONS, code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_back_button_runs_what_it_is_for() {
        assert_eq!(on_keyboard(KeyCode::KEY_F13.0, false), Some(Doing::run(&["launcher", "--keep"])));
        assert_eq!(on_keyboard(KeyCode::KEY_F17.0, false), Some(Doing::run(&["settings-panel"])));
    }

    #[test]
    fn a_key_with_a_second_job_does_that_one_when_l2_is_held() {
        assert_eq!(on_keyboard(KeyCode::KEY_LEFT.0, false), None);
        assert_eq!(
            on_keyboard(KeyCode::KEY_LEFT.0, true),
            Some(Doing::run(&["/usr/local/bin/console-brightness", "down"]))
        );
    }

    /// A key with no second job keeps doing its first one while L2 is held.
    #[test]
    fn holding_l2_does_not_take_away_what_a_key_already_did() {
        assert_eq!(on_keyboard(KeyCode::KEY_F13.0, true), Some(Doing::run(&["launcher", "--keep"])));
    }

    #[test]
    fn the_shoulders_move_between_workspaces_and_carry_when_held() {
        assert_eq!(on_pad(KeyCode::BTN_TR.0, false), Some(Doing::workspace("+1", false)));
        assert_eq!(on_pad(KeyCode::BTN_TL.0, true), Some(Doing::workspace("-1", true)));
    }

    /// The browser, wherever L2 is, because a button on the front of the
    /// machine means one thing and holding a trigger is not a second machine.
    #[test]
    fn view_opens_the_browser() {
        let browser = Some(Doing::run(&["/usr/local/bin/console-browser"]));
        assert_eq!(on_pad(KeyCode::BTN_SELECT.0, false), browser);
        assert_eq!(on_pad(KeyCode::BTN_SELECT.0, true), browser);
    }

    /// The paddle a hand is already on while it is speaking.
    #[test]
    fn the_bottom_left_paddle_takes_what_is_said() {
        assert_eq!(on_keyboard(KeyCode::KEY_F14.0, false), Some(Doing::run(&["dictate"])));
    }

    /// One button, one thing. A button in two tables is a button that could
    /// have been doing something nothing else does.
    #[test]
    fn nothing_is_written_down_twice() {
        let mut every: Vec<u16> = KEYS
            .iter()
            .chain(CARRIED_KEYS.iter())
            .chain(BUTTONS.iter())
            .map(|(key, _)| key.0)
            .chain(SHOULDERS.iter().map(|(key, _)| key.0))
            .collect();
        let many = every.len();
        every.sort_unstable();
        every.dedup();
        assert_eq!(every.len(), many);
    }
}
