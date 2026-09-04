//! How each button reaches the daemon: one button, one thing that arrives.
//!
//! The profile used to say what a button meant. It says what a button *is*
//! now, and nothing else: every button on the front and back of the machine
//! goes to something the daemon can tell apart from every other button, and
//! what any of it means is decided in one table by the one program that can
//! see the screen, the layers and the person's own answers.
//!
//! Three ways to arrive, because the emulated pad can say some of it and not
//! the rest.
//!
//!   * The face buttons, the shoulders, the stick presses and the three named
//!     buttons on the front pass through as themselves. They are gamepad
//!     buttons and InputPlumber has a name for each.
//!   * The d-pad arrives as the hat it is. A hat is two axes with three
//!     positions each, which is four buttons, and the daemon reads them as
//!     four buttons.
//!   * The paddles, Legion right, the button with a keyboard drawn on it and X
//!     arrive as keys. The first four because the emulated pad has four
//!     interchangeable paddle codes and nothing anywhere records which paddle
//!     is which -- guessing it once put the brightness on the wrong side of
//!     the machine. X because the on-screen keyboard's own fork reads the pad
//!     for `North` and toggles itself on it, and a press that both toggled the
//!     keyboard and told the daemon to toggle it would open and shut it in one
//!     press.
//!
//! The keys are F13 to F22, without F18. Nothing on a real keyboard sends
//! them, nothing in this desktop, GTK or the compositor binds them, and none
//! of them types a letter into whatever holds the focus -- which matters more
//! here than it did when only the paddles used them, because now every press
//! of those buttons sends one.
//!
//! F18 is the hole in the run, and it is deliberate. A panel listens for F18
//! as "what else can be done with this row", so it is the one function key in
//! the range that means something to something. A button routed onto it would
//! be a button that opened the row menu wherever a panel was up.

use evdev::{AbsoluteAxisCode, KeyCode};

/// What arrives when a button is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrives {
    /// On the pad, as the gamepad button it is.
    Pad(KeyCode),
    /// On the keyboard InputPlumber publishes, as a key nothing listens for.
    Keys(KeyCode),
    /// On the pad, as one end of one of the hat's two axes.
    Hat(AbsoluteAxisCode, i32),
}

/// Every button this repository has a word for, and what it arrives as.
///
/// Written in the profiles' own names, because that is what the profile has to
/// say to route it and what the machine answers in when it is asked what it
/// has. `vocabulary::spoken_for` is what turns them into the words on the
/// machine.
///
/// A device with a button this repository has no name for cannot be routed to
/// the daemon, and so cannot have a job put on it. That is a real limit and it
/// is written here rather than discovered: the two on this handheld nobody has
/// a word for -- `QuickAccess2` and `RightPaddle3` -- are in the table under
/// InputPlumber's names for exactly that reason.
pub const ROUTE: [(&str, Arrives); 23] = [
    // The face buttons, the shoulders and the stick presses, as themselves.
    ("South", Arrives::Pad(KeyCode::BTN_SOUTH)),
    ("East", Arrives::Pad(KeyCode::BTN_EAST)),
    ("West", Arrives::Pad(KeyCode::BTN_WEST)),
    ("LeftBumper", Arrives::Pad(KeyCode::BTN_TL)),
    ("RightBumper", Arrives::Pad(KeyCode::BTN_TR)),
    ("LeftStick", Arrives::Pad(KeyCode::BTN_THUMBL)),
    ("RightStick", Arrives::Pad(KeyCode::BTN_THUMBR)),
    ("Start", Arrives::Pad(KeyCode::BTN_START)),
    ("Select", Arrives::Pad(KeyCode::BTN_SELECT)),
    ("Guide", Arrives::Pad(KeyCode::BTN_MODE)),
    // The d-pad, as the hat it is.
    ("DPadUp", Arrives::Hat(AbsoluteAxisCode::ABS_HAT0Y, -1)),
    ("DPadDown", Arrives::Hat(AbsoluteAxisCode::ABS_HAT0Y, 1)),
    ("DPadLeft", Arrives::Hat(AbsoluteAxisCode::ABS_HAT0X, -1)),
    ("DPadRight", Arrives::Hat(AbsoluteAxisCode::ABS_HAT0X, 1)),
    // The back of the machine, Legion right, and the two nobody has held.
    ("LeftPaddle1", Arrives::Keys(KeyCode::KEY_F13)),
    ("LeftPaddle2", Arrives::Keys(KeyCode::KEY_F14)),
    ("RightPaddle1", Arrives::Keys(KeyCode::KEY_F15)),
    ("RightPaddle2", Arrives::Keys(KeyCode::KEY_F16)),
    ("QuickAccess", Arrives::Keys(KeyCode::KEY_F17)),
    ("QuickAccess2", Arrives::Keys(KeyCode::KEY_F19)),
    ("RightPaddle3", Arrives::Keys(KeyCode::KEY_F20)),
    ("Keyboard", Arrives::Keys(KeyCode::KEY_F21)),
    // And the one that is a face button and is not passed through with them.
    ("North", Arrives::Keys(KeyCode::KEY_F22)),
];

/// What a button arrives as, if this desktop can route it at all.
pub fn arrives(profile_name: &str) -> Option<Arrives> {
    ROUTE.iter().find(|(named, _)| *named == profile_name).map(|(_, how)| *how)
}

/// Which button a key on the published keyboard is.
pub fn button_of_key(code: u16) -> Option<&'static str> {
    ROUTE
        .iter()
        .find(|(_, how)| matches!(how, Arrives::Keys(key) if key.0 == code))
        .map(|(named, _)| *named)
}

