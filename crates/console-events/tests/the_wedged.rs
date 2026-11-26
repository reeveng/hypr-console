//! One program that stops reading must not stop the others from hearing.
//!
//! The pool tells everyone from one loop, and a write into a socket whose
//! reader has gone to sleep blocks once the kernel's buffer for it is full. On
//! a desktop that is not a slow subscriber, it is every subscriber: the volume
//! stops moving on the bar because a panel behind a picker stopped reading.
//! Nothing in `pool` can be asked about it, because the fault is in the telling
//! rather than in the arithmetic, so this is the serving loop, the real wire
//! and two programs -- one that reads and one that never does.
//!
//! The words are long on purpose and there is more of them than the outbox
//! holds. Two things have to fill before anything is let go -- the socket's own
//! buffer, and then the megabytes the pool is willing to hold for a program
//! that is not reading -- and spelling that much in short lines would be
//! hundreds of thousands of turns of a loop for a test to sit through.

pub mod pool;

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::time::{Duration, Instant};

use console_core_never::Never;
use console_core_number_conversion::index;
use console_events::subscription::{Received, connect_at};
use console_events::wire::{self, Message};
use console_program_contract::{Change, EventGroup};
use console_waiting::{Outcome, Ready, Schedule, until_handed};
use pool::{BEFORE_LONG, Failure, change, heard_before, serve_at, socket};

const WORDS: u32 = 3_000;

const LONG: u32 = 4096;

const LAST: &str = "the last word";
const PROBE: &str = "while the reader is wedged";
const HEALTHY_LATENCY: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Delivered {
  TheLastWord,
  NothingMore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Connection {
  Ended,
  StillOpen,
}

fn until_word(heard: &Receiver<Received>, word: &str) -> Result<Delivered, Never> {
  let Ok(ended) = heard_before(heard, BEFORE_LONG, |received| match received {
    Received::Event(change) => {
      match ::core::primitive::bool::then_some(change.text == word, ()) {
        ::core::option::Option::Some(()) => Some(Delivered::TheLastWord),
        ::core::option::Option::None => None,
      }
    }
    Received::Connected => None,
  });

  Ok(match ended {
    Some(ended_2) => ended_2,
    None => Delivered::NothingMore,
  })
}

fn deaf(at: &Path) -> Result<UnixStream, Failure> {
  let stream = UnixStream::connect(at)?;
  let asked = wire::encoded(&Message::Subscribe(EventGroup::Sound))?;
  let mut asking = stream.try_clone()?;

  writeln!(asking, "{asked}")?;

  Ok(stream)
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the test sets the socket read timeout immediately before draining the finite buffer of its disconnected peer"
  )
)]
fn read_out(wedged: &mut UnixStream) -> Result<Connection, Never> {
  let mut taken = [0; 4096];

  for _turn in 0..WORDS {
    match wedged.read(&mut taken) {
      Ok(0) => return Ok(Connection::Ended),
      Ok(_some_of_what_it_was_told) => {}
      Err(_it_is_still_open_and_saying_nothing) => return Ok(Connection::StillOpen),
    }
  }

  Ok(Connection::StillOpen)
}

enum Offer {
  Pending(Change),
  Accepted,
  Closed,
}

fn publish(sender: &SyncSender<Change>, event: Change) -> std::io::Result<()> {
  let Ok(schedule) = Schedule::asking_every(BEFORE_LONG, Duration::from_millis(1));
  let mut pending = Offer::Pending(event);
  let Ok(attempted) = until_handed(schedule, &mut pending, |state| {
    let next = std::mem::replace(state, Offer::Closed);
    let change = match next {
      Offer::Pending(waiting) => waiting,
      Offer::Accepted | Offer::Closed => return Ok(Ready::Yes),
    };

    Ok(match sender.try_send(change) {
      Ok(()) => {
        *state = Offer::Accepted;

        Ready::Yes
      }
      Err(TrySendError::Full(retry)) => {
        *state = Offer::Pending(retry);

        Ready::NotYet
      }
      Err(TrySendError::Disconnected(_undelivered)) => Ready::Yes,
    })
  });

  match (attempted, pending) {
    (Outcome::Happened, Offer::Accepted) => Ok(()),
    (Outcome::Happened, Offer::Closed | Offer::Pending(_)) => {
      Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }
    (Outcome::RanOut, _) => Err(std::io::Error::from(std::io::ErrorKind::TimedOut)),
  }
}

#[test]
#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the publisher sends a fixed number of messages and abandons the run if one reaches its send deadline"
  )
)]
fn a_program_that_stopped_reading_does_not_stop_the_words_reaching_anyone_else()
-> Result<(), Failure> {
  let at = socket("wedged")?;
  let handed = serve_at(&at)?;
  let mut wedged = deaf(&at)?;

  let Ok(subscriber) = connect_at(&at, &[EventGroup::Sound]);
  let Ok(heard) = subscriber.received();
  let saying = handed
    .recv_timeout(BEFORE_LONG)
    .map_err(|_| "the source was never opened")?;

  assert_eq!(heard.recv_timeout(BEFORE_LONG), Ok(Received::Connected));

  let Ok(long) = index(LONG);
  let long_2 = "a".repeat(long);
  let (probed, samples) = sync_channel(2);
  let delivered = std::thread::scope(|scope| -> Result<(), Failure> {
    let sending = scope.spawn(move || -> std::io::Result<()> {
      for word in 0..WORDS {
        let Ok(said) = change(&format!("{word} {long}", long = long_2));
        publish(&saying, said)?;

        match word {
          120 => {
            let Ok(probe) = change(PROBE);
            probed
              .send(Instant::now())
              .map_err(|_closed| std::io::Error::from(std::io::ErrorKind::BrokenPipe))?;
            publish(&saying, probe)?;
          }
          _ => {}
        }
      }

      let Ok(last) = change(LAST);
      probed
        .send(Instant::now())
        .map_err(|_closed| std::io::Error::from(std::io::ErrorKind::BrokenPipe))?;
      publish(&saying, last)?;

      Ok(())
    });

    for marker in [PROBE, LAST] {
      let Ok(delivered) = until_word(heard, marker);
      let began = samples.recv_timeout(BEFORE_LONG)?;
      let elapsed = began.elapsed();
      assert_eq!(
        delivered,
        Delivered::TheLastWord,
        "healthy subscriber missed {marker}"
      );
      assert!(
        elapsed <= HEALTHY_LATENCY,
        "healthy subscriber took {elapsed:?} for {marker}"
      );
    }

    let map_err = sending.join().map_err(|_| "publication worker panicked")?;
    map_err?;

    Ok(())
  });
  delivered?;

  wedged.set_read_timeout(Some(BEFORE_LONG))?;

  assert_eq!(
    read_out(&mut wedged),
    Ok(Connection::Ended),
    "the program that was let go was left connected and hearing nothing, which it has no \
         way to notice and no way to recover from"
  );

  std::fs::remove_file(&at)?;

  Ok(())
}
