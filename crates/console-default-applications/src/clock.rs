//! Whether the hour on the bar is written 14:30 or 2:30 pm.
//!
//! Here rather than in the panel that offers it or the bar that draws it,
//! because both of them need the same answer and neither has any business
//! depending on the other. It is a thing somebody chose, and this is where the
//! things somebody chose are kept.
//!
//! The shape is a strftime line rather than a name for a shape, because `date`
//! is what turns it into words and that is the only vocabulary it has. Nobody
//! composes one of these at a call site: there are two, they are both here,
//! and `says` is what a person is offered instead -- the hour itself, written
//! both ways, which is the only thing anybody actually wants to compare.

use console_core_never::Never;
use console_core_words::Words;

const SETTING: &str = "clock";

const TWENTY_FOUR: &str = "24";

const TWELVE: &str = "12";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Clock {
    #[words(says = "14:30", shape = "%a %d %b  %H:%M")]
    TwentyFour,
    #[words(says = "2:30 pm", shape = "%a %d %b  %-I:%M %P")]
    Twelve,
}

pub const EVERY: [Clock; 2] = [Clock::TwentyFour, Clock::Twelve];

impl Clock {
    pub fn key(self) -> Result<&'static str, Never> {
        Ok(match self {
            Clock::TwentyFour => TWENTY_FOUR,
            Clock::Twelve => TWELVE,
        })
    }
}

pub fn read(said: &str) -> Result<Clock, Never> {
    Ok(match said.trim() == TWELVE {
        true => Clock::Twelve,
        false => Clock::TwentyFour,
    })
}

pub fn clock() -> Result<Clock, Never> {
    let told = crate::setting(SETTING)?;

    let said = match told {
        Some(said) => said,
        None => String::new(),
    };

    read(&said)
}

pub fn choose(clock: Clock) -> Result<(), Never> {
    let Ok(key) = clock.key();

    crate::set(crate::Setting { key: SETTING, value: key })
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_ways_of_writing_the_hour_are_two_different_readings() {
        let Ok(long) = Clock::TwentyFour.shape();
        let Ok(short) = Clock::Twelve.shape();

        assert_ne!(long, short);
        assert!(long.contains("%H"), "{long} is not the twenty-four hour clock");
        assert!(short.contains("%P"), "{short} does not say which half of the day it is");
    }

    #[test]
    fn a_machine_nobody_has_told_writes_the_hour_the_way_this_one_always_has() {
        assert_eq!(read(""), Ok(Clock::TwentyFour));
        assert_eq!(read("nonsense"), Ok(Clock::TwentyFour));
        assert_eq!(read("12"), Ok(Clock::Twelve));
    }

    #[test]
    fn the_two_readings_a_person_is_offered_are_the_same_moment() {
        let Ok(long) = Clock::TwentyFour.says();
        let Ok(short) = Clock::Twelve.says();

        assert_eq!(long, "14:30");
        assert_eq!(short, "2:30 pm");
    }
}
