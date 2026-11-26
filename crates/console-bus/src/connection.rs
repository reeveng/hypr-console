//! The socket the messages go down, and the four sentences that open it.
//!
//! A session bus is a unix socket named by `DBUS_SESSION_BUS_ADDRESS`, and
//! everything that happens on it happens after a short conversation in
//! plain text: a nul byte, `AUTH EXTERNAL` with this process's user id in
//! hexadecimal, and `BEGIN`. The nul byte is not a formality -- it is how the
//! other end knows to look at the credentials the kernel attached to the
//! connection -- and after `BEGIN` not one byte of text is spoken again.
//!
//! The system bus is the same conversation at another address. The
//! specification gives it one when `DBUS_SYSTEM_BUS_ADDRESS` is unset, which
//! the session bus does not have, so an unset variable is a session bus that
//! is not there and a system bus that is where it always is. bluez lives on
//! the system bus, and the agent a pairing needs is a name on it rather than
//! on anybody's session.
//!
//! Two calls follow that are not this desktop's. `Hello` is how a connection
//! learns the name the bus has given it, and it is mandatory: a bus refuses
//! everything else until it has been asked. `RequestName` is the one that
//! matters here, and it is asked with the two flags that say what taking a
//! name means -- replace whoever holds it, and do not stand in a queue behind
//! them. A daemon that queued would come up looking like it had worked and
//! would draw nothing, because the notifications would be going to whoever was
//! already there.
//!
//! **A call waits for its own answer.** `call` says a message and reads until
//! the reply carrying its serial comes back. Signals and inbound calls wait
//! in a bounded inbox for the receive owner. What comes back is the
//! answer whether it is a result or an error, since a service's error is a
//! sentence in the service's own vocabulary and the crate that speaks it is the
//! one that reads it. `Hello` and `RequestName` are the bus's own, and an error
//! from either is the bus refusing. `subscribe` is the third of the bus's own:
//! `AddMatch`, which is how a connection asks to hear signals that were not
//! addressed to it.
//!
//! A listener can wait for the first byte of a message until the socket closes.
//! Once a message starts, it has a deadline. Calls use one deadline from send
//! through reply, including any unrelated messages received in between.
//! A dispatcher owns one reader and matches a bounded set of pending calls.
//!
//! **A message is read in two goes.** The first sixteen bytes say how long the
//! rest is -- the header fields and the body each carry their own length --
//! so there is no scanning for a boundary and no buffer of leftovers: what is
//! read is exactly one message, every time.

use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::net::Shutdown;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fitted, fixed_index, index};
use console_core_synchronization::TimedMutex;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::io::Errno;
use rustix::net::{RecvFlags, SendFlags, recv, send};
use rustix::process::getuid;

use crate::messages::{self, Error, Header, Message, Value, Whom};
use crate::replies::{Pending, Received};

pub const BUS: &str = "org.freedesktop.DBus";

pub const BUS_PATH: &str = "/org/freedesktop/DBus";

const SESSION: &str = "DBUS_SESSION_BUS_ADDRESS";

const SYSTEM: &str = "DBUS_SYSTEM_BUS_ADDRESS";

const SYSTEM_SOCKET: &str = "unix:path=/var/run/dbus/system_bus_socket";

const TAKING: u32 = 6;

const OPERATION_TIMEOUT: Duration = Duration::from_secs(25);

const AUTH_LINE_LIMIT: u32 = 4096;
const INBOX_MESSAGES: u64 = 64;
const INBOX_BYTES: u64 = 4 << 20;
const PENDING_CALLS: u64 = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionError {
  Nowhere,
  Machine(String),
  Rejected(String),
  Ended,
  TimedOut,
  AuthenticationLineTooLong,
  MessageAllocation,
  SerialExhausted,
  InboxFull,
  CallsFull,
  Error(Error),
}

impl fmt::Display for ConnectionError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ConnectionError::Nowhere => write!(to, "this environment names no bus to connect to"),
      ConnectionError::Machine(why) => write!(to, "the socket would not: {why}"),
      ConnectionError::Rejected(why) => write!(to, "the bus refused: {why}"),
      ConnectionError::Ended => write!(to, "the bus hung up"),
      ConnectionError::TimedOut => write!(to, "the bus operation reached its deadline"),
      ConnectionError::AuthenticationLineTooLong => {
        write!(to, "the bus authentication reply is too long")
      }
      ConnectionError::MessageAllocation => {
        write!(to, "could not reserve space for the bus message")
      }
      ConnectionError::SerialExhausted => {
        write!(to, "the bus used every available message serial")
      }
      ConnectionError::InboxFull => write!(to, "the bus receive inbox filled"),
      ConnectionError::CallsFull => write!(to, "too many bus calls are outstanding"),
      ConnectionError::Error(torn) => write!(to, "{torn}"),
    }
  }
}

impl std::error::Error for ConnectionError {}

impl From<Error> for ConnectionError {
  fn from(torn: Error) -> Self {
    ConnectionError::Error(torn)
  }
}

