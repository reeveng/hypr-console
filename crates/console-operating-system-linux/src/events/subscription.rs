//! The other end: a program asking the pool to tell it things.
//!
//! What comes back is a channel carrying the same [`Change`] a program's own
//! subscription handed it, so moving one over is a change of what it opens
//! rather than of how it is written.
//!
//! **It re-subscribes on every reconnection, and that is the point.** A pool
//! that was restarted knows nothing about who was listening to it, and a
//! program that subscribed once at start would go quiet for ever without
//! saying so. Re-subscribing also asks for the replay again, which is right:
//! after a gap, what a program wants is what is true now rather than the next
//! thing to change.
//!
//! **What is wanted can change while it is being received.** A panel that has
//! gone behind something wants nothing until it comes back, and the whole of
//! the pool's saving is in that: a listener that stays connected and quiet is a
//! handheld that stays asleep. So the event groups are held rather than taken
//! once, [`Subscriber::also`] and [`Subscriber::not`] write a `listen` or a
//! `stop-listening` down the connection that is already open, and the
//! reconnection asks for whatever is wanted at the moment it gets in. Coming
//! back is nothing more than asking again, because the pool replays the last
//! word on a event group to whoever has just subscribed -- so a panel that is
//! uncovered knows the volume without waiting for someone to change it.
//!
//! **Getting in is itself worth receiving.** A subscriber that was away has
//! missed whatever happened while it was, and for an event group where the last
//! word is the whole answer the replay says so by itself -- but for one where
//! the words are *something changed, ask again*, a gap is a reading that is
//! quietly out of date and nothing arrives to correct it. So
//! [`Received::Connected`] is said each time the subscription is made, which is
//! what every watch this replaced did when it connected. Whoever does not need
//! it matches it and does nothing, because the replay is already on its way
//! behind it.
//!
//! **The channel outlives every connection to the pool.** The sender is kept
//! here as well as in the thread that reconnects, so a program waiting on the
//! words is told nothing while the pool is down rather than told that the
//! channel has gone -- which is the difference between a wait and a spin. It is
//! also why a program that wants no event groups at all can still wait here:
//! the nothing it receives is the same nothing, and the pool is not asked for a
//! connection until something is wanted.
//!
//! **The two halves come apart.** A receiver is read by one thread, and a
//! program that sleeps on something other than this channel -- a panel asleep
//! in `poll` on its compositor -- wants a thread of its own blocked on the
//! words while it goes on choosing what it wants to hear. So [`Subscriber::split`]
//! hands back the [`Subscriptions`] and the receiver separately, and each goes
//! to whoever does that half.

use std::collections::BTreeSet;
use std::io::{BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use console_core_bounded_reads::{Line, LineEnding, LinePart};
use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_reconnect::{Round, keep};
use console_program_contract::{Change, EventGroup};

use super::place;
use super::wire::{self, Message};
use crate::unix_stream::{connect as connect_stream, receive_line};

pub struct Subscriber {
  subscriptions: Subscriptions,
  receiver: Receiver<Received>,
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Subscriptions is the sole permanent owner of the desired groups and writer. The reconnect worker keeps a weak handle and only borrows this state for bounded operations"
  )
)]
pub struct Subscriptions {
  desired: Arc<Mutex<Wanted>>,
  sender: SyncSender<Received>,
  _open: SyncSender<()>,
  at: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Received {
  Connected,
  Event(Change),
}

struct Wanted {
  event_groups: BTreeSet<EventGroup>,
  connection: Connection,
}

enum Connection {
  Dormant(Receiver<()>),
  Reconnecting,
  Established(UnixStream),
  Closed,
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "the reconnect worker must notice when the sole Subscriptions owner is gone without extending that owner's lifetime"
  )
)]
type Reconnect = Weak<Mutex<Wanted>>;

const WRITE_DEADLINE: Duration = Duration::from_millis(150);
const REPLY_DEADLINE: Duration = Duration::from_secs(1);
const DELIVERY_RETRY: Duration = Duration::from_millis(25);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desired {
  Some,
  None,
}

impl Subscriber {
  pub fn received(&self) -> Result<&Receiver<Received>, Never> {
    Ok(&self.receiver)
  }

  pub fn desired(&self) -> Result<Desired, Never> {
    self.subscriptions.desired()
  }

