//! Bluetooth, heard from bluez's own signals on the system bus.
//!
//! This was `gdbus monitor --dest org.bluez` through `stdbuf`, a line per
//! signal, and a filter in `again` that read gdbus's way of printing a
//! dictionary -- `{'Powered': <false>}` -- for the two words the bar cares
//! about. That is a format nobody here wrote, read in a crate that is not the
//! one that prints it, and it changed under nobody: gdbus prints a variant the
//! way GLib's debug printer does this year.
//!
//! So the source is a connection of its own, the way the compositor's is a
//! socket of its own. It asks the bus for two things -- every signal bluez
//! sends, and bluez's name changing hands, which is a restarted bluetoothd --
//! and reads each as the message it is. A match rule can say who sent a
//! signal and which interface it is about, and cannot say which keys a
//! changed dictionary holds, so the last part of the narrowing is done here,
//! on the message, rather than handed to every listener as a line it would
//! have to parse back into one:
//!
//! - a radio's `Powered` changing, or a device's `Connected`, is worth asking
//!   after;
//! - a radio appearing or going is, because that is a dongle plugged in or
//!   bluetoothd starting;
//! - bluez's name changing hands is, because everything it said before is gone;
//! - and the rest is not. During a search bluez says every device's strength
//!   every second and every stranger arriving and leaving, which is a search
//!   going on rather than Bluetooth changing.
//!
//! What is passed on is a line naming what changed, which nobody reads and
//! anybody can, the way the pool passes every source's words on.
//!
//! **A connection that drops is made again**, by `keep`, and a bus that will
//! not take one is said on the journal once until it does rather than once a
//! round.

use std::sync::mpsc::SyncSender;

use console_bluetooth::objects::OBJECT_MANAGER;
use console_bluetooth::{SERVICE, adapter, device};
use console_bus::connection::{BUS, Bus, ConnectionError};
use console_bus::messages::{Message, Value};
use console_bus::properties;
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_reconnect::{Round, keep};
use console_program_contract::{Change, EventGroup};

use crate::again::Worth;

const ADDED: &str = "InterfacesAdded";

const REMOVED: &str = "InterfacesRemoved";

const OWNER_CHANGED: &str = "NameOwnerChanged";

pub fn rules() -> Result<[String; 2], Never> {
  Ok([
    format!("type='signal',sender='{SERVICE}'"),
    format!(
      "type='signal',sender='{BUS}',interface='{BUS}',member='{OWNER_CHANGED}',arg0='{SERVICE}'"
    ),
  ])
}

fn naming(held: Option<&Value>, wanted: &str) -> Result<Worth, Never> {
  let Ok(names) = match held {
    Some(list) => list.listed(),
    None => Ok(None),
  };

  let named = names.into_iter().flatten().any(|name| {
    let Ok(text) = name.text();

    text == Some(wanted)
  });

  Ok(match ::core::primitive::bool::then_some(named, ()) {
    ::core::option::Option::Some(()) => Worth::Querying,
    ::core::option::Option::None => Worth::Ignoring,
  })
}

fn changed(message: &Message) -> Result<Worth, Never> {
  let Ok(about) = match message.values.first() {
    Some(interface) => interface.text(),
    None => Ok(None),
  };

  let wanted = match about {
    Some(adapter::INTERFACE) => adapter::POWERED,
    Some(device::INTERFACE) => device::CONNECTED,
    Some(_someone_elses) => return Ok(Worth::Ignoring),
    None => return Ok(Worth::Ignoring),
  };

  let Ok(set) = match message.values.get(1) {
    Some(changes) => changes.keyed(wanted),
    None => Ok(None),
  };

  match set {
    Some(_new_value) => Ok(Worth::Querying),
    None => naming(message.values.get(2), wanted),
  }
}

fn appeared(message: &Message) -> Result<Worth, Never> {
  let Ok(held) = match message.values.get(1) {
    Some(interfaces) => interfaces.keyed(adapter::INTERFACE),
    None => Ok(None),
  };

  match held {
    Some(_a_radio) => Ok(Worth::Querying),
    None => naming(message.values.get(1), adapter::INTERFACE),
  }
}

fn owner(message: &Message) -> Result<Worth, Never> {
  let Ok(name) = match message.values.first() {
    Some(name) => name.text(),
    None => Ok(None),
  };

  Ok(match ::core::primitive::bool::then_some(name == Some(SERVICE), ()) {
    ::core::option::Option::Some(()) => Worth::Querying,
    ::core::option::Option::None => Worth::Ignoring,
  })
}

pub fn worth(message: &Message) -> Result<Worth, Never> {
  match (message.interface.as_deref(), message.member.as_deref()) {
    (Some(properties::INTERFACE), Some(properties::CHANGED)) => changed(message),
    (Some(OBJECT_MANAGER), Some(ADDED | REMOVED)) => appeared(message),
    (Some(BUS), Some(OWNER_CHANGED)) => owner(message),
    (Some(_) | None, Some(_) | None) => Ok(Worth::Ignoring),
  }
}