impl From<io::Error> for ConnectionError {
  fn from(fault: io::Error) -> Self {
    let kind = fault.kind();
    let timed_out = [io::ErrorKind::TimedOut, io::ErrorKind::WouldBlock].contains(&kind);
    let ended = [
      io::ErrorKind::BrokenPipe,
      io::ErrorKind::ConnectionReset,
      io::ErrorKind::UnexpectedEof,
    ]
    .contains(&kind);

    match bool::then_some(timed_out, ()) {
      Some(()) => return Self::TimedOut,
      None => {}
    }

    match bool::then_some(ended, ()) {
      Some(()) => Self::Ended,
      None => Self::Machine(fault.to_string()),
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameRequestResult {
  PrimaryOwner,
  InQueue,
  Exists,
  AlreadyOwner,
}

impl NameRequestResult {
  fn of(value: u32) -> Result<Option<NameRequestResult>, Never> {
    Ok(match value {
      1 => Some(NameRequestResult::PrimaryOwner),
      2 => Some(NameRequestResult::InQueue),
      3 => Some(NameRequestResult::Exists),
      4 => Some(NameRequestResult::AlreadyOwner),
      _ => None,
    })
  }
}

pub struct Receiver {
  stream: UnixStream,
  clock: ReadClock,
  inbox: VecDeque<(Message, u64)>,
  inbox_bytes: u64,
  interrupted: Option<ConnectionError>,
}

#[derive(Debug, PartialEq)]
pub enum Incoming {
  Message(Message),
  Wake,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deferred {
  Ready,
  Empty,
}

pub struct Sender {
  held: TimedMutex<SenderState>,
  clock: ReadClock,
}

struct SenderState {
  stream: UnixStream,
  serial: u32,
}

pub struct Bus {
  hearing: Receiver,
  saying: Sender,
  named: String,
}

pub struct Dispatcher<Tag> {
  bus: Bus,
  pending: Pending<Waiting<Tag>>,
  closed: Option<ConnectionError>,
}

struct Waiting<Tag> {
  tag: Tag,
  until: Instant,
}

#[derive(Debug, PartialEq)]
pub enum Dispatched<Tag> {
  Message(Received<Tag>),
  TimedOut(Tag),
}

#[derive(Debug, PartialEq)]
pub struct Disconnected<Tag> {
  pub reason: ConnectionError,
  pub pending: Vec<Tag>,
}

impl Bus {
  pub fn session() -> Result<Bus, ConnectionError> {
    #[cfg_attr(
      dylint_lib = "explicit026_env_read_once",
      allow(
        explicit026_env_read_once,
        reason = "DBUS_SESSION_BUS_ADDRESS is this crate's own: what a session bus is, where it is and what an unset one means is the question this crate exists to answer"
      )
    )]
    let value = match std::env::var(SESSION) {
      Ok(value) => value,
      Err(_fault) => return Err(ConnectionError::Nowhere),
    };

    Bus::at(&value, Instant::now)
  }

  pub fn system() -> Result<Bus, ConnectionError> {
    #[cfg_attr(
      dylint_lib = "explicit026_env_read_once",
      allow(
        explicit026_env_read_once,
        reason = "DBUS_SYSTEM_BUS_ADDRESS is this crate's own, and unset it means the address the specification gives the system bus rather than no bus at all"
      )
    )]
    let value = match std::env::var(SYSTEM) {
      Ok(value) => value,
      Err(_unset) => SYSTEM_SOCKET.to_string(),
    };

    Bus::at(&value, Instant::now)
  }

  fn at(address: &str, clock: ReadClock) -> Result<Bus, ConnectionError> {
    let stream = opened(address)?;

    let writing = match stream.try_clone() {
      Ok(writing) => writing,
      Err(fault) => return Err(ConnectionError::Machine(fault.to_string())),
    };

    let Ok(saying) = Sender::new(writing, clock);
    let Ok(hearing) = Receiver::new(stream, clock);
    let mut bus = Bus {
      hearing,
      saying,
      named: String::new(),
    };

    bus.greet()?;

    let Ok(hello) = Message::call(&Whom {
      to: BUS,
      at: BUS_PATH,
      on: BUS,
      calling: "Hello",
    });
    let answer = bus.answered(&hello)?;

    let Ok(saying_2) = match answer.values.first() {
      Some(value) => value.text(),
      None => Ok(None),
    };

    bus.named = match saying_2 {
      Some(named) => named.to_string(),
      None => String::new(),
    };

    Ok(bus)
  }

  pub fn unique_name(&self) -> Result<&str, Never> {
    Ok(&self.named)
  }

