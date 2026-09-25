//! What the battery does on the way down, and where each of those starts.
//!
//! Three things happen as a battery empties, and all three are one question
//! asked at three depths: say it, say it louder, and stop the machine before
//! the machine is stopped for it. Where each of them starts is a person's own
//! answer -- twenty-five is early on a quiet evening and late in a game -- so
//! the three numbers are settings, kept where the other settings no one else
//! owns are kept, and this is the shape of them.
//!
//! It is the one reading on this device that moves without anyone pressing
//! anything, which is what makes it worth saying and also what makes it harder
//! than the screen and the volume. Those are raised by the press that caused
//! them and have nowhere else to be. This has no press, so something has to be
//! watching, and whatever watches has to decide when a crossing happened
//! rather than when a number was read.
//!
//! **Whether the cable is in is asked of the cable.** The battery's own status
//! word was the whole of this answer and it cannot carry it: a handheld plugged
//! in at a charge limit says `Not charging`, which is the truth about the
//! battery and reads as a machine running itself flat. That was one icon saying
//! the wrong thing on the bar, and the same mistake refusing an apply on a
//! device sitting on its charger. So the mains supply is read, where being
//! plugged in is a fact rather than something inferred from what the chemistry
//! is doing, and [`Filling`] gained the third state that was always there --
//! filling, charged, or on its own -- with [`Filling::cable`] for the callers whose
//! question is only whether the machine is on the wall.
//!
//! A machine with no mains supply at all is not a machine that is unplugged. It
//! is a machine that cannot be asked, and the status word decides on its own
//! there, which is what this did everywhere before there was anything better to
//! ask.
//!
//! Nothing here reads a clock or raises anything. What is decided and what is
//! done are kept apart as everywhere else here: this says what a reading has
//! come to, the `console-battery` program in `console-settings` is what does
//! it, and `bar-say battery` is the one thing on the machine reading the
//! battery at all.

use console_core_never::Never;
use console_core_words::Words;

const NOTHING_SAID: &str = "";


#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Words)]
pub enum Step {
    #[words(word = "low", key = "battery-low", says = "Low Battery Alert")]
    Low,
    #[words(word = "lower", key = "battery-lower", says = "Very Low Battery Alert")]
    Lower,
    #[words(word = "protect", key = "battery-protect", says = "Shut Down Before Empty")]
    Protect,
}

pub const EVERY: [Step; 3] = [Step::Low, Step::Lower, Step::Protect];

impl Step {
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
        let settings = console_defaults::read(said)?;
        let mut levels = Levels::default();

