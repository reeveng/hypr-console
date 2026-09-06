//! The captured devices, made again through uinput.
//!
//! The other half of `world`. Where that one exists inside a test, these are
//! real input devices for as long as the program runs: the desktop in front of
//! you is reading them, and `press a` clicks whatever the pointer is on.

use std::collections::BTreeMap;
use std::ffi::CString;

use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode,
    MiscCode, PropType, RelativeAxisCode, UinputAbsSetup,
};

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
    pub fn of(descriptors: &BTreeMap<String, Descriptor>) -> Result<Self, String> {
        descriptors
            .iter()
            .map(|(role, descriptor)| built(descriptor).map(|made| (role.clone(), made)))
            .collect::<Result<BTreeMap<String, Made>, String>>()
            .map(|made| Uinput { made })
    }
}

fn built(descriptor: &Descriptor) -> Result<Made, String> {
    let fault = |what: &'static str| move |e: std::io::Error| format!("{what}: {e}");

    let phys = CString::new(descriptor.phys.as_str()).map_err(|_| "a phys with a nul in it")?;
    let id = InputId::new(
        BusType(descriptor.bustype),
        descriptor.vendor,
        descriptor.product,
        descriptor.version,
    );

    let opened = VirtualDevice::builder().map_err(fault("no way in to /dev/uinput"))?;
    let mut builder: VirtualDeviceBuilder = opened
        .name(&descriptor.name)
        .input_id(id)
        .with_phys(&phys)
        .map_err(fault("a physical location"))?;

    match descriptor.capabilities.key.is_empty() {
        true => {},
        false => {
            let keys: AttributeSet<KeyCode> =
                descriptor.capabilities.key.iter().map(|code| KeyCode(*code)).collect();
            let keyed = builder.with_keys(&keys).map_err(fault("the keys"))?;

            builder = keyed;
        }
    }

    match descriptor.capabilities.rel.is_empty() {
        true => {},
        false => {
            let axes: AttributeSet<RelativeAxisCode> =
                descriptor.capabilities.rel.iter().map(|code| RelativeAxisCode(*code)).collect();
            let with_axes =
                builder.with_relative_axes(&axes).map_err(fault("the relative axes"))?;

            builder = with_axes;
        }
    }

    match descriptor.capabilities.msc.is_empty() {
        true => {},
        false => {
            let misc: AttributeSet<MiscCode> =
                descriptor.capabilities.msc.iter().map(|code| MiscCode(*code)).collect();
            let with_misc = builder.with_msc(&misc).map_err(fault("the misc codes"))?;

            builder = with_misc;
        }
    }

    match descriptor.properties.is_empty() {
        true => {},
        false => {
            let props: AttributeSet<PropType> =
                descriptor.properties.iter().map(|code| PropType(*code)).collect();
            let with_props = builder.with_properties(&props).map_err(fault("the properties"))?;

            builder = with_props;
        }
    }

    for axis in &descriptor.capabilities.abs {
        let setup = UinputAbsSetup::new(
            AbsoluteAxisCode(axis.code),
            AbsInfo::new(0, axis.min, axis.max, axis.fuzz, axis.flat, axis.resolution),
        );
        let with_axis = builder.with_absolute_axis(&setup).map_err(fault("an axis"))?;

        builder = with_axis;
    }

    let mut device = builder.build().map_err(fault("the device would not build"))?;
    let mut nodes = device
        .enumerate_dev_nodes_blocking()
        .map_err(fault("the device's nodes would not be listed"))?;
    let first = nodes
        .next()
        .transpose()
        .map_err(fault("the device's node would not be read"))?;
    let path = first.map(|node| node.display().to_string());
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
            Some(made) => made.frame.push(InputEvent::new(kind.0, code, value)),
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
