//! A world of input devices that exist only inside one test.
//!
//! The daemons find their devices by asking evdev what is plugged in. That is
//! the right way round on the machine and the wrong way round in a test: it
//! needs /dev/uinput, root, and a kernel that will then deliver whatever comes
//! out to whatever has focus. So the same daemons are run against this, with
//! devices built from the same capture the real emulator uses.
//!
//! What this gives that the real thing cannot is a clock. Time is a number
//! somebody else holds, so a stick held for exactly one second scrolls exactly
//! as far as the arithmetic says, every run, on any machine.

use std::collections::BTreeMap;

use evdev::{EventType, InputEvent};

use crate::capture::Descriptor;
use crate::devices::{Has, Sink};

/// One device, and the events waiting on it.
#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub path: String,
    pub waiting: Vec<InputEvent>,
    pub plugged: bool,
}

impl Device {
    /// What a profile switch does: the device is gone mid-read.
    pub fn unplug(&mut self) {
        self.plugged = false;
        self.waiting.clear();
    }

    pub fn plug(&mut self) {
        self.plugged = true;
    }

    /// Everything waiting, taken.
    pub fn drain(&mut self) -> Vec<InputEvent> {
        std::mem::take(&mut self.waiting)
    }
}

/// One thing that was written, in the words this crate speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Written {
    pub kind: EventType,
    pub code: u16,
    pub value: i32,
}

/// Every device there is, and it is only these.
///
/// Stands in for both halves at once: the emulator writes into it as if it
/// were a set of uinput devices, and a daemon reads out of it as if it were
/// the input subsystem.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct World {
    pub devices: BTreeMap<String, Device>,
    pub log: Vec<(String, Written)>,
}

impl World {
    /// A world of the given devices, numbered the way the kernel numbers them.
    pub fn of(descriptors: BTreeMap<String, Descriptor>) -> Self {
        World {
            devices: descriptors
                .keys()
                .enumerate()
                .map(|(number, role)| {
                    let device = Device {
                        path: format!("/dev/input/event{number}"),
                        waiting: Vec::new(),
                        plugged: true,
                    };
                    (role.clone(), device)
                })
                .collect(),
            log: Vec::new(),
        }
    }

    /// The paths of everything still plugged in, which is what a daemon sees
    /// when it asks what there is.
    pub fn plugged(&self) -> Vec<String> {
        self.devices.values().filter(|device| device.plugged).map(|d| d.path.clone()).collect()
    }

    /// The role a path belongs to, if anything still answers to it.
    pub fn role_at(&self, path: &str) -> Option<&str> {
        self.devices
            .iter()
            .find(|(_, device)| device.path == path && device.plugged)
            .map(|(role, _)| role.as_str())
    }

    /// Everything written to one device, in order.
    pub fn written(&self, role: &str) -> Vec<Written> {
        self.log.iter().filter(|(said, _)| said == role).map(|(_, what)| *what).collect()
    }

    /// Everything written to one device of one kind, and of one code if named.
    pub fn of_kind(&self, role: &str, kind: EventType, code: Option<u16>) -> Vec<Written> {
        self.written(role)
            .into_iter()
            .filter(|what| what.kind == kind && code.is_none_or(|wanted| what.code == wanted))
            .collect()
    }

    /// Everything one axis or one button added up, which is how far a wheel
    /// turned or how many times a key went down.
    pub fn total(&self, role: &str, kind: EventType, code: u16) -> i32 {
        self.of_kind(role, kind, Some(code)).iter().map(|what| what.value).sum()
    }
}

impl Sink for World {
    fn path(&self, role: &str) -> Option<String> {
        self.devices.get(role).map(|device| device.path.clone())
    }

    fn has(&self, role: &str) -> Has {
        match self.devices.contains_key(role) {
            true => Has::Yes,
            false => Has::No,
        }
    }

    fn write(&mut self, role: &str, kind: EventType, code: u16, value: i32) {
        if let Some(device) = self.devices.get_mut(role) {
            device.waiting.push(InputEvent::new(kind.0, code, value));
            self.log.push((role.to_string(), Written { kind, code, value }));
        }
    }

    fn syn(&mut self, role: &str) {
        if let Some(device) = self.devices.get_mut(role) {
            device.waiting.push(InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0));
        }
    }

    fn close(&mut self) {
        self.devices.values_mut().for_each(Device::unplug);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::captured;

    #[test]
    fn every_device_gets_a_path_of_its_own() {
        let world = World::of(captured().expect("the capture carried in this program parses"));
        let mut paths: Vec<String> = world.devices.values().map(|d| d.path.clone()).collect();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), world.devices.len());
    }

    #[test]
    fn what_is_written_is_waiting_and_is_remembered() {
        let mut world = World::of(captured().expect("the capture carried in this program parses"));
        world.write("pad", EventType::KEY, 304, 1);
        world.syn("pad");
        assert_eq!(world.written("pad"), [Written { kind: EventType::KEY, code: 304, value: 1 }]);
        assert_eq!(world.devices["pad"].waiting.len(), 2, "the event and its report");
    }

    #[test]
    fn a_device_that_has_gone_is_not_there_to_be_found() {
        let mut world = World::of(captured().expect("the capture carried in this program parses"));
        let path = world.path("pad").expect("a pad");
        assert_eq!(world.role_at(&path), Some("pad"));
        world.devices.get_mut("pad").expect("a pad").unplug();
        assert_eq!(world.role_at(&path), None);
        assert!(!world.plugged().contains(&path));
    }

    #[test]
    fn a_wheel_is_how_far_it_turned_rather_than_how_often() {
        let mut world = World::of(captured().expect("the capture carried in this program parses"));
        for notch in [1, 1, -1] {
            world.write("mouse", EventType::RELATIVE, 8, notch);
        }
        assert_eq!(world.total("mouse", EventType::RELATIVE, 8), 1);
        assert_eq!(world.of_kind("mouse", EventType::RELATIVE, Some(8)).len(), 3);
    }

    #[test]
    fn writing_to_a_device_that_is_not_there_writes_nothing() {
        let mut world = World::of(captured().expect("the capture carried in this program parses"));
        world.write("trackball", EventType::KEY, 1, 1);
        assert!(world.log.is_empty());
    }
}
