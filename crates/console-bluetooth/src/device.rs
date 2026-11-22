//! A device bluez knows: where it is on the bus, what it is called, whether
//! it is paired and here, and what a device is asked to do.
//!
//! bluez keeps one object per device it has heard of, at a path under the
//! adapter that heard it -- `/org/bluez/hci0/dev_E0_C1_02_00_B0_C3` -- and
//! publishes them all through the object manager, every interface and every
//! property in one answer, which `objects` reads. So finding a device and
//! learning its name are one question rather than two, and a device that is
//! not in the answer is one bluez has not heard, or has stopped hearing: it
//! forgets a stranger some seconds after a search ends.
//!
//! The device is matched on its `Address` property rather than on the end of
//! its path. The path is bluez's choice of spelling, and the property is the
//! address said as an address.
//!
//! The name is `Alias`, which is what bluez says a device is called: the name
//! somebody gave it here, else the name it gave itself, else its address with
//! hyphens. That last one is what the settings tab learned to hide.
//!
//! How loud it is comes from `RSSI`, which bluez only says of a device it is
//! hearing right now, so a device with no strength is one that is paired and
//! out of earshot rather than one that is faint.
//!
//! Two more things are read for a pairing that has not happened yet.
//! `LegacyPairing` is bluez saying ahead of time that a device can only pair
//! by a PIN, which is how the tab knows to ask for one before the pairing
//! starts rather than after it has failed. `Icon` is the name of the picture
//! bluez chose for the device from its class, and it is the only place bluez
//! says what sort of thing a device is: a keyboard is given a PIN to type, a
//! pair of headphones has one printed in its manual.

use console_bus::messages::{Message, Truth, Value, Whom};
use console_bus::properties::{self, InterfaceName, ObjectPath, PropertyName, Setting};
use console_core_media_access_control_addresses::{Address, Separator};
use console_core_never::Never;

use crate::SERVICE;
use crate::objects;

pub const INTERFACE: &str = "org.bluez.Device1";

pub const ADDRESS: &str = "Address";

pub const ALIAS: &str = "Alias";

pub const PAIRED: &str = "Paired";

pub const CONNECTED: &str = "Connected";

pub const TRUSTED: &str = "Trusted";

pub const RSSI: &str = "RSSI";

pub const LEGACY_PAIRING: &str = "LegacyPairing";

pub const ICON: &str = "Icon";

const KEYBOARD: &str = "input-keyboard";

const AUDIO: &str = "audio-";

