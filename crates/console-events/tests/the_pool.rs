//! The pool, pressed through a real socket rather than read about.
//!
//! `pool`'s own tests ask the arithmetic. What they cannot ask is whether a
//! program that connects, asks for a topic and waits actually hears anything,
//! because that is two threads, a socket and a wire between them -- which is
//! exactly where the twenty-five orphaned subscriptions lived. So the source
//! here is one a test can hand words to, and everything else is the real
//! thing: the real serving loop, the real wire, the real client.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use console_events::listening::{Heard, listen_at};
use console_events::serving;
use console_events::sources::Held;
use console_program_contract::{Changed, Topic};

const BEFORE_LONG: Duration = Duration::from_secs(5);

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
    std::env::temp_dir().join(format!("console-events-test-{}.sock", std::process::id()))
}

fn before_long(heard: &Receiver<Heard>) -> Option<Changed> {
    loop {
        match heard.recv_timeout(BEFORE_LONG) {
            Ok(Heard::Said(changed)) => return Some(changed),
            Ok(Heard::GotIn) => {},
            Err(_) => return None,
        }
    }
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

#[test]
fn a_program_hears_what_the_machine_said_and_whoever_comes_late_hears_it_first() {
    let at = socket();
    let (handing, handed) = std::sync::mpsc::channel();
    let _ = SAYING.set(handing);

    let serving = at.clone();
    let _ = std::thread::spawn(move || serving::serve(&serving, source));

    up(&at);

    let Ok(early) = listen_at(&at, &[Topic::Sound]);
    let Ok(early) = early.heard();
    let saying = handed.recv_timeout(BEFORE_LONG).expect("the source was never opened");

    saying
        .send(Changed { about: Topic::Sound, said: "sink 1 at 40%".to_string() })
        .expect("the pool stopped listening to its own source");

    assert_eq!(
        before_long(early),
        Some(Changed { about: Topic::Sound, said: "sink 1 at 40%".to_string() }),
        "a program that asked for a topic was told nothing when the machine said something"
    );

    let Ok(late) = listen_at(&at, &[Topic::Sound]);
    let Ok(late) = late.heard();

    assert_eq!(
        before_long(late),
        Some(Changed { about: Topic::Sound, said: "sink 1 at 40%".to_string() }),
        "a program that opened between two changes was told nothing until the next one"
    );

    assert!(
        handed.recv_timeout(Duration::from_millis(200)).is_err(),
        "the pool opened a second subscription for the second program, which is the fault it exists to stop"
    );

    let _ = std::fs::remove_file(&at);
}
