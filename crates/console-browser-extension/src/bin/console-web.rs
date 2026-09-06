//! Pack the add-on this desktop puts in its browser, and put it in the profile
//! the browser will look through when it starts.
//!
//! ```text
//! console-web           pack it if anything has changed
//! console-web --always  pack it whether or not anything has changed
//! ```
//!
//! Run by `console apply`, in the same breath as the browsers are told which
//! engine to ask. Everything it touches is under her home -- it reads the
//! palette out of the profile and writes the add-on back into the same profile
//! -- so it is run as her and not as root.
//!
//! It does nothing at all on a machine where nothing has changed. A browser
//! installs an add-on by version and reinstalls it when the version goes up, so
//! packing the same files again under a new number every apply would be the
//! browser taking a new add-on every time the machine was told to catch up.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_browser_extension::{PALETTE, source, stamp};
use console_never::Never;

const NEW: &str = "console-new";

fn main() -> ExitCode {
    let always = std::env::args().any(|word| word == "--always");

    let home = match std::env::var("HOME") {
        Ok(said) => PathBuf::from(said),
        Err(_) => PathBuf::from("/root"),
    };

    let at = home.join(PALETTE);

    let Ok(palette) = std::fs::read_to_string(&at) else {
        eprintln!("{}: no palette to dress the add-on in", at.display());
        return ExitCode::from(1);
    };

    let Ok(dressed) = source::hosted(&palette);

    let Some(palette) = dressed else {
        eprintln!("{}: not a palette this can read", at.display());
        return ExitCode::from(1);
    };

    let Ok(xpi) = console_browser_extension::xpi(&home);
    let Ok(stamped) = console_browser_extension::stamp(&home);
    let Ok(hash) = source::hash(&palette);
    let Ok(held) = note_beside(&stamped);

    let packed_already = !always
        && xpi.is_file()
        && held.as_ref().is_some_and(|held| held.hash == hash);

    match packed_already {
        true => {
            println!("{}: already packed", xpi.display());
            return ExitCode::SUCCESS;
        }
        false => {}
    }

    let was = held.map(|held| held.version).or_else(|| {
        let Ok(packed) = packed_version(&xpi);

        packed
    });

    let Ok(version) = stamp::next(was.as_deref());
    let Ok(files) = source::every(&version, &palette);
    let Ok(made) = console_browser_extension::pack::zip(&files);

    match wrote(&xpi, &made) {
        Ok(()) => {}
        Err(why) => {
            eprintln!("{}: {why}", xpi.display());
            return ExitCode::from(1);
        }
    }

    let Ok(note) = stamp::written(&stamp::Stamp { hash, version: version.clone() });

    match wrote(&stamped, note.as_bytes()) {
        Ok(()) => {}
        Err(why) => {
            eprintln!("{}: {why}", stamped.display());
            return ExitCode::from(1);
        }
    }

    println!("{}: packed, version {version}", xpi.display());
    ExitCode::SUCCESS
}

fn note_beside(at: &Path) -> Result<Option<stamp::Stamp>, Never> {
    Ok(match std::fs::read_to_string(at) {
        Ok(said) => {
            let Ok(held) = stamp::read(&said);

            held
        },
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => None,

        Err(fault) => {
            eprintln!("{}: reading the note beside the add-on: {fault}", at.display());
            None
        }
    })
}

fn packed_version(at: &Path) -> Result<Option<String>, Never> {
    Ok(match std::fs::read(at) {
        Ok(bytes) => {
            let Ok(said) = stamp::packed(&bytes);

            said
        },
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => None,

        Err(fault) => {
            eprintln!("{}: reading the packed add-on for its version: {fault}", at.display());
            None
        }
    })
}

fn wrote(at: &Path, bytes: &[u8]) -> std::io::Result<()> {
    match at.parent() {
        Some(parent) => std::fs::create_dir_all(parent)?,
        None => {}
    }

    let name = at.file_name().and_then(|name| name.to_str()).unwrap_or("file");
    let beside = at.with_file_name(format!("{name}.{NEW}"));
    std::fs::write(&beside, bytes)?;
    std::fs::rename(&beside, at)
}
