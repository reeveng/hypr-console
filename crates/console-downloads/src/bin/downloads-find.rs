//! Look for something, and write down what came back.
//!
//!     downloads-find --audio toto africa
//!     downloads-find --video https://youtu.be/FTQbiNvZqaY
//!     downloads-find --book frankenstein
//!
//! Off the panel, and not in it. A search is a question to a site over
//! someone's tether: a second on a good day and fifteen on a bad one, and a
//! card that waited for it would stop answering the buttons for all of them.
//! The panel uses `--report SCRATCH` to receive the result from its own worker. A
//! standalone invocation keeps the result in the cache.
//!
//! The pictures are fetched here too, for the same reason and before the file
//! is written: a list that arrived and then grew pictures a moment later is a
//! list that moves under a thumb already reaching for a row.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use console_core_arguments::{Flag, Operands, Reason, ValidationError, read};

use console_core_never::Never;
use console_downloads::covers::{self, Fetched};
use console_downloads::gutenberg;
use console_downloads::looking::{self, Found, Looked};
use console_downloads::standard_ebooks;
use console_downloads::store::{self, Kind};
use console_repository::RelativePath;

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: "downloads-find",
  about: "look for something of one kind, and write down what comes back",
  flags: &FIND_FLAGS,
  operands: Operands::Verbatim("WORDS"),
};

const FIND_FLAGS: [Flag; 4] = {
  let [sound, film, book] = Kind::FLAGS;
  [sound, film, book, store::REPORT]
};

fn request(words: &[String]) -> Result<(Kind, String, Option<PathBuf>), ValidationError> {
  let read = read(&COMMAND, words);
  let line = read?;
  let choice = line.one_of(&Kind::FLAGS);
  let flag = choice?;
  let Ok(operands) = line.operands();
  let Ok(report) = line.value(store::REPORT);
  let looking_for = operands.join(" ").trim().to_string();

  match (Kind::from_flag(flag.spelling), looking_for.is_empty()) {
    (Ok(Some(kind)), false) => Ok((kind, looking_for, report.map(PathBuf::from))),
    (Ok(Some(_kind)), true) => {
      let Ok(refusal) = line.refusal(Reason::MissingOperands(vec!["WORDS"]));

      Err(refusal)
    }
    (Ok(None), _empty) => {
      let Ok(refusal) = line.refusal(Reason::MissingFlag(
        Kind::FLAGS.iter().map(|flag_2| flag_2.spelling).collect(),
      ));

      Err(refusal)
    }
  }
}

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();

  let (kind, asked, report) = match request(&words) {
    Ok(request) => request,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };

  match find(kind, &asked, report.as_deref()) {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      eprintln!("downloads-find: {fault}");

      ExitCode::FAILURE
    }
  }
}

fn find(kind: Kind, asked: &str, report: Option<&Path>) -> std::io::Result<()> {
  let Ok(cache) = store::cache();

  match ::core::primitive::bool::then_some(cache.is_none() && report.is_none(), ()) {
    ::core::option::Option::Some(()) => {
      return Err(std::io::Error::other(
        "no HOME, so there is nowhere to write what was found",
      ));
    }
    ::core::option::Option::None => {}
  }

  let Ok(looked) = look(kind, asked);

  match &cache {
    Some(cache_2) => {
      let Ok(()) = fill_covers(cache_2, report, &looked.found);
    }
    None => {}
  }

  match (report, cache) {
    (Some(_), _) => {
      let said = looking::serialize(&looked).map_err(std::io::Error::other)?;
      println!("{said}");

      Ok(())
    }
    (None, Some(cache_3)) => store::keep(&cache_3, kind, &looked),
    (None, None) => Err(std::io::Error::other("no search cache")),
  }
}

fn look(kind: Kind, asked: &str) -> Result<Looked, Never> {
  match kind {
    Kind::Sound | Kind::Film => {
      let Ok(arguments) = looking::search(asked);

      ran(Search {
        kind,
        asked,
        arguments: &arguments,
        read: looking::found_in,
      })
    }
    Kind::Book => books(asked),
  }
}

fn books(asked: &str) -> Result<Looked, Never> {
  let Ok(standard) = standard_ebooks::search(asked);
  let Ok(standard_2) = ran(Search {
    kind: Kind::Book,
    asked,
    arguments: &standard,
    read: standard_ebooks::found_in,
  });
  let Ok(plain) = gutenberg::search(asked);
  let Ok(plain_2) = ran(Search {
    kind: Kind::Book,
    asked,
    arguments: &plain,
    read: gutenberg::found_in,
  });

  let fault = match (standard_2.fault.is_empty(), plain_2.fault.is_empty()) {
    (false, false) => gutenberg::NOT_ANSWERING.to_string(),
    (true, _) | (_, true) => String::new(),
  };

  let found = standard_2.found.into_iter().chain(plain_2.found).collect();

  Ok(Looked {
    asked: asked.to_string(),
    fault,
    found,
  })
}

#[derive(Clone, Copy)]
struct Search<'a> {
  kind: Kind,
  asked: &'a str,
  arguments: &'a [String],
  read: fn(&str) -> Result<Vec<Found>, Never>,
}

