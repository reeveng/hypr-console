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
//! nobody had told anything and add-ons it was supposed to have installed.
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

use std::path::{Path, PathBuf};

use console_defaults::engines;
use console_defaults::policies::{self, CHROMIUM, FIREFOX, LIBREWOLF, Where};

fn main() -> std::process::ExitCode {
    let key = std::env::args().nth(1).unwrap_or_else(engines::chosen);
    let Some(engine) = engines::one(&key) else {
        eprintln!("{key}: not an engine this machine knows");
        return std::process::ExitCode::from(1);
    };

    let said = |place: &Where| match place.file == CHROMIUM.file {
        true => policies::chromium(engine),
        false => policies::mozilla(place, engine, &shipped(place)),
    };

    for place in [&CHROMIUM, &FIREFOX, &LIBREWOLF] {
        if !here(place.program) {
            continue;
        }
        match wrote(Path::new(place.file), &said(place)) {
            Ok(()) => println!("{}: {}", engine.says, place.file),
            Err(why) => eprintln!("{}: {why}", place.file),
        }
    }
    std::process::ExitCode::SUCCESS
}

/// What the browser ships, read at the moment of writing rather than kept.
///
/// An update to the browser is then carried into ours the next time an engine
/// is chosen, instead of ours going quietly out of date against it.
fn shipped(place: &Where) -> String {
    match place.beneath.is_empty() {
        true => String::new(),
        false => std::fs::read_to_string(place.beneath).unwrap_or_default(),
    }
}

/// Whether a program is on this machine.
fn here(program: &str) -> bool {
    let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/usr/local/bin".to_string());
    path.split(':').filter(|at| !at.is_empty()).any(|at| PathBuf::from(at).join(program).exists())
}

/// Write one policy, making the directory it goes in if the browser's package
/// did not.
fn wrote(at: &Path, said: &str) -> std::io::Result<()> {
    if let Some(parent) = at.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(at, said)
}
