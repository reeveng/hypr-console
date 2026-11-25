//! Writing a file so that a machine which stops has either the old one or the
//! new one, and reading one so that a file which will not be read says so.
//!
//! Two halves of one fault, and the second is the one that hid the first.
//!
//! # The writing
//!
//! Nothing in this workspace called `fsync` before this crate existed. What
//! everything did instead was one of two things. The better of them -- write
//! beside the file, rename over it -- is atomic against a *reader*: a rename
//! replaces a name, so anything reading gets all of one file or all of the
//! other and never half of either. That is what it was chosen for and it does
//! it perfectly.
//!
//! It says nothing about power. A rename is a change to a directory, and the
//! bytes of the file it names are somewhere else; a kernel is free to have
//! committed the first and not the second when the machine stops. What comes
//! back is a name that resolves to a file of no length. On a handheld that is
//! not a thought experiment -- it is the battery running out, which is a thing
//! this device does.
//!
//! The worse of the two is `fs::write` straight over the live file. That one
//! is not even atomic against a reader: it truncates and then fills, so there
//! is a window in which the file genuinely is half of itself, and a machine
//! that stops inside that window leaves it that way for good.
//!
//! So there is one way to write a file here, and it does four things in an
//! order that matters:
//!
//!   1. write the whole of it beside where it goes,
//!   2. `sync_all`, so those bytes are on the disk rather than promised,
//!   3. rename it over the live name, which is the moment it happens,
//!   4. `sync_all` the *directory*, so the rename itself is on the disk.
//!
//! Step four is the one that looks superfluous and is not. Without it the file
//! is durable and the fact that it has the live name is not, so a machine that
//! stops can come back with the new bytes sitting under the staging name and
//! the old file still in place -- which is safe, and is also an apply that
//! reported success and did not happen.
//! Session observations can use [`publish`] to make the same atomic name
//! replacement while the process is running, without the persistence steps.
//!
//! # The reading
//!
//! The write side is why a file can be torn. The read side is why no one ever
//! found out.
//!
//! Every one of these files was read with `.ok()` or `unwrap_or_default`, which
//! turns *every* way of failing into the same answer as an empty file. A
//! setting whose file will not open, a setting whose file is half written and a
//! setting no one has ever chosen are three different facts, and all three came
//! back as the third. What that looks like from the outside is a machine that
//! quietly went back to a default, at a moment no one can identify, for a
//! reason no one can recover.
//!
//! `Stored` keeps them apart. A file that is not there is ordinary and means the
//! default. A file that is there and will not be read is a fault, and the
//! caller is handed it rather than a shrug. That is the same rule EXPLICIT006
//! is written for -- an error is not an absence -- applied to the one place it
//! was being broken by a convenience method rather than by a cast.
//!
//! # Which of the four steps it was
//!
//! The write is four things in an order, and for a long time all four failed
//! the same way: a sentence with the path at the front of it. `Unwritten` names
//! them apart, because they are not the same fault. A file that will not be
//! made is a place this desktop may not write; bytes that will not go down is a
//! disk that is full; a rename that will not happen is the old file still being
//! the live one, which is a write that did not take; and a directory that will
//! not sync is a write that did take and may not survive the power going. The
//! sentence each of them used to print is the `Display` arm, so the journal
//! reads as it did and a caller that wants to tell the full disk from the
//! forbidden directory now can.
//!
//! # Two writers at once
//!
//! The staging name used to be one fixed name beside the file, which is only
//! a staging name while there is one writer. Two processes writing one file at
//! the same moment both made that one path, the second truncated the first's
//! half-written bytes, and whichever renamed first could put a torn file in
//! place -- the very thing this crate exists to rule out. So the name carries
//! the process and the thread doing the writing, which are the two things that
//! can be doing it at once. And the file that goes in keeps the mode and the
//! owner of the one it replaces, because a rename puts a new file under the
//! old name rather than new bytes into the old file, and a file that was 0600
//! is not meant to come back 0644 because somebody changed a line in it.
//!
//! # A name nobody has yet
//!
//! A copy made beside a file -- the picture an edit writes -- is somebody's
//! new file rather than a new version of one, and a name that turned up
//! between choosing it and writing it belongs to whoever made it. So the same
//! four steps can end in a rename that refuses a name already taken, which is
//! `RENAME_NOREPLACE`, rather than one that replaces it: the check and the
//! naming are one call, and there is no moment between them to lose.
//!
//! # The directory held by a write
//!
//! A transaction opens its parent without following symlinks, then stages,
//! reads ownership, renames, cleans up and syncs through that same descriptor.
//! Renaming the directory while the write runs cannot redirect those steps.
//! File opens refuse symlinks too; removing a symlink removes only the link.
//! [`read_bytes`] reads a regular file through a symlink-free path, up to
//! [`MAX_TEXT_BYTES`]. [`read_prefix`] uses the same open rules when a
//! caller needs only a bounded prefix of a larger file.
//! Reading accepts regular files, including procfs and sysfs regular files,
//! and refuses text larger than `MAX_TEXT_BYTES` rather than accepting a prefix.
//! New files request mode 0644 and new directories 0755, narrowed by the
//! process umask. Replacing a file preserves its existing mode and owner.

mod location;