  pub fn request_name(&mut self, name: &str) -> Result<NameRequestResult, ConnectionError> {
    let Ok(asking) = Message::call(&Whom {
      to: BUS,
      at: BUS_PATH,
      on: BUS,
      calling: "RequestName",
    });
    let Ok(word) = Value::word(name);
    let Ok(asking_2) = asking.carrying("su", vec![word, Value::Unsigned32(TAKING)]);

    let answer = self.answered(&asking_2)?;

    let Ok(counted) = match answer.values.first() {
      Some(value) => value.counted(),
      None => Ok(None),
    };

    let value = match counted {
      Some(value) => value,
      None => {
        return Err(ConnectionError::Rejected(format!(
          "{name} was answered with nothing"
        )));
      }
    };

    let value_3 = match u32::try_from(value) {
      Ok(value_2) => value_2,
      Err(_reply_outside_u32) => {
        return Err(ConnectionError::Rejected(format!(
          "{name} was answered with {value}"
        )));
      }
    };

    let Ok(got) = NameRequestResult::of(value_3);

    match got {
      Some(got_2) => Ok(got_2),
      None => Err(ConnectionError::Rejected(format!(
        "{name} was answered with {value}",
        value = value_3
      ))),
    }
  }

  pub fn say(&mut self, message: &Message) -> Result<u32, ConnectionError> {
    self.saying.say(message)
  }

  pub fn receive(&mut self) -> Result<Message, ConnectionError> {
    self.hearing.receive()
  }

  pub fn split(self) -> Result<(Receiver, Sender), Never> {
    Ok((self.hearing, self.saying))
  }

  pub fn dispatching<Tag>(self) -> Result<Dispatcher<Tag>, Never> {
    let Ok(pending) = Pending::new();

    Ok(Dispatcher {
      bus: self,
      pending,
      closed: None,
    })
  }

  pub fn call(&mut self, message: &Message) -> Result<Message, ConnectionError> {
    let deadline = Deadline::after(OPERATION_TIMEOUT, self.saying.clock)?;

    self.call_until(message, deadline)
  }

  pub fn call_before(
    &mut self,
    message: &Message,
    deadline: Instant,
  ) -> Result<Message, ConnectionError> {
    self.call_until(
      message,
      Deadline {
        end: deadline,
        clock: self.saying.clock,
      },
    )
  }

  fn call_until(
    &mut self,
    message: &Message,
    deadline: Deadline,
  ) -> Result<Message, ConnectionError> {
    let serial = self.saying.say_before(message, deadline)?;
    let Ok(mut pending) = Pending::new();
    let Ok(()) = pending.sent(serial, ());

    let answered = iterate((self, pending), |(this, mut pending_2)| {
      let (heard, bytes) = match this.hearing.read_message(Some(deadline)) {
        Ok(frame) => frame,
        Err(fault) => return Ok(Step::Halt(Err(fault))),
      };
      let Ok(received) = pending_2.received(heard);

      Ok(match received {
        Received::Reply { reply, .. } => Step::Halt(Ok(reply)),
        Received::Error { error, .. } => Step::Halt(Ok(error)),
        Received::Call(message_2) | Received::Signal(message_2) => {
          match this.hearing.defer(message_2, bytes) {
            Ok(()) => Step::Again((this, pending_2)),
            Err(fault) => Step::Halt(Err(fault)),
          }
        }
        Received::Unmatched(_) => Step::Again((this, pending_2)),
      })
    });

    match answered {
      Ok(answered_2) => answered_2,
      Err(Endless) => Err(ConnectionError::Ended),
    }
  }

  pub fn subscribe(&mut self, rule: &str) -> Result<(), ConnectionError> {
    let Ok(adding) = Message::call(&Whom {
      to: BUS,
      at: BUS_PATH,
      on: BUS,
      calling: "AddMatch",
    });
    let Ok(word) = Value::word(rule);
    let Ok(adding_2) = adding.carrying("s", vec![word]);

    let _added = self.answered(&adding_2)?;

    Ok(())
  }

  fn answered(&mut self, message: &Message) -> Result<Message, ConnectionError> {
    let answer = self.call(message)?;

    match answer.fault.clone() {
      Some(named) => Err(ConnectionError::Rejected(named)),
      None => Ok(answer),
    }
  }

  fn greet(&mut self) -> Result<(), ConnectionError> {
    let deadline = Deadline::after(OPERATION_TIMEOUT, self.saying.clock)?;
    let Ok(mine) = whoever();

    let mut value = String::from("\0AUTH EXTERNAL ");

    let Ok(()) = console_core_bytes::hex::append(
      &mut value, mine.to_string().as_bytes(), console_core_bytes::hex::Case::Lower,
    );

    value.push_str("\r\n");

    self.saying.plainly(value.as_bytes(), deadline)?;

    let answer = self.line(deadline)?;

    match ::core::primitive::bool::then_some(answer.starts_with("OK"), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(ConnectionError::Rejected(answer)),
    }

    self.saying.plainly(b"BEGIN\r\n", deadline)
  }

  fn line(&mut self, deadline: Deadline) -> Result<String, ConnectionError> {
    let mut line = String::new();

    for _ in 0..AUTH_LINE_LIMIT {
      let mut byte = 0;
      read_part(
        &self.hearing.stream,
        std::slice::from_mut(&mut byte),
        Some(deadline),
      )?;

      match ::core::primitive::bool::then_some(byte == b'\n', ()) {
        ::core::option::Option::Some(()) => return Ok(line.trim_end().to_string()),
        ::core::option::Option::None => {}
      }

      line.push(char::from(byte));
    }

    Err(ConnectionError::AuthenticationLineTooLong)
  }
}

