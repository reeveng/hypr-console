//! Tell every browser on this machine what this desktop has decided: which
//! engine a question is asked of, which add-ons it is given, and the handful of
//! preferences that make a browser look like the rest of the machine.
//!
//! ```text
//! console-engine <key>   duckduckgo, startpage or wikipedia
//! console-engine         whatever she has already chosen
//! ```
//!
//! Told without a key it says again what is already true, which is what `console
//! apply` runs: until an engine had been chosen on the panel these files had
//! never been written at all, so a machine made from the manifest had browsers
//! no one had told anything and add-ons it was supposed to have installed.
//!
//! A program of its own because all three browsers read their policy out of
//! /etc, and both the panel that calls it and the person it belongs to are not
//! root.
//! The rule in /etc/sudoers.d/console lets her run this and nothing else, which
//! is a smaller thing to hand over than a shell.
//!
//! A browser that is not on the machine is passed over rather than failed on.
//! The point of having three is that two of them are usually not the one being
//! used, and a Wi-Fi panel does not fail because there is no Bluetooth.

use std::path::Path;

use console_core_arguments::{Command, Operands, Reason, ValidationError};
use console_default_applications::engines::{self, Engine};
use console_default_applications::policies::{self, CHROMIUM, FIREFOX, LIBREWOLF, Where};
use console_core_atomic_writes::Stored;
use console_core_external_programs::{Installed, installed};
use console_core_never::Never;

const COMMAND: Command = Command {
    name: "console-engine",
    about: "tell every browser here which engine to ask, the one already chosen when none is named",
    flags: &[],
    operands: Operands::Optional("ENGINE"),
};

fn told(words: &[String]) -> Result<&'static Engine, ValidationError> {
    let line = console_core_arguments::read(&COMMAND, words)?;
    let Ok(named) = line.operands();
    let Ok(chosen) = engines::current();
    let key = match named.first() {
        Some(key) => key.clone(),
        None => chosen,
    };
    let Ok(known) = engines::one(&key);

    match known {
        Some(engine) => Ok(engine),
        None => {
            let Ok(refusal) = line.refusal(Reason::InvalidValue { of: "ENGINE", value: key });

            Err(refusal)
        }
    }
}

fn main() -> std::process::ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let engine = match told(&words) {
        Ok(engine) => engine,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return std::process::ExitCode::from(code);
        }
    };

    for place in [&CHROMIUM, &FIREFOX, &LIBREWOLF] {
        let Ok(installed) = installed(place.program);

        match installed {
            Installed::Yes => {}
            Installed::No => continue,
        }

        let Ok(shipped) = read_shipped(place);

        let Ok(said) = match place.file == CHROMIUM.file {
            true => policies::chromium(engine),
            false => policies::mozilla(place, engine, &shipped),
        };

        match console_core_atomic_writes::whole_with_folders(Path::new(place.file), said.as_bytes()) {
            Ok(()) => println!("{}: {}", engine.says, place.file),
            Err(why) => eprintln!("{}: {why}", place.file),
        }
    }

    std::process::ExitCode::SUCCESS
}

fn read_shipped(place: &Where) -> Result<String, Never> {
    Ok(match place.beneath.is_empty() {
        true => String::new(),
        false => {
            let Ok(held) = console_core_atomic_writes::read(std::path::Path::new(place.beneath));

            match held {
                Stored::Text(said) => said,
                Stored::Absent => String::new(),
                Stored::Failed(fault) => {
                    eprintln!("console-engine: {}: {fault}", place.beneath);

                    String::new()
                }
            }
        },
    })
}

