//! Starting a desktop application with a file. Its Exec field codes specify
//! where the file argument goes. The panel launcher lets the app keep running
//! after Files closes.

use std::path::Path;

use console_applications::entry::DesktopEntry;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_default_applications::editors::{self, Application, Support};

use crate::paths::{self, NonTextPath};

const A_FILE: [&str; 4] = ["%f", "%F", "%u", "%U"];

pub fn programs(kind: &str) -> Result<Vec<Application>, Never> {
  match ::core::primitive::bool::then_some(kind.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      return Ok(Vec::new());
    }
    ::core::option::Option::None => {}
  }

  let installed = editors::installed()?;
  let mut found: Vec<Application> = installed
    .into_iter()
    .filter(|app| {
      let Ok(support) = app.opens(kind);

      support == Support::Yes
    })
    .collect();

  found.sort_by(|one, two| one.says.cmp(&two.says));
  found.dedup_by(|one, two| one.says == two.says);

  Ok(found)
}

pub fn started(at: &Path, path: &Path) -> Result<(), Never> {
  let said = match std::fs::read_to_string(at) {
    Ok(said) => said,
    Err(fault) => {
      console_core_logging::warn!("{}: {fault}", at.display());

      return Ok(());
    }
  };

  let Ok(entry) = DesktopEntry::read(&said);

  let command = match entry.command.filter(|command| !command.is_empty()) {
    Some(command) => command,
    None => {
      console_core_logging::warn!("{}: nothing to run", at.display());

      return Ok(());
    }
  };

  let Ok(words) = console_applications::words::split(command);

  let words_3 = match words {
    Some(words_2) => words_2,
    None => {
      console_core_logging::warn!("{}: {command:?} is not a command", at.display());

      return Ok(());
    }
  };

  let arguments = match with_file_argument(&words_3, path) {
    Ok(arguments) => arguments,
    Err(unavailable) => {
      console_core_logging::warn!("{unavailable}");
      return Ok(());
    }
  };

  let ready = match ::core::primitive::bool::then_some(
    entry.terminal.map(str::to_lowercase).as_deref() == Some("true"),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(alacritty) = Program::Alacritty.name();

      let mut terminal = vec![alacritty.to_string(), "-e".to_string()];
      terminal.extend(arguments);

      terminal
    }
    ::core::option::Option::None => arguments,
  };

  console_panel::running::left_running(&ready)
}

pub fn with_file_argument<'a>(
  words: &[String],
  path: &'a Path,
) -> Result<Vec<String>, NonTextPath<'a>> {
  let checked_value = paths::text(path)?;
  let said = checked_value.to_owned();
  let mut arguments: Vec<String> = Vec::new();
  let mut put = Presence::Nowhere;

  for word in words {
    match ::core::primitive::bool::then_some(A_FILE.contains(&word.as_str()), ()) {
      ::core::option::Option::Some(()) => {
        arguments.push(said.clone());
        put = Presence::InIt;
      }
      ::core::option::Option::None => arguments.push(word.clone()),
    }
  }

  match put {
    Presence::InIt => {}
    Presence::Nowhere => arguments.push(said),
  }

  Ok(arguments)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presence {
  InIt,
  Nowhere,
}

#[cfg(test)]
mod tests {
  use super::*;

  fn words(said: &[&str]) -> Result<Vec<String>, Never> {
    Ok(said.iter().map(|word| (*word).to_string()).collect())
  }

  #[test]
  fn the_file_goes_where_the_entry_says_it_goes() {
    let Ok(asked) = words(&["gimp", "%U"]);
    let Ok(wanted) = words(&["gimp", "/home/ada/beach.jpg"]);

    assert_eq!(
      with_file_argument(&asked, Path::new("/home/ada/beach.jpg")),
      Ok(wanted)
    );
    let Ok(asked_2) = words(&["mpv", "--fullscreen", "%f"]);
    let Ok(wanted_2) = words(&["mpv", "--fullscreen", "/x/film.mkv"]);

    assert_eq!(
      with_file_argument(&asked_2, Path::new("/x/film.mkv")),
      Ok(wanted_2)
    );
  }

  #[test]
  fn an_entry_that_says_nowhere_is_handed_the_file_at_the_end() {
    let Ok(asked) = words(&["librewolf"]);
    let Ok(wanted) = words(&["librewolf", "/x/page.html"]);

    assert_eq!(
      with_file_argument(&asked, Path::new("/x/page.html")),
      Ok(wanted)
    );
  }

  #[test]
  fn a_word_that_only_looks_like_a_code_is_left_alone() {
    let Ok(asked) = words(&["run", "--rate=%50", "%f"]);
    let Ok(wanted) = words(&["run", "--rate=%50", "/x/y"]);

    assert_eq!(with_file_argument(&asked, Path::new("/x/y")), Ok(wanted));
  }
}
