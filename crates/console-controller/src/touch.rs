//! The touchpad on the right face, turned into pointer movement.
//!
//! InputPlumber cannot do this. Asked to, it answers "Translation not
//! implemented" once per event and drops them, so the pad did nothing at all
//! no matter how it was mapped. Handing it to the compositor instead makes it
//! an absolute device: touching the middle of the pad puts the cursor in the
//! middle of the screen, which is not what a pad under a thumb is for. So it
//! is read here, and what comes out is movement rather than position.


use console_never::Never;
use console_number_conversion::toward_zero_i32;
use evdev::{KeyCode, RelativeAxisCode};

use crate::doing::{Doing, Out};
use crate::means::Press;

pub const GAIN: f64 = 1.4;

pub const POLL: f64 = 0.008;

pub const SWAP: bool = false;
pub const FLIP_X: bool = false;
pub const FLIP_Y: bool = false;

pub const TAP_SECONDS: f64 = 0.25;

pub const TAP_TRAVEL: i32 = 40;

fn click() -> Result<Vec<Doing>, Never> {
    let Ok(down) = Out::key(KeyCode::BTN_LEFT.0, 1);
    let Ok(up) = Out::key(KeyCode::BTN_LEFT.0, 0);

    Ok(vec![Doing::Frame(vec![down]), Doing::Frame(vec![up])])
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Finger {
    pub down: bool,
    pub held: bool,
    was: (Option<i32>, Option<i32>),
    started: f64,
    travel: i32,
    moved: (i32, i32),
    owed: (f64, f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Sideways,
    Down,
}

impl Finger {
    pub fn touched(&mut self, down: Press, now: f64) -> Result<Vec<Doing>, Never> {
        match down {
            Press::Down => {
                *self = Finger {
                    down: true,
                    started: now,
                    held: self.held,
                    owed: self.owed,
                    ..Finger::default()
                };

                return Ok(Vec::new());
            }
            Press::Up => {},
        }

        self.down = false;
        let quick = now - self.started < TAP_SECONDS;

        match quick && self.travel < TAP_TRAVEL {
            true => click(),
            false => Ok(Vec::new()),
        }
    }

    pub fn pressed(&mut self, value: i32) -> Result<Vec<Doing>, Never> {
        self.held = value == 1;

        let Ok(out) = Out::key(KeyCode::BTN_LEFT.0, value);

        Ok(vec![Doing::Frame(vec![out])])
    }

    pub fn at(&mut self, along: Axis, value: i32) -> Result<(), Never> {
        match self.down {
            true => {},
            false => return Ok(()),
        }

        let was = match along {
            Axis::Sideways => &mut self.was.0,
            Axis::Down => &mut self.was.1,
        };
        let step = was.map(|before| value.saturating_sub(before));

        *was = Some(value);
        match step {
            Some(step) => {
                self.travel = self.travel.saturating_add(step.saturating_abs());

                match along {
                    Axis::Sideways => self.moved.0 = self.moved.0.saturating_add(step),
                    Axis::Down => self.moved.1 = self.moved.1.saturating_add(step),
                }
            }
            None => {},
        }

        Ok(())
    }

    pub fn carried(&mut self) -> Result<Vec<Doing>, Never> {
        let (mut across, mut down) = self.moved;
        self.moved = (0, 0);

        match SWAP {
            true => (across, down) = (down, across),
            false => {},
        }

        match FLIP_X {
            true => across = across.saturating_neg(),
            false => {},
        }

        match FLIP_Y {
            true => down = down.saturating_neg(),
            false => {},
        }

        self.owed.0 += f64::from(across) * GAIN;
        self.owed.1 += f64::from(down) * GAIN;

        match self.owed.0.abs() < 1.0 && self.owed.1.abs() < 1.0 {
            true => return Ok(Vec::new()),
            false => {},
        }

        let Ok(whole_x) = toward_zero_i32(self.owed.0);
        let Ok(whole_y) = toward_zero_i32(self.owed.1);

        self.owed.0 -= f64::from(whole_x);
        self.owed.1 -= f64::from(whole_y);

        let Ok(across) = Out::rel(RelativeAxisCode::REL_X.0, whole_x);
        let Ok(down) = Out::rel(RelativeAxisCode::REL_Y.0, whole_y);

        Ok(vec![Doing::Frame(vec![across, down])])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    #[test]
    fn a_quick_touch_that_stayed_still_is_a_click() {
        let mut finger = Finger::default();
        assert!(ok(finger.touched(Press::Down, 1000.0)).is_empty());
        assert_eq!(ok(finger.touched(Press::Up, 1000.1)), ok(click()));
    }

    #[test]
    fn a_touch_that_lingered_is_not_a_click() {
        let mut finger = Finger::default();
        ok(finger.touched(Press::Down, 1000.0));
        assert!(ok(finger.touched(Press::Up, 1000.0 + TAP_SECONDS + 0.01)).is_empty());
    }

    #[test]
    fn a_touch_that_travelled_is_not_a_click() {
        let mut finger = Finger::default();
        ok(finger.touched(Press::Down, 1000.0));
        ok(finger.at(Axis::Sideways, 0));
        ok(finger.at(Axis::Sideways, TAP_TRAVEL + 1));
        assert!(ok(finger.touched(Press::Up, 1000.1)).is_empty());
    }

    #[test]
    fn the_first_report_of_a_touch_moves_nothing() {
        let mut finger = Finger::default();
        ok(finger.touched(Press::Down, 1000.0));
        ok(finger.at(Axis::Sideways, 800));
        assert!(ok(finger.carried()).is_empty());
    }

    #[test]
    fn a_finger_that_moved_moves_the_pointer_by_the_gain() {
        let mut finger = Finger::default();
        ok(finger.touched(Press::Down, 1000.0));
        ok(finger.at(Axis::Sideways, 100));
        ok(finger.at(Axis::Sideways, 200));
        assert_eq!(
            ok(finger.carried()),
            [Doing::Frame(vec![
                ok(Out::rel(RelativeAxisCode::REL_X.0, (100.0 * GAIN) as i32)),
                ok(Out::rel(RelativeAxisCode::REL_Y.0, 0)),
            ])]
        );
    }

    #[test]
    fn what_the_gain_leaves_behind_is_kept() {
        let mut finger = Finger::default();
        ok(finger.touched(Press::Down, 1000.0));
        ok(finger.at(Axis::Sideways, 0));
        let over: usize = (1..=4)
            .map(|step| {
                ok(finger.at(Axis::Sideways, step));
                ok(finger.carried()).len()
            })
            .sum();
        assert!(over > 0, "four units at a gain of {GAIN} is more than one pixel");
    }

    #[test]
    fn pressing_the_pad_in_holds_the_button_down() {
        let mut finger = Finger::default();
        assert_eq!(ok(finger.pressed(1)), [Doing::Frame(vec![ok(Out::key(KeyCode::BTN_LEFT.0, 1))])]);
        assert!(finger.held);
        ok(finger.pressed(0));
        assert!(!finger.held);
    }

    #[test]
    fn a_report_with_no_finger_down_is_nothing() {
        let mut finger = Finger::default();
        ok(finger.at(Axis::Sideways, 500));
        ok(finger.at(Axis::Sideways, 900));
        assert!(ok(finger.carried()).is_empty());
    }
}
