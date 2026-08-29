//! The loop, said as one turn of it.
//!
//! What is here is which device is read, when one that has gone is looked for
//! again, and in what order the three are drained. None of that touches a
//! device: what a device is, is a trait, so the same loop runs against the
//! machine and against a world that exists only inside a test.

use std::collections::BTreeMap;

use evdev::InputEvent;

use crate::doing::Doing;
use crate::finding::{self, Says};
use crate::reading::{Controller, From, Ranges};

/// A device that is no longer there.
///
/// A profile switch takes the pad and the keyboard away every time, so this is
/// the ordinary state of things rather than a fault.
pub struct Gone;

/// The devices this reads, in the order it reads them.
pub const READ: [From; 3] = [From::Pad, From::Keys, From::Touch];

/// How late a turn may be before what queued while it was away is thrown out.
///
/// The daemon is stopped outright while the on-screen keyboard is up, so that
/// a press does not both type a letter and do whatever the desktop makes of
/// it. Stopped is not deaf, though: the devices stay open, the kernel goes on
/// queueing on them, and the whole of it arrived at once the moment the
/// keyboard went away. Every button pressed while typing then happened in one
/// instant, in the order it was pressed, against a desktop that had moved on.
/// That is how the machine left for Game Mode on its own.
///
/// Well above a turn's own pace, so an ordinary loop never trips it, and far
/// below the time anybody spends typing.
pub const AWAY_SECONDS: f64 = 0.25;

/// How long after coming back it goes on throwing events away.
///
/// A backlog does not arrive in one read. A read returns what fits and leaves
/// the rest, the devices lost to a profile switch are opened again a turn or
/// two later, and InputPlumber hands back the state of a pad it has just
/// rebuilt. Every part of that is what was pressed while nobody was listening,
/// arriving in pieces over the moment afterwards, and the first turn back
/// catching only the first piece is how five presses of one button still
/// reached the desktop.
///
/// Short enough to be over before a hand that has just put the keyboard away
/// has reached for anything else.
pub const SETTLING_SECONDS: f64 = 0.5;

/// How many times a device is asked again while it is being emptied.
///
/// Only ever reached by a device handing over events faster than they can be
/// read, which is a device that has gone wrong. Everything else runs dry in
/// one or two.
const DRY: usize = 64;

/// How long before a device that has gone is looked for again.
///
/// Never sat on: a menu holds the pad away for as long as it is open, and
/// everything else this loop reads, the touchpad above all, has to keep
/// working while it does.
pub const HUNT_SECONDS: f64 = 1.0;

/// Whatever the devices are plugged into.
pub trait Plugged {
    /// Everything plugged in just now, as each describes itself.
    fn every(&self) -> Vec<Says>;

    /// Take hold of one, and say whether that worked.
    fn open(&mut self, path: &str) -> bool;

    /// The ranges one reports over.
    fn ranges(&self, path: &str) -> Ranges;

    /// Everything waiting on one, or word that it has gone.
    fn drain(&mut self, path: &str) -> Result<Vec<InputEvent>, Gone>;
}

/// The loop, between one turn and the next.
#[derive(Debug, Default)]
pub struct Turning {
    pub held: Controller,
    /// Where a device is, rather than where to look for it.
    ///
    /// Nothing sets these on the Legion Go, where looking is the right way
    /// round: the pad's path changes every time a profile is reloaded. They
    /// are how a test points this at the devices it made, on a machine that
    /// has a touchpad and a keyboard of its own answering to the same
    /// description.
    told: BTreeMap<From, String>,
    open: BTreeMap<From, String>,
    hunted: BTreeMap<From, f64>,
    last: Option<f64>,
    /// Until when what arrives is still what was pressed while it was away.
    settling: Option<f64>,
}

impl Turning {
    /// A loop pointed at devices somebody else made.
    pub fn pointed_at(told: BTreeMap<From, String>) -> Self {
        Turning { told, ..Turning::default() }
    }

    /// One turn: what arrived, and what it comes to.
    ///
    /// The time is handed in rather than read, because how far a page scrolled
    /// for a stick held so long is arithmetic, and arithmetic has one right
    /// answer.
    pub fn turn(&mut self, machine: &mut impl Plugged, now: f64) -> Vec<Doing> {
        let since = self.last.map_or(0.0, |was| now - was);
        // A turn this late is a daemon that was not running, and what queued
        // while nobody was listening is not a press. It is still read, because
        // the last of it is what the pad is doing now: which way the stick is
        // pushed and whether L2 is held are the state of the machine rather
        // than events, and a daemon that comes back not knowing them is a
        // daemon that comes back wrong. What it will not do is act on any of
        // it.
        let away = self.last.is_some() && since > AWAY_SECONDS;
        self.last = Some(now);
        if away {
            self.settling = Some(now + SETTLING_SECONDS);
        }
        // The turn it comes back on, and the moment after it while the rest of
        // the backlog is still arriving.
        let deaf = away || self.settling.is_some_and(|until| now < until);
        if !deaf {
            self.settling = None;
        }

        self.find(machine, now);
        let mut doing: Vec<Doing> = Vec::new();
        for which in READ {
            let Some(path) = self.open.get(&which).cloned() else { continue };
            // Read once when it is listening. Emptied, when it is not: one read
            // returns what fits in its buffer, and what is left behind is just
            // as stale as what came out.
            for _ in 0..match deaf {
                true => DRY,
                false => 1,
            } {
                match machine.drain(&path) {
                    Ok(arrived) => {
                        let dry = arrived.is_empty();
                        for event in arrived {
                            let kind = event.event_type();
                            let did = self.held.saw(which, kind, event.code(), event.value(), now);
                            if !deaf {
                                doing.extend(did);
                            }
                        }
                        if dry {
                            break;
                        }
                    }
                    Err(Gone) => {
                        self.went(which);
                        break;
                    }
                }
            }
        }
        if deaf {
            return Vec::new();
        }
        doing.extend(self.held.finger.carried());
        doing.extend(self.held.tick(since));
        doing
    }

    /// How long to wait before turning again.
    pub fn poll(&self) -> f64 {
        self.held.poll()
    }

    /// Which of the three this is not holding just now.
    pub fn missing(&self) -> Vec<From> {
        READ.into_iter().filter(|which| !self.open.contains_key(which)).collect()
    }

    /// Where each one it holds was found.
    pub fn holding(&self) -> &BTreeMap<From, String> {
        &self.open
    }

    fn went(&mut self, which: From) {
        self.open.remove(&which);
        if which == From::Pad {
            self.held.pad_went();
        }
    }

    fn find(&mut self, machine: &mut impl Plugged, now: f64) {
        for which in self.missing() {
            let looked = self.hunted.get(&which).copied().unwrap_or(f64::NEG_INFINITY);
            if now - looked < HUNT_SECONDS {
                continue;
            }
            self.hunted.insert(which, now);
            let Some(path) = self.at(machine, which) else { continue };
            if !machine.open(&path) {
                continue;
            }
            if which == From::Pad {
                self.held.reading(machine.ranges(&path));
            }
            self.open.insert(which, path);
        }
    }

    fn at(&self, machine: &impl Plugged, which: From) -> Option<String> {
        if let Some(path) = self.told.get(&which) {
            return Some(path.clone());
        }
        let said = machine.every();
        let found = match which {
            From::Pad => finding::gamepad(&said),
            From::Keys => finding::keyboard(&said),
            From::Touch => finding::touchpad(&said),
        };
        found.map(|says| says.path.clone())
    }
}
