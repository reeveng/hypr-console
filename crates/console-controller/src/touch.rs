//! The touchpad on the right face, turned into pointer movement.
//!
//! InputPlumber cannot do this. Asked to, it answers "Translation not
//! implemented" once per event and drops them, so the pad did nothing at all
//! no matter how it was mapped. Handing it to the compositor instead makes it
//! an absolute device: touching the middle of the pad puts the cursor in the
//! middle of the screen, which is not what a pad under a thumb is for. So it
//! is read here, and what comes out is movement rather than position.


use console_number::toward_zero_i32;
use evdev::{KeyCode, RelativeAxisCode};

use crate::doing::{Doing, Out};
use crate::means::Press;

/// Screen pixels per unit of pad travel.
pub const GAIN: f64 = 1.4;

/// Seconds between reads while a finger is down.
///
/// A finger on the pad is read at the pad's own pace. Anything slower arrives
/// as a series of jumps, which reads as a slow machine.
pub const POLL: f64 = 0.008;

/// Which way round the pad is.
///
/// The pad reports in its own frame, and no part of the system says how that
/// frame sits under a thumb, so it is written down here where a wrong guess is
/// one word to fix.
pub const SWAP: bool = false;
pub const FLIP_X: bool = false;
pub const FLIP_Y: bool = false;

/// A touch shorter than this can be a tap.
pub const TAP_SECONDS: f64 = 0.25;

/// ... if it stayed within this many units.
pub const TAP_TRAVEL: i32 = 40;

/// A click, which is a press and a release and so two frames.
fn click() -> Vec<Doing> {
    vec![
        Doing::Frame(vec![Out::key(KeyCode::BTN_LEFT.0, 1)]),
        Doing::Frame(vec![Out::key(KeyCode::BTN_LEFT.0, 0)]),
    ]
}

/// A finger on the pad, and where it has been.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Finger {
    pub down: bool,
    /// Whether the pad is pressed in, which is a button of its own.
    pub held: bool,
    was: (Option<i32>, Option<i32>),
    started: f64,
    travel: i32,
    moved: (i32, i32),
    /// Movement too small to send yet.
    owed: (f64, f64),
}

/// Which way a movement on the pad is measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Across the pad.
    Sideways,
    /// Up and down it.
    Down,
}

impl Finger {
    /// A finger arriving or leaving.
    ///
    /// A touch that was short and stayed still is a tap, which is a click.
    pub fn touched(&mut self, down: Press, now: f64) -> Vec<Doing> {
        if down == Press::Down {
            *self = Finger { down: true, started: now, held: self.held, owed: self.owed, ..Finger::default() };
            return Vec::new();
        }

        self.down = false;
        let quick = now - self.started < TAP_SECONDS;

        match quick && self.travel < TAP_TRAVEL {
            true => click(),
            false => Vec::new(),
        }
    }

    /// The pad pressed in, rather than tapped. Held for as long as it is in,
    /// which is how a drag is made.
    pub fn pressed(&mut self, value: i32) -> Vec<Doing> {
        self.held = value == 1;
        vec![Doing::Frame(vec![Out::key(KeyCode::BTN_LEFT.0, value)])]
    }

    /// Where the finger is now. What matters is how far it went since the last
    /// report, not where it landed, so the first report of a touch moves
    /// nothing.
    pub fn at(&mut self, along: Axis, value: i32) {
        if !self.down {
            return;
        }

        let was = match along {
            Axis::Sideways => &mut self.was.0,
            Axis::Down => &mut self.was.1,
        };
        let step = was.map(|before| value - before);

        *was = Some(value);
        if let Some(step) = step {
            self.travel += step.abs();

            match along {
                Axis::Sideways => self.moved.0 += step,
                Axis::Down => self.moved.1 += step,
            }
        }
    }

