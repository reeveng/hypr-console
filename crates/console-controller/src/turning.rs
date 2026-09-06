//! The loop, said as one turn of it.
//!
//! What is here is which device is read, when one that has gone is looked for
//! again, and in what order the three are drained. None of that touches a
//! device: what a device is, is a trait, so the same loop runs against the
//! machine and against a world that exists only inside a test.

use std::collections::BTreeMap;

use console_never::Never;
use evdev::InputEvent;

use crate::doing::Doing;
use crate::finding::{self, Says};
use crate::means::Table;
use crate::reading::{Controller, From, Ranges};

pub struct Gone;

pub const READ: [From; 3] = [From::Pad, From::Keys, From::Touch];

pub const AWAY_SECONDS: f64 = 0.25;

pub const SETTLING_SECONDS: f64 = 0.5;

const DRY: usize = 64;

pub const HUNT_SECONDS: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Took {
    Held,
    Refused,
}

pub trait Plugged {
    fn every(&self) -> Vec<Says>;

    fn open(&mut self, path: &str) -> Took;

    fn ranges(&self, path: &str) -> Ranges;

    fn drain(&mut self, path: &str) -> Result<Vec<InputEvent>, Gone>;
}

#[derive(Debug, Default)]
pub struct Turning {
    pub held: Controller,
    told: BTreeMap<From, String>,
    open: BTreeMap<From, String>,
    hunted: BTreeMap<From, f64>,
    last: Option<f64>,
    settling: Option<f64>,
}

impl Turning {
    pub fn pointed_at(told: BTreeMap<From, String>) -> Result<Self, Never> {
        Ok(Turning { told, ..Turning::default() })
    }

    pub fn turn(&mut self, machine: &mut impl Plugged, now: f64) -> Result<Vec<Doing>, Never> {
        let since = self.last.map_or(0.0, |was| now - was);
        let away = self.last.is_some() && since > AWAY_SECONDS;
        self.last = Some(now);

        match away {
            true => self.settling = Some(now + SETTLING_SECONDS),
            false => {},
        }

        let deaf = away || self.settling.is_some_and(|until| now < until);

        match deaf {
            true => {},
            false => self.settling = None,
        }

        let Ok(()) = self.find(machine, now);

        let mut doing: Vec<Doing> = Vec::new();

        for which in READ {
            let Some(path) = self.open.get(&which).cloned() else { continue };

            for _ in 0..match deaf {
                true => DRY,
                false => 1,
            } {
                match machine.drain(&path) {
                    Ok(arrived) => {
                        let dry = arrived.is_empty();

                        for event in arrived {
                            let kind = event.event_type();
                            let Ok(did) = self.held.saw(which, kind, event.code(), event.value(), now);

                            match deaf {
                                true => {},
                                false => doing.extend(did),
                            }
                        }

                        match dry {
                            true => break,
                            false => {},
                        }
                    }
                    Err(Gone) => {
                        let Ok(went) = self.went(which);

                        doing.extend(went);
                        break;
                    }
                }
            }
        }

        match deaf {
            true => return Ok(Vec::new()),
            false => {},
        }

        let Ok(carried) = self.held.finger.carried();
        let Ok(ticked) = self.held.tick(since);

        doing.extend(carried);
        doing.extend(ticked);

        Ok(doing)
    }

    pub fn bound_by(&mut self, table: Table) -> Result<(), Never> {
        self.held.table = table;

        Ok(())
    }

    pub fn poll(&self) -> Result<f64, Never> {
        self.held.poll()
    }

    pub fn missing(&self) -> Result<Vec<From>, Never> {
        Ok(READ.into_iter().filter(|which| !self.open.contains_key(which)).collect())
    }

    pub fn holding(&self) -> Result<&BTreeMap<From, String>, Never> {
        Ok(&self.open)
    }

    fn went(&mut self, which: From) -> Result<Vec<Doing>, Never> {
        self.open.remove(&which);

        match which == From::Pad {
            true => self.held.pad_went(),
            false => Ok(Vec::new()),
        }
    }

    fn find(&mut self, machine: &mut impl Plugged, now: f64) -> Result<(), Never> {
        let Ok(missing) = self.missing();

        for which in missing {
            let looked = self.hunted.get(&which).copied().unwrap_or(f64::NEG_INFINITY);

            match now - looked < HUNT_SECONDS {
                true => continue,
                false => {},
            }

            self.hunted.insert(which, now);

            let Ok(found) = self.at(machine, which);

            let Some(path) = found else { continue };

            match machine.open(&path) {
                Took::Refused => continue,
                Took::Held => {},
            }

            match which {
                From::Pad => {
                    let Ok(()) = self.held.reading(machine.ranges(&path));
                },
                From::Keys | From::Touch => {},
            }

            self.open.insert(which, path);
        }

        Ok(())
    }

    fn at(&self, machine: &impl Plugged, which: From) -> Result<Option<String>, Never> {
        match self.told.get(&which) {
            Some(path) => return Ok(Some(path.clone())),
            None => {},
        }

        let said = machine.every();
        let Ok(found) = match which {
            From::Pad => finding::gamepad(&said),
            From::Keys => finding::keyboard(&said),
            From::Touch => finding::touchpad(&said),
        };

        Ok(found.map(|says| says.path.clone()))
    }
}