fn ran(search: Search<'_>) -> Result<Looked, Never> {
  let Search {
    kind,
    asked,
    arguments,
    read,
  } = search;
  let Ok(missing) = looking::missing_tool_message(kind);
  let asked_2 = asked.to_string();

  let (program, rest) = match arguments.split_first() {
    Some((program, rest)) => (program, rest),
    None => {
      return Ok(Looked {
        asked: asked_2,
        fault: missing.to_string(),
        found: Vec::new(),
      });
    }
  };

  let done = match Command::new(program).args(rest).output() {
    Ok(done) => done,
    Err(_would_not_start) => {
      return Ok(Looked {
        asked: asked_2,
        fault: missing.to_string(),
        found: Vec::new(),
      });
    }
  };

  Ok(match ::core::primitive::bool::then_some(done.status.success(), ()) {
    ::core::option::Option::Some(()) => match std::str::from_utf8(&done.stdout) {
      Ok(text) => {
        let Ok(found) = read(text);

        Looked {
          asked: asked_2,
          fault: String::new(),
          found,
        }
      }
      Err(fault) => Looked {
        asked: asked_2,
        fault: format!("Search returned text that is not UTF-8: {fault}"),
        found: Vec::new(),
      },
    },
    ::core::option::Option::None => {
      let Ok(fault) = match std::str::from_utf8(&done.stderr) {
        Ok(text) => looking::complaint(text),
        Err(fault) => Ok(format!("Search error output is not UTF-8: {fault}")),
      };

      Looked {
        asked: asked_2,
        fault,
        found: Vec::new(),
      }
    }
  })
}

fn fill_covers(cache: &Path, report: Option<&Path>, found: &[Found]) -> Result<(), Never> {
  let Ok(pictures) = store::pictures(cache);

  match std::fs::create_dir_all(&pictures) {
    Ok(()) => {}
    Err(fault) => {
      eprintln!("downloads-find: preparing {}: {fault}", pictures.display());

      return Ok(());
    }
  }

  match report {
    Some(scratch) => stage_covers(cache, scratch, found),
    None => match store::Scratch::make("download-pictures") {
      Ok(work) => {
        let Ok(scratch) = work.path();

        stage_covers(cache, scratch, found)
      }
      Err(fault) => {
        eprintln!("downloads-find: preparing cover scratch: {fault}");

        Ok(())
      }
    },
  }
}

fn stage_covers(cache: &Path, scratch: &Path, found: &[Found]) -> Result<(), Never> {
  for result in found {
    let Ok(()) = picture(cache, scratch, result);
  }

  Ok(())
}

fn picture(cache: &Path, scratch: &Path, found: &Found) -> Result<(), Never> {
  let at = match store::picture_of(cache, &found.id) {
    Ok(Some(at)) => at,
    Ok(None) | Err(_) => return Ok(()),
  };

  match found.picture.is_empty().then_some(()) {
    Some(()) => return Ok(()),
    None => {}
  }

  match console_core_atomic_writes::read_prefix(&at, 1) {
    Ok(_prefix) => return Ok(()),
    Err(fault) => match (fault.kind() == std::io::ErrorKind::NotFound).then_some(()) {
      Some(()) => {}
      None => eprintln!("downloads-find: reloading an unreadable cover {}: {fault}", at.display()),
    },
  }

  let part_name = format!("{}.part", found.id);
  let child = match RelativePath::new(Path::new(&part_name)) {
    Ok(child) => child,
    Err(fault) => {
      eprintln!("downloads-find: invalid cover filename: {fault}");

      return Ok(());
    }
  };
  let part = scratch.join(child);
  let Ok(fetched) = covers::fetched(&found.picture, &part);

  match fetched {
    Fetched::Arrived => {
      let Ok(()) = drawn_out(&part, &at);
    }
    Fetched::Failed => {}
  }

  match store::remove_if_present(&part) {
    Ok(()) => {}
    Err(fault) => eprintln!("downloads-find: clearing {}: {fault}", part.display()),
  }

  Ok(())
}

fn drawn_out(part: &Path, at: &Path) -> Result<(), Never> {
  match covers::shrunk(part, at) {
    Ok(()) => {}
    Err(fault) => {
      eprintln!("downloads-find: {}: {fault}", part.display());
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn words(said: &[&str]) -> Result<Vec<String>, Never> {
    Ok(said.iter().map(|word| (*word).to_string()).collect())
  }

  #[test]
  fn what_a_person_types_reads_as_words_after_two_dashes_even_when_it_starts_with_dashes() {
    let Ok(typed) = words(&["--audio", "--", "--toto africa"]);
    let Ok(plain) = words(&["--video", "toto", "africa"]);
    let Ok(no_kind) = words(&["toto"]);

    assert_eq!(
      request(&typed),
      Ok((Kind::Sound, "--toto africa".to_string(), None))
    );
    assert_eq!(
      request(&plain),
      Ok((Kind::Film, "toto africa".to_string(), None))
    );
    assert_eq!(
      request(&no_kind).map_err(|refusal| refusal.reason),
      Err(Reason::MissingFlag(vec!["--audio", "--video", "--book"]))
    );
  }

  #[test]
  fn report_leaves_the_search_words_untouched() -> Result<(), Never> {
    let typed = words(&["--book", "--report", "scratch", "--", "--first edition"])?;

    assert_eq!(
      request(&typed),
      Ok((
        Kind::Book,
        "--first edition".to_string(),
        Some(PathBuf::from("scratch"))
      ))
    );

    Ok(())
  }
}
