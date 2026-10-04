//! A sound from the catalogue, for anything that is not a Rust program.
//!
//!     console-sound-effect confirm
//!     console-sound-effect --list
//!
//! A binding, a script or a Lua file names the sound by its word. The switch on
//! the Sound tab is asked here as it is everywhere else, so a machine with
//! sound effects off stays quiet whoever asked.

use std::process::ExitCode;

use console_core_arguments::{Command, Flag, Operands, Presence, Takes, read};
use console_sound_effects::catalogue::{Sound, EVERY};
use console_sound_effects::{Degree, SoundEffects};

const LIST: Flag = Flag { spelling: "--list", takes: Takes::None, about: "the word for every sound there is, one to a line" };

const COMMAND: Command = Command {
    name: "console-sound-effect",
    about: "play SOUND from the catalogue, unless sound effects are off on the Sound tab",
    flags: &[LIST],
    operands: Operands::Optional("SOUND"),
};

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();

    let line = match read(&COMMAND, &words) {
        Ok(line) => line,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return ExitCode::from(code);
        }
    };

    let Ok(listing) = line.presence(LIST);

    match listing {
        Presence::Present => {
            for sound in EVERY {
                let Ok(word) = sound.word();

                println!("{word}");
            }

            return ExitCode::SUCCESS;
        }
        Presence::Absent => {},
    }

    let word = match line.exactly(["SOUND"]) {
        Ok([word]) => word,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return ExitCode::from(code);
        }
    };

    let Ok(named) = Sound::from_word(word);

    let sound = match named {
        Some(sound) => sound,
        None => {
            eprintln!("console-sound-effect: no sound called {word}; --list says which there are");

            return ExitCode::from(2);
        }
    };

    let Ok(chosen) = SoundEffects::current();

    match chosen {
        SoundEffects::On => {},
        SoundEffects::Off => {
            eprintln!("console-sound-effect: sound effects are off on the Sound tab");

            return ExitCode::SUCCESS;
        }
    }

    let Ok(cue) = sound.cue();

    match console_sound_effects::play(&cue, Degree(0)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("console-sound-effect: {fault}");

            ExitCode::FAILURE
        }
    }
}