impl<Tag> Dispatcher<Tag> {
  pub fn call(
    &mut self,
    tag: Tag,
    message: &Message,
    timeout: Duration,
  ) -> Result<u32, ConnectionError> {
    let Ok(count) = self.pending.len();

    match (self.closed.as_ref(), count >= PENDING_CALLS) {
      (Some(reason), _) => return Err(reason.clone()),
      (None, true) => return Err(ConnectionError::CallsFull),
      (None, false) => {}
    }

    let clock = self.bus.saying.clock;
    let waiting = Deadline::after(timeout, clock)?;
    let sending = Deadline::after(OPERATION_TIMEOUT.min(timeout), clock)?;
    let serial = self.bus.saying.say_before(message, sending)?;
    let Ok(()) = self.pending.sent(
      serial,
      Waiting {
        tag,
        until: waiting.end,
      },
    );

    Ok(serial)
  }

  pub fn say(&self, message: &Message) -> Result<u32, ConnectionError> {
    match &self.closed {
      Some(why) => Err(why.clone()),
      None => self.bus.saying.say(message),
    }
  }

  pub fn receive(&mut self) -> Result<Dispatched<Tag>, Disconnected<Tag>> {
    match &self.closed {
      Some(reason) => {
        return Err(Disconnected {
          reason: reason.clone(),
          pending: Vec::new(),
        });
      }
      None => {}
    }

    let Ok(earliest) = self.pending.earliest(|waiting| waiting.until);
    let clock = self.bus.hearing.clock;
    let deadline = earliest.map(|(_, end)| Deadline { end, clock });

    match self.bus.hearing.receive_until(deadline) {
      Ok(message) => {
        let Ok(dispatched) = self.pending.received(message);
        let received = match dispatched {
          Received::Reply { to, reply } => Received::Reply { to: to.tag, reply },
          Received::Error { to, error } => Received::Error { to: to.tag, error },
          Received::Call(message_2) => Received::Call(message_2),
          Received::Signal(message_3) => Received::Signal(message_3),
          Received::Unmatched(message_4) => Received::Unmatched(message_4),
        };

        Ok(Dispatched::Message(received))
      }
      Err(reason) => match (reason, self.bus.hearing.interrupted.as_ref(), earliest) {
        (ConnectionError::TimedOut, None, Some((serial, _))) => {
          let Ok(expired) = self.pending.remove(serial);

          match expired {
            Some(waiting) => Ok(Dispatched::TimedOut(waiting.tag)),
            None => self.disconnect(ConnectionError::TimedOut),
          }
        }
        (reason_2, _, _) => self.disconnect(reason_2),
      },
    }
  }

  fn disconnect(
    &mut self,
    reason: ConnectionError,
  ) -> Result<Dispatched<Tag>, Disconnected<Tag>> {
    let _closed = self.bus.hearing.stream.shutdown(Shutdown::Both);
    let Ok(pending) = self.pending.take();
    self.closed = Some(reason.clone());

    Err(Disconnected {
      reason,
      pending: pending.map(|waiting| waiting.tag).collect(),
    })
  }
}

impl Receiver {
  fn new(stream: UnixStream, clock: ReadClock) -> Result<Self, Never> {
    Ok(Self {
      stream,
      clock,
      inbox: VecDeque::new(),
      inbox_bytes: 0,
      interrupted: None,
    })
  }

  fn defer(&mut self, message: Message, bytes: u64) -> Result<(), ConnectionError> {
    let total = self.inbox_bytes.saturating_add(bytes);
    let Ok(count) = fixed_index(self.inbox.len());

    match (count < INBOX_MESSAGES && total <= INBOX_BYTES).then_some(()) {
      Some(()) => {
        self.inbox_bytes = total;
        self.inbox.push_back((message, bytes));

        Ok(())
      }
      None => {
        let _closed = self.stream.shutdown(Shutdown::Both);
        self.interrupted = Some(ConnectionError::InboxFull);

        Err(ConnectionError::InboxFull)
      }
    }
  }

  pub fn receive(&mut self) -> Result<Message, ConnectionError> {
    self.receive_until(None)
  }

  pub fn deferred(&self) -> Result<Deferred, Never> {
    Ok(match self.inbox.front() {
      Some(_message) => Deferred::Ready,
      None => Deferred::Empty,
    })
  }

