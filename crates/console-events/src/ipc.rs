//! Versioned request/reply and snapshot messages on the event relay socket.
//! The existing subscribe, unsubscribe, and publish lines remain version zero.

use std::fmt;
use std::fs::File;
use std::io::{self, BufReader, Write};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use console_compositor::workspace_snapshot::Snapshot;
use console_core_bounded_reads::{Line, LineEnding, LinePart, ReadError};
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::fixed_index;
use console_core_reconnect::{Round, keep};
use console_operating_system_linux::unix_stream::receive_line;
use console_waiting::latch;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use serde::{Deserialize, Serialize};

pub const VERSION: u16 = 1;
pub use crate::wire::REPLY_BYTES;
pub(crate) use console_operating_system_linux::unix_stream::connect as connected;
pub(crate) const STALE_PAYLOAD: &str = "\"stale\"";
const FRAME_TIME: Duration = Duration::from_secs(1);

const PING: u16 = 1;
const GET_WORKSPACE: u16 = 2;
const WATCH_WORKSPACE: u16 = 3;
const PONG: u16 = 101;
const WORKSPACE: u16 = 102;
const FAULT: u16 = 103;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
  Ping,
  GetWorkspace { max_age_ms: u32 },
  WatchWorkspace,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Workspace {
  Stale,
  Fresh {
    generation: u64,
    revision: u64,
    snapshot: Snapshot,
  },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fault {
  UnsupportedVersion,
  UnknownMethod,
  InvalidRequest,
  Unavailable,
  Overloaded,
  TimedOut,
}

#[derive(Debug, PartialEq)]
pub enum Reply {
  Pong,
  Workspace(Workspace),
  Fault(Fault),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trace {
  Quiet,
  Enabled,
}

pub(crate) enum Direction {
  Send,
  Receive,
  Publish,
}

struct Header<'line> {
  version: u16,
  code: u16,
  serial: u64,
  payload: &'line str,
}

#[derive(Debug)]
pub enum Error {
  Io(io::Error),
  Session,
  Closed,
  FrameTooLarge,
  InvalidReply,
  TimedOut,
  SerialExhausted,
  Remote(Fault),
}

impl fmt::Display for Error {
  fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Io(fault) => fault.fmt(output),
      Self::Session => output.write_str("the Console session has no event socket"),
      Self::Closed => output.write_str("the event relay closed the connection"),
      Self::FrameTooLarge => output.write_str("the event relay frame exceeded its byte limit"),
      Self::InvalidReply => output.write_str("the event relay sent an unexpected reply"),
      Self::TimedOut => output.write_str("the event relay did not reply before the deadline"),
      Self::SerialExhausted => output.write_str("the connection used every request serial"),
      Self::Remote(fault) => write!(output, "the event relay refused the request: {fault:?}"),
    }
  }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
  fn from(fault: io::Error) -> Self {
    let kind = fault.kind();

    match (kind == io::ErrorKind::WouldBlock || kind == io::ErrorKind::TimedOut).then_some(()) {
      Some(()) => Self::TimedOut,
      None => Self::Io(fault),
    }
  }
}

impl Header<'_> {
  fn read(line: &str) -> Result<Header<'_>, Fault> {
    let mut words = line.splitn(5, ' ');

    match words.next() {
      Some("ipc") => {}
      Some(_) | None => return Err(Fault::InvalidRequest),
    }

    let version = number(words.next())?;
    let code = number(words.next())?;
    let serial = number(words.next())?;
    let payload = words.next().ok_or(Fault::InvalidRequest)?;

    Ok(Header {
      version,
      code,
      serial,
      payload,
    })
  }

  fn describe(&self, trace: Trace, direction: Direction, bytes: u64) -> Result<(), Never> {
    match trace {
      Trace::Enabled => {
        let named = match self.code {
          PING => "ping",
          GET_WORKSPACE => "get-workspace",
          WATCH_WORKSPACE => "watch-workspace",
          PONG => "pong",
          WORKSPACE => "workspace",
          FAULT => "fault",
          _ => "unknown",
        };
        let action = match direction {
          Direction::Send => "send",
          Direction::Receive => "receive",
          Direction::Publish => "publish",
        };
        console_core_logging::info!(
          "ipc {action} v{} {} {named} serial={} bytes={bytes}",
          self.version,
          self.code,
          self.serial
        );
      }
      Trace::Quiet => {}
    }

    Ok(())
  }
}

