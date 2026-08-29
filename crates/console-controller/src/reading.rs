//! What arrived, and what to do about it.
//!
//! Three devices are read: the pad InputPlumber publishes, the keyboard it
//! publishes beside it, and the controller's own touchpad. This is all of the
//! deciding, and none of the opening.

use evdev::{AbsoluteAxisCode, EventType, KeyCode};

use crate::buttons;
use crate::doing::Doing;
use crate::scroll::{Wheel, pushed};
use crate::touch::Finger;

/// How far L2 must be pulled to count as held.
pub const CARRY_HELD: f64 = 0.5;

/// Which device something arrived on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum From {
    Pad,
    Keys,
    Touch,
}

/// The ranges the pad reports over, read off it when it is found.
///
/// Held rather than asked for every event: the pad goes away whenever a
/// profile is switched, and arithmetic that has to ask a device that is not
/// there is arithmetic that stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ranges {
    pub stick: i32,
    pub trigger: (i32, i32),
}

impl Default for Ranges {
    fn default() -> Self {
        Ranges { stick: 1, trigger: (0, 1) }
    }
}

/// Everything the daemon is holding between one event and the next.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Controller {
    /// Whether L2 is pulled far enough that a button does its second thing.
    pub carrying: bool,
    pub wheel: Wheel,
    pub finger: Finger,
    pub ranges: Ranges,
    stick: (f64, f64),
}

impl Controller {
    /// The pad has been found, and reports over these ranges.
    pub fn reading(&mut self, ranges: Ranges) {
        self.ranges = ranges;
    }

    /// The pad has gone, which a profile switch does every time.
    ///
    /// Reading from nothing is what used to end this process, and it took the
    /// workspace buttons with it.
    pub fn pad_went(&mut self) {
        self.stick = (0.0, 0.0);
    }

    /// One event, and what it comes to.
    ///
    /// The time it arrived is handed in rather than read, because whether a
    /// touch was a tap is arithmetic and arithmetic has one right answer.
    pub fn saw(
        &mut self,
        from: From,
        kind: EventType,
        code: u16,
        value: i32,
        now: f64,
    ) -> Vec<Doing> {
        match from {
            From::Pad => self.on_pad(kind, code, value),
            From::Keys => self.on_keys(kind, code, value),
            From::Touch => self.on_touch(kind, code, value, now),
        }
    }

    fn on_pad(&mut self, kind: EventType, code: u16, value: i32) -> Vec<Doing> {
        match kind {
            EventType::ABSOLUTE => self.on_axis(code, value),
            EventType::KEY => self.on_pad_key(code, value),
            _ => Vec::new(),
        }
    }

    fn on_axis(&mut self, code: u16, value: i32) -> Vec<Doing> {
        // Both triggers report as an axis, and L2 is the one that carries.
        if code == AbsoluteAxisCode::ABS_Z.0 || code == AbsoluteAxisCode::ABS_HAT2Y.0 {
            let (low, high) = self.ranges.trigger;
            let span = f64::from((high - low).max(1));
            self.carrying = f64::from(value - low) / span > CARRY_HELD;
            return Vec::new();
        }
        if code == AbsoluteAxisCode::ABS_RX.0 {
            self.stick.0 = pushed(value, self.ranges.stick);
        } else if code == AbsoluteAxisCode::ABS_RY.0 {
            self.stick.1 = pushed(value, self.ranges.stick);
        }
        Vec::new()
    }

    fn on_pad_key(&mut self, code: u16, value: i32) -> Vec<Doing> {
        if code == KeyCode::BTN_TL2.0 {
            self.carrying = value == 1;
        }
        match value == 1 {
            true => buttons::on_pad(code, self.carrying).into_iter().collect(),
            false => Vec::new(),
        }
    }

    fn on_keys(&mut self, kind: EventType, code: u16, value: i32) -> Vec<Doing> {
        match kind == EventType::KEY && value == 1 {
            true => buttons::on_keyboard(code, self.carrying).into_iter().collect(),
            false => Vec::new(),
        }
    }