    /// Everything the finger did since this was last asked, as movement.
    ///
    /// What the gain leaves behind is kept, or a slow drag rounds to nothing
    /// and the pointer refuses to move at all.
    pub fn carried(&mut self) -> Vec<Doing> {
        let (mut across, mut down) = self.moved;
        self.moved = (0, 0);

        if SWAP {
            (across, down) = (down, across);
        }

        if FLIP_X {
            across = -across;
        }

        if FLIP_Y {
            down = -down;
        }

        self.owed.0 += f64::from(across) * GAIN;
        self.owed.1 += f64::from(down) * GAIN;

        if self.owed.0.abs() < 1.0 && self.owed.1.abs() < 1.0 {
            return Vec::new();
        }

        let (whole_x, whole_y) = (toward_zero_i32(self.owed.0), toward_zero_i32(self.owed.1));
        self.owed.0 -= f64::from(whole_x);
        self.owed.1 -= f64::from(whole_y);
        vec![Doing::Frame(vec![
            Out::rel(RelativeAxisCode::REL_X.0, whole_x),
            Out::rel(RelativeAxisCode::REL_Y.0, whole_y),
        ])]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quick_touch_that_stayed_still_is_a_click() {
        let mut finger = Finger::default();
        assert!(finger.touched(Press::Down, 1000.0).is_empty());
        assert_eq!(finger.touched(Press::Up, 1000.1), click());
    }

    #[test]
    fn a_touch_that_lingered_is_not_a_click() {
        let mut finger = Finger::default();
        finger.touched(Press::Down, 1000.0);
        assert!(finger.touched(Press::Up, 1000.0 + TAP_SECONDS + 0.01).is_empty());
    }

    #[test]
    fn a_touch_that_travelled_is_not_a_click() {
        let mut finger = Finger::default();
        finger.touched(Press::Down, 1000.0);
        finger.at(Axis::Sideways, 0);
        finger.at(Axis::Sideways, TAP_TRAVEL + 1);
        assert!(finger.touched(Press::Up, 1000.1).is_empty());
    }

    /// Position in, movement out. The first report of a touch says where the
    /// finger landed, which is not a distance from anywhere.
    #[test]
    fn the_first_report_of_a_touch_moves_nothing() {
        let mut finger = Finger::default();
        finger.touched(Press::Down, 1000.0);
        finger.at(Axis::Sideways, 800);
        assert!(finger.carried().is_empty());
    }

    #[test]
    fn a_finger_that_moved_moves_the_pointer_by_the_gain() {
        let mut finger = Finger::default();
        finger.touched(Press::Down, 1000.0);
        finger.at(Axis::Sideways, 100);
        finger.at(Axis::Sideways, 200);
        assert_eq!(
            finger.carried(),
            [Doing::Frame(vec![
                Out::rel(RelativeAxisCode::REL_X.0, (100.0 * GAIN) as i32),
                Out::rel(RelativeAxisCode::REL_Y.0, 0),
            ])]
        );
    }

    /// A slow drag would round to nothing every read, and the pointer would
    /// refuse to move at all.
    #[test]
    fn what_the_gain_leaves_behind_is_kept() {
        let mut finger = Finger::default();
        finger.touched(Press::Down, 1000.0);
        finger.at(Axis::Sideways, 0);
        let over: usize = (1..=4)
            .map(|step| {
                finger.at(Axis::Sideways, step);
                finger.carried().len()
            })
            .sum();
        assert!(over > 0, "four units at a gain of {GAIN} is more than one pixel");
    }

    #[test]
    fn pressing_the_pad_in_holds_the_button_down() {
        let mut finger = Finger::default();
        assert_eq!(finger.pressed(1), [Doing::Frame(vec![Out::key(KeyCode::BTN_LEFT.0, 1)])]);
        assert!(finger.held);
        finger.pressed(0);
        assert!(!finger.held);
    }

    /// Nothing is reported while no finger is down, so a stray axis report
    /// between touches cannot move the pointer.
    #[test]
    fn a_report_with_no_finger_down_is_nothing() {
        let mut finger = Finger::default();
        finger.at(Axis::Sideways, 500);
        finger.at(Axis::Sideways, 900);
        assert!(finger.carried().is_empty());
    }
}
