//! Fetch one thing, and say so when it has arrived.
//!
//!     downloads-get --audio https://youtu.be/FTQbiNvZqaY
//!     downloads-get --video https://youtu.be/FTQbiNvZqaY
//!     downloads-get --book https://www.gutenberg.org/ebooks/84 Frankenstein
//!
//! A film is minutes of fetching, and the panel that started this is a card
//! someone has probably closed by the time it lands. So the arrival is said on
//! the screen rather than drawn: the panel's own word in the corner says it was
//! set going, and this says it is done.
//!
//! What is already in the folder is not fetched again, a book excepted. yt-dlp's answer to being
//! asked twice is to download the whole thing, hand it to a converter that will
//! not write over what is there, and fail at the last step with "Conversion
//! failed!" -- having left the metadata and a half-made `.temp.opus` in the
//! folder, which ends in an extension the music panel lists. So it is asked
//! here first, where the answer is one look at a folder.
//!
//! A book is pressed again to have it again, and there is only ever one of it
//! in Books: it arrives under a `.part` name, and only once it has arrived
//! whole is it moved into the library and the other copies taken away -- the
//! same id, or the same title from the other library. A
//! fetch that fails leaves the old copy standing.
//!
//! Nothing here tells an open library what arrived. The folder it was written
//! into is watched by the pool, so the books, the music and the films hear it
//! from the kernel, the same way they hear a file anybody else put there.

use std::ffi::OsStr;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use console_application_downloads::getting;
use console_application_downloads::getting::Fetch;
use console_application_downloads::looking;
use console_application_downloads::store::{self, Kind};
use console_core_arguments::{Operands, Reason, ValidationError, read};
use console_core_never::Never;
use console_notifications::saying::Content;
use console_repository::RelativePath;

const KIND: &str = "download";

const NOTHING: &str = "couldn't be downloaded";

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: "downloads-get",
  about: "fetch one thing of one kind from URL, under TITLE if there is one, and say so when it arrives",
  flags: &Kind::FLAGS,
  operands: Operands::Verbatim("URL TITLE"),
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Request {
  kind: Kind,
  url: String,
  title: Option<String>,
}

fn request(words: &[String]) -> Result<Request, ValidationError> {
  let read = read(&COMMAND, words);
  let line = read?;
  let choice = line.one_of(&Kind::FLAGS);
  let flag = choice?;
  let Ok(operands) = line.operands();

  let (url, title) = match operands {
    [url] => (url, None),
    [url, title] => (url, Some(title.clone())),
    [] => {
      let Ok(refusal) = line.refusal(Reason::MissingOperands(vec!["URL"]));

      return Err(refusal);
    }
    [_url, _title, extra, ..] => {
      let Ok(refusal) = line.refusal(Reason::ExtraArgument(extra.clone()));

      return Err(refusal);
    }
  };

  match Kind::from_flag(flag.spelling) {
    Ok(Some(kind)) => Ok(Request {
      kind,
      url: url.clone(),
      title,
    }),
    Ok(None) => {
      let Ok(refusal) = line.refusal(Reason::MissingFlag(
        Kind::FLAGS.iter().map(|flag_2| flag_2.spelling).collect(),
      ));

      Err(refusal)
    }
  }
}

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();
  let Ok(code) = console_core_arguments::run_main(&COMMAND, &words, request, fetch);

  code
}

fn fetch(request: Request) -> Result<(), Never> {
  let Request {
    kind,
    url,
    title: called,
  } = request;
  let Ok(folder) = getting::into(kind);
  let into = match folder.as_os_str().is_empty().then_some(()) {
    Some(()) => Path::new("."),
    None => folder.as_path(),
  };
  let Ok(nothing) = sentence(&called, NOTHING);

  match std::fs::create_dir_all(into) {
    Ok(()) => {}
    Err(fault) => {
      return failed(Content {
        summary: &nothing,
        body: &format!("{} cannot be written to: {fault}", into.display()),
      });
    }
  }

  match kind {
    Kind::Sound | Kind::Film => {
      let Ok(existing) = already(into, &url);

      match existing {
        Some(have) => {
          let named = match called.as_deref() {
            Some(requested) => requested,
            None => &have,
          };
          return notify_arrived(named, into, ALREADY);
        }
        None => {}
      }
    }
    Kind::Book => {}
  }

  let download = Fetch {
    kind,
    url: &url,
    into,
    title: match called.as_deref() {
      Some(title) => title,
      None => "",
    },
  };

  match staged(download) {
    Ok(path) => {
      let Ok(arrived) = arrived(path.file_name());

      notify_arrived(&arrived, into, IS_IN)
    }
    Err(fault) => failed(Content {
      summary: &nothing,
      body: &fault.to_string(),
    }),
  }
}

fn failed(notice: Content<'_>) -> Result<(), Never> {
  console_notifications::saying::report(
    console_notifications::Application("Downloads"),
    KIND,
    notice,
  )
}

