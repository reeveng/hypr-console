//! Starting things, and not waiting for them where waiting would be felt.
//!
//! `left_running` is the one call here that a person made happen: someone
//! pressed a row and an application starts. So it stamps the press, and the
//! three that no one pressed -- asking a program a question, saying something
//! on the bar, drawing a picture in the background -- take the stamp back off,
//! because the environment is inherited and a stamp handed on is a wait
//! measured from a press that was already answered.
//!
//! **A program let go of is still a child of whoever started it.** [`Detached`](console_program_lifetime::Detached)
//! holds one so that it can be reaped and never ends it, and nothing here was
//! doing the reaping: every application started from a row and every notification
//! said stayed in the process table as a dead entry until the panel that
//! started it exited. Read on the device that was eleven notifications panels, a
//! browser and everything else someone had opened and closed that afternoon.
//!
//! A row that asks for something to be run to the end rather than started and
//! left is not started here: it is a task, and `tasks` waits for it on a
//! thread of its own, because the thread that pressed it is the one holding
//! the frame, and a panel that stops drawing while a program runs is a panel
//! nobody can put away.
//!
//! What has been started is waited for on a thread of its own, and reaped
//! the moment it ends. It used to be held in a list and swept the next time
//! something was started, which was bounded while every panel was a process
//! that exited when it closed. The panels are held by one resident program
//! now, and a sweep that waits for the next press is a dead entry under the
//! host for as long as nobody presses anything -- which is exactly the state
//! a closed panel is meant to cost nothing in.

use std::time::Duration;

use console_core_external_programs::Program;
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_operating_system_linux::program_runtime::commands::{Linux, Timing};
use console_program_contract::{Command, Executable};
use console_program_runtime::commands::{self as execution_commands, Launch, Platform, Streams};

pub use console_operating_system_linux::program_runtime::commands::reap_in_background;

pub fn run_output(program: Program, rest: &[&str]) -> Result<String, Never> {
  let Ok(command) = Command::external(program, rest);

  run_command_output(&command)
}

pub fn run_command_output(command: &Command) -> Result<String, Never> {
  let Ok(mut platform) = Linux::new(Timing::Background);

  Ok(
    match execution_commands::run(&mut platform, command, Streams::COLLECT) {
      Ok(answer) => answer.output.trim().to_string(),
      Err(_would_not_start) => String::new(),
    },
  )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Notification<'a> {
  pub summary: &'a str,
  pub body: &'a str,
}

pub fn say(kind: &str, said: Notification<'_>) -> Result<(), Never> {
  let Ok(command) = Command::internal(
    InternalProgram::ConsoleSay,
    &[kind, said.summary, said.body],
  );
  let Ok(mut platform) = Linux::new(Timing::Background);

  match execution_commands::spawn(&mut platform, &command, Streams::QUIET, Launch::Background) {
    Ok(()) => {}
    Err(fault) => {
      console_core_logging::warn!("console-say: {fault}");
      console_core_logging::warn!("{kind}: {} - {}", said.summary, said.body);
    }
  }

  Ok(())
}

pub fn words_of(command: &Command) -> Result<Vec<String>, Never> {
  let program = match command.program {
    Executable::External(external) => {
      let Ok(name) = external.name();

      name.to_owned()
    }
    Executable::Internal(internal) => {
      let Ok(path) = internal.at();

      match path.into_os_string().into_string() {
        Ok(text) => text,
        Err(unrepresentable) => {
          console_core_logging::warn!(
            "console-panel: a command label cannot represent {unrepresentable:?}"
          );

          return Ok(Vec::new());
        }
      }
    }
  };

  Ok(
    std::iter::once(program)
      .chain(command.arguments.iter().cloned())
      .collect(),
  )
}

