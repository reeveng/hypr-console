//! Unix process signals.

use std::io;
use std::os::fd::BorrowedFd;
use std::time::Duration;

use console_core_never::Never;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::io::Errno;
use rustix::process::{Signal, pidfd_send_signal};

pub fn signal(pid: i32, signal: rustix::process::Signal) -> Result<(), Never> {
  let sent =
    rustix::process::Pid::from_raw(pid).map(|pid_2| rustix::process::kill_process(pid_2, signal));

  match sent {
    Some(Ok(())) => {}
    Some(Err(_already_gone)) => {}
    None => {}
  }

  Ok(())
}

pub fn interrupt_and_wait(
  process: BorrowedFd<'_>,
  signal: Signal,
  wait: Duration,
) -> io::Result<()> {
  let timeout = Timespec::try_from(wait).map_err(io::Error::other)?;

  match pidfd_send_signal(process, signal) {
    Ok(()) => {}
    Err(Errno::SRCH) => return Ok(()),
    Err(error) => return Err(io::Error::from(error)),
  }

  let mut watched = [PollFd::from_borrowed_fd(process, PollFlags::IN)];
  let _ready = poll(&mut watched, Some(&timeout))?;
  let exited = watched
    .first()
    .is_some_and(|ready| ready.revents().intersects(PollFlags::IN | PollFlags::HUP));

  match exited.then_some(()) {
    Some(()) => Ok(()),
    None => Err(io::Error::new(
      io::ErrorKind::TimedOut,
      "process did not exit after interruption",
    )),
  }
}
