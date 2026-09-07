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
use serde::{Deserialize, Serialize};

pub const CAPTURED: &str = include_str!("../fixtures/devices.json");

pub const ROLES: [(&str, &str); 4] = [
    ("Microsoft X-Box One Elite 2 pad", "pad"),
    ("InputPlumber Keyboard", "keyboard"),
    ("InputPlumber Mouse", "mouse"),
    ("  Legion Controller  Touchpad", "touchpad"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct Axis {
    pub code: u16,
    pub flat: i32,
    pub fuzz: i32,
    pub max: i32,
    pub min: i32,
    pub resolution: i32,
}

impl Axis {
    pub fn span(&self) -> Result<i32, Never> {
        Ok(self.max.abs().max(self.min.abs()).max(1))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Capabilities {
    #[serde(rename = "EV_ABS", default, skip_serializing_if = "Vec::is_empty")]
    pub abs: Vec<Axis>,
    #[serde(rename = "EV_FF", default, skip_serializing_if = "Vec::is_empty")]
    pub ff: Vec<u16>,
    #[serde(rename = "EV_KEY", default, skip_serializing_if = "Vec::is_empty")]
    pub key: Vec<u16>,
    #[serde(rename = "EV_MSC", default, skip_serializing_if = "Vec::is_empty")]
    pub msc: Vec<u16>,
    #[serde(rename = "EV_REL", default, skip_serializing_if = "Vec::is_empty")]
    pub rel: Vec<u16>,
}

impl Descriptor {
    pub fn axis(&self, code: u16) -> Result<Option<Axis>, Never> {
        Ok(self.capabilities.abs.iter().copied().find(|axis| axis.code == code))
    }
}

pub fn descriptors(json: &str) -> Result<BTreeMap<String, Descriptor>, String> {
    let captured: Vec<Descriptor> =
        serde_json::from_str(json).map_err(|fault| format!("the capture does not parse: {fault}"))?;
    let mut found = BTreeMap::new();

    for device in captured {
        let Ok(role) = role_of(&device.name);

        match role {
            Some(role) => {
                found.insert(role.to_string(), device);
            }
            None => {},
        }
    }

    Ok(found)
}

pub fn captured() -> Result<BTreeMap<String, Descriptor>, String> {
    descriptors(CAPTURED)
}

fn role_of(name: &str) -> Result<Option<&'static str>, Never> {
    Ok(ROLES.iter().find(|(said, _)| *said == name).map(|(_, role)| *role))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_capture_holds_the_four_devices_the_desktop_reads() {
        let found = captured().expect("the capture carried in this program parses");
        let roles: Vec<&str> = found.keys().map(String::as_str).collect();
        assert_eq!(roles, ["keyboard", "mouse", "pad", "touchpad"]);
    }

    #[test]
    fn a_device_made_through_uinput_has_no_physical_location() {
        assert_eq!(captured().expect("the capture carried in this program parses")["pad"].phys, "");
    }

    #[test]
    fn the_pad_reports_over_a_range_and_the_keyboard_does_not() {
        let found = captured().expect("the capture carried in this program parses");
        assert!(!found["pad"].capabilities.abs.is_empty());
        assert!(found["keyboard"].capabilities.abs.is_empty());
        assert!(!found["keyboard"].capabilities.key.is_empty());
    }

    #[test]
    fn an_axis_with_no_range_is_still_a_span_of_one() {
        let flat = Axis { code: 0, flat: 0, fuzz: 0, max: 0, min: 0, resolution: 0 };
        assert_eq!(flat.span(), Ok(1));
    }

    #[test]
    fn a_device_nothing_has_a_part_for_is_not_carried() {
        let said = r#"[{"name":"Some Other Pad","phys":"","uniq":"","vendor":1,
            "product":2,"version":3,"bustype":4,"properties":[],"capabilities":{}}]"#;
        assert!(descriptors(said).expect("it parses").is_empty());
    }

    #[test]
    fn the_captured_devices_name_nobodys_controller() {
        for (part, device) in descriptors(CAPTURED).expect("the capture parses") {
            assert!(
                device.uniq.is_empty(),
                "the {part} was captured with a serial on it: {}",
                device.uniq
            );
        }
    }
}
