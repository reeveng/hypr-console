//! Everything bluez knows, in one answer.
//!
//! bluez publishes every object it keeps through the object manager at its
//! root: each radio, each device it has heard or paired, every interface on
//! each and every property on those, answered to one `GetManagedObjects`. So
//! what the radio is doing and what is around it are one question rather than
//! a question per device, and everything here that wants to know reads the
//! same answer through `read` -- the settings tab, the status bar, the session
//! putting the radio back, and a pairing looking for the device it was
//! pressed for.
//!
//! They did not always. The tab ran `bluetoothctl` four ways and once more
//! for every device in the room, and looked for `Powered: yes` in what came
//! back; the bar asked `busctl` for this very answer as JSON and counted the
//! trues in it; the session read `bluetoothctl show` a third way. Three
//! readings of one fact, none of which had ever been compared with the others,
//! and a tab that took a program per stranger to draw.
//!
//! An object that holds neither a radio nor a device -- bluez's own root, a
//! battery, a media player a pair of headphones published -- is not read
//! here, because nothing on this desktop has asked about it.
//!
//! `written` is that answer built by hand, for a test to hand a reading in
//! bluez's place. It is behind a feature of the same name rather than inside
//! this crate's tests because the readers are in other crates, and a reply
//! each of them spelled for itself would be bluez's format written twice.

use console_bus::messages::{Message, Value, Whom};
use console_bus::properties::{InterfaceName, PropertyName};
use console_core_media_access_control_addresses::Address;
use console_core_never::Never;

use crate::SERVICE;
use crate::adapter::{self, Adapter, Discovery, Power};
use crate::device::{self, Device};

pub const OBJECT_MANAGER: &str = "org.freedesktop.DBus.ObjectManager";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Objects {
  pub adapters: Vec<Adapter>,
  pub devices: Vec<Device>,
}

impl Objects {
  pub fn power(&self) -> Result<Power, Never> {
    Ok(
      match ::core::primitive::bool::then_some(self
        .adapters
        .iter()
        .any(|adapter| adapter.power == Power::On), ())
      {
        ::core::option::Option::Some(()) => Power::On,
        ::core::option::Option::None => Power::Off,
      },
    )
  }

  pub fn discovery(&self) -> Result<Discovery, Never> {
    Ok(
      match ::core::primitive::bool::then_some(self
        .adapters
        .iter()
        .any(|adapter| adapter.discovery == Discovery::Searching), ())
      {
        ::core::option::Option::Some(()) => Discovery::Searching,
        ::core::option::Option::None => Discovery::NotSearching,
      },
    )
  }

  pub fn device(&self, address: Address) -> Result<Option<&Device>, Never> {
    Ok(self.devices.iter().find(|device| device.address == address))
  }
}

pub fn managed() -> Result<Message, Never> {
  Message::call(&Whom {
    to: SERVICE,
    at: "/",
    on: OBJECT_MANAGER,
    calling: "GetManagedObjects",
  })
}

pub fn read(reply: &Message) -> Result<Objects, Never> {
  let Ok(listed) = match reply.values.first() {
    Some(answered) => answered.listed(),
    None => Ok(None),
  };

  let objects = match listed {
    Some(every) => every,
    None => {
      return Ok(Objects {
        adapters: Vec::new(),
        devices: Vec::new(),
      });
    }
  };

  let adapters = objects
    .iter()
    .filter_map(|object| {
      let Ok(read) = adapter::of(object);

      read
    })
    .collect();

  let devices = objects
    .iter()
    .filter_map(|object| {
      let Ok(read) = device::of(object);

      read
    })
    .collect();

  Ok(Objects { adapters, devices })
}

pub(crate) fn holding<'a>(
  object: &'a Value,
  interface: InterfaceName<'_>,
) -> Result<Option<(&'a str, &'a Value)>, Never> {
  let Ok(paired) = object.pair();

  let (named, interfaces) = match paired {
    Some(both) => both,
    None => return Ok(None),
  };

  let Ok(path) = named.text();
  let Ok(held) = interfaces.keyed(interface.0);

  Ok(match (path, held) {
    (Some(at), Some(properties)) => Some((at, properties)),
    (Some(_), None) | (None, Some(_)) | (None, None) => None,
  })
}

pub(crate) fn property<'a, T>(
  properties: &'a Value,
  name: PropertyName<'_>,
  reading: fn(&'a Value) -> Result<Option<T>, Never>,
) -> Result<Option<T>, Never> {
  let Ok(found) = properties.keyed(name.0);

  match found {
    Some(value) => reading(value),
    None => Ok(None),
  }
}

