//! The typed evdev and uinput calls, request numbers, and structures from
//! Linux's `input.h` and `uinput.h`. Only this module speaks the ioctl ABI.

use std::ffi::{CStr, c_char, c_void};
use std::io;
use std::os::fd::BorrowedFd;
use std::ptr;

use console_core_number_conversion::index;
use rustix::ioctl::{Ioctl, IoctlOutput, Opcode, ioctl};

use crate::codes::{AbsoluteAxisCode, BusType};

pub(crate) const NAME: u32 = 80;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputId {
  pub bus: BusType,
  pub vendor: u16,
  pub product: u16,
  pub version: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AbsInfo {
  pub value: i32,
  pub minimum: i32,
  pub maximum: i32,
  pub fuzz: i32,
  pub flat: i32,
  pub resolution: i32,
}

#[repr(C)]
pub(crate) struct UinputSetup {
  pub id: InputId,
  pub name: [c_char; 80],
  pub effects: u32,
}

#[repr(C)]
pub(crate) struct AbsoluteSetup {
  pub code: u16,
  pub information: AbsInfo,
}

pub(crate) enum Description {
  Name,
  PhysicalPath,
}

pub(crate) enum Capability {
  Keys,
  RelativeAxes,
  AbsoluteAxes,
  Miscellaneous,
  ForceFeedback,
  Properties,
}

#[derive(Clone, Copy)]
pub(crate) enum UinputCode {
  EventType,
  Key,
  RelativeAxis,
  AbsoluteAxis,
  Miscellaneous,
  Property,
}

pub(crate) enum Grab {
  Take,
  Release,
}

struct Axis(u16);

enum Call<'a> {
  Identity(&'a mut InputId),
  Text(Description, &'a mut [u8; 256]),
  Bits(Capability, &'a mut [u8; 96]),
  Absolute(Axis, &'a mut AbsInfo),
  Grab(Grab),
  Create,
  Setup(&'a UinputSetup),
  AbsoluteSetup(&'a AbsoluteSetup),
  Code(UinputCode, u16),
  PhysicalPath(&'a CStr),
  SystemName(&'a mut [u8; 64]),
}

impl Call<'_> {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "each private Call variant pairs a Linux evdev or uinput request with its required argument and preserves the borrowed buffer until the syscall finishes"
    )
  )]
  fn run(self, file: BorrowedFd<'_>) -> io::Result<()> {
    // SAFETY: Call stores the lifetime and argument type for each request.
    // The kernel checks that the descriptor supports the request.
    unsafe { ioctl(file, self) }.map_err(io::Error::from)
  }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the private evdev and uinput request variants bind each ABI opcode to the matching live buffer or immediate argument"
  )
)]
// SAFETY: The request numbers and the argument layouts match the Linux
// input and uinput headers. Write requests borrow initialized buffers,
// and read requests borrow exclusive buffers of the size in the opcode.
unsafe impl Ioctl for Call<'_> {
  type Output = ();

  const IS_MUTATING: bool = true;

  fn opcode(&self) -> Opcode {
    match self {
      Self::Identity(_) => 0x8008_4502,
      Self::Text(Description::Name, _) => 0x8100_4506,
      Self::Text(Description::PhysicalPath, _) => 0x8100_4507,
      Self::Bits(Capability::Keys, _) => 0x8060_4521,
      Self::Bits(Capability::RelativeAxes, _) => 0x8060_4522,
      Self::Bits(Capability::AbsoluteAxes, _) => 0x8060_4523,
      Self::Bits(Capability::Miscellaneous, _) => 0x8060_4524,
      Self::Bits(Capability::ForceFeedback, _) => 0x8060_4535,
      Self::Bits(Capability::Properties, _) => 0x8060_4509,
      Self::Absolute(Axis(axis), _) => 0x8018_4540 | Opcode::from(*axis),
      Self::Grab(_) => 0x4004_4590,
      Self::Create => 0x5501,
      Self::Setup(_) => 0x405c_5503,
      Self::AbsoluteSetup(_) => 0x401c_5504,
      Self::Code(UinputCode::EventType, _) => 0x4004_5564,
      Self::Code(UinputCode::Key, _) => 0x4004_5565,
      Self::Code(UinputCode::RelativeAxis, _) => 0x4004_5566,
      Self::Code(UinputCode::AbsoluteAxis, _) => 0x4004_5567,
      Self::Code(UinputCode::Miscellaneous, _) => 0x4004_5568,
      Self::Code(UinputCode::Property, _) => 0x4004_556e,
      Self::PhysicalPath(_) => 0x4008_556c,
      Self::SystemName(_) => 0x8040_552c,
    }
  }

  fn as_ptr(&mut self) -> *mut c_void {
    match self {
      Self::Identity(output) => ptr::from_mut(*output).cast(),
      Self::Text(_, output) => output.as_mut_ptr().cast(),
      Self::Bits(_, output) => output.as_mut_ptr().cast(),
      Self::Absolute(_, output) => ptr::from_mut(*output).cast(),
      Self::Grab(Grab::Take) => ptr::without_provenance_mut(1),
      Self::Grab(Grab::Release) | Self::Create => ptr::null_mut(),
      Self::Setup(setup) => ptr::from_ref(*setup).cast_mut().cast(),
      Self::AbsoluteSetup(setup) => ptr::from_ref(*setup).cast_mut().cast(),
      Self::Code(_, code) => {
        let Ok(argument) = index(*code);

        ptr::without_provenance_mut(argument)
      }
      Self::PhysicalPath(path) => path.as_ptr().cast_mut().cast(),
      Self::SystemName(output) => output.as_mut_ptr().cast(),
    }
  }

  unsafe fn output_from_ptr(
    _output: IoctlOutput,
    _argument: *mut c_void,
  ) -> rustix::io::Result<()> {
    Ok(())
  }
}

pub(crate) fn identity(file: BorrowedFd<'_>) -> io::Result<InputId> {
  let mut id = InputId::default();
  Call::Identity(&mut id).run(file)?;

  Ok(id)
}

pub(crate) fn text(file: BorrowedFd<'_>, field: Description) -> io::Result<[u8; 256]> {
  let mut bytes = [0; 256];
  Call::Text(field, &mut bytes).run(file)?;

  Ok(bytes)
}

pub(crate) fn bits(file: BorrowedFd<'_>, what: Capability) -> io::Result<[u8; 96]> {
  let mut bytes = [0; 96];
  Call::Bits(what, &mut bytes).run(file)?;

  Ok(bytes)
}

pub(crate) fn absolute(file: BorrowedFd<'_>, axis: AbsoluteAxisCode) -> io::Result<AbsInfo> {
  let requested_axis = match axis.0 {
    code @ 0..=0x3f => Axis(code),
    unknown => {
      return Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("absolute axis {unknown} is outside the kernel's axis range"),
      ));
    }
  };
  let mut information = AbsInfo::default();
  Call::Absolute(requested_axis, &mut information).run(file)?;

  Ok(information)
}

