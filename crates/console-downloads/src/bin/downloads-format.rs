//! Make everything in a folder the one format this device keeps.
//!
//!     downloads-format                     the music folder and the videos folder
//!     downloads-format /run/media/stick    whatever is in there
//!
//! Sound becomes opus and a film becomes mkv, which are what the fetcher
//! already writes, so a folder this has been over is a folder one program can
//! play the whole of. Independently: a film in the music folder is still made a
//! film, because what a file is is a question about the file rather than about
//! which folder someone left it in.
//!
//! What is replaced goes to the wastebasket. `gio trash` rather than unlinking,
//! the same as the Files panel deletes with, so an afternoon's conversion
//! someone regrets is an afternoon's walk back rather than a loss.
//!
//! One folder, not the tree under it. A folder is what someone is standing in
//! and what they asked about; a tree is a thing that runs for an hour over
//! places they were not thinking of.

use std::path::{Path, PathBuf};
use std::process::Command;

use console_downloads::getting;
use console_downloads::same::{self, Wants};
use console_downloads::store::Kind;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_panel::running::{Notification, say};

const THERE: &str = "there";


const KIND: &str = "downloads-format";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Made {
    It,
    None,
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
    let mut made: u32 = 0;
    let mut left: u32 = 0;

    for folder in &where_ {
        let Ok(wanting) = wanting(folder);

        for path in wanting {
            let Ok(made_one) = made_one(&path);

            match made_one {
                Made::It => made = made.saturating_add(1),
                Made::None => left = left.saturating_add(1),
            }
        }
    }

    let Ok(()) = told(Converted { made, left }, &where_);
}

fn wanting(folder: &Path) -> Result<Vec<PathBuf>, Never> {
    let reading = match std::fs::read_dir(folder) {
        Ok(reading) => reading,
        Err(_fault) => return Ok(Vec::new()),
    };

    let mut found: Vec<PathBuf> = reading
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            let Ok(named) = named(path);

            !matches!(named, Wants::None | Wants::Leave)
        })
        .collect();
    found.sort();
    Ok(found)
}

fn named(path: &Path) -> Result<Wants, Never> {
    let named = match path.file_name() {
        Some(named) => named.to_string_lossy().to_string(),
        None => String::new(),
    };

    same::wants(&named)
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
        Wants::None | Wants::Leave => None,
    })
}

fn made_one(path: &Path) -> Result<Made, Never> {
    let kind = match what(path) {
        Ok(Some(kind)) => kind,
        Ok(None) | Err(_) => return Ok(Made::None),
    };

    let Ok(to) = same::beside(path, kind);

    match to.exists() {
        true => return Ok(Made::None),
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
        Kind::Book => return Ok(Made::None),
    };

    match done == Ran::Badly || !part.exists() {
        true => {
            let _ = std::fs::remove_file(&part);
            return Ok(Made::None);
        }
        false => {},
    }

    match std::fs::rename(&part, &to) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("putting the converted file where the old one is: {fault}");
            let _ = std::fs::remove_file(&part);
            return Ok(Made::None);
        }
    }

    let Ok(console_put_away) = ran(&[
        "gio".to_string(),
        "trash".to_string(),
        "--".to_string(),
        path.to_string_lossy().to_string(),
    ]);

    match console_put_away {
        Ran::Badly => {
            let _ = std::fs::remove_file(&to);
            return Ok(Made::None);
        }
        Ran::Fine => {},
    }

    Ok(Made::It)
}

fn ending(kind: Kind) -> Result<&'static str, Never> {
    Ok(match kind {
        Kind::Sound => getting::SOUND,
        Kind::Film => getting::FILM,
        Kind::Book => getting::BOOK,
    })
}

fn cover(path: &Path) -> Result<Option<String>, Never> {
    let Ok(ours) = console_core_places::Base::Cache.ours();

    let jpg = match ours {
        Some(ours) => ours.join("download/cover.jpg"),
        None => return Ok(None),
    };

    match jpg.parent() {
        Some(holding) => {
            let _ = std::fs::create_dir_all(holding);
        }
        None => {},
    }

    let _ = std::fs::remove_file(&jpg);

    let Ok(arguments) = same::cover(path, &jpg);
    let Ok(ran) = ran(&arguments);

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

fn ran(arguments: &[String]) -> Result<Ran, Never> {
    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Ok(Ran::Badly),
    };

    let worked =
        Command::new(program).args(rest).output().is_ok_and(|done| done.status.success());

    Ok(match worked {
        true => Ran::Fine,
        false => Ran::Badly,
    })
}

fn said(arguments: &[String]) -> Result<String, Never> {
    Ok(match console_core_external_programs::printed(arguments) {
        Ok(said) => said,
        Err(_unprinted) => String::new(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Converted {
    made: u32,
    left: u32,
}

fn told(converted: Converted, where_: &[PathBuf]) -> Result<(), Never> {
    let Converted { made, left } = converted;
    let folders: Vec<String> = where_
        .iter()
        .map(|at| match at.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => THERE.to_string(),
        })
        .collect();
    let said = match (made, left) {
        (0, 0) => "Already converted".to_string(),
        (0, left) => format!("Couldn't convert {left} files"),
        (made, 0) => format!("Converted {made} files"),
        (made, left) => format!("Converted {made} files, skipped {left}"),
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
            eprintln!("telling someone the folder is one format: {fault}");
            println!("{} {said}", folders.join(" and "));
        }
    }

    match left > 0 && made == 0 {
        true => {
            let Ok(()) = say(KIND, Notification {
                summary: &format!("Couldn't convert {}", folders.join(" and ")),
                body: &said,
            });
        }
        false => {},
    }

    Ok(())
}
