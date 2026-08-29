//! Turning the right stick into a scroll wheel.
//!
//! InputPlumber can map a stick to a wheel notch, but an axis crossing its
//! deadzone is one press and one release, and a wheel notch does not repeat
//! while it is held. One flick gave one imperceptible notch. Arrow keys
//! repeat, but in a terminal an arrow key is command history, not scrolling.
//!
//! So the stick stays an axis, and this turns how far it is pushed into how
//! fast the wheel turns.

use evdev::RelativeAxisCode;

use crate::doing::Out;

/// Below this the stick is at rest.
pub const DEADZONE: f64 = 0.20;

/// Notches per second at full deflection.
pub const MAX_HZ: f64 = 22.0;

/// How far a stick is pushed, from -1 to 1, with the deadzone taken out.
///
/// Rescaled so motion starts at the edge of the deadzone, and squared so small
/// pushes stay slow and precise.
pub fn pushed(value: i32, span: i32) -> f64 {
    let span = f64::from(span.abs().max(1));
    let part = f64::from(value) / span;
    if part.abs() < DEADZONE {
        return 0.0;
    }
    let past = (part.abs() - DEADZONE) / (1.0 - DEADZONE);
    match value > 0 {
        true => past * past,
        false => -(past * past),
    }
}

/// How far the wheel has been asked to turn and not yet turned.
///
/// Kept as a debt rather than rounded away, so a stick held at a tenth of its
/// travel still scrolls, slowly, rather than not at all.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Wheel {
    pub across: f64,
    pub down: f64,
}

impl Wheel {
    /// A moment of a stick held where it is, as the notches that come out.
    pub fn turned(&mut self, x: f64, y: f64, seconds: f64) -> Vec<Out> {
        // Push up, page goes up.
        self.down += -y * MAX_HZ * seconds;
        self.across += x * MAX_HZ * seconds;
        let mut notches = Vec::new();
        while self.down.abs() >= 1.0 {
            let step = if self.down > 0.0 { 1 } else { -1 };
            notches.push(Out::rel(RelativeAxisCode::REL_WHEEL.0, step));
            self.down -= f64::from(step);
        }
        while self.across.abs() >= 1.0 {
            let step = if self.across > 0.0 { 1 } else { -1 };
            notches.push(Out::rel(RelativeAxisCode::REL_HWHEEL.0, step));
            self.across -= f64::from(step);
        }
        notches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stick_at_rest_is_at_rest() {
        assert_eq!(pushed(0, 32767), 0.0);
        assert_eq!(pushed(6000, 32767), 0.0, "inside the deadzone");
    }

    #[test]
    fn a_stick_pushed_all_the_way_is_all_the_way() {
        assert!((pushed(32767, 32767) - 1.0).abs() < 1e-12);
        assert!((pushed(-32767, 32767) + 1.0).abs() < 1e-12);
    }

    /// Squared, so a small push is much slower than half of a big one.
    #[test]
    fn a_small_push_is_slower_than_its_share() {
        let half = pushed(32767 / 2, 32767);
        assert!(half > 0.0 && half < 0.5, "half a push is {half}");
    }

    /// The arithmetic this daemon exists for: a stick held for a second turns
    /// the wheel as far as the numbers say, on any machine, every run.
    #[test]
    fn a_stick_held_for_a_second_turns_the_wheel_as_far_as_the_arithmetic_says() {
        let mut wheel = Wheel::default();
        let notches: Vec<Out> = (0..50).flat_map(|_| wheel.turned(0.0, -1.0, 0.02)).collect();
        // Give or take the one still owed: a second of ticks of 0.02 does not
        // add up to exactly a second, and what is left over is kept rather
        // than turned early.
        assert_eq!(notches.len() + usize::from(wheel.down >= 0.5), MAX_HZ as usize);
        assert!(wheel.down < 1.0, "nothing whole is left unturned");
        assert!(notches.iter().all(|out| out.value == 1 && out.code == RelativeAxisCode::REL_WHEEL.0));
    }

    #[test]
    fn pushing_up_scrolls_up_and_pushing_down_scrolls_down() {
        let mut wheel = Wheel::default();
        assert_eq!(wheel.turned(0.0, -1.0, 1.0).first().map(|out| out.value), Some(1));
        let mut other = Wheel::default();
        assert_eq!(other.turned(0.0, 1.0, 1.0).first().map(|out| out.value), Some(-1));
    }

    /// What is owed is kept, or a slow push rounds to nothing every tick and
    /// the page never moves at all.
    #[test]
    fn what_is_owed_is_kept_until_it_is_a_whole_notch() {
        let mut wheel = Wheel::default();
        assert!(wheel.turned(0.0, -0.1, 0.02).is_empty());
        let over = (0..100).flat_map(|_| wheel.turned(0.0, -0.1, 0.02)).count();
        assert!(over > 0, "a slow push still scrolls, eventually");
    }
}
