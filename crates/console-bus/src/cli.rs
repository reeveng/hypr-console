//! Command construction for operations that still use the busctl client.

use std::process::Command;

use console_core_external_programs::Program;
use console_core_never::Never;

const CLIENT: Program = Program::Busctl;

pub fn arguments<'a>(words: impl IntoIterator<Item = &'a str>) -> Result<Vec<&'a str>, Never> {
  let Ok(name) = CLIENT.name();

  Ok(std::iter::once(name).chain(words).collect())
}

pub fn owned_arguments(
  words: impl IntoIterator<Item = impl Into<String>>,
) -> Result<Vec<String>, Never> {
  CLIENT.words(words.into_iter().map(Into::into).collect())
}

pub fn effect(words: &[&str]) -> Result<console_program_contract::Command, Never> {
  console_program_contract::Command::external(CLIENT, words)
}

pub fn command(
  words: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Result<Command, Never> {
  let Ok(mut asking) = CLIENT.command();
  asking.args(words);

  Ok(asking)
}

pub fn monitor_session(rules: &[String]) -> Result<Command, Never> {
  let Ok(mut buffering) = Program::Stdbuf.command();
  let Ok(words) = arguments(["--user", "monitor"]);
  buffering.arg("-oL").args(words).args(rules);

  Ok(buffering)
}
