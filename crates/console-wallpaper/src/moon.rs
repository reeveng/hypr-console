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

use console_core_never::Never;

const A_NEW_MOON: f64 = 947_182_440.0;

const CYCLE: f64 = 29.530_588_853;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Moon {
    Full,
    New,
    Waning,
    Waxing,
}

impl Moon {
    pub fn word(&self) -> Result<&'static str, Never> {
        Ok(match self {
            Moon::Full => "full",
            Moon::New => "new",
            Moon::Waning => "waning",
            Moon::Waxing => "waxing",
        })
    }

    pub const EVERY: [Moon; 4] = [Moon::Full, Moon::New, Moon::Waning, Moon::Waxing];

    pub fn of(word: &str) -> Result<Option<Self>, Never> {
        let word = word.trim().to_lowercase();

        for moon in Moon::EVERY {
            let said = moon.word()?;

            match said == word {
                true => return Ok(Some(moon)),
                false => {},
            }
        }

        Ok(None)
    }
}

pub fn through(unix: f64) -> Result<f64, Never> {
    Ok((((unix - A_NEW_MOON) / 86_400.0) / CYCLE).rem_euclid(1.0))
}

pub fn moon(unix: f64) -> Result<Moon, Never> {
    let through = through(unix)?;

    Ok(match through {
        _ if through < 0.0625 || through >= 0.9375 => Moon::New,
        _ if through < 0.4375 => Moon::Waxing,
        _ if through < 0.5625 => Moon::Full,
        _ => Moon::Waning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: f64 = 86_400.0;

    #[test]
    fn the_moon_the_count_starts_from_is_a_new_one() {
        assert_eq!(moon(A_NEW_MOON), Ok(Moon::New));

        let Ok(through) = through(A_NEW_MOON);

        assert!(through < 0.001);
    }

    #[test]
    fn the_moon_fills_and_empties_over_a_cycle() {
        let after = |days: f64| moon(A_NEW_MOON + days * DAY);
        assert_eq!(after(CYCLE * 0.25), Ok(Moon::Waxing));
        assert_eq!(after(CYCLE * 0.5), Ok(Moon::Full));
        assert_eq!(after(CYCLE * 0.75), Ok(Moon::Waning));
        assert_eq!(after(CYCLE), Ok(Moon::New));
    }

    #[test]
    fn the_same_phase_comes_back_a_cycle_later() {
        for cycles in [1.0, 13.0, 200.0, 330.0] {
            let Ok(later) = through(A_NEW_MOON + cycles * CYCLE * DAY);

            let Ok(first) = through(A_NEW_MOON);

            let apart = (later - first).abs();

            assert!(apart < 0.001 || apart > 0.999, "{cycles} cycles on it was {apart} through");
        }
    }

    #[test]
    fn a_moment_before_the_count_starts_is_still_somewhere_in_a_cycle() {
        let long_ago = A_NEW_MOON - 400.0 * CYCLE * DAY - 3.0 * DAY;
        let Ok(through) = through(long_ago);

        assert!((0.0..1.0).contains(&through), "{through}");

        let Ok(moon) = moon(long_ago);

        assert!(Moon::EVERY.contains(&moon));
    }

    #[test]
    fn a_moon_is_written_and_read_as_the_same_word() {
        for moon in Moon::EVERY {
            let Ok(word) = moon.word();

            assert_eq!(Moon::of(word), Ok(Some(moon)));
        }

        assert_eq!(Moon::of("blue"), Ok(None));
    }
}
