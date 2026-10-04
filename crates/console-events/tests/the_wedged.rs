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

mod pool;

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::mpsc::Receiver;

use console_core_never::Never;
use console_core_number_conversion::index;
use console_events::subscription::{Received, connect_at};
use console_events::wire::{self, Message};
use console_program_contract::EventGroup;
use pool::{Failure, BEFORE_LONG, change, serve_at, socket};

const WORDS: u32 = 3_000;

const LONG: u32 = 4096;

const LAST: &str = "the last word";

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

fn to_the_end(heard: &Receiver<Received>) -> Result<Delivered, Never> {
    let ended = std::iter::repeat_with(|| heard.recv_timeout(BEFORE_LONG)).find_map(|received| match received {
        Ok(Received::Event(change)) => match change.text == LAST {
            true => Some(Delivered::TheLastWord),
            false => None,
        },
        Ok(Received::Connected) => None,
        Err(_nothing_more_is_coming) => Some(Delivered::NothingMore),
    });

    Ok(match ended {
        Some(ended) => ended,
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

fn read_out(wedged: &mut UnixStream) -> Result<Connection, Never> {
    let Ok(long) = index(LONG);
    let mut taken = vec![0; long];

    for _turn in 0..WORDS {
        match wedged.read(&mut taken) {
            Ok(0) => return Ok(Connection::Ended),
            Ok(_some_of_what_it_was_told) => {},
            Err(_it_is_still_open_and_saying_nothing) => return Ok(Connection::StillOpen),
        }
    }

    Ok(Connection::StillOpen)
}

#[test]
fn a_program_that_stopped_reading_does_not_stop_the_words_reaching_anyone_else() -> Result<(), Failure> {
    let at = socket("wedged")?;
    let handed = serve_at(&at)?;
    let mut wedged = deaf(&at)?;

    let Ok(subscriber) = connect_at(&at, &[EventGroup::Sound]);
    let Ok(heard) = subscriber.received();
    let saying = handed.recv_timeout(BEFORE_LONG).map_err(|_| "the source was never opened")?;

    assert_eq!(heard.recv_timeout(BEFORE_LONG), Ok(Received::Connected));

    let Ok(long) = index(LONG);
    let long = "a".repeat(long);

    for word in 0..WORDS {
        let Ok(said) = change(&format!("{word} {long}"));

        saying.send(said).map_err(|_| "the pool stopped listening")?;
    }

    let Ok(last) = change(LAST);

    saying.send(last).map_err(|_| "the pool stopped listening")?;

    assert_eq!(
        to_the_end(heard),
        Ok(Delivered::TheLastWord),
        "a program that had stopped reading held up every word to everyone else, so the bar \
         goes quiet because a panel behind a picker went to sleep"
    );

    let _ = wedged.set_read_timeout(Some(BEFORE_LONG));

    assert_eq!(
        read_out(&mut wedged),
        Ok(Connection::Ended),
        "the program that was let go was left connected and hearing nothing, which it has no \
         way to notice and no way to recover from"
    );

    let _ = std::fs::remove_file(&at);

    Ok(())
}
