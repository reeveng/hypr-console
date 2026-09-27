//! How far an axis is pushed from where it rests.
//!
//! A stick says a number inside the range the device reported for it, and the
//! range is the device's to choose: the Legion Go's own sticks run either side
//! of zero, a pad behind InputPlumber or a plugged-in one may run from 0 to
//! 255. The keyboard read a stick from the middle of its range and the desktop
//! read it from zero, so on a stick that rests at 128 the desktop saw one pushed
//! half way and scrolled with nobody touching it.
//!
//! So a push is measured from the middle of the range, as a part of half the
//! range, and what each caller does with it -- a deadzone, a curve, a direction
//! -- stays the caller's.

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub low: i32,
    pub high: i32,
}

pub fn part(value: i32, range: Range) -> Result<f64, Never> {
    let Range { low, high } = range;

    let half = match high > low {
        true => (f64::from(high) - f64::from(low)) / 2.0,
        false => return Ok(0.0),
    };
    let middle = f64::from(low) + half;

    Ok(((f64::from(value) - middle) / half).clamp(-1.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EITHER_SIDE: Range = Range { low: -32768, high: 32767 };

    const FROM_ZERO: Range = Range { low: 0, high: 255 };

    #[test]
    fn a_stick_at_rest_is_not_pushed_whichever_way_its_range_runs() {
        let Ok(centred) = part(0, EITHER_SIDE);
        let Ok(from_zero) = part(128, FROM_ZERO);

        assert!(centred.abs() < 0.001, "{centred}");
        assert!(from_zero.abs() < 0.01, "{from_zero}");
    }

    #[test]
    fn the_ends_of_a_range_are_all_the_way_either_way() {
        assert_eq!(part(255, FROM_ZERO), Ok(1.0));
        assert_eq!(part(0, FROM_ZERO), Ok(-1.0));
        assert_eq!(part(32767, EITHER_SIDE), Ok(1.0));
        assert_eq!(part(-32768, EITHER_SIDE), Ok(-1.0));
    }

    #[test]
    fn a_range_with_no_width_says_nothing_is_pushed_rather_than_dividing_by_it() {
        assert_eq!(part(5, Range { low: 5, high: 5 }), Ok(0.0));
    }
}
