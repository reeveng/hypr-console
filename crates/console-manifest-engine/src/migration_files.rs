//! File moves for the original Legion-to-Console migration.
//! Across mounts, GNU mv preserves metadata and uses the same exact
//! destination and no-overwrite behavior as the in-filesystem rename.

use std::fs;
use std::io;
use std::path::Path;

use console_core_external_programs::Program;
use console_repository::RelativePath;
use rustix::fs::{CWD, RenameFlags, renameat_with};
use rustix::io::Errno;

pub struct HomeMove<'a> {
  pub old: &'a str,
  pub new: &'a str,
}

pub fn move_home(HomeMove { old, new }: HomeMove<'_>) -> io::Result<()> {
  let old_path = checked_relative(Path::new(old))?;
  let new_path = checked_relative(Path::new(new))?;
  let Ok(home) = console_core_places::home();
  let home_2 = home.ok_or_else(|| io::Error::other("HOME is missing"))?;
  let before = home_2.join(old_path);
  let after = home_2.join(new_path);

  match before.symlink_metadata() {
    Ok(_) => {}
    Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
      Some(()) => return Ok(()),
      None => return Err(fault),
    },
  }

  let parent = after
    .parent()
    .ok_or_else(|| io::Error::other("destination has no parent"))?;
  fs::create_dir_all(parent)?;

  let outcome = moved(&before, &after)?;

  match outcome {
    Move::Done => println!("  {old} -> {new}"),
    Move::AlreadyPresent => {}
  }

  Ok(())
}

fn checked_relative(path: &Path) -> io::Result<RelativePath<'_>> {
  RelativePath::new(path).map_err(|fault| io::Error::new(io::ErrorKind::InvalidInput, fault))
}

pub fn move_pictures(old: &Path, new: &Path) -> io::Result<()> {
  let entries = match fs::read_dir(old) {
    Ok(entries) => entries,
    Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
      Some(()) => return Ok(()),
      None => return Err(fault),
    },
  };
  fs::create_dir_all(new)?;

  for entry in entries {
    let entry_2 = entry?;
    let name = entry_2.file_name();

    match ::core::primitive::bool::then_some(name.as_encoded_bytes().starts_with(b"."), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        let relative = checked_relative(Path::new(&name))?;
        moved(&entry_2.path(), &new.join(relative))?;
      }
    }
  }

  match fs::remove_dir(old) {
    Ok(()) => Ok(()),
    Err(fault) => match (fault.kind() == io::ErrorKind::DirectoryNotEmpty).then_some(()) {
      Some(()) => Ok(()),
      None => Err(fault),
    },
  }
}

pub fn sweep(destination: &Path, patterns: &[String]) -> io::Result<()> {
  fs::create_dir_all(destination)?;

  for pattern in patterns {
    let source = Path::new(pattern);
    let parent = source
      .parent()
      .ok_or_else(|| io::Error::other("pattern has no parent"))?;
    let name = source
      .file_name()
      .and_then(|name| name.to_str())
      .ok_or_else(|| io::Error::other("pattern has no name"))?;

    match name.split_once('*') {
      Some((prefix, suffix)) => {
        let entries = match fs::read_dir(parent) {
          Ok(entries) => entries,
          Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
            Some(()) => continue,
            None => return Err(fault),
          },
        };

        sweep_matches(entries, destination, NamePattern { prefix, suffix })?;
      }
      None => {
        let relative = checked_relative(Path::new(name))?;
        moved(source, &destination.join(relative))?;
      }
    }
  }

  Ok(())
}

struct NamePattern<'a> {
  prefix: &'a str,
  suffix: &'a str,
}

fn sweep_matches(
  entries: fs::ReadDir,
  destination: &Path,
  pattern: NamePattern<'_>,
) -> io::Result<()> {
  for entry in entries {
    let item = entry?;
    let file = item.file_name();

    let rest = match file
      .as_encoded_bytes()
      .strip_prefix(pattern.prefix.as_bytes())
    {
      Some(rest) => rest,
      None => continue,
    };

    match ::core::primitive::bool::then_some(rest.ends_with(pattern.suffix.as_bytes()), ()) {
      Some(()) => {
        let name = checked_relative(Path::new(&file))?;
        moved(&item.path(), &destination.join(name))?;
      }
      None => {}
    }
  }

  Ok(())
}

enum Move {
  Done,
  AlreadyPresent,
}

fn moved(from: &Path, to: &Path) -> io::Result<Move> {
  match renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
    Ok(()) => Ok(Move::Done),
    Err(Errno::NOENT | Errno::EXIST) => Ok(Move::AlreadyPresent),
    Err(Errno::XDEV) => {
      let Ok(mut command) = Program::Mv.command();
      let status = command
        .args(["--no-clobber", "--no-target-directory", "--"])
        .arg(from)
        .arg(to)
        .status()?;

      match ::core::primitive::bool::then_some(status.success(), ()) {
        ::core::option::Option::Some(()) => checked_source(from),
        ::core::option::Option::None => Err(io::Error::other(format!(
          "moving {} to {}: {status}",
          from.display(),
          to.display()
        ))),
      }
    }
    Err(fault) => Err(io::Error::from(fault)),
  }
}

fn checked_source(from: &Path) -> io::Result<Move> {
  Ok(match from.symlink_metadata() {
    Ok(_present) => Move::AlreadyPresent,
    Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
      Some(()) => Move::Done,
      None => {
        console_core_logging::warn!("cannot check migration source {}: {fault}", from.display());
        Move::AlreadyPresent
      }
    },
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_atomic_writes::whole;

  #[test]
  fn sweep_matches_literally_and_leaves_existing_destinations_alone()
  -> Result<(), Box<dyn std::error::Error>> {
    let root = console_core_temporary_directories::private("migration-sweep")?;
    let source = root.join("source");
    let attic = root.join("attic");
    fs::create_dir(&source)?;
    fs::create_dir(&attic)?;
    whole(&source.join("legion-original.desktop"), b"original")?;
    whole(&source.join("legion-Bob's notes.desktop"), b"move")?;
    whole(&source.join("legion-unrelated.txt"), b"leave")?;
    whole(&attic.join("legion-original.desktop"), b"prior backup")?;

    sweep(
      &attic,
      &[source.join("legion-*.desktop").display().to_string()],
    )?;

    let backup = fs::read(attic.join("legion-original.desktop"))?;
    let original = fs::read(source.join("legion-original.desktop"))?;
    let moved = fs::read(attic.join("legion-Bob's notes.desktop"))?;
    let unrelated = fs::read(source.join("legion-unrelated.txt"))?;
    assert_eq!(backup, b"prior backup");
    assert_eq!(original, b"original");
    assert_eq!(moved, b"move");
    assert_eq!(unrelated, b"leave");
    fs::remove_dir_all(root)?;
    Ok(())
  }
}
