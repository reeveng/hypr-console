//! Wi-Fi scan completion signals from NetworkManager.

use std::sync::mpsc::SyncSender;

use console_bus::connection::{Bus, ConnectionError};
use console_bus::messages::{Kind, Message};
use console_bus::properties;
use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_reconnect::{Round, keep};
use console_program_contract::{Change, EventGroup};

use crate::again::Worth;
use crate::sources::Outage;

const SERVICE: &str = "org.freedesktop.NetworkManager";
const WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const LAST_SCAN: &str = "LastScan";
pub(crate) const SCAN_COMPLETE: &str = "scan-complete";

fn listen() -> Result<Bus, ConnectionError> {
  let mut bus = Bus::system()?;
  let interface = properties::INTERFACE;
  let member = properties::CHANGED;
  bus.subscribe(&format!(
    "type='signal',sender='{SERVICE}',interface='{interface}',member='{member}',arg0='{WIRELESS}'"
  ))?;

  Ok(bus)
}

fn scanned(message: &Message) -> Result<Worth, Never> {
  let (interface, updated, invalidated) = match message.values.as_slice() {
    [interface, updated, invalidated] => (interface, updated, invalidated),
    _unexpected_fields => return Ok(Worth::Ignoring),
  };

  let Ok(name) = interface.text();

  match (
    message.kind,
    message.interface.as_deref(),
    message.member.as_deref(),
    name,
  ) {
    (Kind::Signal, Some(properties::INTERFACE), Some(properties::CHANGED), Some(WIRELESS)) => {}
    _ => return Ok(Worth::Ignoring),
  }

  let Ok(changed) = updated.keyed(LAST_SCAN);
  let Ok(invalidated_2) = invalidated.listed();
  let needs_read = invalidated_2.into_iter().flatten().any(|property| {
    let Ok(name_2) = property.text();

    name_2 == Some(LAST_SCAN)
  });

  Ok(
    match ::core::primitive::bool::then_some(changed.is_some() || needs_read, ()) {
      Some(()) => Worth::Querying,
      None => Worth::Ignoring,
    },
  )
}

pub(crate) fn hold(say: SyncSender<Change>) -> Result<(), Never> {
  let mut outage = Outage::Unreported;

  keep(move || {
    let mut bus = match listen() {
      Ok(bus) => bus,
      Err(fault) => {
        let Ok(round) = outage.retry(&EventGroup::Wifi, format_args!("{fault}"));

        return round;
      }
    };

    let round = iterate((&mut bus, &mut outage), |(bus_2, outage_2)| {
      let message = match bus_2.receive() {
        Ok(message) => message,
        Err(fault) => {
          let Ok(round) = outage_2.retry(&EventGroup::Wifi, format_args!("{fault}"));

          return Ok(Step::Halt(round));
        }
      };
      *outage_2 = Outage::Unreported;
      let Ok(worth) = scanned(&message);

      match worth {
        Worth::Ignoring => Ok(Step::Again((bus_2, outage_2))),
        Worth::Querying => match say.send(Change {
          event_group: EventGroup::Wifi,
          text: SCAN_COMPLETE.to_string(),
        }) {
          Ok(()) => Ok(Step::Again((bus_2, outage_2))),
          Err(_listening_stopped) => Ok(Step::Halt(Round::Finished)),
        },
      }
    });

    match round {
      Ok(outcome) => outcome,
      Err(_receiver_never_settled) => Round::Another,
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_bus::messages::Value;

  struct PropertyChange<'a> {
    interface: &'a str,
    property: &'a str,
  }

  fn changed(
    PropertyChange {
      interface,
      property,
    }: PropertyChange<'_>,
  ) -> Result<Message, Never> {
    Ok(Message {
      kind: Kind::Signal,
      interface: Some(properties::INTERFACE.to_string()),
      member: Some(properties::CHANGED.to_string()),
      values: vec![
        Value::Word(interface.to_string()),
        Value::List(vec![Value::Group(vec![
          Value::Word(property.to_string()),
          Value::Signed64(42),
        ])]),
        Value::List(Vec::new()),
      ],
      ..Message::default()
    })
  }

  #[test]
  fn a_scan_notification_requires_the_wireless_interface_and_its_scan_property() {
    for (interface, property, expected) in [
      (WIRELESS, LAST_SCAN, Worth::Querying),
      (WIRELESS, "Strength", Worth::Ignoring),
      (SERVICE, LAST_SCAN, Worth::Ignoring),
    ] {
      let Ok(message) = changed(PropertyChange {
        interface,
        property,
      });
      assert_eq!(scanned(&message), Ok(expected));
    }
  }
}
