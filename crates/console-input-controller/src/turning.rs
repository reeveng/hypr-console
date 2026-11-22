//! The loop, said as one turn of it.
//!
//! What is here is which device is read, when one that has gone is looked for
//! again, and in what order they are drained. None of that touches a device:
//! what a device is, is a trait, so the same loop runs against the machine and
//! against a world that exists only inside a test.
//!
//! A sort of device is one of them or all of them, and [`From::wants`] is
//! where that is said. Three of the four are one apiece -- there is one pad,
//! one keyboard InputPlumber makes, one touchpad -- and a keyboard someone
//! plugged in is every one of them, because two are as ordinary as one and a
//! press on the second has to count as much as a press on the first. That is
//! also why the hunt keeps looking after one is open: a second keyboard
//! arrives an hour later and nothing else would go and find it.
//!
//! How long to wait before the next turn is said here too, as a [`Wake`],
//! because it is a fact about what is held rather than about the machine. A
//! turn used to come fifty times a second forever, idle with the screen dark,
//! when almost everything this reads arrives as an event the kernel would wake
//! the loop for. What does not is a stick held over, a button repeating, a
//! finger on the pad and a device that is missing, so those are what ask for a
//! turn on a clock, and everything else waits for a press. The time a turn
//! hands the wheel and the repeat is the time since the last one only when one
//! of them was being looked at across it: a stick pushed after a minute of
//! nothing is a stick pushed now, not a minute of scrolling owed.
//!
//! A device there is one of that went away and was found again is said as an
//! [`Effect::Reconnected`] with how long it was gone, so a pad that drops
//! presses is a line somebody can count rather than a feeling. It is said even
//! on a turn that is otherwise deaf, because the turn after a resume is when
//! a pad is most likely to be coming back.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use console_core_never::Never;
use console_input_event_devices::InputEvent;

use crate::actions::Table;
use crate::clock::{Instant, Seconds};
use crate::effect::{Effect, Reconnected};
use crate::finding::{self, DeviceInfo};
use crate::reading::{Controller, From, Ranges, Wake, Wants};

const NEVER_LOOKED: Seconds = Seconds(f64::NEG_INFINITY);

pub struct Closed;

pub const READ: [From; 4] = [From::Pad, From::Keys, From::Touch, From::Typing];

pub const AWAY_SECONDS: f64 = 0.25;

pub const SETTLING_SECONDS: f64 = 0.5;

const DRY: u32 = 64;

pub const HUNT_SECONDS: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Took {
  Acquired,
  Denied,
}

pub trait Plugged {
  fn every(&self) -> Vec<DeviceInfo>;

  fn open(&mut self, path: &str) -> Took;

  fn ranges(&self, path: &str) -> Ranges;

  fn drain(&mut self, path: &str) -> Result<Vec<InputEvent>, Closed>;
}

enum Draining {
  Listening,
  Settling,
}

#[derive(Debug, Default)]
pub struct Turning {
  pub held: Controller,
  told: BTreeMap<From, String>,
  open: BTreeMap<From, BTreeSet<String>>,
  hunted: BTreeMap<From, Seconds>,
  gone: BTreeMap<From, Seconds>,
  last: Option<Instant>,
  settling: Option<Seconds>,
}

impl Turning {
  pub fn pointed_at(told: BTreeMap<From, String>) -> Result<Self, Never> {
    Ok(Turning {
      told,
      ..Turning::default()
    })
  }

