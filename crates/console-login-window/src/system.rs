//! The C library calls greetd reached through `nix` and `libc`, declared here
//! against the C library std already links, and only the ones it made.
//!
//! Who a person is comes from `getpwnam_r`, so a name resolves the way every
//! other login on the machine resolves it, NSS and all. What a session is
//! started as is set between fork and exec, in the order greetd found it has
//! to be: a session of its own, the console as the controlling terminal, the
//! person's groups, then the group, then the user -- the user last, because
//! after it nothing else may be changed.

use std::ffi::{CStr, CString, OsStr, c_char, c_int, c_ulong};
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::foreign;

#[repr(C)]
struct Passwd {
  name: *mut c_char,
  password: *mut c_char,
  user: u32,
  group: u32,
  gecos: *mut c_char,
  home: *mut c_char,
  shell: *mut c_char,
}

#[cfg(target_pointer_width = "64")]
const _: () = {
  assert!(std::mem::size_of::<Passwd>() == 48);
  assert!(std::mem::align_of::<Passwd>() == 8);
  assert!(std::mem::offset_of!(Passwd, name) == 0);
  assert!(std::mem::offset_of!(Passwd, password) == 8);
  assert!(std::mem::offset_of!(Passwd, user) == 16);
  assert!(std::mem::offset_of!(Passwd, group) == 20);
  assert!(std::mem::offset_of!(Passwd, gecos) == 24);
  assert!(std::mem::offset_of!(Passwd, home) == 32);
  assert!(std::mem::offset_of!(Passwd, shell) == 40);
};

#[cfg(target_pointer_width = "32")]
const _: () = {
  assert!(std::mem::size_of::<Passwd>() == 28);
  assert!(std::mem::align_of::<Passwd>() == 4);
  assert!(std::mem::offset_of!(Passwd, name) == 0);
  assert!(std::mem::offset_of!(Passwd, password) == 4);
  assert!(std::mem::offset_of!(Passwd, user) == 8);
  assert!(std::mem::offset_of!(Passwd, group) == 12);
  assert!(std::mem::offset_of!(Passwd, gecos) == 16);
  assert!(std::mem::offset_of!(Passwd, home) == 20);
  assert!(std::mem::offset_of!(Passwd, shell) == 24);
};

unsafe extern "C" {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    allow(
      explicit051_no_machine_width,
      reason = "the buffer's length is C's `size_t`, which is the machine's width by the C ABI and not by choice here"
    )
  )]
  fn getpwnam_r(
    name: *const c_char,
    entry: *mut Passwd,
    buffer: *mut c_char,
    long: usize,
    found: *mut *mut Passwd,
  ) -> c_int;

  fn getgrouplist(name: *const c_char, group: u32, groups: *mut u32, count: *mut c_int) -> c_int;

  fn ioctl(descriptor: c_int, request: c_ulong, ...) -> c_int;
}

const KDSETMODE: c_ulong = 0x4B3A;
const KD_TEXT: c_ulong = 0x00;
const VT_ACTIVATE: c_ulong = 0x5606;
const VT_WAITACTIVE: c_ulong = 0x5607;

const KEEP_CURRENT_ID: u32 = u32::MAX;
const MAX_SUPPLEMENTARY_GROUPS: c_int = 65_536;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
  pub name: String,
  pub user: u32,
  pub group: u32,
  pub home: PathBuf,
  pub shell: PathBuf,
}

pub fn person(name: &str) -> io::Result<Option<Person>> {
  let asked = match CString::new(name) {
    Ok(asked) => asked,
    Err(_holds_a_nul) => return Ok(None),
  };
  let mut entry = Passwd {
    name: std::ptr::null_mut(),
    password: std::ptr::null_mut(),
    user: 0,
    group: 0,
    gecos: std::ptr::null_mut(),
    home: std::ptr::null_mut(),
    shell: std::ptr::null_mut(),
  };
  let mut buffer = vec![0_u8; 16_384];
  let mut found: *mut Passwd = std::ptr::null_mut();

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "NSS account resolution comes from getpwnam_r, which libcs expose through their C ABI"
    )
  )]
  // SAFETY: every pointer is to memory this frame owns for the whole call,
  // and the length handed in is the buffer's own.
  let code = unsafe {
    getpwnam_r(
      asked.as_ptr(),
      &mut entry,
      buffer.as_mut_ptr().cast::<c_char>(),
      buffer.len(),
      &mut found,
    )
  };

  match (code, found.is_null()) {
    (0, true) => Ok(None),
    (0, false) => {
      let home = account_path(&buffer, entry.home)?;
      let shell = account_path(&buffer, entry.shell)?;

      Ok(Some(Person {
        name: name.to_string(),
        user: entry.user,
        group: entry.group,
        home,
        shell,
      }))
    }
    (code_2, _) => Err(io::Error::from_raw_os_error(code_2)),
  }
}

