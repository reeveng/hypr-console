//! What the pad asked the keyboard to do.
//!
//! The on-screen keyboard reads the pad itself while it is up, and that is a
//! contract rather than an accident: `console_controller::mode::Mode::Keyboard`
//! is the daemon standing down for exactly as long as the keyboard's layer
//! surface is on the screen, and `/etc/inputplumber/profiles/keyboard.yaml` is
//! the profile the pad wears meanwhile. Two readers of one device would both
//! act on the right stick, which navigates and scrolls at once and flickers.
//!
//! Nothing here opens a device, in the same way and for the same reason as
//! `console_controller`: what arrives is handed in and what to do about it is
//! handed back, so every decision can be asked of it twice and answered the
//! same way. The binary is the only part that touches the kernel.
//!
//! Which button is which is `console_pad::vocabulary`'s to say and not this
//! file's. The trap it exists for is live here: the button labelled X on this
//! device is `BTN_NORTH` and the one labelled Y is `BTN_WEST`, and X is the
//! button that puts the keyboard on the screen.

use std::time::{Duration, Instant};

use console_pad::vocabulary;
use evdev::{AbsoluteAxisCode, KeyCode};

/// Before a held direction starts repeating, and between repeats after that.
///
/// A thumb that holds left wants to cross the keyboard, and one that taps left
/// wants one key. The delay is what tells them apart.
pub const BEFORE_REPEAT: Duration = Duration::from_millis(350);
pub const BETWEEN_REPEATS: Duration = Duration::from_millis(90);

/// How far a stick has to leave the middle to count as pushed.
///
/// A stick at rest does not report zero, and a keyboard that believed it would
/// walk on its own.
const DEADZONE: f64 = 0.5;

/// What the pad asked for, named after the intent rather than the button.
///
/// Which button carries which intent is a layout decision and lives in the
/// table below; what the keyboard does about it does not care.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    Up,
    Down,
    Left,
    Right,
    /// Type the selected key.
    Press,
    Backspace,
    Enter,
    /// Hold shift, the way the shift key does.
    Shift,
    /// Put the keyboard on the screen, or take it away.
    ///
    /// The one button that means something while the keyboard is *not* up, and
    /// the reason this module is read even then. It is read from the pad
    /// rather than bound in the compositor because binding it there never held:
    /// function keys did not resolve to a keysym Hyprland would match, raw
    /// keycodes were stored as literal strings, and a modifier plus a letter
    /// races because InputPlumber emits the pair in one event frame and the
    /// letter can reach the focused window before the modifier applies. That
    /// is how pressing X once typed a `k` into a terminal.
    Toggle,
    /// The language before this one, and the one after.
    PreviousLanguage,
    NextLanguage,
}

impl Asked {
    /// Which way this moves a selection, if it moves one at all.
    pub fn direction(self) -> Option<(i32, i32)> {
        match self {
            Asked::Up => Some((0, -1)),
            Asked::Down => Some((0, 1)),
            Asked::Left => Some((-1, 0)),
            Asked::Right => Some((1, 0)),
            _ => None,
        }
    }

    /// Whether holding it goes on asking. A direction repeats; typing does not.
    pub fn repeats(self) -> Repeats {
        match self.direction() {
            Some(_) => Repeats::Held,
            None => Repeats::Once,
        }
    }
}

/// Whether holding a thing down goes on asking for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeats {
    /// Held, it asks again: a direction walks a selection along.
    Held,
    /// Once, however long it is held down.
    Once,
}

/// Which way a button just went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Went {
    /// In. Everything a button means happens on the way down.
    Down,
    /// Out again, which asks for nothing.
    Up,
}

/// Which way a button went, out of the number the kernel reports.
pub fn pushed(value: i32) -> Went {
    match value {
        0 => Went::Up,
        _ => Went::Down,
    }
}

/// Which button means what, in the names a person uses for them.
///
/// Written in `console_pad::vocabulary`'s words rather than in kernel codes, so
/// that the button called X here is the button called X in a profile and in
/// the card that asks which button you just pressed.
const BUTTONS: [(&str, Asked); 8] = [
    ("a", Asked::Press),
    ("b", Asked::Backspace),
    ("x", Asked::Toggle),
    ("y", Asked::Shift),
    ("menu", Asked::Enter),
    ("l1", Asked::PreviousLanguage),
    ("r1", Asked::NextLanguage),
    // Pressing a stick in is a button too, and while a thumb is already resting
    // on one to move about, it is the nearest thing to hand.
    ("l3", Asked::Press),
];

/// What a button press comes to, on the way down. A release asks for nothing:
/// every one of these happens when the button goes in.
pub fn from_button(code: KeyCode, down: Went) -> Option<Asked> {
    if down == Went::Up {
        return None;
    }

    BUTTONS
        .iter()
        .find(|(name, _)| code_of(name) == Some(code))
        .map(|(_, asked)| *asked)
        // r3 is the other stick, and `vocabulary` names both, so it is easier
        // to say here than to write the same line twice above.
        .or_else(|| (code == KeyCode::BTN_THUMBR).then_some(Asked::Press))
}

