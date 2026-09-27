//! The touchpad on the right face, turned into pointer movement.
//!
//! InputPlumber cannot do this. Asked to, it answers "Translation not
//! implemented" once per event and drops them, so the pad did nothing at all
//! no matter how it was mapped. Handing it to the compositor instead makes it
//! an absolute device: touching the middle of the pad puts the cursor in the
//! middle of the screen, which is not what a pad under a thumb is for. So it
//! is read here, and what comes out is movement rather than position.


use console_core_never::Never;
use console_core_number_conversion::toward_zero_i32;
use console_input_event_devices::{KeyCode, RelativeAxisCode};

use crate::effect::{Effect, Output};
use crate::actions::ButtonPress;

pub const GAIN: f64 = 1.4;

pub const POLL: f64 = 0.008;

pub const SWAP: bool = false;
pub const FLIP_X: bool = false;
pub const FLIP_Y: bool = false;

pub const TAP_SECONDS: f64 = 0.25;

pub const TAP_TRAVEL: i32 = 40;

fn click() -> Result<Vec<Effect>, Never> {
    let Ok(down) = Output::key(KeyCode::BTN_LEFT.0, 1);
    let Ok(up) = Output::key(KeyCode::BTN_LEFT.0, 0);

    Ok(vec![Effect::Frame(vec![down]), Effect::Frame(vec![up])])
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(
    dylint_lib = "explicit048_no_unreal_state",
    allow(
        explicit048_no_unreal_state,
        reason = "`down` is a finger on the pad and `held` is the click under it, which are two devices: a finger travels without the button and the button stays held after the finger lifts"
    )
)]
pub struct Touch {
    pub in_contact: bool,
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

impl Touch {
    pub fn handle_press(&mut self, down: ButtonPress, now: f64) -> Result<Vec<Effect>, Never> {
        match down {
            ButtonPress::Down => {
                *self = Touch {
                    in_contact: true,
                    started: now,
                    held: self.held,
                    owed: self.owed,
                    ..Touch::default()
                };

                return Ok(Vec::new());
            }
            ButtonPress::Up => {},
        }

        self.in_contact = false;
        let quick = now - self.started < TAP_SECONDS;

        match quick && self.travel < TAP_TRAVEL {
            true => click(),
            false => Ok(Vec::new()),
        }
    }

    pub fn pressed(&mut self, value: i32) -> Result<Vec<Effect>, Never> {
        self.held = value == 1;

        let Ok(out) = Output::key(KeyCode::BTN_LEFT.0, value);

        Ok(vec![Effect::Frame(vec![out])])
    }

    pub fn at(&mut self, along: Axis, value: i32) -> Result<(), Never> {
        match self.in_contact {
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

    pub fn flush_motion(&mut self) -> Result<Vec<Effect>, Never> {
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

        let Ok(across) = Output::relative(RelativeAxisCode::REL_X.0, whole_x);
        let Ok(down) = Output::relative(RelativeAxisCode::REL_Y.0, whole_y);

        Ok(vec![Effect::Frame(vec![across, down])])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quick_touch_that_stayed_still_is_a_click() {
        let mut finger = Touch::default();

        assert_eq!(finger.handle_press(ButtonPress::Down, 1000.0), Ok(Vec::new()));
        assert_eq!(finger.handle_press(ButtonPress::Up, 1000.1), click());
    }

    #[test]
    fn a_touch_that_lingered_is_not_a_click() {
        let mut finger = Touch::default();
        let Ok(_) = finger.handle_press(ButtonPress::Down, 1000.0);

        assert_eq!(finger.handle_press(ButtonPress::Up, 1000.0 + TAP_SECONDS + 0.01), Ok(Vec::new()));
    }

    #[test]
    fn a_touch_that_travelled_is_not_a_click() {
        let mut finger = Touch::default();
        let Ok(_) = finger.handle_press(ButtonPress::Down, 1000.0);
        let Ok(()) = finger.at(Axis::Sideways, 0);
        let Ok(()) = finger.at(Axis::Sideways, TAP_TRAVEL.saturating_add(1));

        assert_eq!(finger.handle_press(ButtonPress::Up, 1000.1), Ok(Vec::new()));
    }

    #[test]
    fn the_first_report_of_a_touch_moves_nothing() {
        let mut finger = Touch::default();
        let Ok(_) = finger.handle_press(ButtonPress::Down, 1000.0);
        let Ok(()) = finger.at(Axis::Sideways, 800);

        assert_eq!(finger.flush_motion(), Ok(Vec::new()));
    }

    #[test]
    fn a_finger_that_moved_moves_the_pointer_by_the_gain() {
        let mut finger = Touch::default();
        let Ok(_) = finger.handle_press(ButtonPress::Down, 1000.0);
        let Ok(()) = finger.at(Axis::Sideways, 100);
        let Ok(()) = finger.at(Axis::Sideways, 200);
        let Ok(travelled) = console_core_number_conversion::toward_zero_i32(100.0 * GAIN);
        let Ok(across) = Output::relative(RelativeAxisCode::REL_X.0, travelled);
        let Ok(down) = Output::relative(RelativeAxisCode::REL_Y.0, 0);

        assert_eq!(finger.flush_motion(), Ok(vec![Effect::Frame(vec![across, down])]));
    }

    #[test]
    fn what_the_gain_leaves_behind_is_kept() {
        let mut finger = Touch::default();
        let Ok(_) = finger.handle_press(ButtonPress::Down, 1000.0);
        let Ok(()) = finger.at(Axis::Sideways, 0);
        let mut over: Vec<Effect> = Vec::new();

        for step in 1..=4 {
            let Ok(()) = finger.at(Axis::Sideways, step);
            let Ok(carried) = finger.flush_motion();

            over.extend(carried);
        }

        assert!(!over.is_empty(), "four units at a gain of {GAIN} is more than one pixel");
    }

    #[test]
    fn pressing_the_pad_in_holds_the_button_down() {
        let mut finger = Touch::default();
        let Ok(clicked) = Output::key(KeyCode::BTN_LEFT.0, 1);

        assert_eq!(finger.pressed(1), Ok(vec![Effect::Frame(vec![clicked])]));
        assert!(finger.held);

        let Ok(_) = finger.pressed(0);

        assert!(!finger.held);
    }

    #[test]
    fn a_report_with_no_finger_down_is_nothing() {
        let mut finger = Touch::default();
        let Ok(()) = finger.at(Axis::Sideways, 500);
        let Ok(()) = finger.at(Axis::Sideways, 900);

        assert_eq!(finger.flush_motion(), Ok(Vec::new()));
    }
}
