//! The library's explicit visibility selections. Source files stay in place.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use console_core_atomic_writes::Stored;
use console_core_never::Never;
use console_core_places::Base;

#[derive(Debug, Default)]
pub struct Selection(BTreeMap<PathBuf, Visibility>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
  Included,
  Hidden,
}

enum Change {
  Recorded,
  Unchanged,
}

fn location() -> io::Result<PathBuf> {
  let Ok(state) = Base::State.ours();
  let root =
    state.ok_or_else(|| io::Error::other("Music Library state directory is unavailable"))?;

  Ok(root.join("music").join("recording-selections-v1"))
}

impl Selection {
  pub fn load() -> io::Result<Self> {
    let path = location()?;

    Self::load_from(&path)
  }

  fn load_from(path: &Path) -> io::Result<Self> {
    let Ok(stored) = console_core_atomic_writes::read(path);

    match stored {
      Stored::Absent => Ok(Self::default()),
      Stored::Failed(fault) => Err(io::Error::other(fault)),
      Stored::Text(text) => Self::parse(&text),
    }
  }

  fn parse(text: &str) -> io::Result<Self> {
    let mut paths = BTreeMap::new();

    for line in text.lines() {
      let (visibility, url) = match line.split_once(' ') {
        Some(("include", url)) => (Visibility::Included, url),
        Some(("hide", url)) => (Visibility::Hidden, url),
        Some(_) | None => return Err(io::Error::other("invalid Music Library selection")),
      };
      let Ok(decoded) = console_core_file_urls::path(url);
      let path =
        decoded.ok_or_else(|| io::Error::other("invalid file URL in Music Library selection"))?;
      let _old = paths.insert(path, visibility);
    }

    Ok(Self(paths))
  }

  pub fn decision(&self, path: &Path) -> Result<Option<Visibility>, Never> {
    Ok(self.0.get(path).copied())
  }

  pub fn visibility(&self, path: &Path) -> Result<Visibility, Never> {
    let Ok(decision) = self.decision(path);

    Ok(match decision {
      Some(visibility) => visibility,
      None => Visibility::Included,
    })
  }

  pub fn apply(changes: &[(PathBuf, Visibility)]) -> io::Result<()> {
    Self::edit(|selection| {
      for (track, visibility) in changes {
        let _old = selection.0.insert(track.clone(), *visibility);
      }

      Ok(Change::Recorded)
    })
  }

  pub fn forget(paths: &BTreeSet<&Path>) -> io::Result<()> {
    match paths.is_empty().then_some(()) {
      Some(()) => Ok(()),
      None => Self::edit(|selection| {
        let Ok(before) = console_core_number_conversion::fixed_index(selection.0.len());
        selection
          .0
          .retain(|path, _| !paths.contains(path.as_path()));

        let Ok(after) = console_core_number_conversion::fixed_index(selection.0.len());

        Ok(match (after == before).then_some(()) {
          Some(()) => Change::Unchanged,
          None => Change::Recorded,
        })
      }),
    }
  }

  fn edit(change: impl FnOnce(&mut Self) -> Result<Change, Never>) -> io::Result<()> {
    let path = location()?;
    let parent = path
      .parent()
      .ok_or_else(|| io::Error::other("Music Library state has no directory"))?;
    std::fs::create_dir_all(parent)?;
    let locked = console_core_atomic_writes::open_lock_file(&path.with_extension("lock"))?;
    locked.lock()?;

    let mut selection = Self::load_from(&path)?;
    let Ok(changed) = change(&mut selection);

    match changed {
      Change::Recorded => {
        let Ok(text) = selection.encoded();
        console_core_atomic_writes::whole_with_folders(&path, text.as_bytes())
          .map_err(io::Error::other)
      }
      Change::Unchanged => Ok(()),
    }
  }

  fn encoded(&self) -> Result<String, Never> {
    use std::fmt::Write;
    let mut text = String::new();

    for (track, visibility) in &self.0 {
      let Ok(url) = console_core_file_urls::url(track);
      let word = match visibility {
        Visibility::Included => "include",
        Visibility::Hidden => "hide",
      };
      let _written = writeln!(text, "{word} {url}");
    }

    Ok(text)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reviewed_and_unreviewed_inclusions_stay_distinct() -> io::Result<()> {
    let initial = "include file:///music/one%0Atrack.flac\nhide file:///music/two.flac\n";
    let selection = Selection::parse(initial)?;
    assert_eq!(
      selection.decision(Path::new("/music/one\ntrack.flac")),
      Ok(Some(Visibility::Included))
    );
    assert_eq!(
      selection.decision(Path::new("/music/unreviewed.flac")),
      Ok(None)
    );
    assert_eq!(
      selection.visibility(Path::new("/music/two.flac")),
      Ok(Visibility::Hidden)
    );
    assert_eq!(selection.encoded(), Ok(initial.to_owned()));

    Ok(())
  }
}
