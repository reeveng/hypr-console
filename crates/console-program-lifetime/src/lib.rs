//! Shared lifetime policy for detached work and requests to stop running work.
//!
//! A detached child belongs to whoever asked to run it. It can be waited for
//! or reaped after it ends. Dropping the handle leaves it running. Dropping
//! a detached thread handle likewise leaves the thread running.
//!
//! A process bound to its parent's lifetime needs help from the host. The
//! platform adapter supplies process-tree cleanup and interruption by identity.

pub mod threads;

use console_core_never::Never;
use std::io;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Insistence {
  Ask,
  Insist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reached {
  Yes,
  Gone,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Interruption {
  #[default]
  None,
  Requested,
  Insisting,
}

impl Interruption {
  pub fn again(self) -> Result<(Insistence, Interruption), Never> {
    Ok(match self {
      Interruption::None => (Insistence::Ask, Interruption::Requested),
      Interruption::Requested | Interruption::Insisting => {
        (Insistence::Insist, Interruption::Insisting)
      }
    })
  }
}

#[derive(Debug)]
pub struct Detached {
  child: Child,
}

pub fn let_go(command: &mut Command) -> io::Result<Detached> {
  let child = command.spawn()?;

  Ok(Detached { child })
}

impl Detached {
  pub fn take_stdin(&mut self) -> Result<Option<ChildStdin>, Never> {
    Ok(self.child.stdin.take())
  }

  pub fn take_stdout(&mut self) -> Result<Option<ChildStdout>, Never> {
    Ok(self.child.stdout.take())
  }

  pub fn take_stderr(&mut self) -> Result<Option<ChildStderr>, Never> {
    Ok(self.child.stderr.take())
  }

  pub fn still(&mut self) -> Result<Still, Never> {
    still(&mut self.child)
  }

  pub fn wait(&mut self) -> io::Result<ExitStatus> {
    self.child.wait()
  }

  pub fn kill(&mut self) -> io::Result<()> {
    self.child.kill()
  }

  pub fn id(&self) -> Result<u32, Never> {
    Ok(self.child.id())
  }
}

pub fn reap(started: Vec<Detached>) -> Result<Vec<Detached>, Never> {
  Ok(
    started
      .into_iter()
      .filter_map(|mut one| {
        let Ok(still) = one.still();

        match still {
          Still::Running => Some(one),
          Still::Ended => None,
        }
      })
      .collect(),
  )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Still {
  Running,
  Ended,
}

fn still(child: &mut Child) -> Result<Still, Never> {
  Ok(match child.try_wait() {
    Ok(None) => Still::Running,
    Ok(Some(_ended)) => Still::Ended,
    Err(_unasked) => Still::Ended,
  })
}