/// The pad code one of the names in `BUTTONS` arrives as.
///
/// Every name in that table is one `vocabulary` carries, and the test below
/// says so, so a name it does not know is the two having been edited apart
/// rather than a button somebody pressed. It is answered the way an unknown
/// button is -- nothing matches it and the press does nothing -- because the
/// alternative is a keyboard that will not start over a table it could have
/// gone on without.
fn code_of(name: &str) -> Option<KeyCode> {
    match vocabulary::button_name(name) {
        Ok(named) => vocabulary::gamepad_code(named),
        Err(_) => None,
    }
}

/// What the d-pad comes to.
///
/// The d-pad is not four buttons on this pad. An xbox pad publishes it as a
/// hat -- two axes with three positions each -- which is what InputPlumber
/// emits, and a keyboard that waited for `BTN_DPAD_UP` would wait forever.
/// `console_pad::vocabulary::HAT_CODES` is where that is written down.
pub fn from_hat(axis: AbsoluteAxisCode, value: i32) -> Option<Asked> {
    if value == 0 {
        return None;
    }

    match (axis, value < 0) {
        (AbsoluteAxisCode::ABS_HAT0X, true) => Some(Asked::Left),
        (AbsoluteAxisCode::ABS_HAT0X, false) => Some(Asked::Right),
        (AbsoluteAxisCode::ABS_HAT0Y, true) => Some(Asked::Up),
        (AbsoluteAxisCode::ABS_HAT0Y, false) => Some(Asked::Down),
        _ => None,
    }
}

/// What a stick comes to: a direction, or nothing while it is near the middle.
///
/// Either stick moves between keys. It is an analogue axis and this is a grid
/// of keys, so it is read as a direction that is either pushed or not -- whole
/// keys, never pixels. `range` is the axis's own low and high, which the device
/// reports and which differ between pads.
pub fn from_stick(axis: AbsoluteAxisCode, value: i32, range: (i32, i32)) -> Option<Asked> {
    let (low, high) = range;
    let span = match high > low {
        true => f64::from(high - low) / 2.0,
        false => 1.0,
    };
    let pushed = (f64::from(value) - (f64::from(low) + span)) / span;

    if pushed.abs() < DEADZONE {
        return None;
    }

    match axis {
        AbsoluteAxisCode::ABS_X | AbsoluteAxisCode::ABS_RX => match pushed < 0.0 {
            true => Some(Asked::Left),
            false => Some(Asked::Right),
        },
        AbsoluteAxisCode::ABS_Y | AbsoluteAxisCode::ABS_RY => match pushed < 0.0 {
            true => Some(Asked::Up),
            false => Some(Asked::Down),
        },
        _ => None,
    }
}

/// A direction being held, and when it is next due to repeat.
///
/// A stick pushed left reports once and then says nothing until it moves, so a
/// keyboard that acted only on events would move one key and stop. This is the
/// part that keeps asking while the thumb stays where it is.
///
/// The clock is handed in rather than read, so that the delay and the rate can
/// be tested without waiting for them.
#[derive(Debug, Default)]
pub struct Held {
    what: Option<Asked>,
    /// When the next repeat is due. `None` while nothing is held.
    due: Option<Instant>,
}

impl Held {
    /// Take what the pad just said, and answer with what to act on.
    ///
    /// `None` from the pad means the stick came back to the middle or the hat
    /// let go, which stops the repeat. The same direction arriving twice is the
    /// stick reporting a value it has already reported, and asks for nothing:
    /// the repeat is what moves it now.
    pub fn went(&mut self, asked: Option<Asked>, now: Instant) -> Option<Asked> {
        match asked {
            None => {
                self.what = None;
                self.due = None;
                None
            },
            Some(asked) if asked.repeats() == Repeats::Once => Some(asked),
            Some(asked) if self.what == Some(asked) => None,
            Some(asked) => {
                self.what = Some(asked);
                self.due = Some(now + BEFORE_REPEAT);
                Some(asked)
            },
        }
    }

    /// How long the loop may sleep before the next repeat is due, or `None`
    /// when nothing is held and it may wait for the compositor as long as it
    /// likes.
    pub fn until(&self, now: Instant) -> Option<Duration> {
        self.due.map(|due| due.saturating_duration_since(now))
    }

