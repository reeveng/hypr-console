//! The actions that are one call each: what bluez is sent for them, given
//! what it knows, and what its answer means.
//!
//! Turning Bluetooth on or off is a property set on every radio bluez keeps,
//! because the one switch on the tab is all of them. Connecting, disconnecting
//! and forgetting are one call about one device, which is found first because
//! a call goes to a path and a press carries an address.
//!
//! Two of those are already true of a device bluez does not know: something
//! that is not there is not connected and is not remembered, so disconnecting
//! or forgetting it needs no call at all, and the answer bluez gives when it
//! was gone by the time the call arrived means the same. Connecting is the one
//! that fails, and says the device is out of range, because a device bluez has
//! let go of is one it stopped hearing.

use console_bluetooth::adapter::{self, Power};
use console_bluetooth::device;
use console_bluetooth::errors::Error as Refusal;
use console_bluetooth::objects::Objects;
use console_bus::messages::Message;
use console_bus::properties::ObjectPath;
use console_core_never::Never;

use crate::agent::{self, Went};
use crate::command::Once;
use crate::errors::{Error, Reason};

#[derive(Debug, Clone, PartialEq)]
pub enum Calls {
  Unneeded,
  Needed {
    name: String,
    messages: Vec<Message>,
  },
}

pub fn calls(objects: &Objects, once: &Once) -> Result<Result<Calls, Error>, Never> {
  match once {
    Once::Power(power) => every_radio(objects, *power),
    Once::Connect(address) => {
      let Ok(found) = objects.device(*address);

      Ok(match found {
        Some(device) => {
          let Ok(connecting) = device::connect(ObjectPath(&device.path));

          Ok(Calls::Needed {
            name: device.alias.clone(),
            messages: vec![connecting],
          })
        }
        None => Err(Error::Connect {
          name: address.to_string(),
          reason: Reason::OutOfRange,
        }),
      })
    }
    Once::Disconnect(address) => {
      let Ok(found) = objects.device(*address);

      Ok(Ok(match found {
        Some(device) => {
          let Ok(leaving) = device::disconnect(ObjectPath(&device.path));

          Calls::Needed {
            name: device.alias.clone(),
            messages: vec![leaving],
          }
        }
        None => Calls::Unneeded,
      }))
    }
    Once::Forget(address) => {
      let Ok(found) = objects.device(*address);

      Ok(Ok(match found {
        Some(device) => {
          let Ok(removing) = adapter::remove_device(device);

          Calls::Needed {
            name: device.alias.clone(),
            messages: vec![removing],
          }
        }
        None => Calls::Unneeded,
      }))
    }
  }
}

fn every_radio(objects: &Objects, power: Power) -> Result<Result<Calls, Error>, Never> {
  let messages: Vec<Message> = objects
    .adapters
    .iter()
    .map(|radio| {
      let Ok(setting) = adapter::power(ObjectPath(&radio.path), power);

      setting
    })
    .collect();

  Ok(match ::core::primitive::bool::then_some(messages.is_empty(), ()) {
    ::core::option::Option::Some(()) => Err(Error::Power {
      power,
      reason: Reason::NoRadio,
    }),
    ::core::option::Option::None => Ok(Calls::Needed {
      name: "Bluetooth".to_string(),
      messages,
    }),
  })
}

pub fn answered(
  once: &Once,
  name: &str,
  reply: Result<(), Refusal>,
) -> Result<Result<(), Error>, Never> {
  let Ok(went) = match once {
    Once::Power(_) => plainly(reply),
    Once::Connect(_) => agent::connected(reply),
    Once::Disconnect(_) => agent::disconnected(reply),
    Once::Forget(_) => agent::forgotten(reply),
  };

  let reason = match went {
    Went::Succeeded => return Ok(Ok(())),
    Went::Failed(reason) => reason,
  };

  let named = name.to_string();

  Ok(Err(match once {
    Once::Power(power) => Error::Power {
      power: *power,
      reason,
    },
    Once::Connect(_) => Error::Connect {
      name: named,
      reason,
    },
    Once::Disconnect(_) => Error::Disconnect {
      name: named,
      reason,
    },
    Once::Forget(_) => Error::Forget {
      name: named,
      reason,
    },
  }))
}

