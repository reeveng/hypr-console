//! A file name beneath one open directory, held for an entire transaction.
//!
//! Parent directories are opened without following symbolic or magic links.
//! Once opened, renames of their paths do not redirect operations on their
//! descriptors. File opens stay beneath that directory and refuse links too.

use rustix::fd::OwnedFd;
use rustix::fs::{AtFlags, Mode, OFlags, ResolveFlags, Stat};
use rustix::io::Errno;
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path};

const DIRECTORY: OFlags = OFlags::RDONLY
  .union(OFlags::DIRECTORY)
  .union(OFlags::CLOEXEC)
  .union(OFlags::NOFOLLOW);
const NO_LINKS: ResolveFlags = ResolveFlags::NO_SYMLINKS.union(ResolveFlags::NO_MAGICLINKS);

pub(super) struct Location {
  pub(super) directory: OwnedFd,
  pub(super) name: OsString,
}

impl Location {
  pub(super) fn new(at: &Path) -> Result<Self, Errno> {
    let filename = at.file_name().ok_or(Errno::INVAL)?;
    let name = filename.to_os_string();
    let parent = match at.parent() {
      Some(parent) => parent,
      None => Path::new("."),
    };
    let directory = open_directory(parent)?;

    Ok(Self { directory, name })
  }

  pub(super) fn open(&self, flags: OFlags, mode: Mode) -> Result<OwnedFd, Errno> {
    self.open_name(&self.name, flags, mode)
  }

  pub(super) fn open_name(
    &self,
    name: &OsStr,
    flags: OFlags,
    mode: Mode,
  ) -> Result<OwnedFd, Errno> {
    match Path::new(name).file_name() {
      Some(leaf) if leaf == name => rustix::fs::openat(
        &self.directory,
        leaf,
        flags | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        mode,
      ),
      _ => Err(Errno::INVAL),
    }
  }

  pub(super) fn metadata(&self) -> Result<Stat, Errno> {
    rustix::fs::statat(&self.directory, &self.name, AtFlags::SYMLINK_NOFOLLOW)
  }

  pub(super) fn remove(&self, name: &std::ffi::OsStr) -> Result<(), Errno> {
    rustix::fs::unlinkat(&self.directory, name, AtFlags::empty())
  }
}

pub(super) fn open_directory(at: &Path) -> Result<OwnedFd, Errno> {
  let path = match ::core::primitive::bool::then_some(at.as_os_str().is_empty(), ()) {
    ::core::option::Option::Some(()) => Path::new("."),
    ::core::option::Option::None => at,
  };

  match rustix::fs::openat2(rustix::fs::CWD, path, DIRECTORY, Mode::empty(), NO_LINKS) {
    Err(Errno::NOSYS) => walk_directory(path),
    opened => opened,
  }
}

// systemd's RestrictSUIDSGID filter can make openat2 unavailable. Open each
// component with O_NOFOLLOW and hold its directory descriptor for the next.
fn walk_directory(at: &Path) -> Result<OwnedFd, Errno> {
  let flags = DIRECTORY | OFlags::PATH;
  let mut directory = rustix::fs::openat(
    rustix::fs::CWD,
    starting_directory(at),
    flags,
    Mode::empty(),
  )?;

  for component in at.components() {
    let name = match component {
      Component::RootDir | Component::CurDir => continue,
      Component::ParentDir => OsStr::new(".."),
      Component::Normal(name) => name,
      Component::Prefix(_prefix) => return Err(Errno::INVAL),
    };

    directory = rustix::fs::openat(&directory, name, flags, Mode::empty())?;
  }

  rustix::fs::openat(&directory, ".", DIRECTORY, Mode::empty())
}

fn starting_directory(at: &Path) -> &Path {
  match ::core::primitive::bool::then_some(at.is_absolute(), ()) {
    ::core::option::Option::Some(()) => Path::new("/"),
    ::core::option::Option::None => Path::new("."),
  }
}

pub(super) fn make_directories(at: &Path) -> Result<OwnedFd, Errno> {
  let mut directory = open_directory(starting_directory(at))?;

  for component in at.components() {
    directory = match component {
      Component::RootDir | Component::CurDir => directory,
      Component::ParentDir => rustix::fs::openat(&directory, "..", DIRECTORY, Mode::empty())?,
      Component::Normal(name) => descend(&directory, name)?,
      Component::Prefix(_prefix) => return Err(Errno::INVAL),
    };
  }

  Ok(directory)
}

fn descend(directory: &OwnedFd, name: &OsStr) -> Result<OwnedFd, Errno> {
  match rustix::fs::openat(directory, name, DIRECTORY, Mode::empty()) {
    Ok(opened) => return Ok(opened),
    Err(fault) => match ::core::primitive::bool::then_some(fault == Errno::NOENT, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(fault),
    },
  }

  match rustix::fs::mkdirat(directory, name, Mode::from_raw_mode(0o755)) {
    Ok(()) => rustix::fs::fsync(directory)?,
    Err(fault) => match ::core::primitive::bool::then_some(fault == Errno::EXIST, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(fault),
    },
  }

  rustix::fs::openat(directory, name, DIRECTORY, Mode::empty())
}
