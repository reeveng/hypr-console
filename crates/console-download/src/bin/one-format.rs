//! Make everything in a folder the one format this device keeps.
//!
//!     one-format                     the music folder and the videos folder
//!     one-format /run/media/stick    whatever is in there
//!
//! Sound becomes opus and a film becomes mkv, which are what the fetcher
//! already writes, so a folder this has been over is a folder one program can
//! play the whole of. Independently: a film in the music folder is still made a
//! film, because what a file is is a question about the file rather than about
//! which folder somebody left it in.
//!
//! What is replaced goes to the wastebasket. `gio trash` rather than unlinking,
//! the same as the Files panel deletes with, so an afternoon's conversion
//! somebody regrets is an afternoon's walk back rather than a loss.
//!
//! One folder, not the tree under it. A folder is what somebody is standing in
//! and what they asked about; a tree is a thing that runs for an hour over
//! places they were not thinking of.

use std::path::{Path, PathBuf};
use std::process::Command;

use gtk4::glib;
use console_download::getting;
use console_download::same::{self, Wants};
use console_download::store::Kind;
use console_panel::running::say;

/// What a fault here is counted as.
const KIND: &str = "one-format";

/// Whether one thing came out of this the format it should be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Made {
    /// It is the one format now, and what it replaced is in the wastebasket.
    It,
    /// It was left the way it was found.
    Nothing,
}

/// Whether something run did what it was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ran {
    /// It ended, and said it worked.
    Fine,
    /// It would not start, or said it did not.
    Badly,
}

fn main() {
    let where_: Vec<PathBuf> = match std::env::args().nth(1) {
        Some(said) => vec![PathBuf::from(said)],
        None => Kind::BOTH.iter().map(|kind| getting::into(*kind)).collect(),
    };
    let mut made = 0;
    let mut left = 0;

    for folder in &where_ {
        for path in wanting(folder) {
            match made_one(&path) {
                Made::It => made += 1,
                Made::Nothing => left += 1,
            }
        }
    }

    told(made, left, &where_);
}

/// Everything in a folder that is not the format it should be.
///
/// Sorted, so a folder somebody is watching converts down the list rather than
/// in whatever order the disk answers in.
fn wanting(folder: &Path) -> Vec<PathBuf> {
    let Ok(reading) = std::fs::read_dir(folder) else { return Vec::new() };

    let mut found: Vec<PathBuf> = reading
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| !matches!(named(path), Wants::Nothing | Wants::Leave))
        .collect();
    found.sort();
    found
}

/// What a file says it is, by its name.
fn named(path: &Path) -> Wants {
    same::wants(&path.file_name().unwrap_or_default().to_string_lossy())
}

/// What a file actually is, asking it where the name will not say.
fn what(path: &Path) -> Option<Kind> {
    match named(path) {
        Wants::Made(kind) => Some(kind),
        Wants::Ask => Some(same::inside(&said(&same::about(path)))),
        Wants::Nothing | Wants::Leave => None,
    }
}

/// One thing, made the format it should be. Says whether it was.
fn made_one(path: &Path) -> Made {
    let Some(kind) = what(path) else { return Made::Nothing };

    let to = same::beside(path, kind);

    // Something of that name is already there. Two files that would become one
    // name are two files somebody chose to keep, and this is not the program to
    // decide which of them wins.
    if to.exists() {
        return Made::Nothing;
    }

    let part = to.with_extension(format!("part.{}", ending(kind)));
    let done = match kind {
        Kind::Film => ran(&same::film(path, &part)),
        Kind::Sound => ran(&same::sound(path, &part, cover(path).as_deref())),
    };

    if done == Ran::Badly || !part.exists() {
        let _ = std::fs::remove_file(&part);
        return Made::Nothing;
    }

    if let Err(fault) = std::fs::rename(&part, &to) {
        eprintln!("putting the converted file where the old one is: {fault}");
        let _ = std::fs::remove_file(&part);
        return Made::Nothing;
    }

    // Only once the new one is in place, and to the wastebasket rather than
    // gone: what this replaced is somebody's, and the conversion is a thing
    // they may want back.
    let put_away = ran(&[
        "gio".to_string(),
        "trash".to_string(),
        "--".to_string(),
        path.to_string_lossy().to_string(),
    ]);

    if put_away == Ran::Badly {
        // Somewhere with no wastebasket to put it in -- a stick, mostly. The
        // conversion is taken back rather than left: a folder holding both the
        // old file and the new one is further from one format than it was
        // before, and the next run would skip it for having a name already.
        let _ = std::fs::remove_file(&to);
        return Made::Nothing;
    }

    Made::It
}

