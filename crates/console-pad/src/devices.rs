//! The four devices, and the two places they can be.
//!
//! Everything that presses a button writes into a `Devices`. What that is
//! made of is either the kernel, through uinput, or a `World` that exists
//! inside one test. The arithmetic that turns a push into a number is here and
//! not in either of them, so both answer the same.


use console_number::whole_i32;
use std::collections::BTreeMap;

use evdev::EventType;

use crate::capture::{Axis, Descriptor};

/// Somewhere an event can be written.
/// Whether a thing is among the things there are.
///
/// One type for every membership question in this crate -- a role with a
/// device behind it, a button a pad can send, a key a device claims. They are
/// all the same question and a reader who learns it once has learnt it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Has {
    /// It is there.
    Yes,
    /// It is not.
    No,
}

/// Whether one event ends the frame.
///
/// Every event a device sends belongs to a frame, and the reader only acts on
/// a frame once it is reported complete. Most calls send one event and end it;
/// the ones that build a frame out of several say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    /// End the frame here, which is what a single event wants.
    Now,
    /// Hold it open, because more of this frame is coming.
    Later,
}

pub trait Sink {
    /// Where the kernel put it, which is how a daemon is pointed at it.
    fn path(&self, role: &str) -> Option<String>;

    fn write(&mut self, role: &str, kind: EventType, code: u16, value: i32);

    fn syn(&mut self, role: &str);

    fn close(&mut self);

    /// Whether this is one of the devices there are.
    fn has(&self, role: &str) -> Has;
}

/// The devices, and what each one is.
pub struct Devices<S: Sink> {
    pub descriptors: BTreeMap<String, Descriptor>,
    pub sink: S,
}

impl<S: Sink> Devices<S> {
    pub fn new(descriptors: BTreeMap<String, Descriptor>, sink: S) -> Self {
        Devices { descriptors, sink }
    }

    pub fn has(&self, role: &str) -> Has {
        self.sink.has(role)
    }

    pub fn path(&self, role: &str) -> Option<String> {
        self.sink.path(role)
    }

    pub fn paths(&self) -> BTreeMap<String, String> {
        self.descriptors
            .keys()
            .filter_map(|role| self.path(role).map(|path| (role.clone(), path)))
            .collect()
    }

    /// One event, and by default the report that ends the frame.
    pub fn emit(&mut self, role: &str, kind: EventType, code: u16, value: i32, syn: Report) {
        self.sink.write(role, kind, code, value);

        if syn == Report::Now {
            self.sink.syn(role);
        }
    }

    pub fn syn(&mut self, role: &str) {
        self.sink.syn(role);
    }

    pub fn close(&mut self) {
        self.sink.close();
    }

    /// One axis of one device, by the code it reports on.
    pub fn axis(&self, role: &str, code: u16) -> Result<Axis, String> {
        self.descriptors
            .get(role)
            .and_then(|found| found.axis(code))
            .ok_or_else(|| format!("{role} has no axis {code}"))
    }

    /// A push from -1 to 1, in the numbers the axis actually reports.
    pub fn absolute(&self, role: &str, code: u16, amount: f64) -> Result<i32, String> {
        let axis = self.axis(role, code)?;
        Ok(whole_i32(amount.clamp(-1.0, 1.0) * f64::from(axis.span())))
    }

    /// A pull from 0 to 1, over whatever range the trigger reports.
    pub fn along(&self, role: &str, code: u16, amount: f64) -> Result<i32, String> {
        let axis = self.axis(role, code)?;
        Ok(whole_i32(f64::from(axis.min) + amount * f64::from(axis.max - axis.min)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::captured;
    use crate::world::World;

    fn devices() -> Devices<World> {
        Devices::new(captured().expect("the capture carried in this program parses"), World::of(captured().expect("the capture carried in this program parses")))
    }

    #[test]
    fn a_stick_pushed_all_the_way_reads_the_edge_of_its_range() {
        let devices = devices();
        let span = f64::from(devices.axis("pad", 0).expect("ABS_X").span());
        assert_eq!(devices.absolute("pad", 0, 1.0), Ok(span as i32));
        assert_eq!(devices.absolute("pad", 0, 0.0), Ok(0));
        assert_eq!(devices.absolute("pad", 0, -1.0), Ok(-(span as i32)));
    }

    #[test]
    fn a_stick_pushed_further_than_all_the_way_is_still_all_the_way() {
        let devices = devices();
        assert_eq!(devices.absolute("pad", 0, 4.0), devices.absolute("pad", 0, 1.0));
    }

    #[test]
    fn a_trigger_runs_from_one_end_of_its_range_to_the_other() {
        let devices = devices();
        let axis = devices.axis("pad", 2).expect("ABS_Z");
        assert_eq!(devices.along("pad", 2, 0.0), Ok(axis.min));
        assert_eq!(devices.along("pad", 2, 1.0), Ok(axis.max));
    }

    #[test]
    fn an_axis_a_device_does_not_have_says_so() {
        assert!(devices().axis("keyboard", 0).is_err());
    }
}