  pub fn turn(
    &mut self,
    machine: &mut impl Plugged,
    when: Instant,
  ) -> Result<Vec<Effect>, Never> {
    let Ok(across) = self.held.wake();

    let (gap, slept) = match self.last {
      Some(was) => (
        when.since_boot.0 - was.since_boot.0,
        when.suspended.0 - was.suspended.0,
      ),
      None => (0.0, 0.0),
    };

    let since = match across {
      Wake::Within(_) => gap,
      Wake::OnInput => 0.0,
    };

    let away = slept > AWAY_SECONDS;
    let now = when.since_boot;
    self.last = Some(when);

    match ::core::primitive::bool::then_some(away, ()) {
      ::core::option::Option::Some(()) => self.settling = Some(Seconds(now.0 + SETTLING_SECONDS)),
      ::core::option::Option::None => {}
    }

    let deaf = away || self.settling.is_some_and(|until| now < until);

    match ::core::primitive::bool::then_some(deaf, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => self.settling = None,
    }

    let Ok(found) = self.find(machine, now);

    let mut effect: Vec<Effect> = found.clone();

    for which in READ {
      let paths: Vec<String> = match self.open.get(&which) {
        Some(paths) => paths.iter().cloned().collect(),
        None => Vec::new(),
      };

      for path in paths {
        let draining = match ::core::primitive::bool::then_some(deaf, ()) {
          ::core::option::Option::Some(()) => Draining::Settling,
          ::core::option::Option::None => Draining::Listening,
        };
        let Ok(drained) = self.drain_device(machine, which, &path, now, draining);

        effect.extend(drained);
      }
    }

    match ::core::primitive::bool::then_some(deaf, ()) {
      ::core::option::Option::Some(()) => return Ok(found),
      ::core::option::Option::None => {}
    }

    let Ok(carried) = self.held.finger.flush_motion();
    let Ok(ticked) = self.held.tick(Seconds(since));

    effect.extend(carried);
    effect.extend(ticked);

    Ok(effect)
  }

  fn drain_device(
    &mut self,
    machine: &mut impl Plugged,
    which: From,
    path: &str,
    now: Seconds,
    draining: Draining,
  ) -> Result<Vec<Effect>, Never> {
    let tries = match draining {
      Draining::Listening => 1,
      Draining::Settling => DRY,
    };
    let mut effects = Vec::new();

    for _ in 0..tries {
      let arrived = match machine.drain(path) {
        Ok(arrived) => arrived,
        Err(Closed) => {
          let Ok(went) = self.remove_device(which, path, now);
          effects.extend(went);

          break;
        }
      };

      match ::core::primitive::bool::then_some(arrived.is_empty(), ()) {
        ::core::option::Option::Some(()) => break,
        ::core::option::Option::None => {}
      }

      for event in arrived {
        let Ok(did) = self
          .held
          .saw(which, event.kind, event.code, event.value, now);

        match draining {
          Draining::Listening => effects.extend(did),
          Draining::Settling => {}
        }
      }
    }

    Ok(effects)
  }

  pub fn bound_by(&mut self, table: Table) -> Result<(), Never> {
    self.held.table = table;

    Ok(())
  }

  pub fn wake(&self) -> Result<Wake, Never> {
    let Ok(held) = self.held.wake();

    let lost = READ.into_iter().any(|which| {
      let Ok(wants) = which.wants();

      wants == Wants::One && !self.open.contains_key(&which)
    });

    match ::core::primitive::bool::then_some(lost, ()) {
      ::core::option::Option::Some(()) => held.sooner(Wake::Within(HUNT_SECONDS)),
      ::core::option::Option::None => Ok(held),
    }
  }

  pub fn hunt_now(&mut self) -> Result<(), Never> {
    self.hunted.clear();

    Ok(())
  }

  pub fn missing_devices(&self) -> Result<Vec<From>, Never> {
    Ok(
      READ
        .into_iter()
        .filter(|which| !self.open.contains_key(which))
        .collect(),
    )
  }

  pub fn open_devices(&self) -> Result<Vec<(From, String)>, Never> {
    Ok(
      self
        .open
        .iter()
        .flat_map(|(which, paths)| paths.iter().map(|at| (*which, at.clone())))
        .collect(),
    )
  }

  fn remove_device(
    &mut self,
    which: From,
    path: &str,
    now: Seconds,
  ) -> Result<Vec<Effect>, Never> {
    let empty = match self.open.get_mut(&which) {
      Some(paths) => {
        let _ = paths.remove(path);

        paths.is_empty()
      }
      None => false,
    };

    let Ok(wants) = which.wants();

    match (empty, wants) {
      (true, Wants::One) => {
        let _ = self.open.remove(&which);
        let _ = self.gone.insert(which, now);
      }
      (true, Wants::Every) => {
        let _ = self.open.remove(&which);
      }
      (false, Wants::One | Wants::Every) => {}
    }

    match ::core::primitive::bool::then_some(which == From::Pad, ()) {
      ::core::option::Option::Some(()) => self.held.pad_went(),
      ::core::option::Option::None => Ok(Vec::new()),
    }
  }

