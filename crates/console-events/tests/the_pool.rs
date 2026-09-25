//! The pool, pressed through a real socket rather than read about.
//!
//! `pool`'s own tests ask the arithmetic. What they cannot ask is whether a
//! program that connects, asks for a topic and waits actually hears anything,
//! because that is two threads, a socket and a wire between them -- which is
//! exactly where the twenty-five orphaned subscriptions lived. So the source
//! here is one a test can hand words to, and everything else is the real
//! thing: the real serving loop, the real wire, the real client.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use console_events::subscription::{Received, connect_at};
use console_events::serving;
use console_events::sources;
use console_events::wire::{self, Message};
use console_events::sources::Subscribed;
use console_program_contract::{Change, Topic};

const BEFORE_LONG: Duration = Duration::from_secs(5);

static SAYING: OnceLock<Sender<Sender<Change>>> = OnceLock::new();

fn source(topic: &Topic, say: Sender<Change>) -> Result<Subscribed, console_core_never::Never> {
    console_events::sources::handed_to(SAYING.get(), &Topic::Sound, topic, say)
}

fn socket() -> PathBuf {
    std::env::temp_dir().join(format!("console-events-test-{}.sock", std::process::id()))
}

fn before_long(heard: &Receiver<Received>) -> Option<Change> {
    loop {
        match heard.recv_timeout(BEFORE_LONG) {
            Ok(Received::Event(change)) => return Some(change),
            Ok(Received::Connected) => {},
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

    let Ok(early) = connect_at(&at, &[Topic::Sound]);
    let Ok(early) = early.received();
    let saying = handed.recv_timeout(BEFORE_LONG).expect("the source was never opened");

    saying
        .send(Change { topic: Topic::Sound, text: "sink 1 at 40%".to_string() })
        .expect("the pool stopped listening to its own source");

    assert_eq!(
        before_long(early),
        Some(Change { topic: Topic::Sound, text: "sink 1 at 40%".to_string() }),
        "a program that asked for a topic was told nothing when the machine said something"
    );

    let Ok(late) = connect_at(&at, &[Topic::Sound]);
    let Ok(late) = late.received();

    assert_eq!(
        before_long(late),
        Some(Change { topic: Topic::Sound, text: "sink 1 at 40%".to_string() }),
        "a program that opened between two changes was told nothing until the next one"
    );

    assert!(
        handed.recv_timeout(Duration::from_millis(200)).is_err(),
        "the pool opened a second subscription for the second program, which is the fault it exists to stop"
    );

    let _ = std::fs::remove_file(&at);
}

#[test]
fn what_lands_anywhere_under_a_watched_folder_is_heard_and_a_program_cannot_speak_for_the_machine() {
    let at = std::env::temp_dir().join(format!("console-events-folders-{}.sock", std::process::id()));
    let books = std::env::temp_dir().join(format!("console-events-books-{}", std::process::id()));
    std::fs::create_dir_all(&books).expect("a Books folder");

    let serving = at.clone();
    let _ = std::thread::spawn(move || serving::serve(&serving, sources::hold));

    up(&at);

    let watched = Topic::Path(books.clone());
    let Ok(listening) = connect_at(&at, &[watched.clone(), Topic::Units]);
    let Ok(heard) = listening.received();

    loop {
        match heard.recv_timeout(BEFORE_LONG) {
            Ok(Received::Connected) => break,
            Ok(Received::Event(_)) => {},
            Err(_) => panic!("never got in"),
        }
    }

    let mut telling = UnixStream::connect(&at).expect("the pool let a program in");
    let lying = Change { topic: Topic::Units, text: "everything stopped".to_string() };
    let Ok(spelled) = wire::encoded(&Message::Publish(lying));
    telling.write_all(format!("{spelled}\n").as_bytes()).expect("the pool took the line");

    let began = Instant::now();
    let knock = books.join("knock");

    while began.elapsed() < BEFORE_LONG {
        std::fs::write(&knock, b"").expect("a file on disk");

        match heard.recv_timeout(Duration::from_millis(200)) {
            Ok(Received::Event(change)) => match change.topic == watched {
                true => break,
                false => panic!("somebody spoke for the machine: {change:?}"),
            },
            Ok(Received::Connected) | Err(_) => {},
        }
    }

    while heard.recv_timeout(Duration::from_millis(300)).is_ok() {}

    let shelf = books.join("Stoics");
    let book = shelf.join("Meditations [2680].epub");

    std::fs::create_dir_all(&shelf).expect("a shelf made after the watch began");
    assert_eq!(before_long(heard).map(|change| change.text), Some(shelf.display().to_string()), "a new folder went unheard");

    std::fs::write(&book, b"").expect("a book on disk");
    assert_eq!(
        before_long(heard),
        Some(Change { topic: watched.clone(), text: book.display().to_string() }),
        "a book written into a folder made after the watch began went unheard"
    );

    std::fs::remove_file(&book).expect("the book thrown away");
    assert_eq!(
        before_long(heard),
        Some(Change { topic: watched, text: book.display().to_string() }),
        "a book thrown away went unheard"
    );

    let _ = std::fs::remove_dir_all(&books);
    let _ = std::fs::remove_file(&at);
}
