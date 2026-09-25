//! One event node, opened, and what it says about itself.
//!
//! What a device is -- its name, where it is plugged in, who made it and what
//! it can send -- is asked once, when it is opened, because none of it changes
//! while the node is held and every caller wants it before anything else. What
//! does change is where each axis is, so that is asked when it is wanted.
//!
//! A name or a physical path the kernel holds as an empty string is no name
//! or path at all, which is what a device made through uinput without one
//! answers, and is the same answer as one it never set.
//!
//! A node that will not open is not in the list of every node, whatever the
//! reason: most of `/dev/input` belongs to root, and a program that can only
//! read the pad is not told about the lid switch.

use std::ffi::c_int;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd};
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_number_conversion::index;

use crate::codes::{AbsoluteAxisCode, ForceFeedbackCode, MiscCode, PropType, RelativeAxisCode};
use crate::event::{InputEvent, LONG, events};
use crate::kernel::{self, AbsInfo, InputId, checked, ioctl};
use crate::keys::KeyCode;

pub const INPUT: &str = "/dev/input";

const NODE: &str = "event";

const AT_ONCE: u32 = 64;

const BYTE: u16 = 8;

#[derive(Debug)]
pub struct Device {
    pub path: PathBuf,
    pub name: Option<String>,
    pub physical_path: Option<String>,
    pub id: InputId,
    pub keys: Vec<KeyCode>,
    pub relative_axes: Vec<RelativeAxisCode>,
    pub absolute_axes: Vec<AbsoluteAxisCode>,
    pub miscellaneous: Vec<MiscCode>,
    pub force_feedback: Vec<ForceFeedbackCode>,
    pub properties: Vec<PropType>,
    file: File,
}

impl Device {
    pub fn open(path: &Path) -> io::Result<Device> {
        let file = File::open(path)?;
        let id = identity(&file)?;
        let keys = bits(&file, kernel::GET_KEYS)?;
        let relative_axes = bits(&file, kernel::GET_RELATIVE_AXES)?;
        let absolute_axes = bits(&file, kernel::GET_ABSOLUTE_AXES)?;
        let miscellaneous = bits(&file, kernel::GET_MISC)?;
        let force_feedback = bits(&file, kernel::GET_FORCE_FEEDBACK)?;
        let properties = bits(&file, kernel::GET_PROPERTIES)?;
        let Ok(name) = text(&file, kernel::GET_NAME);
        let Ok(physical_path) = text(&file, kernel::GET_PHYSICAL_PATH);

        Ok(Device {
            path: path.to_path_buf(),
            name,
            physical_path,
            id,
            keys: keys.into_iter().map(KeyCode).collect(),
            relative_axes: relative_axes.into_iter().map(RelativeAxisCode).collect(),
            absolute_axes: absolute_axes.into_iter().map(AbsoluteAxisCode).collect(),
            miscellaneous: miscellaneous.into_iter().map(MiscCode).collect(),
            force_feedback: force_feedback.into_iter().map(ForceFeedbackCode).collect(),
            properties: properties.into_iter().map(PropType).collect(),
            file,
        })
    }

    pub fn every() -> Result<Vec<Device>, Never> {
        let entries = match fs::read_dir(INPUT) {
            Ok(entries) => entries,
            Err(_no_input_devices) => return Ok(Vec::new()),
        };
        let mut paths: Vec<PathBuf> = entries
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(NODE))
            .map(|entry| entry.path())
            .collect();

        paths.sort();