const DEVICE: &str = "/dev_";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pairing {
  Paired,
  NotPaired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connection {
  Connected,
  NotConnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingMethod {
  Pin,
  Simple,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
  Keyboard,
  Audio,
  Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
  pub path: String,
  pub adapter: String,
  pub address: Address,
  pub alias: String,
  pub pairing: Pairing,
  pub connection: Connection,
  pub signal: Option<i16>,
  pub pairs_with: PairingMethod,
  pub category: Category,
}

pub fn category(icon: Option<&str>) -> Result<Category, Never> {
  Ok(match icon {
    Some(KEYBOARD) => Category::Keyboard,
    Some(named) => match ::core::primitive::bool::then_some(named.starts_with(AUDIO), ()) {
      ::core::option::Option::Some(()) => Category::Audio,
      ::core::option::Option::None => Category::Other,
    },
    None => Category::Other,
  })
}

pub fn of(object: &Value) -> Result<Option<Device>, Never> {
  let Ok(holding) = objects::holding(object, InterfaceName(INTERFACE));

  let (path, properties) = match holding {
    Some(both) => both,
    None => return Ok(None),
  };

  let adapter = match path.rsplit_once(DEVICE) {
    Some((adapter, _device)) => adapter,
    None => return Ok(None),
  };

  let Ok(written) = objects::property(properties, PropertyName(ADDRESS), Value::text);

  let address = match written.map(str::parse::<Address>) {
    Some(Ok(address)) => address,
    Some(Err(_not_an_address)) => return Ok(None),
    None => return Ok(None),
  };

  let Ok(called) = objects::property(properties, PropertyName(ALIAS), Value::text);
  let Ok(paired) = objects::property(properties, PropertyName(PAIRED), Value::truth);
  let Ok(connected) = objects::property(properties, PropertyName(CONNECTED), Value::truth);
  let Ok(heard) = objects::property(properties, PropertyName(RSSI), Value::counted);
  let Ok(legacy) = objects::property(properties, PropertyName(LEGACY_PAIRING), Value::truth);
  let Ok(icon) = objects::property(properties, PropertyName(ICON), Value::text);
  let Ok(sort) = category(icon);

  Ok(Some(Device {
    path: path.to_string(),
    adapter: adapter.to_string(),
    address,
    alias: match called {
      Some(alias) => alias.to_string(),
      None => address.to_string(),
    },
    pairing: match paired {
      Some(Truth::Yes) => Pairing::Paired,
      Some(Truth::No) | None => Pairing::NotPaired,
    },
    connection: match connected {
      Some(Truth::Yes) => Connection::Connected,
      Some(Truth::No) | None => Connection::NotConnected,
    },
    signal: match heard.map(i16::try_from) {
      Some(Ok(strength)) => Some(strength),
      Some(Err(_not_a_strength)) => None,
      None => None,
    },
    pairs_with: match legacy {
      Some(Truth::Yes) => PairingMethod::Pin,
      Some(Truth::No) | None => PairingMethod::Simple,
    },
    category: sort,
  }))
}

pub fn find(reply: &Message, address: Address) -> Result<Option<Device>, Never> {
  let Ok(read) = objects::read(reply);

  Ok(
    read
      .devices
      .into_iter()
      .find(|device| device.address == address),
  )
}

pub fn address(path: &str) -> Result<Option<Address>, Never> {
  let written = match path.rsplit_once(DEVICE) {
    Some((_adapter, written)) => written,
    None => return Ok(None),
  };

  Ok(match Address::read(written, Separator::Underscore) {
    Ok(address) => Some(address),
    Err(_not_a_device) => None,
  })
}

fn calling(path: ObjectPath<'_>, method: &str) -> Result<Message, Never> {
  Message::call(&Whom {
    to: SERVICE,
    at: path.0,
    on: INTERFACE,
    calling: method,
  })
}

pub fn pair(path: ObjectPath<'_>) -> Result<Message, Never> {
  calling(path, "Pair")
}

pub fn connect(path: ObjectPath<'_>) -> Result<Message, Never> {
  calling(path, "Connect")
}

pub fn disconnect(path: ObjectPath<'_>) -> Result<Message, Never> {
  calling(path, "Disconnect")
}

pub fn trust(path: ObjectPath<'_>) -> Result<Message, Never> {
  let Ok(yes) = Value::variant("b", Value::Truth(Truth::Yes));

  properties::call_set(
    &Setting {
      to: SERVICE,
      at: path,
      interface: InterfaceName(INTERFACE),
      name: PropertyName(TRUSTED),
    },
    yes,
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  use console_bus::properties::PropertyName;

  use crate::objects::written::{heard, managed, object, said, set};

  const MS218: Address = Address([0xE0, 0xC1, 0x02, 0x00, 0xB0, 0xC3]);

  const PATH: &str = "/org/bluez/hci0/dev_E0_C1_02_00_B0_C3";

  fn known(path: &str, properties: Vec<Value>) -> Result<Value, Never> {
    object(path, InterfaceName(INTERFACE), properties)
  }

  #[test]
  fn a_device_is_found_by_its_address_and_named_by_its_alias() {
    let Ok(address) = said(PropertyName(ADDRESS), "E0:C1:02:00:B0:C3");
    let Ok(alias) = said(PropertyName(ALIAS), "MINISO-MS218");
    let Ok(other_address) = said(PropertyName(ADDRESS), "00:75:B2:D0:78:E6");
    let Ok(other_alias) = said(PropertyName(ALIAS), "00-75-B2-D0-78-E6");
    let Ok(mouse) = known(PATH, vec![address, alias]);
    let Ok(stranger) = known(
      "/org/bluez/hci0/dev_00_75_B2_D0_78_E6",
      vec![other_address, other_alias],
    );
    let Ok(reply) = managed(vec![stranger, mouse]);
    let Ok(found) = find(&reply, MS218);

    assert_eq!(
      found.as_ref().map(|device| device.path.as_str()),
      Some(PATH)
    );
    assert_eq!(
      found.as_ref().map(|device| device.alias.as_str()),
      Some("MINISO-MS218")
    );
    assert_eq!(
      found.as_ref().map(|device| device.adapter.as_str()),
      Some("/org/bluez/hci0")
    );
  }

  #[test]
  fn a_device_bluez_has_not_heard_is_not_found() {
    let Ok(other_address) = said(PropertyName(ADDRESS), "00:75:B2:D0:78:E6");
    let Ok(stranger) = known("/org/bluez/hci0/dev_00_75_B2_D0_78_E6", vec![other_address]);
    let Ok(reply) = managed(vec![stranger]);
    let Ok(nothing) = managed(Vec::new());

    assert_eq!(find(&reply, MS218), Ok(None));
    assert_eq!(find(&nothing, MS218), Ok(None));
    assert_eq!(
      find(&Message::default(), MS218),
      Ok(None),
      "an answer with nothing in it finds nothing"
    );
  }

  #[test]
  fn what_a_device_has_not_said_reads_as_the_quiet_answer() {
    let Ok(address) = said(PropertyName(ADDRESS), "E0:C1:02:00:B0:C3");
    let Ok(bare) = known(PATH, vec![address]);
    let Ok(reply) = managed(vec![bare]);
    let Ok(found) = find(&reply, MS218);
    let read = found.map(|device| {
      (
        device.alias,
        device.pairing,
        device.connection,
        device.signal,
      )
    });

    assert_eq!(
      read,
      Some((
        "E0:C1:02:00:B0:C3".to_string(),
        Pairing::NotPaired,
        Connection::NotConnected,
        None
      )),
      "no alias is its address, no word on pairing is not paired, and nothing heard is no strength"
    );
  }

  #[test]
  fn a_paired_device_in_earshot_says_so_and_how_loud_it_is() {
    let Ok(address) = said(PropertyName(ADDRESS), "E0:C1:02:00:B0:C3");
    let Ok(paired) = set(PropertyName(PAIRED), Truth::Yes);
    let Ok(connected) = set(PropertyName(CONNECTED), Truth::Yes);
    let Ok(loud) = heard(PropertyName(RSSI), -48);
    let Ok(mouse) = known(PATH, vec![address, paired, connected, loud]);
    let Ok(reply) = managed(vec![mouse]);
    let Ok(found) = find(&reply, MS218);
    let read = found.map(|device| (device.pairing, device.connection, device.signal));

    assert_eq!(
      read,
      Some((Pairing::Paired, Connection::Connected, Some(-48)))
    );
  }

  #[test]
  fn a_device_that_pairs_only_by_pin_says_so_before_it_is_paired() {
    let Ok(address) = said(PropertyName(ADDRESS), "E0:C1:02:00:B0:C3");
    let Ok(legacy) = set(PropertyName(LEGACY_PAIRING), Truth::Yes);
    let Ok(icon) = said(PropertyName(ICON), "audio-headset");
    let Ok(headset) = known(PATH, vec![address, legacy, icon]);
    let Ok(reply) = managed(vec![headset]);
    let Ok(found) = find(&reply, MS218);
    let read = found.map(|device| (device.pairs_with, device.category));

    assert_eq!(read, Some((PairingMethod::Pin, Category::Audio)));
  }

  #[test]
  fn the_sort_of_device_is_read_off_the_picture_bluez_chose_for_it() {
    assert_eq!(category(Some("input-keyboard")), Ok(Category::Keyboard));
    assert_eq!(category(Some("audio-headphones")), Ok(Category::Audio));
    assert_eq!(category(Some("audio-card")), Ok(Category::Audio));
    assert_eq!(category(Some("input-mouse")), Ok(Category::Other));
    assert_eq!(
      category(None),
      Ok(Category::Other),
      "a device bluez drew nothing for is not a keyboard"
    );
  }

  #[test]
  fn something_that_is_not_under_a_radio_is_not_a_device() {
    let Ok(address) = said(PropertyName(ADDRESS), "E0:C1:02:00:B0:C3");
    let Ok(elsewhere) = known("/org/bluez/somewhere", vec![address]);
    let Ok(reply) = managed(vec![elsewhere]);

    assert_eq!(find(&reply, MS218), Ok(None));
  }

  #[test]
  fn a_device_s_address_is_read_back_out_of_its_path() {
    assert_eq!(address(PATH), Ok(Some(MS218)));
    assert_eq!(address("/org/bluez/hci0"), Ok(None));
    assert_eq!(address("/org/bluez/hci0/dev_E0_C1"), Ok(None));
  }

  #[test]
  fn trusting_is_a_property_set_on_the_device_with_a_true_in_it() {
    let Ok(asked) = trust(ObjectPath(PATH));

    assert_eq!(asked.path.as_deref(), Some(PATH));
    assert_eq!(asked.interface.as_deref(), Some(properties::INTERFACE));
    assert_eq!(asked.member.as_deref(), Some(properties::SET));
    assert_eq!(asked.shape, "ssv");
    assert_eq!(
      asked.values.get(2),
      Some(&Value::Variant {
        shape: "b".to_string(),
        value: Box::new(Value::Truth(Truth::Yes))
      })
    );
  }
}