  pub fn receive_with(&mut self, wakeup: BorrowedFd<'_>) -> Result<Incoming, ConnectionError> {
    let Ok(deferred) = self.deferred();
    let timeout = match deferred {
      Deferred::Ready => Some(Timespec::default()),
      Deferred::Empty => None,
    };
    let watched = [
      PollFd::new(&self.stream, PollFlags::IN),
      PollFd::from_borrowed_fd(wakeup, PollFlags::IN),
    ];
    let events = iterate(watched, |mut waiting| {
      Ok(match poll(&mut waiting, timeout.as_ref()) {
        Ok(_) => Step::Halt(Ok(waiting)),
        Err(Errno::INTR) => Step::Again(waiting),
        Err(fault) => {
          let io_fault = io::Error::from(fault);

          Step::Halt(Err(ConnectionError::from(io_fault)))
        }
      })
    })
    .map_err(|Endless| ConnectionError::Ended)?;
    let [bus, waker] = events?;

    match (
      deferred,
      bus.revents().is_empty(),
      waker.revents().is_empty(),
    ) {
      (_, _, false) => Ok(Incoming::Wake),
      (Deferred::Ready, _, true) | (Deferred::Empty, false, true) => {
        self.receive().map(Incoming::Message)
      }
      (Deferred::Empty, true, true) => Err(ConnectionError::Ended),
    }
  }

  fn receive_until(&mut self, deadline: Option<Deadline>) -> Result<Message, ConnectionError> {
    match self.inbox.pop_front() {
      Some((message, bytes)) => {
        self.inbox_bytes = self.inbox_bytes.saturating_sub(bytes);

        Ok(message)
      }
      None => self.read_message(deadline).map(|(message, _)| message),
    }
  }

  fn read_message(
    &mut self,
    deadline: Option<Deadline>,
  ) -> Result<(Message, u64), ConnectionError> {
    let mut header = Header::default();
    let [first, ..] = &mut header;
    read_part(&self.stream, std::slice::from_mut(first), deadline)?;

    let frame = match self.frame_after_first_byte(header, deadline) {
      Ok(frame) => frame,
      Err(reason) => {
        let _closed_at_unknown_message_boundary = self.stream.shutdown(Shutdown::Both);
        self.interrupted = Some(reason.clone());

        return Err(reason);
      }
    };

    let message = messages::read(&frame)?;
    let Ok(bytes) = fixed_index(frame.len());

    Ok((message, bytes))
  }

  fn frame_after_first_byte(
    &self,
    mut header: Header,
    deadline: Option<Deadline>,
  ) -> Result<Vec<u8>, ConnectionError> {
    let frame_deadline = match deadline {
      Some(calling) => calling,
      None => Deadline::after(OPERATION_TIMEOUT, self.clock)?,
    };
    let [_, rest @ ..] = &mut header;
    read_full(&self.stream, rest, frame_deadline)?;

    let whole = messages::length(&header)?;
    read_frame(&self.stream, &header, whole, frame_deadline)
  }
}

impl AsFd for Receiver {
  fn as_fd(&self) -> BorrowedFd<'_> {
    self.stream.as_fd()
  }
}

fn read_full(
  stream: &UnixStream,
  mut output: &mut [u8],
  deadline: Deadline,
) -> Result<(), ConnectionError> {
  let Ok(max_reads) = fixed_index(output.len());

  for _ in 0..max_reads {
    match ::core::primitive::bool::then_some(output.is_empty(), ()) {
      ::core::option::Option::Some(()) => break,
      ::core::option::Option::None => {}
    }

    let received = read_part(stream, output, Some(deadline))?;
    let Ok(offset) = index(received);
    let rest = output.get_mut(offset..).ok_or(Error::Short)?;
    output = rest;
  }

  Ok(())
}

fn read_frame(
  stream: &UnixStream,
  header: &Header,
  size: u32,
  deadline: Deadline,
) -> Result<Vec<u8>, ConnectionError> {
  let mut frame = header.to_vec();
  let mut scratch = [0; 8192];
  let Ok(chunk_size) = fitted::<_, u32>(scratch.len());
  let mut written = messages::HEAD;

  for _ in messages::HEAD..size {
    match ::core::primitive::bool::then_some(written == size, ()) {
      ::core::option::Option::Some(()) => break,
      ::core::option::Option::None => {}
    }

    let room = size.saturating_sub(written).min(chunk_size);
    let Ok(offered_end) = index(room);
    let offered = scratch.get_mut(..offered_end).ok_or(Error::Short)?;
    let received = read_part(stream, offered, Some(deadline))?;
    let Ok(received_end) = index(received);
    let bytes = offered.get(..received_end).ok_or(Error::Short)?;
    let required = written.saturating_add(received);
    let Ok(allocated) = fitted::<_, u32>(frame.capacity());

    match required.cmp(&allocated) {
      core::cmp::Ordering::Greater => {
        let capacity = allocated.saturating_mul(2).max(required).min(size);
        let additional = capacity.saturating_sub(written);
        let Ok(reservation) = index(additional);

        #[cfg_attr(
          dylint_lib = "explicit088_no_arriving_length",
          expect(
            explicit088_no_arriving_length,
            reason = "messages::length checks the protocol maximum before this function is called, and each bounded reservation follows a successful read"
          )
        )]
        let reserved = frame.try_reserve_exact(reservation);
        reserved.map_err(|_| ConnectionError::MessageAllocation)?;
      }
      core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {}
    }

    frame.extend_from_slice(bytes);
    written = required;
  }

  match ::core::primitive::bool::then_some(written == size, ()) {
    ::core::option::Option::Some(()) => Ok(frame),
    ::core::option::Option::None => Err(
      <crate::connection::ConnectionError as ::core::convert::From<crate::messages::Error>>::from(
        Error::Short,
      ),
    ),
  }
}

