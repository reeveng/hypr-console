//! What the machine talks to over the short road.
//!
//! Three things are true of a device at once and they are not the same
//! question. Bonded is whether the two machines have exchanged keys, trusted is
//! whether this one will let it back in without being asked again, and
//! connected is whether it is here now. The tab read only the last of those for
//! a long time and offered `connect` on every row, which is a word bluez
//! refuses for anything it has not been introduced to -- so the one device that
//! most needs the tab, a keyboard no one has paired yet, was the one device the
//! tab could not do anything with.
//!
//! A stranger is drawn by what it has said about itself, which for most of what
//! is in the air is nothing at all. bluez names a device that has not told it a
//! name after its own address, so a room arrives as a column of hex, and the
//! thing someone is actually holding is one line in it. Most of that column is
//! phones: Android's Find My Device has every one of them beacon under Google's
//! service number from an address that changes every few minutes, so one phone
//! on a table is a new row every time it moves house, and the cache under the
//! tab kept a file for each of them.
//!
//! The answer used to be the order -- what said a name above what did not -- and
//! it left the hex on the page under the thing being looked for. A stranger
//! that has said nothing is now not on the page at all, which is the default
//! Android and Apple both ship: nothing could be done with the row, because
//! nobody can tell which address is the headphones in their hand, and a device
//! that is waiting to be paired says its name. A device this machine has
//! paired is listed whatever it said, because that row is one somebody made.
//! Among what is left, what is loud goes above what is faint. A mouse in the
//! hand is the loudest thing in the room.
//!
//! What bluez knows is one answer, asked over the system bus and read by
//! `console-bluetooth`, the reading the bar and the session read it by. It used
//! to be three of bluetoothctl's printouts and one more for every device on the
//! list, searched for `Paired: yes` and for the number in brackets after
//! `RSSI:`. The answer is kept as the bytes it arrived in, so the tab drawn
//! before bluez has answered is bluez's last reply read the same way. A bluez
//! that could not be asked is kept as no answer, which reads as a radio that is
//! off -- the way the bar draws it, and the one thing the tab can offer about
//! it, which is the switch.

use console_bluetooth::device::{Connection, Device, Pairing};
use console_bluetooth::objects::{self, Objects};
use console_bus::connection::Bus;
use console_bus::messages;
use console_core_media_access_control_addresses::{Address, Separator};
use console_core_never::Never;

const NONE_OF_IT: i32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
  Alias,
  Itself,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listed {
  Shown,
  Hidden,
}

pub fn asked() -> Result<Vec<u8>, Never> {
  let reached = Bus::system();

  let mut bus = match reached {
    Ok(bus) => bus,
    Err(fault) => {
      console_core_logging::warn!("console-settings: the system bus: {fault}");

      return Ok(Vec::new());
    }
  };

  let Ok(asking) = objects::managed();
  let called = bus.call(&asking);

  let answer = match called {
    Ok(answer) => answer,
    Err(fault) => {
      console_core_logging::warn!("console-settings: asking bluez what it knows: {fault}");

      return Ok(Vec::new());
    }
  };

  match &answer.fault {
    Some(refused) => console_core_logging::warn!(
      "console-settings: bluez would not say what it knows: {refused}"
    ),
    None => {}
  }

  Ok(match answer.bytes(answer.serial) {
    Ok(bytes) => bytes,
    Err(fault) => {
      console_core_logging::warn!("console-settings: keeping what bluez said: {fault}");

      Vec::new()
    }
  })
}

pub fn read(answer: &[u8]) -> Result<Objects, Never> {
  let unknown = Objects {
    adapters: Vec::new(),
    devices: Vec::new(),
  };

  let reply = match messages::read(answer) {
    Ok(reply) => reply,
    Err(_never_answered) => return Ok(unknown),
  };

  match reply.fault {
    Some(_refused) => Ok(unknown),
    None => objects::read(&reply),
  }
}

pub fn shown(objects: &Objects) -> Result<Vec<Device>, Never> {
  Ok(
    objects
      .devices
      .iter()
      .filter(|device| {
        let Ok(listed) = listed(device);

        listed == Listed::Shown
      })
      .cloned()
      .collect(),
  )
}