use console_core_never::Never;
use location::Location;
use rustix::fs::{CWD, FileType, Mode, OFlags, RenameFlags};
use rustix::io::Errno;
use std::ffi::OsString;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub const BESIDE: &str = "console-writing";

pub const MAX_TEXT_BYTES: u32 = 16_777_216;

pub fn beside(live: &Path) -> Result<PathBuf, Never> {
  let mut name = match live.file_name() {
    Some(name) => name.to_os_string(),
    None => OsString::from("file"),
  };

  let process = rustix::process::getpid().as_raw_pid();
  let thread = rustix::thread::gettid().as_raw_pid();

  name.push(format!(".{BESIDE}.{process}.{thread}"));

  Ok(live.with_file_name(name))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stored {
  Text(String),
  Absent,
  Failed(String),
}

impl Stored {
  pub fn text(self) -> Result<Option<String>, Never> {
    Ok(match self {
      Stored::Text(text) => Some(text),
      Stored::Absent | Stored::Failed(_) => None,
    })
  }
}

pub fn read(at: &Path) -> Result<Stored, Never> {
  Ok(match read_text(at) {
    Ok(text) => Stored::Text(text),
    Err(fault) => {
      match ::core::primitive::bool::then_some(fault.kind() == std::io::ErrorKind::NotFound, ()) {
        ::core::option::Option::Some(()) => Stored::Absent,
        ::core::option::Option::None => Stored::Failed(fault.to_string()),
      }
    }
  })
}

pub fn read_text(at: &Path) -> io::Result<String> {
  let bytes = read_bytes(at)?;

  String::from_utf8(bytes).map_err(|fault| io::Error::new(io::ErrorKind::InvalidData, fault))
}

pub fn read_bytes(at: &Path) -> io::Result<Vec<u8>> {
  let bytes = bytes_up_to(at, u64::from(MAX_TEXT_BYTES).saturating_add(1))?;
  let length = u64::try_from(bytes.len()).map_err(|_too_wide| io::Error::from(Errno::FBIG))?;

  match bool::then_some(length <= u64::from(MAX_TEXT_BYTES), ()) {
    Some(()) => Ok(bytes),
    None => Err(io::Error::from(Errno::FBIG)),
  }
}

pub fn read_prefix(at: &Path, limit: u32) -> io::Result<Vec<u8>> {
  bytes_up_to(at, u64::from(limit))
}

#[doc = "Open a lock file and create it if needed, retaining the inode and bytes of an existing file."]
pub fn open_lock_file(at: &Path) -> io::Result<File> {
  let flags = OFlags::CREATE | OFlags::WRONLY | OFlags::CLOEXEC;
  let mode = Mode::RUSR | Mode::WUSR;
  let descriptor = rustix::fs::open(at, flags, mode)?;

  Ok(File::from(descriptor))
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the opened descriptor is nonblocking and must be a regular file. The byte count is bounded at the reader, while completion of filesystem I/O remains the kernel's responsibility"
  )
)]
fn bytes_up_to(at: &Path, limit: u64) -> io::Result<Vec<u8>> {
  let target = Location::new(at)?;
  let descriptor = target.open(OFlags::RDONLY | OFlags::NONBLOCK, Mode::empty())?;
  let metadata = rustix::fs::fstat(&descriptor)?;

  match FileType::from_raw_mode(metadata.st_mode) {
    FileType::RegularFile => {}
    FileType::Directory => return Err(io::Error::from(Errno::ISDIR)),
    FileType::Symlink
    | FileType::Fifo
    | FileType::Socket
    | FileType::CharacterDevice
    | FileType::BlockDevice
    | FileType::Unknown => return Err(io::Error::from(Errno::INVAL)),
  }

  let mut reader = File::from(descriptor).take(limit);
  let mut bytes = Vec::new();

  reader.read_to_end(&mut bytes)?;

  Ok(bytes)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unread(pub PathBuf, pub String);

impl fmt::Display for Unread {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    let Unread(at, fault) = self;

    write!(to, "{}: {fault}", at.display())
  }
}

impl std::error::Error for Unread {}

pub fn text_or_empty(at: &Path) -> Result<String, Unread> {
  let Ok(stored) = read(at);

  match stored {
    Stored::Text(text) => Ok(text),
    Stored::Absent => Ok(String::new()),
    Stored::Failed(fault) => Err(Unread(at.to_path_buf(), fault)),
  }
}

pub fn number<Number: std::str::FromStr>(at: &Path) -> Result<Option<Number>, Never> {
  let Ok(stored) = read(at);
  let Ok(text) = stored.text();

  Ok(match text.map(|said| said.trim().parse::<Number>()) {
    Some(Ok(number)) => Some(number),
    None => None,
    Some(Err(_not_a_number)) => None,
  })
}

#[derive(Debug)]
pub enum Unwritten {
  Making(PathBuf, std::io::Error),
  Filling(PathBuf, std::io::Error),
  Settling(PathBuf, std::io::Error),
  Moving(PathBuf, std::io::Error),
  Keeping(PathBuf, std::io::Error),
  Naming(PathBuf, std::io::Error),
  Cleaning(PathBuf, Box<Unwritten>, std::io::Error),
}

