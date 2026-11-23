//! The Legion Go's desktop, running on this machine, in a window.
//!
//! The device is a screen you have to pick up and a session you have to log
//! into. Most of what is worth looking at, the bar, the menu, the keyboard, the
//! panel, the colors everything wears, is ordinary Wayland software reading
//! ordinary config files, and this machine can run all of it. So it does: a
//! compositor of its own, inside a window, at the size the device's screen
//! actually is, reading the files the device reads.
//!
//! What it is not is the device. Read `## What this is not` in the
//! `desktop.md` note in Orbit before trusting it about anything.
//!
//! The staged copy is the whole of the trick. Every file the desktop reads is
//! copied out of files/ into one directory of this session's own, and every
//! absolute path inside those files is rewritten to point back into it. So a
//! stylesheet that says /usr/share/backgrounds/console.webp finds the picture
//! that is going to be installed there, without this machine having a
//! /usr/share/backgrounds it is allowed to write to. The copy is deleted when
//! the command ends, so there is nothing in it worth being careful with.

pub mod connection;
pub mod nested;
pub mod session;
pub mod staging;

use std::fmt;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_repository::RelativePath;

#[derive(Debug)]
pub enum Unnested {
  Machine(std::io::Error),
  Read(PathBuf, std::io::Error),
  Undeclared(console_screen::Undeclared),
  Staging(&'static str, std::io::Error),
  Unwritten(&'static str, console_core_atomic_writes::Unwritten),
  NoExecStart(PathBuf),
  NoCompositor,
  NoScreen,
  InvalidStage(PathBuf),
  NowhereToWrite,
  NothingToPress,
}

impl fmt::Display for Unnested {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Unnested::Machine(fault) => write!(to, "{fault}"),
      Unnested::Read(at, fault) => write!(to, "{}: {fault}", at.display()),
      Unnested::Undeclared(fault) => write!(to, "{fault}"),
      Unnested::Staging(what, fault) => write!(to, "{what}: {fault}"),
      Unnested::Unwritten(what, fault) => write!(to, "{what}: {fault}"),
      Unnested::NoExecStart(at) => write!(
        to,
        "{} names no absolute ExecStart, so the stage would start a keyboard \
                 nothing could raise",
        at.display()
      ),
      Unnested::NoCompositor => write!(to, "the nested compositor never came up"),
      Unnested::NoScreen => write!(to, "the screen never appeared"),
      Unnested::InvalidStage(at) => write!(
        to,
        "CONSOLE_STAGE must name one directory under .stage: {}",
        at.display()
      ),
      Unnested::NowhereToWrite => {
        write!(to, "a picture wants somewhere to be written")
      }
      Unnested::NothingToPress => write!(
        to,
        "a press wants the file the panel writes its draws to and a --then to press"
      ),
    }
  }
}

impl std::error::Error for Unnested {}

impl From<console_screen::Undeclared> for Unnested {
  fn from(fault: console_screen::Undeclared) -> Self {
    Unnested::Undeclared(fault)
  }
}

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  allow(
    explicit026_env_read_once,
    reason = "CONSOLE_STAGE names which stage tree this run is using, and a stage is what this crate is. What it used to read as well -- PATH, the runtime directory, the compositor's signature -- belongs to other crates and is asked of them now"
  )
)]
pub(crate) fn env_var(name: &str) -> Result<Option<String>, Never> {
  Ok(match std::env::var(name) {
    Ok(said) => Some(said),
    Err(std::env::VarError::NotPresent) => None,
    Err(fault) => {
      console_core_logging::warn!("console-desktop: {name}: {fault}");

      None
    }
  })
}

pub fn root() -> Result<PathBuf, Never> {
  let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
  let Ok(repository) = console_repository::above(manifest);

  Ok(
    match repository
      .as_deref()
      .or_else(|| manifest.ancestors().nth(2))
    {
      Some(root) => root.to_path_buf(),
      None => manifest.to_path_buf(),
    },
  )
}

pub fn screen() -> Result<console_screen::Screen, Unnested> {
  let Ok(root) = root();

  let at = root.join(console_screen::CONFIGURATION);
  let said = std::fs::read_to_string(&at).map_err(|why| Unnested::Read(at.clone(), why))?;

  console_screen::Screen::read(&said).map_err(Unnested::Undeclared)
}

pub fn stages() -> Result<PathBuf, Never> {
  let Ok(root) = root();

  Ok(root.join(".stage"))
}

pub fn stage() -> Result<PathBuf, Unnested> {
  let Ok(told) = env_var("CONSOLE_STAGE");
  let named = match told {
    Some(named) => named,
    None => format!("session-{}", std::process::id()),
  };
  let Ok(stages) = stages();
  let Ok(child) = child_name(&named);
  let relative = child.ok_or_else(|| Unnested::InvalidStage(PathBuf::from(&named)))?;

  Ok(stages.join(relative))
}

pub(crate) fn child_name(name: &str) -> Result<Option<RelativePath<'_>>, Never> {
  let path = Path::new(name);

  Ok(
    match (path.file_name() == Some(path.as_os_str())).then_some(()) {
      Some(()) => match RelativePath::new(path) {
        Ok(child) => Some(child),
        Err(fault) => {
          console_core_logging::debug!("console-desktop: invalid child name: {fault}");

          None
        }
      },
      None => None,
    },
  )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Process {
  Present,
  Gone,
}

pub(crate) fn process_presence(pid: rustix::process::Pid) -> Result<Process, Never> {
  Ok(match rustix::process::test_kill_process(pid) {
    Err(rustix::io::Errno::SRCH) => Process::Gone,
    Ok(()) => Process::Present,
    Err(fault) => {
      console_core_logging::debug!("console-desktop: cannot check process {pid:?}: {fault}");

      Process::Present
    }
  })
}

pub(crate) fn open_regular(at: &Path) -> std::io::Result<(std::fs::File, std::fs::Metadata)> {
  use rustix::fs::{Mode, OFlags};

  let descriptor = rustix::fs::open(
    at,
    OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
    Mode::empty(),
  )?;
  let file = std::fs::File::from(descriptor);
  let metadata = file.metadata()?;

  match metadata.is_file().then_some(()) {
    Some(()) => Ok((file, metadata)),
    None => Err(std::io::Error::new(
      std::io::ErrorKind::InvalidInput,
      format!("{} is not a regular file", at.display()),
    )),
  }
}

pub fn scope_of(stage: &Path) -> Result<Option<String>, Never> {
  Ok(
    stage
      .file_name()
      .and_then(std::ffi::OsStr::to_str)
      .map(|name| format!("console-desktop-{name}")),
  )
}

pub fn runtime() -> Result<PathBuf, Never> {
  let told = console_core_places::runtime()?;

  Ok(match told {
    Some(told_2) => told_2,
    None => {
      let uid = rustix::process::getuid().as_raw();

      PathBuf::from(format!("/run/user/{uid}"))
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_stage_or_signature_names_one_directory() {
    let Ok(valid) = child_name("session-123");

    assert!(valid.is_some());

    for name in ["", ".", "..", "../outside", "inside/nested", "/absolute"] {
      let Ok(refused) = child_name(name);

      assert!(refused.is_none(), "accepted {name:?}");
    }
  }
}