/// The extension of the format a kind is kept in.
fn ending(kind: Kind) -> &'static str {
    match kind {
        Kind::Sound => getting::SOUND,
        Kind::Film => getting::FILM,
    }
}

/// The cover of a song, as the comment the new file will carry it in.
///
/// Taken out into a file of its own first, because a picture cannot be handed
/// from one ffmpeg to the next: the opus muxer refuses a picture stream, and
/// what it does take is a comment, which has to be the whole picture as text
/// before the conversion starts.
fn cover(path: &Path) -> Option<String> {
    let jpg = glib::user_cache_dir().join("console/download/cover.jpg");

    if let Some(holding) = jpg.parent() {
        let _ = std::fs::create_dir_all(holding);
    }

    let _ = std::fs::remove_file(&jpg);

    if ran(&same::cover(path, &jpg)) == Ran::Badly {
        return None;
    }

    let held = match std::fs::read(&jpg) {
        Ok(held) => held,
        Err(fault) => {
            eprintln!("reading back the cover ffmpeg wrote: {fault}");
            return None;
        },
    };
    let _ = std::fs::remove_file(&jpg);

    match held.is_empty() {
        true => None,
        false => Some(same::block("image/jpeg", &held)),
    }
}

/// Something run, said whether it worked.
fn ran(argv: &[String]) -> Ran {
    let Some((program, rest)) = argv.split_first() else { return Ran::Badly };

    let worked =
        Command::new(program).args(rest).output().is_ok_and(|done| done.status.success());

    match worked {
        true => Ran::Fine,
        false => Ran::Badly,
    }
}

/// What a command printed.
fn said(argv: &[String]) -> String {
    let Some((program, rest)) = argv.split_first() else { return String::new() };

    let Ok(done) = Command::new(program).args(rest).output() else { return String::new() };

    String::from_utf8_lossy(&done.stdout).to_string()
}

/// What was done, said where somebody sees it.
///
/// Said even when it was nothing, because this is started by a press and a
/// press that says nothing is a press somebody makes again. What was left alone
/// is counted rather than named: a folder where nothing could be converted is
/// worth one line, and twelve lines is a wall.
fn told(made: usize, left: usize, where_: &[PathBuf]) {
    let folders: Vec<String> = where_
        .iter()
        .map(|at| at.file_name().map(|name| name.to_string_lossy().to_string()))
        .map(|name| name.unwrap_or_else(|| "there".to_string()))
        .collect();
    let said = match (made, left) {
        (0, 0) => "was already one format".to_string(),
        (0, left) => format!("has {left} nothing here could convert"),
        (made, 0) => format!("is one format now: {made} converted"),
        (made, left) => format!("is one format now: {made} converted, {left} left alone"),
    };
    let started = Command::new("notify-send")
        .args(["--app-name=Console", "--icon=folder-music", "--"])
        .arg(folders.join(" and "))
        .arg(&said)
        .status();

    if let Err(fault) = started {
        eprintln!("telling somebody the folder is one format: {fault}");
        println!("{} {said}", folders.join(" and "));
    }

    if left > 0 && made == 0 {
        say(KIND, &format!("{} is not one format", folders.join(" and ")), &said);
    }
}
