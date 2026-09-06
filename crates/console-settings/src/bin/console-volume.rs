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


use console_external_programs::Program;
use console_never::Never;
use console_notifications::saying::{Kept, Notice, raise_kept};
use console_settings::rocker::{self, Press};

fn pactl(argv: &[String]) -> Result<String, Never> {
    let mut asking = Program::Pactl.command()?;

    let Ok(said) = asking.args(argv).output() else {
        return Ok(String::new());
    };

    Ok(String::from_utf8_lossy(&said.stdout).to_string())
}

fn said() -> Result<(), Never> {
    let level = pactl(&["get-sink-volume".to_string(), rocker::SINK.to_string()])?;
    let mute = pactl(&["get-sink-mute".to_string(), rocker::SINK.to_string()])?;
    let muted = rocker::muted(&mute)?;
    let level = rocker::level(&level)?;
    let words = rocker::said(level, muted)?;

    let notice = Notice::new(&words, "")?;
    let mut notice = notice.lasting(1500)?;

    let value = rocker::value(level)?;

    match value {
        Some(value) => {
            let valued = notice.valued(value)?;

            notice = valued;
        },
        None => {},
    }

    let kept = Kept::named("volume")?;
    let Ok(()) = raise_kept(notice, &kept);

    Ok(())
}

fn main() -> std::process::ExitCode {
    let Ok(named) = std::env::args().nth(1).as_deref().map(Press::named).transpose();

    let Some(press) = named.flatten() else {
        eprintln!("usage: console-volume [up|down|mute]");
        return std::process::ExitCode::from(2);
    };

    let Ok(asks) = rocker::asks(press);

    for argv in asks {
        let Ok(_) = pactl(&argv);
    }

    let Ok(()) = said();

    std::process::ExitCode::SUCCESS
}