fn account_path(buffer: &[u8], text: *const c_char) -> io::Result<PathBuf> {
  match ::core::primitive::bool::then_some(text.is_null(), ()) {
    ::core::option::Option::Some(()) => Ok(PathBuf::new()),
    ::core::option::Option::None => {
      let text_bytes = foreign::text_at(buffer, text)?;

      Ok(PathBuf::from(OsStr::from_bytes(text_bytes.to_bytes())))
    }
  }
}

fn checked(code: c_int) -> io::Result<()> {
  match code {
    -1 => Err(io::Error::last_os_error()),
    _ => Ok(()),
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terminal {
  Controlling,
  None,
}

fn assigned_id(raw: u32, invalid: io::ErrorKind) -> io::Result<u32> {
  match raw {
    KEEP_CURRENT_ID => Err(io::Error::from(invalid)),
    assigned => Ok(assigned),
  }
}

fn supplementary_groups(name: &CStr, primary: u32) -> io::Result<Vec<rustix::process::Gid>> {
  let mut groups = vec![0; 16];

  for _ in 0..4 {
    let Ok(count) = console_core_number_conversion::fitted::<_, c_int>(groups.len());
    let mut required = count;
    let output = groups.as_mut_ptr();

    #[cfg_attr(
      dylint_lib = "explicit058_contain_unsafe",
      expect(
        explicit058_contain_unsafe,
        reason = "getgrouplist resolves the named account's NSS group memberships through its C ABI"
      )
    )]
    // SAFETY: name is NUL-terminated and groups holds count initialized
    // entries for getgrouplist to fill.
    let found = unsafe { getgrouplist(name.as_ptr(), primary, output, &mut required) };
    let required_2 =
      console_core_number_conversion::checked_index(required).map_err(io::Error::other)?;

    match (
      found >= 0,
      required_2 > groups.len(),
      required <= MAX_SUPPLEMENTARY_GROUPS,
    ) {
      (true, false, _) => {
        groups.truncate(required_2);
        return groups
          .into_iter()
          .map(|group| {
            assigned_id(group, io::ErrorKind::InvalidData).map(rustix::process::Gid::from_raw)
          })
          .collect();
      }
      (false, true, true) => {
        #[cfg_attr(
          dylint_lib = "explicit088_no_arriving_length",
          expect(
            explicit088_no_arriving_length,
            reason = "the checked native index is at most MAX_SUPPLEMENTARY_GROUPS in this match arm"
          )
        )]
        let () = groups.resize(required_2, 0);
      }
      _ => return Err(io::Error::from(io::ErrorKind::InvalidData)),
    }
  }

  Err(io::Error::from(io::ErrorKind::InvalidData))
}

pub fn started_as(command: &mut Command, person: &Person, terminal: Terminal) -> io::Result<()> {
  use rustix::process::{Gid, Uid, ioctl_tiocsctty, setsid};
  use rustix::thread::{set_thread_gid, set_thread_groups, set_thread_uid};

  let name = CString::new(person.name.as_str()).map_err(io::Error::other)?;

  let assigned_user = assigned_id(person.user, io::ErrorKind::InvalidInput)?;
  let assigned_group = assigned_id(person.group, io::ErrorKind::InvalidInput)?;

  let groups = supplementary_groups(&name, assigned_group)?;
  let group = Gid::from_raw(assigned_group);
  let user = Uid::from_raw(assigned_user);

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the child must establish its session and drop groups, group and user in order between fork and exec"
    )
  )]
  // SAFETY: the child runs only kernel operations with data prepared in the parent; the command supplies stdin for a controlling terminal.
  let _pre_exec = unsafe {
    command.pre_exec(move || {
      let _session = setsid()?;

      match terminal {
        Terminal::Controlling => ioctl_tiocsctty(rustix::stdio::stdin())?,
        Terminal::None => {}
      }

      set_thread_groups(&groups)?;
      set_thread_gid(group)?;
      set_thread_uid(user)?;

      Ok(())
    })
  };

  Ok(())
}