        for step in EVERY {
            let key = step.key()?;
            let found = settings.iter().find(|(named, _)| named == key);

            let (_key, value) = match found {
                Some((_key, value)) => (_key, value),
                None => continue,
            };

            let level = match value.parse() {
                Ok(level) => level,
                Err(fault) => {
                    eprintln!("console-battery: {key}: {value:?}: {fault}");

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
        let at = console_defaults::where_()?;

        let said = match at.map(|at| console_core_atomic_writes::text_or_empty(&at)) {
            Some(Ok(said)) => said,
            None => String::new(),
            Some(Err(fault)) => {
                eprintln!("console-battery: {fault}");

                String::new()
            }
        };


        Levels::read(&said)
    }

    pub fn set(self, step: Step, level: i32) -> Result<Self, Never> {
        let moved = self.with(step, level)?;
        let key = step.key()?;
        let at = moved.at(step)?;

        console_defaults::set(console_defaults::Setting { key, value: &at.to_string() })?;

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

pub const SUPPLIES: &str = "/sys/class/power_supply";

pub const MAINS: &str = "Mains";

pub const BATTERY: &str = "Battery";

pub const CHARGING: &str = "Charging";

pub const FULL: &str = "Full";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Words)]
pub enum Mains {
    #[words(word = "plugged")]
    On,
    #[words(word = "unplugged")]
    Off,
    #[default]
    #[words(word = "unasked")]
    NothingToAsk,
}

impl Mains {
    pub fn named(word: &str) -> Result<Self, Never> {
        Ok(match word {
            "plugged" => Mains::On,
            "unplugged" => Mains::Off,
            _nothing_said_about_a_cable => Mains::NothingToAsk,
        })
    }
}

pub fn mains() -> Result<Mains, Never> {
    let supplies = match std::fs::read_dir(SUPPLIES) {
        Ok(supplies) => supplies,
        Err(_this_machine_says_nothing_about_its_supplies) => return Ok(Mains::NothingToAsk),
    };
    let mut found = Mains::NothingToAsk;

    for supply in supplies.flatten() {
        let at = supply.path();

        let kind = match std::fs::read_to_string(at.join("type")) {
            Ok(kind) => kind,
            Err(_not_a_supply_that_says_what_it_is) => continue,
        };

        match kind.trim() == MAINS {
            true => {},
            false => continue,
        }

        let online = match std::fs::read_to_string(at.join("online")) {
            Ok(online) => online,
            Err(_a_mains_supply_that_will_not_say) => continue,
        };

        match online.trim() {
            "1" => return Ok(Mains::On),
            _not_this_one => found = Mains::Off,
        }
    }

    Ok(found)
}

pub fn charge() -> Result<String, Never> {
    let Ok(mains) = mains();
    let Ok(cable) = mains.word();

    let supplies = match std::fs::read_dir(SUPPLIES) {
        Ok(supplies) => supplies,
        Err(_unreadable) => return Ok(String::new()),
    };

    let found = supplies
        .flatten()
        .map(|supply| supply.path())
        .filter(|at| {
            at.file_name().is_some_and(|name| name.to_string_lossy().starts_with("BAT"))
        })
        .filter_map(|at| {
            let (capacity, status) = match (
                std::fs::read_to_string(at.join("capacity")),
                std::fs::read_to_string(at.join("status")),
            ) {
                (Ok(capacity), Ok(status)) => (capacity, status),
                (Err(_unreadable), _) | (_, Err(_unreadable)) => return None,
            };

            Some(format!("{} {cable} {}", capacity.trim(), status.trim()))
        })
        .next();

    Ok(match found {
        Some(said) => said,
        None => String::new(),
    })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Charge {
    pub percent: Option<i32>,
    pub filling: Filling,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Filling {
    Yes,
    Charged,
    #[default]
    No,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cable {
    Connected,
    Disconnected,
}

impl Filling {
    pub fn cable(self) -> Result<Cable, Never> {
        Ok(match self {
            Filling::Yes | Filling::Charged => Cable::Connected,
            Filling::No => Cable::Disconnected,
        })
    }
}

pub fn filling(mains: Mains, status: &str) -> Result<Filling, Never> {
    Ok(match (mains, status) {
        (_whatever_the_cable_says, CHARGING) => Filling::Yes,
        (Mains::On, FULL) => Filling::Yes,
        (Mains::On, _not_filling_but_on_the_cable) => Filling::Charged,
        (Mains::Off | Mains::NothingToAsk, FULL) => Filling::No,
        (Mains::Off | Mains::NothingToAsk, _on_its_own) => Filling::No,
    })
}

impl Charge {
    pub fn of(said: &str) -> Result<Self, Never> {
        let mut words = said.split_whitespace();
        let percent = words.next().and_then(|word| match word.parse() {
            Ok(percent) => Some(percent),
            Err(fault) => {
                eprintln!("console-battery: the battery said {word:?}: {fault}");

                None
            }
        });

        let cable = match words.next() {
            Some(cable) => cable,
            None => NOTHING_SAID,
        };

        let Ok(mains) = Mains::named(cable);
        let status = words.collect::<Vec<&str>>().join(" ");
        let Ok(filling) = filling(mains, &status);

        Ok(Charge { percent, filling })
    }
}

pub const MARGIN: i32 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Alert {
    pub act: Option<Step>,
    pub told: Option<Step>,
}

pub fn asked(
    levels: Levels,
    charge: i32,
    filling: Filling,
    told: Option<Step>,
) -> Result<Alert, Never> {
    let Ok(cable) = filling.cable();

    match cable {
        Cable::Connected => return Ok(Alert { act: None, told: None }),
        Cable::Disconnected => {}
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
        Some(now) => match held.is_none_or(|before| now > before) {
            true => Alert { act: Some(now), told: Some(now) },
            false => Alert { act: None, told: held },
        },
        None => Alert { act: None, told: held },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_is_said_once_and_not_again_while_it_is_held() {
        let levels = Levels::default();
        let Ok(first) = asked(levels, 19, Filling::No, None);

        assert_eq!(first, Alert { act: Some(Step::Low), told: Some(Step::Low) });

        let Ok(again) = asked(levels, 19, Filling::No, first.told);

        assert_eq!(again, Alert { act: None, told: Some(Step::Low) });

        let Ok(lower) = asked(levels, 18, Filling::No, again.told);

        assert_eq!(lower.act, None, "still the same step");
    }

    #[test]
    fn a_reading_that_falls_through_two_steps_owes_the_deeper_one() {
        let Ok(said) = asked(Levels::default(), 4, Filling::No, Some(Step::Low));

        assert_eq!(said, Alert { act: Some(Step::Protect), told: Some(Step::Protect) });
    }

    #[test]
    fn nothing_happens_to_a_battery_that_is_filling() {
        let Ok(said) = asked(Levels::default(), 3, Filling::Yes, Some(Step::Lower));

        assert_eq!(said, Alert { act: None, told: None });
    }

    #[test]
    fn nothing_happens_to_a_battery_on_the_cable_that_is_not_filling() {
        let Ok(said) = asked(Levels::default(), 3, Filling::Charged, Some(Step::Lower));

        assert_eq!(
            said,
            Alert { act: None, told: None },
            "a machine at its charge limit was told it was about to run out"
        );
    }

    #[test]
    fn a_battery_that_is_not_filling_on_the_cable_is_still_on_the_cable() {
        assert_eq!(Filling::Charged.cable(), Ok(Cable::Connected));
        assert_eq!(Filling::Yes.cable(), Ok(Cable::Connected));
        assert_eq!(Filling::No.cable(), Ok(Cable::Disconnected));
    }

    #[test]
    fn a_charge_limit_reads_as_held_and_not_as_a_machine_running_flat() {
        let Ok(said) = filling(Mains::On, "Not charging");

        assert_eq!(said, Filling::Charged);
        assert_eq!(Charge::of("78 plugged Not charging"), Ok(Charge { percent: Some(78), filling: Filling::Charged }));
    }

    #[test]
    fn a_machine_with_no_mains_supply_is_read_the_way_it_always_was() {
        assert_eq!(filling(Mains::NothingToAsk, "Charging"), Ok(Filling::Yes));
        assert_eq!(filling(Mains::NothingToAsk, "Discharging"), Ok(Filling::No));
        assert_eq!(filling(Mains::NothingToAsk, "Not charging"), Ok(Filling::No));
    }

    #[test]
    fn a_cable_that_is_out_is_a_battery_on_its_own_whatever_it_is_full_of() {
        assert_eq!(filling(Mains::Off, "Full"), Ok(Filling::No));
        assert_eq!(filling(Mains::On, "Full"), Ok(Filling::Yes));
    }

    #[test]
    fn what_the_cable_is_called_survives_being_written_down_and_read_back() {
        for mains in [Mains::On, Mains::Off, Mains::NothingToAsk] {
            let Ok(word) = mains.word();

            assert_eq!(Mains::named(word), Ok(mains));
        }

        assert_eq!(Mains::named("something else"), Ok(Mains::NothingToAsk));
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
            Charge::of("72 unplugged Discharging"),
            Ok(Charge { percent: Some(72), filling: Filling::No })
        );
        assert_eq!(
            Charge::of("100 plugged Full"),
            Ok(Charge { percent: Some(100), filling: Filling::Yes })
        );
        assert_eq!(Charge::of(""), Ok(Charge { percent: None, filling: Filling::No }));
    }

    #[test]
    fn a_machine_with_no_battery_is_not_a_machine_about_to_stop() {
        let Ok(none) = Charge::of("");
        let Ok(empty) = Charge::of("0 unplugged Discharging");

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
