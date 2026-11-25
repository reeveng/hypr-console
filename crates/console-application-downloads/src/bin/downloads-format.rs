//! Make everything in a folder the one format this device keeps.
//!
//!     downloads-format                     the music folder and the videos folder
//!     downloads-format /run/media/stick    whatever is in there
//!     downloads-format --music-library     audio within the configured music folder
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
//! The folder command works on the selected directory. Music Library walks
//! album subfolders as well and converts regular audio files.

use std::ffi::{OsStr, OsString};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

use console_core_arguments::{Flag, Operands, Presence, Takes, read};
use console_core_directory_listing::Descend;
use console_core_internal_programs::InternalProgram;

use console_application_downloads::getting;
use console_application_downloads::same::{self, Wants};
use console_application_downloads::store::{self, Kind};
use console_core_never::Never;
use console_music_player::selection::Selection;
use console_notifications::saying::Content as Notification;

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

#[derive(Clone, Copy)]
enum Purpose {
  MusicLibrary,
  Folder,
}

const MUSIC_LIBRARY: Flag = Flag {
  spelling: "--music-library",
  takes: Takes::None,
  about: "convert audio in the configured Music Library and its album folders to Opus",
};

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: "downloads-format",
  about: "make everything in FOLDER the one format this device keeps; with no FOLDER, the music folder and the videos folder",
  flags: &[MUSIC_LIBRARY],
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
  let Ok(music) = line.presence(MUSIC_LIBRARY);

  match (music, operands.first()) {
    (Presence::Present, Some(_)) => {
      eprintln!("--music-library uses the configured library folder; omit FOLDER");

      return ExitCode::from(2);
    }
    (Presence::Present, None) => {
      let Ok(library) = getting::into(Kind::Sound);
      let Ok(()) = format_music(&library);
    }
    (Presence::Absent, chosen) => {
      let Ok(()) = format(chosen.map(Path::new));
    }
  }

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
  let mut converted = Converted::default();

  for folder_2 in &where_ {
    let Ok(wanting) = wanting(folder_2);

    for path in wanting {
      let Ok(wanted) = same::wants_at(&path);
      let Ok(result) = match what(&path, wanted) {
        Ok(Some(kind)) => made_one(&path, kind, Purpose::Folder),
        Ok(None) => Ok(Made::None),
        Err(fault) => {
          console_core_logging::warn!("probing {} for conversion: {fault}", path.display());

          Ok(Made::None)
        }
      };
      let Ok(()) = converted.record(result);
    }
  }

  notify(converted, &where_)
}

fn format_music(folder: &Path) -> Result<(), Never> {
  let Ok(listed) = console_core_directory_listing::recursive(folder, |path| {
    let Ok(descend) = visible_entry(path);

    descend
  });
  let mut converted = Converted::default();

  for entry in listed {
    let path = match entry {
      Ok(path) => path,
      Err(fault) => {
        console_core_logging::warn!("listing tracks for conversion: {fault}");
        let Ok(()) = converted.record(Made::None);

        continue;
      }
    };

    match music_candidate(&path) {
      Ok(Some(kind)) => {
        let Ok(result) = made_one(&path, kind, Purpose::MusicLibrary);
        let Ok(()) = converted.record(result);
      }
      Ok(None) => {}
      Err(fault) => {
        console_core_logging::warn!("checking {} for conversion: {fault}", path.display());
        let Ok(()) = converted.record(Made::None);
      }
    }
  }

  match converted.made {
    0 => {}
    _ => {
      let Ok(()) = refresh_music_catalog();
    }
  }

  notify(converted, &[folder.to_path_buf()])
}

fn refresh_music_catalog() -> Result<(), Never> {
  let Ok(mut index) = InternalProgram::MusicIndex.command();

  match index.status() {
    Ok(status) => match status.success().then_some(()) {
      Some(()) => {}
      None => console_core_logging::warn!("refreshing the converted Music Library: {status}"),
    },
    Err(fault) => console_core_logging::warn!("refreshing the converted Music Library: {fault}"),
  }

  Ok(())
}

fn visible_entry(path: &Path) -> Result<Descend, Never> {
  Ok(match path.file_name().map(OsStr::as_encoded_bytes) {
    Some(name) => match name.starts_with(b".").then_some(()) {
      Some(()) => Descend::Past,
      None => Descend::Into,
    },
    None => Descend::Into,
  })
}

