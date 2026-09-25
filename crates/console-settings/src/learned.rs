//! How bright she likes it in this much light, learned from the rocker.
//!
//! Nothing here ships a curve, and that is the whole argument. A table of lux
//! against brightness written in this repository would be one person's eyes on
//! one panel in one room, and every machine that ever ran it would inherit a
//! preference nobody on it had expressed. The sensor cannot be calibrated here
//! either: what the raw number means depends on the glass over it, and what a
//! comfortable screen is depends on who is looking.
//!
//! So it is taught instead. Every press of the brightness rocker is a person
//! saying what they want in the light they are sitting in, which is a reading
//! and a level and therefore a sample, and it costs them nothing to give
//! because they were pressing it anyway. Before the first press this knows
//! nothing and does nothing; the first thing it ever does is something it was
//! told.
//!
//! **A band and not a curve.** The light in a room runs over four decades from
//! a dark bedroom to direct sun, the eye reads it as a logarithm, and nobody
//! is going to press the rocker often enough to fit anything. So the ladder is
//! a band per half-decade of the sensor's own number, one level held in each,
//! and a band that has been taught twice keeps half of what it knew and half
//! of what it has just been told -- so a press made for some other reason
//! moves it rather than replacing it, and two presses agreeing carry it most
//! of the way.
//!
//! **Between taught bands it interpolates, outside them it holds.** Linear in
//! the band, which is logarithmic in the light, which is the shape the eye
//! reads by. A machine taught one band answers that level everywhere: wrong in
//! bright sun, and wrong in the direction of a screen somebody can still see.
//!
//! **Being wrong costs one press, and the press is the next sample.** That is
//! what makes the whole of this safe to run without asking. There is no state
//! it can get into that a person cannot correct in the way they were already
//! going to, and correcting it is how it stops being wrong.

use std::path::{Path, PathBuf};

use console_core_atomic_writes::Stored;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::light::Lit;
use crate::screen::{BRIGHTEST, DARKEST};

pub const NAMED: &str = "learned-brightness";

pub const ASKED: &str = "follow-light";

pub const EDGES: [i64; 10] = [1, 3, 10, 30, 100, 300, 1000, 3000, 10000, 30000];

pub const BANDS: u32 = 11;