fn plainly(reply: Result<(), Refusal>) -> Result<Went, Never> {
  Ok(match reply {
    Ok(()) => Went::Succeeded,
    Err(refusal) => {
      let Ok(reason) = agent::reason(&refusal);

      Went::Failed(reason)
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  use console_bluetooth::adapter::{Adapter, Discovery};
  use console_bluetooth::device::{Category, Connection, Device, Pairing, PairingMethod};
  use console_bluetooth::errors::ErrorKind;
  use console_core_media_access_control_addresses::Address;

  const MS218: Address = Address([0xE0, 0xC1, 0x02, 0x00, 0xB0, 0xC3]);

  const STRANGER: Address = Address([0x00, 0x75, 0xB2, 0xD0, 0x78, 0xE6]);

  fn known() -> Result<Objects, Never> {
    Ok(Objects {
      adapters: vec![Adapter {
        path: "/org/bluez/hci0".to_string(),
        power: Power::On,
        discovery: Discovery::NotSearching,
      }],
      devices: vec![Device {
        path: "/org/bluez/hci0/dev_E0_C1_02_00_B0_C3".to_string(),
        adapter: "/org/bluez/hci0".to_string(),
        address: MS218,
        alias: "MINISO-MS218".to_string(),
        pairing: Pairing::Paired,
        connection: Connection::Connected,
        signal: Some(-60),
        pairs_with: PairingMethod::Simple,
        category: Category::Other,
      }],
    })
  }

  fn members(calls: &Result<Calls, Error>) -> Result<Vec<String>, Never> {
    Ok(match calls {
      Ok(Calls::Needed { messages, .. }) => messages
        .iter()
        .filter_map(|message| message.member.clone())
        .collect(),
      Ok(Calls::Unneeded) => Vec::new(),
      Err(_nothing_to_send) => Vec::new(),
    })
  }

  fn refused(kind: ErrorKind) -> Result<Refusal, Never> {
    Ok(Refusal::Bluez {
      kind,
      message: "from bluez".to_string(),
    })
  }

  #[test]
  fn each_action_on_a_known_device_is_its_one_call() {
    let Ok(objects) = known();
    let Ok(connecting) = calls(&objects, &Once::Connect(MS218));
    let Ok(leaving) = calls(&objects, &Once::Disconnect(MS218));
    let Ok(removing) = calls(&objects, &Once::Forget(MS218));
    let Ok(on) = calls(&objects, &Once::Power(Power::On));

    assert_eq!(members(&connecting), Ok(vec!["Connect".to_string()]));
    assert_eq!(members(&leaving), Ok(vec!["Disconnect".to_string()]));
    assert_eq!(members(&removing), Ok(vec!["RemoveDevice".to_string()]));
    assert_eq!(members(&on), Ok(vec!["Set".to_string()]));
  }

  #[test]
  fn a_device_bluez_does_not_know_is_already_disconnected_and_forgotten_and_cannot_be_connected()
  {
    let Ok(objects) = known();

    assert_eq!(
      calls(&objects, &Once::Disconnect(STRANGER)),
      Ok(Ok(Calls::Unneeded))
    );
    assert_eq!(
      calls(&objects, &Once::Forget(STRANGER)),
      Ok(Ok(Calls::Unneeded))
    );
    assert_eq!(
      calls(&objects, &Once::Connect(STRANGER)),
      Ok(Err(Error::Connect {
        name: "00:75:B2:D0:78:E6".to_string(),
        reason: Reason::OutOfRange
      }))
    );
  }

  #[test]
  fn a_machine_with_no_radio_cannot_turn_bluetooth_on() {
    let nothing = Objects {
      adapters: Vec::new(),
      devices: Vec::new(),
    };

    assert_eq!(
      calls(&nothing, &Once::Power(Power::On)),
      Ok(Err(Error::Power {
        power: Power::On,
        reason: Reason::NoRadio
      }))
    );
  }

  #[test]
  fn bluez_saying_it_was_already_so_is_the_action_having_happened() {
    let Ok(connected) = refused(ErrorKind::AlreadyConnected);
    let Ok(not_connected) = refused(ErrorKind::NotConnected);
    let Ok(unknown) = refused(ErrorKind::DoesNotExist);
    let Ok(failed) = refused(ErrorKind::Failed);

    assert_eq!(
      answered(&Once::Connect(MS218), "MINISO-MS218", Err(connected)),
      Ok(Ok(()))
    );
    assert_eq!(
      answered(&Once::Disconnect(MS218), "MINISO-MS218", Err(not_connected)),
      Ok(Ok(()))
    );
    assert_eq!(
      answered(&Once::Forget(MS218), "MINISO-MS218", Err(unknown)),
      Ok(Ok(()))
    );
    assert_eq!(
      answered(&Once::Power(Power::On), "Bluetooth", Err(failed)),
      Ok(Err(Error::Power {
        power: Power::On,
        reason: Reason::Message("from bluez".to_string())
      }))
    );
  }
}
