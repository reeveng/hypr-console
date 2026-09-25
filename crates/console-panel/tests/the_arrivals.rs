//! A card that is open when something lands in a folder she keeps things in
//! is asked for its rows again.
//!
//! Pressed through the real pool on a socket of the test's own, watching a
//! folder of the test's own: a file is written, and the card's rows are woken.

use std::time::{Duration, Instant};

use console_events::{serving, sources};
use console_panel::{arrivals, frames};

const BEFORE_LONG: Duration = Duration::from_secs(5);

#[test]
fn a_song_landing_in_a_kept_folder_asks_the_open_card_for_its_rows_again() {
    let at = std::env::temp_dir().join(format!("console-panel-arrivals-{}.sock", std::process::id()));
    let music = std::env::temp_dir().join(format!("console-panel-arrivals-{}", std::process::id()));
    std::fs::create_dir_all(&music).expect("a Music folder");

    let serving = at.clone();
    let _ = std::thread::spawn(move || serving::serve(&serving, sources::hold));
    let began = Instant::now();

    while !at.exists() && began.elapsed() < BEFORE_LONG {
        std::thread::sleep(Duration::from_millis(5));
    }

    let Ok(()) = arrivals::follow_at(&at, std::slice::from_ref(&music));
    let _ = frames::woken();

    let mut woken = frames::FrameReceived::No;

    while woken == frames::FrameReceived::No && began.elapsed() < BEFORE_LONG {
        std::fs::write(music.join("Africa [x].opus"), b"").expect("a song on disk");
        std::thread::sleep(Duration::from_millis(50));
        woken = frames::woken().expect("woken").rows;
    }

    assert_eq!(woken, frames::FrameReceived::Yes, "the rows were never asked for again");

    let _ = std::fs::remove_dir_all(&music);
    let _ = std::fs::remove_file(&at);
}
