//! The pool, pressed through a real socket rather than read about.
//!
//! `pool`'s own tests ask the arithmetic. What they cannot ask is whether a
//! program that connects, asks for an event group and waits actually hears
//! anything, because that is two threads, a socket and a wire between them --
//! which is exactly where the twenty-five orphaned subscriptions lived. So the
//! source here is one a test can hand words to, and everything else is the real
//! thing: the real serving loop, the real wire, the real client.

mod pool;

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use console_events::serving;
use console_events::sources;
use console_events::subscription::{Received, connect_at};
use console_events::wire::{self, Message};
use console_program_contract::{Change, EventGroup};
use console_program_lifetime::threads;
use console_waiting::{Outcome, Ready, Schedule, until_handed};
use pool::{Failure, BEFORE_LONG, before_long, change, serve_at, socket, up};

#[test]
fn a_program_hears_what_the_machine_said_and_whoever_comes_late_hears_it_first() -> Result<(), Failure> {
    let at = socket("test")?;
    let handed = serve_at(&at)?;

    let Ok(early) = connect_at(&at, &[EventGroup::Sound]);
    let Ok(early) = early.received();
    let saying = handed.recv_timeout(BEFORE_LONG).map_err(|_| "the source was never opened")?;
    let Ok(said) = change("sink 1 at 40%");

    saying.send(said.clone()).map_err(|_| "the pool stopped listening to its own source")?;

    assert_eq!(
        before_long(early),
        Ok(Some(said.clone())),
        "a program that asked for an event group was told nothing when the machine said something"
    );

    let Ok(late) = connect_at(&at, &[EventGroup::Sound]);
    let Ok(late) = late.received();

    assert_eq!(
        before_long(late),
        Ok(Some(said)),
        "a program that opened between two changes was told nothing until the next one"
    );

    let opened_again = handed.recv_timeout(Duration::from_millis(200));

    assert!(
        matches!(opened_again, Err(std::sync::mpsc::RecvTimeoutError::Timeout)),
        "the pool opened a second subscription for the second program, which is the fault it exists to stop"
    );

    let _ = std::fs::remove_file(&at);

    Ok(())
}

#[cfg_attr(
    dylint_lib = "explicit040_no_torn_write",
    allow(
        explicit040_no_torn_write,
        reason = "what is asked is what the pool hears when a file lands under a watched folder under its own name; a write staged beside it would land a second name first, and the test would hear that one"
    )
)]
#[test]
fn what_lands_anywhere_under_a_watched_folder_is_heard_and_a_program_cannot_speak_for_the_machine() -> Result<(), Failure> {
    let at = socket("folders")?;
    let books = console_core_temporary_directories::fresh("events-books")?;
    let serving = at.clone();
    let Ok(()) = threads::let_go(std::thread::spawn(move || {
        let _the_test_reads_whether_it_came_up = serving::serve(&serving, sources::hold);
    }));

    up(&at)?;

    let watched = EventGroup::Path(books.clone());
    let Ok(listening) = connect_at(&at, &[watched.clone(), EventGroup::Units]);
    let Ok(heard) = listening.received();

    let got_in = std::iter::repeat_with(|| heard.recv_timeout(BEFORE_LONG)).find_map(|received| match received {
        Ok(Received::Connected) => Some(Outcome::Happened),
        Ok(Received::Event(_)) => None,
        Err(_never_got_in) => Some(Outcome::RanOut),
    });

    match got_in {
        Some(Outcome::Happened) => {},
        Some(Outcome::RanOut) | None => return Err(Failure::from("never got in")),
    }

    let mut telling = UnixStream::connect(&at)?;
    let lying = Change { event_group: EventGroup::Units, text: "everything stopped".to_string() };
    let Ok(spelled) = wire::encoded(&Message::Publish(lying));

    telling.write_all(format!("{spelled}\n").as_bytes())?;

    let knock = books.join("knock");
    let mut seen: (Option<std::io::Error>, Option<Change>) = (None, None);
    let Ok(patience) = Schedule::asking_every(BEFORE_LONG, Duration::from_millis(1));
    let Ok(knocked) = until_handed(patience, &mut seen, |(unwritten, spoken_for)| {
        match std::fs::write(&knock, b"") {
            Ok(()) => {},
            Err(fault) => *unwritten = Some(fault),
        }

        Ok(match heard.recv_timeout(Duration::from_millis(200)) {
            Ok(Received::Event(change)) => match change.event_group == watched {
                true => Ready::Yes,
                false => {
                    *spoken_for = Some(change);

                    Ready::Yes
                }
            },
            Ok(Received::Connected) => Ready::NotYet,
            Err(_nothing_arrived_yet) => Ready::NotYet,
        })
    });
    let (unwritten, spoken_for) = seen;

    match (unwritten, spoken_for, knocked) {
        (Some(fault), _, _) => return Err(Failure::from(fault)),
        (None, Some(change), _) => return Err(Failure::from(format!("somebody spoke for the machine: {change:?}"))),
        (None, None, Outcome::RanOut) => return Err(Failure::from("a knock on the watched folder went unheard")),
        (None, None, Outcome::Happened) => {},
    }

    let _stirred_by_the_knock = std::iter::repeat_with(|| heard.recv_timeout(Duration::from_millis(300)))
        .map_while(|heard| match heard {
            Ok(stirred) => Some(stirred),
            Err(_quiet_again) => None,
        })
        .count();

    let shelf = books.join("Stoics");
    let book = shelf.join("Meditations [2680].epub");

    std::fs::create_dir_all(&shelf)?;

    assert_eq!(
        before_long(heard).map(|change| change.map(|change| change.text)),
        Ok(Some(shelf.display().to_string())),
        "a new folder went unheard"
    );

    std::fs::write(&book, b"")?;

    assert_eq!(
        before_long(heard),
        Ok(Some(Change { event_group: watched.clone(), text: book.display().to_string() })),
        "a book written into a folder made after the watch began went unheard"
    );

    std::fs::remove_file(&book)?;

    assert_eq!(
        before_long(heard),
        Ok(Some(Change { event_group: watched, text: book.display().to_string() })),
        "a book thrown away went unheard"
    );

    let _ = std::fs::remove_dir_all(&books);
    let _ = std::fs::remove_file(&at);

    Ok(())
}
