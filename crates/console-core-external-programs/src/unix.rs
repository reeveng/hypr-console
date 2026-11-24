//! Resolve Unix executables without starting a process.

use std::ffi::OsStr;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use console_core_never::Never;

use super::path_entries;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installed {
  Yes,
  No,
}

impl From<bool> for Installed {
  fn from(found: bool) -> Self {
    match bool::then_some(found, ()) {
      Some(()) => Self::Yes,
      None => Self::No,
    }
  }
}

struct ProgramName<'a>(&'a OsStr);

struct InvalidProgramName;

impl<'a> ProgramName<'a> {
  fn new(program: &'a OsStr) -> Result<Self, InvalidProgramName> {
    match program.as_encoded_bytes() {
      b"" | b"." | b".." => Err(InvalidProgramName),
      bytes => match (bytes.contains(&0) || bytes.contains(&b'/')).then_some(()) {
        Some(()) => Err(InvalidProgramName),
        None => Ok(Self(program)),
      },
    }
  }
}

impl AsRef<Path> for ProgramName<'_> {
  fn as_ref(&self) -> &Path {
    Path::new(self.0)
  }
}

#[cfg_attr(
  dylint_lib = "explicit001_fallible_result",
  expect(
    explicit001_fallible_result,
    reason = "availability callers need a yes or no; provide returns the lookup error when needed"
  )
)]
pub fn installed(program: impl AsRef<OsStr>) -> Result<Installed, Never> {
  Ok(Installed::from(provide(program).is_ok()))
}

pub fn provide(program: impl AsRef<OsStr>) -> io::Result<PathBuf> {
  provide_with(program, |_| Ok(()))
}

#[doc = "Check platform access per candidate and return the absolute path with symlinks intact."]
pub fn provide_with(
  program: impl AsRef<OsStr>,
  access: impl Fn(&Path) -> io::Result<()>,
) -> io::Result<PathBuf> {
  let requested = program.as_ref();
  let lookup = match ProgramName::new(requested) {
    Ok(name) => search(&name, access),
    Err(InvalidProgramName) => check(Path::new(requested), access),
  };

  lookup.map_err(|error| {
    io::Error::new(
      error.kind(),
      format!(
        "executable {} is unavailable: {error}",
        Path::new(requested).display()
      ),
    )
  })
}

fn search(
  name: &ProgramName<'_>,
  access: impl Fn(&Path) -> io::Result<()>,
) -> io::Result<PathBuf> {
  let Ok(entries) = path_entries();
  let directories = match entries {
    Some(configured) => configured,
    None => vec![PathBuf::from("/bin"), PathBuf::from("/usr/bin")],
  };
  let mut fault = io::Error::new(
    io::ErrorKind::NotFound,
    "not found in the executable search path",
  );

  for directory in directories {
    match check(&directory.join(name), &access) {
      Ok(executable) => return Ok(executable),
      Err(error) => match (error.kind() == io::ErrorKind::NotFound).then_some(()) {
        Some(()) => continue,
        None => fault = error,
      },
    }
  }

  Err(fault)
}

const RUNNABLE: u32 = 0o111;

fn check(at: &Path, access: impl FnOnce(&Path) -> io::Result<()>) -> io::Result<PathBuf> {
  let about = std::fs::metadata(at)?;

  match (about.is_file(), about.permissions().mode() & RUNNABLE) {
    (false, _) => Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      "expected a regular file",
    )),
    (true, 0) => Err(io::Error::new(
      io::ErrorKind::PermissionDenied,
      "file is not executable",
    )),
    (true, _) => {
      access(at)?;
      std::path::absolute(at)
    }
  }
}
