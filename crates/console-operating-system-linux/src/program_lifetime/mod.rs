//! Linux process lifetimes.
//!
//! A child started with [`alongside`] belongs to its parent. The pre-exec hook
//! installs `PR_SET_PDEATHSIG` so Linux signals it when the spawning thread
//! dies. Dropping the guard kills and reaps the child. A [`Forked`] process
//! adds a dedicated process group and an identity that can be interrupted.
//! Transient systemd scopes can manage a full desktop run across process groups.
//! See [`crate::systemd`] for the user manager integration.
//!
//! [`BoundToParent::terminate_within`] queues SIGKILL and polls for reaping.
//! Once the signal is queued, drop checks for exit without extending the
//! deadline. If the kernel has not made the process reapable, the caller still
//! receives the timeout.

mod forks;
pub(crate) mod hooks;
mod pipe;
mod unix;

pub(crate) use forks::ForkCancellation;
pub use forks::{Forked, ProcessIdentity, forked};
pub use pipe::{PipeReady, TimedPipe, TimedReadPipe};
pub use unix::{interrupt_and_wait, signal};

use console_core_never::Never;
use console_program_lifetime::Still;
use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open};
use std::io;
use std::os::fd::AsFd;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus};
use std::time::Duration;

use console_waiting::{Schedule, clock::LiveClock, until_some_handed_on_result};

#[derive(Debug)]
pub struct BoundToParent {
  child: Child,
  cleanup: Cleanup,
}

#[derive(Debug)]
enum Cleanup {
  KillAndReap,
  ReapIfExited,
}

pub fn alongside(command: &mut Command) -> io::Result<BoundToParent> {
  let Ok(()) = hooks::bind(command);
  let child = command.spawn()?;
  Ok(BoundToParent {
    child,
    cleanup: Cleanup::KillAndReap,
  })
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

  pub fn terminate_within(&mut self, patience: Duration) -> io::Result<ExitStatus> {
    self.child.kill()?;
    self.cleanup = Cleanup::ReapIfExited;

    let Ok(mut clock) = LiveClock::started();
    let Ok(schedule) = Schedule::asking_every(patience, Duration::from_millis(10));
    let exited = until_some_handed_on_result(&mut clock, schedule, self, Self::try_wait)?;

    exited.ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "killed child did not exit"))
  }

  pub fn interrupt_and_wait(&mut self, signal: Signal, wait: Duration) -> io::Result<ExitStatus> {
    let checked_value = self.try_wait()?;

    match checked_value {
      Some(status) => return Ok(status),
      None => {}
    }

    let process = pidfd_open(Pid::from_child(&self.child), PidfdFlags::empty())?;
    interrupt_and_wait(process.as_fd(), signal, wait)?;
    self.wait()
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
    match self.cleanup {
      Cleanup::KillAndReap => {
        let Ok(()) = kill_and_reap(&mut self.child);
      }
      Cleanup::ReapIfExited => match self.child.try_wait() {
        Ok(_) => {}
        Err(fault) => {
          console_core_logging::warn!("could not reap child {}: {fault}", self.child.id())
        }
      },
    }
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
      Err(check) => console_core_logging::warn!(
        "could not stop child {}: {fault}; checking: {check}",
        child.id()
      ),
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
