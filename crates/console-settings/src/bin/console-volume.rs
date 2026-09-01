//! The volume rocker on the top edge.
//!
//!     console-volume up | down | mute
//!
//! What it does is `console_settings::rocker`, which can be asked without a
//! sound server. This is the part that needs one, and the notice afterwards.
//!
//! One notice, replaced. Held down, the rocker steps five per cent at a time
//! and every step would otherwise be another card, so the number the last one
//! came back under is kept and handed to `--replace-id`.

use std::process::Command;

use console_notices::saying::{Kept, Notice, raise_kept};
use console_settings::rocker::{self, Press};

/// pactl, asked something, and what it said.
fn pactl(argv: &[String]) -> String {
    let Ok(said) = Command::new("pactl").args(argv).output() else {
        return String::new();
    };
    String::from_utf8_lossy(&said.stdout).to_string()
}

/// Say where it has got to, where anything can be told.
///
/// Never the reason the rocker fails. The volume has already moved by the time
/// this runs, and a notification that would not draw is not worth handing that
/// back over.
fn said() {
    let level = pactl(&["get-sink-volume".to_string(), rocker::SINK.to_string()]);
    let muted = rocker::muted(&pactl(&["get-sink-mute".to_string(), rocker::SINK.to_string()]));
    let level = rocker::level(&level);

    let mut notice = Notice::new(&rocker::said(level, muted), "").lasting(1500);
    // The value hint is what mako fills the card with, in the pink
    // `progress-color` the theme writes. So the card is the bar, and the
    // sentence on it is the figure -- the two halves of the same reading.
    if let Some(value) = rocker::value(level) {
        notice = notice.valued(value);
    }
    raise_kept(notice, &Kept::named("volume"));
}

fn main() -> std::process::ExitCode {
    let Some(press) = std::env::args().nth(1).as_deref().and_then(Press::named) else {
        eprintln!("usage: console-volume [up|down|mute]");
        return std::process::ExitCode::from(2);
    };

    for argv in rocker::asks(press) {
        pactl(&argv);
    }
    said();
    std::process::ExitCode::SUCCESS
}
