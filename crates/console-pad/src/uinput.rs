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

/// One device the kernel is publishing on this machine's behalf.
struct Made {
    device: VirtualDevice,
    path: Option<String>,
    frame: Vec<InputEvent>,
}

/// The devices, as the kernel now has them.
pub struct Uinput {
    made: BTreeMap<String, Made>,
}

impl Uinput {
    /// Every captured device, built and plugged in.
    ///
    /// Force feedback is left out. Nothing here reads it, and a uinput device
    /// that claims it has to answer for effects it was never asked to play.
    pub fn of(descriptors: &BTreeMap<String, Descriptor>) -> Result<Self, String> {
        descriptors
            .iter()
            .map(|(role, descriptor)| built(descriptor).map(|made| (role.clone(), made)))
            .collect::<Result<BTreeMap<String, Made>, String>>()
            .map(|made| Uinput { made })
    }
}

fn built(descriptor: &Descriptor) -> Result<Made, String> {
    fn fault(what: &'static str) -> impl Fn(std::io::Error) -> String {
        move |e| format!("{what}: {e}")
    }

    let phys = CString::new(descriptor.phys.as_str()).map_err(|_| "a phys with a nul in it")?;
    let id = InputId::new(
        BusType(descriptor.bustype),
        descriptor.vendor,
        descriptor.product,
        descriptor.version,
    );

    let mut builder: VirtualDeviceBuilder = VirtualDevice::builder()
        .map_err(fault("no way in to /dev/uinput"))?
        .name(&descriptor.name)
        .input_id(id)
        .with_phys(&phys)
        .map_err(fault("a physical location"))?;

    if !descriptor.capabilities.key.is_empty() {
        let keys: AttributeSet<KeyCode> =
            descriptor.capabilities.key.iter().map(|code| KeyCode(*code)).collect();
        builder = builder.with_keys(&keys).map_err(fault("the keys"))?;
    }

    if !descriptor.capabilities.rel.is_empty() {
        let axes: AttributeSet<RelativeAxisCode> =
            descriptor.capabilities.rel.iter().map(|code| RelativeAxisCode(*code)).collect();
        builder = builder.with_relative_axes(&axes).map_err(fault("the relative axes"))?;
    }

    if !descriptor.capabilities.msc.is_empty() {
        let misc: AttributeSet<MiscCode> =
            descriptor.capabilities.msc.iter().map(|code| MiscCode(*code)).collect();
        builder = builder.with_msc(&misc).map_err(fault("the misc codes"))?;
    }

    if !descriptor.properties.is_empty() {
        let props: AttributeSet<PropType> =
            descriptor.properties.iter().map(|code| PropType(*code)).collect();
        builder = builder.with_properties(&props).map_err(fault("the properties"))?;
    }

    for axis in &descriptor.capabilities.abs {
        let setup = UinputAbsSetup::new(
            AbsoluteAxisCode(axis.code),
            AbsInfo::new(0, axis.min, axis.max, axis.fuzz, axis.flat, axis.resolution),
        );
        builder = builder.with_absolute_axis(&setup).map_err(fault("an axis"))?;
    }

    let mut device = builder.build().map_err(fault("the device would not build"))?;
    // Where the kernel put it, which is how a daemon is pointed at it. A
    // device nothing can be pointed at is not a device this made, so a node
    // list that will not come back is a failure rather than a device with no
    // path: the second reads, to everything downstream, as "it worked".
    let mut nodes = device
        .enumerate_dev_nodes_blocking()
        .map_err(fault("the device's nodes would not be listed"))?;
    let path = nodes
        .next()
        .transpose()
        .map_err(fault("the device's node would not be read"))?
        .map(|node| node.display().to_string());
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

    /// Held until the frame is reported, because a stick is two numbers and a
    /// reader that sees one of them has seen a stick that moved diagonally.
    fn write(&mut self, role: &str, kind: EventType, code: u16, value: i32) {
        if let Some(made) = self.made.get_mut(role) {
            made.frame.push(InputEvent::new(kind.0, code, value));
        }
    }

    fn syn(&mut self, role: &str) {
        if let Some(made) = self.made.get_mut(role) {
            let frame = std::mem::take(&mut made.frame);

            if !frame.is_empty() {
                let _ = made.device.emit(&frame);
            }
        }
    }

    fn close(&mut self) {
        self.made.clear();
    }
}
