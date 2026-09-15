//! One program that stops reading must not deafen the others.
//!
//! The pool tells everybody from one loop, and a write into a socket whose
//! reader has gone to sleep blocks once the kernel's buffer for it is full. On
//! a desktop that is not a slow subscriber, it is every subscriber: the volume
//! stops moving on the bar because a panel behind a chooser stopped reading.
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

use console_events::listening::{Heard, listen_at};
use console_events::serving;
use console_events::sources::Held;
use console_events::wire::{self, Says};
use console_program_contract::{Changed, Topic};

const BEFORE_LONG: Duration = Duration::from_secs(5);

const WORDS: usize = 3_000;

const LONG: usize = 4096;

const LAST: &str = "the last word";

static SAYING: OnceLock<Sender<Sender<Changed>>> = OnceLock::new();

fn source(topic: &Topic, say: Sender<Changed>) -> Result<Held, console_core_never::Never> {
    Ok(match topic {
        Topic::Sound => match SAYING.get() {
            Some(handing) => match handing.send(say) {
                Ok(()) => Held::Yes,
                Err(_) => Held::Nothing,
            },
            None => Held::Nothing,
        },
        Topic::Compositor
        | Topic::Network
        | Topic::Notices
        | Topic::Units
        | Topic::Player
        | Topic::Path(_) => Held::Nothing,
    })
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

fn about(said: &str) -> Changed {
    Changed { about: Topic::Sound, said: said.to_string() }
}

fn heard_the_last_word(heard: &Receiver<Heard>) -> bool {
    loop {
        match heard.recv_timeout(BEFORE_LONG) {
            Ok(Heard::Said(changed)) => match changed.said == LAST {
                true => return true,
                false => {},
            },
            Ok(Heard::GotIn) => {},
            Err(_nothing_more_is_coming) => return false,
        }
    }
}

fn deaf(at: &Path) -> UnixStream {
    let stream = UnixStream::connect(at).expect("the pool would not take a second program");
    let asked = wire::spelt(&Says::Listen(Topic::Sound)).expect("the wire");
    let mut asking = stream.try_clone().expect("the connection");

    writeln!(asking, "{asked}").expect("asking to listen");

    stream
}

#[test]
fn a_program_that_stopped_reading_does_not_stop_the_words_reaching_anybody_else() {
    let at = socket();
    let (handing, handed) = channel();
    let _ = SAYING.set(handing);

    let serving = at.clone();
    let _ = std::thread::spawn(move || serving::serve(&serving, source));

    up(&at);

    let mut wedged = deaf(&at);

    let listening = listen_at(&at, &[Topic::Sound]).expect("listening");
    let heard = listening.heard().expect("the words");
    let saying = handed.recv_timeout(BEFORE_LONG).expect("the source was never opened");

    assert_eq!(heard.recv_timeout(BEFORE_LONG), Ok(Heard::GotIn));

    let long = "a".repeat(LONG);

    for word in 0..WORDS {
        saying.send(about(&format!("{word} {long}"))).expect("the pool stopped listening");
    }

    saying.send(about(LAST)).expect("the pool stopped listening");

    assert!(
        heard_the_last_word(heard),
        "a program that had stopped reading held up every word to everybody else, so the bar \
         goes quiet because a panel behind a chooser went to sleep"
    );

    let _ = wedged.set_read_timeout(Some(BEFORE_LONG));

    let mut taken = [0; LONG];
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