fn music_candidate(path: &Path) -> std::io::Result<Option<Kind>> {
  let Ok(visibility) = visible_entry(path);
  let Ok(wanted) = same::wants_at(path);

  match (visibility, wanted) {
    (Descend::Into, Wants::Made(Kind::Sound) | Wants::Ask) => {}
    (Descend::Past, _) | (Descend::Into, _) => return Ok(None),
  }

  let metadata = std::fs::symlink_metadata(path)?;

  match metadata.is_file().then_some(()) {
    Some(()) => {}
    None => return Ok(None),
  }

  let kind = what(path, wanted)?;

  Ok(kind.filter(|media| *media == Kind::Sound))
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

fn what(path: &Path, wanted: Wants) -> std::io::Result<Option<Kind>> {
  Ok(match wanted {
    Wants::Made(kind) => Some(kind),
    Wants::Ask => {
      let Ok(about) = same::about(path);
      let said = run_output(&about)?;
      let Ok(inside) = same::inside(&said);

      Some(inside)
    }
    Wants::None | Wants::Leave => None,
  })
}

fn made_one(path: &Path, conversion: Kind, purpose: Purpose) -> Result<Made, Never> {
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
  let original = match std::fs::symlink_metadata(path) {
    Ok(original) => match (purpose, original.is_file().then_some(())) {
      (Purpose::MusicLibrary, None) => return Ok(Made::None),
      (Purpose::Folder, _) | (Purpose::MusicLibrary, Some(())) => {
        let Ok(revision) = FileRevision::of(&original);

        revision
      }
    },
    Err(fault) => {
      console_core_logging::warn!("reading {} before conversion: {fault}", path.display());

      return Ok(Made::None);
    }
  };
  let Ok(converted) = convert(&files, conversion);

  match converted {
    Ran::Fine => put_in_place(&files, original, purpose),
    Ran::Badly => Ok(Made::None),
  }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct FileRevision {
  device: u64,
  inode: u64,
  changed: i64,
  nanos: i64,
}

impl FileRevision {
  fn of(info: &std::fs::Metadata) -> Result<Self, Never> {
    Ok(Self {
      device: info.dev(),
      inode: info.ino(),
      changed: info.ctime(),
      nanos: info.ctime_nsec(),
    })
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

fn put_in_place(
  files: &Conversion<'_>,
  revision: FileRevision,
  purpose: Purpose,
) -> Result<Made, Never> {
  let editable = match std::fs::symlink_metadata(files.original) {
    Ok(file) => {
      let Ok(current) = FileRevision::of(&file);

      (current == revision).then_some(())
    }
    Err(fault) => {
      console_core_logging::warn!("checking {} after conversion: {fault}", files.original.display());

      return Ok(Made::None);
    }
  };

  match editable {
    Some(()) => {}
    None => {
      console_core_logging::warn!("{} changed during conversion", files.original.display());

      return Ok(Made::None);
    }
  }

  let to = files.destination;

  match console_core_atomic_writes::rename_without_overwriting(files.prepared, to) {
    Ok(()) => {}
    Err(fault) => {
      eprintln!("putting {} in place: {fault}", to.display());

      return Ok(Made::None);
    }
  }

  match replace_original(files, purpose) {
    Err(fault) => {
      console_core_logging::warn!("replacing {}: {fault}", files.original.display());

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

fn replace_original(files: &Conversion<'_>, purpose: Purpose) -> std::io::Result<()> {
  match purpose {
    Purpose::MusicLibrary => {
      Selection::copy_decision(files.original, files.destination)?;
    }
    Purpose::Folder => {}
  }

  console_gio::trash(files.original)
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

fn run_output(arguments: &[OsString]) -> std::io::Result<String> {
  let done = invoke(arguments)?;

  match done.status.success().then_some(()) {
    Some(()) => String::from_utf8(done.stdout).map_err(std::io::Error::other),
    None => Err(std::io::Error::other(format!("format probe exited with {}", done.status))),
  }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Converted {
  made: u32,
  left: u32,
}

impl Converted {
  fn record(&mut self, result: Made) -> Result<(), Never> {
    match result {
      Made::It => self.made = self.made.saturating_add(1),
      Made::None => self.left = self.left.saturating_add(1),
    }

    Ok(())
  }
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
  let options = console_notifications::delivery::Options {
    app_name: Some("Downloads"),
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
      let Ok(()) = console_notifications::saying::report(
        console_notifications::Application("Downloads"),
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
