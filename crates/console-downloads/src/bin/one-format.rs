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

use console_downloads::getting;
use console_downloads::same::{self, Wants};
use console_downloads::store::Kind;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_panel::running::say;
use gtk4::glib;

const KIND: &str = "one-format";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Made {
    It,
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ran {
    Fine,
    Badly,
}

fn main() {
    let where_: Vec<PathBuf> = match std::env::args().nth(1) {
        Some(said) => vec![PathBuf::from(said)],
        None => Kind::BOTH
            .iter()
            .map(|kind| {
                let Ok(into) = getting::into(*kind);

                into
            })
            .collect(),
    };
    let mut made: usize = 0;
    let mut left: usize = 0;

    for folder in &where_ {
        let Ok(wanting) = wanting(folder);

        for path in wanting {
            let Ok(made_one) = made_one(&path);

            match made_one {
                Made::It => made = made.saturating_add(1),
                Made::Nothing => left = left.saturating_add(1),
            }
        }
    }

    let Ok(()) = told(made, left, &where_);
}

fn wanting(folder: &Path) -> Result<Vec<PathBuf>, Never> {
    let Ok(reading) = std::fs::read_dir(folder) else { return Ok(Vec::new()) };

    let mut found: Vec<PathBuf> = reading
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            let Ok(named) = named(path);

            !matches!(named, Wants::Nothing | Wants::Leave)
        })
        .collect();
    found.sort();
    Ok(found)
}

fn named(path: &Path) -> Result<Wants, Never> {
    same::wants(&path.file_name().unwrap_or_default().to_string_lossy())
}

fn what(path: &Path) -> Result<Option<Kind>, Never> {
    let Ok(named) = named(path);

    Ok(match named {
        Wants::Made(kind) => Some(kind),
        Wants::Ask => {
            let Ok(about) = same::about(path);
            let Ok(said) = said(&about);
            let Ok(inside) = same::inside(&said);

            Some(inside)
        },
        Wants::Nothing | Wants::Leave => None,
    })
}

fn made_one(path: &Path) -> Result<Made, Never> {
    let Ok(Some(kind)) = what(path) else { return Ok(Made::Nothing) };

    let Ok(to) = same::beside(path, kind);

    match to.exists() {
        true => return Ok(Made::Nothing),
        false => {},
    }

    let Ok(ending) = ending(kind);
    let part = to.with_extension(format!("part.{ending}"));
    let Ok(done) = match kind {
        Kind::Film => {
            let Ok(film) = same::film(path, &part);

            ran(&film)
        },
        Kind::Sound => {
            let Ok(cover) = cover(path);
            let Ok(sound) = same::sound(path, &part, cover.as_deref());

            ran(&sound)
        },
    };

    match done == Ran::Badly || !part.exists() {
        true => {
            let _ = std::fs::remove_file(&part);
            return Ok(Made::Nothing);
        }
        false => {},
    }

    match std::fs::rename(&part, &to) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("putting the converted file where the old one is: {fault}");
            let _ = std::fs::remove_file(&part);
            return Ok(Made::Nothing);
        }
    }

    let Ok(put_away) = ran(&[
        "gio".to_string(),
        "trash".to_string(),
        "--".to_string(),
        path.to_string_lossy().to_string(),
    ]);

    match put_away {
        Ran::Badly => {
            let _ = std::fs::remove_file(&to);
            return Ok(Made::Nothing);
        }
        Ran::Fine => {},
    }

    Ok(Made::It)
}

fn ending(kind: Kind) -> Result<&'static str, Never> {
    Ok(match kind {
        Kind::Sound => getting::SOUND,
        Kind::Film => getting::FILM,
    })
}

fn cover(path: &Path) -> Result<Option<String>, Never> {
    let jpg = glib::user_cache_dir().join("console/download/cover.jpg");

    match jpg.parent() {
        Some(holding) => {
            let _ = std::fs::create_dir_all(holding);
        }
        None => {},
    }

    let _ = std::fs::remove_file(&jpg);

    let Ok(argv) = same::cover(path, &jpg);
    let Ok(ran) = ran(&argv);

    match ran {
        Ran::Badly => return Ok(None),
        Ran::Fine => {},
    }

    let held = match std::fs::read(&jpg) {
        Ok(held) => held,
        Err(fault) => {
            eprintln!("reading back the cover ffmpeg wrote: {fault}");
            return Ok(None);
        },
    };
    let _ = std::fs::remove_file(&jpg);

    Ok(match held.is_empty() {
        true => None,
        false => {
            let Ok(block) = same::block("image/jpeg", &held);

            Some(block)
        },
    })
}

fn ran(argv: &[String]) -> Result<Ran, Never> {
    let Some((program, rest)) = argv.split_first() else { return Ok(Ran::Badly) };

    let worked =
        Command::new(program).args(rest).output().is_ok_and(|done| done.status.success());

    Ok(match worked {
        true => Ran::Fine,
        false => Ran::Badly,
    })
}

fn said(argv: &[String]) -> Result<String, Never> {
    let Some((program, rest)) = argv.split_first() else { return Ok(String::new()) };

    let Ok(done) = Command::new(program).args(rest).output() else { return Ok(String::new()) };

    Ok(String::from_utf8_lossy(&done.stdout).to_string())
}

fn told(made: usize, left: usize, where_: &[PathBuf]) -> Result<(), Never> {
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
    let Ok(mut notifysend) = Program::NotifySend.command();

    let started = notifysend
        .args(["--app-name=Console", "--icon=folder-music", "--"])
        .arg(folders.join(" and "))
        .arg(&said)
        .status();

    match started {
        Ok(_) => {},
        Err(fault) => {
            eprintln!("telling somebody the folder is one format: {fault}");
            println!("{} {said}", folders.join(" and "));
        }
    }

    match left > 0 && made == 0 {
        true => {
            let Ok(()) = say(KIND, &format!("{} is not one format", folders.join(" and ")), &said);
        }
        false => {},
    }

    Ok(())
}
