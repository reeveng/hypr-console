//! Everything under a directory, however deep, as one list.
//!
//! The walk had been written out in about ten crates, each the same way: a
//! stack of directories, a `while let Some(..) = waiting.pop()`, a `read_dir`
//! and a push for every directory found. EXPLICIT053 asked each of those for
//! the answer its `while` left unnamed, and the answer was the same everywhere,
//! so it is written here once rather than ten times over as a `loop`.
//!
//! It is read a depth at a time: the entries of the directory asked about, then
//! the entries of every directory among those, and so on until a depth has no
//! directory left to open. That is `successors`, which ends when its step says
//! there is nothing after, so the walk says where it stops rather than being
//! left when a condition fails. A caller that wants the entries in an order
//! sorts them; the order here is a depth at a time and is promised no further.
//!
//! What the caller decides is which directories are opened, because that is
//! what differed between the ten: one stays out of `__pycache__`, one out of
//! the programs a panel was built into, one out of what a copy keeps. A link
//! to a directory is listed and never opened, because a link can point back
//! up the tree it sits in and a walk that follows it does not end.
//! Directory opens refuse links, including a directory replaced by a link
//! after it was listed. Entry types come from that listing, with a relative
//! metadata lookup when the filesystem does not supply them.
//!
//! `files` is the question most callers were asking: every file under a
//! directory, links included, sorted, and a directory that cannot be read
//! walked past.
//!
//! A directory that cannot be read is an entry of its own, `Unlisted`, in the
//! place its contents would have been. A walk over a tree somebody else is
//! writing meets that, and whether it is a fault or a thing to walk past is
//! the caller's to say.

use console_core_never::Never;
use rustix::fd::OwnedFd;
use rustix::fs::{AtFlags, Dir, DirEntry, FileType, Mode, OFlags};
use std::fmt;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Descend {
  Into,
  Past,
}

#[derive(Debug)]
pub struct Unlisted {
  pub directory: PathBuf,

  pub fault: std::io::Error,
}

impl fmt::Display for Unlisted {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      to,
      "{} could not be listed: {}",
      self.directory.display(),
      self.fault
    )
  }
}

impl std::error::Error for Unlisted {}

pub type Entry = Result<PathBuf, Unlisted>;

enum Listed {
  File(PathBuf),
  Directory(PathBuf),
}

type Described = Result<Listed, Unlisted>;

pub fn recursive<'a>(
  root: &Path,
  descend: impl Fn(&Path) -> Descend + 'a,
) -> Result<impl Iterator<Item = Entry> + 'a, Never> {
  let Ok(walk) = described(root, descend);

  Ok(walk.map(|entry| {
    entry.map(|found| match found {
      Listed::File(path) | Listed::Directory(path) => path,
    })
  }))
}

#[cfg_attr(
  dylint_lib = "explicit055_no_unbounded_loop",
  expect(
    explicit055_no_unbounded_loop,
    reason = "this is the site that owns the filesystem walk; the next depth is None when no selected directory has entries, and directory opens refuse symlinks that could lead back up the tree"
  )
)]
fn described<'a>(
  root: &Path,
  descend: impl Fn(&Path) -> Descend + 'a,
) -> Result<impl Iterator<Item = Described> + 'a, Never> {
  let Ok(first) = listed(root);

  Ok(
    std::iter::successors(Some(first), move |depth| {
      let Ok(next) = deeper(depth, &descend);

      match next.as_slice() {
        [] => None,
        [_, ..] => Some(next),
      }
    })
    .flatten(),
  )
}

pub fn files(root: &Path) -> Result<Vec<PathBuf>, Never> {
  let Ok(listing) = described(root, |_| Descend::Into);
  let mut found: Vec<PathBuf> = listing
    .filter_map(|entry| match entry {
      Ok(Listed::File(path)) => Some(path),
      Ok(Listed::Directory(_path)) => None,
      Err(_unlisted) => None,
    })
    .collect();

  found.sort();

  Ok(found)
}