pub(crate) fn grab(file: BorrowedFd<'_>, grabbed: Grab) -> io::Result<()> {
  Call::Grab(grabbed).run(file)
}

pub(crate) fn create(file: BorrowedFd<'_>) -> io::Result<()> {
  Call::Create.run(file)
}

pub(crate) fn setup(file: BorrowedFd<'_>, described: &UinputSetup) -> io::Result<()> {
  Call::Setup(described).run(file)
}

pub(crate) fn absolute_setup(file: BorrowedFd<'_>, setup: &AbsoluteSetup) -> io::Result<()> {
  Call::AbsoluteSetup(setup).run(file)
}

pub(crate) fn code(file: BorrowedFd<'_>, what: UinputCode, number: u16) -> io::Result<()> {
  Call::Code(what, number).run(file)
}

pub(crate) fn physical_path(file: BorrowedFd<'_>, path: &CStr) -> io::Result<()> {
  Call::PhysicalPath(path).run(file)
}

pub(crate) fn system_name(file: BorrowedFd<'_>) -> io::Result<[u8; 64]> {
  let mut bytes = [0; 64];
  Call::SystemName(&mut bytes).run(file)?;

  Ok(bytes)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ioctl_structs_match_the_linux_input_abi() {
    assert_eq!(std::mem::size_of::<InputId>(), 8);
    assert_eq!(std::mem::size_of::<AbsInfo>(), 24);
    assert_eq!(std::mem::size_of::<AbsoluteSetup>(), 28);
    assert_eq!(std::mem::offset_of!(AbsoluteSetup, information), 4);
    assert_eq!(std::mem::size_of::<UinputSetup>(), 92);
    assert_eq!(std::mem::offset_of!(UinputSetup, name), 8);
    assert_eq!(std::mem::offset_of!(UinputSetup, effects), 88);
  }
}
