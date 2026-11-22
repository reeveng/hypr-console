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
//! An undecodable name or path is also unavailable to the textual API.
//!
//! A node that will not open is not in the list of every node, whatever the
//! reason: most of `/dev/input` belongs to root, and a program that can only
//! read the pad is not told about the lid switch. A reader follows the
//! descriptor’s mode. Polling callers set `nonblocking` before reading.

use std::fs::{self, File};
use std::io::{self, Read};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd};
use std::path::{Path, PathBuf};

use console_core_never::Never;

use crate::codes::{AbsoluteAxisCode, ForceFeedbackCode, MiscCode, PropType, RelativeAxisCode};
use crate::event::{EMPTY_BATCH, InputEvent, events};
use crate::kernel::{self, AbsInfo, Capability, Description, Grab, InputId};
use crate::keys::KeyCode;

pub const INPUT: &str = "/dev/input";

pub(crate) const NODE: &[u8] = b"event";

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
    let id = kernel::identity(file.as_fd())?;
    let keys = bits(&file, Capability::Keys)?;
    let relative_axes = bits(&file, Capability::RelativeAxes)?;
    let absolute_axes = bits(&file, Capability::AbsoluteAxes)?;
    let miscellaneous = bits(&file, Capability::Miscellaneous)?;
    let force_feedback = bits(&file, Capability::ForceFeedback)?;
    let properties = bits(&file, Capability::Properties)?;
    let Ok(name) = text(&file, Description::Name);
    let Ok(physical_path) = text(&file, Description::PhysicalPath);

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
      .filter(|entry| entry.file_name().as_encoded_bytes().starts_with(NODE))
      .map(|entry| entry.path())
      .collect();

    paths.sort();

    Ok(
      paths
        .iter()
        .filter_map(|path| match Device::open(path) {
          Ok(device) => Some(device),
          Err(_would_not_open) => None,
        })
        .collect(),
    )
  }

  pub fn absolute(&self) -> io::Result<Vec<(AbsoluteAxisCode, AbsInfo)>> {
    let mut every = Vec::new();

    for axis in &self.absolute_axes {
      let information = kernel::absolute(self.file.as_fd(), *axis)?;
      every.push((*axis, information));
    }

    Ok(every)
  }

  pub fn grab(&self) -> io::Result<()> {
    kernel::grab(self.file.as_fd(), Grab::Take)
  }

  pub fn ungrab(&self) -> io::Result<()> {
    kernel::grab(self.file.as_fd(), Grab::Release)
  }

  pub fn nonblocking(&self) -> io::Result<()> {
    let flags = rustix::fs::fcntl_getfl(&self.file)?;
    rustix::fs::fcntl_setfl(&self.file, flags | rustix::fs::OFlags::NONBLOCK)?;

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "this public reader follows its descriptor's mode, and polling callers set O_NONBLOCK through Device::nonblocking before reading"
    )
  )]
  pub fn read_events(&mut self) -> io::Result<Vec<InputEvent>> {
    let mut buffer = EMPTY_BATCH;
    let reading = self
      .file
      .read(&mut buffer)
      .map(|long| buffer.get(..long).map(events));
    let read = reading?;

    match read {
      Some(Ok(read_2)) => Ok(read_2),
      None => Ok(Vec::new()),
    }
  }
}

impl AsRawFd for Device {
  #[cfg_attr(
    dylint_lib = "explicit090_no_raw_descriptor",
    expect(
      explicit090_no_raw_descriptor,
      reason = "the public AsRawFd implementation lends the number required by legacy callers while Device keeps ownership"
    )
  )]
  fn as_raw_fd(&self) -> RawFd {
    self.file.as_raw_fd()
  }
}

impl AsFd for Device {
  fn as_fd(&self) -> BorrowedFd<'_> {
    self.file.as_fd()
  }
}

fn text(file: &File, field: Description) -> Result<Option<String>, Never> {
  let bytes = match kernel::text(file.as_fd(), field) {
    Ok(bytes) => bytes,
    Err(_description_unavailable) => return Ok(None),
  };
  let said = match bytes.split(|byte| *byte == 0).next() {
    Some([]) | None => return Ok(None),
    Some(said) => said,
  };

  Ok(match std::str::from_utf8(said) {
    Ok(text) => Some(text.to_owned()),
    Err(_description_is_not_utf8) => None,
  })
}

fn bits(file: &File, what: Capability) -> io::Result<Vec<u16>> {
  let buffer = kernel::bits(file.as_fd(), what)?;

  let Ok(set) = set_in(&buffer);

  Ok(set)
}

fn set_in(bytes: &[u8]) -> Result<Vec<u16>, Never> {
  Ok(
    (0_u16..)
      .zip(bytes)
      .flat_map(|(at, byte)| {
        (0..BYTE).filter_map(move |bit| match byte & 1_u8.wrapping_shl(u32::from(bit)) {
          0 => None,
          _ => Some(at.saturating_mul(BYTE).saturating_add(bit)),
        })
      })
      .collect(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;

  #[test]
  fn a_bit_is_the_code_it_stands_at() {
    let Ok(set) = set_in(&[0b0000_0101, 0, 0b1000_0000]);

    assert_eq!(set, vec![0, 2, 23]);
  }

  #[test]
  fn the_pad_s_first_button_is_found_where_the_kernel_puts_it() -> Result<(), Box<dyn Error>> {
    let mut bytes = vec![0_u8; 96];

    let Ok(at) = console_core_number_conversion::index(KeyCode::BTN_SOUTH.0.div_euclid(8));

    match bytes.get_mut(at) {
      Some(byte) => *byte = 1,
      None => {
        return Err(Box::from(
          "the pad's first button is past the last of the bits",
        ));
      }
    }

    let Ok(set) = set_in(&bytes);

    assert_eq!(set, vec![KeyCode::BTN_SOUTH.0]);

    Ok(())
  }
}