fn deeper(
  depth: &[Described],
  descend: &impl Fn(&Path) -> Descend,
) -> Result<Vec<Described>, Never> {
  Ok(
    depth
      .iter()
      .filter_map(|entry| match entry {
        Ok(Listed::Directory(path)) => Some(path),
        Ok(Listed::File(_path)) => None,
        Err(_unlisted) => None,
      })
      .filter(|path| descend(path) == Descend::Into)
      .flat_map(|path| {
        let Ok(entries) = listed(path);

        entries
      })
      .collect(),
  )
}

fn listed(directory: &Path) -> Result<Vec<Described>, Never> {
  Ok(match read_directory(directory) {
    Ok(entries) => entries,
    Err(fault) => vec![Err(fault)],
  })
}

fn read_directory(directory: &Path) -> Result<Vec<Described>, Unlisted> {
  let fault_at = |fault: rustix::io::Errno| Unlisted {
    directory: directory.to_path_buf(),
    fault: std::io::Error::from(fault),
  };
  let fd = rustix::fs::open(
    directory,
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
    Mode::empty(),
  )
  .map_err(fault_at)?;
  let entries = Dir::read_from(&fd).map_err(fault_at)?;

  Ok(
    entries
      .filter_map(|entry| match entry {
        Ok(found) => match found.file_name().to_bytes() {
          b"." | b".." => None,
          _name => Some(describe(found, &fd, directory)),
        },
        Err(fault) => Some(Err(fault_at(fault))),
      })
      .collect(),
  )
}

