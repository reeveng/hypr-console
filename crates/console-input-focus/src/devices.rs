//! The layer that talks to the machine: an input device, taken and handed back.
//! DeviceKind device is which is not decided here. `console_input_gamepad::finding`
//! is the one place that says the pad to read is the one InputPlumber made
//! rather than the one someone is holding, and the daemon applies exactly that
//! rule -- so a claim that went looking on its own could take a different
//! device than the daemon reads, which is the fault this crate exists to stop,
//! arrived at from inside.

use std::collections::BTreeMap;
use std::os::fd::{AsFd, BorrowedFd};

use console_core_words::Words;
use console_input_gamepad::finding::{self, DeviceInfo};
use console_core_never::Never;
use console_input_event_devices::{AbsoluteAxisCode, Device, InputEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Words)]
pub enum DeviceKind {
    #[words(said = "gamepad")]
    Pad,
    #[words(said = "keyboard")]
    Keys,
    #[words(said = "a keyboard someone plugged in")]
    Typing,
    #[words(said = "touchpad")]
    Touch,
}

impl DeviceKind {
    fn among(self, said: &[DeviceInfo]) -> Result<Vec<&DeviceInfo>, Never> {
        let one = match self {
            DeviceKind::Pad => finding::gamepad(said)?,
            DeviceKind::Keys => finding::keyboard(said)?,
            DeviceKind::Typing => return finding::typing(said),
            DeviceKind::Touch => finding::touchpad(said)?,
        };

        Ok(one.into_iter().collect())
    }
}

pub type Spans = Vec<(AbsoluteAxisCode, (i32, i32))>;

pub const CONTROLLER: [DeviceKind; 2] = [DeviceKind::Pad, DeviceKind::Keys];

pub const EVERYTHING_PRESSED: [DeviceKind; 3] = [DeviceKind::Pad, DeviceKind::Keys, DeviceKind::Typing];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimError {
    NothingToTake,
    HeldElsewhere { path: String, why: String },
    Failed { path: String, why: String },
}

impl ClaimError {
    pub fn said(&self) -> Result<String, Never> {
        Ok(match self {
            ClaimError::NothingToTake => {
                "nothing to read: none of these devices is on this machine".to_string()
            }
            ClaimError::HeldElsewhere { path, why } => {
                format!("{path}: something else has the input ({why})")
            }
            ClaimError::Failed { path, why } => format!("{path}: {why}"),
        })
    }
}

struct Claimed {
    path: String,
    which: DeviceKind,
    device: Device,
}

pub struct Claim {
    held: Vec<Claimed>,
}

impl Claim {
    pub fn of(wanted: &[DeviceKind]) -> Result<Claim, ClaimError> {
        let Ok(every) = Device::every();
        let seen: Vec<(String, Device)> =
            every.into_iter().map(|device| (device.path.display().to_string(), device)).collect();
        let said: Vec<DeviceInfo> = seen
            .iter()
            .map(|(path, device)| {
                let Ok(says) = finding::describe(path, device);

                says
            })
            .collect();
        let mut found: BTreeMap<String, DeviceKind> = BTreeMap::new();

        for which in wanted {
            let Ok(among) = which.among(&said);

            for says in among {
                let _ = found.entry(says.path.clone()).or_insert(*which);
            }
        }

        match found.is_empty() {
            true => return Err(ClaimError::NothingToTake),
            false => {}
        }

        let mut held = Vec::new();

        for (path, device) in seen {
            let asked = found.get(&path).copied();

            let which = match asked {
                Some(which) => which,
                None => continue,
            };

            let taken = take(path, which, device)?;

            held.push(taken);
        }

        Ok(Claim { held })
    }

    pub fn holding(&self) -> Result<Vec<(DeviceKind, &str)>, Never> {
        Ok(self.held.iter().map(|taken| (taken.which, taken.path.as_str())).collect())
    }

    pub fn watching(&self) -> Result<Vec<BorrowedFd<'_>>, Never> {
        Ok(self.held.iter().map(|taken| taken.device.as_fd()).collect())
    }

    pub fn spans(&self, which: DeviceKind) -> Result<Spans, ClaimError> {
        let mut spans = Spans::new();

        for taken in self.held.iter().filter(|taken| taken.which == which) {
            let told = match taken.device.absolute() {
                Ok(told) => told,
                Err(fault) => {
                    return Err(ClaimError::Failed {
                        path: taken.path.clone(),
                        why: format!("it would not say what its sticks run between: {fault}"),
                    });
                }
            };

            spans.extend(told.into_iter().map(|(axis, information)| (axis, (information.minimum, information.maximum))));
        }

        Ok(spans)
    }

    pub fn arrived(&mut self) -> Result<Received, Never> {
        let mut heard = Received::default();

        let mut lost: Vec<String> = Vec::new();

        for taken in &mut self.held {
            match taken.device.read_events() {
                Ok(arrived) => heard.events.extend(arrived.into_iter().map(|event| (taken.which, event))),
                Err(fault) => match fault.kind() == std::io::ErrorKind::WouldBlock {
                    true => {}
                    false => {
                        heard.unplugged.push(taken.which);
                        lost.push(taken.path.clone());
                    }
                },
            }
        }

        #[cfg_attr(
            dylint_lib = "explicit028_no_search_in_a_loop",
            allow(
                explicit028_no_search_in_a_loop,
                reason = "what was lost is what went away during one read, and what is held is the devices on the machine: a handful against a handful"
            )
        )]
        self.held.retain(|taken| !lost.contains(&taken.path));

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
pub struct Received {
    pub events: Vec<(DeviceKind, InputEvent)>,
    pub unplugged: Vec<DeviceKind>,
}

fn take(path: String, which: DeviceKind, device: Device) -> Result<Claimed, ClaimError> {
    match device.nonblocking() {
        Ok(()) => {}
        Err(fault) => {
            return Err(ClaimError::Failed {
                path,
                why: format!("it will not read without blocking: {fault}"),
            });
        }
    }

    match device.grab() {
        Ok(()) => Ok(Claimed { path, which, device }),
        Err(fault) => Err(ClaimError::HeldElsewhere { path, why: fault.to_string() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_controller_is_both_of_the_devices_a_button_arrives_on() {
        assert_eq!(CONTROLLER, [DeviceKind::Pad, DeviceKind::Keys]);
        assert!(!CONTROLLER.contains(&DeviceKind::Touch), "a finger is not a button");
    }

    #[test]
    fn every_device_says_what_it_is_in_words() {
        for which in [DeviceKind::Pad, DeviceKind::Keys, DeviceKind::Typing, DeviceKind::Touch] {
            let Ok(said) = which.said();

            assert!(!said.is_empty(), "{which:?} has no name to complain in");
        }
    }

    #[test]
    fn a_refusal_says_which_it_was_in_words() {
        let elsewhere = ClaimError::HeldElsewhere {
            path: "/dev/input/event5".to_string(),
            why: "Device or resource busy".to_string(),
        };
        let Ok(said) = elsewhere.said();
        let Ok(nothing) = ClaimError::NothingToTake.said();

        assert!(said.contains("something else has the input"));
        assert!(said.contains("event5"), "and which device it was");
        assert!(nothing.contains("nothing to read"));
    }
}