  pub fn subscribe(&self, event_group: &EventGroup) -> Result<(), Never> {
    self.subscriptions.subscribe(event_group)
  }

  pub fn unsubscribe(&self, event_group: &EventGroup) -> Result<(), Never> {
    self.subscriptions.unsubscribe(event_group)
  }

  pub fn split(self) -> Result<(Subscriptions, Receiver<Received>), Never> {
    Ok((self.subscriptions, self.receiver))
  }
}

impl Subscriptions {
  pub fn desired(&self) -> Result<Desired, Never> {
    let Ok(wanted) = lock(&self.desired);

    Ok(
      match ::core::primitive::bool::then_some(wanted.event_groups.is_empty(), ()) {
        ::core::option::Option::Some(()) => Desired::None,
        ::core::option::Option::None => Desired::Some,
      },
    )
  }

  pub fn subscribe(&self, event_group: &EventGroup) -> Result<(), Never> {
    let Ok(mut wanted) = lock(&self.desired);

    match ::core::primitive::bool::then_some(wanted.event_groups.insert(event_group.clone()), ())
    {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(()),
    }

    let Ok(()) = send(&mut wanted, &Message::Subscribe(event_group.clone()));

    let stopped = match std::mem::replace(&mut wanted.connection, Connection::Reconnecting) {
      Connection::Dormant(stopped) => stopped,
      ongoing @ (Connection::Reconnecting | Connection::Established(_) | Connection::Closed) => {
        wanted.connection = ongoing;

        return Ok(());
      }
    };

    drop(wanted);

    let at = match &self.at {
      Some(at) => at,
      None => return Ok(()),
    };

    #[cfg_attr(
      dylint_lib = "explicit081_no_shared_ownership",
      expect(
        explicit081_no_shared_ownership,
        reason = "downgrade supplies the reconnect worker with a non-owning reference; the subscriptions handle retains sole permanent ownership"
      )
    )]
    let worker = Arc::downgrade(&self.desired);

    keep_connected(at.clone(), worker, self.sender.clone(), stopped)
  }

  pub fn unsubscribe(&self, event_group: &EventGroup) -> Result<(), Never> {
    let Ok(mut wanted) = lock(&self.desired);

    match ::core::primitive::bool::then_some(wanted.event_groups.remove(event_group), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(()),
    }

    send(&mut wanted, &Message::Unsubscribe(event_group.clone()))
  }
}

impl Drop for Subscriptions {
  fn drop(&mut self) {
    let connection = {
      let Ok(mut wanted) = lock(&self.desired);

      std::mem::replace(&mut wanted.connection, Connection::Closed)
    };
    let Ok(()) = close_connection(connection);
  }
}

pub fn connect(event_groups: &[EventGroup]) -> Result<Subscriber, Never> {
  let at = match place::socket() {
    Ok(at) => Some(at),
    Err(fault) => {
      console_core_logging::warn!("console-events: {fault}, so there is no pool to ask");

      None
    }
  };

  started(at, event_groups)
}

pub fn connect_at(socket: &Path, event_groups: &[EventGroup]) -> Result<Subscriber, Never> {
  started(Some(socket.to_path_buf()), event_groups)
}

fn started(at: Option<PathBuf>, event_groups: &[EventGroup]) -> Result<Subscriber, Never> {
  let (sender, receiver) = sync_channel(256);
  let (open, stopped) = sync_channel(0);
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the subscriptions owner needs a weak reference for its reconnect worker and retains the only permanent strong reference"
    )
  )]
  let subscriptions = Subscriptions {
    desired: Arc::new(Mutex::new(Wanted {
      event_groups: BTreeSet::new(),
      connection: Connection::Dormant(stopped),
    })),
    sender,
    _open: open,
    at,
  };
  let subscriber = Subscriber {
    subscriptions,
    receiver,
  };

  for event_group in event_groups {
    #[cfg_attr(
      dylint_lib = "explicit043_no_unmatched_listen",
      allow(
        explicit043_no_unmatched_listen,
        reason = "the event groups the connection is opened with, which it is stopped listening by being dropped -- the pair this rule is about is an event group added part way through a life that goes on"
      )
    )]
    let Ok(()) = subscriber.subscribe(event_group);
  }

  Ok(subscriber)
}