fn read_part(
  stream: &UnixStream,
  output: &mut [u8],
  deadline: Option<Deadline>,
) -> Result<u32, ConnectionError> {
  iterate(output, |part| {
    match deadline.map(Deadline::remaining).transpose() {
      Ok(_time_left) => {}
      Err(fault) => return Ok(Step::Halt(Err(fault))),
    }

    Ok(match recv(stream, &mut *part, RecvFlags::DONTWAIT) {
      Ok((0, _)) => Step::Halt(Err(ConnectionError::Ended)),
      Ok((got, _)) => Step::Halt(
        u32::try_from(got).map_err(|_outside_u32| ConnectionError::Error(Error::MessageTooLarge)),
      ),
      Err(Errno::INTR) => Step::Again(part),
      Err(Errno::AGAIN) => match wait_ready(stream, PollFlags::IN, deadline) {
        Ok(()) => Step::Again(part),
        Err(fault) => Step::Halt(Err(fault)),
      },
      Err(fault) => {
        let io_fault = io::Error::from(fault);
        Step::Halt(Err(ConnectionError::from(io_fault)))
      }
    })
  })
  .map_err(|Endless| ConnectionError::Ended)?
}

fn wait_ready(
  stream: &UnixStream,
  events: PollFlags,
  deadline: Option<Deadline>,
) -> Result<(), ConnectionError> {
  iterate([PollFd::new(stream, events)], |mut watched| {
    let timeout = match poll_timeout(deadline) {
      Ok(remaining) => remaining,
      Err(fault) => return Ok(Step::Halt(Err(fault))),
    };

    Ok(match poll(&mut watched, timeout.as_ref()) {
      Ok(0) => Step::Halt(Err(ConnectionError::TimedOut)),
      Ok(_) => Step::Halt(Ok(())),
      Err(Errno::INTR) => Step::Again(watched),
      Err(fault) => {
        let io_fault = io::Error::from(fault);
        Step::Halt(Err(ConnectionError::from(io_fault)))
      }
    })
  })
  .map_err(|Endless| ConnectionError::Ended)?
}

fn poll_timeout(deadline: Option<Deadline>) -> Result<Option<Timespec>, ConnectionError> {
  let remaining = deadline.map(Deadline::remaining).transpose()?;

  remaining
    .map(Timespec::try_from)
    .transpose()
    .map_err(|_outside_timespec| ConnectionError::TimedOut)
}

type ReadClock = fn() -> Instant;

#[derive(Clone, Copy)]
struct Deadline {
  end: Instant,
  clock: ReadClock,
}

impl Deadline {
  fn after(duration: Duration, clock: ReadClock) -> Result<Self, ConnectionError> {
    let end = clock()
      .checked_add(duration)
      .ok_or(ConnectionError::TimedOut)?;

    Ok(Self { end, clock })
  }

  fn remaining(self) -> Result<Duration, ConnectionError> {
    let remaining = self.end.saturating_duration_since((self.clock)());

    match bool::then_some(remaining.is_zero(), ()) {
      Some(()) => Err(ConnectionError::TimedOut),
      None => Ok(remaining),
    }
  }
}

impl Sender {
  fn new(stream: UnixStream, clock: ReadClock) -> Result<Self, Never> {
    let Ok(held) = TimedMutex::new(SenderState { stream, serial: 0 });
    Ok(Self { held, clock })
  }

  pub fn say(&self, message: &Message) -> Result<u32, ConnectionError> {
    let deadline = Deadline::after(OPERATION_TIMEOUT, self.clock)?;

    self.say_before(message, deadline)
  }

  fn say_before(&self, message: &Message, deadline: Deadline) -> Result<u32, ConnectionError> {
    self
      .held
      .with_until(deadline.end, deadline.clock, |held| {
        let serial = held
          .serial
          .checked_add(1)
          .ok_or(ConnectionError::SerialExhausted)?;
        let bytes = message.bytes(serial)?;

        write_full(&held.stream, &bytes, deadline)?;
        held.serial = serial;

        Ok(serial)
      })
      .ok_or(ConnectionError::TimedOut)?
  }

  fn plainly(&self, value: &[u8], deadline: Deadline) -> Result<(), ConnectionError> {
    self
      .held
      .with_until(deadline.end, deadline.clock, |held| {
        write_full(&held.stream, value, deadline)
      })
      .ok_or(ConnectionError::TimedOut)?
  }
}

