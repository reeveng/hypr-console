//! What the short-road radio was doing before the machine left for Steam.
//!
//! `/etc/bluetooth/main.conf` says the radio stays off until someone asks,
//! because a radio no one is listening to is a radio spending a battery, and
//! that once it has been asked for it stays on. Game Mode is the hole in that
//! sentence. Steam powers the adapter down a second after it comes up, and
//! coming back put nothing back: a keyboard someone was typing on, gone, with
//! nothing on the screen saying who took it. The journal says it plainly --
//! `btd_adv_monitor_power_down` one second after `steam`, and nothing after it
//! until someone reached for the tab.
//!
//! So what it was is written down on the way out and put back on the way in.
//! One way round only: a radio the person turned off before leaving is one
//! they turned off, and Steam's own power-down is not an answer worth keeping.
//! A radio someone turned on in Game Mode is theirs as well, and comes back
//! on.
//!
//! Under the runtime directory rather than the state directory, and that is
//! the whole of what keeps the policy. What is written there is gone at the
//! next boot, so a machine shut down from Game Mode comes up with the radio
//! off, the way a machine no one has asked yet is supposed to.
//!
//! The word `Powered` was bluez's and was spelled in three crates -- the
//! settings tab, the bar and this one, each reading `bluetoothctl` or `busctl`
//! for it a way of its own -- and this file said the fourth would be when it
//! moved. It moved: whether the radio is on is `console_bluetooth::objects`,
//! the one reading of what bluez knows, asked over the system bus, and
//! putting it back is bluez's own property set on each radio rather than
//! `bluetoothctl power on` run as a step. A program in the middle of a
//! property was a fault that came back as a line of text; what comes back now
//! is the bus that would not be reached or bluez's error by name.

use std::fmt;
use std::path::{Path, PathBuf};

use console_bluetooth::adapter::{self, Power};
use console_bluetooth::errors::Error as Refusal;
use console_bluetooth::objects::{self, Objects};
use console_bus::connection::{Bus, ConnectionError};
use console_bus::properties::ObjectPath;
use console_core_never::Never;
use console_core_words::Words;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Words)]
pub enum Radio {
  #[words(word = "on")]
  On,
  #[words(word = "off")]
  Off,
}

pub const EVERY: [Radio; 2] = [Radio::On, Radio::Off];

pub const NAMED: &str = "radio";

impl Radio {
  pub fn of_word(word: &str) -> Result<Option<Radio>, Never> {
    Ok(EVERY.into_iter().find(|radio| {
      let Ok(said) = radio.word();

      said == word.trim()
    }))
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unkept {
  Unreached(ConnectionError),
  Unasked(Refusal),
  Unpowered { power: Power, why: Refusal },
  Read(String),
  Unwritten(String),
}

impl fmt::Display for Unkept {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Unkept::Unreached(why) => write!(to, "bluez could not be reached on the system bus: {why}"),
      Unkept::Unasked(why) => write!(to, "bluez would not say whether the radio is on: {why}"),
      Unkept::Unpowered {
        power: Power::On,
        why,
      } => write!(to, "bluez would not turn the radio back on: {why}"),
      Unkept::Unpowered {
        power: Power::Off,
        why,
      } => write!(to, "bluez would not turn the radio off: {why}"),
      Unkept::Read(why) => write!(to, "what the radio was doing: {why}"),
      Unkept::Unwritten(why) => write!(to, "writing down what the radio is doing: {why}"),
    }
  }
}

impl std::error::Error for Unkept {}

pub fn shown(objects: &Objects) -> Result<Radio, Never> {
  let Ok(power) = objects.power();

  Ok(match power {
    Power::On => Radio::On,
    Power::Off => Radio::Off,
  })
}

pub fn putting_back(before: Option<Radio>, now: Radio) -> Result<Option<Power>, Never> {
  Ok(match (before, now) {
    (Some(Radio::On), Radio::Off) => Some(Power::On),
    (Some(Radio::On), Radio::On) | (Some(Radio::Off), _) | (None, _) => None,
  })
}

pub fn beside(runtime: &Path) -> Result<PathBuf, Never> {
  Ok(runtime.join(NAMED))
}

fn reached() -> Result<Bus, Unkept> {
  match Bus::system() {
    Ok(bus) => Ok(bus),
    Err(why) => Err(Unkept::Unreached(why)),
  }
}

fn asked(bus: &mut Bus) -> Result<Objects, Unkept> {
  let Ok(asking) = objects::managed();

  let answer = match bus.call(&asking) {
    Ok(answer) => answer,
    Err(why) => return Err(Unkept::Unreached(why)),
  };

  match &answer.fault {
    Some(_refused) => {
      let Ok(refusal) = Refusal::of(&answer);

      Err(Unkept::Unasked(refusal))
    }
    None => {
      let Ok(read) = objects::read(&answer);

      Ok(read)
    }
  }
}