fn line(message: &Message) -> Result<String, Never> {
  let path = match &message.path {
    Some(path) => path.as_str(),
    None => "",
  };
  let interface = match &message.interface {
    Some(interface) => interface.as_str(),
    None => "",
  };
  let member = match &message.member {
    Some(member) => member.as_str(),
    None => "",
  };

  Ok(format!("{path}: {interface}.{member}"))
}

fn listening() -> Result<Bus, ConnectionError> {
  let mut bus = Bus::system()?;
  let Ok(rules) = rules();

  for rule in rules {
    bus.subscribe(&rule)?;
  }

  Ok(bus)
}

fn relayed(bus: &mut Bus, say: &SyncSender<Change>) -> Result<Round, Never> {
  let ended = iterate((bus, say), |(listening, telling)| {
    let message = match listening.receive() {
      Ok(message) => message,
      Err(why) => {
        eprintln!("console-events: bluez stopped being heard: {why}; listening again");

        return Ok(Step::Halt(Round::Another));
      }
    };

    let Ok(asking) = worth(&message);

    match asking {
      Worth::Querying => {
        let Ok(text) = line(&message);

        match telling.send(Change {
          event_group: EventGroup::Bluetooth,
          text,
        }) {
          Ok(()) => Ok(Step::Again((listening, telling))),
          Err(_no_one_is_listening) => Ok(Step::Halt(Round::Finished)),
        }
      }
      Worth::Ignoring => Ok(Step::Again((listening, telling))),
    }
  });

  Ok(match ended {
    Ok(round) => round,
    Err(Endless) => Round::Another,
  })
}