pub fn listed(device: &Device) -> Result<Listed, Never> {
  let Ok(named) = naming(device);

  Ok(match (named, device.pairing) {
    (Named::Alias, Pairing::Paired)
    | (Named::Alias, Pairing::NotPaired)
    | (Named::Itself, Pairing::Paired) => Listed::Shown,
    (Named::Itself, Pairing::NotPaired) => Listed::Hidden,
  })
}

pub fn naming(device: &Device) -> Result<Named, Never> {
  Ok(match Address::read(&device.alias, Separator::Hyphen) {
    Ok(named) => match ::core::primitive::bool::then_some(named == device.address, ()) {
      ::core::option::Option::Some(()) => Named::Itself,
      ::core::option::Option::None => Named::Alias,
    },
    Err(_a_name_of_its_own) => Named::Alias,
  })
}

pub const FAINTEST: i16 = -100;

pub const LOUDEST: i16 = -50;

pub fn share(heard: i16) -> Result<i32, Never> {
  let over = i32::from(heard).saturating_sub(i32::from(FAINTEST));
  let whole = i32::from(LOUDEST).saturating_sub(i32::from(FAINTEST));
  let share = match over.saturating_mul(100).checked_div(whole) {
    Some(share) => share,
    None => NONE_OF_IT,
  };

  Ok(share.clamp(0, 100))
}

fn place(device: &Device) -> Result<u32, Never> {
  let Ok(named) = naming(device);

  Ok(match device.connection {
    Connection::Connected => 0,
    Connection::NotConnected => match device.pairing {
      Pairing::Paired => 1,
      Pairing::NotPaired => match named {
        Named::Alias => 2,
        Named::Itself => 3,
      },
    },
  })
}

pub fn in_order(mut devices: Vec<Device>) -> Result<Vec<Device>, Never> {
  devices.sort_by_key(|one| {
    let Ok(place) = place(one);
    let heard = match one.signal {
      Some(heard) => heard,
      None => FAINTEST,
    };

    let loud = heard.saturating_neg();

    (place, loud, one.alias.clone())
  });

  Ok(devices)
}

#[cfg(test)]
pub(crate) mod room {
  use console_bluetooth::device::{Category, Connection, Device, Pairing, PairingMethod};
  use console_core_media_access_control_addresses::Address;
  use console_core_never::Never;

  pub type Standing = (Pairing, Connection, Option<i16>);

  pub const STRANGER: Standing = (Pairing::NotPaired, Connection::NotConnected, None);

  pub const FRIEND: Standing = (Pairing::Paired, Connection::NotConnected, None);

  pub const JOINED: Standing = (Pairing::Paired, Connection::Connected, None);

  pub fn device(
    last: u8,
    alias: &str,
    (pairing, connection, signal): Standing,
  ) -> Result<Device, Never> {
    let address = Address([0xE0, 0xC1, 0x02, 0x00, 0xB0, last]);

    Ok(Device {
      path: format!("/org/bluez/hci0/dev_E0_C1_02_00_B0_{last:02X}"),
      adapter: "/org/bluez/hci0".to_string(),
      address,
      alias: alias.to_string(),
      pairing,
      connection,
      signal,
      pairs_with: PairingMethod::Simple,
      category: Category::Other,
    })
  }

  pub fn quiet(last: u8, standing: Standing) -> Result<Device, Never> {
    device(last, &format!("E0-C1-02-00-B0-{last:02X}"), standing)
  }

