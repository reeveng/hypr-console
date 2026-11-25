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
//! `console_application_files::unzipping` is what decides the names and whether the folder
//! inside the archive is a wrapper to lift away. This is the part that touches
//! the machine.

use std::ffi::OsString;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_application_files::paths::ChildName;
use console_application_files::places::Is;
use console_application_files::unzipping;
use console_core_arguments::{Command, Operands, ValidationError, read};
use console_core_external_programs::Program;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_state_machine::{Machine, Transition};
use console_notifications::saying::Content as Notification;

#[path = "files-unzip/extraction.rs"]
mod extraction;

use extraction::{Destination, Effect, Extraction, Failure, Renamed, Report, State};

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
  let Ok(unpacked) = unpacked(
    Source {
      archive: &archive,
      name: named,
      folder: &holding,
    },
    ran,
  );

  unpacked
}

struct Source<'a> {
  archive: &'a Path,
  name: &'a str,
  folder: &'a Path,
}

fn unpacked<E: std::fmt::Display>(
  source: Source<'_>,
  extract: impl Fn(&Path, &Path) -> Result<(), E>,
) -> Result<ExitCode, Never> {
  let Ok(stem) = unzipping::named_for(source.name);
  let Ok(first) = Extraction::initial_transition(
    &Destination {
      folder: source.folder,
      stem: &stem,
    },
    None,
  );
  let ended = console_core_iteration::iterate(first, |turn| {
    let Transition { state, effects } = turn;
    let mut received = None;

    for effect in effects {
      let Ok(report) = perform(&source, effect, &extract);

      match report {
        Some(outcome) => received = Some(outcome),
        None => {}
      }
    }

    Ok(match (state, received) {
      (State::Finished(code), _) => Step::Halt(code),
      (waiting, Some(report)) => {
        let Ok(next) = Extraction::transition(waiting, report);

        Step::Again(next)
      }
      (_, None) => {
        eprintln!("files-unzip: extraction did not finish");

        Step::Halt(ExitCode::FAILURE)
      }
    })
  });

  Ok(match ended {
    Ok(code) => code,
    Err(fault) => {
      eprintln!("files-unzip: {fault}");
      ExitCode::FAILURE
    }
  })
}

fn perform<E: std::fmt::Display>(
  archive: &Source<'_>,
  effect: Effect,
  extract: impl Fn(&Path, &Path) -> Result<(), E>,
) -> Result<Option<Report>, Never> {
  Ok(match effect {
    Effect::Reserve => Some(Report::Reserved(staging_in(archive.folder))),
    Effect::Extract(staging) => Some(Report::Extracted(
      extract(archive.archive, &staging).map_err(|fault| fault.to_string()),
    )),
    Effect::Inspect(staging) => {
      let Ok(entries) = inside(&staging);

      Some(Report::Inspected(entries))
    }
    Effect::Publish {
      source,
      destination,
    } => {
      let outcome =
        match console_core_atomic_writes::rename_without_overwriting(&source, &destination) {
          Ok(()) => Renamed::Published,
          Err(fault) => match (fault.kind() == ErrorKind::AlreadyExists).then_some(()) {
            Some(()) => Renamed::Occupied,
            None => Renamed::Failed(format!("{}: {fault}", destination.display())),
          },
        };

      Some(Report::Renamed(outcome))
    }
    Effect::Notify(failure) => {
      let Ok(()) = match failure {
        Failure::Detail(why) => notify_failure(archive.name, Why(&why)),
        Failure::Full => console_notifications::saying::report(
          console_notifications::Application("Files"),
          KIND,
          Notification {
            summary: archive.name,
            body: FULL,
          },
        ),
      };

      None
    }
    Effect::Clean(staging) => {
      let Ok(()) = clean(&staging);

      Some(Report::Cleaned)
    }
  })
}

fn clean(staging: &Path) -> Result<(), Never> {
  match std::fs::remove_dir_all(staging) {
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

  Ok(())
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

enum FailedExtraction {
  Launch(io::Error),
  Exit { diagnostic: Option<String> },
}

impl std::fmt::Display for FailedExtraction {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Launch(fault) => write!(to, "{fault}"),
      Self::Exit {
        diagnostic: Some(text),
      } => to.write_str(text),
      Self::Exit { diagnostic: None } => to.write_str("7z could not unpack this"),
    }
  }
}

fn ran(archive: &Path, unpacking: &Path) -> Result<(), FailedExtraction> {
  let Ok(mut seven) = Program::SevenZip.command();

  let mut out = OsString::from("-o");
  out.push(unpacking.as_os_str());

  let done = seven
    .args(["x", "-y", "-bso0", "-bsp0", "-p"])
    .arg(out)
    .arg("--")
    .arg(archive)
    .output()
    .map_err(FailedExtraction::Launch)?;

  match ::core::primitive::bool::then_some(done.status.success(), ()) {
    ::core::option::Option::Some(()) => Ok(()),
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

      Err(FailedExtraction::Exit {
        diagnostic: last.map(|line| line.trim().to_owned()),
      })
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
  console_notifications::saying::report(
    console_notifications::Application("Files"),
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
    let first = staging_in(&scratch)?;
    let second = staging_in(&scratch)?;
    assert_ne!(first, second);
    std::fs::remove_dir_all(first)?;
    std::fs::remove_dir_all(second)?;
    let existing = scratch.join("Archive");
    symlink("does not exist", &existing)?;
    let name = OsString::from_vec(b"wrapper-\xff".to_vec());
    let Ok(code) = unpacked(
      Source {
        archive: Path::new("Archive.zip"),
        name: "Archive.zip",
        folder: &scratch,
      },
      |_, staging| {
        let child = ChildName::new(&name).map_err(|fault| fault.to_string())?;
        let wrapper = staging.join(child);
        std::fs::create_dir(&wrapper).map_err(|fault| fault.to_string())?;
        console_core_atomic_writes::whole(&wrapper.join("contents"), b"unpacked")
          .map_err(|fault| fault.to_string())
      },
    );
    assert_eq!(code, ExitCode::SUCCESS);
    let dir = std::fs::read_dir(&scratch)?;
    assert_eq!(dir.count(), 2);
    let existing_link = std::fs::read_link(existing)?;
    let bytes = std::fs::read(scratch.join("Archive 2/contents"))?;
    assert_eq!(existing_link, Path::new("does not exist"));
    assert_eq!(bytes, b"unpacked");
    std::fs::remove_dir_all(scratch)?;

    Ok(())
  }
}