fn keep_connected(
  at: PathBuf,
  desired: Reconnect,
  sender: SyncSender<Received>,
  stopped: Receiver<()>,
) -> Result<(), Never> {
  keep(move || {
    let Ok(round) = round(&at, &desired, &sender, &stopped);

    round
  })
}

enum Interest {
  Active,
  Gone,
}

fn interest(desired: &Reconnect) -> Result<Interest, Never> {
  let owner = match desired.upgrade() {
    Some(owner) => owner,
    None => return Ok(Interest::Gone),
  };
  let Ok(wanted) = lock(&owner);

  Ok(match wanted.connection {
    Connection::Closed => Interest::Gone,
    Connection::Dormant(_) | Connection::Reconnecting | Connection::Established(_) => {
      Interest::Active
    }
  })
}

fn open_connection(socket: &Path) -> std::io::Result<UnixStream> {
  let stream = connect_stream(socket)?;
  stream.set_write_timeout(Some(WRITE_DEADLINE))?;

  Ok(stream)
}

fn round(
  socket: &Path,
  desired: &Reconnect,
  sender: &SyncSender<Received>,
  stopped: &Receiver<()>,
) -> Result<Round, Never> {
  let Ok(interest) = interest(desired);

  match interest {
    Interest::Gone => return Ok(Round::Finished),
    Interest::Active => {}
  }

  let stream = match open_connection(socket) {
    Ok(stream) => stream,
    Err(fault) => {
      console_core_logging::trace!(
        "console-events: connecting to {}: {fault}",
        socket.display()
      );

      return Ok(Round::Another);
    }
  };
  let reading = match stream.try_clone() {
    Ok(reading) => reading,
    Err(fault) => {
      console_core_logging::warn!("console-events: keeping a socket for reading: {fault}");

      return Ok(Round::Another);
    }
  };
  let Ok(subscribed) = subscribed(desired, stream);

  match subscribed {
    SubscribeResult::Subscribed => read_events(reading, desired, sender, stopped),
    SubscribeResult::Rejected => Ok(Round::Another),
    SubscribeResult::Closed => Ok(Round::Finished),
  }
}

#[derive(Copy, Clone)]
enum Delivery {
  Sent,
  Closed,
}

fn deliver(
  desired: &Reconnect,
  sender: &SyncSender<Received>,
  stopped: &Receiver<()>,
  event: Received,
) -> Result<Delivery, Never> {
  let delivered = iterate(event, |pending| {
    let Ok(interest) = interest(desired);

    match interest {
      Interest::Gone => return Ok(Step::Halt(Delivery::Closed)),
      Interest::Active => {}
    }

    Ok(match sender.try_send(pending) {
      Ok(()) => Step::Halt(Delivery::Sent),
      Err(TrySendError::Disconnected(_)) => Step::Halt(Delivery::Closed),
      Err(TrySendError::Full(queued)) => match stopped.recv_timeout(DELIVERY_RETRY) {
        Err(RecvTimeoutError::Timeout) => Step::Again(queued),
        Ok(()) | Err(RecvTimeoutError::Disconnected) => Step::Halt(Delivery::Closed),
      },
    })
  });

  Ok(match delivered {
    Ok(done) => done,
    Err(fault) => {
      console_core_logging::error!("console-events: forwarding stopped: {fault}");

      Delivery::Closed
    }
  })
}