  pub fn named(said: &[&str], standing: Standing) -> Result<Vec<Device>, Never> {
    Ok(
      said
        .iter()
        .zip(0_u8..)
        .map(|(alias, last)| {
          let Ok(one) = device(last, alias, standing);

          one
        })
        .collect(),
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use console_bluetooth::adapter::{self, Discovery, Power};
  use console_bluetooth::objects::written::{managed, object, said, set};
  use console_bus::messages::Truth;
  use console_bus::properties::{InterfaceName, PropertyName};

  use super::room::{FRIEND, JOINED, STRANGER, device, quiet};

  #[test]
  fn a_device_named_after_its_own_address_has_said_nothing_about_itself() {
    let Ok(silent) = quiet(0xC3, STRANGER);
    let Ok(says) = device(0xC3, "Blue Keys", STRANGER);

    assert_eq!(naming(&silent), Ok(Named::Itself));
    assert_eq!(naming(&says), Ok(Named::Alias));
  }

  #[test]
  fn a_stranger_that_said_nothing_about_itself_is_not_listed() {
    let Ok(beacon) = quiet(0xE6, STRANGER);
    let Ok(headphones) = device(0xC3, "MINISO-MS218", STRANGER);
    let Ok(paired_here) = quiet(0x66, FRIEND);

    let shown: Vec<Result<Listed, Never>> = [beacon, headphones, paired_here]
      .iter()
      .map(listed)
      .collect();

    assert_eq!(
      shown,
      [Ok(Listed::Hidden), Ok(Listed::Shown), Ok(Listed::Shown),],
      "a phone's Find My Device beacon goes, the headphones stay, and a nameless device paired here stays"
    );
  }

  #[test]
  fn the_ends_of_the_bar_are_the_ends_of_what_a_radio_hears() {
    assert_eq!(share(LOUDEST), Ok(100));
    assert_eq!(share(FAINTEST), Ok(0));
    assert_eq!(
      share(-30),
      Ok(100),
      "nearer than the loudest is still the whole bar"
    );
    assert_eq!(share(-120), Ok(0));
    assert_eq!(share(-75), Ok(50));
  }

  #[test]
  fn what_is_here_goes_above_what_is_known_above_what_said_a_name_above_the_rest() {
    let Ok(quiet_one) = quiet(1, (Pairing::NotPaired, Connection::NotConnected, Some(-40)));
    let Ok(faint) = device(
      2,
      "Faint Mouse",
      (Pairing::NotPaired, Connection::NotConnected, Some(-90)),
    );
    let Ok(near) = device(
      3,
      "Near Mouse",
      (Pairing::NotPaired, Connection::NotConnected, Some(-45)),
    );
    let Ok(known) = device(4, "Known Pads", FRIEND);
    let Ok(speaker) = device(5, "Speaker", JOINED);

    let Ok(in_order) = in_order(vec![quiet_one, faint, near, known, speaker]);
    let names: Vec<&str> = in_order
      .iter()
      .map(|device| device.alias.as_str())
      .collect();

    assert_eq!(
      names,
      [
        "Speaker",
        "Known Pads",
        "Near Mouse",
        "Faint Mouse",
        "E0-C1-02-00-B0-01"
      ]
    );
  }

  #[test]
  fn what_was_kept_is_read_the_way_the_answer_was_when_it_arrived()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(powered) = set(PropertyName(adapter::POWERED), Truth::Yes);
    let Ok(radio) = object(
      "/org/bluez/hci0",
      InterfaceName(adapter::INTERFACE),
      vec![powered],
    );
    let Ok(address) = said(
      PropertyName(console_bluetooth::device::ADDRESS),
      "E0:C1:02:00:B0:C3",
    );
    let Ok(alias) = said(
      PropertyName(console_bluetooth::device::ALIAS),
      "MINISO-MS218",
    );
    let Ok(headphones) = object(
      "/org/bluez/hci0/dev_E0_C1_02_00_B0_C3",
      InterfaceName(console_bluetooth::device::INTERFACE),
      vec![address, alias],
    );
    let Ok(answer) = managed(vec![radio, headphones]);
    let kept = answer.bytes(7)?;

    let Ok(live) = objects::read(&answer);
    let Ok(again) = read(&kept);

    assert_eq!(again, live);
    assert_eq!(again.power(), Ok(Power::On));
    assert_eq!(again.discovery(), Ok(Discovery::NotSearching));
    assert_eq!(
      again
        .devices
        .iter()
        .map(|device| device.alias.as_str())
        .collect::<Vec<&str>>(),
      ["MINISO-MS218"]
    );

    Ok(())
  }

  #[test]
  fn nothing_kept_and_a_refusal_both_read_as_a_radio_that_is_off()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(nothing) = read(&[]);

    assert_eq!(nothing.power(), Ok(Power::Off));
    assert!(nothing.devices.is_empty());

    let Ok(asking) = objects::managed();
    let Ok(refusal) = asking.failing(
      console_bus::messages::ErrorName("org.freedesktop.DBus.Error.ServiceUnknown"),
      "The name org.bluez was not provided by any .service files",
    );
    let kept = refusal.bytes(7)?;
    let Ok(refused) = read(&kept);

    assert_eq!(refused.power(), Ok(Power::Off));

    Ok(())
  }
}
