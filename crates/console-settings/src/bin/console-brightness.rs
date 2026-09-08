//! Screen brightness, in steps, within the range this panel can actually show.
//!
//!     console-brightness up | down | get
//!     console-brightness dim | undim
//!
//! `undim` also puts the panel back on, which is the machine's only way out of
//! a screen that has gone dark and stayed dark. See `panel_on`.
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

use console_core_never::Never;
use console_notifications::saying::{Kept, Notice, raise_kept};
use console_settings::screen::{
    self, DIMMED, Moved, Way, as_points, now, remembered, set, stepped, undimming,
};

fn main() -> std::process::ExitCode {
    let word = std::env::args().nth(1).unwrap_or_default();

    let Ok(reading) = now();

    let now = match reading {
        Some(now) => now,
        None => {
            eprintln!("console-brightness: no backlight at {}", screen::DEVICE);

            return std::process::ExitCode::FAILURE;
        }
    };

    match word == "get" {
        true => {
            let Ok(points) = as_points(now);

            println!("{points}");
            return std::process::ExitCode::SUCCESS;
        }
        false => {},
    }

    match word == "dim" || word == "undim" {
        true => {
            let Ok(done) = match word.as_str() {
                "dim" => dim(now),
                _ => undim(now),
            };

            return done;
        }
        false => {},
    }

    let Ok(named) = Way::named(&word);

    let way = match named {
        Some(way) => way,
        None => {
            eprintln!("usage: console-brightness [up|down|get|dim|undim]");

            return std::process::ExitCode::from(2);
        }
    };

    let Ok(going) = stepped(now, way);

    let Ok(moved) = set(going);

    match moved {
        Moved::No => {
            eprintln!("console-brightness: the screen would not take it");
            return std::process::ExitCode::FAILURE;
        }
        Moved::Yes => {},
    }

    let Ok(()) = said(going);

    std::process::ExitCode::SUCCESS
}

fn said(going: i64) -> Result<(), Never> {
    let points = as_points(going)?;
    let words = screen::said(points)?;
    let notice = Notice::new(&words, "")?;
    let notice = notice.lasting(1500)?;
    let notice = notice.valued(points)?;
    let kept = Kept::named("brightness")?;
    let Ok(()) = raise_kept(notice, &kept);

    Ok(())
}

fn dim(now: i64) -> Result<std::process::ExitCode, Never> {
    let remembered = remembered()?;

    let kept = match remembered {
        Some(kept) => kept,
        None => {
            eprintln!("console-brightness: no XDG_RUNTIME_DIR, so nothing could be remembered");

            return Ok(std::process::ExitCode::FAILURE);
        }
    };

    match kept.exists() {
        true => return Ok(std::process::ExitCode::SUCCESS),
        false => {},
    }

    match std::fs::write(&kept, format!("{now}\n")) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("console-brightness: could not write {}: {fault}", kept.display());

            return Ok(std::process::ExitCode::FAILURE);
        }
    }

    let moved = set(DIMMED)?;

    Ok(match moved {
        Moved::Yes => std::process::ExitCode::SUCCESS,
        Moved::No => std::process::ExitCode::FAILURE,
    })
}

fn kept_at(kept: &std::path::Path) -> Result<Option<i64>, Never> {
    let held = match std::fs::read_to_string(kept) {
        Ok(held) => held,
        Err(_) => return Ok(None),
    };

    let was = match held.trim().parse::<i64>() {
        Ok(was) => was,
        Err(_) => return Ok(None),
    };

    Ok(Some(was))
}

fn panel_on() -> Result<(), Never> {
    let done = console_compositor::told(
        console_compositor::Told::Dispatch,
        r#"hl.dsp.dpms({ action = "enable" })"#,
    )?;

    match done {
        console_compositor::Done::Taken => {},
        console_compositor::Done::Refused(why) => {
            eprintln!("console-brightness: the panel would not come on: {why}");
        }
    }

    Ok(())
}

fn undim(now: i64) -> Result<std::process::ExitCode, Never> {
    panel_on()?;

    let remembered = remembered()?;

    let kept = match remembered {
        Some(kept) => kept,
        None => return Ok(std::process::ExitCode::SUCCESS),
    };

    let was = kept_at(&kept)?;
    let _ = std::fs::remove_file(&kept);

    let back = match was {
        Some(was) => undimming(now, was)?,
        None => None,
    };

    match back {
        Some(back) => {
            let moved = set(back)?;

            Ok(match moved {
                Moved::Yes => std::process::ExitCode::SUCCESS,
                Moved::No => std::process::ExitCode::FAILURE,
            })
        },
        None => Ok(std::process::ExitCode::SUCCESS),
    }
}