impl fmt::Display for Unwritten {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Unwritten::Making(at, fault) => write!(to, "{}: making it: {fault}", at.display()),
      Unwritten::Filling(at, fault) => write!(to, "{}: writing it: {fault}", at.display()),
      Unwritten::Settling(at, fault) => {
        write!(to, "{}: putting it on the disk: {fault}", at.display())
      }
      Unwritten::Moving(at, fault) => {
        write!(to, "{}: moving it into place: {fault}", at.display())
      }
      Unwritten::Keeping(at, fault) => {
        write!(to, "{}: keeping its mode and owner: {fault}", at.display())
      }
      Unwritten::Naming(holding, fault) => write!(
        to,
        "{}: telling the disk about the new name: {fault}",
        holding.display()
      ),
      Unwritten::Cleaning(at, original, fault) => write!(
        to,
        "{original}; {}: removing its staging file: {fault}",
        at.display()
      ),
    }
  }
}

impl std::error::Error for Unwritten {}

pub fn whole(at: &Path, bytes: &[u8]) -> Result<(), Unwritten> {
  let target = Location::new(at)
    .map_err(|fault| Unwritten::Making(at.to_path_buf(), io::Error::from(fault)))?;

  commit(&target, at, bytes, Replacement::Allowed)
}

#[doc = "Replace a runtime observation atomically without waiting for it to reach persistent storage."]
pub fn publish(at: &Path, bytes: &[u8]) -> Result<(), Unwritten> {
  let target = Location::new(at)
    .map_err(|fault| Unwritten::Making(at.to_path_buf(), io::Error::from(fault)))?;

  commit(&target, at, bytes, Replacement::Published)
}

pub fn whole_with_permissions(
  at: &Path,
  bytes: &[u8],
  permissions: std::fs::Permissions,
) -> Result<(), Unwritten> {
  use std::os::unix::fs::PermissionsExt;

  let target = Location::new(at)
    .map_err(|fault| Unwritten::Making(at.to_path_buf(), io::Error::from(fault)))?;
  let mode = Mode::from_raw_mode(permissions.mode());

  commit(&target, at, bytes, Replacement::WithMode(mode))
}

pub fn whole_without_overwriting(at: &Path, bytes: &[u8]) -> Result<(), Unwritten> {
  let target = Location::new(at)
    .map_err(|fault| Unwritten::Making(at.to_path_buf(), io::Error::from(fault)))?;

  commit(&target, at, bytes, Replacement::Rejected)
}

pub fn whole_with_folders(at: &Path, bytes: &[u8]) -> Result<(), Unwritten> {
  whole_in_folders(at, bytes, Replacement::Allowed)
}

pub fn whole_with_folders_and_permissions(
  at: &Path,
  bytes: &[u8],
  permissions: std::fs::Permissions,
) -> Result<(), Unwritten> {
  use std::os::unix::fs::PermissionsExt;

  let mode = Mode::from_raw_mode(permissions.mode());
  whole_in_folders(at, bytes, Replacement::WithMode(mode))
}

pub fn whole_in_new_file_with_folders(at: &Path, bytes: &[u8]) -> Result<(), Unwritten> {
  whole_in_folders(at, bytes, Replacement::Rejected)
}

fn whole_in_folders(at: &Path, bytes: &[u8], replacement: Replacement) -> Result<(), Unwritten> {
  let name = at
    .file_name()
    .ok_or_else(|| Unwritten::Making(at.to_path_buf(), io::Error::from(Errno::INVAL)))?;
  let parent = match at.parent() {
    Some(parent) => parent,
    None => Path::new("."),
  };
  let directory = location::make_directories(parent)
    .map_err(|fault| Unwritten::Making(parent.to_path_buf(), io::Error::from(fault)))?;
  let target = Location {
    directory,
    name: name.to_os_string(),
  };

  commit(&target, at, bytes, replacement)
}

pub fn rename_without_overwriting(from: &Path, to: &Path) -> Result<(), io::Error> {
  rustix::fs::renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE).map_err(io::Error::from)
}

pub fn write_and_sync(at: &Path, bytes: &[u8]) -> Result<(), Unwritten> {
  let target = Location::new(at)
    .map_err(|fault| Unwritten::Making(at.to_path_buf(), io::Error::from(fault)))?;
  let descriptor = target
    .open(
      OFlags::WRONLY | OFlags::CREATE | OFlags::TRUNC,
      Mode::from_raw_mode(0o644),
    )
    .map_err(|fault| Unwritten::Making(at.to_path_buf(), io::Error::from(fault)))?;
  let mut file = File::from(descriptor);

  file
    .write_all(bytes)
    .map_err(|fault| Unwritten::Filling(at.to_path_buf(), fault))?;

  rustix::fs::fsync(&file)
    .map_err(|fault| Unwritten::Settling(at.to_path_buf(), io::Error::from(fault)))
}

#[derive(Clone, Copy)]
enum Replacement {
  Allowed,
  Published,
  WithMode(Mode),
  Rejected,
}

