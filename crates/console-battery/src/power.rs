//! Interprets the kernel's battery and external-power attributes.

use std::io;
use std::path::Path;

use console_core_never::Never;

use crate::{BATTERY, CHARGING, FULL, MAINS, SUPPLIES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerSource {
  WithoutBattery,
  OnBattery,
  PluggedIn,
}

#[must_use]
pub fn power_source() -> Result<PowerSource, Never> {
  source_at(Path::new(SUPPLIES))
}

enum Battery {
  None,
  Present,
  OnCable,
}

enum ExternalPower {
  Unknown,
  Offline,
  Online,
}

fn source_at(supplies: &Path) -> Result<PowerSource, Never> {
  let entries = match std::fs::read_dir(supplies) {
    Ok(entries) => entries,
    Err(_supply_directory_unavailable) => return Ok(PowerSource::WithoutBattery),
  };
  let mut battery = Battery::None;
  let mut external = ExternalPower::Unknown;

  for supply in entries.flatten() {
    let at = supply.path();
    let Ok(scope) = system_supply(&at);

    match scope {
      SystemSupply::System => {}
      SystemSupply::Peripheral => continue,
    }

    let Ok(reported_type) = reading(&at, Attribute::Type);
    let kind = match reported_type {
      Reading::Value(value) => value,
      Reading::Absent | Reading::Unreadable => continue,
    };

    match kind.as_str() {
      BATTERY => {
        let Ok(next) = observe_battery(&at, battery);
        battery = next;
      }
      MAINS | "USB" | "USB_DCP" | "USB_CDP" | "USB_ACA" | "USB_C" | "USB_PD" | "USB_PD_DRP"
      | "Wireless" => {
        let Ok(next) = observe_external_power(&at, external);
        external = next;
      }
      _ => {}
    }
  }

  Ok(match (battery, external) {
    (Battery::None, _) => PowerSource::WithoutBattery,
    (Battery::Present | Battery::OnCable, ExternalPower::Online)
    | (Battery::OnCable, ExternalPower::Unknown) => PowerSource::PluggedIn,
    (Battery::Present | Battery::OnCable, ExternalPower::Offline)
    | (Battery::Present, ExternalPower::Unknown) => PowerSource::OnBattery,
  })
}

fn observe_battery(at: &Path, previous: Battery) -> Result<Battery, Never> {
  let Ok(presence) = present(at);

  match presence {
    Presence::Absent => return Ok(previous),
    Presence::Present => {}
  }

  let Ok(reading) = reading(at, Attribute::Status);
  let Ok(status) = reading.text();

  Ok(match (status.as_deref(), previous) {
    (Some(CHARGING | FULL | "Not charging"), _) => Battery::OnCable,
    (Some(_) | None, Battery::None) => Battery::Present,
    (Some(_) | None, known) => known,
  })
}

fn observe_external_power(at: &Path, previous: ExternalPower) -> Result<ExternalPower, Never> {
  let Ok(online) = reading(at, Attribute::Online);
  let reported = match online {
    Reading::Value(reported) => reported.parse::<u8>(),
    Reading::Absent | Reading::Unreadable => return Ok(previous),
  };

  Ok(match (reported, previous) {
    (Ok(1..), _) | (Ok(0), ExternalPower::Online) => ExternalPower::Online,
    (Ok(0), ExternalPower::Unknown | ExternalPower::Offline) => ExternalPower::Offline,
    (Err(_not_an_online_value), known) => known,
  })
}

pub(crate) enum SystemSupply {
  System,
  Peripheral,
}

pub(crate) fn system_supply(at: &Path) -> Result<SystemSupply, Never> {
  let Ok(reading) = reading(at, Attribute::Scope);
  let Ok(scope) = reading.text();

  Ok(match scope.as_deref() {
    Some("Device") => SystemSupply::Peripheral,
    Some(_) | None => SystemSupply::System,
  })
}

pub(crate) enum Presence {
  Present,
  Absent,
}

pub(crate) fn present(at: &Path) -> Result<Presence, Never> {
  let Ok(reading) = reading(at, Attribute::Present);
  let Ok(present) = reading.text();

  Ok(match present.as_deref() {
    Some("0") => Presence::Absent,
    Some(_) | None => Presence::Present,
  })
}

pub(crate) enum Attribute {
  Type,
  Scope,
  Present,
  Status,
  Online,
}

pub(crate) enum Reading {
  Value(String),
  Absent,
  Unreadable,
}

impl Reading {
  fn text(self) -> Result<Option<String>, Never> {
    Ok(match self {
      Self::Value(text) => Some(text),
      Self::Absent | Self::Unreadable => None,
    })
  }
}

pub(crate) fn reading(at: &Path, which: Attribute) -> Result<Reading, Never> {
  let path = match which {
    Attribute::Type => at.join("type"),
    Attribute::Scope => at.join("scope"),
    Attribute::Present => at.join("present"),
    Attribute::Status => at.join("status"),
    Attribute::Online => at.join("online"),
  };

  Ok(match std::fs::read_to_string(path) {
    Ok(text) => Reading::Value(text.trim().to_owned()),
    Err(fault) => match bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
      Some(()) => Reading::Absent,
      None => Reading::Unreadable,
    },
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn only_present_system_batteries_enable_power_profiles()
  -> Result<(), Box<dyn std::error::Error>> {
    let root = console_core_temporary_directories::fresh("console-power-source")?;
    let peripheral = root.join("mouse");
    std::fs::create_dir(&peripheral)?;
    console_core_atomic_writes::whole(&peripheral.join("type"), b"Battery\n")?;
    console_core_atomic_writes::whole(&peripheral.join("scope"), b"Device\n")?;

    let battery = root.join("platform-battery");
    std::fs::create_dir(&battery)?;
    console_core_atomic_writes::whole(&battery.join("type"), b"Battery\n")?;
    console_core_atomic_writes::whole(&battery.join("present"), b"0\n")?;
    assert_eq!(source_at(&root), Ok(PowerSource::WithoutBattery));

    console_core_atomic_writes::whole(&battery.join("present"), b"1\n")?;
    console_core_atomic_writes::whole(&battery.join("status"), b"Discharging\n")?;
    assert_eq!(source_at(&root), Ok(PowerSource::OnBattery));

    let dock = root.join("dock");
    std::fs::create_dir(&dock)?;
    console_core_atomic_writes::whole(&dock.join("type"), b"USB_PD\n")?;
    console_core_atomic_writes::whole(&dock.join("online"), b"1\n")?;
    console_core_atomic_writes::whole(&battery.join("status"), b"Not charging\n")?;
    assert_eq!(source_at(&root), Ok(PowerSource::PluggedIn));

    console_core_atomic_writes::whole(&dock.join("online"), b"0\n")?;
    assert_eq!(source_at(&root), Ok(PowerSource::OnBattery));

    console_core_atomic_writes::whole(&dock.join("online"), b"unknown\n")?;
    assert_eq!(source_at(&root), Ok(PowerSource::PluggedIn));

    console_core_atomic_writes::whole(&battery.join("status"), b"Discharging\n")?;
    assert_eq!(source_at(&root), Ok(PowerSource::OnBattery));
    Ok(())
  }
}