  fn find(&mut self, machine: &mut impl Plugged, now: Seconds) -> Result<Vec<Effect>, Never> {
    let mut back = Vec::new();

    for which in READ {
      let Ok(wants) = which.wants();

      let already = self.open.contains_key(&which);

      let looking = match wants {
        Wants::One => !already,
        Wants::Every => true,
      };

      match ::core::primitive::bool::then_some(looking, ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => continue,
      }

      let looked = match self.hunted.get(&which).copied() {
        Some(looked) => looked,
        None => NEVER_LOOKED,
      };

      match ::core::primitive::bool::then_some(now.0 - looked.0 < HUNT_SECONDS, ()) {
        ::core::option::Option::Some(()) => continue,
        ::core::option::Option::None => {}
      }

      self.hunted.insert(which, now);

      let Ok(found) = self.at(machine, which);

      'over_paths: for path in found {
        match ::core::primitive::bool::then_some(self
          .open
          .get(&which)
          .is_some_and(|paths| paths.contains(&path)), ())
        {
          ::core::option::Option::Some(()) => continue 'over_paths,
          ::core::option::Option::None => {}
        }

        match machine.open(&path) {
          Took::Denied => continue 'over_paths,
          Took::Acquired => {}
        }

        match which {
          From::Pad => {
            let Ok(()) = self.held.reading(machine.ranges(&path));
          }
          From::Keys | From::Touch | From::Typing => {}
        }

        let _ = self.open.entry(which).or_default().insert(path);

        match self
          .gone
          .remove(&which)
          .map(|lost| std::time::Duration::try_from_secs_f64(now.0 - lost.0))
        {
          Some(Ok(gone)) => back.push(Effect::Reconnected(Reconnected {
            device: which,
            gone,
          })),
          Some(Err(_the_clock_went_backwards)) => {}
          None => {}
        }

        match wants {
          Wants::One => break 'over_paths,
          Wants::Every => {}
        }
      }
    }

