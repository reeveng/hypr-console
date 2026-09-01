//! Screen brightness, in steps, within the range this panel can actually show.
//!
//!     console-brightness up | down | get
//!     console-brightness dim | undim
//!
//! `dim` and `undim` are the pair the idle daemon runs, and they are here
//! rather than in its configuration because putting a screen back where it was
//! means having remembered where that was. A `brightnessctl -s` in a config
//! file would remember it in a place nothing else on this machine can read,
//! and would restore over the top of somebody who reached for the rocker while
//! it was dim.
//!
//! `get` is the same range read the other way round, in points of a hundred, so
//! the settings panel can draw a bar of it. Nothing else may work out what full
//! is: a second opinion about this screen is two numbers that part company the
//! day either of them moves. `console_settings::screen` is that one opinion,
//! and the panel reads it there rather than running this.
//!
//! `up` and `down` say where they got to, the way the volume rocker does. A
//! press under L2 happens with a game in front of it and the settings panel
//! shut, so without a notice the only report of it is the screen itself --
//! which is the one thing somebody adjusting the screen cannot judge, because
//! it is what their eyes have just adapted to. `dim` and `undim` say nothing:
//! nobody pressed them, and a machine that woke you to tell you it had dimmed
//! itself would be worse than one that did it quietly.

use console_notices::saying::{Kept, Notice, raise_kept};
use console_settings::screen::{
    self, DIMMED, Way, as_points, now, remembered, set, stepped, undimming,
};

fn main() -> std::process::ExitCode {
    let word = std::env::args().nth(1).unwrap_or_default();

    let Some(now) = now() else {
        eprintln!("console-brightness: no backlight at {}", screen::DEVICE);
        return std::process::ExitCode::FAILURE;
    };

    if word == "get" {
        println!("{}", as_points(now));
        return std::process::ExitCode::SUCCESS;
    }

    if word == "dim" || word == "undim" {
        return match word.as_str() {
            "dim" => dim(now),
            _ => undim(now),
        };
    }

    let Some(way) = Way::named(&word) else {
        eprintln!("usage: console-brightness [up|down|get|dim|undim]");
        return std::process::ExitCode::from(2);
    };

    let going = stepped(now, way);
    if !set(going) {
        eprintln!("console-brightness: the screen would not take it");
        return std::process::ExitCode::FAILURE;
    }
    said(going);
    std::process::ExitCode::SUCCESS
}

/// Say where it has got to, where anything can be told.
///
/// One notice, replaced, the same as the rocker's: held down, left under L2
/// steps every repeat and every step would otherwise be another card, so the
/// number the last one came back under is kept and handed to `--replace-id`.
///
/// A press at either end says the level it is already at rather than nothing.
/// That is the answer to the question the press was asking -- the screen is as
/// bright as it goes -- and silence there would read as a button that had
/// stopped working.
fn said(going: i64) {
    let points = as_points(going);
    let notice = Notice::new(&screen::said(points), "").lasting(1500).valued(points);
    raise_kept(notice, &Kept::named("brightness"));
}

/// Take the screen down, and write down where it was.
///
/// A second dim changes nothing. The idle daemon fires each listener once, but
/// a machine that dimmed twice and remembered the second reading would restore
/// to the dim it had already applied, and the screen would never come back.
fn dim(now: i64) -> std::process::ExitCode {
    let Some(kept) = remembered() else {
        eprintln!("console-brightness: no XDG_RUNTIME_DIR, so nothing could be remembered");
        return std::process::ExitCode::FAILURE;
    };
    if kept.exists() {
        return std::process::ExitCode::SUCCESS;
    }
    if std::fs::write(&kept, format!("{now}\n")).is_err() {
        eprintln!("console-brightness: could not write {}", kept.display());
        return std::process::ExitCode::FAILURE;
    }
    match set(DIMMED) {
        true => std::process::ExitCode::SUCCESS,
        false => std::process::ExitCode::FAILURE,
    }
}

/// Put it back, unless a hand has been on it since.
///
/// The note is taken away either way. Leaving it would mean the next dim found
/// a machine that thinks it is already dim, and a screen at full brightness
/// that nothing will ever take down again.
fn undim(now: i64) -> std::process::ExitCode {
    let Some(kept) = remembered() else { return std::process::ExitCode::SUCCESS };
    let was = std::fs::read_to_string(&kept).ok().and_then(|held| held.trim().parse().ok());
    let _ = std::fs::remove_file(&kept);
    match was.and_then(|was| undimming(now, was)) {
        Some(back) => match set(back) {
            true => std::process::ExitCode::SUCCESS,
            false => std::process::ExitCode::FAILURE,
        },
        None => std::process::ExitCode::SUCCESS,
    }
}
