//! The command that would start a window's program again.
//!
//! Four questions in order, and the first that names something this machine can
//! run wins: what the process was started with, what its executable is called,
//! what the window called itself when it opened, and what it titled itself. The
//! first is the only one that carries arguments, and it is the only one that is
//! ever right about a program started with any; the others are there because a
//! process can be gone, or wrapped, or have rewritten its own command line into
//! something no shell would take back.
//!
//! ## Nothing is asked of `which`
//!
//! The fork this came from ran `which` once per candidate per window, which is a
//! process spawned to read a variable this program already has. `PATH` is a list
//! of directories and the question is whether one of them holds a file someone
//! can run, so that is what is asked. It also keeps a name out of
//! [`console_core_external_programs::Program`] that would not have belonged
//! there: that list is the programs *this desktop* runs and did not write, and
//! the names here come from other people's windows.
//!
//! ## A command line keeps its words
//!
//! `foot --title=Two Words` is two arguments, and flattening it into a string
//! first hands the shell that reopens the window three. So the vector stays a
//! vector until the last moment, and each word is quoted on its own. The other
//! way round, an Electron program rewrites its own command line as a single
//! argument with the spaces still in it, and quoting that whole names a program
//! that does not exist -- so a lone argument carrying spaces is read back as the
//! words it used to be.

use std::borrow::Cow;
use std::collections::HashMap;

use console_compositor::Window;

use crate::Unresumed;
use console_core_external_programs::{Installed, installed};
use console_core_never::Never;

pub fn on_path(command: &str) -> Result<Installed, Never> {
  match command.split_whitespace().next() {
    Some(binary) => installed(binary.trim_matches('\'')),
    None => Ok(Installed::No),
  }
}

pub fn quote_word(word: &str) -> Result<String, Never> {
  let safe = |letter: char| letter.is_ascii_alphanumeric() || "-_./:=@+,".contains(letter);

  Ok(
    match ::core::primitive::bool::then_some(!word.is_empty() && word.chars().all(safe), ()) {
      ::core::option::Option::Some(()) => word.to_string(),
      ::core::option::Option::None => format!("'{}'", word.replace('\'', r"'\''")),
    },
  )
}

fn renamed_command(command: &str) -> Result<Option<(&'static str, &str)>, Never> {
  let (binary, arguments) = match command
    .find(char::is_whitespace)
    .and_then(|end| command.split_at_checked(end))
  {
    Some(parts) => parts,
    None => (command, ""),
  };
  let executable = binary.trim_matches('\'');
  let name = match executable.strip_prefix("/usr/local/bin/") {
    Some(named) => named,
    None => executable,
  };
  let Ok(renamed) = console_core_places::session::renamed(name);

  Ok(renamed.map(|program| (program, arguments)))
}

pub(crate) fn launch_command(command: &str) -> Result<Cow<'_, str>, Never> {
  let Ok(renamed) = renamed_command(command);

  Ok(match renamed {
    Some((name, arguments)) => match on_path(name) {
      Ok(Installed::Yes) => Cow::Owned(format!("{name}{arguments}")),
      Ok(Installed::No) => Cow::Borrowed(command),
    },
    None => Cow::Borrowed(command),
  })
}

fn separate_words(arguments: &[String]) -> Result<Vec<String>, Never> {
  Ok(match arguments {
    [only] => {
      match ::core::primitive::bool::then_some(only.split_whitespace().nth(1).is_some(), ()) {
        ::core::option::Option::Some(()) => only.split_whitespace().map(str::to_string).collect(),
        ::core::option::Option::None => arguments.to_vec(),
      }
    }
    every => every.to_vec(),
  })
}

fn command_from_argv(arguments: &[String]) -> Result<String, Never> {
  let Ok(words) = separate_words(arguments);

  let (first, rest) = match words.split_first() {
    Some(split) => split,
    None => return Ok(String::new()),
  };

  let binary = match first.rsplit('/').next() {
    Some(binary) => binary.to_string(),
    None => first.to_string(),
  };

  Ok(
    std::iter::once(binary)
      .chain(rest.iter().cloned())
      .map(|word| {
        let Ok(quoted) = quote_word(&word);

        quoted
      })
      .collect::<Vec<String>>()
      .join(" "),
  )
}

fn from_its_command_line(window: &Window) -> Result<String, Unresumed> {
  let at = format!("/proc/{}/cmdline", window.pid);

  let said =
    std::fs::read_to_string(&at).map_err(|fault| Unresumed::Unsaid(at.clone(), fault))?;

  let arguments: Vec<String> = said
    .split('\0')
    .filter(|argument| !argument.is_empty())
    .map(str::to_string)
    .collect();

  match arguments.first() {
    Some(_it_said_something) => {}
    None => return Err(Unresumed::SaidNothing(at)),
  }

  let Ok(pid) = console_core_number_conversion::fitted::<i64, i32>(window.pid);
  let Ok(restored) = crate::terminal::restore_command(&arguments, pid);
  let reported = || {
    let first_argument = arguments.first()?;
    let executable = first_argument.rsplit('/').next()?;
    let Ok(known) = console_core_places::session::application(executable);
    let application = known?;
    let pid_2 = match u32::try_from(pid) {
      ::core::result::Result::Ok(value) => ::core::option::Option::Some(value),
      ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
    }?;
    let Ok(arguments_3) = console_core_places::session::arguments(pid_2, application);
    Some(
      std::iter::once(executable.to_owned())
        .chain({
          let checked_value = arguments_3?;
          checked_value
        })
        .collect(),
    )
  };

  let arguments_2 = match restored.or_else(reported) {
    Some(restored_2) => restored_2,
    None => arguments,
  };

  let Ok(command) = command_from_argv(&arguments_2);

  Ok(command)
}

