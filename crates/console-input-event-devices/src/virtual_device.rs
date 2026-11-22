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

use std::ffi::{OsStr, c_char};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::fd::AsFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use console_core_number_conversion::index;
use rustix::fs::{Mode, OFlags};

use crate::codes::{AbsoluteAxisCode, EventType, MiscCode, PropType, RelativeAxisCode};
use crate::device::{INPUT, NODE};
use crate::event::InputEvent;
use crate::kernel::{self, AbsInfo, AbsoluteSetup, InputId, UinputCode};
use crate::keys::KeyCode;

const UINPUT: &str = "/dev/uinput";

const VIRTUAL: &str = "/sys/devices/virtual/input";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Setup {
  pub name: String,
  pub id: InputId,
  pub physical_path: Option<String>,
  pub keys: Vec<KeyCode>,
  pub relative_axes: Vec<RelativeAxisCode>,
  pub absolute_axes: Vec<(AbsoluteAxisCode, AbsInfo)>,
  pub miscellaneous: Vec<MiscCode>,
  pub properties: Vec<PropType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
  Kinds,
  Keys,
  RelativeAxes,
  AbsoluteAxes,
  Miscellaneous,
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
      Step::Miscellaneous => "its miscellaneous codes",
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
    device.set_name(setup)?;
    check_step(Step::Creating, kernel::create(device.file.as_fd()))?;

    Ok(device)
  }

  fn kinds(&self, setup: &Setup) -> Result<(), Unmade> {
    let sent = [
      (EventType::KEY, setup.keys.is_empty()),
      (EventType::RELATIVE, setup.relative_axes.is_empty()),
      (EventType::ABSOLUTE, setup.absolute_axes.is_empty()),
      (EventType::MISC, setup.miscellaneous.is_empty()),
    ];

    for (kind, none) in sent {
      match ::core::primitive::bool::then_some(none, ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => {
          self.set_code(Step::Kinds, UinputCode::EventType, kind.0)?
        }
      }
    }

    Ok(())
  }

  fn codes(&self, setup: &Setup) -> Result<(), Unmade> {
    let every = [
      (
        Step::Keys,
        UinputCode::Key,
        setup.keys.iter().map(|key| key.0).collect::<Vec<u16>>(),
      ),
      (
        Step::RelativeAxes,
        UinputCode::RelativeAxis,
        setup.relative_axes.iter().map(|axis| axis.0).collect(),
      ),
      (
        Step::Miscellaneous,
        UinputCode::Miscellaneous,
        setup
          .miscellaneous
          .iter()
          .map(|miscellaneous| miscellaneous.0)
          .collect(),
      ),
      (
        Step::Properties,
        UinputCode::Property,
        setup.properties.iter().map(|property| property.0).collect(),
      ),
    ];

    for (step, request, codes) in every {
      for code in codes {
        self.set_code(step, request, code)?;
      }
    }

    Ok(())
  }

  fn axes(&self, axes: &[(AbsoluteAxisCode, AbsInfo)]) -> Result<(), Unmade> {
    for (axis, information) in axes {
      let setup = AbsoluteSetup {
        code: axis.0,
        information: *information,
      };

      self.set_code(Step::AbsoluteAxes, UinputCode::AbsoluteAxis, axis.0)?;
      let told = kernel::absolute_setup(self.file.as_fd(), &setup);
      check_step(Step::AbsoluteAxes, told)?;
    }

    Ok(())
  }

  fn plugged(&self, physical_path: Option<&str>) -> Result<(), Unmade> {
    let path = match physical_path {
      Some(path) => path,
      None => return Ok(()),
    };
    let terminated = match std::ffi::CString::new(path) {
      Ok(terminated) => terminated,
      Err(_embedded_nul) => return Err(Unmade::NulInPhysicalPath),
    };
    let told = kernel::physical_path(self.file.as_fd(), &terminated);

    check_step(Step::PhysicalPath, told)
  }

  fn set_name(&self, setup: &Setup) -> Result<(), Unmade> {
    let mut name: [c_char; 80] = [0; 80];
    let written = setup.name.as_bytes();
    let Ok(room) = index(kernel::NAME);

    match ::core::primitive::bool::then_some(written.len() < room, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(Unmade::NameTooLong),
    }

    for (into, byte) in name.iter_mut().zip(written) {
      *into = c_char::from_le_bytes([*byte]);
    }

    let described = kernel::UinputSetup {
      id: setup.id,
      name,
      effects: 0,
    };

    let told = kernel::setup(self.file.as_fd(), &described);

    check_step(Step::Describing, told)
  }

  fn set_code(&self, step: Step, request: UinputCode, argument: u16) -> Result<(), Unmade> {
    let told = kernel::code(self.file.as_fd(), request, argument);

    check_step(step, told)
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
    let buffer = kernel::system_name(self.file.as_fd())?;
    let named = match buffer.split(|byte| *byte == 0).next() {
      Some(named) => ChildName::new(OsStr::from_bytes(named))?,
      None => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "uinput returned no system name",
        ));
      }
    };
    let directory = Path::new(VIRTUAL).join(named);
    let entries = fs::read_dir(directory)?;
    let mut nodes = Vec::new();

    for entry in entries.flatten() {
      let name = entry.file_name();

      match name.as_encoded_bytes().starts_with(NODE).then_some(()) {
        ::core::option::Option::Some(()) => {
          let child = ChildName::new(&name)?;
          nodes.push(Path::new(INPUT).join(child));
        }
        ::core::option::Option::None => {}
      }
    }

    nodes.sort();

    Ok(nodes)
  }
}

struct ChildName<'a>(&'a OsStr);

impl<'a> ChildName<'a> {
  fn new(name: &'a OsStr) -> io::Result<Self> {
    match Path::new(name)
      .file_name()
      .filter(|component| *component == name)
    {
      Some(component) => Ok(Self(component)),
      None => Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "uinput device name must be one path component",
      )),
    }
  }
}

impl AsRef<Path> for ChildName<'_> {
  fn as_ref(&self) -> &Path {
    Path::new(self.0)
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
  match rustix::fs::open(
    UINPUT,
    OFlags::RDWR | OFlags::NONBLOCK | OFlags::CLOEXEC,
    Mode::empty(),
  ) {
    Ok(file) => Ok(File::from(file)),
    Err(why) => Err(Unmade::NoUinput(io::Error::from(why))),
  }
}

fn check_step(step: Step, told: io::Result<()>) -> Result<(), Unmade> {
  told.map_err(|why| Unmade::Rejected { step, why })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn uinput_nodes_stay_under_their_parent_and_keep_native_names() -> io::Result<()> {
    for bad in [
      "",
      ".",
      "..",
      "/event1",
      "../event1",
      "event1/",
      "input1/event1",
    ] {
      match ChildName::new(OsStr::new(bad)) {
        Ok(_accepted) => return Err(io::Error::other(format!("accepted child name {bad:?}"))),
        Err(rejected) => assert_eq!(rejected.kind(), io::ErrorKind::InvalidData),
      }
    }

    let name = OsStr::from_bytes(b"event\xff");
    let valid = ChildName::new(name)?;
    assert_eq!(valid.as_ref().as_os_str(), name);

    Ok(())
  }
}
