//! What bluez is answered while a device is introduced, and what its answers
//! mean.
//!
//! The agent answers for one device. bluez asks whether a device may pair and
//! whether it may reach a service before it is trusted, and during an
//! introduction the honest answer to both is "the one being introduced, and
//! nothing else": a keyboard connects back the moment it has its keys, before
//! the trust that would have let it straight in, and turning that away is what
//! leaves a device paired and gone. Anything else asking at the same moment is
//! a device nobody here pressed.
//!
//! Every kind of error bluez names is given a meaning here, in one `match`.
//! While the kinds were BlueR's they were marked non-exhaustive, and the only
//! reading the compiler took was a list with a fallback, where a kind bluez
//! adds would quietly have meant whatever the fallback said. Now they are this
//! tree's, a kind added to `console-bluetooth` is a line this file has to
//! write, and the kinds that mean nothing a person can act on carry bluez's
//! own sentence.
//!
//! A PIN is the one code this agent chooses rather than shows. A device from
//! before Bluetooth 2.1 asks for one whatever the agent claimed, and what to
//! give depends on what the device is. A keyboard is given six fresh digits,
//! put on a card for somebody to type on it. Headphones are given 0000,
//! which is what nearly every pair that old has printed in its manual and what
//! every phone tries first. Anything else is given the PIN somebody typed on
//! the tab before the pairing began -- the tab asked, because bluez had said
//! the device would want one -- and with none the pairing is turned down and
//! says that it wanted a PIN. The six digits are drawn at the edge and handed
//! in, because a machine that drew them itself could not be pressed twice
//! with the same answer.

use console_bluetooth::agent::PinCode;
use console_bluetooth::device::{Category, Device, PairingMethod};
use console_bluetooth::errors::{Error, ErrorKind};
use console_core_media_access_control_addresses::Address;
use console_core_never::Never;

use crate::errors::Reason;

#[cfg_attr(
  dylint_lib = "explicit104_no_secret_in_source",
  expect(
    explicit104_no_secret_in_source,
    reason = "0000 is the public factory default printed in legacy headset manuals; this fallback holds no device or user credential"
  )
)]
pub const HEADPHONES_PIN: &str = "0000";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
  Allow,
  Reject,
}

pub fn answering(asked: Address, introducing: Address) -> Result<Answer, Never> {
  Ok(
    match ::core::primitive::bool::then_some(asked == introducing, ()) {
      ::core::option::Option::Some(()) => Answer::Allow,
      ::core::option::Option::None => Answer::Reject,
    },
  )
}

