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

use evdev::{AbsoluteAxisCode, Device, EventType, KeyCode, MiscCode, PropType, RelativeAxisCode};
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

fn described(device: &Device) -> Result<Descriptor, Unread> {
    let id = device.input_id();
    let listed = |kind: EventType| -> Vec<u16> {
        let every = match kind {
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
        });

        match every {
            Some(every) => every,
            None => Vec::new(),
        }
    };
    let mut properties: Vec<u16> =
        device.properties().iter().map(|PropType(what)| what).collect();
    properties.sort_unstable();
    let mut abs: Vec<Axis> = device
        .get_absinfo()
        .map_err(Unread)
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
        })?;
    abs.sort_unstable_by_key(|axis| axis.code);

    Ok(Descriptor {
        bustype: id.bus_type().0,
        capabilities: Capabilities {
            abs,
            ff: listed(EventType::FORCEFEEDBACK),
            key: listed(EventType::KEY),
            msc: listed(EventType::MISC),
            rel: listed(EventType::RELATIVE),
        },
        name: {
            let Ok(name) = finding::named(device);

            name
        },
        phys: {
            let Ok(phys) = finding::wired(device);

            phys
        },
        product: id.product(),
        properties,
        uniq: String::new(),
        vendor: id.vendor(),
        version: id.version(),
    })
}

fn main() -> ExitCode {
    let mut found: BTreeMap<usize, Descriptor> = BTreeMap::new();

    for (_, device) in evdev::enumerate() {
        let Ok(name) = finding::named(&device);

        let at = match ROLES.iter().position(|(wanted, _)| *wanted == name) {
            Some(at) => at,
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

    let missing: Vec<&str> = ROLES
        .iter()
        .enumerate()
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
