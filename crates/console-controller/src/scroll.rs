//! Turning the right stick into a scroll wheel.
//!
//! InputPlumber can map a stick to a wheel notch, but an axis crossing its
//! deadzone is one press and one release, and a wheel notch does not repeat
//! while it is held. One flick gave one imperceptible notch. Arrow keys
//! repeat, but in a terminal an arrow key is command history, not scrolling.
//!
//! So the stick stays an axis, and this turns how far it is pushed into how
//! fast the wheel turns.

use console_never::Never;
use evdev::RelativeAxisCode;

use crate::doing::Out;

pub const DEADZONE: f64 = 0.20;

pub const MAX_HZ: f64 = 14.0;

pub fn pushed(value: i32, span: i32) -> Result<f64, Never> {
    let span = f64::from(span.abs().max(1));
    let part = f64::from(value) / span;

    match part.abs() < DEADZONE {
        true => return Ok(0.0),
        false => {},
    }

    let past = (part.abs() - DEADZONE) / (1.0 - DEADZONE);

    Ok(match value > 0 {
        true => past * past,
        false => -(past * past),
    })
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Wheel {
    pub across: f64,
    pub down: f64,
}

impl Wheel {
    pub fn turned(&mut self, x: f64, y: f64, seconds: f64) -> Result<Vec<Out>, Never> {
        self.down += -y * MAX_HZ * seconds;
        self.across += x * MAX_HZ * seconds;
        let mut notches = Vec::new();

        while self.down.abs() >= 1.0 {
            let step = match self.down > 0.0 {
                true => 1,
                false => -1,
            };
            let Ok(out) = Out::rel(RelativeAxisCode::REL_WHEEL.0, step);

            notches.push(out);
            self.down -= f64::from(step);
        }

        while self.across.abs() >= 1.0 {
            let step = match self.across > 0.0 {
                true => 1,
                false => -1,
            };
            let Ok(out) = Out::rel(RelativeAxisCode::REL_HWHEEL.0, step);

            notches.push(out);
            self.across -= f64::from(step);
        }

        Ok(notches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stick_at_rest_is_at_rest() {
        assert_eq!(pushed(0, 32767), Ok(0.0));
        assert_eq!(pushed(6000, 32767), Ok(0.0), "inside the deadzone");
    }

    #[test]
    fn a_stick_pushed_all_the_way_is_all_the_way() {
        let Ok(right) = pushed(32767, 32767);
        let Ok(left) = pushed(-32767, 32767);

        assert!((right - 1.0).abs() < 1e-12);
        assert!((left + 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_small_push_is_slower_than_its_share() {
        let Ok(half) = pushed(32767 / 2, 32767);
        assert!(half > 0.0 && half < 0.5, "half a push is {half}");
    }

    #[test]
    fn a_stick_held_for_a_second_turns_the_wheel_as_far_as_the_arithmetic_says() {
        let mut wheel = Wheel::default();
        let notches: Vec<Out> = (0..50)
            .flat_map(|_| {
                let Ok(turned) = wheel.turned(0.0, -1.0, 0.02);

                turned
            })
            .collect();
        assert_eq!(notches.len() + usize::from(wheel.down >= 0.5), MAX_HZ as usize);
        assert!(wheel.down < 1.0, "nothing whole is left unturned");
        assert!(notches.iter().all(|out| out.value == 1 && out.code == RelativeAxisCode::REL_WHEEL.0));
    }

    #[test]
    fn pushing_up_scrolls_up_and_pushing_down_scrolls_down() {
        let mut wheel = Wheel::default();
        let Ok(up) = wheel.turned(0.0, -1.0, 1.0);
        let mut other = Wheel::default();
        let Ok(down) = other.turned(0.0, 1.0, 1.0);

        assert_eq!(up.first().map(|out| out.value), Some(1));
        assert_eq!(down.first().map(|out| out.value), Some(-1));
    }

    #[test]
    fn what_is_owed_is_kept_until_it_is_a_whole_notch() {
        let mut wheel = Wheel::default();
        let Ok(first) = wheel.turned(0.0, -0.1, 0.02);

        assert!(first.is_empty());

        let over = (0..100)
            .flat_map(|_| {
                let Ok(turned) = wheel.turned(0.0, -0.1, 0.02);

                turned
            })
            .count();
        assert!(over > 0, "a slow push still scrolls, eventually");
    }
}
