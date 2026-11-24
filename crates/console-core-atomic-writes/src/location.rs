//! A file name beneath one open directory, held for an entire transaction.
//!
//! Parent directories are opened without following symbolic or magic links.
//! Once opened, renames of their paths do not redirect operations on their
//! descriptors. File opens stay beneath that directory and refuse links too.
//! When systemd's RestrictSUIDSGID filter makes openat2 unavailable, the
//! fallback opens each component with O_NOFOLLOW and holds its directory
//! descriptor for the next.

use console_core_never::Never;
use rustix::fd::OwnedFd;
use rustix::fs::{AtFlags, Mode, OFlags, ResolveFlags, Stat};
use rustix::io::Errno;
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path};

const DIRECTORY: OFlags = OFlags::RDONLY
  .union(OFlags::DIRECTORY)
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
    let leaf = Path::new(name)
      .file_name()
      .filter(|leaf| *leaf == name)
      .ok_or(Errno::INVAL)?;

    rustix::fs::openat(
      &self.directory,
      leaf,
      flags | OFlags::CLOEXEC | OFlags::NOFOLLOW,
      mode,
    )
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

  match rustix::fs::openat2(
    rustix::fs::CWD,
    path,
    DIRECTORY | OFlags::CLOEXEC,
    Mode::empty(),
    NO_LINKS,
  ) {
    Ok(directory) => Ok(directory),
    Err(Errno::NOSYS) => walk_directory(path),
    Err(fault) => Err(fault),
  }
}

fn walk_directory(at: &Path) -> Result<OwnedFd, Errno> {
  let flags = DIRECTORY | OFlags::PATH | OFlags::CLOEXEC;
  let Ok(start) = starting_directory(at);
  let mut directory = rustix::fs::openat(rustix::fs::CWD, start, flags, Mode::empty())?;

  for component in at.components() {
    let name = match component {
      Component::RootDir | Component::CurDir => continue,
      Component::ParentDir => OsStr::new(".."),
      Component::Normal(name) => name,
      Component::Prefix(_prefix) => return Err(Errno::INVAL),
    };

    let next = rustix::fs::openat(&directory, name, flags, Mode::empty())?;
    directory = next;
  }

  rustix::fs::openat(&directory, ".", DIRECTORY | OFlags::CLOEXEC, Mode::empty())
}

fn starting_directory(at: &Path) -> Result<&Path, Never> {
  match at.is_absolute().then_some(()) {
    Some(()) => Ok(Path::new("/")),
    None => Ok(Path::new(".")),
  }
}

pub(super) fn make_directories(at: &Path) -> Result<OwnedFd, Errno> {
  let Ok(start) = starting_directory(at);
  let mut directory = open_directory(start)?;

  for component in at.components() {
    directory = match component {
      Component::RootDir | Component::CurDir => directory,
      Component::ParentDir => {
        let parent =
          rustix::fs::openat(&directory, "..", DIRECTORY | OFlags::CLOEXEC, Mode::empty())?;
        parent
      }
      Component::Normal(name) => {
        let child = descend(&directory, name)?;
        child
      }
      Component::Prefix(_prefix) => return Err(Errno::INVAL),
    };
  }

  Ok(directory)
}

fn descend(directory: &OwnedFd, name: &OsStr) -> Result<OwnedFd, Errno> {
  match rustix::fs::openat(directory, name, DIRECTORY | OFlags::CLOEXEC, Mode::empty()) {
    Ok(opened) => Ok(opened),
    Err(Errno::NOENT) => make_child(directory, name),
    Err(fault) => Err(fault),
  }
}

fn make_child(directory: &OwnedFd, name: &OsStr) -> Result<OwnedFd, Errno> {
  let open = || rustix::fs::openat(directory, name, DIRECTORY | OFlags::CLOEXEC, Mode::empty());

  match rustix::fs::mkdirat(directory, name, Mode::from_raw_mode(0o755)) {
    Ok(()) => {
      rustix::fs::fsync(directory)?;
      open()
    }
    Err(Errno::EXIST) => open(),
    Err(fault) => Err(fault),
  }
}