#[cfg(any(test, feature = "written"))]
pub mod written {
  use console_bus::messages::{Kind, Message, Truth, Value};
  use console_bus::properties::{InterfaceName, PropertyName};
  use console_core_never::Never;

  pub fn entry(name: &str, value: Value) -> Result<Value, Never> {
    let Ok(word) = Value::word(name);

    Ok(Value::Group(vec![word, value]))
  }

  pub fn said(name: PropertyName<'_>, text: &str) -> Result<Value, Never> {
    let Ok(word) = Value::word(text);
    let Ok(variant) = Value::variant("s", word);

    entry(name.0, variant)
  }

  pub fn set(name: PropertyName<'_>, truth: Truth) -> Result<Value, Never> {
    let Ok(variant) = Value::variant("b", Value::Truth(truth));

    entry(name.0, variant)
  }

  pub fn heard(name: PropertyName<'_>, strength: i16) -> Result<Value, Never> {
    let Ok(variant) = Value::variant("n", Value::Signed16(strength));

    entry(name.0, variant)
  }

  pub fn object(
    path: &str,
    interface: InterfaceName<'_>,
    properties: Vec<Value>,
  ) -> Result<Value, Never> {
    let Ok(held) = entry(interface.0, Value::List(properties));

    Ok(Value::Group(vec![
      Value::Path(path.to_string()),
      Value::List(vec![held]),
    ]))
  }

  pub fn managed(objects: Vec<Value>) -> Result<Message, Never> {
    Ok(Message {
      kind: Kind::Answer,
      shape: "a{oa{sa{sv}}}".to_string(),
      values: vec![Value::List(objects)],
      ..Message::default()
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use console_bus::messages::Truth;

  use crate::adapter::{DISCOVERING, POWERED};
  use crate::device::{ADDRESS, ALIAS, CONNECTED, Connection, PAIRED, Pairing};

  use written::{managed, object, said, set};

  const RADIO: &str = "/org/bluez/hci0";

  const MS218: Address = Address([0xE0, 0xC1, 0x02, 0x00, 0xB0, 0xC3]);

  fn radio(power: Truth, searching: Truth) -> Result<Value, Never> {
    let Ok(powered) = set(PropertyName(POWERED), power);
    let Ok(discovering) = set(PropertyName(DISCOVERING), searching);

    object(
      RADIO,
      InterfaceName(adapter::INTERFACE),
      vec![powered, discovering],
    )
  }

  fn mouse(paired: Truth, connected: Truth) -> Result<Value, Never> {
    let Ok(address) = said(PropertyName(ADDRESS), "E0:C1:02:00:B0:C3");
    let Ok(alias) = said(PropertyName(ALIAS), "MINISO-MS218");
    let Ok(pairing) = set(PropertyName(PAIRED), paired);
    let Ok(connection) = set(PropertyName(CONNECTED), connected);

    object(
      "/org/bluez/hci0/dev_E0_C1_02_00_B0_C3",
      InterfaceName(device::INTERFACE),
      vec![address, alias, pairing, connection],
    )
  }

  #[test]
  fn the_radio_and_what_is_around_it_come_out_of_one_answer() {
    let Ok(on) = radio(Truth::Yes, Truth::No);
    let Ok(paired) = mouse(Truth::Yes, Truth::Yes);
    let Ok(reply) = managed(vec![on, paired]);
    let Ok(read) = super::read(&reply);

    assert_eq!(read.power(), Ok(Power::On));
    assert_eq!(read.discovery(), Ok(Discovery::NotSearching));

    let Ok(found) = read.device(MS218);
    let read_back =
      found.map(|device| (device.alias.as_str(), device.pairing, device.connection));

    assert_eq!(
      read_back,
      Some(("MINISO-MS218", Pairing::Paired, Connection::Connected))
    );
  }

  #[test]
  fn a_machine_with_no_radio_is_a_radio_that_is_off() {
    let Ok(nothing) = managed(Vec::new());
    let Ok(read) = super::read(&nothing);
    let Ok(unanswered) = super::read(&Message::default());

    assert_eq!(read.power(), Ok(Power::Off));
    assert_eq!(
      unanswered.power(),
      Ok(Power::Off),
      "an answer with nothing in it is not a radio that is on"
    );
    assert_eq!(unanswered.devices, Vec::new());
  }

  #[test]
  fn a_radio_that_is_searching_says_so() {
    let Ok(searching) = radio(Truth::Yes, Truth::Yes);
    let Ok(reply) = managed(vec![searching]);
    let Ok(read) = super::read(&reply);

    assert_eq!(read.discovery(), Ok(Discovery::Searching));
  }
}