        Ok(paths
            .iter()
            .filter_map(|path| match Device::open(path) {
                Ok(device) => Some(device),
                Err(_would_not_open) => None,
            })
            .collect())
    }

    pub fn absolute(&self) -> io::Result<Vec<(AbsoluteAxisCode, AbsInfo)>> {
        let mut every = Vec::new();

        for axis in &self.absolute_axes {
            let mut information = AbsInfo::default();
            let request = kernel::GET_ABSOLUTE | std::ffi::c_ulong::from(axis.0);

            // SAFETY: an open file, and a struct the size the request
            // number says, which the kernel writes into and nothing else holds.
            let asked = unsafe { ioctl(self.file.as_raw_fd(), request, &raw mut information) };

            checked(asked)?;
            every.push((*axis, information));
        }

        Ok(every)
    }

    pub fn grab(&self) -> io::Result<()> {
        held(&self.file, 1)
    }

    pub fn ungrab(&self) -> io::Result<()> {
        held(&self.file, 0)
    }

    pub fn nonblocking(&self) -> io::Result<()> {
        let open = self.file.as_raw_fd();

        // SAFETY: an open file, and a command that takes no argument.
        let flags = unsafe { kernel::fcntl(open, kernel::GET_FLAGS) };
        let flags = checked(flags)?;

        // SAFETY: an open file, and the flags it already had with one more.
        let set = unsafe { kernel::fcntl(open, kernel::SET_FLAGS, flags | kernel::NOT_BLOCKING) };

        checked(set)?;

        Ok(())
    }

    pub fn read_events(&mut self) -> io::Result<Vec<InputEvent>> {
        let Ok(room) = index(LONG.saturating_mul(AT_ONCE));
        let mut buffer = vec![0_u8; room];
        let reading = self.file.read(&mut buffer).map(|long| buffer.get(..long).map(events));
        let read = reading?;

        match read {
            Some(Ok(read)) => Ok(read),
            None => Ok(Vec::new()),
        }
    }
}

impl AsRawFd for Device {
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

impl AsFd for Device {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }
}

fn held(file: &File, grabbed: c_int) -> io::Result<()> {
    // SAFETY: an open file, and a request that takes an int by value.
    let asked = unsafe { ioctl(file.as_raw_fd(), kernel::GRAB, grabbed) };

    checked(asked)?;

    Ok(())
}

fn identity(file: &File) -> io::Result<InputId> {
    let mut id = InputId::default();

    // SAFETY: an open file, and a struct the size the request number
    // says, which the kernel writes into and nothing else holds.
    let asked = unsafe { ioctl(file.as_raw_fd(), kernel::GET_ID, &raw mut id) };

    checked(asked)?;

    Ok(id)
}

fn text(file: &File, request: std::ffi::c_ulong) -> Result<Option<String>, Never> {
    let Ok(room) = index(kernel::TEXT);
    let mut buffer = vec![0_u8; room];

    // SAFETY: an open file, and a buffer the length the request number
    // says, which the kernel writes into and nothing else holds.
    let asked = unsafe { ioctl(file.as_raw_fd(), request, buffer.as_mut_ptr()) };

    Ok(match checked(asked) {
        Ok(_) => buffer
            .split(|byte| *byte == 0)
            .next()
            .filter(|said| !said.is_empty())
            .map(|said| String::from_utf8_lossy(said).into_owned()),
        Err(_the_kernel_would_not_say) => None,
    })
}

fn bits(file: &File, request: std::ffi::c_ulong) -> io::Result<Vec<u16>> {
    let Ok(room) = index(kernel::BITS);
    let mut buffer = vec![0_u8; room];

    // SAFETY: an open file, and a buffer the length the request number
    // says, which the kernel writes into and nothing else holds.
    let asked = unsafe { ioctl(file.as_raw_fd(), request, buffer.as_mut_ptr()) };

    checked(asked)?;

    let Ok(set) = set_in(&buffer);

    Ok(set)
}

fn set_in(bytes: &[u8]) -> Result<Vec<u16>, Never> {
    Ok((0_u16..)
        .zip(bytes)
        .flat_map(|(at, byte)| {
            (0..BYTE).filter_map(move |bit| match byte & 1_u8.wrapping_shl(u32::from(bit)) {
                0 => None,
                _ => Some(at.saturating_mul(BYTE).saturating_add(bit)),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bit_is_the_code_it_stands_at() {
        let Ok(set) = set_in(&[0b0000_0101, 0, 0b1000_0000]);

        assert_eq!(set, vec![0, 2, 23]);
    }

    #[test]
    fn the_pad_s_first_button_is_found_where_the_kernel_puts_it() {
        let mut bytes = vec![0_u8; 96];

        bytes[0x130 / 8] = 1;
        let Ok(set) = set_in(&bytes);

        assert_eq!(set, vec![KeyCode::BTN_SOUTH.0]);
    }
}
