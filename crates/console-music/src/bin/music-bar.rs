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

use std::io::Write;
use std::process::ExitCode;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

use console_music::player;
use console_never::Never;
use console_panel::door::{Up, is_open, watching_layers};

const EVERY: Duration = Duration::from_secs(2);

const PANEL: &str = "music-panel";

const PAUSE: &str = "\u{f03e4}";

fn main() -> ExitCode {
    let Some(icon) = std::env::args().nth(1) else {
        eprintln!("usage: music-bar ICON");
        return ExitCode::FAILURE;
    };

    let mut last = String::new();

    let Ok(opening) = listening();

    loop {
        let Ok(said) = line(&icon);

        match said == last {
            true => {},
            false => {
                println!("{said}");
                let _ = std::io::stdout().flush();
                last = said;
            }
        }

        match opening.recv_timeout(EVERY) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => (),
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(EVERY),
        }
    }
}

fn listening() -> Result<std::sync::mpsc::Receiver<()>, Never> {
    let (say, heard) = channel();

    match watching_layers(say) {
        Ok(()) => {},
        Err(fault) => eprintln!("music-bar: nothing will say when the panel opens: {fault}"),
    }

    Ok(heard)
}

fn line(icon: &str) -> Result<String, Never> {
    let asked = player::playing()?;

    let playing = asked.unwrap_or_default();
    let (mark, class) = match (playing.stopped, playing.paused) {
        (true, _) => (icon.to_string(), "stopped"),
        (_, true) => (PAUSE.to_string(), "paused"),
        _ => (icon.to_string(), "playing"),
    };
    let lit = match is_open(PANEL) {
        Ok(Up::OnScreen) => Some("open"),
        Ok(Up::NotThere) => None,
        Err(fault) => {
            eprintln!("music-bar: {fault}");
            None
        }
    };
    let worn: Vec<&str> = std::iter::once(class).chain(lit).collect();
    let quoted = quoted(&mark)?;

    Ok(format!(r#"{{"text": {}, "class": {}}}"#, quoted, serde_json::Value::from(worn)))
}

fn quoted(said: &str) -> Result<String, Never> {
    Ok(serde_json::Value::String(said.to_string()).to_string())
}
