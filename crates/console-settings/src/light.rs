//! How much light is in the room, as the sensor's own number.
//!
//! The panel this desktop was written for has an ambient light sensor on the
//! AMD sensor hub, and what it says is read here and nowhere else. The same
//! rule the backlight already follows: the sensor is walked for rather than
//! named, because `iio:device0` is one machine's ordering of three sensors and
//! the accelerometer is first on a machine that enumerates them the other way.
//! What makes a directory here the light one is that it has a reading of light
//! in it.
//!
//! **The number is the sensor's raw one, and the scale beside it is not read.**
//! `in_illuminance_scale` is a decimal, this panel's is a tenth, and turning a
//! raw reading into lux would put a float in the middle of the one calculation
//! in this feature that has to give the same answer twice. It buys nothing:
//! nothing here ever says a number of lux out loud, and what the reading is
//! for is deciding which band of light the room is in -- a question about
//! where this reading sits against the others this sensor has given, which the
//! raw number answers exactly as well. A sensor whose raw numbers run a
//! thousand times higher than this one's lands in higher bands and learns
//! them; the ladder is wide enough for that and nothing in it is this panel's.
//!
//! The colour channels are not here, and it is not an oversight. This sensor
//! offers colour temperature and chromaticity beside the illuminance, which is
//! what the evening warmth would want, and on this machine every one of them
//! reads zero for ever while the illuminance tracks the room. The kernel says
//! why, once every few seconds: `Event data for report 4 was too short`. The
//! firmware sends a longer record than the driver accepts and the driver drops
//! it. So the warm curve keeps its clock, and this reads the one channel that
//! answers.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use console_core_never::Never;

use crate::screen::whole;

pub const UNDER: &str = "/sys/bus/iio/devices";

pub const RAW: &str = "in_illuminance_raw";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Lit(pub i64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sensor {
    pub at: PathBuf,
}

impl Sensor {
    pub fn reading(&self) -> Result<PathBuf, Never> {
        Ok(self.at.join(RAW))
    }

    pub fn now(&self) -> Result<Option<Lit>, Never> {
        let Ok(at) = self.reading();

        let said = match std::fs::read_to_string(at) {
            Err(_the_sensor_would_not_say) => return Ok(None),
            Ok(said) => said,
        };

        let Ok(read) = whole(&said);

        Ok(read.map(Lit))
    }
}

pub fn read(at: &Path) -> Result<Option<Sensor>, Never> {
    Ok(match at.join(RAW).exists() {
        true => Some(Sensor { at: at.to_path_buf() }),
        false => None,
    })
}

pub fn found(under: &Path) -> Result<Option<Sensor>, Never> {
    let held = match std::fs::read_dir(under) {
        Err(_no_sensors_on_this_machine) => return Ok(None),
        Ok(held) => held,
    };

    let mut every: Vec<(OsString, Sensor)> = Vec::new();

    for one in held.flatten() {
        let Ok(read) = read(&one.path());

        match read {
            Some(sensor) => every.push((one.file_name(), sensor)),
            None => {},
        }
    }

    every.sort_by(|left, right| left.0.cmp(&right.0));

    Ok(every.into_iter().next().map(|(_whatever_it_was_called, sensor)| sensor))
}

pub fn here() -> Result<Option<Sensor>, Never> {
    found(Path::new(UNDER))
}
