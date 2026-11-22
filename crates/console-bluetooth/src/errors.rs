//! What bluez answers a call with when it did not do what it was asked.
//!
//! An error on the bus is a name and, usually, a sentence. bluez names its own
//! under `org.bluez.Error.`, and the name is the part a program can decide
//! something about: `AlreadyExists` after a pairing means the device was
//! already where the press meant to put it, and `AuthenticationFailed` means
//! the code was typed wrong. The sentence is for a person, and bluez writes it
//! for whoever wrote the call -- `br-connection-page-timeout` -- so it is
//! carried as it came rather than read.
//!
//! A name bluez does not use is the bus's own, almost always: no daemon on the
//! name, a daemon that went away while it was being waited on, an object that
//! is not there. Those are kept by the name they came with rather than folded
//! into one of bluez's, because "bluetoothd is not running" and "the pairing
//! failed" are not the same thing to anybody reading the journal.

use std::fmt;

use console_bus::messages::Message;
use console_core_never::Never;
use console_core_words::Words;

pub const PREFIX: &str = "org.bluez.Error.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum ErrorKind {
  #[words(name = "AlreadyConnected")]
  AlreadyConnected,
  #[words(name = "AlreadyExists")]
  AlreadyExists,
  #[words(name = "AuthenticationCanceled")]
  AuthenticationCanceled,
  #[words(name = "AuthenticationFailed")]
  AuthenticationFailed,
  #[words(name = "AuthenticationRejected")]
  AuthenticationRejected,
  #[words(name = "AuthenticationTimeout")]
  AuthenticationTimeout,
  #[words(name = "Canceled")]
  Canceled,
  #[words(name = "ConnectionAttemptFailed")]
  ConnectionAttemptFailed,
  #[words(name = "DoesNotExist")]
  DoesNotExist,
  #[words(name = "Failed")]
  Failed,
  #[words(name = "InProgress")]
  InProgress,
  #[words(name = "InvalidArguments")]
  InvalidArguments,
  #[words(name = "NotAuthorized")]
  NotAuthorized,
  #[words(name = "NotAvailable")]
  NotAvailable,
  #[words(name = "NotConnected")]
  NotConnected,
  #[words(name = "NotPermitted")]
  NotPermitted,
  #[words(name = "NotReady")]
  NotReady,
  #[words(name = "NotSupported")]
  NotSupported,
  #[words(name = "Rejected")]
  Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
  Bluez { kind: ErrorKind, message: String },
  Bus { name: String, message: String },
}

impl Error {
  pub fn of(reply: &Message) -> Result<Error, Never> {
    let Ok(said) = match reply.values.first() {
      Some(value) => value.text(),
      None => Ok(None),
    };

    let message = match said {
      Some(sentence) => sentence.to_string(),
      None => String::new(),
    };

    let name = match &reply.fault {
      Some(name) => name.clone(),
      None => String::new(),
    };

    let Ok(known) = match name.strip_prefix(PREFIX) {
      Some(bluez) => ErrorKind::from_name(bluez),
      None => Ok(None),
    };

    Ok(match known {
      Some(kind) => Error::Bluez { kind, message },
      None => Error::Bus { name, message },
    })
  }

  pub fn name(&self) -> Result<String, Never> {
    Ok(match self {
      Error::Bluez { kind, .. } => {
        let Ok(name) = kind.name();

        format!("{PREFIX}{name}")
      }
      Error::Bus { name, .. } => name.clone(),
    })
  }
}

impl fmt::Display for Error {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    let message = match self {
      Error::Bluez { message, .. } | Error::Bus { message, .. } => message,
    };

    match ::core::primitive::bool::then_some(message.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(name) = self.name();

        write!(to, "{name}")
      }
      ::core::option::Option::None => write!(to, "{message}"),
    }
  }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
  use super::*;

  use console_bus::messages::{ErrorName, Kind, Value};

  fn answered(name: ErrorName<'_>, said: &str) -> Result<Message, Never> {
    let Ok(word) = Value::word(said);

    Ok(Message {
      kind: Kind::ErrorReply,
      fault: Some(name.0.to_string()),
      shape: "s".to_string(),
      values: vec![word],
      ..Message::default()
    })
  }

  #[test]
  fn a_name_bluez_uses_is_read_as_its_kind_and_the_sentence_is_kept() {
    let Ok(reply) = answered(
      ErrorName("org.bluez.Error.Failed"),
      "br-connection-page-timeout",
    );
    let Ok(error) = Error::of(&reply);

    assert_eq!(
      error,
      Error::Bluez {
        kind: ErrorKind::Failed,
        message: "br-connection-page-timeout".to_string()
      }
    );
    assert_eq!(error.to_string(), "br-connection-page-timeout");
  }

  #[test]
  fn a_name_that_is_not_bluez_s_is_kept_by_the_name_it_came_with() {
    let Ok(reply) = answered(
      ErrorName("org.freedesktop.DBus.Error.ServiceUnknown"),
      "The name is not activatable",
    );
    let Ok(error) = Error::of(&reply);

    assert_eq!(
      error,
      Error::Bus {
        name: "org.freedesktop.DBus.Error.ServiceUnknown".to_string(),
        message: "The name is not activatable".to_string(),
      }
    );
  }

  #[test]
  fn an_error_with_no_sentence_says_its_name() {
    let Ok(reply) = answered(ErrorName("org.bluez.Error.AlreadyExists"), "");
    let Ok(error) = Error::of(&reply);

    assert_eq!(error.to_string(), "org.bluez.Error.AlreadyExists");
  }

  #[test]
  fn every_kind_is_read_back_from_the_name_it_is_written_as() {
    let Ok(names) = ErrorKind::VARIANTS
      .iter()
      .map(|kind| {
        let Ok(name) = kind.name();

        let Ok(read) = ErrorKind::from_name(name);

        Ok::<_, Never>((*kind, read))
      })
      .collect::<Result<Vec<_>, Never>>();

    assert!(
      names.iter().all(|(kind, read)| *read == Some(*kind)),
      "a kind that does not survive its own name: {names:?}"
    );
  }
}
