//! The radio: the object bluez keeps for each Bluetooth controller on the
//! machine, at `/org/bluez/hci0` for the first.
//!
//! Whether it is on is its `Powered` property and whether it is looking for
//! devices is `Discovering`, which are the two things anybody here asks of
//! it. Turning it on is setting the first; a search is `StartDiscovery`, and
//! it lasts as long as the connection that asked for it, because bluez stops
//! a search the moment whoever started it leaves the bus. Forgetting a device
//! is asked of the radio rather than of the device, because what is removed is
//! the radio's memory of it.

use console_bus::messages::{Message, Truth, Value, Whom};
use console_bus::properties::{self, InterfaceName, ObjectPath, PropertyName, Setting};
use console_core_never::Never;

use crate::SERVICE;
use crate::device::Device;
use crate::objects;

pub const INTERFACE: &str = "org.bluez.Adapter1";

pub const POWERED: &str = "Powered";

pub const DISCOVERING: &str = "Discovering";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Power {
  On,
  Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Discovery {
  Searching,
  NotSearching,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adapter {
  pub path: String,
  pub power: Power,
  pub discovery: Discovery,
}

pub fn of(object: &Value) -> Result<Option<Adapter>, Never> {
  let Ok(holding) = objects::holding(object, InterfaceName(INTERFACE));

  let (path, properties) = match holding {
    Some(both) => both,
    None => return Ok(None),
  };

  let Ok(powered) = objects::property(properties, PropertyName(POWERED), Value::truth);
  let Ok(discovering) = objects::property(properties, PropertyName(DISCOVERING), Value::truth);

  Ok(Some(Adapter {
    path: path.to_string(),
    power: match powered {
      Some(Truth::Yes) => Power::On,
      Some(Truth::No) | None => Power::Off,
    },
    discovery: match discovering {
      Some(Truth::Yes) => Discovery::Searching,
      Some(Truth::No) | None => Discovery::NotSearching,
    },
  }))
}

pub fn power(at: ObjectPath<'_>, power: Power) -> Result<Message, Never> {
  let truth = match power {
    Power::On => Truth::Yes,
    Power::Off => Truth::No,
  };
  let Ok(powered) = Value::variant("b", Value::Truth(truth));

  properties::call_set(
    &Setting {
      to: SERVICE,
      at,
      interface: InterfaceName(INTERFACE),
      name: PropertyName(POWERED),
    },
    powered,
  )
}

fn calling(at: ObjectPath<'_>, method: &str) -> Result<Message, Never> {
  Message::call(&Whom {
    to: SERVICE,
    at: at.0,
    on: INTERFACE,
    calling: method,
  })
}

pub fn start_discovery(at: ObjectPath<'_>) -> Result<Message, Never> {
  calling(at, "StartDiscovery")
}

pub fn stop_discovery(at: ObjectPath<'_>) -> Result<Message, Never> {
  calling(at, "StopDiscovery")
}

pub fn remove_device(device: &Device) -> Result<Message, Never> {
  let Ok(removing) = calling(ObjectPath(&device.adapter), "RemoveDevice");

  removing.carrying("o", vec![Value::Path(device.path.clone())])
}

#[cfg(test)]
mod tests {
  use super::*;

  use console_core_media_access_control_addresses::Address;

  use crate::device::{Category, Connection, Pairing, PairingMethod};

  const RADIO: &str = "/org/bluez/hci0";

  #[test]
  fn turning_the_radio_on_is_its_powered_property_set_true() {
    let Ok(on) = power(ObjectPath(RADIO), Power::On);
    let Ok(off) = power(ObjectPath(RADIO), Power::Off);

    assert_eq!(on.path.as_deref(), Some(RADIO));
    assert_eq!(on.member.as_deref(), Some(properties::SET));
    assert_eq!(on.values.get(1), Some(&Value::Word(POWERED.to_string())));
    assert_eq!(
      on.values.get(2),
      Some(&Value::Variant {
        shape: "b".to_string(),
        value: Box::new(Value::Truth(Truth::Yes))
      })
    );
    assert_eq!(
      off.values.get(2),
      Some(&Value::Variant {
        shape: "b".to_string(),
        value: Box::new(Value::Truth(Truth::No))
      })
    );
  }

  #[test]
  fn a_device_is_forgotten_by_the_radio_that_remembers_it() {
    let paired = Device {
      path: "/org/bluez/hci0/dev_E0_C1_02_00_B0_C3".to_string(),
      adapter: RADIO.to_string(),
      address: Address([0xE0, 0xC1, 0x02, 0x00, 0xB0, 0xC3]),
      alias: "MINISO-MS218".to_string(),
      pairing: Pairing::Paired,
      connection: Connection::NotConnected,
      signal: None,
      pairs_with: PairingMethod::Simple,
      category: Category::Other,
    };
    let Ok(removing) = remove_device(&paired);

    assert_eq!(removing.path.as_deref(), Some(RADIO));
    assert_eq!(removing.interface.as_deref(), Some(INTERFACE));
    assert_eq!(removing.member.as_deref(), Some("RemoveDevice"));
    assert_eq!(removing.values, vec![Value::Path(paired.path.clone())]);
  }
}
