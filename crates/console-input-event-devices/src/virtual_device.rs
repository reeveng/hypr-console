//! A device made through uinput, for as long as it is held.
//!
//! What it is is said in one `Setup` and made in one call, so a device is
//! either wholly what was asked for or not there at all: every step is a
//! request to the kernel and any of them can be refused, and the one that was
//! is named in what comes back. Letting go of it takes it off the machine,
//! because closing the file is what uinput hears as the device going.
//!
//! Every batch that is sent ends in a report, which is where the kernel
//! decides the events before it happened at once.

use std::ffi::{c_char, c_int, c_ulong};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use console_core_number_conversion::index;

use crate::codes::{AbsoluteAxisCode, EventType, MiscCode, PropType, RelativeAxisCode};
use crate::device::INPUT;
use crate::event::InputEvent;
use crate::kernel::{self, AbsInfo, AbsoluteSetup, InputId, checked, ioctl};
use crate::keys::KeyCode;

const UINPUT: &str = "/dev/uinput";

const VIRTUAL: &str = "/sys/devices/virtual/input";

const NODE: &str = "event";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Setup {
    pub name: String,
    pub id: InputId,
    pub physical_path: Option<String>,
    pub keys: Vec<KeyCode>,
    pub relative_axes: Vec<RelativeAxisCode>,
    pub absolute_axes: Vec<(AbsoluteAxisCode, AbsInfo)>,
    pub misc: Vec<MiscCode>,
    pub properties: Vec<PropType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Kinds,
    Keys,
    RelativeAxes,
    AbsoluteAxes,
    Misc,
    Properties,
    PhysicalPath,
    Describing,
    Creating,
}

#[derive(Debug)]
pub enum Unmade {
    NoUinput(io::Error),
    NameTooLong,
    NulInPhysicalPath,
    Rejected { step: Step, why: io::Error },
}

impl fmt::Display for Step {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        to.write_str(match self {
            Step::Kinds => "which kinds of event it sends",
            Step::Keys => "its keys",
            Step::RelativeAxes => "its relative axes",
            Step::AbsoluteAxes => "its absolute axes",
            Step::Misc => "its miscellaneous codes",
            Step::Properties => "its properties",
            Step::PhysicalPath => "where it is plugged in",
            Step::Describing => "its name and who made it",
            Step::Creating => "the device itself",
        })
    }
}

impl fmt::Display for Unmade {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unmade::NoUinput(why) => write!(to, "{UINPUT} would not open: {why}"),
            Unmade::NameTooLong => write!(to, "a uinput name is at most {} bytes", kernel::NAME),
            Unmade::NulInPhysicalPath => write!(to, "a physical path cannot hold a NUL"),
            Unmade::Rejected { step, why } => write!(to, "uinput refused {step}: {why}"),
        }
    }
}

impl std::error::Error for Unmade {}

pub struct VirtualDevice {
    file: File,
}

impl VirtualDevice {
    pub fn create(setup: &Setup) -> Result<VirtualDevice, Unmade> {
        let file = uinput()?;
        let device = VirtualDevice { file };

        device.kinds(setup)?;
        device.codes(setup)?;
        device.axes(&setup.absolute_axes)?;
        device.plugged(setup.physical_path.as_deref())?;
        device.described(setup)?;
        device.asked(Step::Creating, kernel::CREATE, 0)?;

        Ok(device)
    }

    fn kinds(&self, setup: &Setup) -> Result<(), Unmade> {
        let sent = [
            (EventType::KEY, setup.keys.is_empty()),
            (EventType::RELATIVE, setup.relative_axes.is_empty()),
            (EventType::ABSOLUTE, setup.absolute_axes.is_empty()),
            (EventType::MISC, setup.misc.is_empty()),
        ];

        for (kind, none) in sent {
            match none {
                true => {}
                false => self.asked(Step::Kinds, kernel::SET_EVENT_TYPE, kind.0)?,
            }
        }

        Ok(())
    }

    fn codes(&self, setup: &Setup) -> Result<(), Unmade> {
        let every = [
            (Step::Keys, kernel::SET_KEY, setup.keys.iter().map(|key| key.0).collect::<Vec<u16>>()),
            (Step::RelativeAxes, kernel::SET_RELATIVE_AXIS, setup.relative_axes.iter().map(|axis| axis.0).collect()),
            (Step::Misc, kernel::SET_MISC, setup.misc.iter().map(|misc| misc.0).collect()),
            (Step::Properties, kernel::SET_PROPERTY, setup.properties.iter().map(|property| property.0).collect()),
        ];

        for (step, request, codes) in every {
            for code in codes {
                self.asked(step, request, code)?;
            }
        }

        Ok(())
    }

