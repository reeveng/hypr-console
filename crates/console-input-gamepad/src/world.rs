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

use console_core_never::Never;
use evdev::{EventType, InputEvent};

use crate::capture::Descriptor;
use crate::devices::{Has, Sink};

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub path: String,
    pub waiting: Vec<InputEvent>,
    pub plugged: bool,
}

impl Device {
    pub fn unplug(&mut self) -> Result<(), Never> {
        self.plugged = false;
        self.waiting.clear();

        Ok(())
    }

    pub fn plug(&mut self) -> Result<(), Never> {
        self.plugged = true;

        Ok(())
    }

    pub fn drain(&mut self) -> Result<Vec<InputEvent>, Never> {
        Ok(std::mem::take(&mut self.waiting))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Written {
    pub kind: EventType,
    pub code: u16,
    pub value: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct World {
    pub devices: BTreeMap<String, Device>,
    pub log: Vec<(String, Written)>,
}

impl World {
    pub fn of(descriptors: BTreeMap<String, Descriptor>) -> Result<Self, Never> {
        Ok(World {
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
        })
    }

    pub fn plugged(&self) -> Result<Vec<String>, Never> {
        Ok(self.devices.values().filter(|device| device.plugged).map(|d| d.path.clone()).collect())
    }

    pub fn role_at(&self, path: &str) -> Result<Option<&str>, Never> {
        Ok(self
            .devices
            .iter()
            .find(|(_, device)| device.path == path && device.plugged)
            .map(|(role, _)| role.as_str()))
    }

    pub fn written(&self, role: &str) -> Result<Vec<Written>, Never> {
        Ok(self.log.iter().filter(|(said, _)| said == role).map(|(_, what)| *what).collect())
    }

    pub fn of_kind(
        &self,
        role: &str,
        kind: EventType,
        code: Option<u16>,
    ) -> Result<Vec<Written>, Never> {
        let written = self.written(role)?;

        Ok(written
            .into_iter()
            .filter(|what| what.kind == kind && code.is_none_or(|wanted| what.code == wanted))
            .collect())
    }

    pub fn total(&self, role: &str, kind: EventType, code: u16) -> Result<i32, Never> {
        let of_kind = self.of_kind(role, kind, Some(code))?;

        Ok(of_kind.iter().map(|what| what.value).sum())
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
        match self.devices.get_mut(role) {
            Some(device) => {
                device.waiting.push(InputEvent::new(kind.0, code, value));
                self.log.push((role.to_string(), Written { kind, code, value }));
            }
            None => {},
        }
    }

    fn syn(&mut self, role: &str) {
        match self.devices.get_mut(role) {
            Some(device) => {
                device.waiting.push(InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0));
            }
            None => {},
        }
    }

    fn close(&mut self) {
        for device in self.devices.values_mut() {
            let Ok(()) = device.unplug();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::captured;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    fn world() -> World {
        ok(World::of(captured().expect("the capture carried in this program parses")))
    }

    #[test]
    fn every_device_gets_a_path_of_its_own() {
        let world = world();
        let mut paths: Vec<String> = world.devices.values().map(|d| d.path.clone()).collect();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), world.devices.len());
    }

    #[test]
    fn what_is_written_is_waiting_and_is_remembered() {
        let mut world = world();
        world.write("pad", EventType::KEY, 304, 1);
        world.syn("pad");
        assert_eq!(ok(world.written("pad")), [Written { kind: EventType::KEY, code: 304, value: 1 }]);
        assert_eq!(world.devices["pad"].waiting.len(), 2, "the event and its report");
    }

    #[test]
    fn a_device_that_has_gone_is_not_there_to_be_found() {
        let mut world = world();
        let path = world.path("pad").expect("a pad");
        assert_eq!(ok(world.role_at(&path)), Some("pad"));
        ok(world.devices.get_mut("pad").expect("a pad").unplug());
        assert_eq!(ok(world.role_at(&path)), None);
        assert!(!ok(world.plugged()).contains(&path));
    }

    #[test]
    fn a_wheel_is_how_far_it_turned_rather_than_how_often() {
        let mut world = world();
        for notch in [1, 1, -1] {
            world.write("mouse", EventType::RELATIVE, 8, notch);
        }
        assert_eq!(ok(world.total("mouse", EventType::RELATIVE, 8)), 1);
        assert_eq!(ok(world.of_kind("mouse", EventType::RELATIVE, Some(8))).len(), 3);
    }

    #[test]
    fn writing_to_a_device_that_is_not_there_writes_nothing() {
        let mut world = world();
        world.write("trackball", EventType::KEY, 1, 1);
        assert!(world.log.is_empty());
    }
}