fn commit(
  target: &Location,
  at: &Path,
  bytes: &[u8],
  replacement: Replacement,
) -> Result<(), Unwritten> {
  let Ok(staged) = beside(at);
  let name = staged
    .file_name()
    .ok_or_else(|| Unwritten::Making(staged.clone(), io::Error::from(Errno::INVAL)))?;

  stage_and_sync(target, &staged, bytes, replacement)?;

  let flags = match replacement {
    Replacement::Allowed | Replacement::Published | Replacement::WithMode(_) => {
      RenameFlags::empty()
    }
    Replacement::Rejected => RenameFlags::NOREPLACE,
  };

  match rustix::fs::renameat_with(
    &target.directory,
    name,
    &target.directory,
    &target.name,
    flags,
  ) {
    Ok(()) => {}
    Err(fault) => {
      return clear_after(
        target,
        &staged,
        Unwritten::Moving(at.to_path_buf(), io::Error::from(fault)),
      );
    }
  }

  match replacement {
    Replacement::Published => Ok(()),
    Replacement::Allowed | Replacement::WithMode(_) | Replacement::Rejected => {
      sync_renamed_directory(target, at)
    }
  }
}

fn sync_renamed_directory(target: &Location, at: &Path) -> Result<(), Unwritten> {
  rustix::fs::fsync(&target.directory).map_err(|fault| {
    let parent = at.parent().filter(|path| !path.as_os_str().is_empty());
    let holding = match parent {
      Some(path) => path,
      None => Path::new("."),
    };

    Unwritten::Naming(holding.to_path_buf(), io::Error::from(fault))
  })
}

fn stage_and_sync(
  target: &Location,
  at: &Path,
  bytes: &[u8],
  replacement: Replacement,
) -> Result<(), Unwritten> {
  let name = at
    .file_name()
    .ok_or_else(|| Unwritten::Making(at.to_path_buf(), io::Error::from(Errno::INVAL)))?;
  let mode = match replacement {
    Replacement::WithMode(_) => Mode::RUSR | Mode::WUSR,
    Replacement::Allowed | Replacement::Published | Replacement::Rejected => {
      Mode::from_raw_mode(0o644)
    }
  };
  let descriptor = target
    .open_name(name, OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL, mode)
    .map_err(|fault| Unwritten::Making(at.to_path_buf(), io::Error::from(fault)))?;
  let mut file = File::from(descriptor);

  let written = fill_staged(&mut file, target, at, bytes, replacement);

  match written {
    Ok(()) => Ok(()),
    Err(fault) => clear_after(target, at, fault),
  }
}

fn clear_after(target: &Location, at: &Path, original: Unwritten) -> Result<(), Unwritten> {
  let name = match at.file_name() {
    Some(name) => name,
    None => {
      return Err(Unwritten::Cleaning(
        at.to_path_buf(),
        Box::new(original),
        io::Error::from(Errno::INVAL),
      ));
    }
  };

  match remove_at(target, name) {
    Ok(()) => Err(original),
    Err(cleanup) => Err(Unwritten::Cleaning(
      at.to_path_buf(),
      Box::new(original),
      cleanup,
    )),
  }
}

fn fill_staged(
  file: &mut File,
  target: &Location,
  at: &Path,
  bytes: &[u8],
  replacement: Replacement,
) -> Result<(), Unwritten> {
  match replacement {
    Replacement::Allowed | Replacement::Published => {
      copy_ownership(target, &at.with_file_name(&target.name), file)?
    }
    Replacement::WithMode(_) | Replacement::Rejected => {}
  }

  file
    .write_all(bytes)
    .map_err(|fault| Unwritten::Filling(at.to_path_buf(), fault))?;

  match replacement {
    Replacement::WithMode(mode) => rustix::fs::fchmod(&*file, mode).map_err(|fault| {
      Unwritten::Keeping(at.with_file_name(&target.name), io::Error::from(fault))
    })?,
    Replacement::Allowed | Replacement::Published | Replacement::Rejected => {}
  }

  match replacement {
    Replacement::Published => Ok(()),
    Replacement::Allowed | Replacement::WithMode(_) | Replacement::Rejected => {
      rustix::fs::fsync(file)
        .map_err(|fault| Unwritten::Settling(at.to_path_buf(), io::Error::from(fault)))
    }
  }
}

fn copy_ownership(target: &Location, live: &Path, staged: &File) -> Result<(), Unwritten> {
  let was = match target.metadata() {
    Ok(was) => was,
    Err(fault) => match ::core::primitive::bool::then_some(fault == Errno::NOENT, ()) {
      ::core::option::Option::Some(()) => return Ok(()),
      ::core::option::Option::None => {
        return Err(Unwritten::Keeping(
          live.to_path_buf(),
          io::Error::from(fault),
        ));
      }
    },
  };

  match ::core::primitive::bool::then_some(
    FileType::from_raw_mode(was.st_mode) == FileType::Symlink,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Err(Unwritten::Keeping(
        live.to_path_buf(),
        io::Error::from(Errno::LOOP),
      ));
    }
    ::core::option::Option::None => {}
  }

  let keeping = |fault| Unwritten::Keeping(live.to_path_buf(), fault);

  let made = rustix::fs::fstat(staged)
    .map_err(io::Error::from)
    .map_err(keeping)?;

  match (made.st_uid == was.st_uid, made.st_gid == was.st_gid) {
    (true, true) => {}
    (_, _) => rustix::fs::fchown(
      staged,
      Some(rustix::process::Uid::from_raw(was.st_uid)),
      Some(rustix::process::Gid::from_raw(was.st_gid)),
    )
    .map_err(std::io::Error::from)
    .map_err(keeping)?,
  }

  rustix::fs::fchmod(staged, Mode::from_raw_mode(was.st_mode))
    .map_err(io::Error::from)
    .map_err(keeping)
}

