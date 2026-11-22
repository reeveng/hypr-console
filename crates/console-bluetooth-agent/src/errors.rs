//! What is said when something asked of Bluetooth did not happen.
//!
//! A pairing that did not work used to be reported as the last line
//! bluetoothctl printed, which was whatever it happened to end on -- on the
//! device, the last line of its own help, because the command it had been
//! handed began with its own name a second time. What bluez says has a kind,
//! and every kind somebody can do something about is said here as what they
//! can do; the rest carries bluez's own sentence rather than one written over
//! it.
//!
//! Each thing the tab asks has its own way of having gone wrong, because they
//! are different news. "Couldn't connect to the Pad" is a device that is not
//! here; "the Pad paired, and did not connect" is one that is half here, and a
//! press on its row is what finishes it. A device is named by its name and
//! never by its address, which is what the press carries and the one thing on
//! a card that nobody holding a pair of headphones can match to anything.

use std::fmt;

use console_bluetooth::adapter::Power;
use console_core_never::Never;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
  OutOfRange,
  TimedOut,
  RadioOff,
  NoRadio,
  NotAccepted,
  Declined,
  InProgress,
  NeedsPin,
  Message(String),
}

impl fmt::Display for Reason {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Reason::OutOfRange => write!(to, "Make sure it is turned on, in pairing mode and nearby."),
      Reason::TimedOut => write!(to, "It stopped answering before pairing finished."),
      Reason::RadioOff => write!(to, "Bluetooth is turned off."),
      Reason::NoRadio => write!(
        to,
        "This machine has no Bluetooth radio that bluez can see."
      ),
      Reason::NotAccepted => write!(
        to,
        "It did not accept the pairing. If a code was shown, type it exactly."
      ),
      Reason::Declined => write!(to, "It turned the pairing down."),
      Reason::InProgress => write!(to, "It is already pairing."),
      Reason::NeedsPin => write!(
        to,
        "It asks for a PIN. Pair it again and type the one from its box or manual."
      ),
      Reason::Message(said) => write!(to, "{said}"),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
  Unreachable(String),
  Pair { name: String, reason: Reason },
  Trust { name: String, reason: Reason },
  PairedNotConnected { name: String, reason: Reason },
  Connect { name: String, reason: Reason },
  Disconnect { name: String, reason: Reason },
  Forget { name: String, reason: Reason },
  Power { power: Power, reason: Reason },
  Search { reason: Reason },
}

fn turned(power: Power) -> Result<&'static str, Never> {
  Ok(match power {
    Power::On => "on",
    Power::Off => "off",
  })
}

impl fmt::Display for Error {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Error::Unreachable(why) => write!(to, "bluez could not be asked: {why}"),
      Error::Pair { name, reason } => write!(to, "{name} would not pair: {reason}"),
      Error::Trust { name, reason } => {
        write!(to, "{name} paired and would not be trusted: {reason}")
      }
      Error::PairedNotConnected { name, reason } => {
        write!(to, "{name} paired and would not connect: {reason}")
      }
      Error::Connect { name, reason } => write!(to, "{name} would not connect: {reason}"),
      Error::Disconnect { name, reason } => write!(to, "{name} would not disconnect: {reason}"),
      Error::Forget { name, reason } => write!(to, "{name} could not be forgotten: {reason}"),
      Error::Power { power, reason } => {
        let Ok(way) = turned(*power);

        write!(to, "Bluetooth would not turn {way}: {reason}")
      }
      Error::Search { reason } => write!(to, "Bluetooth would not search: {reason}"),
    }
  }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn what_bluez_said_in_its_own_words_is_carried_through_unchanged() {
    let error = Error::PairedNotConnected {
      name: "Pad".to_string(),
      reason: Reason::Message("br-connection-profile-unavailable".to_string()),
    };

    assert_eq!(
      error.to_string(),
      "Pad paired and would not connect: br-connection-profile-unavailable"
    );
  }

  #[test]
  fn the_radio_says_which_way_it_would_not_turn() {
    let error = Error::Power {
      power: Power::On,
      reason: Reason::Message("Blocked through rfkill".to_string()),
    };

    assert_eq!(
      error.to_string(),
      "Bluetooth would not turn on: Blocked through rfkill"
    );
  }
}