pub fn reason(error: &Error) -> Result<Reason, Never> {
  let kind = match error {
    Error::Bluez { kind, .. } => kind,
    Error::Bus { .. } => return Ok(Reason::Message(error.to_string())),
  };

  Ok(match kind {
    ErrorKind::NotAvailable
    | ErrorKind::DoesNotExist
    | ErrorKind::ConnectionAttemptFailed
    | ErrorKind::NotConnected => Reason::OutOfRange,
    ErrorKind::AuthenticationTimeout => Reason::TimedOut,
    ErrorKind::NotReady => Reason::RadioOff,
    ErrorKind::AuthenticationFailed => Reason::NotAccepted,
    ErrorKind::AuthenticationRejected
    | ErrorKind::AuthenticationCanceled
    | ErrorKind::NotAuthorized
    | ErrorKind::NotPermitted
    | ErrorKind::Rejected
    | ErrorKind::Canceled => Reason::Declined,
    ErrorKind::InProgress => Reason::InProgress,
    ErrorKind::AlreadyConnected
    | ErrorKind::AlreadyExists
    | ErrorKind::Failed
    | ErrorKind::InvalidArguments
    | ErrorKind::NotSupported => Reason::Message(error.to_string()),
  })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Went {
  Succeeded,
  Failed(Reason),
}

fn already(went: Result<(), Error>, there: ErrorKind) -> Result<Went, Never> {
  let error = match went {
    Ok(()) => return Ok(Went::Succeeded),
    Err(error) => error,
  };

  let arrived = match &error {
    Error::Bluez { kind, .. } => *kind == there,
    Error::Bus { .. } => false,
  };

  match ::core::primitive::bool::then_some(arrived, ()) {
    ::core::option::Option::Some(()) => Ok(Went::Succeeded),
    ::core::option::Option::None => {
      let Ok(reason) = reason(&error);

      Ok(Went::Failed(reason))
    }
  }
}

pub fn paired(went: Result<(), Error>) -> Result<Went, Never> {
  already(went, ErrorKind::AlreadyExists)
}

pub fn connected(went: Result<(), Error>) -> Result<Went, Never> {
  already(went, ErrorKind::AlreadyConnected)
}

pub fn disconnected(went: Result<(), Error>) -> Result<Went, Never> {
  already(went, ErrorKind::NotConnected)
}

pub fn forgotten(went: Result<(), Error>) -> Result<Went, Never> {
  already(went, ErrorKind::DoesNotExist)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pins {
  pub typed: Option<PinCode>,
  pub chosen: Option<PinCode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinAnswer {
  Give(PinCode),
  Show(PinCode),
  Refuse,
}

pub fn pin_answer(device: &Device, pins: &Pins) -> Result<PinAnswer, Never> {
  let Ok(headphones) = PinCode::read(HEADPHONES_PIN);

  Ok(
    match (&pins.typed, device.category, &pins.chosen, headphones) {
      (Some(typed), _, _, _) => PinAnswer::Give(typed.clone()),
      (None, Category::Keyboard, Some(chosen), _) => PinAnswer::Show(chosen.clone()),
      (None, Category::Audio, _, Some(printed)) => PinAnswer::Give(printed),
      (None, Category::Keyboard, None, _)
      | (None, Category::Audio, _, None)
      | (None, Category::Other, _, _) => PinAnswer::Refuse,
    },
  )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinEntry {
  Typed,
  NotTyped,
}

pub fn pin_entry(device: &Device) -> Result<PinEntry, Never> {
  Ok(match (device.pairs_with, device.category) {
    (PairingMethod::Pin, Category::Other) => PinEntry::Typed,
    (PairingMethod::Pin, Category::Keyboard | Category::Audio) | (PairingMethod::Simple, _) => {
      PinEntry::NotTyped
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  const INTRODUCED: Address = Address([0xE0, 0xC1, 0x02, 0x00, 0xB0, 0xC3]);

  const STRANGER: Address = Address([0x00, 0x75, 0xB2, 0xD0, 0x78, 0xE6]);

  fn said(kind: ErrorKind) -> Result<Error, Never> {
    Ok(Error::Bluez {
      kind,
      message: "from bluez".to_string(),
    })
  }

  fn device(pairs_with: PairingMethod, category: Category) -> Result<Device, Never> {
    Ok(Device {
      path: "/org/bluez/hci0/dev_E0_C1_02_00_B0_C3".to_string(),
      adapter: "/org/bluez/hci0".to_string(),
      address: INTRODUCED,
      alias: "Old Thing".to_string(),
      pairing: console_bluetooth::device::Pairing::NotPaired,
      connection: console_bluetooth::device::Connection::NotConnected,
      signal: None,
      pairs_with,
      category,
    })
  }

  fn pin(written: &str) -> Result<Option<PinCode>, Never> {
    PinCode::read(written)
  }

  #[test]
  fn a_pin_is_chosen_by_what_the_device_is() -> Result<(), Box<dyn std::error::Error>> {
    let Ok(keyboard) = device(PairingMethod::Pin, Category::Keyboard);
    let Ok(headphones) = device(PairingMethod::Pin, Category::Audio);
    let Ok(other) = device(PairingMethod::Pin, Category::Other);
    let Ok(chosen) = pin("482913");
    let Ok(typed) = pin("1234");
    let Ok(manual) = pin(HEADPHONES_PIN);
    let untyped = Pins {
      typed: None,
      chosen: chosen.clone(),
    };
    let given = Pins {
      typed: typed.clone(),
      chosen: chosen.clone(),
    };
    let drawn = chosen.ok_or("482913 is a PIN")?;
    let printed = manual.ok_or("0000 is a PIN")?;
    let entered = typed.ok_or("1234 is a PIN")?;

    assert_eq!(pin_answer(&keyboard, &untyped), Ok(PinAnswer::Show(drawn)));
    assert_eq!(
      pin_answer(&headphones, &untyped),
      Ok(PinAnswer::Give(printed))
    );
    assert_eq!(
      pin_answer(&other, &untyped),
      Ok(PinAnswer::Refuse),
      "nobody typed one and nothing says what it is"
    );
    assert_eq!(
      pin_answer(&other, &given),
      Ok(PinAnswer::Give(entered.clone()))
    );
    assert_eq!(
      pin_answer(&keyboard, &given),
      Ok(PinAnswer::Give(entered)),
      "what somebody typed is what they meant, whatever the device is"
    );
    assert_eq!(
      pin_answer(
        &keyboard,
        &Pins {
          typed: None,
          chosen: None
        }
      ),
      Ok(PinAnswer::Refuse),
      "a keyboard with no digits drawn for it is not given someone else's"
    );

    Ok(())
  }

  #[test]
  fn the_tab_asks_for_a_pin_only_where_nothing_else_can_say_it() {
    let Ok(old) = device(PairingMethod::Pin, Category::Other);
    let Ok(old_keyboard) = device(PairingMethod::Pin, Category::Keyboard);
    let Ok(old_headphones) = device(PairingMethod::Pin, Category::Audio);
    let Ok(new) = device(PairingMethod::Simple, Category::Other);

    assert_eq!(pin_entry(&old), Ok(PinEntry::Typed));
    assert_eq!(
      pin_entry(&old_keyboard),
      Ok(PinEntry::NotTyped),
      "a keyboard is given one to type"
    );
    assert_eq!(
      pin_entry(&old_headphones),
      Ok(PinEntry::NotTyped),
      "headphones are given the printed one"
    );
    assert_eq!(
      pin_entry(&new),
      Ok(PinEntry::NotTyped),
      "a device that pairs the new way asks no PIN"
    );
  }

  #[test]
  fn a_device_already_gone_was_disconnected_and_forgotten() {
    let Ok(not_connected) = said(ErrorKind::NotConnected);
    let Ok(unknown) = said(ErrorKind::DoesNotExist);

    assert_eq!(disconnected(Err(not_connected)), Ok(Went::Succeeded));
    assert_eq!(forgotten(Err(unknown.clone())), Ok(Went::Succeeded));
    assert_eq!(
      disconnected(Err(unknown)),
      Ok(Went::Failed(Reason::OutOfRange))
    );
  }

  #[test]
  fn the_device_being_introduced_is_let_in_and_nothing_else_is() {
    assert_eq!(answering(INTRODUCED, INTRODUCED), Ok(Answer::Allow));
    assert_eq!(answering(STRANGER, INTRODUCED), Ok(Answer::Reject));
  }

  #[test]
  fn a_device_already_where_the_press_meant_to_put_it_has_arrived() {
    let Ok(exists) = said(ErrorKind::AlreadyExists);
    let Ok(connected_already) = said(ErrorKind::AlreadyConnected);

    assert_eq!(paired(Ok(())), Ok(Went::Succeeded));
    assert_eq!(paired(Err(exists.clone())), Ok(Went::Succeeded));
    assert_eq!(
      connected(Err(connected_already.clone())),
      Ok(Went::Succeeded)
    );
    assert_eq!(
      connected(Err(exists.clone())),
      Ok(Went::Failed(Reason::Message("from bluez".to_string())))
    );
    assert_eq!(
      paired(Err(connected_already)),
      Ok(Went::Failed(Reason::Message("from bluez".to_string())))
    );
  }

  #[test]
  fn what_bluez_refused_with_is_said_as_what_somebody_can_do_about_it() {
    let Ok(gone) = said(ErrorKind::ConnectionAttemptFailed);
    let Ok(wrong_code) = said(ErrorKind::AuthenticationFailed);
    let Ok(off) = said(ErrorKind::NotReady);

    assert_eq!(reason(&gone), Ok(Reason::OutOfRange));
    assert_eq!(reason(&wrong_code), Ok(Reason::NotAccepted));
    assert_eq!(reason(&off), Ok(Reason::RadioOff));
  }

  #[test]
  fn a_kind_with_nothing_to_do_about_it_carries_bluez_s_own_sentence() {
    let Ok(failed) = said(ErrorKind::Failed);
    let gone = Error::Bus {
      name: "org.freedesktop.DBus.Error.NoReply".to_string(),
      message: "bluetoothd went away".to_string(),
    };

    assert_eq!(
      reason(&failed),
      Ok(Reason::Message("from bluez".to_string()))
    );
    assert_eq!(
      reason(&gone),
      Ok(Reason::Message("bluetoothd went away".to_string()))
    );
  }
}
