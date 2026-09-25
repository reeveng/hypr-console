//! Turning the right stick into a scroll wheel.
//!
//! InputPlumber can map a stick to a wheel notch, but an axis crossing its
//! deadzone is one press and one release, and a wheel notch does not repeat
//! while it is held. One flick gave one imperceptible notch. Arrow keys
//! repeat, but in a terminal an arrow key is command history, not scrolling.
//!
//! So the stick stays an axis, and this turns how far it is pushed into how
//! fast the wheel turns.

use console_core_geometry::Point;
use console_core_never::Never;
use console_input_event_devices::RelativeAxisCode;

use crate::effect::Output;

pub const DEADZONE: f64 = 0.20;

pub const MAX_HZ: f64 = 14.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stick {
    pub value: i32,
    pub span: i32,
}

pub fn pushed(stick: Stick) -> Result<f64, Never> {
    let Stick { value, span } = stick;
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
    pub x: f64,
    pub y: f64,
}

impl Wheel {
    pub fn turned(&mut self, by: Point<f64>, seconds: f64) -> Result<Vec<Output>, Never> {
        self.y += -by.y * MAX_HZ * seconds;
        self.x += by.x * MAX_HZ * seconds;
        let mut notches = Vec::new();

        while self.y.abs() >= 1.0 {
            let step = match self.y > 0.0 {
                true => 1,
                false => -1,
            };
            let Ok(out) = Output::relative(RelativeAxisCode::REL_WHEEL.0, step);

            notches.push(out);
            self.y -= f64::from(step);
        }

        while self.x.abs() >= 1.0 {
            let step = match self.x > 0.0 {
                true => 1,
                false => -1,
            };
            let Ok(out) = Output::relative(RelativeAxisCode::REL_HWHEEL.0, step);

            notches.push(out);
            self.x -= f64::from(step);
        }

        Ok(notches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stick_at_rest_is_at_rest() {
        assert_eq!(pushed(Stick { value: 0, span: 32767 }), Ok(0.0));
        assert_eq!(pushed(Stick { value: 6000, span: 32767 }), Ok(0.0), "inside the deadzone");
    }

    #[test]
    fn a_stick_pushed_all_the_way_is_all_the_way() {
        let Ok(right) = pushed(Stick { value: 32767, span: 32767 });
        let Ok(left) = pushed(Stick { value: -32767, span: 32767 });

        assert!((right - 1.0).abs() < 1e-12);
        assert!((left + 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_small_push_is_slower_than_its_share() {
        let Ok(half) = pushed(Stick { value: 32767 / 2, span: 32767 });
        assert!(half > 0.0 && half < 0.5, "half a push is {half}");
    }

    #[test]
    fn a_stick_held_for_a_second_turns_the_wheel_as_far_as_the_arithmetic_says() {
        let mut wheel = Wheel::default();
        let notches: Vec<Output> = (0..50)
            .flat_map(|_| {
                let Ok(turned) = wheel.turned(Point { x: 0.0, y: -1.0 }, 0.02);

                turned
            })
            .collect();
        assert_eq!(u32::try_from(notches.len()).unwrap() + u32::from(wheel.y >= 0.5), MAX_HZ as u32);
        assert!(wheel.y < 1.0, "nothing whole is left unturned");
        assert!(notches.iter().all(|out| out.value == 1 && out.code == RelativeAxisCode::REL_WHEEL.0));
    }

    #[test]
    fn pushing_up_scrolls_up_and_pushing_down_scrolls_down() {
        let mut wheel = Wheel::default();
        let Ok(up) = wheel.turned(Point { x: 0.0, y: -1.0 }, 1.0);
        let mut other = Wheel::default();
        let Ok(down) = other.turned(Point { x: 0.0, y: 1.0 }, 1.0);

        assert_eq!(up.first().map(|out| out.value), Some(1));
        assert_eq!(down.first().map(|out| out.value), Some(-1));
    }

    #[test]
    fn what_is_owed_is_kept_until_it_is_a_whole_notch() {
        let mut wheel = Wheel::default();
        let Ok(first) = wheel.turned(Point { x: 0.0, y: -0.1 }, 0.02);

        assert!(first.is_empty());

        let over: Vec<Output> = (0..100)
            .flat_map(|_| {
                let Ok(turned) = wheel.turned(Point { x: 0.0, y: -0.1 }, 0.02);

                turned
            })
            .collect();
        assert!(!over.is_empty(), "a slow push still scrolls, eventually");
    }
}
