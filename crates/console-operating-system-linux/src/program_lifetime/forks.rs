//! A child somebody is waiting to hear back from, and may want to stop first.
//!
//! effect's `fork`: work started beside the one that asked for it, answered
//! when it ends, interrupted when whoever owns it says so, and never outliving
//! it. [`super::BoundToParent`] already had the last of those and none of the
//! rest, because everything it was written for -- a watcher, an inhibitor --
//! is meant to run until it is dropped. A search, a download and a pairing are
//! meant to end, and somebody is waiting to be told how.
//!
//! **What a fork starts is stopped with it.** A fork is a process group of its
//! own, and when the program at its head ends, whatever it left in that group
//! is ended too. `downloads-get` runs yt-dlp with `output`, and yt-dlp runs
//! ffmpeg: stopping the first by its pid alone left the other two writing into
//! the folder of a download the row said had stopped. Ended and not asked,
//! because the one that could have asked them to finish has already gone, and
//! a child that ignored a polite signal would hold the pipes the answer is read
//! from open for ever.
//!
//! **A fork is reached by a value.** [`ProcessIdentity`] is the pid and the
//! inode the kernel's pidfs gives that one process and no other for as long as
//! the machine is up, so it can be held in a machine's state, compared in a
//! test and handed across a thread. Interrupting by it opens a pidfd for the
//! pid and signals only when the inode is the one written down: a pid on its
//! own names whichever process has it now, and the one that had it may have
//! been reaped and its number handed on.
//!
//! **The group is ended before the head is reaped.** A group is named by the
//! number of the process at its head, and that number cannot be handed to
//! anybody else while the head is unreaped. So [`Forked::wait`] waits for the
//! head to end without reaping it, ends what is left in its group, and only
//! then reaps -- and nothing else here signals a group at all.
//!
//! **Asking twice is insisting.** An interrupt asks with SIGTERM, which a
//! program can meet by finishing what it was writing. A program that is
//! interrupted again is sent SIGKILL. Neither waits for anything: when to give
//! up on a polite stop is a decision, and a decision is the machine's.
//! [`console_program_lifetime::Interruption`] is how far that has gone for one fork, kept by whoever
//! holds it -- the runtime's loop and the panel's tasks both do.
//!
//! The pidfs inode is a Linux 6.9 answer. On a kernel before it every pidfd
//! shares one inode, the comparison always agrees, and an interrupt is as good
//! as the pid it carries -- which is what every `kill` before pidfds was.

use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus};