    fn axes(&self, axes: &[(AbsoluteAxisCode, AbsInfo)]) -> Result<(), Unmade> {
        for (axis, info) in axes {
            let setup = AbsoluteSetup { code: axis.0, info: *info };

            self.asked(Step::AbsoluteAxes, kernel::SET_ABSOLUTE_AXIS, axis.0)?;

            // SAFETY: an open file, and a struct the size the request
            // number says, which the kernel only reads.
            let told = unsafe { ioctl(self.file.as_raw_fd(), kernel::ABSOLUTE_SETUP, &raw const setup) };

            refused(Step::AbsoluteAxes, told)?;
        }

        Ok(())
    }

    fn plugged(&self, physical_path: Option<&str>) -> Result<(), Unmade> {
        let physical_path = match physical_path {
            Some(physical_path) => physical_path,
            None => return Ok(()),
        };
        let physical_path = match std::ffi::CString::new(physical_path) {
            Ok(physical_path) => physical_path,
            Err(_) => return Err(Unmade::NulInPhysicalPath),
        };

        // SAFETY: an open file, and a NUL-ended string the kernel copies
        // before the call returns.
        let told = unsafe { ioctl(self.file.as_raw_fd(), kernel::SET_PHYSICAL_PATH, physical_path.as_ptr()) };

        refused(Step::PhysicalPath, told)
    }

    fn described(&self, setup: &Setup) -> Result<(), Unmade> {
        let mut name: [c_char; 80] = [0; 80];
        let written = setup.name.as_bytes();
        let Ok(room) = index(kernel::NAME);

        match written.len() < room {
            true => {}
            false => return Err(Unmade::NameTooLong),
        }

        for (into, byte) in name.iter_mut().zip(written) {
            *into = c_char::from_le_bytes([*byte]);
        }

        let described = kernel::UinputSetup { id: setup.id, name, effects: 0 };

        // SAFETY: an open file, and a struct the size the request number
        // says, which the kernel only reads.
        let told = unsafe { ioctl(self.file.as_raw_fd(), kernel::SETUP, &raw const described) };

        refused(Step::Describing, told)
    }

    fn asked(&self, step: Step, request: c_ulong, argument: u16) -> Result<(), Unmade> {
        // SAFETY: an open file, and a request that takes an int by value.
        let told = unsafe { ioctl(self.file.as_raw_fd(), request, c_int::from(argument)) };

        refused(step, told)
    }

    pub fn emit(&mut self, events: &[InputEvent]) -> io::Result<()> {
        let mut bytes = Vec::new();

        for event in events.iter().chain([&InputEvent::REPORT]) {
            let Ok(written) = event.bytes();

            bytes.extend(written);
        }

        self.file.write_all(&bytes)
    }

    pub fn nodes(&self) -> io::Result<Vec<PathBuf>> {
        let Ok(room) = index(kernel::SYSTEM_NAME);
        let mut buffer = vec![0_u8; room];

        // SAFETY: an open file, and a buffer the length the request number
        // says, which the kernel writes into and nothing else holds.
        let told = unsafe { ioctl(self.file.as_raw_fd(), kernel::GET_SYSTEM_NAME, buffer.as_mut_ptr()) };

        checked(told)?;

        let named = match buffer.split(|byte| *byte == 0).next() {
            Some(named) => String::from_utf8_lossy(named).into_owned(),
            None => String::new(),
        };
        let entries = fs::read_dir(PathBuf::from(VIRTUAL).join(named))?;
        let mut nodes: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.file_name())
            .filter(|name| name.to_string_lossy().starts_with(NODE))
            .map(|name| PathBuf::from(INPUT).join(name))
            .collect();

        nodes.sort();

        Ok(nodes)
    }
}

#[cfg_attr(
    dylint_lib = "explicit040_no_torn_write",
    allow(
        explicit040_no_torn_write,
        reason = "uinput is a character device rather than a file: what is written is a device being described and then events, nothing is kept, and there is no file beside it to rename over it"
    )
)]
fn uinput() -> Result<File, Unmade> {
    match OpenOptions::new().read(true).write(true).custom_flags(kernel::NOT_BLOCKING).open(UINPUT) {
        Ok(file) => Ok(file),
        Err(why) => Err(Unmade::NoUinput(why)),
    }
}

fn refused(step: Step, told: c_int) -> Result<(), Unmade> {
    match checked(told) {
        Ok(_) => Ok(()),
        Err(why) => Err(Unmade::Rejected { step, why }),
    }
}
