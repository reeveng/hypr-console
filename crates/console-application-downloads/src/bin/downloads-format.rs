//! Make everything in a folder the one format this device keeps.
//!
//!     downloads-format                     the music folder and the videos folder
//!     downloads-format /run/media/stick    whatever is in there
//!
//! Sound becomes opus and a film becomes mkv, which are what the fetcher
//! already writes, so a folder this has been over is a folder one program can
//! play the whole of. Independently: a film in the music folder is still made a
//! film, because what a file is is a question about the file rather than about
//! which folder someone left it in.
//!
//! What is replaced goes to the wastebasket. `gio trash` rather than unlinking,
//! the same as the Files panel deletes with, so an afternoon's conversion
//! someone regrets is an afternoon's walk back rather than a loss.
//!
//! One folder, not the tree under it. A folder is what someone is standing in
//! and what they asked about; a tree is a thing that runs for an hour over
//! places they were not thinking of.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

use console_core_arguments::{Operands, read};

use console_application_downloads::getting;
use console_application_downloads::same::{self, Wants};
use console_application_downloads::store::{self, Kind};
use console_core_never::Never;
use console_panel::running::{Notification, say};

const THERE: &str = "there";

const KIND: &str = "downloads-format";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Made {
  It,
  None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ran {
  Fine,
  Badly,
}

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: "downloads-format",
  about: "make everything in FOLDER the one format this device keeps; with no FOLDER, the music folder and the videos folder",
  flags: &[],
  operands: Operands::Optional("FOLDER"),
};

fn main() -> ExitCode {
  let native: Vec<OsString> = std::env::args_os().skip(1).collect();
  let folder = match native.as_slice() {
    [path] => {
      (path.to_str().is_none() && !path.as_encoded_bytes().starts_with(b"--")).then_some(path)
    }
    [end, path] => (end == "--" && path.to_str().is_none()).then_some(path),
    _ => None,
  };

  match folder {
    Some(path) => {
      let Ok(()) = format(Some(Path::new(path)));

      return ExitCode::SUCCESS;
    }
    None => {}
  }

  let words = match native
    .into_iter()
    .map(OsString::into_string)
    .collect::<Result<Vec<_>, _>>()
  {
    Ok(words) => words,
    Err(word) => {
      eprintln!("invalid downloads-format argument {word:?}");

      return ExitCode::from(2);
    }
  };

  let line = match read(&COMMAND, &words) {
    Ok(line) => line,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };
  let Ok(operands) = line.operands();
  let Ok(()) = format(operands.first().map(Path::new));

  ExitCode::SUCCESS
}

fn format(folder: Option<&Path>) -> Result<(), Never> {
  let where_: Vec<PathBuf> = match folder {
    Some(chosen) => vec![chosen.to_path_buf()],
    None => Kind::BOTH
      .iter()
      .map(|kind| {
        let Ok(into) = getting::into(*kind);

        into
      })
      .collect(),
  };
  let mut made: u32 = 0;
  let mut left: u32 = 0;

  for folder_2 in &where_ {
    let Ok(wanting) = wanting(folder_2);

    for path in wanting {
      let Ok(made_one) = made_one(&path);

      match made_one {
        Made::It => made = made.saturating_add(1),
        Made::None => left = left.saturating_add(1),
      }
    }
  }

  let Ok(()) = notify(Converted { made, left }, &where_);

  Ok(())
}

fn wanting(folder: &Path) -> Result<Vec<PathBuf>, Never> {
  let reading = match std::fs::read_dir(folder) {
    Ok(reading) => reading,
    Err(_unreadable) => return Ok(Vec::new()),
  };

  let mut found: Vec<PathBuf> = reading
    .flatten()
    .filter_map(|entry| {
      let path = entry.path();
      let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(fault) => {
          eprintln!("examining {}: {fault}", path.display());

          return None;
        }
      };
      let Ok(wanted) = same::wants_at(&path);

      (metadata.is_file() && !matches!(wanted, Wants::None | Wants::Leave)).then_some(path)
    })
    .collect();
  found.sort();
  Ok(found)
}

fn what(path: &Path) -> Result<Option<Kind>, Never> {
  let Ok(named) = same::wants_at(path);

  Ok(match named {
    Wants::Made(kind) => Some(kind),
    Wants::Ask => {
      let Ok(about) = same::about(path);
      let Ok(said) = run_output(&about);
      let Ok(inside) = same::inside(&said);

      Some(inside)
    }
    Wants::None | Wants::Leave => None,
  })
}

fn made_one(path: &Path) -> Result<Made, Never> {
  let Ok(kind) = what(path);
  let conversion = match kind {
    Some(kind_2) => kind_2,
    None => return Ok(Made::None),
  };
  let Ok(to) = same::beside(path, conversion);

  match std::fs::symlink_metadata(&to) {
    Ok(_existing) => return Ok(Made::None),
    Err(fault) => match (fault.kind() == std::io::ErrorKind::NotFound).then_some(()) {
      Some(()) => {}
      None => {
        eprintln!("checking {}: {fault}", to.display());

        return Ok(Made::None);
      }
    },
  }

  let parent = match path
    .parent()
    .filter(|parent| !parent.as_os_str().is_empty())
  {
    Some(parent) => parent,
    None => Path::new("."),
  };
  let work = match store::Scratch::beside(parent, "conversion") {
    Ok(work) => work,
    Err(fault) => {
      eprintln!(
        "preparing conversion scratch beside {}: {fault}",
        path.display()
      );

      return Ok(Made::None);
    }
  };
  let Ok(folder) = work.path();
  let part = match conversion {
    Kind::Film => folder.join("converted.mkv"),
    Kind::Sound => folder.join("converted.opus"),
    Kind::Book => return Ok(Made::None),
  };
  let files = Conversion {
    original: path,
    prepared: &part,
    destination: &to,
  };
  let Ok(converted) = convert(&files, conversion);

  match converted {
    Ran::Fine => put_in_place(&files),
    Ran::Badly => Ok(Made::None),
  }
}

