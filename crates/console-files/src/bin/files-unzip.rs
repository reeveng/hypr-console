//! Unpack one archive into a folder of its own, and stop.
//!
//!     files-unzip ~/Downloads/WickedWhims.zip
//!
//! Off the panel and not in it, for the reason the pictures are: a mod is
//! tens of megabytes of compressed package files and a panel that unpacked
//! them where it draws would answer nothing until it was done.
//! The card runs this as a task, says on the archive's row that it is
//! unzipping while it does, and draws the folder again when it ends, so what
//! arrived is on the screen without anyone asking for it.
//!
//! The unpacking happens into a hidden folder beside the archive and the
//! result is renamed into place, which is one rename rather than a folder that
//! fills up while someone watches it. A dot in front is all it takes: the
//! listing does not show what starts with one, so an unzip that dies halfway
//! leaves nothing on the screen to explain.
//!
//! `console_files::unzipping` is what decides the names and whether the folder
//! inside the archive is a wrapper to lift away. This is the part that touches
//! the machine.

use std::ffi::OsString;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_core_arguments::{Command, Operands, ValidationError, read};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_files::paths::{ChildName, InvalidName};
use console_files::places::Is;
use console_files::unzipping::{self, Lift};
use console_panel::running::{Notification, say};

const KIND: &str = "files-unzip";

const FULL: &str = "No free folder name to unzip into.";

const ARCHIVE: [&str; 1] = ["ARCHIVE"];

const COMMAND: Command = Command {
  name: "files-unzip",
  about: "unpack one archive into a folder of its own, and stop",
  flags: &[],
  operands: Operands::Named(&ARCHIVE),
};

fn archive(words: &[String]) -> Result<PathBuf, ValidationError> {
  let read = read(&COMMAND, words);
  let line = read?;
  let operands = line.exactly(ARCHIVE);
  let [archive] = operands?;

  Ok(PathBuf::from(archive))
}

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();

  let archive = match archive(&words) {
    Ok(archive) => archive,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };

  let holding = match archive.parent().map(Path::to_path_buf) {
    Some(holding) => holding,
    None => {
      eprintln!("files-unzip: {}: nothing holds this", archive.display());
      return ExitCode::FAILURE;
    }
  };

  let named = match archive.file_name().and_then(std::ffi::OsStr::to_str) {
    Some(named) => named,
    None => {
      eprintln!("files-unzip: {}: this names no archive", archive.display());
      return ExitCode::FAILURE;
    }
  };
  let Ok(unpacked) = unpacked(&archive, named, &holding);

  unpacked
}

fn unpacked(archive: &Path, named: &str, holding: &Path) -> Result<ExitCode, Never> {
  let staging = match staging_in(holding) {
    Ok(staging) => staging,
    Err(fault) => {
      let Ok(()) = notify_failure(named, Why(&format!("{}: {fault}", holding.display())));
      return Ok(ExitCode::FAILURE);
    }
  };
  let Ok(ran) = ran(archive, &staging);
  let Ok(code) = match ran {
    Ran::Fine => completed(
      Extraction {
        staging: &staging,
        holding,
      },
      named,
    ),
    Ran::Badly(why) => {
      let Ok(()) = notify_failure(named, Why(&why));
      Ok(ExitCode::FAILURE)
    }
  };

  match std::fs::remove_dir_all(&staging) {
    Ok(()) => {}
    Err(fault) => {
      match ::core::primitive::bool::then_some(fault.kind() == ErrorKind::NotFound, ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => {
          eprintln!("files-unzip: cleaning up {}: {fault}", staging.display())
        }
      }
    }
  }

  Ok(code)
}

fn staging_in(holding: &Path) -> io::Result<PathBuf> {
  let process = std::process::id();

  for attempt in 1..=unzipping::TRIES {
    let label = format!("{process}-{attempt}");
    let Ok(named) = unzipping::while_unpacking(&label);
    let child = ChildName::new(&named).map_err(io::Error::other)?;
    let candidate = holding.join(child);

    match std::fs::create_dir(&candidate) {
      Ok(()) => return Ok(candidate),
      Err(fault) => {
        match ::core::primitive::bool::then_some(fault.kind() == ErrorKind::AlreadyExists, ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => return Err(fault),
        }
      }
    }
  }

  Err(io::Error::from(ErrorKind::AlreadyExists))
}

struct Extraction<'a> {
  staging: &'a Path,
  holding: &'a Path,
}

fn completed(extraction: Extraction<'_>, named: &str) -> Result<ExitCode, Never> {
  let Ok(entries) = inside(extraction.staging);
  let Ok(lift) = unzipping::lifting(&entries);
  let out_of = match lift {
    Lift::TheFolderInside(name) => {
      let child = match ChildName::new(&name) {
        Ok(child) => child,
        Err(fault) => {
          let Ok(()) = notify_failure(named, Why(&fault.to_string()));
          return Ok(ExitCode::FAILURE);
        }
      };
      extraction.staging.join(child)
    }
    Lift::None => extraction.staging.to_path_buf(),
  };
  let Ok(stem) = unzipping::named_for(named);
  let destination = Extraction {
    staging: &out_of,
    holding: extraction.holding,
  };

  Ok(match publish(destination, &stem) {
    Ok(()) => ExitCode::SUCCESS,
    Err(Publication::Failed(at, fault)) => {
      let Ok(()) = notify_failure(named, Why(&format!("{}: {fault}", at.display())));
      ExitCode::FAILURE
    }
    Err(Publication::Full) => {
      let Ok(()) = say(
        KIND,
        Notification {
          summary: named,
          body: FULL,
        },
      );
      ExitCode::FAILURE
    }
  })
}