fn from_its_executable(window: &Window) -> Result<String, Unresumed> {
  let at = format!("/proc/{}/exe", window.pid);

  let target = std::fs::read_link(&at).map_err(|fault| Unresumed::Unsaid(at.clone(), fault))?;

  match target.file_name() {
    Some(named) => match named.to_str() {
      Some(text) => Ok(text.to_string()),
      None => Err(Unresumed::Unsaid(
        at,
        std::io::Error::new(
          std::io::ErrorKind::InvalidData,
          "the executable name is not UTF-8",
        ),
      )),
    },
    None => Err(Unresumed::Untitled(at)),
  }
}

fn from_what_it_called_itself(window: &Window) -> Result<String, Unresumed> {
  Ok(window.first_class.to_lowercase())
}

fn from_what_it_titled_itself(window: &Window) -> Result<String, Unresumed> {
  Ok(window.first_title.to_lowercase())
}

type Describe = fn(&Window) -> Result<String, Unresumed>;

const ASKING: [Describe; 4] = [
  from_its_command_line,
  from_its_executable,
  from_what_it_called_itself,
  from_what_it_titled_itself,
];

pub fn from_desktop_files() -> Result<HashMap<String, String>, Never> {
  let found = console_applications::found::machine()?;

  Ok(
    found
      .applications
      .into_iter()
      .map(|(name, application)| (name, application.command))
      .collect(),
  )
}

pub fn what_starts_it(
  window: &Window,
  known: &HashMap<String, String>,
) -> Result<String, Unresumed> {
  for asking in ASKING {
    let command = match asking(window) {
      Ok(command) => command,
      Err(_that_one_could_not_say) => continue,
    };

    let Ok(on_path) = on_path(&command);

    match on_path {
      Installed::Yes => return Ok(command),
      Installed::No => {}
    }

    match known.get(&command) {
      Some(said) => return Ok(said.clone()),
      None => {}
    }
  }

  from_its_command_line(window)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn command(words: &[&str]) -> Result<String, Never> {
    let arguments: Vec<String> = words.iter().map(|word| (*word).to_string()).collect();

    command_from_argv(&arguments)
  }

  #[test]
  fn a_saved_application_command_keeps_its_arguments_when_renamed() {
    assert_eq!(
      renamed_command("books --resume 'My Book.pdf'"),
      Ok(Some((
        "console-application-books",
        " --resume 'My Book.pdf'"
      )))
    );
    assert_eq!(
      renamed_command("/usr/local/bin/viewer '/home/reader/Some Photo.png'"),
      Ok(Some((
        "console-application-media-viewer",
        " '/home/reader/Some Photo.png'"
      )))
    );
    assert_eq!(
      renamed_command("calculator"),
      Ok(Some(("console-application-calculator", "")))
    );
    assert_eq!(renamed_command("/opt/viewer photo.png"), Ok(None));
    assert_eq!(renamed_command("books-catalog"), Ok(None));
  }

  #[test]
  fn a_program_comes_back_by_its_name_rather_than_by_where_it_was_installed() {
    assert_eq!(command(&["/usr/bin/firefox"]), Ok("firefox".to_string()));
    assert_eq!(command(&["code"]), Ok("code".to_string()));
    assert_eq!(
      command(&["/usr/bin/firefox", "--new-window"]),
      Ok("firefox --new-window".to_string())
    );
    assert_eq!(
      command(&["/nix/store/.firefox-wrapped"]),
      Ok(".firefox-wrapped".to_string())
    );
  }

  #[test]
  fn keeps_an_argument_that_carries_a_space() {
    assert_eq!(
      command(&["/usr/bin/foot", "--title=Two Words"]),
      Ok("foot '--title=Two Words'".to_string())
    );
    assert_eq!(
      command(&["foot", "-e", "sh", "-c", "sleep 900"]),
      Ok("foot -e sh -c 'sleep 900'".to_string())
    );
  }

  #[test]
  fn reads_back_the_words_of_a_command_line_a_program_flattened() {
    assert_eq!(
      command(&["/usr/lib/signal-desktop/signal-desktop --"]),
      Ok("signal-desktop --".to_string())
    );
  }

  #[test]
  fn quotes_a_word_that_would_otherwise_be_shell_syntax() {
    assert_eq!(quote_word("plain"), Ok("plain".to_string()));
    assert_eq!(
      quote_word("--working-directory=/home/ada"),
      Ok("--working-directory=/home/ada".to_string())
    );
    assert_eq!(quote_word("a b"), Ok("'a b'".to_string()));
    assert_eq!(quote_word("$HOME"), Ok("'$HOME'".to_string()));
    assert_eq!(quote_word("it's"), Ok(r"'it'\''s'".to_string()));
    assert_eq!(quote_word(""), Ok("''".to_string()));
  }

  #[test]
  fn a_command_line_with_nothing_in_it_names_nothing() {
    assert_eq!(command_from_argv(&[]), Ok(String::new()));
  }

  #[test]
  fn a_program_this_machine_has_is_told_apart_from_one_it_does_not() {
    assert_eq!(on_path("sh"), Ok(Installed::Yes));
    assert_eq!(
      on_path("definitely_not_a_real_command_123456"),
      Ok(Installed::No)
    );
    assert_eq!(on_path(""), Ok(Installed::No));
  }

  #[test]
  fn the_first_word_is_what_is_looked_for_rather_than_the_whole_line() {
    assert_eq!(on_path("sh -c 'echo hello'"), Ok(Installed::Yes));
  }
}