fn number<T: std::str::FromStr>(word: Option<&str>) -> Result<T, Fault> {
  let text = word.ok_or(Fault::InvalidRequest)?;

  text
    .parse()
    .map_err(|_invalid_integer| Fault::InvalidRequest)
}

impl Request {
  pub fn line(self, serial: u64) -> Result<String, Never> {
    let (code, payload) = match self {
      Self::Ping => (PING, "-".to_owned()),
      Self::GetWorkspace { max_age_ms } => (GET_WORKSPACE, max_age_ms.to_string()),
      Self::WatchWorkspace => (WATCH_WORKSPACE, "-".to_owned()),
    };

    Ok(format!("ipc {VERSION} {code} {serial} {payload}\n"))
  }

  pub(crate) fn read(line: &str) -> Result<(u64, Result<Self, Fault>), Fault> {
    let header = Header::read(line)?;
    let request = match (header.version, header.serial) {
      (VERSION, 0) => Err(Fault::InvalidRequest),
      (VERSION, _) => match (header.code, header.payload) {
        (PING, "-") => Ok(Self::Ping),
        (GET_WORKSPACE, ms) => ms
          .parse()
          .map(|max_age_ms| Self::GetWorkspace { max_age_ms })
          .map_err(|_| Fault::InvalidRequest),
        (WATCH_WORKSPACE, "-") => Ok(Self::WatchWorkspace),
        (PING | WATCH_WORKSPACE, _) => Err(Fault::InvalidRequest),
        _ => Err(Fault::UnknownMethod),
      },
      (_, _) => Err(Fault::UnsupportedVersion),
    };

    Ok((header.serial, request))
  }
}

impl Reply {
  pub fn line(&self, serial: u64) -> Result<String, serde_json::Error> {
    match self {
      Self::Pong => Ok(format!("ipc {VERSION} {PONG} {serial} -\n")),
      Self::Workspace(workspace) => workspace_line(workspace, serial),
      Self::Fault(fault) => {
        let payload = serde_json::to_string(fault)?;

        Ok(format!("ipc {VERSION} {FAULT} {serial} {payload}\n"))
      }
    }
  }

  pub fn read(line: &str) -> Result<(u64, Self), Error> {
    let header = Header::read(line).map_err(|_| Error::InvalidReply)?;
    let reply = match (header.version, header.code, header.payload) {
      (VERSION, PONG, "-") => Self::Pong,
      (VERSION, WORKSPACE, json) => {
        let map_err = serde_json::from_str(json).map_err(|_| Error::InvalidReply)?;

        Self::Workspace(map_err)
      }
      (VERSION, FAULT, json) => {
        let map_err = serde_json::from_str(json).map_err(|_| Error::InvalidReply)?;

        Self::Fault(map_err)
      }
      _ => return Err(Error::InvalidReply),
    };

    Ok((header.serial, reply))
  }
}

pub(crate) fn workspace_line(
  workspace: &Workspace,
  serial: u64,
) -> Result<String, serde_json::Error> {
  let payload = serde_json::to_string(workspace)?;

  let Ok(line) = workspace_frame(&payload, serial);

  Ok(line)
}

pub(crate) fn workspace_frame(payload: &str, serial: u64) -> Result<String, Never> {
  Ok(format!("ipc {VERSION} {WORKSPACE} {serial} {payload}\n"))
}

pub(crate) fn trace_line(line: &str, trace: Trace, direction: Direction) -> Result<(), Never> {
  match trace {
    Trace::Quiet => return Ok(()),
    Trace::Enabled => {}
  }

  let Ok(bytes) = fixed_index(line.len());

  match Header::read(line) {
    Ok(header) => header.describe(trace, direction, bytes),
    Err(_invalid_header) => Ok(()),
  }
}

pub struct Client {
  stream: BufReader<UnixStream>,
  frame: Line,
  serial: u64,
  trace: Trace,
}

pub struct Watch {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the watch worker replaces one pending snapshot and the GUI independently takes it"
    )
  )]
  current: Arc<Mutex<Option<Workspace>>>,
  _active: File,
}

impl Watch {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the watch worker and GUI hold the two ends of a single replaceable update slot"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit047_no_captured_permission",
    expect(
      explicit047_no_captured_permission,
      reason = "the reconnect runner owns the wake descriptor and publishes every completed watch update through it"
    )
  )]
  pub fn session(mut waking: File, trace: Trace) -> Result<Self, Error> {
    let at = crate::place::socket().map_err(|_| Error::Session)?;
    let stop = latch::pipe()?;
    let current = Arc::new(Mutex::new(None));
    let watching = Arc::clone(&current);
    let Ok(()) = keep(move || {
      let Ok(round) = watch_attempt(&at, &stop.waiting, trace, &watching, &mut waking);

      round
    });

    Ok(Self {
      current,
      _active: stop.saying,
    })
  }

  pub fn take(&self) -> Result<Option<Workspace>, Never> {
    let mut current = match self.current.try_lock() {
      Ok(current) => current,
      Err(std::sync::TryLockError::WouldBlock) => return Ok(None),
      Err(std::sync::TryLockError::Poisoned(fault)) => fault.into_inner(),
    };

    Ok(current.take())
  }
}