struct Conversion<'a> {
  original: &'a Path,
  prepared: &'a Path,
  destination: &'a Path,
}

fn convert(files: &Conversion<'_>, kind: Kind) -> Result<Ran, Never> {
  match kind {
    Kind::Film => {
      let Ok(film) = same::film(files.original, files.prepared);

      ran(&film)
    }
    Kind::Sound => {
      let Ok(image) = cover(files);
      let Ok(sound) = same::sound(files.original, files.prepared, image.as_deref());

      ran(&sound)
    }
    Kind::Book => Ok(Ran::Badly),
  }
}

fn put_in_place(files: &Conversion<'_>) -> Result<Made, Never> {
  let to = files.destination;

  match console_core_atomic_writes::rename_without_overwriting(files.prepared, to) {
    Ok(()) => {}
    Err(fault) => {
      eprintln!("putting {} in place: {fault}", to.display());

      return Ok(Made::None);
    }
  }

  match console_gio::trash(files.original) {
    Err(fault) => {
      eprintln!("trashing {}: {fault}", files.original.display());

      match store::remove_if_present(to) {
        Ok(()) => {}
        Err(fault_2) => eprintln!(
          "removing the converted copy {}: {fault}",
          to.display(),
          fault = fault_2
        ),
      }

      Ok(Made::None)
    }
    Ok(()) => Ok(Made::It),
  }
}

fn cover(files: &Conversion<'_>) -> Result<Option<String>, Never> {
  let jpg = files.prepared.with_file_name("cover.jpg");
  let Ok(arguments) = same::cover(files.original, &jpg);
  let Ok(read) = ran(&arguments);

  match read {
    Ran::Badly => return Ok(None),
    Ran::Fine => {}
  }

  let held = match console_core_atomic_writes::read_bytes(&jpg) {
    Ok(held) => held,
    Err(fault) => {
      eprintln!("reading back the cover ffmpeg wrote: {fault}");

      return Ok(None);
    }
  };

  Ok(match held.is_empty().then_some(()) {
    Some(()) => None,
    None => {
      let Ok(block) = same::block("image/jpeg", &held);

      Some(block)
    }
  })
}

fn invoke(arguments: &[OsString]) -> std::io::Result<Output> {
  let (program, rest) = arguments
    .split_first()
    .ok_or_else(|| std::io::Error::other("missing program"))?;

  Command::new(program).args(rest).output()
}

fn ran(arguments: &[OsString]) -> Result<Ran, Never> {
  Ok(match invoke(arguments) {
    Ok(done) => match done.status.success().then_some(()) {
      Some(()) => Ran::Fine,
      None => Ran::Badly,
    },
    Err(fault) => {
      eprintln!("the conversion tool could not start: {fault}");

      Ran::Badly
    }
  })
}

fn run_output(arguments: &[OsString]) -> Result<String, Never> {
  Ok(match invoke(arguments) {
    Ok(done) => match String::from_utf8(done.stdout) {
      Ok(text) => text,
      Err(fault) => {
        eprintln!("the format probe returned output that is not UTF-8: {fault}");

        String::new()
      }
    },
    Err(fault) => {
      eprintln!("the format probe could not start: {fault}");

      String::new()
    }
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Converted {
  made: u32,
  left: u32,
}

fn notify(converted: Converted, where_: &[PathBuf]) -> Result<(), Never> {
  let Converted { made, left } = converted;
  let folders: Vec<String> = where_
    .iter()
    .map(|at| match at.file_name() {
      Some(name) => match name.to_str() {
        Some(text) => text.to_string(),
        None => format!("{name:?}"),
      },
      None => THERE.to_string(),
    })
    .collect();
  let said = match (made, left) {
    (0, 0) => "Already converted".to_string(),
    (0, left_2) => format!("Couldn't convert {left} files", left = left_2),
    (made_2, 0) => format!("Converted {made} files", made = made_2),
    (made_3, left_3) => format!(
      "Converted {made} files, skipped {left}",
      left = left_3,
      made = made_3
    ),
  };
  let options = console_operating_system_linux::notification::Options {
    app_name: Some("Console"),
    icon: Some("folder-music"),
    ..Default::default()
  };
  let Ok(mut notify) = options.command(&folders.join(" and "), Some(&said));
  let started = notify.status();

  match started {
    Ok(_) => {}
    Err(fault) => {
      eprintln!("telling someone the folder is one format: {fault}");
      println!("{} {said}", folders.join(" and "));
    }
  }

  match ::core::primitive::bool::then_some(left > 0 && made == 0, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(()) = say(
        KIND,
        Notification {
          summary: &format!("Couldn't convert {}", folders.join(" and ")),
          body: &said,
        },
      );
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}
