//! Wait for Wayland, a caller's file descriptors, or its next deadline.

use std::cmp::Ordering;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::time::Duration;

use console_core_never::Never;
use console_core_number_conversion::fitted;
use rustix::event::{Nsecs, PollFd, PollFlags, Secs, Timespec, poll};

use crate::client::backend::WaylandError;
use crate::client::{Connection, DispatchError, EventQueue};

#[derive(Debug)]
pub enum WaitError {
  Wayland(DispatchError),
  Poll(rustix::io::Errno),
  Closed,
}

impl From<WaitError> for io::Error {
  fn from(fault: WaitError) -> Self {
    match fault {
      WaitError::Wayland(DispatchError::Backend(WaylandError::Io(io))) => io,
      WaitError::Wayland(other) => Self::other(other),
      WaitError::Poll(errno) => Self::from(errno),
      WaitError::Closed => Self::new(
        io::ErrorKind::BrokenPipe,
        "the compositor closed the connection",
      ),
    }
  }
}

#[derive(Clone, Copy)]
pub enum Flushed {
  Sent,
  Pending,
}

enum SocketFault {
  Again,
  Closed(WaylandError),
}

impl From<WaylandError> for SocketFault {
  fn from(fault: WaylandError) -> Self {
    let nonblocking =
      matches!(&fault, WaylandError::Io(io) if io.kind() == io::ErrorKind::WouldBlock);

    match bool::then_some(nonblocking, ()) {
      Some(()) => Self::Again,
      None => Self::Closed(fault),
    }
  }
}

pub fn flush(connection: &Connection) -> Result<Flushed, DispatchError> {
  match connection.flush().map_err(SocketFault::from) {
    Ok(()) => Ok(Flushed::Sent),
    Err(SocketFault::Again) => Ok(Flushed::Pending),
    Err(SocketFault::Closed(fault)) => Err(DispatchError::Backend(fault)),
  }
}

pub fn wait<State>(
  connection: &Connection,
  queue: &mut EventQueue<State>,
  state: &mut State,
  also: &[BorrowedFd<'_>],
  until: Option<Duration>,
) -> Result<(), WaitError> {
  let dispatched = queue
    .dispatch_pending(state)
    .map(|count| count.cmp(&0))
    .map_err(WaitError::Wayland)?;
  let flushed = flush(connection).map_err(WaitError::Wayland)?;

  match dispatched {
    Ordering::Equal => {}
    Ordering::Greater | Ordering::Less => return Ok(()),
  }

  let read = match connection.prepare_read() {
    Some(guard) => guard,
    None => {
      queue.dispatch_pending(state).map_err(WaitError::Wayland)?;
      return Ok(());
    }
  };
  let interests = match flushed {
    Flushed::Sent => PollFlags::IN,
    Flushed::Pending => PollFlags::IN | PollFlags::OUT,
  };
  let display = PollFd::from_borrowed_fd(connection.as_fd(), interests);
  let deadline = until.map(|duration| {
    let Ok(timeout) = timeout(duration);
    timeout
  });
  let ready = poll_display(display, also, deadline.as_ref())?;

  match (
    bool::then_some(
      ready.intersects(PollFlags::HUP | PollFlags::ERR | PollFlags::NVAL),
      (),
    ),
    bool::then_some(ready.contains(PollFlags::IN), ()),
  ) {
    (Some(()), _) => return Err(WaitError::Closed),
    (None, Some(())) => match read.read().map_err(SocketFault::from) {
      Ok(_) => {}
      Err(SocketFault::Again) => return Ok(()),
      Err(SocketFault::Closed(fault)) => {
        return Err(WaitError::Wayland(DispatchError::Backend(fault)));
      }
    },
    (None, None) => drop(read),
  }

  match bool::then_some(ready.contains(PollFlags::OUT), ()) {
    Some(()) => {
      let _flush = flush(connection).map_err(WaitError::Wayland)?;
    }
    None => {}
  }

  queue.dispatch_pending(state).map_err(WaitError::Wayland)?;
  Ok(())
}

fn timeout(long: Duration) -> Result<Timespec, Never> {
  let Ok(seconds) = fitted::<u64, Secs>(long.as_secs());
  let Ok(nanoseconds) = fitted::<u32, Nsecs>(long.subsec_nanos());
  Ok(Timespec {
    tv_sec: seconds,
    tv_nsec: nanoseconds,
  })
}

fn poll_display<'a>(
  display: PollFd<'a>,
  also: &[BorrowedFd<'a>],
  deadline: Option<&Timespec>,
) -> Result<PollFlags, WaitError> {
  let watch = |fd| PollFd::from_borrowed_fd(fd, PollFlags::IN);

  match also {
    [] => ready(&mut [display], deadline),
    [fd] => ready(&mut [display, watch(*fd)], deadline),
    extra => {
      let mut watched = vec![display];
      watched.extend(extra.iter().copied().map(watch));
      ready(&mut watched, deadline)
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  allow(
    explicit057_no_unbounded_wait,
    reason = "an event-driven caller with no deadline sleeps until a descriptor wakes it"
  )
)]
fn ready(
  watched: &mut [PollFd<'_>],
  deadline: Option<&Timespec>,
) -> Result<PollFlags, WaitError> {
  match poll(watched, deadline) {
    Ok(_) => Ok(
      watched
        .first()
        .map_or_else(PollFlags::empty, PollFd::revents),
    ),
    Err(rustix::io::Errno::INTR) => Ok(PollFlags::empty()),
    Err(fault) => Err(WaitError::Poll(fault)),
  }
}