fn watch_attempt(
  at: &Path,
  stopped: &impl AsFd,
  trace: Trace,
  slot: &Mutex<Option<Workspace>>,
  wake: &mut File,
) -> Result<Round, Never> {
  let mut watched = [PollFd::new(stopped, PollFlags::IN)];
  let now = Timespec {
    tv_sec: 0,
    tv_nsec: 0,
  };

  match poll(&mut watched, Some(&now)) {
    Ok(0) => {}
    Ok(_stopped) => return Ok(Round::Finished),
    Err(_stop_descriptor_failed) => return Ok(Round::Finished),
  }

  Ok(match watch_round(at, stopped, trace, slot, wake) {
    Ok(()) => Round::Finished,
    Err(fault) => {
      console_core_logging::debug!("workspace feed: {fault}");

      match update(slot, wake, Workspace::Stale) {
        Ok(()) => Round::Another,
        Err(_gui_stopped) => Round::Finished,
      }
    }
  })
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the only other mutex user takes one Option without running I/O under the guard"
  )
)]
fn update(
  current: &Mutex<Option<Workspace>>,
  waking: &mut File,
  workspace: Workspace,
) -> Result<(), Error> {
  let mut latest = match current.lock() {
    Ok(latest) => latest,
    Err(fault) => fault.into_inner(),
  };
  *latest = Some(workspace);
  drop(latest);

  let written = match waking.write(&[1]) {
    Ok(_one_byte) => return Ok(()),
    Err(fault) => fault,
  };

  match (written.kind() == io::ErrorKind::WouldBlock).then_some(()) {
    Some(()) => Ok(()),
    None => Err(Error::from(written)),
  }
}

fn watch_round(
  at: &Path,
  stopped: &impl AsFd,
  trace: Trace,
  slot: &Mutex<Option<Workspace>>,
  wake: &mut File,
) -> Result<(), Error> {
  let mut client = Client::at(at, trace)?;
  let initial = client.call(Request::WatchWorkspace, Duration::from_millis(150))?;

  match initial {
    Reply::Workspace(workspace) => update(slot, wake, workspace)?,
    Reply::Pong | Reply::Fault(_) => return Err(Error::InvalidReply),
  }

  let streamed = iterate((client, wake), |(mut client_2, wake_2)| {
    let next = match client_2.workspace_until_stopped(stopped) {
      Ok(next) => next,
      Err(fault) => return Ok(Step::Halt(Err(fault))),
    };

    Ok(match next {
      Some(workspace) => match update(slot, wake_2, workspace) {
        Ok(()) => Step::Again((client_2, wake_2)),
        Err(fault) => Step::Halt(Err(fault)),
      },
      None => Step::Halt(Ok(())),
    })
  });
  let stopped_2 = streamed.map_err(|Endless| Error::Closed)?;

  stopped_2
}

impl Client {
  pub fn session(trace: Trace) -> Result<Self, Error> {
    let at = crate::place::socket().map_err(|_| Error::Session)?;

    Self::at(&at, trace)
  }