fn staged(destination: Fetch<'_>) -> io::Result<PathBuf> {
  let scratch = store::Scratch::beside(destination.into, "download")?;
  let Ok(folder) = scratch.path();
  let absolute = folder.canonicalize()?;
  let arguments = getting::arguments(Fetch {
    into: &absolute,
    ..destination
  })?;
  let Ok(missing) = looking::missing_tool_message(destination.kind);
  let (program, rest) = arguments
    .split_first()
    .ok_or_else(|| io::Error::other(missing))?;
  let done = Command::new(program)
    .args(rest)
    .output()
    .map_err(|fault| io::Error::other(format!("{missing} {fault}")))?;

  match done.status.success().then_some(()) {
    Some(()) => {}
    None => {
      let why = match std::str::from_utf8(&done.stderr) {
        Ok(error) => {
          let Ok(why) = looking::complaint(error);

          why
        }
        Err(fault) => format!("{program:?} returned unreadable diagnostics ({fault})"),
      };

      return Err(io::Error::other(why));
    }
  }

  let part = reported_file(&absolute, &done.stdout)?;
  let entry = std::fs::symlink_metadata(&part)?;

  match entry.is_file().then_some(()) {
    Some(()) => publish(destination, &part),
    None => Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "the download tool returned a non-regular file",
    )),
  }
}

fn reported_file(folder: &Path, reported: &[u8]) -> io::Result<PathBuf> {
  let bytes = match reported.strip_suffix(b"\n") {
    Some(line) => line,
    None => reported,
  };
  let entries = std::fs::read_dir(folder)?;

  for entry in entries {
    let entry_2 = entry?;
    let path = entry_2.path();
    let matching = match bytes.strip_suffix(path.as_os_str().as_bytes()) {
      Some([]) => true,
      Some(prefix) => prefix.ends_with(b"\n"),
      None => false,
    };

    match matching.then_some(()) {
      Some(()) => return Ok(path),
      None => {}
    }
  }

  Err(io::Error::new(
    io::ErrorKind::InvalidData,
    "the download tool did not report a file in its directory",
  ))
}

