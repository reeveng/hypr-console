use std::io;
use std::path::PathBuf;

use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use rustix::fs::{FileType, Mode, OFlags, ResolveFlags, fstat, open, openat2};

pub const PAM_DIRECTORY: &str = "/etc/console-auth";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
  Available,
  Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verification {
  Pending,
  Accepted,
  Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PamService {
  Fingerprint,
  Password,
  LongPassword,
}

impl PamService {
  pub(crate) fn name(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Fingerprint => "console-fingerprint",
      Self::Password => "console-lock-password",
      Self::LongPassword => "console-lock-password-long",
    })
  }

  pub(crate) fn availability(self) -> Result<Availability, Never> {
    Ok(match self.configured_type() {
      Ok(FileType::RegularFile) => Availability::Available,
      Ok(_) => Availability::Unavailable,
      Err(_inaccessible_or_absent) => Availability::Unavailable,
    })
  }

  fn configured_type(self) -> io::Result<FileType> {
    let rules = open(
      PAM_DIRECTORY,
      OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
      Mode::empty(),
    )?;
    let Ok(name) = self.name();
    let file = openat2(
      &rules,
      name,
      OFlags::PATH | OFlags::CLOEXEC,
      Mode::empty(),
      ResolveFlags::BENEATH | ResolveFlags::NO_MAGICLINKS,
    )?;
    let metadata = fstat(&file)?;
    Ok(FileType::from_raw_mode(metadata.st_mode))
  }
}

pub fn worker_path() -> io::Result<PathBuf> {
  const WORKER: &str = {
    let Ok(name) = InternalProgram::LoginWindow.name();
    name
  };
  let here = std::env::current_exe()?;
  let folder = here.parent().ok_or(io::ErrorKind::NotFound)?;
  Ok(folder.join(WORKER))
}
