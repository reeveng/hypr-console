//! What the moon is doing.
//!
//! The one thing worth knowing about a night that the sun cannot say. A clear
//! night under a full moon and a clear night under no moon at all are different
//! nights to stand outside in, and the sun's height calls them both `night`.
//!
//! Needs no place. The moon shows the same face to the whole earth at the same
//! moment, so unlike everything in `sun` this is a function of the clock alone.
//!
//! What is worked out here is the mean phase: the moon's age since a new moon,
//! divided by the average length of the cycle. The real moon runs ahead of and
//! behind that by up to about half a day, because its orbit is an ellipse and
//! it does not travel it at an even rate. That matters to somebody pointing a
//! telescope and does not matter to somebody choosing a picture, so the mean is
//! what is used and the error is written down here rather than corrected for.

/// A new moon that everything counts from: 6 January 2000, a quarter past six
/// in the evening UTC.
const A_NEW_MOON: f64 = 947_182_440.0;

/// How long the moon takes to come back to the same phase, on average.
const CYCLE: f64 = 29.530_588_853;

/// What the moon looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Moon {
    Full,
    New,
    Waning,
    Waxing,
}

impl Moon {
    pub fn word(&self) -> &'static str {
        match self {
            Moon::Full => "full",
            Moon::New => "new",
            Moon::Waning => "waning",
            Moon::Waxing => "waxing",
        }
    }

    pub const EVERY: [Moon; 4] = [Moon::Full, Moon::New, Moon::Waning, Moon::Waxing];

    pub fn of(word: &str) -> Option<Self> {
        let word = word.trim().to_lowercase();
        Moon::EVERY.into_iter().find(|moon| moon.word() == word)
    }
}

/// How far through the cycle the moon is: nought at new, a half at full.
pub fn through(unix: f64) -> f64 {
    (((unix - A_NEW_MOON) / 86_400.0) / CYCLE).rem_euclid(1.0)
}

/// What the moon looks like now.
///
/// Full and new get an eighth of the cycle each, which is about three and a
/// half nights either side, and that is roughly how long the moon looks full to
/// somebody who is not measuring it. The two halves between them are the rest.
pub fn moon(unix: f64) -> Moon {
    let through = through(unix);
    match through {
        _ if through < 0.0625 || through >= 0.9375 => Moon::New,
        _ if through < 0.4375 => Moon::Waxing,
        _ if through < 0.5625 => Moon::Full,
        _ => Moon::Waning,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: f64 = 86_400.0;

    #[test]
    fn the_moon_the_count_starts_from_is_a_new_one() {
        assert_eq!(moon(A_NEW_MOON), Moon::New);
        assert!(through(A_NEW_MOON) < 0.001);
    }

    /// Half a cycle after a new moon is a full one, and the quarters between
    /// them are the two halves of the month.
    #[test]
    fn the_moon_fills_and_empties_over_a_cycle() {
        let after = |days: f64| moon(A_NEW_MOON + days * DAY);
        assert_eq!(after(CYCLE * 0.25), Moon::Waxing);
        assert_eq!(after(CYCLE * 0.5), Moon::Full);
        assert_eq!(after(CYCLE * 0.75), Moon::Waning);
        assert_eq!(after(CYCLE), Moon::New);
    }

    /// The whole of what this is: the same phase comes back a cycle later, and
    /// it comes back a great many cycles later too.
    #[test]
    fn the_same_phase_comes_back_a_cycle_later() {
        for cycles in [1.0, 13.0, 200.0, 330.0] {
            let apart = (through(A_NEW_MOON + cycles * CYCLE * DAY) - through(A_NEW_MOON)).abs();
            assert!(apart < 0.001 || apart > 0.999, "{cycles} cycles on it was {apart} through");
        }
    }

    /// Before the epoch as well as after it, because a machine with its clock
    /// wrong is not a machine that should crash or show a phase it cannot have.
    #[test]
    fn a_moment_before_the_count_starts_is_still_somewhere_in_a_cycle() {
        let long_ago = A_NEW_MOON - 400.0 * CYCLE * DAY - 3.0 * DAY;
        assert!((0.0..1.0).contains(&through(long_ago)), "{}", through(long_ago));
        assert!(Moon::EVERY.contains(&moon(long_ago)));
    }

    #[test]
    fn a_moon_is_written_and_read_as_the_same_word() {
        for moon in Moon::EVERY {
            assert_eq!(Moon::of(moon.word()), Some(moon));
        }
        assert_eq!(Moon::of("blue"), None);
    }
}
