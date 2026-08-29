//! Write down what an input device is, so somewhere else can pretend to be it.
//!
//! Run on the Legion Go. Every device the controller daemon looks for is dumped
//! as JSON, and `console-pad` builds a device from that JSON through uinput. What
//! the daemon then finds is the same shape as the real thing: the same name, the
//! same axes, the same ranges, and no physical location, which is the one
//! difference between a real pad and the one InputPlumber publishes.

use std::process::ExitCode;

use evdev::{AbsoluteAxisCode, Device, EventType, KeyCode, MiscCode, PropType, RelativeAxisCode};
use console_pad::capture::{Axis, Capabilities, Descriptor, ROLES};

/// Everything the kernel says about one device.
fn described(device: &Device) -> Descriptor {
    let id = device.input_id();
    let listed = |kind: EventType| -> Vec<u16> {
        match kind {
            EventType::KEY => {
                device.supported_keys().map(|set| set.iter().map(|KeyCode(code)| code).collect())
            }
            EventType::RELATIVE => device
                .supported_relative_axes()
                .map(|set| set.iter().map(|RelativeAxisCode(code)| code).collect()),
            EventType::MISC => {
                device.misc_properties().map(|set| set.iter().map(|MiscCode(code)| code).collect())
            }
            EventType::FORCEFEEDBACK => device
                .supported_ff()
                .map(|set| set.iter().map(|effect| effect.0).collect()),
            _ => None,
        }
        .map(|mut every: Vec<u16>| {
            every.sort_unstable();
            every
        })
        .unwrap_or_default()
    };
    let mut properties: Vec<u16> =
        device.properties().iter().map(|PropType(what)| what).collect();
    properties.sort_unstable();
    let mut abs: Vec<Axis> = device
        .get_absinfo()
        .map(|every| {
            every
                .map(|(AbsoluteAxisCode(code), info)| Axis {
                    code,
                    flat: info.flat(),
                    fuzz: info.fuzz(),
                    max: info.maximum(),
                    min: info.minimum(),
                    resolution: info.resolution(),
                })
                .collect()
        })
        .unwrap_or_default();
    abs.sort_unstable_by_key(|axis| axis.code);

    Descriptor {
        bustype: id.bus_type().0,
        capabilities: Capabilities {
            abs,
            ff: listed(EventType::FORCEFEEDBACK),
            key: listed(EventType::KEY),
            msc: listed(EventType::MISC),
            rel: listed(EventType::RELATIVE),
        },
        name: device.name().unwrap_or_default().to_string(),
        phys: device.physical_path().unwrap_or_default().to_string(),
        product: id.product(),
        properties,
        // Not recorded. A serial names one machine and nothing here ever reads
        // it: the emulator builds its devices out of the capabilities and the
        // names, and uinput is never told a serial at all. Capturing it would
        // put the number off somebody's controller into the repository for the
        // sake of a field nothing asks about.
        uniq: String::new(),
        vendor: id.vendor(),
        version: id.version(),
    }
}

fn main() -> ExitCode {
    let mut found: Vec<(usize, Descriptor)> = Vec::new();
    for (_, device) in evdev::enumerate() {
        let name = device.name().unwrap_or_default().to_string();
        let Some(at) = ROLES.iter().position(|(wanted, _)| *wanted == name) else { continue };
        if found.iter().any(|(already, _)| *already == at) {
            continue;
        }
        found.push((at, described(&device)));
    }
    found.sort_by_key(|(at, _)| *at);

    let missing: Vec<&str> = ROLES
        .iter()
        .enumerate()
        .filter(|(at, _)| !found.iter().any(|(there, _)| there == at))
        .map(|(_, (name, _))| *name)
        .collect();
    if !missing.is_empty() {
        eprintln!("not present: {}", missing.join(", "));
    }

    let written: Vec<Descriptor> = found.into_iter().map(|(_, device)| device).collect();
    match serde_json::to_string_pretty(&written) {
        Ok(said) => println!("{said}"),
        Err(fault) => {
            eprintln!("the capture would not write: {fault}");
            return ExitCode::from(1);
        }
    }
    ExitCode::from(u8::from(!missing.is_empty()))
}