/// Which button a gamepad button is.
pub fn button_of_pad(code: u16) -> Option<&'static str> {
    ROUTE
        .iter()
        .find(|(_, how)| matches!(how, Arrives::Pad(key) if key.0 == code))
        .map(|(named, _)| *named)
}

/// Which button one end of the hat is.
pub fn button_of_hat(code: u16, value: i32) -> Option<&'static str> {
    ROUTE
        .iter()
        .find(|(_, how)| matches!(how, Arrives::Hat(axis, end) if axis.0 == code && *end == value))
        .map(|(named, _)| *named)
}

/// Whether a name is one of the hat's two axes.
pub fn is_hat(code: u16) -> Hat {
    let found = ROUTE.iter().any(|(_, how)| matches!(how, Arrives::Hat(axis, _) if axis.0 == code));

    match found {
        true => Hat::Axis,
        false => Hat::NotAnAxis,
    }
}

/// Whether a code is one of the hat's two axes.
///
/// The d-pad arrives as two axes with three positions each rather than as four
/// buttons, so a code that is one of them is read a different way entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hat {
    /// One of the two, so the value says which way rather than how far.
    Axis,
    /// Any other axis, read as itself.
    NotAnAxis,
}

/// The mapping this button needs in the profile, or nothing where it needs
/// none.
///
/// A hat is passed through by naming the same button on the way out, which is
/// what the emulated pad publishes it as. A key is a key. There is no third
/// shape: a button that arrives as itself still has to be named, because a
/// profile that leaves a source out leaves what happens to it to InputPlumber.
pub fn mapping(profile_name: &str) -> Option<String> {
    let how = arrives(profile_name)?;
    let (said, target) = match how {
        Arrives::Keys(key) => {
            let named = key_named(key)?;
            (format!("as {named}"), format!("      - keyboard: {named}\n"))
        }
        Arrives::Pad(_) => (
            "as itself".to_string(),
            format!("      - gamepad:\n          button: {profile_name}\n"),
        ),
        Arrives::Hat(_, _) => (
            "as one end of the hat".to_string(),
            format!("      - gamepad:\n          button: {profile_name}\n"),
        ),
    };
    // Named for what it turns into rather than for what it does, because what
    // it does is not this file's to say any more. Somebody reading the profile
    // on the machine can still see at a glance which button arrives how, which
    // is the only question it can answer.
    Some(format!(
        "  - name: {profile_name} - {said}\n\
         \x20   source_event:\n      gamepad:\n        button: {profile_name}\n\
         \x20   target_events:\n{target}\n"
    ))
}

/// A key, in the shape InputPlumber writes one.
///
/// `KEY_F13` is `KeyF13`. Only the keys this table lends out, because a name
/// InputPlumber does not take is a button that never arrives.
fn key_named(key: KeyCode) -> Option<String> {
    let said = format!("{key:?}");
    let tail = said.strip_prefix("KEY_")?;
    let mut letters = tail.chars();
    let first = letters.next()?;
    Some(format!("Key{}{}", first, letters.as_str().to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vocabulary::{BUTTONS, key_code};

    /// Every button this repository has a word for is routed somewhere. A
    /// button in the vocabulary and not in here is a button somebody can name,
    /// look for on the setup screen, and never reach.
    #[test]
    fn every_button_this_desktop_names_arrives_somewhere() {
        for (spoken, profile_name) in BUTTONS {
            assert!(arrives(profile_name).is_some(), "{spoken} ({profile_name}) arrives nowhere");
        }
    }

    /// Nothing arrives as anything else. Two buttons on one code is two
    /// buttons the daemon cannot tell apart, which is the whole of what this
    /// table is for.
    #[test]
    fn no_two_buttons_arrive_the_same_way() {
        let mut every: Vec<String> = ROUTE.iter().map(|(_, how)| format!("{how:?}")).collect();
        let many = every.len();
        every.sort();
        every.dedup();
        assert_eq!(every.len(), many);
    }

    #[test]
    fn a_press_is_read_back_as_the_button_it_came_from() {
        assert_eq!(button_of_keys(KeyCode::KEY_F13), Some("LeftPaddle1"));
        assert_eq!(button_of_pad(KeyCode::BTN_SOUTH.0), Some("South"));
        assert_eq!(button_of_hat(AbsoluteAxisCode::ABS_HAT0Y.0, -1), Some("DPadUp"));
        assert_eq!(button_of_hat(AbsoluteAxisCode::ABS_HAT0Y.0, 0), None, "the middle is no button");
    }

    fn button_of_keys(key: KeyCode) -> Option<&'static str> {
        button_of_key(key.0)
    }

    /// The keys are named in a shape InputPlumber takes and the kernel gives
    /// back. A name only one of the two knows is a button that never arrives.
    #[test]
    fn every_key_is_named_in_a_way_both_ends_know() {
        for (_, how) in ROUTE {
            let Arrives::Keys(key) = how else { continue };
            let said = key_named(key).expect("a name");
            assert_eq!(key_code(&said), Ok(key), "{said}");
        }
    }

    /// The mapping it writes is the mapping the profile needs, in both shapes.
    #[test]
    fn a_button_is_routed_as_a_key_or_as_itself() {
        let paddle = mapping("LeftPaddle1").expect("a mapping");
        assert!(paddle.contains("button: LeftPaddle1"), "{paddle}");
        assert!(paddle.contains("- keyboard: KeyF13"), "{paddle}");
        // And it says which, because that is the one question the file can
        // answer about a button now.
        assert!(paddle.contains("LeftPaddle1 - as KeyF13"), "{paddle}");
        let face = mapping("South").expect("a mapping");
        assert!(face.contains("- gamepad:\n          button: South"), "{face}");
        assert!(face.contains("South - as itself"), "{face}");
        assert_eq!(mapping("NoSuchButton"), None);
    }
}
