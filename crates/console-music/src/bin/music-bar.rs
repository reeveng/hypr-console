//! What the bar says about the music.
//!
//!     music-bar 󰝚
//!
//! One line of JSON whenever the answer changes, which is what a waybar custom
//! module reads. The icon is always there, playing or not: it is the way into
//! the Music panel with a finger, and a control that disappears when nothing is
//! playing is a control nobody can find to start anything.
//!
//! It is lit while the panel is up, exactly as the menu and the keyboard are,
//! so the icon says whether a tap opens the music or puts it away.
//!
//! waybar's own mpris module is the other way to do this, and it is the reason
//! this exists: it draws nothing at all while no player is running.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::ExitCode;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

use console_music::player;
use console_panel::door::{events, is_open, worth_asking_after};

/// How often the player is asked.
///
/// It is asked rather than listened to, because the answer is two D-Bus
/// properties and the alternative is holding a signal subscription open for the
/// life of the session. Two seconds is under what anybody notices between a
/// song changing and the bar saying so.
const EVERY: Duration = Duration::from_secs(2);

/// What the panel calls its own surface, which is the program's name.
const PANEL: &str = "music-panel";

const PAUSE: &str = "\u{f03e4}";

fn main() -> ExitCode {
    let Some(icon) = std::env::args().nth(1) else {
        eprintln!("usage: music-bar ICON");
        return ExitCode::FAILURE;
    };
    let mut last = String::new();
    let opening = listening();

    loop {
        let said = line(&icon);

        if said != last {
            println!("{said}");
            let _ = std::io::stdout().flush();
            last = said;
        }
        // Woken by the panel opening or closing, so the icon lights the moment
        // it does rather than at the end of the next two seconds.
        match opening.recv_timeout(EVERY) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => (),
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(EVERY),
        }
    }
}

/// A word from the compositor whenever a layer opens or closes.
fn listening() -> std::sync::mpsc::Receiver<()> {
    let (say, heard) = channel();
    std::thread::spawn(move || {
        let Some(socket) = events() else { return };
        let Ok(stream) = UnixStream::connect(&socket) else { return };
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if worth_asking_after(&line) && say.send(()).is_err() {
                return;
            }
        }
    });
    heard
}

/// What the bar is told, as waybar reads it.
///
/// The classes are a list, never one string with a space in it. waybar hands a
/// string to GTK as a single class name and a class name cannot hold a space,
/// so `"stopped open"` was one class called `stopped open` and the stylesheet
/// had no rule for it. This icon always says what the music is doing, so it
/// always carried a word already, and it was therefore the one reading on the
/// bar that could never light at all while its own panel was in front.
fn line(icon: &str) -> String {
    let playing = player::playing().unwrap_or_default();
    let (mark, class) = match (playing.stopped, playing.paused) {
        (true, _) => (icon.to_string(), "stopped"),
        (_, true) => (format!("{PAUSE} {}", playing.title), "paused"),
        _ => (format!("{icon} {}", playing.title), "playing"),
    };
    let worn: Vec<&str> = std::iter::once(class).chain(is_open(PANEL).then_some("open")).collect();
    format!(r#"{{"text": {}, "class": {}}}"#, quoted(&mark), serde_json::Value::from(worn))
}

/// A string, as JSON holds it.
fn quoted(said: &str) -> String {
    serde_json::Value::String(said.to_string()).to_string()
}