pub fn console(at: &Path) -> io::Result<File> {
  use rustix::fs::OFlags;

  let (terminal, _metadata) = open_device(at, OFlags::RDWR | OFlags::NOCTTY)?;
  Ok(File::from(terminal))
}

pub(crate) fn null_input() -> io::Result<std::os::fd::OwnedFd> {
  use rustix::fs::{OFlags, major, minor};

  let (device, metadata) =
    open_device(Path::new("/dev/null"), OFlags::RDONLY | OFlags::NONBLOCK)?;

  match (major(metadata.st_rdev), minor(metadata.st_rdev)) {
    (1, 3) => Ok(device),
    _ => Err(io::Error::from(io::ErrorKind::InvalidInput)),
  }
}

fn open_device(
  at: &Path,
  access: rustix::fs::OFlags,
) -> io::Result<(std::os::fd::OwnedFd, rustix::fs::Stat)> {
  use rustix::fs::{CWD, FileType, Mode, OFlags, ResolveFlags, fstat, openat2};

  let relative = at.strip_prefix("/dev").map_err(io::Error::other)?;
  let no_links = ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS;
  let devices = openat2(
    CWD,
    "/dev",
    OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
    Mode::empty(),
    no_links,
  )?;
  let terminal = openat2(
    &devices,
    relative,
    access | OFlags::CLOEXEC,
    Mode::empty(),
    no_links | ResolveFlags::BENEATH,
  )?;
  let checked_value = fstat(&terminal)?;
  let mode = checked_value.st_mode;

  match ::core::primitive::bool::then_some(
    FileType::from_raw_mode(mode) == FileType::CharacterDevice,
    (),
  ) {
    ::core::option::Option::Some(()) => Ok((terminal, checked_value)),
    ::core::option::Option::None => Err(io::Error::from(io::ErrorKind::InvalidInput)),
  }
}

pub fn shown(console: &File, number: u32) -> io::Result<()> {
  terminal_request(console, TerminalRequest::TextMode)?;
  terminal_request(console, TerminalRequest::Activate(number))?;
  terminal_request(console, TerminalRequest::WaitActive(number))
}

enum TerminalRequest {
  TextMode,
  Activate(u32),
  WaitActive(u32),
}

fn terminal_request(console: &File, request: TerminalRequest) -> io::Result<()> {
  let (command, value) = match request {
    TerminalRequest::TextMode => (KDSETMODE, KD_TEXT),
    TerminalRequest::Activate(number) => (VT_ACTIVATE, c_ulong::from(number)),
    TerminalRequest::WaitActive(number) => (VT_WAITACTIVE, c_ulong::from(number)),
  };

  #[cfg_attr(
    dylint_lib = "explicit090_no_raw_descriptor",
    expect(
      explicit090_no_raw_descriptor,
      reason = "the C virtual-terminal ioctl ABI takes the descriptor number while the borrowed File keeps it live"
    )
  )]
  let descriptor = console.as_raw_fd();

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the kernel validates terminal requests and their values, and libc exposes them through its variadic ioctl"
    )
  )]
  // SAFETY: console retains the open descriptor, and every request above
  // takes an integer value rather than a pointer.
  let result = unsafe { ioctl(descriptor, command, value) };

  checked(result)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn account_paths_preserve_the_bytes_that_nss_returned() -> io::Result<()> {
    let bytes = b"/home/name\xff\0";
    let path = account_path(bytes, bytes.as_ptr().cast())?;

    assert_eq!(path.as_os_str().as_bytes(), b"/home/name\xff");

    Ok(())
  }
}
