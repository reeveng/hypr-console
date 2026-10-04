//! The volume rocker on the top edge.
//!
//!     console-volume up | down | mute
//!
//! What it does is `console_settings::rocker`, which can be asked without a
//! sound server. This is the part that needs one, and the notification afterwards.
//!
//! One notification, replaced. Held down, the rocker steps five per cent at a time
//! and every step would otherwise be another card, so the number the last one
//! came back under is kept and handed to `--replace-id`.
//!
//! Every step that can be heard is heard, at a pitch that climbs with the level,
//! so the rocker says how loud the machine is now at the loudness it now is. A
//! machine left silent plays nothing, because nothing would come out, and
//! neither does one whose sound effects were never turned on.


use console_core_arguments::{Operands, read_with};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_notifications::saying::{StatePath, Notification, Content, raise_kept};
use console_settings::level::Muted;
use console_settings::rocker::{self, ButtonPress};
use console_sound_effects::catalogue::Sound;
use console_sound_effects::{Degree, SoundEffects};

fn pactl(arguments: &[String]) -> Result<String, Never> {
    let mut asking = Program::Pactl.command()?;

    let said = match asking.args(arguments).output() {
        Ok(message) => message,
        Err(_would_not_start) => return Ok(String::new()),
    };

    Ok(String::from_utf8_lossy(&said.stdout).to_string())
}

fn show_volume() -> Result<(), Never> {
    let level = pactl(&["get-sink-volume".to_string(), rocker::SINK.to_string()])?;
    let mute = pactl(&["get-sink-mute".to_string(), rocker::SINK.to_string()])?;
    let muted = rocker::muted(&mute)?;
    let level = rocker::level(&level)?;
    let words = rocker::volume_label(level, muted)?;

    let notification = Notification::new(Content { summary: &words, body: "" })?;
    let mut notification = notification.lasting(1500)?;

    let value = rocker::value(level)?;

    match value {
        Some(value) => {
            let valued = notification.valued(value)?;

            notification = valued;
        },
        None => {},
    }

    let kept = StatePath::new("volume")?;
    let Ok(()) = raise_kept(notification, &kept);

    let Ok(()) = play_feedback(muted, value);

    Ok(())
}

fn play_feedback(muted: Muted, value: Option<i64>) -> Result<(), Never> {
    let Ok(effects) = SoundEffects::current();

    let percent = match (effects, muted, value) {
        (SoundEffects::On, Muted::No, Some(value)) => value,
        (SoundEffects::Off, _, _) | (SoundEffects::On, Muted::Yes, _) | (SoundEffects::On, Muted::No, None) => {
            return Ok(());
        }
    };

    let Ok(percent) = fitted::<i64, u32>(percent);
    let Ok(root) = Degree::at_level(percent);

    let Ok(cue) = Sound::Volume.cue();

    match console_sound_effects::play(&cue, root) {
        Ok(()) => {},
        Err(fault) => eprintln!("console-volume: {fault}"),
    }

    Ok(())
}

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
    name: "console-volume",
    about: "the volume rocker on the top edge",
    flags: &[],
    operands: Operands::None,
};

fn main() -> std::process::ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let read = read_with::<ButtonPress, String>(&COMMAND, &words).and_then(|line| line.require_subcommand());

    let press = match read {
        Ok(press) => press,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return std::process::ExitCode::from(code);
        }
    };

    let Ok(asks) = rocker::asks(press);

    for arguments in asks {
        let Ok(_) = pactl(&arguments);
    }

    let Ok(()) = show_volume();

    std::process::ExitCode::SUCCESS
}