fn publish(destination: Fetch<'_>, part: &Path) -> io::Result<PathBuf> {
  let from = part
    .file_name()
    .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;

  match destination.kind {
    Kind::Sound | Kind::Film => {
      let name = RelativePath::new(Path::new(from)).map_err(io::Error::other)?;
      let to = destination.into.join(name);
      console_core_atomic_writes::rename_without_overwriting(part, &to)?;

      Ok(to)
    }
    Kind::Book => {
      let encoded = from.as_bytes();
      let name = encoded
        .strip_suffix(getting::UNFINISHED.as_bytes())
        .filter(|unwrapped| unwrapped.ends_with(b".epub"))
        .ok_or_else(|| {
          io::Error::new(
            io::ErrorKind::InvalidData,
            "the book tool did not return an EPUB",
          )
        })?;
      let child =
        RelativePath::new(Path::new(OsStr::from_bytes(name))).map_err(io::Error::other)?;
      let to = destination.into.join(child);
      let Ok(id) = id_of(destination.url);
      let Ok(copies) = book_copies(
        destination.into,
        getting::Book {
          id: &id,
          title: destination.title,
        },
      );

      std::fs::rename(part, &to)?;
      let Ok(()) = remove_copies(copies, &to);

      Ok(to)
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Landed<'a>(&'a str);

const IS_IN: Landed<'static> = Landed("is in");
const ALREADY: Landed<'static> = Landed("was already in");

fn sentence(called: &Option<String>, said: &str) -> Result<String, Never> {
  Ok(match called {
    Some(called_2) => format!("{called} {said}", called = called_2),
    None => format!("It {said}"),
  })
}

fn already(into: &Path, url: &str) -> Result<Option<String>, Never> {
  let id = match getting::id_in(url) {
    Ok(Some(id)) => id,
    Ok(None) | Err(_) => return Ok(None),
  };

  let reading = match std::fs::read_dir(into) {
    Ok(reading) => reading,
    Err(fault) => {
      eprintln!(
        "looking in {} for what is already there: {fault}",
        into.display()
      );
      return Ok(None);
    }
  };
  let holding = reading.flatten().find_map(|entry| {
    let name = entry.file_name();
    let Ok(found) = getting::matches_id(&name, &id);

    (found == getting::Have::It).then_some(name)
  });
  let Ok(named) = arrived(holding.as_deref());

  Ok(holding.map(|_| named))
}

fn id_of(url: &str) -> Result<String, Never> {
  let Ok(id) = getting::id_in(url);

  Ok(match id {
    Some(number) => number,
    None => String::new(),
  })
}

fn book_copies(into: &Path, book: getting::Book<'_>) -> Result<Vec<PathBuf>, Never> {
  let reading = match std::fs::read_dir(into) {
    Ok(reading) => reading,
    Err(fault) => {
      eprintln!("reading existing books in {}: {fault}", into.display());

      return Ok(Vec::new());
    }
  };
  let names = reading
    .flatten()
    .filter_map(|entry| match entry.file_name().into_string() {
      Ok(name) => Some(name),
      Err(native) => {
        eprintln!(
          "skipping a book name with non-UTF-8 bytes during title comparison: {native:?}"
        );

        None
      }
    });
  let Ok(copies) = getting::copies(names, book);
  let mut paths = Vec::new();

  for copy in &copies {
    match RelativePath::new(Path::new(copy)) {
      Ok(relative) => paths.push(into.join(relative)),
      Err(fault) => eprintln!("checking an existing book name: {fault}"),
    }
  }

  Ok(paths)
}

fn remove_copies(copies: Vec<PathBuf>, keeping: &Path) -> Result<(), Never> {
  for copy in copies.into_iter().filter(|copy| copy != keeping) {
    match store::remove_if_present(&copy) {
      Ok(()) => {}
      Err(fault) => eprintln!("removing old copy {}: {fault}", copy.display()),
    }
  }

  Ok(())
}

fn arrived(name: Option<&OsStr>) -> Result<String, Never> {
  match name {
    Some(file) => match file.to_str() {
      Some(text) => console_core_file_names::title(text),
      None => Ok(format!("{file:?}")),
    },
    None => Ok("It".to_string()),
  }
}

fn notify_arrived(name: &str, into: &Path, said: Landed<'_>) -> Result<(), Never> {
  let said_2 = said.0;
  let where_ = into.file_name().map(|name_2| match name_2.to_str() {
    Some(text) => text.to_string(),
    None => format!("{name_2:?}"),
  });
  let folder = match where_ {
    Some(folder) => folder,
    None => into.display().to_string(),
  };
  let body = format!("{said_2} {folder}");
  let options = console_notifications::delivery::Options {
    app_name: Some("Downloads"),
    icon: Some("folder-download"),
    ..Default::default()
  };
  let Ok(mut notify) = options.command(name, Some(&body));
  let started = notify.status();

  match started {
    Ok(_) => {}
    Err(fault) => {
      eprintln!("telling someone it landed: {fault}");
      println!("{name} {said} {}", into.display(), said = said_2);
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_arrived_file_is_said_as_a_title() {
    let file = OsStr::new("Africa [FTQbiNvZqaY].opus");

    assert_eq!(arrived(Some(file)), Ok("Africa".to_string()));
    assert_eq!(arrived(None), Ok("It".to_string()));
  }

  #[test]
  fn the_tool_can_report_native_names_only_from_its_own_directory() -> io::Result<()> {
    let work = store::Scratch::make("download-native-test")?;
    let Ok(folder) = work.path();
    let child = RelativePath::new(Path::new(OsStr::from_bytes(b"live\nset \xff [x].opus")))
      .map_err(io::Error::other)?;
    let at = folder.join(child);
    console_core_atomic_writes::whole(&at, b"the downloaded contents")
      .map_err(io::Error::other)?;
    let mut reported = b"a status line\n".to_vec();
    reported.extend_from_slice(at.as_os_str().as_bytes());
    reported.push(b'\n');

    let found = reported_file(folder, &reported)?;
    assert_eq!(found, at);
    let refused = reported_file(folder, b"/somewhere-else/live.opus");
    assert!(matches!(refused, Err(fault) if fault.kind() == io::ErrorKind::InvalidData));

    Ok(())
  }

  #[test]
  fn what_is_said_about_a_fault_names_the_thing_it_is_about() {
    assert_eq!(
      sentence(&Some("Africa".to_string()), NOTHING),
      Ok("Africa couldn't be downloaded".to_string())
    );
    assert_eq!(
      sentence(&None, NOTHING),
      Ok("It couldn't be downloaded".to_string())
    );
  }

  fn words(said: &[&str]) -> Result<Vec<String>, Never> {
    Ok(said.iter().map(|word| (*word).to_string()).collect())
  }

  #[test]
  fn a_title_goes_on_as_it_is_and_the_kind_is_one_flag() {
    let Ok(book) = words(&[
      "--book",
      "https://www.gutenberg.org/ebooks/84",
      "--Frankenstein--",
    ]);
    let Ok(two_kinds) = words(&["--book", "--audio", "https://youtu.be/FTQbiNvZqaY"]);

    assert_eq!(
      request(&book),
      Ok(Request {
        kind: Kind::Book,
        url: "https://www.gutenberg.org/ebooks/84".to_string(),
        title: Some("--Frankenstein--".to_string())
      })
    );
    assert_eq!(
      request(&two_kinds).map_err(|refusal| refusal.reason),
      Err(Reason::ExtraArgument("--book".to_string()))
    );
  }
}
