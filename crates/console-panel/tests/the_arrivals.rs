//! A card that is open when something lands in a folder she keeps things in
//! is asked for its rows again.
//!
//! Pressed through the real pool on a socket of the test's own, watching a
//! folder of the test's own: a file is written, and the card's rows are woken.

use std::time::Duration;

use console_events::{serving, sources};
use console_panel::{arrivals, frames};

const BEFORE_LONG: Duration = Duration::from_secs(5);

type Failure = Box<dyn std::error::Error>;

#[test]
fn a_song_landing_in_a_kept_folder_asks_the_open_card_for_its_rows_again() -> Result<(), Failure> {
    let socket = console_core_temporary_directories::fresh("panel-arrivals-socket")?;
    let at = socket.join("panel-arrivals.sock");
    let music = console_core_temporary_directories::fresh("panel-arrivals")?;

    let serving = at.clone();
    let Ok(()) = console_program_lifetime::threads::let_go(std::thread::spawn(move || {
        let _unserved = serving::serve(&serving, sources::hold);
    }));
    let Ok(patience) = console_waiting::Schedule::of(BEFORE_LONG);
    let Ok(_up) = console_waiting::until(patience, || {
        Ok(match at.exists() {
            true => console_waiting::Ready::Yes,
            false => console_waiting::Ready::NotYet,
        })
    });

    let Ok(()) = arrivals::follow_at(&at, std::slice::from_ref(&music));
    let Ok(_before) = frames::wake_state();

    let song = music.join("Africa [x].opus");
    let woken = console_waiting::until(patience, || {
        let written = console_core_atomic_writes::whole(&song, b"");
        let Ok(heard) = frames::wake_state();

        Ok(match (written, heard.rows) {
            (Ok(()), frames::FrameReceived::Yes) => console_waiting::Ready::Yes,
            (Ok(()), frames::FrameReceived::No) => console_waiting::Ready::NotYet,
            (Err(_unwritten), _) => console_waiting::Ready::NotYet,
        })
    });
    let woken = match woken {
        Ok(console_waiting::Outcome::Happened) => frames::FrameReceived::Yes,
        Ok(console_waiting::Outcome::RanOut) => frames::FrameReceived::No,
    };

    assert_eq!(woken, frames::FrameReceived::Yes, "the rows were never asked for again");

    let _ = std::fs::remove_dir_all(&music);
    let _ = std::fs::remove_file(&at);

    Ok(())
}