    Ok(back)
  }

  fn at(&self, machine: &impl Plugged, which: From) -> Result<Vec<String>, Never> {
    match self.told.get(&which) {
      Some(path) => return Ok(vec![path.clone()]),
      None => {}
    }

    let said = machine.every();

    let Ok(every) = match which {
      From::Typing => finding::keyboards(&said),
      From::Pad | From::Keys | From::Touch => {
        let Ok(one) = match which {
          From::Pad => finding::gamepad(&said),
          From::Keys => finding::keyboard(&said),
          From::Touch | From::Typing => finding::touchpad(&said),
        };

        Ok(one.into_iter().collect())
      }
    };

    Ok(every.into_iter().map(|says| says.path.clone()).collect())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Default)]
  struct Machine {
    plugged: BTreeSet<String>,
  }

  impl Plugged for Machine {
    fn every(&self) -> Vec<DeviceInfo> {
      Vec::new()
    }

    fn open(&mut self, path: &str) -> Took {
      match ::core::primitive::bool::then_some(self.plugged.contains(path), ()) {
        ::core::option::Option::Some(()) => Took::Acquired,
        ::core::option::Option::None => Took::Denied,
      }
    }

    fn ranges(&self, _path: &str) -> Ranges {
      Ranges::default()
    }

    fn drain(&mut self, path: &str) -> Result<Vec<InputEvent>, Closed> {
      match ::core::primitive::bool::then_some(self.plugged.contains(path), ()) {
        ::core::option::Option::Some(()) => Ok(Vec::new()),
        ::core::option::Option::None => Err(Closed),
      }
    }
  }

  const PAD: &str = "/dev/input/event1";

  fn sample_paths() -> Result<BTreeMap<From, String>, Never> {
    Ok(
      [
        (From::Pad, PAD),
        (From::Keys, "/dev/input/event2"),
        (From::Touch, "/dev/input/event3"),
      ]
      .into_iter()
      .map(|(which, at)| (which, at.to_string()))
      .collect(),
    )
  }

  fn at(since_boot: Seconds) -> Result<Instant, Never> {
    Ok(Instant {
      since_boot,
      suspended: Seconds(0.0),
    })
  }

  fn turned(
    turning: &mut Turning,
    machine: &mut Machine,
    since_boot: Seconds,
  ) -> Result<Vec<Effect>, Never> {
    let Ok(now) = at(since_boot);

    turning.turn(machine, now)
  }

  #[test]
  fn with_everything_found_and_nothing_held_it_waits_for_a_press() {
    let Ok(paths) = sample_paths();
    let mut machine = Machine {
      plugged: paths.values().cloned().collect(),
    };
    let Ok(mut turning) = Turning::pointed_at(paths);
    let Ok(_) = turned(&mut turning, &mut machine, Seconds(1000.0));

    assert_eq!(turning.wake(), Ok(Wake::OnInput));
  }

  #[test]
  fn a_pad_that_is_missing_is_looked_for_on_a_clock() {
    let Ok(paths) = sample_paths();
    let mut machine = Machine {
      plugged: paths.values().filter(|at| *at != PAD).cloned().collect(),
    };
    let Ok(mut turning) = Turning::pointed_at(paths);
    let Ok(_) = turned(&mut turning, &mut machine, Seconds(1000.0));

    assert_eq!(turning.wake(), Ok(Wake::Within(HUNT_SECONDS)));
  }

  #[test]
  fn something_plugged_in_is_looked_for_at_once_rather_than_after_the_hunt() {
    let Ok(paths) = sample_paths();
    let mut machine = Machine {
      plugged: paths.values().filter(|at| *at != PAD).cloned().collect(),
    };
    let Ok(mut turning) = Turning::pointed_at(paths);
    let Ok(_) = turned(&mut turning, &mut machine, Seconds(1000.0));

    machine.plugged.insert(PAD.to_string());

    let Ok(_) = turned(&mut turning, &mut machine, Seconds(1000.1));

    assert_eq!(
      turning.missing_devices(),
      Ok(vec![From::Pad, From::Typing]),
      "not yet: the hunt was a moment ago"
    );

    let Ok(()) = turning.hunt_now();
    let Ok(_) = turned(&mut turning, &mut machine, Seconds(1000.2));

    assert_eq!(turning.missing_devices(), Ok(vec![From::Typing]));
  }

  fn reconnections(effects: &[Effect]) -> Result<Vec<Reconnected>, Never> {
    Ok(
      effects
        .iter()
        .filter_map(|effect| match effect {
          Effect::Reconnected(back) => Some(*back),
          Effect::Run(_)
          | Effect::Dispatch(_)
          | Effect::Frame(_)
          | Effect::Tell(_)
          | Effect::Using(_) => None,
        })
        .collect(),
    )
  }

  #[test]
  fn a_pad_that_went_and_came_back_says_how_long_it_was_gone() {
    let Ok(paths) = sample_paths();
    let mut machine = Machine {
      plugged: paths.values().cloned().collect(),
    };
    let Ok(mut turning) = Turning::pointed_at(paths);
    let Ok(first) = turned(&mut turning, &mut machine, Seconds(1000.0));

    assert_eq!(
      reconnections(&first),
      Ok(Vec::new()),
      "finding it the first time is not coming back"
    );

    machine.plugged.remove(PAD);

    let Ok(_) = turned(&mut turning, &mut machine, Seconds(1001.0));

    machine.plugged.insert(PAD.to_string());

    let Ok(back) = turned(&mut turning, &mut machine, Seconds(1004.0));

    assert_eq!(
      reconnections(&back),
      Ok(vec![Reconnected {
        device: From::Pad,
        gone: std::time::Duration::from_secs(3)
      }])
    );

    let Ok(after) = turned(&mut turning, &mut machine, Seconds(1006.0));

    assert_eq!(reconnections(&after), Ok(Vec::new()), "it came back once");
  }
}
