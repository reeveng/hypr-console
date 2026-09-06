//! The layer that talks to the machine: an input device, taken and handed back.
//!
//! Which device is which is not decided here. `console_gamepad::finding` is the
//! one place that says the pad to read is the one InputPlumber made rather than
//! the one somebody is holding, and the daemon applies exactly that rule -- so a
//! claim that went looking on its own could take a different device than the
//! daemon reads, which is the fault this crate exists to stop, arrived at from
//! inside.

use std::os::fd::{AsRawFd, RawFd};

use console_gamepad::finding::{self, Says};
use console_never::Never;
use evdev::{AbsoluteAxisCode, Device, InputEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Which {
    Pad,
    Keys,
    Touch,
}

impl Which {
    pub fn said(self) -> Result<&'static str, Never> {
        Ok(match self {
            Which::Pad => "gamepad",
            Which::Keys => "keyboard",
            Which::Touch => "touchpad",
        })
    }

    fn among(self, said: &[Says]) -> Result<Option<&Says>, Never> {
        match self {
            Which::Pad => finding::gamepad(said),
            Which::Keys => finding::keyboard(said),
            Which::Touch => finding::touchpad(said),
        }
    }
}

pub type Spans = Vec<(AbsoluteAxisCode, (i32, i32))>;

pub const CONTROLLER: [Which; 2] = [Which::Pad, Which::Keys];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    NothingToTake,
    HeldElsewhere { path: String, why: String },
    Fault { path: String, why: String },
}

impl Refused {
    pub fn said(&self) -> Result<String, Never> {
        Ok(match self {
            Refused::NothingToTake => {
                "nothing to read: none of these devices is on this machine".to_string()
            }
            Refused::HeldElsewhere { path, why } => {
                format!("{path}: something else has the input ({why})")
            }
            Refused::Fault { path, why } => format!("{path}: {why}"),
        })
    }
}

struct Taken {
    path: String,
    which: Which,
    device: Device,
}

pub struct Claim {
    held: Vec<Taken>,
}

impl Claim {
    pub fn of(wanted: &[Which]) -> Result<Claim, Refused> {
        let seen: Vec<(String, Device)> = evdev::enumerate()
            .map(|(path, device)| (path.display().to_string(), device))
            .collect();
        let said: Vec<Says> = seen
            .iter()
            .map(|(path, device)| {
                let Ok(says) = finding::says(path, device);

                says
            })
            .collect();
        let found: Vec<(Which, String)> = wanted
            .iter()
            .filter_map(|which| {
                let Ok(among) = which.among(&said);

                among.map(|says| (*which, says.path.clone()))
            })
            .collect();

        match found.is_empty() {
            true => return Err(Refused::NothingToTake),
            false => {}
        }

        let mut held = Vec::new();

        for (path, device) in seen {
            let asked = found.iter().find(|(_, at)| *at == path).map(|(which, _)| *which);

            let Some(which) = asked else { continue };

            let taken = take(path, which, device)?;

            held.push(taken);
        }

        Ok(Claim { held })
    }

    pub fn holding(&self) -> Result<Vec<(Which, &str)>, Never> {
        Ok(self.held.iter().map(|taken| (taken.which, taken.path.as_str())).collect())
    }

    pub fn watching(&self) -> Result<Vec<RawFd>, Never> {
        Ok(self.held.iter().map(|taken| taken.device.as_raw_fd()).collect())
    }

    pub fn spans(&self, which: Which) -> Result<Spans, Refused> {
        let mut spans = Spans::new();

        for taken in self.held.iter().filter(|taken| taken.which == which) {
            let told = match taken.device.get_absinfo() {
                Ok(told) => told,
                Err(fault) => {
                    return Err(Refused::Fault {
                        path: taken.path.clone(),
                        why: format!("it would not say what its sticks run between: {fault}"),
                    });
                }
            };

            spans.extend(told.map(|(axis, info)| (axis, (info.minimum(), info.maximum()))));
        }

        Ok(spans)
    }

    pub fn arrived(&mut self) -> Result<Heard, Never> {
        let mut heard = Heard::default();

        for taken in &mut self.held {
            match taken.device.fetch_events() {
                Ok(arrived) => heard.events.extend(arrived.map(|event| (taken.which, event))),
                Err(fault) if fault.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => heard.gone.push(taken.which),
            }
        }

        self.held.retain(|taken| !heard.gone.contains(&taken.which));

        Ok(heard)
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        for taken in &mut self.held {
            let _ = taken.device.ungrab();
        }
    }
}

#[derive(Debug, Default)]
pub struct Heard {
    pub events: Vec<(Which, InputEvent)>,
    pub gone: Vec<Which>,
}

fn take(path: String, which: Which, mut device: Device) -> Result<Taken, Refused> {
    match device.set_nonblocking(true) {
        Ok(()) => {}
        Err(fault) => {
            return Err(Refused::Fault {
                path,
                why: format!("it will not read without blocking: {fault}"),
            });
        }
    }

    match device.grab() {
        Ok(()) => Ok(Taken { path, which, device }),
        Err(fault) => Err(Refused::HeldElsewhere { path, why: fault.to_string() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_controller_is_both_of_the_devices_a_button_arrives_on() {
        assert_eq!(CONTROLLER, [Which::Pad, Which::Keys]);
        assert!(!CONTROLLER.contains(&Which::Touch), "a finger is not a button");
    }

    #[test]
    fn every_device_says_what_it_is_in_words() {
        for which in [Which::Pad, Which::Keys, Which::Touch] {
            let Ok(said) = which.said();

            assert!(!said.is_empty(), "{which:?} has no name to complain in");
        }
    }

    #[test]
    fn a_refusal_says_which_it_was_in_words() {
        let elsewhere = Refused::HeldElsewhere {
            path: "/dev/input/event5".to_string(),
            why: "Device or resource busy".to_string(),
        };
        let Ok(said) = elsewhere.said();
        let Ok(nothing) = Refused::NothingToTake.said();

        assert!(said.contains("something else has the input"));
        assert!(said.contains("event5"), "and which device it was");
        assert!(nothing.contains("nothing to read"));
    }
}