pub fn and_waited(arguments: Vec<String>) -> Result<(), Never> {
  on_exit(arguments, |_exit| {})
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exit {
  Success,
  Failure { said: String },
}

pub fn on_exit(
  arguments: Vec<String>,
  then: impl FnOnce(Exit) + Send + 'static,
) -> Result<(), Never> {
  let doing = std::thread::spawn(move || {
    let Ok(invocation) =
      console_operating_system_linux::program_runtime::commands::invocation(&arguments);
    let invocation_3 = match invocation {
      Some(invocation_2) => invocation_2,
      None => return,
    };
    let Ok(mut platform) = Linux::new(Timing::Background);
    let exit = match platform.run(&invocation_3, Streams::COLLECT, None) {
      Ok(completion) => match completion.status {
        console_program_contract::ExitStatus::Success => Exit::Success,
        console_program_contract::ExitStatus::Failure(_)
        | console_program_contract::ExitStatus::Interrupt => Exit::Failure {
          said: completion.error_output.trim().to_string(),
        },
      },
      Err(fault) => Exit::Failure {
        said: format!(
          "{} would not start: {fault}",
          invocation_3.program.display()
        ),
      },
    };

    then(exit);
  });

  console_program_lifetime::threads::let_go(doing)
}

pub fn left_running(arguments: &[String]) -> Result<(), Never> {
  let Ok(invocation) =
    console_operating_system_linux::program_runtime::commands::invocation(arguments);
  let invocation_3 = match invocation {
    Some(invocation_2) => invocation_2,
    None => return Ok(()),
  };
  let Ok(mut platform) = Linux::new(Timing::Pressed);

  match platform.spawn(&invocation_3, Streams::QUIET, Launch::Application) {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("left_running: nothing started: {fault}"),
  }

  Ok(())
}

pub const PATIENCE: Duration = Duration::from_secs(45);

pub const SETTLING: Duration = Duration::from_millis(250);

#[cfg(test)]
mod tests {
  use super::*;

  use console_program_lifetime::let_go;
  use console_waiting::{Outcome, Ready, Schedule, until};

  type Failure = Box<dyn std::error::Error>;

  #[test]
  fn a_program_let_go_is_reaped_when_it_ends_rather_than_at_the_next_press() -> Result<(), Failure>
  {
    let mut ending = Program::True.command()?;
    let started = let_go(&mut ending)?;
    let id = started.id()?;
    let at = std::path::PathBuf::from(format!("/proc/{id}"));

    reap_in_background(started)?;

    let patience = Schedule::of(Duration::from_secs(5))?;
    let gone = until(patience, || {
      Ok(match std::fs::metadata(&at) {
        Ok(_still_in_proc) => Ready::NotYet,
        Err(fault) => match ::core::primitive::bool::then_some(
          fault.kind() == std::io::ErrorKind::NotFound,
          (),
        ) {
          ::core::option::Option::Some(()) => Ready::Yes,
          ::core::option::Option::None => Ready::NotYet,
        },
      })
    });

    assert_eq!(
      gone,
      Ok(Outcome::Happened),
      "{} is still in the process table after it ended, so a host that stays up holds a \
             dead entry for every picture it asked for until someone presses something",
      at.display()
    );

    Ok(())
  }

  #[test]
  fn a_program_of_ours_is_run_from_beside_this_one_and_not_from_whatever_path_says()
  -> Result<(), Failure> {
    let runs = console_program_contract::Command::internal(InternalProgram::Files, &["/music"])?;
    let at = InternalProgram::Files.at()?;
    let program = at.to_str().ok_or("the fixture program path is not UTF-8")?;

    assert_eq!(
      words_of(&runs),
      Ok(vec![program.to_owned(), "/music".to_string()]),
      "a panel asked to run files ran the name, which is the installed copy rather than the one \
             staged or built beside it"
    );

    Ok(())
  }

  #[test]
  fn a_program_somebody_else_wrote_is_run_by_its_name() -> Result<(), Failure> {
    let runs = console_program_contract::Command::external(Program::True, &[])?;

    assert_eq!(words_of(&runs), Ok(vec!["true".to_string()]));

    Ok(())
  }
}
