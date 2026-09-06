//! What the battery does on the way down, and where each of those starts.
//!
//! Three things happen as a battery empties, and all three are one question
//! asked at three depths: say it, say it louder, and stop the machine before
//! the machine is stopped for it. Where each of them starts is a person's own
//! answer -- twenty-five is early on a quiet evening and late in a game -- so
//! the three numbers are settings, kept where the other settings nobody else
//! owns are kept, and this is the shape of them.
//!
//! It is the one reading on this device that moves without anybody pressing
//! anything, which is what makes it worth saying and also what makes it harder
//! than the screen and the volume. Those are raised by the press that caused
//! them and have nowhere else to be. This has no press, so something has to be
//! watching, and whatever watches has to decide when a crossing happened
//! rather than when a number was read.
//!
//! Nothing here reads a clock or raises anything. What is decided and what is
//! done are kept apart as everywhere else here: this says what a reading has
//! come to, `console-battery` is what does it, and `bar-say battery` is the
//! one thing on the machine reading the battery at all.

use console_never::Never;

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum Step {
    Low,
    Lower,
    Protect,
}

pub const EVERY: [Step; 3] = [Step::Low, Step::Lower, Step::Protect];

impl Step {
    pub fn word(self) -> Result<&'static str, Never> {
        Ok(match self {
            Step::Low => "low",
            Step::Lower => "lower",
            Step::Protect => "protect",
        })
    }

    pub fn named(word: &str) -> Result<Option<Self>, Never> {
        for step in EVERY {
            let named = step.word()?;

            match named == word {
                true => return Ok(Some(step)),
                false => {},
            }
        }

        Ok(None)
    }

    pub fn key(self) -> Result<&'static str, Never> {
        Ok(match self {
            Step::Low => "battery-low",
            Step::Lower => "battery-lower",
            Step::Protect => "battery-protect",
        })
    }

    pub fn says(self) -> Result<&'static str, Never> {
        Ok(match self {
            Step::Low => "Say it is getting low",
            Step::Lower => "Say it is getting really low",
            Step::Protect => "Stop before the battery does",
        })
    }

    pub fn at(self) -> Result<i32, Never> {
        Ok(match self {
            Step::Low => 25,
            Step::Lower => 10,
            Step::Protect => 5,
        })
    }

    pub fn number(self) -> Result<u32, Never> {
        Ok(match self {
            Step::Low => 1,
            Step::Lower => 2,
            Step::Protect => 3,
        })
    }

    pub fn of_number(number: u32) -> Result<Option<Self>, Never> {
        for step in EVERY {
            let numbered = step.number()?;

            match numbered == number {
                true => return Ok(Some(step)),
                false => {},
            }
        }

        Ok(None)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Levels {
    pub low: i32,
    pub lower: i32,
    pub protect: i32,
}

impl Default for Levels {
    fn default() -> Self {
        let Ok(low) = Step::Low.at();
        let Ok(lower) = Step::Lower.at();
        let Ok(protect) = Step::Protect.at();

        Levels { low, lower, protect }
    }
}

pub const NEVER: i32 = 0;

impl Levels {
    pub fn at(self, step: Step) -> Result<i32, Never> {
        Ok(match step {
            Step::Low => self.low,
            Step::Lower => self.lower,
            Step::Protect => self.protect,
        })
    }

    pub fn with(self, step: Step, level: i32) -> Result<Self, Never> {
        let (floor, ceiling) = match step {
            Step::Low => (self.lower, 100),
            Step::Lower => (self.protect, self.low),
            Step::Protect => (NEVER, self.lower),
        };
        let level = level.clamp(floor, ceiling);
        let mut levels = self;

        match step {
            Step::Low => levels.low = level,
            Step::Lower => levels.lower = level,
            Step::Protect => levels.protect = level,
        }

        Ok(levels)
    }

    pub fn sane(self) -> Result<Self, Never> {
        let low = self.low.clamp(NEVER, 100);
        let lower = self.lower.clamp(NEVER, low);
        let protect = self.protect.clamp(NEVER, lower);

        Ok(Levels { low, lower, protect })
    }

    pub fn read(said: &str) -> Result<Self, Never> {
        let settings = crate::read(said)?;
        let mut levels = Levels::default();

        for step in EVERY {
            let key = step.key()?;
            let found = settings.iter().find(|(named, _)| named == key);

            let Some((_key, value)) = found else { continue };

            let level = match value.parse() {
                Ok(level) => level,
                Err(fault) => {
                    eprintln!("console-defaults: {key}: {value:?}: {fault}");

                    continue;
                }
            };

            match step {
                Step::Low => levels.low = level,
                Step::Lower => levels.lower = level,
                Step::Protect => levels.protect = level,
            }
        }

        levels.sane()
    }

    pub fn here() -> Result<Self, Never> {
        let at = crate::where_()?;

        let said = match std::fs::read_to_string(&at) {
            Ok(said) => said,
            Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(fault) => {
                eprintln!("console-defaults: {}: {fault}", at.display());

                String::new()
            }
        };

        Levels::read(&said)
    }

    pub fn set(self, step: Step, level: i32) -> Result<Self, Never> {
        let moved = self.with(step, level)?;
        let key = step.key()?;
        let at = moved.at(step)?;

        crate::set(key, &at.to_string())?;

        Ok(moved)
    }

    pub fn reached(self, charge: i32) -> Result<Option<Step>, Never> {
        for step in EVERY.into_iter().rev() {
            let level = self.at(step)?;

            match level != NEVER && charge <= level {
                true => return Ok(Some(step)),
                false => {},
            }
        }

        Ok(None)
    }
}

pub fn charge() -> Result<String, Never> {
    let Ok(supplies) = std::fs::read_dir("/sys/class/power_supply") else {
        return Ok(String::new());
    };

    Ok(supplies
        .flatten()
        .map(|supply| supply.path())
        .filter(|at| {
            at.file_name().is_some_and(|name| name.to_string_lossy().starts_with("BAT"))
        })
        .filter_map(|at| {
            let (Ok(capacity), Ok(status)) = (
                std::fs::read_to_string(at.join("capacity")),
                std::fs::read_to_string(at.join("status")),
            ) else {
                return None;
            };

            Some(format!("{} {}", capacity.trim(), status.trim()))
        })
        .next()
        .unwrap_or_default())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Charge {
    pub percent: Option<i32>,
    pub filling: Filling,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Filling {
    Yes,
    #[default]
    No,
}

impl Charge {
    pub fn of(said: &str) -> Result<Self, Never> {
        let mut words = said.split_whitespace();
        let percent = words.next().and_then(|word| match word.parse() {
            Ok(percent) => Some(percent),
            Err(fault) => {
                eprintln!("console-defaults: the battery said {word:?}: {fault}");

                None
            }
        });
        let filling = match words.next().is_some_and(|word| word == "Charging" || word == "Full") {
            true => Filling::Yes,
            false => Filling::No,
        };

        Ok(Charge { percent, filling })
    }
}

pub const MARGIN: i32 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Said {
    pub act: Option<Step>,
    pub told: Option<Step>,
}

pub fn asked(
    levels: Levels,
    charge: i32,
    filling: Filling,
    told: Option<Step>,
) -> Result<Said, Never> {
    match filling {
        Filling::Yes => return Ok(Said { act: None, told: None }),
        Filling::No => {}
    }

    let held = match told {
        Some(step) => {
            let level = levels.at(step)?;

            match charge < level.saturating_add(MARGIN) {
                true => Some(step),
                false => None,
            }
        }
        None => None,
    };

    let reached = levels.reached(charge)?;

    Ok(match reached {
        Some(now) if held.is_none_or(|before| now > before) => {
            Said { act: Some(now), told: Some(now) }
        }
        Some(_) => Said { act: None, told: held },
        None => Said { act: None, told: held },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_is_said_once_and_not_again_while_it_is_held() {
        let levels = Levels::default();
        let Ok(first) = asked(levels, 19, Filling::No, None);

        assert_eq!(first, Said { act: Some(Step::Low), told: Some(Step::Low) });

        let Ok(again) = asked(levels, 19, Filling::No, first.told);

        assert_eq!(again, Said { act: None, told: Some(Step::Low) });

        let Ok(lower) = asked(levels, 18, Filling::No, again.told);

        assert_eq!(lower.act, None, "still the same step");
    }

    #[test]
    fn a_reading_that_falls_through_two_steps_owes_the_deeper_one() {
        let Ok(said) = asked(Levels::default(), 4, Filling::No, Some(Step::Low));

        assert_eq!(said, Said { act: Some(Step::Protect), told: Some(Step::Protect) });
    }

    #[test]
    fn nothing_happens_to_a_battery_that_is_filling() {
        let Ok(said) = asked(Levels::default(), 3, Filling::Yes, Some(Step::Lower));

        assert_eq!(said, Said { act: None, told: None });
    }

    #[test]
    fn a_charge_that_climbs_clear_of_a_step_can_meet_it_again() {
        let levels = Levels::default();
        let Ok(climbed) = asked(levels, 40, Filling::No, Some(Step::Low));
        let Ok(met) = asked(levels, 24, Filling::No, None);

        assert_eq!(climbed.told, None);
        assert_eq!(met.act, Some(Step::Low));
    }

    #[test]
    fn a_reading_wobbling_on_a_step_does_not_say_it_twice() {
        let levels = Levels::default();
        let Ok(said) = asked(levels, 25, Filling::No, None);

        assert_eq!(said.act, Some(Step::Low));

        let Ok(above) = asked(levels, 26, Filling::No, said.told);
        let Ok(back) = asked(levels, 25, Filling::No, said.told);

        assert_eq!(above.told, Some(Step::Low));
        assert_eq!(back.act, None);
    }

    #[test]
    fn a_step_set_to_never_is_never_reached() {
        let levels = Levels { low: 25, lower: 10, protect: NEVER };

        assert_eq!(levels.reached(0), Ok(Some(Step::Lower)));
        assert_eq!(Levels { low: NEVER, lower: NEVER, protect: NEVER }.reached(0), Ok(None));
    }

    #[test]
    fn a_step_stops_where_the_one_under_it_is() {
        let levels = Levels::default();
        let Ok(up) = levels.with(Step::Lower, 30);
        let Ok(down) = levels.with(Step::Lower, 0);
        let Ok(full) = levels.with(Step::Low, 100);

        assert_eq!(up.lower, levels.low);
        assert_eq!(down.lower, levels.protect);
        assert_eq!(full.low, 100);
    }

    #[test]
    fn a_file_with_the_three_in_the_wrong_order_is_read_in_order() {
        let said = "battery-low=5\nbattery-lower=40\nbattery-protect=90\n";

        assert_eq!(Levels::read(said), Ok(Levels { low: 5, lower: 5, protect: 5 }));
    }

    #[test]
    fn a_file_that_says_nothing_is_this_desktops_own_answers() {
        assert_eq!(Levels::read(""), Ok(Levels::default()));
        assert_eq!(Levels::read("search=startpage\n"), Ok(Levels::default()));
    }

    #[test]
    fn a_charge_is_a_number_and_whether_it_is_filling() {
        assert_eq!(
            Charge::of("72 Discharging"),
            Ok(Charge { percent: Some(72), filling: Filling::No })
        );
        assert_eq!(Charge::of("100 Full"), Ok(Charge { percent: Some(100), filling: Filling::Yes }));
        assert_eq!(Charge::of(""), Ok(Charge { percent: None, filling: Filling::No }));
    }

    #[test]
    fn a_machine_with_no_battery_is_not_a_machine_about_to_stop() {
        let Ok(none) = Charge::of("");
        let Ok(empty) = Charge::of("0 Discharging");

        assert_eq!(none.percent, None);
        assert_eq!(empty.percent, Some(0));
    }

    #[test]
    fn a_step_survives_as_one_number() {
        for step in EVERY {
            let Ok(number) = step.number();

            assert_eq!(Step::of_number(number), Ok(Some(step)));
        }

        assert_eq!(Step::of_number(0), Ok(None));
    }

    #[test]
    fn a_step_is_asked_for_by_its_own_word() {
        assert_eq!(Step::named("protect"), Ok(Some(Step::Protect)));
        assert_eq!(Step::named("nothing"), Ok(None));
    }
}
