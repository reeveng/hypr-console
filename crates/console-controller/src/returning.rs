//! The way back out of Game Mode, which is the one thing on the front of the
//! machine this desktop keeps while Steam has the screen.
//!
//! Legion left is what leaves for Game Mode, so holding Legion left is what
//! comes back: one button for the door, whichever side of it you are on.
//!
//! A hold, and not the press, because that button is Steam's. Taken outright
//! it would cost Game Mode its own menu, which is where the library, the power
//! and the way out of a game are, and a machine that cannot quit a game is
//! worse off than one that takes a second to leave. So the press arrives at
//! Steam untouched and this is only about what happens if it is kept down.
//!
//! Held alone, because Steam's own shortcuts are that button and another one
//! together: holding Steam and B to make a game quit is somebody staying in
//! Game Mode, and it takes longer than this does.
//!
//! Nothing here opens a device. What arrived is handed in and what to do about
//! it is handed back, the same way the rest of this crate is written.

use evdev::{EventType, KeyCode};

use crate::doing::Doing;

/// The button, which is the one the desktop's own daemon reads to leave.
pub const BUTTON: KeyCode = KeyCode::BTN_MODE;

/// How long it has to be held.
///
/// Long enough that a thumb resting on the button Steam's menu is under does
/// not take the session away, and short enough that nobody holding it wonders
/// whether the press arrived at all.
pub const HELD_SECONDS: f64 = 1.0;

/// What holding it comes to.
pub const RUNS: [&str; 1] = ["/usr/local/bin/desktop-mode"];

/// Whether the button is down, since when, and whether it is still alone.
#[derive(Debug, Default, PartialEq)]
pub struct Returning {
    since: Option<f64>,
    /// Something else was pressed while it was held, which makes this one of
    /// Steam's chords rather than a way out.
    shared: bool,
    /// Said once per hold. The session takes a moment to go, and asking twice
    /// asks a session that is already leaving to leave again.
    left: bool,
}

impl Returning {
    /// Something arrived on the pad.
    pub fn saw(&mut self, kind: EventType, code: u16, value: i32, now: f64) {
        if kind != EventType::KEY {
            return;
        }

        match (code == BUTTON.0, value) {
            (true, 1) => *self = Returning { since: Some(now), ..Returning::default() },
            (true, 0) => *self = Returning::default(),
            (false, 1) => self.shared = self.since.is_some(),
            // A key repeating is the same press, and a button released while
            // this one is held leaves the hold as shared as it already was.
            _ => (),
        }
    }

    /// The pad has gone, which a profile switch does every time.
    pub fn gone(&mut self) {
        *self = Returning::default();
    }

    /// What this moment comes to: the way back, once, or nothing.
    pub fn turn(&mut self, now: f64) -> Option<Doing> {
        let since = self.since.filter(|_| !self.shared && !self.left)?;

        if now - since < HELD_SECONDS {
            return None;
        }

        self.left = true;
        Some(Doing::run(&RUNS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The button pressed, and what it comes to a moment later.
    fn held(seconds: f64) -> Option<Doing> {
        let mut returning = Returning::default();
        returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0);
        returning.turn(1000.0 + seconds)
    }

    fn way_back() -> Option<Doing> {
        Some(Doing::run(&["/usr/local/bin/desktop-mode"]))
    }

    /// A press is Steam's: it opens Steam's menu and this says nothing about
    /// it.
    #[test]
    fn a_press_is_not_a_way_out() {
        assert_eq!(held(HELD_SECONDS / 2.0), None);
    }

    #[test]
    fn held_on_its_own_it_comes_back_to_the_desktop() {
        assert_eq!(held(HELD_SECONDS), way_back());
    }

    /// Steam's own shortcuts are this button and another one together, and the
    /// one that quits a game is held for longer than this is.
    #[test]
    fn held_with_another_button_it_is_steams_chord_and_not_a_way_out() {
        let mut returning = Returning::default();
        returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0);
        returning.saw(EventType::KEY, KeyCode::BTN_EAST.0, 1, 1000.1);
        assert_eq!(returning.turn(1000.0 + HELD_SECONDS), None);
    }

    /// A thumb on a stick is not another button. Only a chord is.
    #[test]
    fn a_stick_pushed_while_it_is_held_is_not_another_button() {
        let mut returning = Returning::default();
        returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0);
        returning.saw(EventType::ABSOLUTE, 0, 4000, 1000.1);
        assert_eq!(returning.turn(1000.0 + HELD_SECONDS), way_back());
    }

    /// The session takes a moment to go and the button is still down while it
    /// does.
    #[test]
    fn it_is_said_once_however_long_the_button_is_kept_down() {
        let mut returning = Returning::default();
        returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0);
        assert_eq!(returning.turn(1001.0), way_back());
        assert_eq!(returning.turn(1002.0), None);
        assert_eq!(returning.turn(1010.0), None);
    }

    /// Which is what a hold that came to nothing has to leave behind: a chord
    /// pressed once must not make the next hold do nothing.
    #[test]
    fn letting_go_puts_it_back_the_way_it_was() {
        let mut returning = Returning::default();
        returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0);
        returning.saw(EventType::KEY, KeyCode::BTN_EAST.0, 1, 1000.1);
        returning.saw(EventType::KEY, BUTTON.0, 0, 1000.2);
        assert_eq!(returning, Returning::default());
        returning.saw(EventType::KEY, BUTTON.0, 1, 1001.0);
        assert_eq!(returning.turn(1002.0), way_back());
    }

    /// A button pressed while nothing is being held is Steam's alone, and
    /// nothing here is watching for it.
    #[test]
    fn another_button_on_its_own_is_nothing_to_do_with_this() {
        let mut returning = Returning::default();
        returning.saw(EventType::KEY, KeyCode::BTN_EAST.0, 1, 1000.0);
        assert_eq!(returning, Returning::default());
    }

    /// A profile switch takes the pad away every time, and a button held as it
    /// went is not a button anybody is still holding.
    #[test]
    fn a_pad_that_went_away_takes_the_hold_with_it() {
        let mut returning = Returning::default();
        returning.saw(EventType::KEY, BUTTON.0, 1, 1000.0);
        returning.gone();
        assert_eq!(returning.turn(1002.0), None);
    }
}
