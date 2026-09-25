//! The captured devices, made again through uinput.
//!
//! The other half of `world`. Where that one exists inside a test, these are
//! real input devices for as long as the program runs: the desktop in front of
//! you is reading them, and `press a` clicks whatever the pointer is on.

use std::collections::BTreeMap;
use console_input_event_devices::{
    AbsInfo, AbsoluteAxisCode, BusType, EventType, InputEvent, InputId, KeyCode, MiscCode, PropType,
    RelativeAxisCode, Setup, VirtualDevice,
};

use crate::capture::Descriptor;
use crate::devices::{Has, Sink};
use crate::GamepadError;

struct Made {
    device: VirtualDevice,
    path: Option<String>,
    frame: Vec<InputEvent>,
}

pub struct Uinput {
    made: BTreeMap<String, Made>,
}

impl Uinput {
    pub fn of(descriptors: &BTreeMap<String, Descriptor>) -> Result<Self, GamepadError> {
        descriptors
            .iter()
            .map(|(role, descriptor)| built(descriptor).map(|made| (role.clone(), made)))
            .collect::<Result<BTreeMap<String, Made>, GamepadError>>()
            .map(|made| Uinput { made })
    }
}

fn built(descriptor: &Descriptor) -> Result<Made, GamepadError> {
    let setup = Setup {
        name: descriptor.name.clone(),
        id: InputId {
            bus: BusType(descriptor.bustype),
            vendor: descriptor.vendor,
            product: descriptor.product,
            version: descriptor.version,
        },
        physical_path: match descriptor.phys.is_empty() {
            true => None,
            false => Some(descriptor.phys.clone()),
        },
        keys: descriptor.capabilities.key.iter().map(|code| KeyCode(*code)).collect(),
        relative_axes: descriptor.capabilities.rel.iter().map(|code| RelativeAxisCode(*code)).collect(),
        absolute_axes: descriptor
            .capabilities
            .abs
            .iter()
            .map(|axis| {
                let info = AbsInfo {
                    value: 0,
                    minimum: axis.min,
                    maximum: axis.max,
                    fuzz: axis.fuzz,
                    flat: axis.flat,
                    resolution: axis.resolution,
                };

                (AbsoluteAxisCode(axis.code), info)
            })
            .collect(),
        misc: descriptor.capabilities.msc.iter().map(|code| MiscCode(*code)).collect(),
        properties: descriptor.properties.iter().map(|code| PropType(*code)).collect(),
    };
    let device = VirtualDevice::create(&setup).map_err(GamepadError::Device)?;
    let nodes = device.nodes().map_err(GamepadError::ListNodes)?;
    let path = nodes.first().map(|node| node.display().to_string());

    Ok(Made { device, path, frame: Vec::new() })
}

impl Sink for Uinput {
    fn path(&self, role: &str) -> Option<String> {
        self.made.get(role).and_then(|made| made.path.clone())
    }

    fn has(&self, role: &str) -> Has {
        match self.made.contains_key(role) {
            true => Has::Yes,
            false => Has::No,
        }
    }

    fn write(&mut self, role: &str, kind: EventType, code: u16, value: i32) {
        match self.made.get_mut(role) {
            Some(made) => made.frame.push(InputEvent { kind, code, value }),
            None => {},
        }
    }

    fn syn(&mut self, role: &str) {
        match self.made.get_mut(role) {
            Some(made) => {
                let frame = std::mem::take(&mut made.frame);

                match frame.is_empty() {
                    true => {},
                    false => {
                        let _ = made.device.emit(&frame);
                    }
                }
            }
            None => {},
        }
    }

    fn close(&mut self) {
        self.made.clear();
    }
}