fn read_events(
  reading: UnixStream,
  desired: &Reconnect,
  sender: &SyncSender<Received>,
  stopped: &Receiver<()>,
) -> Result<Round, Never> {
  let Ok(connected) = deliver(desired, sender, stopped, Received::Connected);

  match connected {
    Delivery::Sent => {}
    Delivery::Closed => return retired(desired),
  }

  let mut input = BufReader::new(reading);
  let Ok(mut frame) = Line::new(wire::REPLY_BYTES, LineEnding::Required);

  #[cfg_attr(
    dylint_lib = "explicit054_no_loop",
    expect(
      explicit054_no_loop,
      reason = "this worker owns a persistent subscription and closes it on source errors or when delivery is retired"
    )
  )]
  'reading: loop {
    match receive_line(&mut input, &mut frame, None, REPLY_DEADLINE) {
      Ok(LinePart::Complete) => {}
      Ok(LinePart::End | LinePart::Partial) => break 'reading,
      Err(fault) => {
        console_core_logging::debug!("console-events: reading from the pool: {fault}");

        break 'reading;
      }
    }

    let Ok(bytes) = frame.bytes();
    let line = match std::str::from_utf8(bytes) {
      Ok(line) => match line.strip_suffix('\r') {
        Some(stripped) => stripped,
        None => line,
      },
      Err(fault) => {
        console_core_logging::debug!("console-events: decoding a reply from the pool: {fault}");

        break 'reading;
      }
    };
    let Ok(message) = wire::decoded(line);

    match message {
      Some(Message::Publish(change)) => {
        let Ok(delivered) = deliver(desired, sender, stopped, Received::Event(change));

        match delivered {
          Delivery::Sent => {}
          Delivery::Closed => return retired(desired),
        }
      }
      Some(Message::Subscribe(_) | Message::Unsubscribe(_)) | None => {}
    }
  }

  disconnected(desired)
}

fn disconnected(desired: &Reconnect) -> Result<Round, Never> {
  let owner = match desired.upgrade() {
    Some(owner) => owner,
    None => return Ok(Round::Finished),
  };
  let Ok(mut wanted) = lock(&owner);

  Ok(match wanted.connection {
    Connection::Closed => Round::Finished,
    Connection::Dormant(_) | Connection::Reconnecting | Connection::Established(_) => {
      wanted.connection = Connection::Reconnecting;

      Round::Another
    }
  })
}

fn retired(desired: &Reconnect) -> Result<Round, Never> {
  let owner = match desired.upgrade() {
    Some(owner) => owner,
    None => return Ok(Round::Finished),
  };
  let ended = {
    let Ok(mut wanted) = lock(&owner);

    std::mem::replace(&mut wanted.connection, Connection::Closed)
  };
  let Ok(()) = close_connection(ended);

  Ok(Round::Finished)
}

enum SubscribeResult {
  Subscribed,
  Rejected,
  Closed,
}

fn subscribed(desired: &Reconnect, mut stream: UnixStream) -> Result<SubscribeResult, Never> {
  let owner = match desired.upgrade() {
    Some(owner) => owner,
    None => return Ok(SubscribeResult::Closed),
  };
  let Ok(mut wanted) = lock(&owner);

  match wanted.connection {
    Connection::Closed => return Ok(SubscribeResult::Closed),
    Connection::Dormant(_) | Connection::Reconnecting | Connection::Established(_) => {}
  }

  for event_group in &wanted.event_groups {
    let Ok(spelled) = wire::encoded(&Message::Subscribe(event_group.clone()));

    match writeln!(stream, "{spelled}") {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::debug!("console-events: replaying subscriptions: {fault}");

        return Ok(SubscribeResult::Rejected);
      }
    }
  }

  wanted.connection = Connection::Established(stream);

  Ok(SubscribeResult::Subscribed)
}

fn send(wanted: &mut Wanted, message: &Message) -> Result<(), Never> {
  let Ok(spelled) = wire::encoded(message);

  let sent = match &mut wanted.connection {
    Connection::Established(stream) => writeln!(stream, "{spelled}"),
    Connection::Dormant(_) | Connection::Reconnecting | Connection::Closed => return Ok(()),
  };

  match sent {
    Ok(()) => {}
    Err(fault) => {
      console_core_logging::debug!("console-events: updating a subscription: {fault}");

      let broken = std::mem::replace(&mut wanted.connection, Connection::Reconnecting);
      let Ok(()) = close_connection(broken);
    }
  }

  Ok(())
}

fn close_connection(connection: Connection) -> Result<(), Never> {
  match connection {
    Connection::Established(stream) => match stream.shutdown(Shutdown::Both) {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::trace!("console-events: closing a subscription: {fault}")
      }
    },
    Connection::Dormant(_) | Connection::Reconnecting | Connection::Closed => {}
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the two owners never invoke caller code under the guard. Each section updates the desired groups or performs socket writes with a write timeout"
  )
)]
fn lock(wanted: &Mutex<Wanted>) -> Result<MutexGuard<'_, Wanted>, Never> {
  Ok(match wanted.lock() {
    Ok(held) => held,
    Err(poisoned) => poisoned.into_inner(),
  })
}