pub fn sync_parent_directory(at: &Path) -> Result<(), Unwritten> {
  let holding = match at.parent() {
    Some(holding) => match ::core::primitive::bool::then_some(holding.as_os_str().is_empty(), ())
    {
      ::core::option::Option::Some(()) => Path::new("."),
      ::core::option::Option::None => holding,
    },
    None => return Ok(()),
  };

  let directory = location::open_directory(holding)
    .map_err(|fault| Unwritten::Naming(holding.to_path_buf(), io::Error::from(fault)))?;

  rustix::fs::fsync(directory)
    .map_err(|fault| Unwritten::Naming(holding.to_path_buf(), io::Error::from(fault)))
}

#[derive(Debug)]
pub struct Ungone(pub PathBuf, pub std::io::Error);

impl fmt::Display for Ungone {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    let Ungone(at, fault) = self;

    write!(to, "{} will not go away: {fault}", at.display())
  }
}

impl std::error::Error for Ungone {}

pub fn gone(at: &Path) -> Result<(), Ungone> {
  let target = match Location::new(at) {
    Ok(target) => target,
    Err(fault) => match ::core::primitive::bool::then_some(fault == Errno::NOENT, ()) {
      ::core::option::Option::Some(()) => return Ok(()),
      ::core::option::Option::None => {
        return Err(Ungone(at.to_path_buf(), io::Error::from(fault)));
      }
    },
  };

  remove_at(&target, &target.name).map_err(|fault| Ungone(at.to_path_buf(), fault))
}