pub fn query_radio() -> Result<Radio, Unkept> {
  let mut bus = reached()?;
  let read = asked(&mut bus)?;
  let Ok(radio) = shown(&read);

  Ok(radio)
}

pub fn power_radio(power: Power) -> Result<(), Unkept> {
  let mut bus = reached()?;
  let read = asked(&mut bus)?;

  for radio in &read.adapters {
    let Ok(powering) = adapter::power(ObjectPath(&radio.path), power);

    let answer = match bus.call(&powering) {
      Ok(answer) => answer,
      Err(why) => return Err(Unkept::Unreached(why)),
    };

    match &answer.fault {
      Some(_refused) => {
        let Ok(why) = Refusal::of(&answer);

        return Err(Unkept::Unpowered { power, why });
      }
      None => {}
    }
  }

  Ok(())
}

pub fn remember(runtime: &Path, radio: Radio) -> Result<(), Unkept> {
  let Ok(at) = beside(runtime);
  let Ok(word) = radio.word();

  match std::fs::create_dir_all(runtime) {
    Ok(()) => {}
    Err(fault) => return Err(Unkept::Unwritten(fault.to_string())),
  }

  match console_core_atomic_writes::whole(&at, word.as_bytes()) {
    Ok(()) => Ok(()),
    Err(fault) => Err(Unkept::Unwritten(fault.to_string())),
  }
}

pub fn load_saved(runtime: &Path) -> Result<Option<Radio>, Unkept> {
  let Ok(at) = beside(runtime);
  let Ok(held) = console_core_atomic_writes::read(&at);

  match held {
    console_core_atomic_writes::Stored::Absent => Ok(None),
    console_core_atomic_writes::Stored::Failed(why) => Err(Unkept::Read(why)),
    console_core_atomic_writes::Stored::Text(said) => {
      let Ok(radio) = Radio::of_word(&said);

      Ok(radio)
    }
  }
}

pub fn forget(runtime: &Path) -> Result<(), Unkept> {
  let Ok(at) = beside(runtime);

  match std::fs::remove_file(&at) {
    Ok(()) => Ok(()),
    Err(fault) => match ::core::primitive::bool::then_some(
      fault.kind() == std::io::ErrorKind::NotFound,
      (),
    ) {
      ::core::option::Option::Some(()) => Ok(()),
      ::core::option::Option::None => Err(Unkept::Read(fault.to_string())),
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use console_bluetooth::adapter::{Adapter, Discovery};

  fn radios(powers: &[Power]) -> Result<Objects, Never> {
    let adapters = (0_u8..)
      .zip(powers)
      .map(|(number, power)| Adapter {
        path: format!("/org/bluez/hci{number}"),
        power: *power,
        discovery: Discovery::NotSearching,
      })
      .collect();

    Ok(Objects {
      adapters,
      devices: Vec::new(),
    })
  }

  #[test]
  fn the_radio_is_on_when_bluez_says_a_radio_is_powered() {
    let Ok(on) = radios(&[Power::On]);
    let Ok(off) = radios(&[Power::Off]);
    let Ok(either) = radios(&[Power::Off, Power::On]);
    let Ok(none) = radios(&[]);

    assert_eq!(shown(&on), Ok(Radio::On));
    assert_eq!(shown(&off), Ok(Radio::Off));
    assert_eq!(
      shown(&either),
      Ok(Radio::On),
      "a second radio switched on is a radio on"
    );
    assert_eq!(
      shown(&none),
      Ok(Radio::Off),
      "a machine with no radio is not a radio that is on"
    );
  }

  #[test]
  fn a_radio_steam_turned_off_comes_back_and_one_someone_turned_off_does_not() {
    assert_eq!(
      putting_back(Some(Radio::On), Radio::Off),
      Ok(Some(Power::On))
    );
    assert_eq!(
      putting_back(Some(Radio::Off), Radio::Off),
      Ok(None),
      "they turned it off"
    );
    assert_eq!(
      putting_back(Some(Radio::Off), Radio::On),
      Ok(None),
      "they turned it on there"
    );
    assert_eq!(
      putting_back(Some(Radio::On), Radio::On),
      Ok(None),
      "nothing took it"
    );
    assert_eq!(
      putting_back(None, Radio::Off),
      Ok(None),
      "a login is not a return"
    );
  }

  #[test]
  fn what_was_written_down_is_what_comes_back() -> Result<(), Box<dyn std::error::Error>> {
    let held = console_core_temporary_directories::fresh("radio")?;

    assert_eq!(load_saved(&held), Ok(None), "nothing has been left here");

    remember(&held, Radio::On)?;

    assert_eq!(load_saved(&held), Ok(Some(Radio::On)));

    forget(&held)?;

    assert_eq!(load_saved(&held), Ok(None));

    std::fs::remove_dir_all(&held)?;
    Ok(())
  }
}