#[derive(Debug)]
enum Publication {
  Full,
  Failed(PathBuf, io::Error),
}

fn publish(extraction: Extraction<'_>, stem: &str) -> Result<(), Publication> {
  let Ok(candidates) = unzipping::candidates(stem);

  for named in candidates {
    let child = match ChildName::new(&named) {
      Ok(child) => child,
      Err(InvalidName) => continue,
    };
    let target = extraction.holding.join(child);

    match console_core_atomic_writes::rename_without_overwriting(extraction.staging, &target) {
      Ok(()) => return Ok(()),
      Err(fault) => {
        match ::core::primitive::bool::then_some(fault.kind() == ErrorKind::AlreadyExists, ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => return Err(Publication::Failed(target, fault)),
        }
      }
    }
  }

  Err(Publication::Full)
}

enum Ran {
  Fine,
  Badly(String),
}

fn ran(archive: &Path, unpacking: &Path) -> Result<Ran, Never> {
  let Ok(mut seven) = Program::SevenZip.command();

  let mut out = OsString::from("-o");
  out.push(unpacking.as_os_str());

  let started = seven
    .args(["x", "-y", "-bso0", "-bsp0", "-p"])
    .arg(out)
    .arg("--")
    .arg(archive)
    .output();

  let done = match started {
    Ok(done) => done,
    Err(fault) => return Ok(Ran::Badly(format!("{fault}"))),
  };

  match ::core::primitive::bool::then_some(done.status.success(), ()) {
    ::core::option::Option::Some(()) => Ok(Ran::Fine),
    ::core::option::Option::None => {
      #[cfg_attr(
        dylint_lib = "explicit089_no_lossy_text",
        expect(
          explicit089_no_lossy_text,
          reason = "7z stderr is shown as a diagnostic and does not name an action or path"
        )
      )]
      let said = String::from_utf8_lossy(&done.stderr);
      let last = said.lines().rev().find(|line| !line.trim().is_empty());

      Ok(Ran::Badly(match last {
        Some(said_2) => said_2.trim().to_string(),
        None => "7z could not unpack this".to_string(),
      }))
    }
  }
}

fn inside(unpacking: &Path) -> Result<Vec<(OsString, Is)>, Never> {
  let reading = match std::fs::read_dir(unpacking) {
    Ok(reading) => reading,
    Err(_unreadable) => return Ok(Vec::new()),
  };

  Ok(
    reading
      .flatten()
      .map(|entry| {
        let name = entry.file_name();
        let is = match entry.path().metadata() {
          Ok(metadata) => match metadata.is_dir().then_some(()) {
            Some(()) => Is::AFolder,
            None => Is::AFile,
          },
          Err(_unavailable) => Is::AFile,
        };

        (name, is)
      })
      .collect(),
  )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Why<'a>(&'a str);

fn notify_failure(named: &str, why: Why<'_>) -> Result<(), Never> {
  say(
    KIND,
    Notification {
      summary: &format!("Couldn't unzip {named}"),
      body: why.0,
    },
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::os::unix::ffi::OsStringExt;
  use std::os::unix::fs::symlink;

  #[test]
  fn extraction_keeps_existing_names_and_publishes_native_children() -> io::Result<()> {
    let scratch =
      console_core_temporary_directories::private("files-unzip").map_err(io::Error::other)?;
    let staging = staging_in(&scratch)?;
    let another = staging_in(&scratch)?;
    assert_ne!(staging, another);

    let existing = scratch.join("Archive");
    symlink("does not exist", &existing)?;
    let name = OsString::from_vec(b"wrapper-\xff".to_vec());
    let child = ChildName::new(&name).map_err(io::Error::other)?;
    let wrapper = staging.join(child);
    std::fs::create_dir(&wrapper)?;
    let contents = wrapper.join("contents");
    console_core_atomic_writes::whole(&contents, b"unpacked").map_err(io::Error::other)?;
    let Ok(inside) = inside(&staging);
    let Ok(lift) = unzipping::lifting(&inside);
    assert_eq!(lift, Lift::TheFolderInside(name));

    let extraction = Extraction {
      staging: &wrapper,
      holding: &scratch,
    };
    publish(extraction, "Archive").map_err(|fault| io::Error::other(format!("{fault:?}")))?;
    let existing_link = std::fs::read_link(existing)?;
    let bytes = std::fs::read(scratch.join("Archive 2/contents"))?;
    assert_eq!(existing_link, Path::new("does not exist"));
    assert_eq!(bytes, b"unpacked");
    std::fs::remove_dir_all(scratch)?;

    Ok(())
  }
}
