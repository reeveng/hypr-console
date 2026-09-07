//! Stopping being told, and starting again.
//!
//! A panel behind a chooser wants nothing until it is uncovered, and what it
//! wants the moment it is uncovered is *what is true now* rather than the next
//! thing to change. Those are the two halves of the same question and neither
//! can be asked of `pool` alone: the first is a line down a socket that the
//! serving loop has to act on, and the second is the replay arriving because
//! somebody subscribed again rather than because a source said anything. So
//! this is the real serving loop, the real wire and a source the test speaks
//! through, and the source is watched to prove it was opened once.
//!
//! Getting in is asserted here as well, in the one place its order can be
//! relied on: it comes before any word a source says. `bar-door` is the watch
//! with no tick underneath it, and that word is the whole of how it recovers
//! from a gap.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};

use console_events::listening::{Heard, Wanting, listen_at};
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
    std::env::temp_dir().join(format!("console-events-deafening-{}.sock", std::process::id()))
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

fn before_long(heard: &std::sync::mpsc::Receiver<Heard>) -> Option<Changed> {
    loop {
        match heard.recv_timeout(BEFORE_LONG) {
            Ok(Heard::Said(changed)) => return Some(changed),
            Ok(Heard::GotIn) => {},
            Err(_) => return None,
        }
    }
}

#[test]
fn a_program_that_asks_again_is_told_what_is_true_now_rather_than_waiting_for_a_change() {
    let at = socket();
    let (handing, handed) = channel();
    let _ = SAYING.set(handing);

    let serving = at.clone();
    let _ = std::thread::spawn(move || serving::serve(&serving, source));

    up(&at);

    let Ok(listening) = listen_at(&at, &[Topic::Sound]);
    let Ok(heard) = listening.heard();
    let saying = handed.recv_timeout(BEFORE_LONG).expect("the source was never opened");

    assert_eq!(
        heard.recv_timeout(BEFORE_LONG),
        Ok(Heard::GotIn),
        "getting in was not said, so a watch whose words mean *ask again* has no way to know \
         it was away"
    );

    saying.send(about("40%")).expect("the pool stopped listening to its own source");

    assert_eq!(before_long(heard), Some(about("40%")));

    let Ok(()) = listening.not(&Topic::Sound);
    let Ok(wanting) = listening.wanting();

    assert_eq!(wanting, Wanting::Nothing, "it still wants what it just gave up");

    let Ok(()) = listening.also(&Topic::Sound);

    assert_eq!(
        before_long(heard),
        Some(about("40%")),
        "a program that started listening again was told nothing until the next change, which \
         is a panel coming back with a reading it cannot have"
    );

    assert!(
        handed.recv_timeout(Duration::from_millis(200)).is_err(),
        "the pool opened the source a second time for a program that had never left it"
    );

    let _ = std::fs::remove_file(&at);
}
