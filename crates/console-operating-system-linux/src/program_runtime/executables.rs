//! Each Linux process adapter retains successful executable lookups for one minute.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use console_core_external_programs::{Program, path_entries, provide_with};
use console_core_never::Never;
use rustix::fs::{Access, AtFlags, CWD, accessat};

const FRESH_FOR: Duration = Duration::from_secs(60);

enum Search {
  Absolute,
  Relative,
}

impl Search {
  fn configured() -> Result<Self, Never> {
    let Ok(configured) = path_entries();
    let relative = configured
      .into_iter()
      .flatten()
      .any(|directory| directory.is_relative());

    Ok(match relative.then_some(()) {
      Some(()) => Self::Relative,
      None => Self::Absolute,
    })
  }
}

fn resolve(name: &str) -> io::Result<PathBuf> {
  provide_with(name, |path| {
    accessat(CWD, path, Access::EXEC_OK, AtFlags::EACCESS).map_err(io::Error::from)
  })
}

#[derive(Default)]
pub(super) struct Cache {
  entries: BTreeMap<&'static str, Resolved>,
  search: Option<Search>,
}

struct Resolved {
  path: PathBuf,
  checked_at: Instant,
}

impl Cache {
  pub(super) fn executable(&mut self, program: Program, now: Instant) -> io::Result<PathBuf> {
    let Ok(name) = program.name();
    let search = self.search.get_or_insert_with(|| {
      let Ok(configured) = Search::configured();

      configured
    });

    match search {
      Search::Relative => resolve(name),
      Search::Absolute => self.provide(name, now, || resolve(name)),
    }
  }

  pub(super) fn launched<T>(&mut self, path: &Path, result: io::Result<T>) -> io::Result<T> {
    match result {
      Ok(child) => Ok(child),
      Err(error) => {
        let Ok(()) = self.launch_failed(path, &error);

        Err(error)
      }
    }
  }

  fn launch_failed(&mut self, path: &Path, error: &io::Error) -> Result<(), Never> {
    match matches!(
      error.kind(),
      io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied | io::ErrorKind::NotADirectory
    )
    .then_some(())
    {
      Some(()) => self.forget(path),
      None => Ok(()),
    }
  }

  fn provide(
    &mut self,
    name: &'static str,
    now: Instant,
    resolve: impl FnOnce() -> io::Result<PathBuf>,
  ) -> io::Result<PathBuf> {
    let cached = self
      .entries
      .get(name)
      .filter(|resolved| now.saturating_duration_since(resolved.checked_at) < FRESH_FOR)
      .map(|resolved| resolved.path.clone());

    match cached {
      Some(path) => return Ok(path),
      None => {}
    }

    let path = resolve()?;
    let _previous = self.entries.insert(
      name,
      Resolved {
        path: path.clone(),
        checked_at: now,
      },
    );

    Ok(path)
  }

  fn forget(&mut self, path: &Path) -> Result<(), Never> {
    self.entries.retain(|_, resolved| resolved.path != path);

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_success_is_reused_until_it_expires_or_its_launch_fails() -> io::Result<()> {
    let mut cache = Cache::default();
    let first = PathBuf::from("/installed/first");
    let next = PathBuf::from("/installed/next");
    let latest = PathBuf::from("/installed/latest");
    let now = Instant::now();
    let established = cache.provide("tool", now, || Ok(first.clone()))?;
    let reused = cache.provide("tool", now, || Ok(next.clone()))?;
    let expired = now
      .checked_add(FRESH_FOR)
      .ok_or_else(|| io::Error::other("test clock overflow"))?;
    let refreshed = cache.provide("tool", expired, || Ok(next.clone()))?;
    let failed = cache.launched::<()>(&next, Err(io::Error::from(io::ErrorKind::NotFound)));
    let replaced = cache.provide("tool", expired, || Ok(latest.clone()))?;

    assert_eq!(established, first);
    assert_eq!(reused, first);
    assert_eq!(refreshed, next);
    assert_eq!(
      failed.map_err(|error| error.kind()),
      Err(io::ErrorKind::NotFound)
    );
    assert_eq!(replaced, latest);

    Ok(())
  }

  #[test]
  fn installation_can_satisfy_a_previously_missing_dependency_immediately() -> io::Result<()> {
    let mut cache = Cache::default();
    let now = Instant::now();
    let missing = cache.provide("tool", now, || {
      Err(io::Error::from(io::ErrorKind::NotFound))
    });
    let installed = cache.provide("tool", now, || Ok(PathBuf::from("/installed/tool")))?;

    assert_eq!(
      missing.map_err(|error| error.kind()),
      Err(io::ErrorKind::NotFound)
    );
    assert_eq!(installed, Path::new("/installed/tool"));

    Ok(())
  }
}
