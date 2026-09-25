//! A sound from the catalogue, for anything that is not a Rust program.
//!
//!     console-sound-effect confirm
//!     console-sound-effect --list
//!
//! A binding, a script or a Lua file names the sound by its word. The switch on
//! the Sound tab is asked here as it is everywhere else, so a machine with
//! sound effects off stays quiet whoever asked.

use console_sound_effects::catalogue::{Sound, EVERY};
use console_sound_effects::{Degree, SoundEffects};

fn main() -> std::process::ExitCode {
    let asked = std::env::args().nth(1);

    let word = match asked.as_deref() {
        Some("--list") => {
            for sound in EVERY {
                let Ok(word) = sound.word();

                println!("{word}");
            }

            return std::process::ExitCode::SUCCESS;
        }
        Some(word) => word,
        None => {
            eprintln!("usage: console-sound-effect <sound> | --list");

            return std::process::ExitCode::from(2);
        }
    };

    let Ok(named) = Sound::named(word);

    let sound = match named {
        Some(sound) => sound,
        None => {
            eprintln!("console-sound-effect: no sound called {word}; --list says which there are");

            return std::process::ExitCode::from(2);
        }
    };

    let Ok(chosen) = SoundEffects::chosen();

    match chosen {
        SoundEffects::On => {},
        SoundEffects::Off => {
            eprintln!("console-sound-effect: sound effects are off on the Sound tab");

            return std::process::ExitCode::SUCCESS;
        }
    }

    let Ok(cue) = sound.cue();

    match console_sound_effects::play(&cue, Degree(0)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("console-sound-effect: {fault}");

            std::process::ExitCode::FAILURE
        }
    }
}
