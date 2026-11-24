//! Where the device is named, and the one place it is read.
//!
//! `CONSOLE_HOST` has no default, because an address is someone's machine and
//! this tree does not carry one. The programs in `console-device` may not read
//! it: they are `console_core_state_machine::Machine`s and a value that arrives
//! through the side of one is a value no transcript can put a different answer
//! in. So it is read out here, by the binaries, and handed in as the first word
//! of an arguments. It is its own crate because the publish and the device
//! stage want the name and nothing else a deploy is made of.
//!
//! Absent and unreadable are not the same thing. Nothing set means there is no
//! device to talk to, which the programs say for themselves and much better
//! than this could; a variable that is set to something no name could be is a
//! fault, and saying "there is no device" about it would send someone looking
//! at the wrong end of the machine.

pub const HOST: &str = "CONSOLE_HOST";

use console_core_external_programs::{Program, shell};
use console_core_never::Never;

pub fn homes() -> Result<[&'static str; 6], Never> {
  let Ok(ls) = Program::Ls.name();

  Ok([
    ls,
    "-1Lp",
    "--color=never",
    "--quoting-style=literal",
    "--",
    "/home",
  ])
}

pub fn fallback() -> Result<[&'static str; 4], Never> {
  let Ok(id) = Program::Id.name();

  Ok([id, "-nu", "--", "1000"])
}

pub fn homes_command() -> Result<String, Never> {
  let Ok(words) = homes();

  shell::join(words)
}

pub fn fallback_command() -> Result<String, Never> {
  let Ok(words) = fallback();

  shell::join(words)
}

pub fn sole_home(said: &str) -> Result<Option<&str>, Never> {
  let mut homes = said.lines();
  let first = match homes
    .next()
    .and_then(|directory| directory.strip_suffix('/'))
  {
    Some(directory) => directory,
    None => return Ok(None),
  };

  match homes.next() {
    None => account(first),
    Some(_another_line) => Ok(None),
  }
}

pub fn account(said: &str) -> Result<Option<&str>, Never> {
  let name = said.trim_end_matches('\n');

  let valid = !name.is_empty()
    && name
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || b"_.-$".contains(&byte))
    && !matches!(name, "." | "..");

  Ok(valid.then_some(name))
}

#[derive(Debug)]
pub struct Unnamed(pub std::env::VarError);

impl std::fmt::Display for Unnamed {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(to, "{HOST}: {}", self.0)
  }
}

impl std::error::Error for Unnamed {}

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  allow(
    explicit026_env_read_once,
    reason = "CONSOLE_HOST names the device, and the head above says why it is read out here beside the binaries rather than inside a program. Three other crates read it before this was the one place"
  )
)]
pub fn device() -> Result<String, Unnamed> {
  match std::env::var(HOST) {
    Ok(said) => Ok(said),
    Err(std::env::VarError::NotPresent) => Ok(String::new()),
    Err(fault) => Err(Unnamed(fault)),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn only_a_single_directory_supplies_the_owner_name() {
    assert_eq!(sole_home("someone/\n"), Ok(Some("someone")));
    assert_eq!(sole_home("someone/\nother/\n"), Ok(None));
    assert_eq!(sole_home("someone/\nnotes\n"), Ok(None));
    assert_eq!(sole_home("someone\n"), Ok(None));
    assert_eq!(sole_home("unexpected syntax; no/\n"), Ok(None));
    assert_eq!(account("another_user\n"), Ok(Some("another_user")));
  }
}
