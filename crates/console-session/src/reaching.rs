//! Invoking a program as the desktop user from a remote command boundary.
//!
//! runuser preserves the child's exit status. Device confirmation uses that
//! status to distinguish a decline from a card that could not be displayed.

use console_core_external_programs::{Program, shell};
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Whom<'a>(pub &'a str);

pub struct Confirmation<'a> {
  pub label: &'a str,
  pub question: &'a str,
}

pub fn confirmation(whom: Whom<'_>, card: Confirmation<'_>) -> Result<String, Never> {
  let Ok(console) = InternalProgram::Console.path();

  as_user(
    whom,
    &[console, "device-confirm", card.label, card.question],
  )
}

pub fn session_words<'a>(
  whom: Option<Whom<'a>>,
  arguments: &[&'a str],
) -> Result<Vec<&'a str>, Never> {
  let Ok(console) = InternalProgram::Console.path();
  let owner = whom.map_or("-", |whom_2| whom_2.0);
  let mut words = vec![console, "device-session", owner];
  words.extend_from_slice(arguments);

  Ok(words)
}

pub fn user_words<'a>(whom: Whom<'a>, arguments: &[&'a str]) -> Result<Vec<&'a str>, Never> {
  let Ok(runuser) = Program::Runuser.name();
  let mut words = vec![runuser, "-u", whom.0, "--"];
  words.extend_from_slice(arguments);

  Ok(words)
}

pub fn as_user(whom: Whom<'_>, arguments: &[&str]) -> Result<String, Never> {
  let Ok(words) = user_words(whom, arguments);

  shell::join(words)
}