    fn on_touch(&mut self, kind: EventType, code: u16, value: i32, now: f64) -> Vec<Doing> {
        match (kind, code) {
            (EventType::KEY, code) if code == KeyCode::BTN_TOUCH.0 => {
                self.finger.touched(value == 1, now)
            }
            (EventType::KEY, code) if code == KeyCode::BTN_0.0 => self.finger.pressed(value),
            (EventType::ABSOLUTE, code)
                if code == AbsoluteAxisCode::ABS_X.0 || code == AbsoluteAxisCode::ABS_Y.0 =>
            {
                self.finger.at(code == AbsoluteAxisCode::ABS_X.0, value);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// A moment has passed. The stick is where it was, so the wheel turns.
    pub fn tick(&mut self, seconds: f64) -> Vec<Doing> {
        let notches = self.wheel.turned(self.stick.0, self.stick.1, seconds);
        match notches.is_empty() {
            true => Vec::new(),
            false => vec![Doing::Frame(notches)],
        }
    }

    /// How long to wait before reading again.
    ///
    /// A finger on the pad is read at the pad's own pace. Anything slower
    /// arrives as a series of jumps, which reads as a slow machine.
    pub fn poll(&self) -> f64 {
        match self.finger.down {
            true => crate::touch::POLL,
            false => POLL,
        }
    }
}

/// Seconds between ticks when nothing is under a thumb.
pub const POLL: f64 = 0.02;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doing::Out;
    use evdev::RelativeAxisCode;

    /// The pad's real ranges, as the capture records them.
    fn ranges() -> Ranges {
        Ranges { stick: 32767, trigger: (0, 1023) }
    }

    fn controller() -> Controller {
        let mut held = Controller::default();
        held.reading(ranges());
        held
    }

    fn pressed(held: &mut Controller, from: From, code: KeyCode) -> Vec<Doing> {
        let down = held.saw(from, EventType::KEY, code.0, 1, 1000.0);
        held.saw(from, EventType::KEY, code.0, 0, 1000.0);
        down
    }

    #[test]
    fn a_button_acts_when_it_goes_down_and_not_when_it_comes_up() {
        let mut held = controller();
        assert_eq!(
            held.saw(From::Keys, EventType::KEY, KeyCode::KEY_F13.0, 1, 1000.0),
            [Doing::run(&["launcher", "--keep"])]
        );
        assert!(held.saw(From::Keys, EventType::KEY, KeyCode::KEY_F13.0, 0, 1000.0).is_empty());
    }

    #[test]
    fn the_shoulders_carry_the_window_while_l2_is_held() {
        let mut held = controller();
        assert_eq!(pressed(&mut held, From::Pad, KeyCode::BTN_TR), [Doing::workspace("+1", false)]);
        held.saw(From::Pad, EventType::KEY, KeyCode::BTN_TL2.0, 1, 1000.0);
        assert!(held.carrying);
        assert_eq!(pressed(&mut held, From::Pad, KeyCode::BTN_TR), [Doing::workspace("+1", true)]);
    }

    /// L2 is an axis before it is a button, and how far it is pulled is what
    /// says whether it is being held.
    #[test]
    fn pulling_l2_past_halfway_is_holding_it() {
        let mut held = controller();
        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_Z.0, 400, 1000.0);
        assert!(!held.carrying, "not far enough");
        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_Z.0, 900, 1000.0);
        assert!(held.carrying);
    }

    #[test]
    fn the_right_stick_turns_the_wheel_and_the_left_one_does_not() {
        let mut held = controller();
        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_Y.0, -32767, 1000.0);
        assert!(held.tick(1.0).is_empty(), "the left stick is not a wheel");
        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RY.0, -32767, 1000.0);
        let turned = held.tick(1.0);
        assert!(matches!(turned.as_slice(), [Doing::Frame(notches)] if notches.len() == 22));
    }

    /// The pad goes away whenever a profile is switched. The stick has to stop
    /// where it stood, or the wheel turns forever on a device that is gone.
    #[test]
    fn a_pad_that_went_away_stops_the_wheel() {
        let mut held = controller();
        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RY.0, -32767, 1000.0);
        held.pad_went();
        assert!(held.tick(1.0).is_empty());
    }

    #[test]
    fn a_tap_on_the_touchpad_is_a_click() {
        let mut held = controller();
        held.saw(From::Touch, EventType::KEY, KeyCode::BTN_TOUCH.0, 1, 1000.0);
        let clicked = held.saw(From::Touch, EventType::KEY, KeyCode::BTN_TOUCH.0, 0, 1000.05);
        assert_eq!(clicked.len(), 2, "down and up");
    }

    #[test]
    fn a_finger_on_the_pad_is_read_at_the_pads_own_pace() {
        let mut held = controller();
        assert_eq!(held.poll(), POLL);
        held.saw(From::Touch, EventType::KEY, KeyCode::BTN_TOUCH.0, 1, 1000.0);
        assert_eq!(held.poll(), crate::touch::POLL);
    }

    /// Everything the daemon publishes goes out of its own device, so a wheel
    /// notch and a pointer move are the same kind of thing.
    #[test]
    fn what_comes_out_is_movement_on_one_device() {
        let mut held = controller();
        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RX.0, 32767, 1000.0);
        let turned = held.tick(1.0);
        let Some(Doing::Frame(notches)) = turned.first() else {
            panic!("a frame of notches");
        };
        assert!(notches.contains(&Out::rel(RelativeAxisCode::REL_HWHEEL.0, 1)));
    }
}