    /// The repeat that is due now, if one is.
    pub fn due(&mut self, now: Instant) -> Option<Asked> {
        let due = self.due?;

        if now < due {
            return None;
        }

        self.due = Some(now + BETWEEN_REPEATS);
        self.what
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one thing about this pad worth testing twice: X is north. The
    /// button that raises the keyboard is the one a person points at, and
    /// getting it wrong puts the keyboard on Y.
    #[test]
    fn the_button_labelled_x_is_the_one_that_raises_the_keyboard() {
        assert_eq!(from_button(KeyCode::BTN_NORTH, Went::Down), Some(Asked::Toggle));
        assert_eq!(from_button(KeyCode::BTN_WEST, Went::Down), Some(Asked::Shift));
        assert_eq!(from_button(KeyCode::BTN_SOUTH, Went::Down), Some(Asked::Press));
        assert_eq!(from_button(KeyCode::BTN_EAST, Went::Down), Some(Asked::Backspace));
    }

    /// A button asks on the way down and not on the way back up, or every
    /// press would type twice.
    #[test]
    fn a_button_asks_once_and_on_the_way_down() {
        assert_eq!(from_button(KeyCode::BTN_SOUTH, Went::Up), None);
    }

    /// The d-pad arrives as a hat on this pad, not as four buttons.
    #[test]
    fn the_dpad_is_a_hat() {
        assert_eq!(from_hat(AbsoluteAxisCode::ABS_HAT0Y, -1), Some(Asked::Up));
        assert_eq!(from_hat(AbsoluteAxisCode::ABS_HAT0X, 1), Some(Asked::Right));
        assert_eq!(from_hat(AbsoluteAxisCode::ABS_HAT0X, 0), None, "let go");
        assert_eq!(from_button(KeyCode::BTN_DPAD_UP, Went::Down), None, "not a button here");
    }

    /// What `code_of` is written on: a name in this table that `vocabulary`
    /// does not carry is the two having been edited apart, and it would go
    /// unnoticed as a button that quietly stopped working.
    #[test]
    fn every_button_this_table_names_is_one_the_vocabulary_carries() {
        for (name, _) in BUTTONS {
            assert!(code_of(name).is_some(), "{name} is not a button `vocabulary` knows");
        }
    }

    /// A stick at rest is not a stick being pushed, whatever number it reports.
    #[test]
    fn a_stick_near_the_middle_asks_for_nothing() {
        let range = (0, 255);
        assert_eq!(from_stick(AbsoluteAxisCode::ABS_X, 128, range), None);
        assert_eq!(from_stick(AbsoluteAxisCode::ABS_X, 140, range), None);
        assert_eq!(from_stick(AbsoluteAxisCode::ABS_X, 255, range), Some(Asked::Right));
        assert_eq!(from_stick(AbsoluteAxisCode::ABS_X, 0, range), Some(Asked::Left));
        assert_eq!(from_stick(AbsoluteAxisCode::ABS_RY, 0, range), Some(Asked::Up));
    }

    /// A stick that goes on reporting the direction it is already pushed in
    /// asks for nothing: the repeat is what moves it after the first key.
    #[test]
    fn a_direction_already_held_does_not_ask_again() {
        let now = Instant::now();
        let mut held = Held::default();
        assert_eq!(held.went(Some(Asked::Left), now), Some(Asked::Left));
        assert_eq!(held.went(Some(Asked::Left), now), None);
        assert_eq!(held.went(Some(Asked::Right), now), Some(Asked::Right), "a turn is a new ask");
    }

    /// Holding waits, and then repeats at a rate. Both are measured from the
    /// clock handed in, so this test takes no time at all.
    #[test]
    fn a_held_direction_waits_then_repeats() {
        let start = Instant::now();
        let mut held = Held::default();
        held.went(Some(Asked::Down), start);
        assert_eq!(held.due(start), None, "not yet");
        assert_eq!(held.due(start + BEFORE_REPEAT - Duration::from_millis(1)), None);
        assert_eq!(held.due(start + BEFORE_REPEAT), Some(Asked::Down), "the first repeat");
        let then = start + BEFORE_REPEAT;
        assert_eq!(held.due(then), None, "and not again immediately");
        assert_eq!(held.due(then + BETWEEN_REPEATS), Some(Asked::Down));
    }

    /// Letting go stops it, and stops the loop waking for it.
    #[test]
    fn letting_go_stops_the_repeat() {
        let now = Instant::now();
        let mut held = Held::default();
        held.went(Some(Asked::Up), now);
        assert!(held.until(now).is_some(), "something to wake for");
        assert_eq!(held.went(None, now), None);
        assert_eq!(held.due(now + BEFORE_REPEAT * 4), None);
        assert_eq!(held.until(now), None, "and nothing to wake for");
    }

    /// Typing does not repeat: a held A types one letter, not forty.
    #[test]
    fn a_press_is_not_a_thing_that_repeats() {
        let now = Instant::now();
        let mut held = Held::default();
        assert_eq!(held.went(Some(Asked::Press), now), Some(Asked::Press));
        assert_eq!(held.went(Some(Asked::Press), now), Some(Asked::Press), "still not a repeat");
        assert_eq!(held.until(now), None);
    }
}
