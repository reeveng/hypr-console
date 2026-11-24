//! The panel that was up when the machine went down, so it comes back up.
//!
//! A restart that nobody asked for -- the battery running out, the button held,
//! a deploy restarting the host -- should put back what was on the screen, the
//! way a book comes back at its page. So the host writes which panel it put up
//! and what it was asked with, every time it puts one up, and takes it away
//! when a person puts the panel away: B, a row that closes it, or its own
//! button pressed a second time. Restart and Shut Down are rows that close the
//! panel as they run, so what they leave behind is nothing, and the machine
//! that comes back from them comes back to the desktop rather than to Settings.
//!
//! What is not a person putting it away is left alone. The host being asked to
//! stop and the panel's program being ended are what a machine going down looks
//! like from here, and forgetting then is forgetting the one time it mattered.
//!
//! Put back is the panel's own program started again with the same arguments,
//! which is every road in: it takes the lock, asks the host, and the host draws
//! it as though it had been pressed.

use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_operating_system_linux::program_runtime::commands::{Linux, Timing};
use console_program_contract::{Command, Executable};
use console_program_runtime::commands::{self as execution_commands, Launch, Streams};

use crate::notes::{self, Note};

pub const NOTE: Note<'static> = Note {
  program: "panels",
  called: "left-open",
};

const BETWEEN: char = '\0';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftOpen {
  pub who: String,
  pub arguments: Vec<String>,
}

pub fn serialize(open: &LeftOpen) -> Result<String, Never> {
  let words: Vec<&str> = std::iter::once(open.who.as_str())
    .chain(open.arguments.iter().map(String::as_str))
    .collect();

  Ok(words.join(&BETWEEN.to_string()))
}

pub fn read(said: &str) -> Result<Option<LeftOpen>, Never> {
  let mut words = said.split(BETWEEN);

  Ok(match words.next() {
    Some("") | None => None,
    Some(who) => Some(LeftOpen {
      who: who.to_string(),
      arguments: words.map(str::to_string).collect(),
    }),
  })
}

pub fn opened(open: &LeftOpen) -> Result<(), Never> {
  let Ok(said) = serialize(open);

  notes::write(NOTE, &said)
}

pub fn put_away() -> Result<(), Never> {
  notes::write(NOTE, "")
}

pub fn load() -> Result<Option<LeftOpen>, Never> {
  let Ok(said) = notes::read(NOTE);

  match said {
    Some(said_2) => read(&said_2),
    None => Ok(None),
  }
}

pub fn started(program: InternalProgram, arguments: Vec<String>) -> Result<(), Never> {
  let command = Command {
    program: Executable::Internal(program),
    arguments,
  };
  let Ok(mut platform) = Linux::new(Timing::Background);

  match execution_commands::spawn(&mut platform, &command, Streams::QUIET, Launch::Application) {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("console-panels: putting a panel back: {fault}"),
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_panel_and_its_arguments_come_back_as_they_went_in() {
    let open = LeftOpen {
      who: "settings-panel".to_string(),
      arguments: vec!["Sound".to_string(), "two words".to_string(), String::new()],
    };
    let Ok(said) = serialize(&open);

    assert_eq!(read(&said), Ok(Some(open)));
  }

  #[test]
  fn a_panel_put_away_leaves_nothing_to_put_back() {
    assert_eq!(read(""), Ok(None));
  }
}