pub fn hold(say: SyncSender<Change>) -> Result<(), Never> {
  let mut said = false;

  let Ok(()) = keep(move || {
    let mut bus = match listening() {
      Ok(bus) => bus,
      Err(why) => {
        match ::core::primitive::bool::then_some(said, ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => {
            eprintln!(
              "console-events: the system bus would not let bluez be heard: {why}; \
                             nothing is being told what Bluetooth is doing until it does"
            );
            said = true;
          }
        }

        return Round::Another;
      }
    };

    said = false;

    let Ok(round) = relayed(&mut bus, &say);

    round
  });

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  use console_bus::messages::{Kind, Truth};
  use console_bus::properties::{InterfaceName, ObjectPath};

  const RADIO: &str = "/org/bluez/hci0";

  const MOUSE: &str = "/org/bluez/hci0/dev_E0_C1_02_00_B0_C3";

  fn entry(name: &str, value: Value) -> Result<Value, Never> {
    Ok(Value::Group(vec![Value::Word(name.to_string()), value]))
  }

  fn signal(
    at: ObjectPath<'_>,
    on: InterfaceName<'_>,
    member: &str,
    values: Vec<Value>,
  ) -> Result<Message, Never> {
    Ok(Message {
      kind: Kind::Signal,
      path: Some(at.0.to_string()),
      interface: Some(on.0.to_string()),
      member: Some(member.to_string()),
      values,
      ..Message::default()
    })
  }

  fn properties_changed(
    at: ObjectPath<'_>,
    interface: InterfaceName<'_>,
    changes: Vec<Value>,
  ) -> Result<Message, Never> {
    signal(
      at,
      InterfaceName(properties::INTERFACE),
      properties::CHANGED,
      vec![
        Value::Word(interface.0.to_string()),
        Value::List(changes),
        Value::List(Vec::new()),
      ],
    )
  }

  fn truth(name: &str, truth: Truth) -> Result<Value, Never> {
    let Ok(variant) = Value::variant("b", Value::Truth(truth));

    entry(name, variant)
  }

  #[test]
  fn the_radio_turning_on_or_off_is_worth_asking_after() {
    let Ok(powered) = truth(adapter::POWERED, Truth::No);
    let Ok(state) = entry("PowerState", Value::Word("off".to_string()));
    let Ok(off) = properties_changed(
      ObjectPath(RADIO),
      InterfaceName(adapter::INTERFACE),
      vec![powered, state],
    );

    assert_eq!(worth(&off), Ok(Worth::Querying));
  }

  #[test]
  fn something_connecting_is_worth_asking_after() {
    let Ok(connected) = truth(device::CONNECTED, Truth::Yes);
    let Ok(arrived) = properties_changed(
      ObjectPath(MOUSE),
      InterfaceName(device::INTERFACE),
      vec![connected],
    );

    assert_eq!(worth(&arrived), Ok(Worth::Querying));
  }

  #[test]
  fn a_property_bluez_forgot_rather_than_changed_is_still_a_change() {
    let Ok(forgotten) = signal(
      ObjectPath(MOUSE),
      InterfaceName(properties::INTERFACE),
      properties::CHANGED,
      vec![
        Value::Word(device::INTERFACE.to_string()),
        Value::List(Vec::new()),
        Value::List(vec![Value::Word(device::CONNECTED.to_string())]),
      ],
    );

    assert_eq!(worth(&forgotten), Ok(Worth::Querying));
  }

  #[test]
  fn a_search_going_on_is_not_bluetooth_changing() {
    let Ok(strength) = Value::variant("n", Value::Signed16(-71));
    let Ok(loudness) = entry(device::RSSI, strength);
    let Ok(heard) = properties_changed(
      ObjectPath(MOUSE),
      InterfaceName(device::INTERFACE),
      vec![loudness],
    );
    let Ok(resolved) = truth("ServicesResolved", Truth::Yes);
    let Ok(services) = properties_changed(
      ObjectPath(MOUSE),
      InterfaceName(device::INTERFACE),
      vec![resolved],
    );
    let Ok(discovering) = truth(adapter::DISCOVERING, Truth::Yes);
    let Ok(searching) = properties_changed(
      ObjectPath(RADIO),
      InterfaceName(adapter::INTERFACE),
      vec![discovering],
    );
    let Ok(connected_elsewhere) = truth(device::CONNECTED, Truth::Yes);
    let Ok(not_bluez) = properties_changed(
      ObjectPath(RADIO),
      InterfaceName("org.bluez.Battery1"),
      vec![connected_elsewhere],
    );

    for message in [heard, services, searching, not_bluez] {
      assert_eq!(worth(&message), Ok(Worth::Ignoring), "{message:?}");
    }
  }

  #[test]
  fn a_radio_appearing_or_going_is_worth_asking_after_and_a_stranger_is_not() {
    let Ok(powered) = truth(adapter::POWERED, Truth::Yes);
    let Ok(radio) = entry(adapter::INTERFACE, Value::List(vec![powered]));
    let Ok(added) = signal(
      ObjectPath("/"),
      InterfaceName(OBJECT_MANAGER),
      ADDED,
      vec![Value::Path(RADIO.to_string()), Value::List(vec![radio])],
    );
    let Ok(removed) = signal(
      ObjectPath("/"),
      InterfaceName(OBJECT_MANAGER),
      REMOVED,
      vec![
        Value::Path(RADIO.to_string()),
        Value::List(vec![
          Value::Word(properties::INTERFACE.to_string()),
          Value::Word(adapter::INTERFACE.to_string()),
        ]),
      ],
    );
    let Ok(address) = entry(
      device::ADDRESS,
      Value::Word("E0:C1:02:00:B0:C3".to_string()),
    );
    let Ok(stranger) = entry(device::INTERFACE, Value::List(vec![address]));
    let Ok(arrived) = signal(
      ObjectPath("/"),
      InterfaceName(OBJECT_MANAGER),
      ADDED,
      vec![Value::Path(MOUSE.to_string()), Value::List(vec![stranger])],
    );
    let Ok(left) = signal(
      ObjectPath("/"),
      InterfaceName(OBJECT_MANAGER),
      REMOVED,
      vec![
        Value::Path(MOUSE.to_string()),
        Value::List(vec![Value::Word(device::INTERFACE.to_string())]),
      ],
    );

    assert_eq!(worth(&added), Ok(Worth::Querying));
    assert_eq!(worth(&removed), Ok(Worth::Querying));
    assert_eq!(worth(&arrived), Ok(Worth::Ignoring));
    assert_eq!(worth(&left), Ok(Worth::Ignoring));
  }

  #[test]
  fn bluez_s_name_changing_hands_is_worth_asking_after() {
    let Ok(restarted) = signal(
      ObjectPath("/org/freedesktop/DBus"),
      InterfaceName(BUS),
      OWNER_CHANGED,
      vec![
        Value::Word(SERVICE.to_string()),
        Value::Word(":1.7".to_string()),
        Value::Word(String::new()),
      ],
    );
    let Ok(someone_else) = signal(
      ObjectPath("/org/freedesktop/DBus"),
      InterfaceName(BUS),
      OWNER_CHANGED,
      vec![
        Value::Word("org.freedesktop.NetworkManager".to_string()),
        Value::Word(String::new()),
        Value::Word(":1.9".to_string()),
      ],
    );

    assert_eq!(worth(&restarted), Ok(Worth::Querying));
    assert_eq!(worth(&someone_else), Ok(Worth::Ignoring));
  }

  #[test]
  fn the_rules_ask_for_bluez_s_signals_and_its_name() {
    let Ok([signals, name]) = rules();

    assert_eq!(signals, "type='signal',sender='org.bluez'");
    assert!(
      name.contains("member='NameOwnerChanged'") && name.contains("arg0='org.bluez'"),
      "{name}"
    );
  }
}
