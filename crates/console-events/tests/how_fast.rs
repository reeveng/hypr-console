//! Not a check. A number, for as long as someone is looking at one.

pub mod pool;

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{Float, fitted};
use console_events::subscription::{Received, connect_at};
use console_events::wire::{self, Message};
use console_program_contract::{Change, EventGroup};
use console_program_lifetime::threads;
use console_waiting::{Outcome, Ready, Schedule};
use pool::{BEFORE_LONG, Failure, serve_at, socket};

const A_WHILE: Duration = Duration::from_secs(120);

const EVERY_SO_OFTEN: Duration = Duration::from_millis(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Subscriber {
  Subscribed,
  BytesOnly,
}

#[derive(Debug, Default)]
struct Counted {
  taken: AtomicU64,
  again: AtomicU64,
}

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  allow(
    explicit026_env_read_once,
    reason = "this is run by hand, and the numbers are what the person running it asks for on the command line"
  )
)]
fn how_many(named: &str, usual: u64) -> Result<u64, Never> {
  Ok(match std::env::var(named) {
    Ok(said) => match said.parse() {
      Ok(many) => many,
      Err(_not_a_number) => usual,
    },
    Err(_not_asked_for) => usual,
  })
}

#[cfg_attr(
  dylint_lib = "explicit055_no_unbounded_loop",
  expect(
    explicit055_no_unbounded_loop,
    reason = "the benchmark listener consumes events for the lifetime of the subscription and stops on channel closure"
  )
)]
fn subscribed(at: &Path, counted: &Counted) -> Result<(), Never> {
  let Ok(subscriber) = connect_at(at, &[EventGroup::Sound]);
  let Ok(heard) = subscriber.received();

  for word in heard {
    match word {
      Received::Event(_) => {
        let _ = counted.taken.fetch_add(1, Ordering::SeqCst);
      }
      Received::Connected => {
        let _ = counted.again.fetch_add(1, Ordering::SeqCst);
      }
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the benchmark sets a socket read timeout to its measurement window before reading"
  )
)]
fn bytes_only(at: &Path, counted: &Counted) -> Result<(), Failure> {
  let stream = UnixStream::connect(at)?;

  stream.set_read_timeout(Some(A_WHILE))?;

  let asked = wire::encoded(&Message::Subscribe(EventGroup::Sound))?;
  let mut asking = stream.try_clone()?;

  asking.write_all(format!("{asked}\n").as_bytes())?;

  let _ = counted.again.fetch_add(1, Ordering::SeqCst);
  let buffer = Box::new([0; 1 << 16]);

  let read = iterate((stream, buffer), |(mut stream_2, mut buffer_2)| {
    let got = match stream_2.read(buffer_2.as_mut_slice()) {
      Ok(0) => return Ok(Step::Halt(Ok(()))),
      Ok(read) => match buffer_2.get(..read) {
        Some(got) => got,
        None => {
          return Ok(Step::Halt(Err(Failure::from(
            "read more than the buffer holds",
          ))));
        }
      },
      Err(fault) => return Ok(Step::Halt(Err(Failure::from(fault)))),
    };
    let Ok(lines) = fitted::<_, u64>(got.iter().filter(|byte| **byte == b'\n').count());
    let _ = counted.taken.fetch_add(lines, Ordering::SeqCst);

    Ok(Step::Again((stream_2, buffer_2)))
  });

  match read {
    Ok(read_2) => read_2,
    Err(Endless) => Ok(()),
  }
}

fn until_counted(
  count: &AtomicU64,
  (wanted, patience): (u64, Duration),
) -> Result<Outcome, Never> {
  let Ok(schedule) = Schedule::asking_every(patience, EVERY_SO_OFTEN);

  console_waiting::until(schedule, || {
    Ok(match ::core::primitive::bool::then_some(count.load(Ordering::SeqCst) >= wanted, ()) {
      ::core::option::Option::Some(()) => Ready::Yes,
      ::core::option::Option::None => Ready::NotYet,
    })
  })
}

#[test]
#[ignore]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "the detached benchmark listeners and their reporting thread share one pair of atomic counters"
  )
)]
fn how_many_words_a_second() -> Result<(), Failure> {
  let Ok(who) = how_many("WHO", 8);
  let Ok(words) = how_many("WORDS", 100_000);
  let Ok(raw) = how_many("RAW", 0);
  let listening = match raw {
    0 => Subscriber::Subscribed,
    _any_other => Subscriber::BytesOnly,
  };
  let at = socket("how-fast")?;
  let handed = serve_at(&at)?;
  let counted = Arc::new(Counted::default());

  for _who in 0..who {
    let mine = at.clone();
    let theirs = Arc::clone(&counted);

    #[cfg_attr(
      dylint_lib = "explicit103_no_thread_per_item",
      expect(
        explicit103_no_thread_per_item,
        reason = "the listener benchmark needs each concurrent socket reader to run independently, including the intentionally stalled listener"
      )
    )]
    let Ok(()) = threads::let_go(std::thread::spawn(move || match listening {
      Subscriber::Subscribed => {
        let Ok(()) = subscribed(&mine, &theirs);
      }
      Subscriber::BytesOnly => {
        let _a_listener_that_stopped_is_counted_short = bytes_only(&mine, &theirs);
      }
    }));
  }

  let saying = handed
    .recv_timeout(BEFORE_LONG)
    .map_err(|_| "the source was never opened")?;
  let Ok(_everyone_in) = until_counted(&counted.again, (who, BEFORE_LONG));
  let wanted = who.saturating_mul(words);
  let began = Instant::now();

  for word in 0..words {
    let said = format!("Event 'change' on sink #{word} at 0x7f3a2b4c5d6e volume 0.42");

    saying
      .send(Change {
        event_group: EventGroup::Sound,
        text: said,
      })
      .map_err(|_| "the pool stopped listening")?;
  }

  let sent = began.elapsed();
  let Ok(_all_out) = until_counted(&counted.taken, (wanted, A_WHILE));
  let took = began.elapsed().as_secs_f64();
  let taken = counted.taken.load(Ordering::SeqCst);
  let Ok(words_said) = words.float();
  let Ok(lines_out) = taken.float();

  println!(
    "{who} listeners, {words} words: handed over in {sent:?}, {taken} of {wanted} \
         lines out in {took:.3}s, {} subscriptions made -- {:.0} words/s, {:.0} lines/s",
    counted.again.load(Ordering::SeqCst),
    words_said / took,
    lines_out / took
  );

  std::fs::remove_file(&at)?;

  Ok(())
}