fn remove_at(target: &Location, name: &std::ffi::OsStr) -> io::Result<()> {
  match target.remove(name) {
    Ok(()) => Ok(()),
    Err(fault) => match ::core::primitive::bool::then_some(fault == Errno::NOENT, ()) {
      ::core::option::Option::Some(()) => Ok(()),
      ::core::option::Option::None => Err(io::Error::from(fault)),
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;

  fn somewhere(named: &str) -> Result<PathBuf, Box<dyn Error>> {
    let here = console_core_temporary_directories::fresh(&format!("writing-{named}"))?;

    Ok(here)
  }

  fn metadata(at: &Path) -> io::Result<rustix::fs::Stat> {
    let target = Location::new(at)?;

    target.metadata().map_err(io::Error::from)
  }

  fn mkdir(at: &Path) -> io::Result<()> {
    let target = Location::new(at)?;

    rustix::fs::mkdirat(&target.directory, &target.name, Mode::from_raw_mode(0o755))
      .map_err(io::Error::from)
  }

  fn symlink(source: &Path, at: &Path) -> io::Result<()> {
    let target = Location::new(at)?;

    rustix::fs::symlinkat(source, &target.directory, &target.name).map_err(io::Error::from)
  }

  fn chmod(at: &Path, mode: Mode) -> io::Result<()> {
    let target = Location::new(at)?;
    let fd = target.open(OFlags::WRONLY, Mode::empty())?;

    rustix::fs::fchmod(fd, mode).map_err(io::Error::from)
  }

  fn directory_names(at: &Path) -> io::Result<Vec<OsString>> {
    use std::os::unix::ffi::OsStringExt;

    let directory = location::open_directory(at)?;
    let entries = rustix::fs::Dir::new(directory)?;

    entries
      .filter_map(|entry| match entry {
        Ok(found) => match found.file_name().to_bytes() {
          b"." | b".." => None,
          name => Some(Ok(OsString::from_vec(name.to_vec()))),
        },
        Err(fault) => Some(Err(io::Error::from(fault))),
      })
      .collect()
  }

  #[test]
  fn a_transaction_keeps_the_directory_it_opened_after_its_path_is_replaced()
  -> Result<(), Box<dyn Error>> {
    let here = somewhere("parent-swapped")?;
    let parent = here.join("parent");
    let moved = here.join("moved");
    let outside = here.join("outside");

    mkdir(&parent)?;
    mkdir(&outside)?;
    whole(&parent.join("live"), b"original")?;
    whole(&outside.join("live"), b"untouched")?;

    let at = parent.join("live");
    let target = Location::new(&at)?;
    let source = Location::new(&parent)?;
    let destination = Location::new(&moved)?;

    rustix::fs::renameat(
      &source.directory,
      &source.name,
      &destination.directory,
      &destination.name,
    )?;
    symlink(&outside, &parent)?;
    commit(&target, &at, b"replacement", Replacement::Allowed)?;

    let written = read_bytes(&moved.join("live"))?;
    let untouched = read_bytes(&outside.join("live"))?;
    let moved_names = directory_names(&moved)?;
    let outside_names = directory_names(&outside)?;

    assert_eq!(written, b"replacement");
    assert_eq!(untouched, b"untouched");
    assert_eq!(moved_names, vec![OsString::from("live")]);
    assert_eq!(outside_names, vec![OsString::from("live")]);

    Ok(())
  }

  #[test]
  fn a_symlink_in_a_parent_path_cannot_redirect_any_operation() -> Result<(), Box<dyn Error>> {
    let here = somewhere("parent-link")?;
    let outside = here.join("outside");
    let link = here.join("link");

    mkdir(&outside)?;
    whole(&outside.join("live"), b"untouched")?;
    symlink(&outside, &link)?;

    let through = link.join("live");

    assert!(matches!(
      whole(&through, b"changed"),
      Err(Unwritten::Making(..))
    ));
    assert!(matches!(
      whole_without_overwriting(&through, b"changed"),
      Err(Unwritten::Making(..))
    ));
    assert!(matches!(
      write_and_sync(&through, b"changed"),
      Err(Unwritten::Making(..))
    ));
    assert!(matches!(
      whole_with_folders(&link.join("new/thing"), b"changed"),
      Err(Unwritten::Making(..))
    ));
    assert!(matches!(gone(&through), Err(Ungone(..))));
    assert!(matches!(
      sync_parent_directory(&through),
      Err(Unwritten::Naming(..))
    ));

    let Ok(stored) = read(&through);
    let untouched = read_bytes(&outside.join("live"))?;
    let outside_names = directory_names(&outside)?;

    assert!(matches!(stored, Stored::Failed(_fault)));
    assert_eq!(untouched, b"untouched");
    assert_eq!(outside_names, vec![OsString::from("live")]);

    Ok(())
  }

  #[test]
  fn file_symlinks_are_not_read_or_truncated_and_removal_only_unlinks_them()
  -> Result<(), Box<dyn Error>> {
    let here = somewhere("file-link")?;
    let victim = here.join("victim");
    let link = here.join("link");

    whole(&victim, b"untouched")?;
    symlink(&victim, &link)?;

    let Ok(stored) = read(&link);
    let prefix = read_prefix(&victim, 3)?;

    assert!(matches!(stored, Stored::Failed(_fault)));
    assert!(matches!(read_prefix(&link, 3), Err(_symlink)));
    assert_eq!(prefix, b"unt");
    assert!(matches!(
      whole(&link, b"changed"),
      Err(Unwritten::Keeping(..))
    ));
    assert!(matches!(
      write_and_sync(&link, b"changed"),
      Err(Unwritten::Making(..))
    ));

    gone(&link)?;

    let untouched = read_bytes(&victim)?;
    let names = directory_names(&here)?;

    assert_eq!(untouched, b"untouched");
    assert_eq!(names, vec![OsString::from("victim")]);

    Ok(())
  }

  #[test]
  fn opened_files_and_directories_close_on_exec() -> Result<(), Box<dyn Error>> {
    let here = somewhere("cloexec")?;
    let at = here.join("file");

    whole(&at, b"data")?;

    let target = Location::new(&at)?;
    let descriptor = target.open(OFlags::RDONLY, Mode::empty())?;
    let parent_flags = rustix::io::fcntl_getfd(&target.directory)?;
    let file_flags = rustix::io::fcntl_getfd(descriptor)?;

    assert!(parent_flags.contains(rustix::io::FdFlags::CLOEXEC));
    assert!(file_flags.contains(rustix::io::FdFlags::CLOEXEC));

    Ok(())
  }

  #[test]
  fn non_utf8_file_names_keep_their_own_staging_names() -> Result<(), Box<dyn Error>> {
    use std::os::unix::ffi::OsStrExt;

    let here = somewhere("non-utf8")?;
    let at = here
      .join("file")
      .with_file_name(std::ffi::OsStr::from_bytes(b"\xff"));

    whole(&at, b"data")?;

    let Ok(staged) = beside(&at);
    let filename = staged.file_name().ok_or(io::Error::from(Errno::INVAL))?;
    let written = read_bytes(&at)?;

    assert!(filename.as_bytes().starts_with(b"\xff.console-writing."));
    assert_eq!(written, b"data");

    Ok(())
  }

  #[test]
  fn the_text_limit_accepts_an_exact_fit_and_refuses_a_prefix_of_an_oversized_file()
  -> Result<(), Box<dyn Error>> {
    let here = somewhere("read-limit")?;
    let at = here.join("file");

    whole(&at, b"")?;

    let target = Location::new(&at)?;
    let file = target.open(OFlags::WRONLY, Mode::empty())?;

    rustix::fs::ftruncate(&file, u64::from(MAX_TEXT_BYTES))?;

    let exact = read_bytes(&at)?;
    let length = u32::try_from(exact.len())?;

    assert_eq!(length, MAX_TEXT_BYTES);

    rustix::fs::ftruncate(&file, u64::from(MAX_TEXT_BYTES).saturating_add(1))?;

    let refused = read_bytes(&at).map_err(|fault| fault.kind());
    let prefix = read_prefix(&at, 3)?;
    let Ok(stored) = read(&at);

    assert_eq!(refused, Err(io::ErrorKind::FileTooLarge));
    assert_eq!(prefix, [0; 3]);
    assert!(matches!(stored, Stored::Failed(_too_long)));

    Ok(())
  }

  #[test]
  fn invalid_utf8_is_a_read_failure_instead_of_replacement_text() -> Result<(), Box<dyn Error>> {
    let here = somewhere("invalid-text")?;
    let at = here.join("file");

    whole(&at, b"\xff")?;

    let Ok(stored) = read(&at);

    assert!(matches!(stored, Stored::Failed(_not_text)));

    Ok(())
  }

  #[test]
  fn a_file_that_was_never_there_is_already_gone() -> Result<(), Box<dyn Error>> {
    let here = somewhere("gone")?;
    let at = here.join("never");

    gone(&at)?;
    whole(&at, b"here")?;
    gone(&at)?;

    assert_eq!(
      read_bytes(&at).map_err(|fault| fault.kind()),
      Err(io::ErrorKind::NotFound)
    );

    Ok(())
  }

  #[test]
  fn a_name_somebody_already_has_is_refused_rather_than_written_over()
  -> Result<(), Box<dyn Error>> {
    let here = somewhere("without-overwriting")?;
    let at = here.join("beach edited.png");

    whole_without_overwriting(&at, b"first")?;

    let second = whole_without_overwriting(&at, b"second");
    let kept = read_bytes(&at)?;
    let left = directory_names(&here)?;

    let refused = match &second {
      Err(Unwritten::Moving(_, fault)) => Some(fault.kind()),
      Ok(())
      | Err(
        Unwritten::Making(..)
        | Unwritten::Filling(..)
        | Unwritten::Settling(..)
        | Unwritten::Keeping(..)
        | Unwritten::Naming(..)
        | Unwritten::Cleaning(..),
      ) => None,
    };

    assert_eq!(
      refused,
      Some(std::io::ErrorKind::AlreadyExists),
      "{second:?}"
    );
    assert_eq!(kept, b"first");
    assert_eq!(
      left,
      vec![OsString::from("beach edited.png")],
      "the staged copy was left beside it"
    );

    Ok(())
  }

  #[test]
  fn the_folders_a_file_goes_in_are_made_for_it() -> Result<(), Box<dyn Error>> {
    let here = somewhere("folders")?;
    let at = here.join("one/two/thing");

    whole_with_folders(&at, b"deep")?;

    let written = read_bytes(&at)?;

    assert_eq!(written, b"deep");

    Ok(())
  }

  #[test]
  fn a_file_that_is_not_there_reads_as_nothing_and_one_that_cannot_be_read_says_so()
  -> Result<(), Box<dyn Error>> {
    let here = somewhere("unread")?;

    assert_eq!(text_or_empty(&here.join("never")), Ok(String::new()));
    assert!(
      matches!(text_or_empty(&here), Err(Unread(..))),
      "a folder read as a file"
    );

    Ok(())
  }

  #[test]
  fn a_number_kept_in_a_file_is_read_back_and_anything_else_is_none() -> Result<(), Box<dyn Error>>
  {
    let here = somewhere("number")?;

    whole(&here.join("kept"), b"42\n")?;
    whole(&here.join("words"), b"forty-two")?;

    assert_eq!(number::<u32>(&here.join("kept")), Ok(Some(42)));
    assert_eq!(number::<u32>(&here.join("words")), Ok(None));
    assert_eq!(number::<u32>(&here.join("never")), Ok(None));

    Ok(())
  }

  #[test]
  fn a_folder_is_not_a_file_to_take_away() -> Result<(), Box<dyn Error>> {
    let at = somewhere("folder")?;

    assert!(
      matches!(gone(&at), Err(Ungone(..))),
      "a folder was answered as gone"
    );

    Ok(())
  }

  #[test]
  fn a_file_is_written_and_is_what_was_written() -> Result<(), Box<dyn Error>> {
    let here = somewhere("plain")?;
    let at = here.join("thing");

    whole(&at, b"what it says")?;

    let written = read_bytes(&at)?;

    assert_eq!(written, b"what it says");
    Ok(())
  }

  #[test]
  fn what_waits_is_in_the_directory_it_is_going_into() {
    let live = Path::new("/usr/local/bin/launcher");
    let Ok(beside) = beside(live);

    assert_eq!(beside.parent(), live.parent());
  }

  #[test]
  fn nothing_is_left_beside_the_file_afterwards() -> Result<(), Box<dyn Error>> {
    let here = somewhere("tidy")?;
    let at = here.join("thing");

    whole(&at, b"one")?;

    let Ok(beside) = beside(&at);

    assert_eq!(
      metadata(&beside)
        .map(|_metadata| ())
        .map_err(|fault| fault.kind()),
      Err(io::ErrorKind::NotFound)
    );
    Ok(())
  }

  #[test]
  fn writing_over_a_file_replaces_all_of_it() -> Result<(), Box<dyn Error>> {
    let here = somewhere("over")?;
    let at = here.join("thing");

    whole(&at, b"a long first version")?;
    whole(&at, b"short")?;

    let written = read_bytes(&at)?;

    assert_eq!(written, b"short");
    Ok(())
  }

  #[test]
  fn a_write_that_fails_leaves_the_old_file_and_no_litter() -> Result<(), Box<dyn Error>> {
    let here = somewhere("cannot")?;
    let at = here.join("thing");

    whole(&at, b"the one that was there")?;

    let Ok(beside) = beside(&at);

    mkdir(&beside)?;

    let through = match whole(&at, b"the new one") {
      Ok(()) => "it wrote through an obstacle",
      Err(_unwritten) => "it stopped at the obstacle",
    };

    assert_eq!(through, "it stopped at the obstacle");
    let written = read_bytes(&at)?;

    assert_eq!(written, b"the one that was there");
    Ok(())
  }

  #[test]
  fn a_staging_symlink_is_refused_and_neither_target_is_changed() -> Result<(), Box<dyn Error>> {
    let here = somewhere("staging-symlink")?;
    let at = here.join("live");
    let victim = here.join("victim");

    whole(&at, b"original")?;
    whole(&victim, b"untouched")?;

    let Ok(staged) = beside(&at);

    symlink(&victim, &staged)?;

    assert!(matches!(
      whole(&at, b"replacement"),
      Err(Unwritten::Making(..))
    ));
    assert!(matches!(
      whole_without_overwriting(&at, b"replacement"),
      Err(Unwritten::Making(..))
    ));

    let written = read_bytes(&at)?;
    let untouched = read_bytes(&victim)?;
    let about = metadata(&staged)?;

    assert_eq!(written, b"original");
    assert_eq!(untouched, b"untouched");
    assert_eq!(FileType::from_raw_mode(about.st_mode), FileType::Symlink);

    Ok(())
  }

  #[test]
  fn a_staging_file_owned_by_somebody_else_is_left_alone() -> Result<(), Box<dyn Error>> {
    let here = somewhere("staging-collision")?;
    let at = here.join("live");
    let Ok(staged) = beside(&at);

    write_and_sync(&staged, b"somebody else's bytes")?;

    assert!(matches!(
      whole(&at, b"replacement"),
      Err(Unwritten::Making(..))
    ));

    let untouched = read_bytes(&staged)?;
    let absent = read_bytes(&at).map_err(|fault| fault.kind());

    assert_eq!(untouched, b"somebody else's bytes");
    assert_eq!(absent, Err(std::io::ErrorKind::NotFound));

    Ok(())
  }

  #[test]
  fn a_relative_file_syncs_the_current_directory() -> Result<(), Unwritten> {
    sync_parent_directory(Path::new("relative-file"))
  }

  #[test]
  fn a_file_keeps_its_mode_when_it_is_written_over() -> Result<(), Box<dyn Error>> {
    let here = somewhere("mode")?;
    let at = here.join("thing");

    whole(&at, b"private")?;
    chmod(&at, Mode::from_raw_mode(0o600))?;
    whole(&at, b"still private")?;

    let about = metadata(&at)?;
    let mode = about.st_mode & 0o777;

    assert_eq!(
      mode, 0o600,
      "writing a file over itself changed who may read it"
    );
    Ok(())
  }

  #[test]
  fn a_file_can_be_published_with_the_callers_mode() -> Result<(), Box<dyn Error>> {
    use std::os::unix::fs::PermissionsExt;

    let here = somewhere("supplied-mode")?;
    let at = here.join("program");

    whole_with_permissions(&at, b"executable", std::fs::Permissions::from_mode(0o751))?;

    let about = metadata(&at)?;
    let contents = read_bytes(&at)?;

    assert_eq!(about.st_mode & 0o777, 0o751);
    assert_eq!(contents, b"executable");

    Ok(())
  }

  #[test]
  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "two scoped workers perform fifty writes each to regular files in a local fixture; joining them is required before their borrowed buffers are released, and completion of filesystem I/O is the kernel's responsibility"
    )
  )]
  fn two_writers_at_once_each_leave_a_whole_file() -> Result<(), Box<dyn Error>> {
    let here = somewhere("two")?;
    let at = here.join("thing");
    let long = vec![b'a'; 1_048_576];
    let short = vec![b'b'; 1_024];

    whole(&at, &short)?;

    std::thread::scope(|scope| {
      let one = scope.spawn(|| {
        for _ in 0..50 {
          whole(&at, &long)?;
        }

        Ok::<(), Unwritten>(())
      });
      let two = scope.spawn(|| {
        for _ in 0..50 {
          whole(&at, &short)?;
        }

        Ok::<(), Unwritten>(())
      });

      for _ in 0..100 {
        let held = read_bytes(&at)?;

        assert!(
          held == long || held == short,
          "a write came back torn, {} bytes",
          held.len()
        );
      }

      let first = one.join().map_err(|_panicked| io::Error::from(Errno::IO))?;
      let second = two.join().map_err(|_panicked| io::Error::from(Errno::IO))?;

      first?;
      second?;

      Ok(())
    })
  }

  #[test]
  fn nothing_there_and_will_not_be_read_are_two_different_answers() -> Result<(), Box<dyn Error>>
  {
    let here = somewhere("held")?;
    let Ok(never) = read(&here.join("never-written"));

    assert_eq!(never, Stored::Absent);

    let at = here.join("thing");

    whole(&at, b"said")?;

    let Ok(held) = read(&at);

    assert_eq!(held, Stored::Text("said".to_string()));

    let Ok(directory) = read(&here);
    let read_as = match directory {
      Stored::Failed(_) => "a fault",
      Stored::Text(_) => "text",
      Stored::Absent => "nothing",
    };

    assert_eq!(
      read_as, "a fault",
      "a directory read as something other than a fault"
    );

    Ok(())
  }

  #[test]
  fn folding_the_two_together_is_possible_and_has_to_be_said() {
    let Ok(said) = Stored::Text(String::from("x")).text();
    let Ok(nothing) = Stored::Absent.text();
    let Ok(unreadable) = Stored::Failed(String::from("boom")).text();

    assert_eq!(said, Some("x".to_string()));
    assert_eq!(nothing, None);
    assert_eq!(unreadable, None);
  }
}