#[cfg_attr(
  dylint_lib = "explicit086_no_climbing_join",
  expect(
    explicit086_no_climbing_join,
    reason = "the kernel supplies one directory entry name with no slash, and read_directory has removed dot and dot-dot"
  )
)]
fn describe(entry: DirEntry, fd: &OwnedFd, directory: &Path) -> Result<Listed, Unlisted> {
  let name = std::ffi::OsStr::from_bytes(entry.file_name().to_bytes());
  let path = directory.join(name);
  let kind = match entry.file_type() {
    FileType::Unknown => {
      let metadata =
        rustix::fs::statat(fd, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|fault| Unlisted {
          directory: directory.to_path_buf(),
          fault: std::io::Error::from(fault),
        })?;

      FileType::from_raw_mode(metadata.st_mode)
    }
    known @ (FileType::RegularFile
    | FileType::Directory
    | FileType::Symlink
    | FileType::Fifo
    | FileType::Socket
    | FileType::CharacterDevice
    | FileType::BlockDevice) => known,
  };

  Ok(match kind {
    FileType::Directory => Listed::Directory(path),
    FileType::Unknown
    | FileType::RegularFile
    | FileType::Symlink
    | FileType::Fifo
    | FileType::Socket
    | FileType::CharacterDevice
    | FileType::BlockDevice => Listed::File(path),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  type Failure = Box<dyn std::error::Error>;

  fn tree(named: &str) -> Result<PathBuf, Failure> {
    let at = console_core_temporary_directories::fresh(named)?;

    std::fs::create_dir_all(at.join("a/b/c"))?;
    std::fs::create_dir_all(at.join("skipped/inside"))?;
    console_core_atomic_writes::whole(&at.join("top"), b"")?;
    console_core_atomic_writes::whole(&at.join("a/b/c/deep"), b"")?;
    console_core_atomic_writes::whole(&at.join("skipped/inside/hidden"), b"")?;
    std::os::unix::fs::symlink(&at, at.join("a/back-up"))?;

    Ok(at)
  }

  fn under(at: &Path, descend: impl Fn(&Path) -> Descend) -> Result<Vec<PathBuf>, Failure> {
    let Ok(walk) = recursive(at, descend);
    let mut found = walk
      .map(|entry| {
        entry.map(|path| match path.strip_prefix(at) {
          Ok(inside) => inside.to_path_buf(),
          Err(_outside) => path,
        })
      })
      .collect::<Result<Vec<PathBuf>, Unlisted>>()?;

    found.sort();

    Ok(found)
  }

  #[test]
  fn every_depth_is_listed_until_one_has_no_directory_in_it() -> Result<(), Failure> {
    let at = tree("listing-every")?;
    let found = under(&at, |_| Descend::Into)?;

    assert!(found.contains(&PathBuf::from("a/b/c/deep")), "{found:?}");
    assert!(
      found.contains(&PathBuf::from("skipped/inside/hidden")),
      "{found:?}"
    );
    assert!(found.contains(&PathBuf::from("top")), "{found:?}");

    Ok(())
  }

  #[test]
  fn a_directory_the_caller_walks_past_is_listed_and_not_opened() -> Result<(), Failure> {
    let at = tree("listing-past")?;
    let found = under(&at, |path| {
      match path.file_name().and_then(std::ffi::OsStr::to_str) {
        Some("skipped") => Descend::Past,
        Some(_) | None => Descend::Into,
      }
    })?;

    assert!(found.contains(&PathBuf::from("skipped")), "{found:?}");
    assert!(
      !found.contains(&PathBuf::from("skipped/inside")),
      "{found:?}"
    );

    Ok(())
  }

  #[test]
  fn a_link_back_up_the_tree_is_listed_and_not_followed() -> Result<(), Failure> {
    let at = tree("listing-link")?;
    let found = under(&at, |_| Descend::Into)?;

    assert!(found.contains(&PathBuf::from("a/back-up")), "{found:?}");
    assert!(
      !found.contains(&PathBuf::from("a/back-up/top")),
      "{found:?}"
    );

    Ok(())
  }

  #[test]
  fn the_files_are_everything_but_the_directories_and_come_back_sorted() -> Result<(), Failure> {
    let at = tree("listing-files")?;
    let Ok(files) = files(&at);
    let expected = vec![
      at.join("a/b/c/deep"),
      at.join("a/back-up"),
      at.join("skipped/inside/hidden"),
      at.join("top"),
    ];

    assert_eq!(files, expected);

    Ok(())
  }

  #[test]
  fn a_directory_replaced_by_a_link_during_descent_is_refused() -> Result<(), Failure> {
    let at = console_core_temporary_directories::fresh("listing-replaced")?;
    let outside = console_core_temporary_directories::fresh("listing-outside")?;
    let child = at.join("child");

    std::fs::create_dir(&child)?;
    console_core_atomic_writes::whole(&outside.join("hidden"), b"")?;

    let Ok(walk) = recursive(&at, |path| {
      let replaced =
        std::fs::remove_dir(path).and_then(|()| std::os::unix::fs::symlink(&outside, path));

      assert!(matches!(replaced, Ok(())), "{replaced:?}");

      Descend::Into
    });
    let found: Vec<Entry> = walk.collect();

    assert!(matches!(
      found.as_slice(),
      [Ok(_path), Err(_refused_directory)]
    ));

    for entry in found {
      match entry {
        Ok(path) => assert_eq!(path, child),
        Err(fault) => assert_eq!(fault.directory, child),
      }
    }

    Ok(())
  }

  #[test]
  fn links_to_missing_files_are_still_files() -> Result<(), Failure> {
    let at = console_core_temporary_directories::fresh("listing-dangling")?;
    let link = at.join("dangling");

    std::os::unix::fs::symlink(at.join("missing"), &link)?;

    let Ok(found) = files(&at);

    assert_eq!(found, vec![link]);

    Ok(())
  }

  #[test]
  fn names_that_are_not_utf8_are_preserved() -> Result<(), Failure> {
    let at = console_core_temporary_directories::fresh("listing-non-utf8")?;
    let name = std::ffi::OsStr::from_bytes(b"\xff");
    #[cfg_attr(
      dylint_lib = "explicit086_no_climbing_join",
      expect(
        explicit086_no_climbing_join,
        reason = "the name is the single byte 0xff written in this test"
      )
    )]
    let path = at.join(name);

    console_core_atomic_writes::whole(&path, b"")?;

    let Ok(found) = files(&at);

    assert_eq!(found, vec![path]);

    Ok(())
  }

  #[test]
  fn a_directory_that_is_not_there_is_one_entry_saying_so() {
    let Ok(walk) = recursive(Path::new("/nowhere/at/all"), |_| Descend::Into);
    let found: Vec<Entry> = walk.collect();

    assert_eq!(found.len(), 1);
    assert!(matches!(found.first(), Some(Err(Unlisted { .. }))));
  }
}
