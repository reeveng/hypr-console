//! The captured devices, made again through uinput.
//!
//! The other half of `world`. Where that one exists inside a test, these are
//! real input devices for as long as the program runs: the desktop in front of
//! you is reading them, and `press a` clicks whatever the pointer is on.

use console_input_event_devices::{
  AbsInfo, AbsoluteAxisCode, BusType, EventType, InputEvent, InputId, KeyCode, MiscCode,
  PropType, RelativeAxisCode, Setup, VirtualDevice,
};
use std::collections::BTreeMap;

use crate::GamepadError;
use crate::capture::Descriptor;
use crate::devices::{Has, Sink};

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
    physical_path: match ::core::primitive::bool::then_some(descriptor.phys.is_empty(), ()) {
      ::core::option::Option::Some(()) => None,
      ::core::option::Option::None => Some(descriptor.phys.clone()),
    },
    keys: descriptor
      .capabilities
      .key
      .iter()
      .map(|code| KeyCode(*code))
      .collect(),
    relative_axes: descriptor
      .capabilities
      .relative
      .iter()
      .map(|code| RelativeAxisCode(*code))
      .collect(),
    absolute_axes: descriptor
      .capabilities
      .absolute
      .iter()
      .map(|axis| {
        let information = AbsInfo {
          value: 0,
          minimum: axis.minimum,
          maximum: axis.maximum,
          fuzz: axis.fuzz,
          flat: axis.flat,
          resolution: axis.resolution,
        };

        (AbsoluteAxisCode(axis.code), information)
      })
      .collect(),
    miscellaneous: descriptor
      .capabilities
      .miscellaneous
      .iter()
      .map(|code| MiscCode(*code))
      .collect(),
    properties: descriptor
      .properties
      .iter()
      .map(|code| PropType(*code))
      .collect(),
  };
  let device = VirtualDevice::create(&setup).map_err(GamepadError::Device)?;
  let nodes = device.nodes().map_err(GamepadError::ListNodes)?;
  let path = nodes.first().map(|node| node.display().to_string());

  Ok(Made {
    device,
    path,
    frame: Vec::new(),
  })
}

impl Sink for Uinput {
  fn path(&self, role: &str) -> Option<String> {
    self.made.get(role).and_then(|made| made.path.clone())
  }

  fn has(&self, role: &str) -> Has {
    match ::core::primitive::bool::then_some(self.made.contains_key(role), ()) {
      ::core::option::Option::Some(()) => Has::Yes,
      ::core::option::Option::None => Has::No,
    }
  }

  fn write(&mut self, role: &str, kind: EventType, code: u16, value: i32) {
    match self.made.get_mut(role) {
      Some(made) => made.frame.push(InputEvent { kind, code, value }),
      None => {}
    }
  }

  fn syn(&mut self, role: &str) {
    let made = match self.made.get_mut(role) {
      Some(made) => made,
      None => return,
    };
    let frame = std::mem::take(&mut made.frame);

    match bool::then_some(frame.is_empty(), ()) {
      Some(()) => return,
      None => {}
    }

    match made.device.emit(&frame) {
      Ok(()) => {}
      Err(fault) => console_core_logging::warn!("could not emit a frame for {role}: {fault}"),
    }
  }

  fn close(&mut self) {
    self.made.clear();
  }
}
