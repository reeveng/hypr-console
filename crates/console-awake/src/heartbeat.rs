//! Keep an SSH inhibitor observable until the connection's input closes.

use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::time::{Duration, Instant};

use console_core_never::Never;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::io::{Errno, read, write};

pub const EVERY: Duration = Duration::from_secs(10);
pub const COMMAND: &str = "device-inhibit-heartbeat";

pub fn run(
  input: impl AsFd,
  output: impl AsFd,
  every: Duration,
  now: impl Fn() -> Instant,
) -> io::Result<()> {
  let from = input.as_fd();
  let to = output.as_fd();
  let input_flags = fcntl_getfl(from)?;
  fcntl_setfl(from, input_flags | OFlags::NONBLOCK)?;
  let result = nonblocking_output(
    to,
    Heartbeats {
      from,
      every,
      last: now(),
      now,
    },
  );
  let restored = fcntl_setfl(from, input_flags);
  result?;
  restored.map_err(io::Error::from)
}

fn nonblocking_output(
  to: BorrowedFd<'_>,
  mut heartbeats: Heartbeats<'_, impl Fn() -> Instant>,
) -> io::Result<()> {
  let flags = fcntl_getfl(to)?;
  fcntl_setfl(to, flags | OFlags::NONBLOCK)?;
  let sent = heartbeats.try_for_each(|event| {
    let pulse = event?;

    match pulse {
      Pulse::Due => send_pulse(to),
      Pulse::Waiting => Ok(()),
    }
  });
  let restored = fcntl_setfl(to, flags);
  sent?;
  restored.map_err(io::Error::from)
}

fn send_pulse(to: BorrowedFd<'_>) -> io::Result<()> {
  for _ in 0..3 {
    match write(to, b"\n") {
      Ok(1) => return Ok(()),
      Ok(_) => return Err(io::Error::from(io::ErrorKind::WriteZero)),
      Err(Errno::INTR) => continue,
      Err(fault) => return Err(io::Error::from(fault)),
    }
  }

  Err(io::Error::from(io::ErrorKind::Interrupted))
}

enum Pulse {
  Due,
  Waiting,
}

struct Heartbeats<'fd, Clock> {
  from: BorrowedFd<'fd>,
  every: Duration,
  last: Instant,
  now: Clock,
}

impl<Clock: Fn() -> Instant> Heartbeats<'_, Clock> {
  fn wait(&mut self) -> io::Result<Option<Pulse>> {
    let elapsed = (self.now)().saturating_duration_since(self.last);
    let timeout =
      Timespec::try_from(self.every.saturating_sub(elapsed)).map_err(io::Error::other)?;
    let mut readiness = [PollFd::new(&self.from, PollFlags::IN)];
    let mut received = [0; 4096];

    match poll(&mut readiness, Some(&timeout)) {
      Ok(0) => {}
      Ok(_) => match read(self.from, received.as_mut_slice()) {
        Ok(0) => return Ok(None),
        Ok(_) => {}
        Err(Errno::AGAIN) => {
          let Ok(pulse) = self.pulse();
          return Ok(Some(pulse));
        }
        Err(Errno::INTR) => return Ok(Some(Pulse::Waiting)),
        Err(fault) => return Err(io::Error::from(fault)),
      },
      Err(Errno::INTR) => return Ok(Some(Pulse::Waiting)),
      Err(fault) => return Err(io::Error::from(fault)),
    }

    let Ok(pulse) = self.pulse();
    Ok(Some(pulse))
  }

  fn pulse(&mut self) -> Result<Pulse, Never> {
    let current = (self.now)();
    let due = current.saturating_duration_since(self.last) >= self.every;

    Ok(match due.then_some(()) {
      Some(()) => {
        self.last = current;
        Pulse::Due
      }
      None => Pulse::Waiting,
    })
  }
}

impl<Clock: Fn() -> Instant> Iterator for Heartbeats<'_, Clock> {
  type Item = io::Result<Pulse>;

  fn next(&mut self) -> Option<Self::Item> {
    self.wait().transpose()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::os::unix::net::UnixStream;
  use std::sync::mpsc::sync_channel;

  fn received_pulses(receiving: &UnixStream) -> io::Result<[u8; 2]> {
    let timeout = Timespec::try_from(Duration::from_secs(3)).map_err(io::Error::other)?;
    let mut pulses = [0; 2];

    for pulse in &mut pulses {
      let mut expected = [PollFd::new(receiving, PollFlags::IN)];

      match poll(&mut expected, Some(&timeout)) {
        Ok(0) => return Err(io::Error::from(io::ErrorKind::TimedOut)),
        Ok(_) => {}
        Err(fault) => return Err(io::Error::from(fault)),
      }

      match read(receiving, std::slice::from_mut(pulse)) {
        Ok(1) => {}
        Ok(_) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
        Err(fault) => return Err(io::Error::from(fault)),
      }
    }

    Ok(pulses)
  }

  #[test]
  fn periodic_output_stops_when_the_client_closes_input() -> io::Result<()> {
    let (input, client) = UnixStream::pair()?;
    let (output, receiving) = UnixStream::pair()?;
    receiving.set_nonblocking(true)?;
    std::thread::scope(|scope| {
      let (finished, outcome) = sync_channel(1);
      let _worker = scope.spawn(move || {
        let result = run(input, output, Duration::from_millis(20), Instant::now);
        let _fixture_finished = finished.send(result);
      });
      let pulses = received_pulses(&receiving);
      drop(client);
      let completed = outcome
        .recv_timeout(Duration::from_secs(3))
        .map_err(io::Error::other)?;
      completed?;
      let two = pulses?;
      assert_eq!(two, *b"\n\n");
      Ok(())
    })
  }

  #[test]
  fn loss_of_the_output_channel_releases_a_held_connection() -> io::Result<()> {
    let (input, _client) = UnixStream::pair()?;
    let (output, receiving) = UnixStream::pair()?;
    drop(receiving);

    let fault = run(input, output, Duration::from_millis(20), Instant::now).unwrap_err();
    assert_eq!(fault.kind(), io::ErrorKind::BrokenPipe);

    Ok(())
  }
}
