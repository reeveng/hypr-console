//! Not a check. A number, for as long as someone is looking at one.

mod pool;

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{Float, fitted, index};
use console_events::subscription::{Received, connect_at};
use console_events::wire::{self, Message};
use console_program_contract::{Change, Topic};
use console_program_lifetime::threads;
use console_waiting::{Outcome, Ready, Schedule};
use pool::{Failure, BEFORE_LONG, serve_at, socket};

const A_WHILE: Duration = Duration::from_secs(120);

const EVERY_SO_OFTEN: Duration = Duration::from_millis(5);

const READ_AT_ONCE: u32 = 1 << 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Subscriber {
    Subscribed,
    BytesOnly,
}

#[derive(Debug, Clone, Default)]
struct Counted {
    taken: Arc<AtomicU64>,
    again: Arc<AtomicU64>,
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(explicit026_env_read_once, reason = "this is run by hand, and the numbers are what the person running it asks for on the command line")
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

fn subscribed(at: &Path, counted: &Counted) -> Result<(), Never> {
    let Ok(subscriber) = connect_at(at, &[Topic::Sound]);
    let Ok(heard) = subscriber.received();

    for word in heard {
        match word {
            Received::Event(_) => {
                let _ = counted.taken.fetch_add(1, Ordering::Relaxed);
            },
            Received::Connected => {
                let _ = counted.again.fetch_add(1, Ordering::Relaxed);
            },
        }
    }

    Ok(())
}

fn bytes_only(at: &Path, counted: &Counted) -> Result<(), Failure> {
    let stream = UnixStream::connect(at)?;
    let asked = wire::encoded(&Message::Subscribe(Topic::Sound))?;
    let mut asking = stream.try_clone()?;

    asking.write_all(format!("{asked}\n").as_bytes())?;

    let _ = counted.again.fetch_add(1, Ordering::Relaxed);
    let Ok(at_once) = index(READ_AT_ONCE);
    let buffer = vec![0; at_once];

    let read = iterate((stream, buffer), |(mut stream, mut buffer)| {
        let got = match stream.read(&mut buffer) {
            Ok(0) => return Ok(Step::Halt(Ok(()))),
            Ok(read) => match buffer.get(..read) {
                Some(got) => got,
                None => return Ok(Step::Halt(Err(Failure::from("read more than the buffer holds")))),
            },
            Err(fault) => return Ok(Step::Halt(Err(Failure::from(fault)))),
        };
        let Ok(lines) = fitted::<_, u64>(got.iter().filter(|byte| **byte == b'\n').count());
        let _ = counted.taken.fetch_add(lines, Ordering::Relaxed);

        Ok(Step::Again((stream, buffer)))
    });

    match read {
        Ok(read) => read,
        Err(Endless) => Ok(()),
    }
}

fn until_counted(count: &AtomicU64, (wanted, patience): (u64, Duration)) -> Result<Outcome, Never> {
    let Ok(schedule) = Schedule::asking_every(patience, EVERY_SO_OFTEN);

    console_waiting::until(schedule, || {
        Ok(match count.load(Ordering::Relaxed) >= wanted {
            true => Ready::Yes,
            false => Ready::NotYet,
        })
    })
}

#[test]
#[ignore]
#[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(explicit039_no_reading_the_clock, reason = "how long the words took is the whole of what this measures")
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
    let counted = Counted::default();

    for _who in 0..who {
        let mine = at.clone();
        let theirs = counted.clone();

        let Ok(()) = threads::let_go(std::thread::spawn(move || match listening {
            Subscriber::Subscribed => {
                let Ok(()) = subscribed(&mine, &theirs);
            },
            Subscriber::BytesOnly => {
                let _a_listener_that_stopped_is_counted_short = bytes_only(&mine, &theirs);
            },
        }));
    }

    let saying = handed.recv_timeout(BEFORE_LONG).map_err(|_| "the source was never opened")?;
    let Ok(_everyone_in) = until_counted(&counted.again, (who, BEFORE_LONG));
    let wanted = who.saturating_mul(words);
    let began = Instant::now();

    for word in 0..words {
        let said = format!("Event 'change' on sink #{word} at 0x7f3a2b4c5d6e volume 0.42");

        saying.send(Change { topic: Topic::Sound, text: said }).map_err(|_| "the pool stopped listening")?;
    }

    let sent = began.elapsed();
    let Ok(_all_out) = until_counted(&counted.taken, (wanted, A_WHILE));
    let took = began.elapsed().as_secs_f64();
    let taken = counted.taken.load(Ordering::Relaxed);
    let Ok(words_said) = words.float();
    let Ok(lines_out) = taken.float();

    println!(
        "{who} listeners, {words} words: handed over in {sent:?}, {taken} of {wanted} \
         lines out in {took:.3}s, {} subscriptions made -- {:.0} words/s, {:.0} lines/s",
        counted.again.load(Ordering::Relaxed),
        words_said / took,
        lines_out / took
    );

    let _ = std::fs::remove_file(&at);

    Ok(())
}