fn write_full(
  stream: &UnixStream,
  bytes: &[u8],
  deadline: Deadline,
) -> Result<(), ConnectionError> {
  let Ok(max_writes) = fixed_index(bytes.len());
  let mut pending = bytes;

  for _ in 0..max_writes {
    let sent = match write_part(stream, pending, deadline) {
      Ok(sent) => sent,
      Err(fault) => {
        match bool::then_some(pending.len() < bytes.len(), ()) {
          Some(()) => {
            let _closed_after_partial_frame = stream.shutdown(Shutdown::Both);
          }
          None => {}
        }

        return Err(fault);
      }
    };
    let Ok(offset) = index(sent);

    pending = match pending.get(offset..) {
      Some([]) => return Ok(()),
      Some(rest) => rest,
      None => {
        let _closed_after_impossible_write_count = stream.shutdown(Shutdown::Both);
        return Err(ConnectionError::from(Error::Short));
      }
    };
  }

  Ok(())
}

fn write_part(
  stream: &UnixStream,
  bytes: &[u8],
  deadline: Deadline,
) -> Result<u64, ConnectionError> {
  let flags = SendFlags::DONTWAIT | SendFlags::NOSIGNAL;

  iterate((), |()| {
    match deadline.remaining() {
      Ok(_time_left) => {}
      Err(fault) => return Ok(Step::Halt(Err(fault))),
    }

    Ok(match send(stream, bytes, flags) {
      Ok(0) => Step::Halt(Err(ConnectionError::Ended)),
      Ok(sent) => {
        let Ok(written) = fixed_index(sent);
        Step::Halt(Ok(written))
      }
      Err(Errno::INTR) => Step::Again(()),
      Err(Errno::AGAIN) => match wait_ready(stream, PollFlags::OUT, Some(deadline)) {
        Ok(()) => Step::Again(()),
        Err(fault) => Step::Halt(Err(fault)),
      },
      Err(fault) => {
        let io_fault = io::Error::from(fault);
        Step::Halt(Err(ConnectionError::from(io_fault)))
      }
    })
  })
  .map_err(|Endless| ConnectionError::Ended)?
}

fn whoever() -> Result<u32, Never> {
  let mine = getuid();

  Ok(mine.as_raw())
}

