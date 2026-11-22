//! The devices a Legion Go publishes, written down.
//!
//! InputPlumber grabs the physical controller and publishes three devices of
//! its own: a pad, a keyboard and a mouse. Those three, plus the controller's
//! touchpad, which InputPlumber never touches, are everything the desktop's
//! daemons read. Nothing reads the physical controller, so nothing here
//! pretends to be one.
//!
//! What they are is not invented. `capture-devices` wrote down the real ones
//! on the machine itself, down to the range of every axis, and this reads that
//! back. The one property that cannot be captured, and matters most, is that a
//! device made through uinput has no physical location: that empty `phys` is
//! the only thing telling the pad InputPlumber published apart from the pad a
//! person is holding, and it is how the daemons tell them apart too.

use std::collections::BTreeMap;

use console_core_never::Never;
use console_core_number_conversion::fixed_index;
use serde::{Deserialize, Serialize};

use crate::GamepadError;
use crate::bounded::{List, Text};

pub const CAPTURED: &str = include_str!("../fixtures/devices.json");

const MAX_CAPTURE_BYTES: u32 = 512 * 1024;
const MAX_DEVICES: u32 = 64;
const MAX_FIELD_BYTES: u32 = 4096;
const MAX_CODES: u32 = 4096;

pub const ROLES: [(&str, &str); 4] = [
  ("Microsoft X-Box One Elite 2 pad", "pad"),
  ("InputPlumber Keyboard", "keyboard"),
  ("InputPlumber Mouse", "mouse"),
  ("  Legion Controller  Touchpad", "touchpad"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Axis {
  pub code: u16,
  pub flat: i32,
  pub fuzz: i32,
  pub maximum: i32,
  pub minimum: i32,
  pub resolution: i32,
}

impl Axis {
  pub fn span(&self) -> Result<i32, Never> {
    Ok(self.maximum.abs().max(self.minimum.abs()).max(1))
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Descriptor {
  pub bustype: u16,
  pub capabilities: Capabilities,
  pub name: String,
  pub phys: String,
  pub product: u16,
  pub properties: Vec<u16>,
  pub uniq: String,
  pub vendor: u16,
  pub version: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Capabilities {
  #[serde(rename = "EV_ABS", default, skip_serializing_if = "Vec::is_empty")]
  pub absolute: Vec<Axis>,
  #[serde(rename = "EV_FF", default, skip_serializing_if = "Vec::is_empty")]
  pub force_feedback: Vec<u16>,
  #[serde(rename = "EV_KEY", default, skip_serializing_if = "Vec::is_empty")]
  pub key: Vec<u16>,
  #[serde(rename = "EV_MSC", default, skip_serializing_if = "Vec::is_empty")]
  pub miscellaneous: Vec<u16>,
  #[serde(rename = "EV_REL", default, skip_serializing_if = "Vec::is_empty")]
  pub relative: Vec<u16>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapturedDescriptor {
  bustype: u16,
  capabilities: Capabilities,
  name: Text<MAX_FIELD_BYTES>,
  phys: Text<MAX_FIELD_BYTES>,
  product: u16,
  properties: List<u16, MAX_CODES>,
  uniq: Text<MAX_FIELD_BYTES>,
  vendor: u16,
  version: u16,
}

impl From<CapturedDescriptor> for Descriptor {
  fn from(captured: CapturedDescriptor) -> Self {
    Self {
      bustype: captured.bustype,
      capabilities: captured.capabilities,
      name: captured.name.0,
      phys: captured.phys.0,
      product: captured.product,
      properties: captured.properties.0,
      uniq: captured.uniq.0,
      vendor: captured.vendor,
      version: captured.version,
    }
  }
}

impl<'de> Deserialize<'de> for Descriptor {
  fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
    CapturedDescriptor::deserialize(decoder).map(Self::from)
  }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapturedCapabilities {
  #[serde(rename = "EV_ABS", default)]
  absolute: List<Axis, MAX_CODES>,
  #[serde(rename = "EV_FF", default)]
  force_feedback: List<u16, MAX_CODES>,
  #[serde(rename = "EV_KEY", default)]
  key: List<u16, MAX_CODES>,
  #[serde(rename = "EV_MSC", default)]
  miscellaneous: List<u16, MAX_CODES>,
  #[serde(rename = "EV_REL", default)]
  relative: List<u16, MAX_CODES>,
}

impl From<CapturedCapabilities> for Capabilities {
  fn from(captured: CapturedCapabilities) -> Self {
    Self {
      absolute: captured.absolute.0,
      force_feedback: captured.force_feedback.0,
      key: captured.key.0,
      miscellaneous: captured.miscellaneous.0,
      relative: captured.relative.0,
    }
  }
}

impl<'de> Deserialize<'de> for Capabilities {
  fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
    CapturedCapabilities::deserialize(decoder).map(Self::from)
  }
}

impl Descriptor {
  pub fn axis(&self, code: u16) -> Result<Option<Axis>, Never> {
    Ok(
      self
        .capabilities
        .absolute
        .iter()
        .copied()
        .find(|axis| axis.code == code),
    )
  }
}

pub fn descriptors(json: &str) -> Result<BTreeMap<String, Descriptor>, GamepadError> {
  let Ok(bytes) = fixed_index(json.len());

  match bytes.cmp(&u64::from(MAX_CAPTURE_BYTES)) {
    std::cmp::Ordering::Greater => return Err(GamepadError::CaptureTooLarge(MAX_CAPTURE_BYTES)),
    std::cmp::Ordering::Equal | std::cmp::Ordering::Less => {}
  }

  let List(captured) =
    serde_json::from_str::<List<Descriptor, MAX_DEVICES>>(json).map_err(GamepadError::Capture)?;
  let mut found = BTreeMap::new();

  for device in captured {
    let Ok(role) = role_of(&device.name);

    match role {
      Some(role_2) => {
        found.insert(role_2.to_string(), device);
      }
      None => {}
    }
  }

  Ok(found)
}

pub fn load_capture() -> Result<BTreeMap<String, Descriptor>, GamepadError> {
  descriptors(CAPTURED)
}

fn role_of(name: &str) -> Result<Option<&'static str>, Never> {
  Ok(
    ROLES
      .iter()
      .find(|(said, _)| *said == name)
      .map(|(_, role)| *role),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_capture_holds_the_four_devices_the_desktop_reads() -> Result<(), GamepadError> {
    let found = load_capture()?;
    let roles: Vec<&str> = found.keys().map(String::as_str).collect();

    assert_eq!(roles, ["keyboard", "mouse", "pad", "touchpad"]);

    Ok(())
  }

  #[test]
  fn a_device_made_through_uinput_has_no_physical_location() -> Result<(), GamepadError> {
    let found = load_capture()?;

    assert_eq!(found.get("pad").map(|pad| pad.phys.as_str()), Some(""));

    Ok(())
  }

  #[test]
  fn the_pad_reports_over_a_range_and_the_keyboard_does_not() -> Result<(), GamepadError> {
    let found = load_capture()?;
    let pad = found.get("pad").ok_or(GamepadError::NotFound("pad"))?;
    let keyboard = found
      .get("keyboard")
      .ok_or(GamepadError::NotFound("keyboard"))?;

    assert!(!pad.capabilities.absolute.is_empty());
    assert!(keyboard.capabilities.absolute.is_empty());
    assert!(!keyboard.capabilities.key.is_empty());

    Ok(())
  }

  #[test]
  fn an_axis_with_no_range_is_still_a_span_of_one() {
    let flat = Axis {
      code: 0,
      flat: 0,
      fuzz: 0,
      maximum: 0,
      minimum: 0,
      resolution: 0,
    };

    assert_eq!(flat.span(), Ok(1));
  }

  #[test]
  fn a_device_nothing_has_a_part_for_is_not_carried() -> Result<(), GamepadError> {
    let said = r#"[{"name":"Some Other Pad","phys":"","uniq":"","vendor":1,
            "product":2,"version":3,"bustype":4,"properties":[],"capabilities":{}}]"#;
    let carried = descriptors(said)?;

    assert!(carried.is_empty());

    Ok(())
  }

  #[test]
  fn captures_reject_unknown_capabilities_and_limit_code_lists() {
    let unknown = serde_json::from_str::<Capabilities>(r#"{"EV_UNKNOWN":[]}"#);

    assert!(matches!(unknown, Err(fault) if fault.to_string().contains("unknown field")));

    let codes = vec![0u16; 4097];
    let json = serde_json::json!({ "EV_KEY": codes });

    let oversized = serde_json::from_value::<Capabilities>(json);

    assert!(matches!(oversized, Err(fault) if fault.to_string().contains("sequence exceeds")));
  }

  #[test]
  fn the_captured_devices_name_no_ones_controller() -> Result<(), GamepadError> {
    let carried = descriptors(CAPTURED)?;

    for (part, device) in carried {
      assert!(
        device.uniq.is_empty(),
        "the {part} was captured with a serial on it: {}",
        device.uniq
      );
    }

    Ok(())
  }
}
