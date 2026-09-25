//! Write down what an input device is, so somewhere else can pretend to be it.
//!
//! Run on the Legion Go. Every device the controller daemon looks for is dumped
//! as JSON, and `console-input-gamepad` builds a device from that JSON through
//! uinput. What the daemon then finds is the same shape as the real thing: the
//! same name, the same axes, the same ranges, and no physical location, which
//! is the one difference between a real pad and the one InputPlumber
//! publishes.

use std::collections::BTreeMap;
use std::process::ExitCode;

use console_core_never::Never;
use console_input_event_devices::{AbsoluteAxisCode, Device};
use console_input_gamepad::capture::{Axis, Capabilities, Descriptor, ROLES};
use console_input_gamepad::finding;

#[derive(Debug)]
struct Unread(std::io::Error);

impl std::fmt::Display for Unread {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(to, "its axes would not be read: {}", self.0)
    }
}

impl std::error::Error for Unread {}

fn sorted(mut every: Vec<u16>) -> Result<Vec<u16>, Never> {
    every.sort_unstable();

    Ok(every)
}

fn described(device: &Device) -> Result<Descriptor, Unread> {
    let id = device.id;
    let axes = device.absolute().map_err(Unread)?;
    let mut abs: Vec<Axis> = axes
        .into_iter()
        .map(|(AbsoluteAxisCode(code), info)| Axis {
            code,
            flat: info.flat,
            fuzz: info.fuzz,
            max: info.maximum,
            min: info.minimum,
            resolution: info.resolution,
        })
        .collect();
    abs.sort_unstable_by_key(|axis| axis.code);

    let Ok(ff) = sorted(device.force_feedback.iter().map(|effect| effect.0).collect());
    let Ok(key) = sorted(device.keys.iter().map(|key| key.0).collect());
    let Ok(msc) = sorted(device.misc.iter().map(|misc| misc.0).collect());
    let Ok(rel) = sorted(device.relative_axes.iter().map(|axis| axis.0).collect());
    let Ok(properties) = sorted(device.properties.iter().map(|property| property.0).collect());

    Ok(Descriptor {
        bustype: id.bus.0,
        capabilities: Capabilities { abs, ff, key, msc, rel },
        name: {
            let Ok(name) = finding::named(device);

            name
        },
        phys: {
            let Ok(phys) = finding::wired(device);

            phys
        },
        product: id.product,
        properties,
        uniq: String::new(),
        vendor: id.vendor,
        version: id.version,
    })
}

fn main() -> ExitCode {
    let mut found: BTreeMap<u32, Descriptor> = BTreeMap::new();

    let Ok(every) = Device::every();

    for device in every {
        let Ok(name) = finding::named(&device);

        let at = match (0_u32..).zip(ROLES.iter()).find(|(_, (wanted, _))| *wanted == name) {
            Some((at, _)) => at,
            None => continue,
        };

        match found.contains_key(&at) {
            true => continue,
            false => {},
        }

        let said = match described(&device) {
            Ok(said) => said,
            Err(why) => {
                eprintln!("{name} would not be read: {why}");
                return ExitCode::from(1);
            },
        };

        let _ = found.insert(at, said);
    }

    let missing: Vec<&str> = (0_u32..)
        .zip(ROLES.iter())
        .filter(|(at, _)| !found.contains_key(at))
        .map(|(_, (name, _))| *name)
        .collect();

    match missing.is_empty() {
        true => {},
        false => eprintln!("not present: {}", missing.join(", ")),
    }

    let written: Vec<Descriptor> = found.into_values().collect();

    match serde_json::to_string_pretty(&written) {
        Ok(said) => println!("{said}"),
        Err(fault) => {
            eprintln!("the capture would not write: {fault}");
            return ExitCode::from(1);
        }
    }

    ExitCode::from(u8::from(!missing.is_empty()))
}
