//! The four devices, and the two places they can be.
//!
//! Everything that presses a button writes into a `Devices`. What that is
//! made of is either the kernel, through uinput, or a `World` that exists
//! inside one test. The arithmetic that turns a push into a number is here and
//! not in either of them, so both answer the same.


use console_core_never::Never;
use console_core_number_conversion::whole_i32;
use std::collections::BTreeMap;

use console_input_event_devices::EventType;

use crate::GamepadError;
use crate::capture::{Axis, Descriptor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Has {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    Now,
    Later,
}

pub trait Sink {
    fn path(&self, role: &str) -> Option<String>;

    fn write(&mut self, role: &str, kind: EventType, code: u16, value: i32);

    fn syn(&mut self, role: &str);

    fn close(&mut self);

    fn has(&self, role: &str) -> Has;
}

pub struct Devices<S: Sink> {
    pub descriptors: BTreeMap<String, Descriptor>,
    pub sink: S,
}

impl<S: Sink> Devices<S> {
    pub fn new(descriptors: BTreeMap<String, Descriptor>, sink: S) -> Result<Self, Never> {
        Ok(Devices { descriptors, sink })
    }

    pub fn has(&self, role: &str) -> Result<Has, Never> {
        Ok(self.sink.has(role))
    }

    pub fn path(&self, role: &str) -> Result<Option<String>, Never> {
        Ok(self.sink.path(role))
    }

    pub fn paths(&self) -> Result<BTreeMap<String, String>, Never> {
        let mut found = BTreeMap::new();

        for role in self.descriptors.keys() {
            let path = self.path(role)?;

            match path {
                Some(path) => {
                    found.insert(role.clone(), path);
                }
                None => {},
            }
        }

        Ok(found)
    }

    pub fn emit(
        &mut self,
        role: &str,
        kind: EventType,
        code: u16,
        value: i32,
        syn: Report,
    ) -> Result<(), Never> {
        self.sink.write(role, kind, code, value);

        match syn {
            Report::Now => self.sink.syn(role),
            Report::Later => {},
        }

        Ok(())
    }

    pub fn syn(&mut self, role: &str) -> Result<(), Never> {
        self.sink.syn(role);

        Ok(())
    }

    pub fn close(&mut self) -> Result<(), Never> {
        self.sink.close();

        Ok(())
    }

    pub fn axis(&self, role: &str, code: u16) -> Result<Axis, GamepadError> {
        let held = match self.descriptors.get(role) {
            Some(found) => {
                let Ok(axis) = found.axis(code);

                axis
            }
            None => None,
        };

        held.ok_or_else(|| GamepadError::NoAxis(role.to_string(), code))
    }

    pub fn absolute(&self, role: &str, code: u16, amount: f64) -> Result<i32, GamepadError> {
        let axis = self.axis(role, code)?;

        let Ok(span) = axis.span();

        let Ok(along) = whole_i32(amount.clamp(-1.0, 1.0) * f64::from(span));

        Ok(along)
    }

    pub fn along(&self, role: &str, code: u16, amount: f64) -> Result<i32, GamepadError> {
        let axis = self.axis(role, code)?;

        let Ok(along) =
            whole_i32(f64::from(axis.minimum) + amount * f64::from(axis.maximum.saturating_sub(axis.minimum)));

        Ok(along)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::captured;
    use crate::world::World;

    fn devices() -> Devices<World> {
        let descriptors = captured().expect("the capture carried in this program parses");
        let Ok(world) = World::of(captured().expect("the capture carried in this program parses"));

        let Ok(devices) = Devices::new(descriptors, world);

        devices
    }

    #[test]
    fn a_stick_pushed_all_the_way_reads_the_edge_of_its_range() {
        let devices = devices();
        let axis = devices.axis("pad", 0).expect("ABS_X");

        let Ok(span) = axis.span();

        let span = f64::from(span);
        assert_eq!(devices.absolute("pad", 0, 1.0).expect("the edge"), span as i32);
        assert_eq!(devices.absolute("pad", 0, 0.0).expect("the middle"), 0);
        assert_eq!(devices.absolute("pad", 0, -1.0).expect("the other edge"), -(span as i32));
    }

    #[test]
    fn a_stick_pushed_further_than_all_the_way_is_still_all_the_way() {
        let devices = devices();
        assert_eq!(
            devices.absolute("pad", 0, 4.0).expect("further"),
            devices.absolute("pad", 0, 1.0).expect("all the way")
        );
    }

    #[test]
    fn a_trigger_runs_from_one_end_of_its_range_to_the_other() {
        let devices = devices();
        let axis = devices.axis("pad", 2).expect("ABS_Z");
        assert_eq!(devices.along("pad", 2, 0.0).expect("let go"), axis.minimum);
        assert_eq!(devices.along("pad", 2, 1.0).expect("pulled"), axis.maximum);
    }

    #[test]
    fn an_axis_a_device_does_not_have_says_so() {
        assert!(devices().axis("keyboard", 0).is_err());
    }
}