pub const HALVES: i64 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Band(pub u32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Levels {
    pub bands: Vec<Option<i64>>,
}

pub fn band(lit: Lit) -> Result<Band, Never> {
    let Lit(raw) = lit;

    let Ok(under) = fitted(EDGES.iter().filter(|edge| **edge <= raw).count());

    Ok(Band(under))
}

fn ladder(which: u32) -> Result<i64, Never> {
    fitted::<_, i64>(which)
}

fn between(under: (u32, i64), over: (u32, i64), which: u32) -> Result<i64, Never> {
    let (low, at_low) = under;
    let (high, at_high) = over;

    let Ok(low) = ladder(low);
    let Ok(high) = ladder(high);
    let Ok(which) = ladder(which);

    let across = high.saturating_sub(low).max(1);
    let along = which.saturating_sub(low);
    let climb = at_high.saturating_sub(at_low);

    Ok(at_low.saturating_add(climb.saturating_mul(along).saturating_div(across)))
}

impl Levels {
    pub fn nothing() -> Result<Self, Never> {
        let Ok(many) = index(BANDS);

        Ok(Levels { bands: vec![None; many] })
    }

    fn under(&self, which: u32) -> Result<Option<(u32, i64)>, Never> {
        let Ok(which) = index(which);

        Ok(self
            .bands
            .iter()
            .enumerate()
            .take(which)
            .filter_map(|(at, held)| held.map(|level| (at, level)))
            .next_back()
            .map(|(at, level)| {
                let Ok(at) = fitted(at);

                (at, level)
            }))
    }

    fn over(&self, which: u32) -> Result<Option<(u32, i64)>, Never> {
        let Ok(which) = index(which);

        Ok(self
            .bands
            .iter()
            .enumerate()
            .skip(which.saturating_add(1))
            .filter_map(|(at, held)| held.map(|level| (at, level)))
            .next()
            .map(|(at, level)| {
                let Ok(at) = fitted(at);

                (at, level)
            }))
    }

    pub fn taught(&self, band: Band, level: i64) -> Result<Self, Never> {
        let Band(which) = band;
        let Ok(at) = index(which);

        let mut bands = self.bands.clone();

        let slot = match bands.get_mut(at) {
            Some(slot) => slot,
            None => return Ok(self.clone()),
        };

        let level = level.clamp(DARKEST, BRIGHTEST);

        *slot = Some(match *slot {
            Some(was) => was.saturating_add(level).saturating_div(HALVES),
            None => level,
        });

        Ok(Levels { bands })
    }

    pub fn asked(&self, band: Band) -> Result<Option<i64>, Never> {
        let Band(which) = band;
        let Ok(at) = index(which);

        let here = match self.bands.get(at) {
            Some(here) => *here,
            None => return Ok(None),
        };

        match here {
            Some(here) => return Ok(Some(here)),
            None => {},
        }

        let Ok(under) = self.under(which);
        let Ok(over) = self.over(which);

        let climbed = match (under, over) {
            (Some(under), Some(over)) => {
                let Ok(climbed) = between(under, over, which);

                Some(climbed)
            }
            (Some((_below, at_low)), None) => Some(at_low),
            (None, Some((_above, at_high))) => Some(at_high),
            (None, None) => None,
        };

        Ok(climbed)
    }

    pub fn read(held: &str) -> Result<Self, Never> {
        let Ok(nothing) = Levels::nothing();

        Ok(held.lines().fold(nothing, |levels, line| {
            let mut said = line.split_whitespace();

            let which = match said.next().map(str::parse::<u32>) {
                Some(Ok(which)) => which,
                None => return levels,
                Some(Err(_not_a_number)) => return levels,
            };

            let level = match said.next().map(str::parse::<i64>) {
                Some(Ok(level)) => level,
                None => return levels,
                Some(Err(_not_a_number)) => return levels,
            };

            let mut bands = levels.bands.clone();
            let Ok(which) = index(which);

            match bands.get_mut(which) {
                Some(slot) => *slot = Some(level.clamp(DARKEST, BRIGHTEST)),
                None => return levels,
            }

            Levels { bands }
        }))
    }

    pub fn written(&self) -> Result<String, Never> {
        Ok(self
            .bands
            .iter()
            .enumerate()
            .filter_map(|(at, held)| held.map(|level| format!("{at} {level}\n")))
            .collect())
    }
}

pub fn at(home: &Path) -> Result<PathBuf, Never> {
    let Ok(ours) = console_core_places::Base::Configuration.ours_under(home);

    Ok(ours.join(NAMED))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Standing {
    Loaded(Levels),
    Invalid(String),
}

pub fn standing(home: &Path) -> Result<Standing, Never> {
    let Ok(at) = at(home);
    let Ok(held) = console_core_atomic_writes::read(&at);

    Ok(match held {
        Stored::Text(said) => {
            let Ok(levels) = Levels::read(&said);

            Standing::Loaded(levels)
        }
        Stored::Absent => {
            let Ok(nothing) = Levels::nothing();

            Standing::Loaded(nothing)
        }
        Stored::Failed(why) => Standing::Invalid(why),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Following {
    Yes,
    No,
}

impl Following {
    pub fn read(held: &str) -> Result<Self, Never> {
        Ok(match held.trim() {
            "no" => Following::No,
            _nothing_said_or_anything_else => Following::Yes,
        })
    }

    pub fn other(self) -> Result<Self, Never> {
        Ok(match self {
            Following::Yes => Following::No,
            Following::No => Following::Yes,
        })
    }

    pub fn written(self) -> Result<&'static str, Never> {
        Ok(match self {
            Following::Yes => "yes",
            Following::No => "no",
        })
    }
}

pub fn asked_at(home: &Path) -> Result<PathBuf, Never> {
    let Ok(ours) = console_core_places::Base::Configuration.ours_under(home);

    Ok(ours.join(ASKED))
}

pub fn following(home: &Path) -> Result<Following, Never> {
    let Ok(at) = asked_at(home);
    let Ok(held) = console_core_atomic_writes::read(&at);

    Ok(match held {
        Stored::Text(said) => Following::read(&said)?,
        Stored::Absent => Following::Yes,
        Stored::Failed(_a_switch_that_will_not_say_is_a_switch_nobody_set) => Following::Yes,
    })
}

pub fn keep(home: &Path, levels: &Levels) -> Result<(), console_core_atomic_writes::Unwritten> {
    let Ok(at) = at(home);
    let Ok(written) = levels.written();

    console_core_atomic_writes::whole(&at, written.as_bytes())
}
