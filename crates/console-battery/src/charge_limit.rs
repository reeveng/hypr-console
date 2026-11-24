//! Charge ceilings exposed by the battery driver in sysfs.

use std::io;
use std::path::{Path, PathBuf};

use console_core_never::Never;

use crate::power::{Attribute, Presence, Reading, SystemSupply, present, reading, system_supply};
use crate::{BATTERY, SUPPLIES};

pub const COMMAND: &str = "charge-limit";
const END: &str = "charge_control_end_threshold";

#[derive(Debug, PartialEq, Eq)]
pub struct Limit {
  pub battery: String,
  pub percent: u8,
}

#[must_use]
pub fn percent(text: &str) -> Result<Option<u8>, Never> {
  let trimmed = text.trim();
  let payload = match trimmed.strip_suffix('%') {
    Some(value) => value.trim_end(),
    None => trimmed,
  };

  Ok(
    match payload.parse::<u8>() {
      ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
      ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None,
    }
    .filter(|value| (1..=100).contains(value)),
  )
}

#[must_use]
pub fn available() -> Result<Vec<Limit>, Never> {
  available_at(Path::new(SUPPLIES))
}

fn available_at(root: &Path) -> Result<Vec<Limit>, Never> {
  let entries = match std::fs::read_dir(root) {
    Ok(entries) => entries,
    Err(_supply_directory_unavailable) => return Ok(Vec::new()),
  };

  let mut limits: Vec<Limit> = entries
    .flatten()
    .filter_map(|supply| {
      let at = supply.path();
      let name = supply.file_name().into_string();
      let battery = match name {
        Ok(battery) => battery,
        Err(_not_a_text_name) => return None,
      };
      let Ok(candidate) = threshold(&at);
      let attribute = candidate?;
      let reported = std::fs::read_to_string(&attribute.0);
      let value = match reported {
        Ok(value) => value,
        Err(_attribute_unavailable) => return None,
      };
      let Ok(parsed) = percent(&value);
      let percent = parsed?;
      Some(Limit { battery, percent })
    })
    .collect();
  limits.sort_by(|left, right| left.battery.cmp(&right.battery));
  Ok(limits)
}

struct Threshold(PathBuf);

fn threshold(battery: &Path) -> Result<Option<Threshold>, Never> {
  let path = battery.join(END);
  let supported =
    std::fs::metadata(&path).is_ok_and(|file| file.is_file() && !file.permissions().readonly());

  match bool::then_some(supported, ()) {
    Some(()) => {}
    None => return Ok(None),
  }

  let Ok(scope) = system_supply(battery);

  match scope {
    SystemSupply::System => {}
    SystemSupply::Peripheral => return Ok(None),
  }

  let Ok(presence) = present(battery);

  match presence {
    Presence::Present => {}
    Presence::Absent => return Ok(None),
  }

  let Ok(kind) = reading(battery, Attribute::Type);

  Ok(match kind {
    Reading::Value(label) => bool::then_some(label == BATTERY, Threshold(path)),
    Reading::Absent | Reading::Unreadable => None,
  })
}

pub fn set(battery: &str, value: u8) -> io::Result<()> {
  set_at(Path::new(SUPPLIES), battery, value)
}

fn set_at(root: &Path, battery: &str, value: u8) -> io::Result<()> {
  match { ::core::primitive::bool::then_some(!(1..=100).contains(&value), ()) } {
    ::core::option::Option::Some(()) => {
      return Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "Charge limit must be from 1 to 100 percent",
      ));
    }
    ::core::option::Option::None => {}
  }

  let supplies = std::fs::read_dir(root)?;
  let attribute = supplies
    .flatten()
    .find(|supply| supply.file_name() == battery)
    .and_then(|supply| {
      let Ok(candidate) = threshold(&supply.path());
      candidate
    })
    .ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::NotFound,
        "This battery has no adjustable charge limit",
      )
    })?;

  write_threshold(attribute, value)
}

#[cfg_attr(
  dylint_lib = "explicit040_no_torn_write",
  expect(
    explicit040_no_torn_write,
    reason = "the sysfs charge-threshold inode delivers writes to the driver; renaming a file over it cannot set the attribute"
  )
)]
fn write_threshold(Threshold(path): Threshold, value: u8) -> io::Result<()> {
  std::fs::write(path, format!("{value}\n"))
}

#[cfg(test)]
mod tests {
  use std::os::unix::fs::MetadataExt;

  use super::*;

  #[test]
  fn reads_and_sets_only_a_supported_system_battery() -> Result<(), Box<dyn std::error::Error>> {
    let root = console_core_temporary_directories::fresh("console-charge-limit")?;
    let battery = root.join("BAT1");
    std::fs::create_dir(&battery)?;
    console_core_atomic_writes::whole(&battery.join("type"), b"Battery\n")?;
    let Ok(limits) = available_at(&root);
    assert!(limits.is_empty());

    let attribute = battery.join(END);
    console_core_atomic_writes::whole(&attribute, b"100\n")?;
    let before = std::fs::metadata(&attribute)?;
    set_at(&root, "BAT1", 83)?;
    let after = std::fs::metadata(&attribute)?;
    assert_eq!(before.ino(), after.ino());
    assert_eq!(
      available_at(&root),
      Ok(vec![Limit {
        battery: "BAT1".to_owned(),
        percent: 83
      }])
    );
    assert!(
      matches!(set_at(&root, "../BAT1", 90), Err(fault) if fault.kind() == io::ErrorKind::NotFound)
    );
    assert!(
      matches!(set_at(&root, "BAT1", 0), Err(fault) if fault.kind() == io::ErrorKind::InvalidInput)
    );
    let to_string = std::fs::read_to_string(battery.join(END))?;
    assert_eq!(to_string, "83\n");

    console_core_atomic_writes::whole(&battery.join("scope"), b"Device\n")?;
    let Ok(peripheral_limits) = available_at(&root);
    assert!(peripheral_limits.is_empty());
    assert!(
      matches!(set_at(&root, "BAT1", 90), Err(fault) if fault.kind() == io::ErrorKind::NotFound)
    );
    Ok(())
  }
}
