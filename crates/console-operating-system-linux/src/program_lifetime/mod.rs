//! Linux process lifetimes.
//!
//! A child started with [`alongside`] belongs to its parent. The pre-exec hook
//! installs `PR_SET_PDEATHSIG` so Linux signals it when the spawning thread
//! dies. Dropping the guard kills and reaps the child. A [`Forked`] process
//! adds a dedicated process group and an identity that can be interrupted.
//! Transient systemd scopes can manage a full desktop run across process groups.
//! See [`crate::systemd`] for the user manager integration.

mod forks;
pub(crate) mod hooks;
mod pipe;
mod unix;

pub(crate) use forks::ForkCancellation;
pub use forks::{Forked, ProcessIdentity, forked};
pub use pipe::{PipeReady, TimedPipe, TimedReadPipe};
pub use unix::signal;

use console_core_never::Never;
use console_program_lifetime::Still;
use std::io;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus};

#[derive(Debug)]
pub struct BoundToParent {
  child: Child,
}

pub fn alongside(command: &mut Command) -> io::Result<BoundToParent> {
  let Ok(()) = hooks::bind(command);
  let child = command.spawn()?;
  Ok(BoundToParent { child })
}

impl BoundToParent {
  pub fn take_stdout(&mut self) -> Result<Option<ChildStdout>, Never> {
    Ok(self.child.stdout.take())
  }

  pub fn take_stdin(&mut self) -> Result<Option<ChildStdin>, Never> {
    Ok(self.child.stdin.take())
  }

  pub fn take_stderr(&mut self) -> Result<Option<ChildStderr>, Never> {
    Ok(self.child.stderr.take())
  }

  pub fn id(&self) -> Result<u32, Never> {
    Ok(self.child.id())
  }

  pub fn wait(&mut self) -> io::Result<ExitStatus> {
    self.child.wait()
  }

  pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
    self.child.try_wait()
  }

  pub fn still(&mut self) -> Result<Still, Never> {
    Ok(match self.try_wait() {
      Ok(None) => Still::Running,
      Ok(Some(_ended)) => Still::Ended,
      Err(_unasked) => Still::Ended,
    })
  }
}

impl Drop for BoundToParent {
  fn drop(&mut self) {
    let Ok(()) = kill_and_reap(&mut self.child);
  }
}

fn kill_and_reap(child: &mut Child) -> Result<(), Never> {
  match child.kill() {
    Ok(()) => match child.wait() {
      Ok(_) => {}
      Err(fault) => console_core_logging::warn!("could not reap child {}: {fault}", child.id()),
    },
    Err(fault) => match child.try_wait() {
      Ok(Some(_ended)) => return Ok(()),
      Ok(None) => console_core_logging::warn!("could not stop child {}: {fault}", child.id()),
      Err(check) => console_core_logging::warn!("could not stop child {}: {fault}; checking: {check}", child.id()),
    },
  }

  Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
fn process_presence(process: rustix::process::Pid) -> Result<Still, Never> {
  Ok(match rustix::process::test_kill_process(process) {
    Err(rustix::io::Errno::SRCH) => Still::Ended,
    Ok(()) => Still::Running,
    Err(fault) => {
      console_core_logging::trace!("checking the fixture process: {fault}");

      Still::Running
    }
  })
}
