//! The library scan can be observed and claimed independently of the window.

use std::fs::File;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_places::Base;

pub const LOCK_NAME: &str = "analysis.lock";
pub const BUSY_EXIT_CODE: u8 = 2;

pub enum Claim {
  Started(Guard),
  AlreadyRunning,
}

pub struct Guard(File);

impl Drop for Guard {
  fn drop(&mut self) {
    match self.0.unlock() {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::warn!("could not release the Music Library scan: {fault}")
      }
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Activity {
  Idle,
  Running,
}

pub fn directory() -> Result<Option<PathBuf>, Never> {
  let Ok(root) = Base::Cache.ours();

  Ok(root.map(|at| at.join("music")))
}

pub fn claim() -> io::Result<Claim> {
  let Ok(directory) = directory();
  let cache = directory.ok_or_else(|| io::Error::other("Music Library cache is unavailable"))?;
  std::fs::create_dir_all(&cache)?;

  acquire(&cache.join(LOCK_NAME))
}

fn acquire(at: &Path) -> io::Result<Claim> {
  let file = console_core_atomic_writes::open_lock_file(at)?;

  match file.try_lock() {
    Ok(()) => Ok(Claim::Started(Guard(file))),
    Err(std::fs::TryLockError::WouldBlock) => Ok(Claim::AlreadyRunning),
    Err(std::fs::TryLockError::Error(fault)) => Err(fault),
  }
}

pub fn activity() -> Result<Activity, Never> {
  let Ok(directory) = directory();
  let active = match directory {
    Some(cache) => observed(&cache.join(LOCK_NAME)),
    None => return Ok(Activity::Idle),
  };

  Ok(match active {
    Ok(activity) => activity,
    Err(fault) => {
      console_core_logging::warn!("could not check the Music Library scan: {fault}");

      Activity::Idle
    }
  })
}

fn observed(at: &Path) -> io::Result<Activity> {
  let file = match File::open(at) {
    Ok(file) => file,
    Err(fault) => match (fault.kind() == ErrorKind::NotFound).then_some(()) {
      Some(()) => return Ok(Activity::Idle),
      None => return Err(fault),
    },
  };

  match file.try_lock_shared() {
    Ok(()) => Ok(Activity::Idle),
    Err(std::fs::TryLockError::WouldBlock) => Ok(Activity::Running),
    Err(std::fs::TryLockError::Error(fault)) => Err(fault),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_second_scan_observes_the_first_until_it_finishes() -> io::Result<()> {
    let cache = console_core_temporary_directories::fresh("music-library-scan")
      .map_err(io::Error::other)?;
    let at = cache.join(LOCK_NAME);
    let before = observed(&at)?;
    assert_eq!(before, Activity::Idle);

    let first = acquire(&at)?;
    let guard = match first {
      Claim::Started(guard) => guard,
      Claim::AlreadyRunning => {
        return Err(io::Error::other("the first scan could not claim its lock"));
      }
    };

    let during = observed(&at)?;
    assert_eq!(during, Activity::Running);
    let overlapping = acquire(&at)?;
    assert!(matches!(overlapping, Claim::AlreadyRunning));
    drop(guard);

    let after = observed(&at)?;
    assert_eq!(after, Activity::Idle);
    let retry = acquire(&at)?;
    assert!(matches!(retry, Claim::Started(_)));

    Ok(())
  }
}
