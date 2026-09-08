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
//!
//! It asked the player twice every two seconds for as long as the desktop was
//! up, which is what a bar module does when nothing tells it. `Topic::Player`
//! tells it: the player says `PropertiesChanged` for the pair this draws from,
//! the pool holds one monitor over the MPRIS path, and `player` says which of
//! those lines is a change rather than this program's own asking coming back.
//! What is left is a net under it rather than a cadence, so a machine where the
//! player is not running or the pool is down redraws now and then instead of
//! never.

use std::io::Write;
use std::process::ExitCode;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

use console_music_panel::player;
use console_core_never::Never;
use console_program_contract::Topic;
use console_panel::door::{Up, is_open};

const EVERY: Duration = Duration::from_secs(10);

const PANEL: &str = "music-panel";

const PAUSE: &str = "\u{f03e4}";

fn main() -> ExitCode {
    let icon = match std::env::args().nth(1) {
        Some(icon) => icon,
        None => {
            eprintln!("usage: music-bar ICON");
            return ExitCode::FAILURE;
        }
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
            #[cfg_attr(
                dylint_lib = "explicit021_no_sleeping",
                allow(
                    explicit021_no_sleeping,
                    reason = "nothing is left to say when the panel opens, so there is no longer a thing to wait for; the bar falls back to redrawing on a cadence rather than spinning on a dead channel"
                )
            )]
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(EVERY),
        }
    }
}

fn listening() -> Result<std::sync::mpsc::Receiver<()>, Never> {
    let (say, heard) = channel();

    let Ok(()) = console_events::again::layers(say.clone());
    let Ok(()) = console_events::again::about(&Topic::Player, player::worth_asking_after, say);

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
