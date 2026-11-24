//! Associate reusable loudness measurements with the paths that use them.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use console_core_never::Never;

use super::super::{FileId, Measurement, Revision, cached};
use super::{Source, identity::AudioKey};
use crate::library::{DiskPresence, disk_presence};

const DIRECTORY: &str = "sources-v1";
const MAX_RECORD: u32 = 49_152;
const MAX_OWNER: u32 = 16_384;

struct Owner {
  path: PathBuf,
  file: FileId,
  audio: Option<AudioKey>,
}

pub(super) fn at(root: &Path, path: &Path) -> Result<PathBuf, Never> {
  let Ok(key) = AudioKey::for_path(path);

  Ok(root.join(DIRECTORY).join(key))
}

pub(super) fn lock(root: &Path) -> io::Result<File> {
  fs::create_dir_all(root)?;
  let guard = console_core_atomic_writes::open_lock_file(&root.join("owners.lock"))?;
  guard.lock()?;

  Ok(guard)
}

pub(super) fn encode(
  source: &Source,
  audio: Option<&AudioKey>,
  measurement: &str,
) -> Result<String, Never> {
  let Ok(url) = console_core_file_urls::url(&source.path);
  let Ok(file) = source.file.name();
  let word = match audio {
    Some(key) => {
      let Ok(key_text) = key.text();
      key_text
    }
    None => "-",
  };

  Ok(format!("{url}\n{}\n{word}\n{measurement}", file.0))
}

fn owner(record: &str) -> io::Result<(Owner, &str)> {
  let invalid = || io::Error::new(ErrorKind::InvalidData, "invalid audio cache owner");
  let (url, after_path) = record.split_once('\n').ok_or_else(invalid)?;
  let (file_key, after_file) = after_path.split_once('\n').ok_or_else(invalid)?;
  let (fingerprint, measurement) = after_file.split_once('\n').ok_or_else(invalid)?;
  let Ok(decoded) = console_core_file_urls::path(url);
  let path = decoded.filter(|at| at.is_absolute()).ok_or_else(invalid)?;
  let (device_text, inode_text) = file_key.split_once('-').ok_or_else(invalid)?;
  let device = device_text.parse().map_err(io::Error::other)?;
  let inode = inode_text.parse().map_err(io::Error::other)?;
  let audio = match fingerprint {
    "-" => None,
    _ => {
      let Ok(parsed) = AudioKey::parse(fingerprint);
      let key = parsed.ok_or_else(invalid)?;
      Some(key)
    }
  };

  Ok((
    Owner {
      path,
      file: FileId { device, inode },
      audio,
    },
    measurement,
  ))
}

fn record(at: &Path, limit: u32) -> io::Result<String> {
  let bytes = console_core_atomic_writes::read_prefix(at, limit)?;
  String::from_utf8(bytes).map_err(io::Error::other)
}

pub(super) fn cached_at(
  root: &Path,
  source: &Source,
  stamp: &Revision,
) -> Result<Option<Measurement>, Never> {
  let Ok(path) = at(root, &source.path);
  let text = match record(&path, MAX_RECORD) {
    Ok(text) => text,
    Err(_cache_unavailable) => return Ok(None),
  };

  match owner(&text) {
    Ok((held, measurement)) => {
      match (held.path == source.path && held.file == source.file).then_some(()) {
        Some(()) => cached(measurement, stamp),
        None => Ok(None),
      }
    }
    Err(_invalid_owner) => Ok(None),
  }
}

struct Inventory {
  removed: Vec<(PathBuf, Owner)>,
  files: BTreeSet<FileId>,
  audio: BTreeSet<AudioKey>,
  coverage: Coverage,
}

enum Coverage {
  Complete,
  Incomplete,
}

impl Inventory {
  fn read(root: &Path) -> io::Result<Self> {
    let mut inventory = Self {
      removed: Vec::new(),
      files: BTreeSet::new(),
      audio: BTreeSet::new(),
      coverage: Coverage::Complete,
    };
    let entries = match fs::read_dir(root.join(DIRECTORY)) {
      Ok(found) => found,
      Err(fault) => match (fault.kind() == ErrorKind::NotFound).then_some(()) {
        Some(()) => return Ok(inventory),
        None => return Err(fault),
      },
    };

    for item in entries {
      let entry = item?;
      let name = entry.file_name();
      let spelling = match name.to_str() {
        Some(spelling) => spelling,
        None => continue,
      };
      let Ok(key) = AudioKey::parse(spelling);

      match key {
        Some(_record_name) => match inventory.add(entry.path()) {
          Ok(()) => {}
          Err(fault) => {
            console_core_logging::debug!("could not check an audio cache owner: {fault}");
            inventory.coverage = Coverage::Incomplete;
          }
        },
        None => {}
      }
    }

    Ok(inventory)
  }

  fn add(&mut self, at: PathBuf) -> io::Result<()> {
    let text = record(&at, MAX_OWNER)?;
    let (held, _) = owner(&text)?;
    let presence = match disk_presence(&held.path, held.file.device) {
      Ok(presence) => presence,
      Err(fault) => {
        console_core_logging::debug!(
          "could not verify {} for cache cleanup: {fault}",
          held.path.display()
        );
        DiskPresence::Retain
      }
    };

    match presence {
      DiskPresence::Removed => self.removed.push((at, held)),
      DiskPresence::Retain => {
        let _previous = self.files.insert(held.file);

        match held.audio {
          Some(key) => {
            let _previous = self.audio.insert(key);
          }
          None => {}
        }
      }
    }

    Ok(())
  }

  fn prune(self, root: &Path) -> io::Result<()> {
    match self.coverage {
      Coverage::Complete => {}
      Coverage::Incomplete => return Ok(()),
    }

    for (at, held) in self.removed {
      let presence = disk_presence(&held.path, held.file.device)?;

      match presence {
        DiskPresence::Removed => {}
        DiskPresence::Retain => continue,
      }

      match self.files.contains(&held.file).then_some(()) {
        Some(()) => {}
        None => remove_old_revision(root, held.file)?,
      }

      match held.audio.filter(|key| !self.audio.contains(key)) {
        Some(key) => remove(&root.join("encoded-v1").join(key))?,
        None => {}
      }

      remove(&at)?;
    }

    Ok(())
  }
}

fn remove_old_revision(root: &Path, id: FileId) -> io::Result<()> {
  let Ok(name) = id.name();
  remove(&root.join("revisions").join(&name))?;
  remove(&root.join(name))
}

fn remove(at: &Path) -> io::Result<()> {
  match fs::remove_file(at) {
    Ok(()) => Ok(()),
    Err(fault) => match (fault.kind() == ErrorKind::NotFound).then_some(()) {
      Some(()) => Ok(()),
      None => Err(fault),
    },
  }
}

pub(in crate::loudness) fn prune_removed(root: &Path) -> io::Result<()> {
  let _guard = lock(root)?;
  let inventory = Inventory::read(root)?;

  inventory.prune(root)
}

pub(in crate::loudness) fn prune_old_revisions(
  root: &Path,
  removed: &BTreeSet<FileId>,
  retained: &BTreeSet<FileId>,
) -> io::Result<()> {
  match removed.is_empty().then_some(()) {
    Some(()) => return Ok(()),
    None => {}
  }

  let _guard = lock(root)?;
  let inventory = Inventory::read(root)?;

  match inventory.coverage {
    Coverage::Complete => {}
    Coverage::Incomplete => return Ok(()),
  }

  for id in removed
    .difference(retained)
    .filter(|id| !inventory.files.contains(id))
  {
    remove_old_revision(root, *id)?;
  }

  Ok(())
}
