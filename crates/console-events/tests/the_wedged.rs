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

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use console_events::subscription::{Received, connect_at};
use console_events::serving;
use console_events::sources::Subscribed;
use console_events::wire::{self, Message};
use console_program_contract::{Change, Topic};

const BEFORE_LONG: Duration = Duration::from_secs(5);

const WORDS: u32 = 3_000;

const LONG: u32 = 4096;

const LAST: &str = "the last word";

static SAYING: OnceLock<Sender<Sender<Change>>> = OnceLock::new();

fn source(topic: &Topic, say: Sender<Change>) -> Result<Subscribed, console_core_never::Never> {
    console_events::sources::handed_to(SAYING.get(), &Topic::Sound, topic, say)
}

fn socket() -> PathBuf {
    std::env::temp_dir().join(format!("console-events-wedged-{}.sock", std::process::id()))
}

fn up(at: &Path) {
    let began = Instant::now();

    while began.elapsed() < BEFORE_LONG {
        match at.exists() {
            true => return,
            false => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

fn change(text: &str) -> Change {
    Change { topic: Topic::Sound, text: text.to_string() }
}

fn heard_the_last_word(heard: &Receiver<Received>) -> bool {
    loop {
        match heard.recv_timeout(BEFORE_LONG) {
            Ok(Received::Event(change)) => match change.text == LAST {
                true => return true,
                false => {},
            },
            Ok(Received::Connected) => {},
            Err(_nothing_more_is_coming) => return false,
        }
    }
}

fn deaf(at: &Path) -> UnixStream {
    let stream = UnixStream::connect(at).expect("the pool would not take a second program");
    let asked = wire::encoded(&Message::Subscribe(Topic::Sound)).expect("the wire");
    let mut asking = stream.try_clone().expect("the connection");

    writeln!(asking, "{asked}").expect("asking to listen");

    stream
}

#[test]
fn a_program_that_stopped_reading_does_not_stop_the_words_reaching_anyone_else() {
    let at = socket();
    let (handing, handed) = channel();
    let _ = SAYING.set(handing);

    let serving = at.clone();
    let _ = std::thread::spawn(move || serving::serve(&serving, source));

    up(&at);

    let mut wedged = deaf(&at);

    let subscriber = connect_at(&at, &[Topic::Sound]).expect("listening");
    let heard = subscriber.received().expect("the words");
    let saying = handed.recv_timeout(BEFORE_LONG).expect("the source was never opened");

    assert_eq!(heard.recv_timeout(BEFORE_LONG), Ok(Received::Connected));

    let long = "a".repeat(LONG.try_into().unwrap());

    for word in 0..WORDS {
        saying.send(change(&format!("{word} {long}"))).expect("the pool stopped listening");
    }

    saying.send(change(LAST)).expect("the pool stopped listening");

    assert!(
        heard_the_last_word(heard),
        "a program that had stopped reading held up every word to everyone else, so the bar \
         goes quiet because a panel behind a picker went to sleep"
    );

    let _ = wedged.set_read_timeout(Some(BEFORE_LONG));

    let mut taken = vec![0; LONG.try_into().unwrap()];
    let mut ended = false;

    for _turn in 0..WORDS {
        match wedged.read(&mut taken) {
            Ok(0) => {
                ended = true;

                break;
            }
            Ok(_some_of_what_it_was_told) => {},
            Err(_it_is_still_open_and_saying_nothing) => break,
        }
    }

    assert!(
        ended,
        "the program that was let go was left connected and hearing nothing, which it has no \
         way to notice and no way to recover from"
    );

    let _ = std::fs::remove_file(&at);
}