  pub fn at(at: &Path, trace: Trace) -> Result<Self, Error> {
    let stream = connected(at)?;
    let Ok(frame) = Line::new(REPLY_BYTES, LineEnding::Required);

    Ok(Self {
      stream: BufReader::new(stream),
      frame,
      serial: 0,
      trace,
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    expect(
      explicit039_no_reading_the_clock,
      reason = "the socket driver applies one absolute deadline to the send and the reply"
    )
  )]
  pub fn call(&mut self, request: Request, timeout: Duration) -> Result<Reply, Error> {
    let deadline = Instant::now().checked_add(timeout).ok_or(Error::TimedOut)?;
    let serial = self.serial.checked_add(1).ok_or(Error::SerialExhausted)?;
    let Ok(line) = request.line(serial);
    let Ok(()) = trace_line(line.trim_end(), self.trace, Direction::Send);
    let outcome = self.exchange(&line, serial, deadline);

    match outcome {
      Err(Error::Remote(fault)) => Err(Error::Remote(fault)),
      Err(fault) => {
        let _closed = self.stream.get_ref().shutdown(std::net::Shutdown::Both);

        Err(fault)
      }
      Ok(reply) => Ok(reply),
    }
  }

  fn exchange(&mut self, line: &str, serial: u64, deadline: Instant) -> Result<Reply, Error> {
    let remaining = time_left(deadline)?;
    let stream = self.stream.get_mut();
    stream.set_write_timeout(Some(remaining))?;
    stream.write_all(line.as_bytes())?;
    self.serial = serial;
    let (answered, reply) = self.receive(Some(deadline))?;

    match (answered == serial, reply) {
      (true, Reply::Fault(fault)) => Err(Error::Remote(fault)),
      (true, reply_2) => Ok(reply_2),
      (false, _) => Err(Error::InvalidReply),
    }
  }

  pub fn next_workspace(&mut self, deadline: Option<Instant>) -> Result<Workspace, Error> {
    match {
      let checked_value = self.receive(deadline)?;
      checked_value
    } {
      (0, Reply::Workspace(workspace)) => Ok(workspace),
      (_, Reply::Fault(fault)) => Err(Error::Remote(fault)),
      _ => Err(Error::InvalidReply),
    }
  }

  pub fn workspace_until_stopped(
    &mut self,
    stopped: &impl AsFd,
  ) -> Result<Option<Workspace>, Error> {
    match self.stream.buffer().is_empty().then_some(()) {
      Some(()) => {
        let mut sockets = [
          PollFd::new(stopped, PollFlags::IN),
          PollFd::new(self.stream.get_ref(), PollFlags::IN),
        ];
        poll(&mut sockets, None).map_err(io::Error::from)?;
        let [stop, _stream] = sockets;

        match stop.revents().is_empty().then_some(()) {
          Some(()) => {}
          None => return Ok(None),
        }
      }
      None => {}
    }

    self.next_workspace(None).map(Some)
  }

  fn receive(&mut self, deadline: Option<Instant>) -> Result<(u64, Reply), Error> {
    match receive_line(&mut self.stream, &mut self.frame, deadline, FRAME_TIME) {
      Ok(LinePart::Complete) => {}
      Ok(LinePart::End | LinePart::Partial) => return Err(Error::Closed),
      Err(ReadError::TooLong { .. }) => return Err(Error::FrameTooLarge),
      Err(ReadError::Io(fault)) => return Err(Error::from(fault)),
      Err(ReadError::NotUtf8(_)) => return Err(Error::InvalidReply),
    }

    let Ok(bytes) = self.frame.bytes();
    let line = std::str::from_utf8(bytes).map_err(|_utf8| Error::InvalidReply)?;
    let Ok(()) = trace_line(line, self.trace, Direction::Receive);

    Reply::read(line)
  }
}

#[cfg_attr(
  dylint_lib = "explicit039_no_reading_the_clock",
  expect(
    explicit039_no_reading_the_clock,
    reason = "this provides the remaining kernel timeout on a socket operation"
  )
)]
fn time_left(deadline: Instant) -> Result<Duration, Error> {
  let remaining = deadline.saturating_duration_since(Instant::now());

  match remaining.is_zero().then_some(()) {
    Some(()) => Err(Error::TimedOut),
    None => Ok(remaining),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn version_one_request_and_reply_ids_are_stable() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(Request::Ping.line(9), Ok("ipc 1 1 9 -\n".to_string()));
    assert_eq!(
      Request::read("ipc 1 2 43 250"),
      Ok((43, Ok(Request::GetWorkspace { max_age_ms: 250 })))
    );
    assert_eq!(
      Request::read("ipc 2 1 9 -"),
      Ok((9, Err(Fault::UnsupportedVersion)))
    );
    assert_eq!(
      Request::read("ipc 1 99 9 -"),
      Ok((9, Err(Fault::UnknownMethod)))
    );
    assert_eq!(
      Request::read("ipc 1 1 0 -"),
      Ok((0, Err(Fault::InvalidRequest)))
    );
    let line = Reply::Pong.line(9)?;
    assert_eq!(line, "ipc 1 101 9 -\n");
    let stale = Reply::Workspace(Workspace::Stale).line(0)?;
    assert_eq!(stale, "ipc 1 102 0 \"stale\"\n");
    assert!(matches!(
      Reply::read("ipc 1 103 9 \"overloaded\""),
      Ok((9, Reply::Fault(Fault::Overloaded)))
    ));

    Ok(())
  }
}
