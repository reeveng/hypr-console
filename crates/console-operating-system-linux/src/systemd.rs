//! Child process trees owned by systemd user scopes.

use console_core_external_programs::{Installed, Program, installed};
use console_core_never::Never;
use std::ffi::{OsStr, OsString};
use std::process::Command;
use std::process::Stdio;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrapped {
  InAScope,
  AsItWasHandedIn,
}

pub fn in_a_scope_of_its_own(
  named: Option<&str>,
  arguments: &[String],
) -> Result<(Wrapped, Vec<String>), Never> {
  let Ok(scopes) = scopes();

  match scopes {
    Scopes::Unavailable => return Ok((Wrapped::AsItWasHandedIn, arguments.to_vec())),
    Scopes::Available => {}
  }

  let Ok(words) = scope_words(named, arguments);

  Ok((Wrapped::InAScope, words))
}

pub fn command_in_a_scope_of_its_own(
  named: Option<&str>,
  program: &OsStr,
  arguments: &[String],
) -> Result<(Wrapped, Command), Never> {
  let Ok(available) = scopes();

  match available {
    Scopes::Unavailable => {
      let mut command = Command::new(program);
      command.args(arguments);

      Ok((Wrapped::AsItWasHandedIn, command))
    }
    Scopes::Available => {
      let words: Vec<OsString> = std::iter::once(program.to_os_string())
        .chain(arguments.iter().map(OsString::from))
        .collect();
      let Ok(words_2) = scope_words(named, &words);
      let Ok(mut command) = Program::SystemdRun.command();

      command.args(words_2.iter().skip(1));

      Ok((Wrapped::InAScope, command))
    }
  }
}

fn scope_words<Word>(named: Option<&str>, arguments: &[Word]) -> Result<Vec<Word>, Never>
where
  Word: Clone + From<String> + From<&'static str>,
{
  let unit = named.map(|name| Word::from(format!("--unit={name}")));
  let Ok(program) = Program::SystemdRun.name();

  Ok(
    std::iter::once(Word::from(program))
      .chain(["--user", "--scope", "--quiet"].map(Word::from))
      .chain(unit)
      .chain(std::iter::once(Word::from("--")))
      .chain(arguments.iter().cloned())
      .collect(),
  )
}

pub fn nothing_left_in(named: &str) -> Result<(), Never> {
  let Ok(scopes) = scopes();

  match scopes {
    Scopes::Unavailable => return Ok(()),
    Scopes::Available => {}
  }

  let Ok(mut asking) = Program::Systemctl.command();

  let _ended = asking
    .args(["--user", "stop", "--no-block", &format!("{named}.scope")])
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status();

  Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scopes {
  Available,
  Unavailable,
}

pub fn scopes() -> Result<Scopes, Never> {
  let Ok(systemd_run) = Program::SystemdRun.name();
  let Ok(found) = installed(systemd_run);

  match found {
    Installed::No => return Ok(Scopes::Unavailable),
    Installed::Yes => {}
  }

  let Ok(mut asking) = Program::Systemctl.command();
  let answered = asking
    .args(["--user", "show", "-p", "Version"])
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status();

  Ok(match answered {
    Ok(how) => match ::core::primitive::bool::then_some(how.success(), ()) {
      ::core::option::Option::Some(()) => Scopes::Available,
      ::core::option::Option::None => Scopes::Unavailable,
    },
    Err(_no_systemctl) => Scopes::Unavailable,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn scope_arguments_preserve_non_utf8_paths_and_literal_arguments() {
    use std::os::unix::ffi::OsStringExt;

    let program = OsString::from_vec(b"/staged/\xff/program".to_vec());
    let arguments = vec![program.clone(), OsString::from("two words; $(untouched)")];
    let Ok(words) = scope_words(None, &arguments);

    assert_eq!(words.get(4), Some(&OsString::from("--")));
    assert_eq!(words.get(5), Some(&program));
    assert_eq!(words.get(6), arguments.get(1));
  }

  #[test]
  fn the_wrap_is_in_front_of_everything_else() {
    let arguments = vec![
      "firefox".to_string(),
      "--new-window".to_string(),
      "https://example.com".to_string(),
    ];
    let Ok((wrapped, made)) = in_a_scope_of_its_own(None, &arguments);

    match wrapped {
      Wrapped::AsItWasHandedIn => {
        eprintln!("skipped: no systemd-run on PATH; scopes cannot be made");

        return;
      }
      Wrapped::InAScope => {}
    }

    assert_eq!(
      made,
      vec![
        "systemd-run".to_string(),
        "--user".to_string(),
        "--scope".to_string(),
        "--quiet".to_string(),
        "--".to_string(),
        "firefox".to_string(),
        "--new-window".to_string(),
        "https://example.com".to_string(),
      ]
    );
  }

  #[test]
  fn a_program_named_like_a_flag_is_kept_a_program() {
    let arguments = vec!["--something".to_string()];
    let Ok((wrapped, made)) = in_a_scope_of_its_own(None, &arguments);

    match wrapped {
      Wrapped::AsItWasHandedIn => return,
      Wrapped::InAScope => {}
    }

    assert_eq!(
      made.get(4).map(String::as_str),
      Some("--"),
      "the terminator is what keeps the program a program"
    );
    assert!(
      made.iter().take(4).all(|word| word != "--"),
      "the terminator is what keeps the program a program"
    );
    assert_eq!(made.last().map(String::as_str), Some("--something"));
  }

  #[test]
  fn a_scope_that_is_named_can_be_stopped_by_that_name() {
    let arguments = vec!["sleep".to_string()];
    let Ok((wrapped, made)) = in_a_scope_of_its_own(Some("console-run-1"), &arguments);

    match wrapped {
      Wrapped::AsItWasHandedIn => return,
      Wrapped::InAScope => {}
    }

    assert!(
      made.contains(&"--unit=console-run-1".to_string()),
      "a scope nothing named is a scope nothing can stop: {made:?}"
    );
  }
}