use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_program_lifetime::{Insistence, Reached};
use rustix::fs::fstat;
use rustix::io::Errno;
use rustix::process::{
  Pid, PidfdFlags, Signal, WaitId, WaitIdOptions, kill_process_group, pidfd_open,
  pidfd_send_signal, waitid,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessIdentity {
  process: Pid,
  inode: u64,
}

#[derive(Debug)]
pub struct Forked {
  child: Child,
  descriptor: OwnedFd,
  identity: ProcessIdentity,
  reaped: Reaped,
}

pub(crate) struct ForkCancellation(OwnedFd);

impl ForkCancellation {
  pub(crate) fn interrupt(&self, insistence: Insistence) -> Result<Reached, Never> {
    let Ok(interrupt) = signal(insistence);

    Ok(match pidfd_send_signal(&self.0, interrupt) {
      Ok(()) => Reached::Yes,
      Err(_already_gone) => Reached::Gone,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reaped {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Same {
  Process,
  Other,
}

pub fn forked(command: &mut Command) -> io::Result<Forked> {
  let Ok(()) = super::hooks::bind(command);

  command.process_group(0);

  let mut child = command.spawn()?;
  let process = Pid::from_child(&child);
  let named = pidfd_open(process, PidfdFlags::empty()).and_then(|descriptor| {
    let stat = fstat(&descriptor)?;

    Ok((descriptor, stat.st_ino))
  });

  match named {
    Ok((descriptor, inode)) => Ok(Forked {
      child,
      descriptor,
      identity: ProcessIdentity { process, inode },
      reaped: Reaped::No,
    }),
    Err(fault) => {
      let Ok(()) = end_the_group(process);
      let Ok(()) = super::kill_and_reap(&mut child);

      Err(io::Error::from(fault))
    }
  }
}

impl Forked {
  pub fn identity(&self) -> Result<ProcessIdentity, Never> {
    Ok(self.identity)
  }

  pub(crate) fn cancellation(&self) -> io::Result<ForkCancellation> {
    self.descriptor.try_clone().map(ForkCancellation)
  }

  pub fn take_stdout(&mut self) -> Result<Option<ChildStdout>, Never> {
    Ok(self.child.stdout.take())
  }

  pub fn take_stderr(&mut self) -> Result<Option<ChildStderr>, Never> {
    Ok(self.child.stderr.take())
  }

  pub fn wait(&mut self) -> io::Result<ExitStatus> {
    match self.reaped {
      Reaped::Yes => return self.child.wait(),
      Reaped::No => {}
    }

    let ended = iterate(&self.descriptor, |descriptor| {
      Ok(
        match waitid(
          WaitId::PidFd(descriptor.as_fd()),
          WaitIdOptions::EXITED | WaitIdOptions::NOWAIT,
        ) {
          Err(Errno::INTR) => Step::Again(descriptor),
          Ok(_) => Step::Halt(Ok(())),
          Err(fault) => Step::Halt(Err(fault)),
        },
      )
    });

    match ended {
      Ok(Ok(())) => {
        let Ok(()) = end_the_group(self.identity.process);
      }
      Ok(Err(fault)) => console_core_logging::debug!("could not wait on the pidfd: {fault}"),
      Err(fault) => console_core_logging::error!("the pidfd wait ended without a result: {fault}"),
    }

    self.reap()
  }

  pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
    match self.reaped {
      Reaped::Yes => return self.child.try_wait(),
      Reaped::No => {}
    }

    let ended = waitid(
      WaitId::PidFd(self.descriptor.as_fd()),
      WaitIdOptions::EXITED | WaitIdOptions::NOWAIT | WaitIdOptions::NOHANG,
    );

    match ended {
      Ok(Some(_ended)) => {
        let Ok(()) = end_the_group(self.identity.process);

        self.reap().map(Some)
      }
      Ok(None) | Err(Errno::INTR) => Ok(None),
      Err(fault) => Err(io::Error::from(fault)),
    }
  }

  fn reap(&mut self) -> io::Result<ExitStatus> {
    let status = self.child.wait()?;

    self.reaped = Reaped::Yes;

    Ok(status)
  }
}

impl Drop for Forked {
  fn drop(&mut self) {
    match self.reaped {
      Reaped::Yes => {}
      Reaped::No => {
        let Ok(()) = end_the_group(self.identity.process);
        let Ok(()) = super::kill_and_reap(&mut self.child);
      }
    }
  }
}

impl ProcessIdentity {
  pub fn interrupt(self, insistence: Insistence) -> Result<Reached, Never> {
    let descriptor = match pidfd_open(self.process, PidfdFlags::empty()) {
      Ok(descriptor) => descriptor,
      Err(_already_gone) => return Ok(Reached::Gone),
    };
    let same = match fstat(&descriptor) {
      Ok(stat) => match ::core::primitive::bool::then_some(stat.st_ino == self.inode, ()) {
        ::core::option::Option::Some(()) => Same::Process,
        ::core::option::Option::None => Same::Other,
      },
      Err(_unasked) => Same::Other,
    };

    let Ok(interrupt) = signal(insistence);

    Ok(match same {
      Same::Other => Reached::Gone,
      Same::Process => match pidfd_send_signal(&descriptor, interrupt) {
        Ok(()) => Reached::Yes,
        Err(_already_gone) => Reached::Gone,
      },
    })
  }
}

fn signal(insistence: Insistence) -> Result<Signal, Never> {
  Ok(match insistence {
    Insistence::Ask => Signal::TERM,
    Insistence::Insist => Signal::KILL,
  })
}

fn end_the_group(head: Pid) -> Result<(), Never> {
  match kill_process_group(head, Signal::KILL) {
    Ok(()) => {}
    Err(Errno::SRCH) => return Ok(()),
    Err(fault) => console_core_logging::warn!("could not stop process group {head:?}: {fault}"),
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::error::Error;
  use std::io::{BufRead, BufReader};
  use std::os::unix::process::ExitStatusExt;
  use std::process::Stdio;
  use std::time::Duration;

  use console_core_external_programs::Program;
  use console_program_lifetime::Still;
  use console_waiting::{Outcome, Ready, Schedule, until_handed};

  use super::*;
  use crate::program_lifetime::process_presence;

  type Failure = Box<dyn Error>;

  #[cfg_attr(
    dylint_lib = "explicit082_no_built_shell_line",
    expect(
      explicit082_no_built_shell_line,
      reason = "the test fixture passes a complete shell program written by the test"
    )
  )]
  fn saying(script: &str) -> Result<Command, Never> {
    let Ok(mut command) = Program::Sh.command();

    command
      .args(["-c", script])
      .stdin(Stdio::null())
      .stdout(Stdio::piped())
      .stderr(Stdio::null());

    Ok(command)
  }

  fn first_line(fork: &mut Forked) -> Result<String, Failure> {
    let Ok(printing) = fork.take_stdout();
    let said = printing.ok_or("the fork has nothing to read")?;
    let mut line = String::new();

    BufReader::new(said).read_line(&mut line)?;

    Ok(line.trim().to_string())
  }

  fn child_pid(text: &str) -> Result<Pid, Failure> {
    let raw = text.parse::<i32>()?;
    let process = Pid::from_raw(raw).ok_or("the fork did not report a child ID")?;

    Ok(process)
  }

  fn gone_within(process: Pid, seconds: u64) -> Result<Outcome, Never> {
    let Ok(patience) = Schedule::of(Duration::from_secs(seconds));

    until_handed(patience, &mut (), |()| {
      let Ok(state) = process_presence(process);

      Ok(match state {
        Still::Running => Ready::NotYet,
        Still::Ended => Ready::Yes,
      })
    })
  }

  #[test]
  fn an_interrupted_fork_ends_of_the_signal_it_was_asked_with() -> Result<(), Failure> {
    let Ok(mut command) = saying("echo up; exec sleep 600");
    let mut fork = forked(&mut command)?;
    let up = first_line(&mut fork)?;
    let Ok(identity) = fork.identity();

    assert_eq!(up, "up");
    assert_eq!(identity.interrupt(Insistence::Ask), Ok(Reached::Yes));

    let status = fork.wait()?;

    assert_eq!(status.signal(), Some(Signal::TERM.as_raw()), "{status:?}");

    Ok(())
  }

  #[test]
  fn what_a_fork_started_is_ended_when_it_ends() -> Result<(), Failure> {
    let Ok(mut command) = saying("sleep 600 & echo $!");
    let mut fork = forked(&mut command)?;
    let left = first_line(&mut fork)?;
    let started = child_pid(&left)?;

    assert_eq!(process_presence(started), Ok(Still::Running), "what it started never started: {left}");

    let status = fork.wait()?;

    assert!(status.success(), "{status:?}");
    assert_eq!(
      gone_within(started, 10),
      Ok(Outcome::Happened),
      "what the fork started outlived it"
    );

    Ok(())
  }

  #[test]
  fn dropping_a_fork_ends_it_and_what_it_started() -> Result<(), Failure> {
    let Ok(mut command) = saying("sleep 600 & echo $!; exec sleep 600");
    let mut fork = forked(&mut command)?;
    let left = first_line(&mut fork)?;
    let Ok(identity) = fork.identity();
    let started = child_pid(&left)?;

    drop(fork);

    assert_eq!(
      gone_within(identity.process, 10),
      Ok(Outcome::Happened),
      "the fork outlived what held it"
    );
    assert_eq!(
      gone_within(started, 10),
      Ok(Outcome::Happened),
      "what the fork started outlived what held it"
    );

    Ok(())
  }

  #[test]
  fn a_fork_that_will_not_be_asked_is_ended_when_it_is_asked_again() -> Result<(), Failure> {
    let Ok(mut command) = saying("trap '' TERM; echo up; exec sleep 600");
    let mut fork = forked(&mut command)?;
    let _up = first_line(&mut fork)?;
    let Ok(identity) = fork.identity();
    assert_eq!(identity.interrupt(Insistence::Ask), Ok(Reached::Yes));
    assert_eq!(
      gone_within(identity.process, 1),
      Ok(Outcome::RanOut),
      "a program that ignores being asked was ended by the asking"
    );
    assert_eq!(identity.interrupt(Insistence::Insist), Ok(Reached::Yes));

    let status = fork.wait()?;

    assert_eq!(status.signal(), Some(Signal::KILL.as_raw()), "{status:?}");

    Ok(())
  }

  #[test]
  fn an_identity_reaches_only_the_process_it_was_written_down_for() -> Result<(), Failure> {
    let Ok(mut command) = saying("echo up; exec sleep 600");
    let mut fork = forked(&mut command)?;
    let _up = first_line(&mut fork)?;
    let Ok(identity) = fork.identity();
    let someone_else = ProcessIdentity {
      inode: identity.inode.wrapping_add(1),
      ..identity
    };

    assert_eq!(
      someone_else.interrupt(Insistence::Insist),
      Ok(Reached::Gone)
    );
    assert_eq!(identity.interrupt(Insistence::Insist), Ok(Reached::Yes));

    let _status = fork.wait()?;

    assert_eq!(
      identity.interrupt(Insistence::Insist),
      Ok(Reached::Gone),
      "a reaped fork was still reached"
    );

    Ok(())
  }
}