fn opened(value: &str) -> Result<UnixStream, ConnectionError> {
  let first = match value.split(';').next() {
    Some(first) => first,
    None => return Err(ConnectionError::Nowhere),
  };

  let mut path: Option<&str> = None;
  let mut hidden: Option<&str> = None;

  for part in first.split(',') {
    match part.split_once('=') {
      Some(("unix:path", where_)) => path = Some(where_),
      Some(("path", where_)) => path = Some(where_),
      Some(("unix:abstract", where_)) => hidden = Some(where_),
      Some(("abstract", where_)) => hidden = Some(where_),
      Some(_) | None => {}
    }
  }

  let opened = match (path, hidden) {
    (Some(path_2), _) => console_operating_system_linux::unix_stream::connect(Path::new(path_2)),
    (None, Some(hidden_2)) => {
      console_operating_system_linux::unix_stream::connect_abstract(hidden_2.as_bytes())
    }
    (None, None) => return Err(ConnectionError::Nowhere),
  };

  opened.map_err(|fault| ConnectionError::Machine(fault.to_string()))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn deferred_messages_keep_wire_order_and_charge_their_actual_bytes()
  -> Result<(), ConnectionError> {
    let (stream, _peer) = UnixStream::pair().map_err(ConnectionError::from)?;
    let Ok(mut reader) = Receiver::new(stream, Instant::now);

    let first = Message {
      serial: 1,
      kind: messages::Kind::Signal,
      ..Message::default()
    };
    let second = Message {
      serial: 2,
      ..Message::default()
    };
    reader.defer(first.clone(), INBOX_BYTES.saturating_sub(1))?;
    reader.defer(second.clone(), 1)?;
    assert_eq!(reader.receive(), Ok(first));
    assert_eq!(reader.receive(), Ok(second));
    assert_eq!(reader.inbox_bytes, 0);

    let too_large = INBOX_BYTES.saturating_add(1);
    assert_eq!(
      reader.defer(Message::default(), too_large),
      Err(ConnectionError::InboxFull)
    );
    assert!(reader.inbox.is_empty());

    Ok(())
  }

  use std::io::Write;
  use std::os::fd::AsFd;

  const SHORT: Duration = Duration::from_millis(100);

  #[test]
  fn a_waiting_writer_times_out_without_consuming_a_serial() -> Result<(), ConnectionError> {
    let (stream, peer) = UnixStream::pair().map_err(ConnectionError::from)?;
    let Ok(sender) = Sender::new(stream, Instant::now);
    let outer = Deadline::after(OPERATION_TIMEOUT, Instant::now)?;

    let attempted_write = sender
      .held
      .with_until(outer.end, outer.clock, |_| -> Result<(), ConnectionError> {
        let deadline = Deadline::after(SHORT, Instant::now)?;
        assert_eq!(
          sender.say_before(&Message::default(), deadline),
          Err(ConnectionError::TimedOut)
        );
        Ok(())
      })
      .ok_or(ConnectionError::TimedOut)?;
    attempted_write?;

    assert_eq!(sender.say(&Message::default()), Ok(1));
    let Ok(mut receiver) = Receiver::new(peer, Instant::now);
    let deadline = Deadline::after(SHORT, Instant::now)?;
    let received = receiver.receive_until(Some(deadline))?;
    assert_eq!(received.serial, 1);

    Ok(())
  }

  #[test]
  fn a_ready_wakeup_leaves_the_next_bus_message_intact() -> Result<(), ConnectionError> {
    let (stream, mut peer) = UnixStream::pair().map_err(ConnectionError::from)?;
    let (woken, mut wake) = UnixStream::pair().map_err(ConnectionError::from)?;
    let Ok(mut receiver) = Receiver::new(stream, Instant::now);
    let bytes = Message::default().bytes(1)?;

    peer.write_all(&bytes).map_err(ConnectionError::from)?;
    wake.write_all(b"x").map_err(ConnectionError::from)?;

    assert_eq!(receiver.receive_with(woken.as_fd()), Ok(Incoming::Wake));

    let mut signal = [0];
    let deadline = Deadline::after(SHORT, Instant::now)?;
    let _read = read_part(&woken, &mut signal, Some(deadline))?;
    let next = receiver.receive_with(woken.as_fd())?;

    assert!(matches!(next, Incoming::Message(message) if message.serial == 1));
    drop(peer);
    assert_eq!(
      receiver.receive_with(woken.as_fd()),
      Err(ConnectionError::Ended)
    );

    Ok(())
  }

  #[test]
  fn an_oversized_frame_is_rejected_before_waiting_for_its_body() -> Result<(), ConnectionError> {
    let (stream, mut peer) = UnixStream::pair().map_err(ConnectionError::from)?;
    let Ok(mut receiver) = Receiver::new(stream, Instant::now);
    let body = messages::MAX_MESSAGE_BYTES.to_le_bytes();
    let head = [b'l', 1, 0, 1];
    let header = [head, body, 1_u32.to_le_bytes(), 0_u32.to_le_bytes()].concat();
    peer.write_all(&header).map_err(ConnectionError::from)?;
    let deadline = Deadline::after(SHORT, Instant::now)?;

    assert_eq!(
      receiver.receive_until(Some(deadline)),
      Err(ConnectionError::Error(Error::MessageTooLarge))
    );

    Ok(())
  }

  #[test]
  fn a_timeout_before_a_frame_starts_keeps_the_stream_aligned() -> Result<(), ConnectionError> {
    let (stream, mut peer) = UnixStream::pair().map_err(ConnectionError::from)?;
    let Ok(mut receiver) = Receiver::new(stream, Instant::now);
    let deadline = Deadline::after(SHORT, Instant::now)?;

    assert_eq!(
      receiver.receive_until(Some(deadline)),
      Err(ConnectionError::TimedOut)
    );

    let message = Message::default();
    let bytes = message.bytes(1)?;
    peer.write_all(&bytes).map_err(ConnectionError::from)?;
    let received = receiver.receive()?;

    assert_eq!(received.serial, 1);

    Ok(())
  }

  #[test]
  fn a_timeout_after_a_frame_starts_closes_the_stream() -> Result<(), ConnectionError> {
    let (stream, mut peer) = UnixStream::pair().map_err(ConnectionError::from)?;
    let Ok(mut receiver) = Receiver::new(stream, Instant::now);
    peer.write_all(b"l").map_err(ConnectionError::from)?;
    let deadline = Deadline::after(SHORT, Instant::now)?;

    assert_eq!(
      receiver.receive_until(Some(deadline)),
      Err(ConnectionError::TimedOut)
    );
    assert_eq!(receiver.receive(), Err(ConnectionError::Ended));

    Ok(())
  }

  #[test]
  fn a_timeout_after_a_write_starts_closes_the_stream() -> Result<(), ConnectionError> {
    let (stream, peer) = UnixStream::pair().map_err(ConnectionError::from)?;
    rustix::net::sockopt::set_socket_send_buffer_size(&stream, 4096).map_err(io::Error::from)?;
    let payload = [7_u8; 65_536];
    let Ok(payload_size) = fitted::<_, u32>(payload.len());
    let deadline = Deadline::after(Duration::from_secs(1), Instant::now)?;

    assert_eq!(
      write_full(&stream, &payload, deadline),
      Err(ConnectionError::TimedOut)
    );

    let read_until = Deadline::after(SHORT, Instant::now)?;
    let mut scratch = [0; 8192];
    let mut received = 0_u32;

    for _ in 0..payload_size {
      match read_part(&peer, &mut scratch, Some(read_until)) {
        Ok(count) => received = received.saturating_add(count),
        Err(ConnectionError::Ended) => {
          assert!((1..payload_size).contains(&received));
          return Ok(());
        }
        Err(fault) => return Err(fault),
      }
    }

    Err(ConnectionError::TimedOut)
  }
}
