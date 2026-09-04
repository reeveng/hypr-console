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

use console_web::{PALETTE, source, stamp};

/// What a file is called while it is written and not yet in place.
///
/// The same word `console apply` leaves beside a file it is halfway through,
/// for the same reason: an add-on written into the place the browser reads it
/// from is an add-on a browser can be reading while it is half there.
const NEW: &str = "console-new";

fn main() -> ExitCode {
    let always = std::env::args().any(|word| word == "--always");

    // Unset and set-to-something-that-is-not-text are one answer here, because
    // there is nothing a caller does about either beyond falling back. This is
    // run as her by `console apply`, so an unset HOME means it was started some
    // other way, and root's home is where it went before there was a rule
    // about saying so. Keeping that fallback is the point: a pack that refuses
    // because HOME is unset is a deploy that stops for no reason.
    let home = match std::env::var("HOME") {
        Ok(said) => PathBuf::from(said),
        Err(_) => PathBuf::from("/root"),
    };

    let at = home.join(PALETTE);

    let Ok(palette) = std::fs::read_to_string(&at) else {
        eprintln!("{}: no palette to dress the add-on in", at.display());
        return ExitCode::from(1);
    };

    let Some(palette) = source::hosted(&palette) else {
        eprintln!("{}: not a palette this can read", at.display());
        return ExitCode::from(1);
    };

    let xpi = console_web::xpi(&home);
    let stamped = console_web::stamp(&home);
    let hash = source::hash(&palette);
    let held = note_beside(&stamped);

    if !always
        && xpi.is_file()
        && held.as_ref().is_some_and(|held| held.hash == hash)
    {
        println!("{}: already packed", xpi.display());
        return ExitCode::SUCCESS;
    }

    let was = held.map(|held| held.version).or_else(|| packed_version(&xpi));
    let version = stamp::next(was.as_deref());
    let made = console_web::pack::zip(&source::every(&version, &palette));

    if let Err(why) = wrote(&xpi, &made) {
        eprintln!("{}: {why}", xpi.display());
        return ExitCode::from(1);
    }

    let note = stamp::written(&stamp::Stamp { hash, version: version.clone() });

    if let Err(why) = wrote(&stamped, note.as_bytes()) {
        eprintln!("{}: {why}", stamped.display());
        return ExitCode::from(1);
    }

    println!("{}: packed, version {version}", xpi.display());
    ExitCode::SUCCESS
}

/// The note beside the add-on, or nothing if there is not one to read.
///
/// No note is ordinary: it is what the first pack on a machine finds, and what
/// is left after somebody clears the profile. A note that is there and will not
/// read is not ordinary, and it used to arrive here as the same silence. It is
/// said now, and then carried on from, because an unreadable note is a reason
/// to pack again rather than a reason to stop.
fn note_beside(at: &Path) -> Option<stamp::Stamp> {
    match std::fs::read_to_string(at) {
        Ok(said) => stamp::read(&said),
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => None,

        Err(fault) => {
            eprintln!("{}: reading the note beside the add-on: {fault}", at.display());
            None
        }
    }
}

/// The version inside the packed archive, for the day the note has gone.
///
/// The same distinction the note gets, for the same reason: no archive is
/// ordinary, and an archive that will not read is worth a sentence.
fn packed_version(at: &Path) -> Option<String> {
    match std::fs::read(at) {
        Ok(bytes) => stamp::packed(&bytes),
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => None,

        Err(fault) => {
            eprintln!("{}: reading the packed add-on for its version: {fault}", at.display());
            None
        }
    }
}

/// Write one file, whole or not at all, making the directory it goes in.
fn wrote(at: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = at.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let name = at.file_name().and_then(|name| name.to_str()).unwrap_or("file");
    let beside = at.with_file_name(format!("{name}.{NEW}"));
    std::fs::write(&beside, bytes)?;
    std::fs::rename(&beside, at)
}
