//! Unpack one archive into a folder of its own, and stop.
//!
//!     files-unzip ~/Downloads/WickedWhims.zip
//!
//! Off the panel and not in it, for the reason the pictures are: a mod is
//! tens of megabytes of compressed package files and a panel that unpacked
//! them where it draws would answer nothing until it was done.
//! `Showing::later` runs this and draws the folder again when it ends, so what
//! arrived is on the screen without anybody asking for it.
//!
//! The unpacking happens into a hidden folder beside the archive and the
//! result is renamed into place, which is one rename rather than a folder that
//! fills up while somebody watches it. A dot in front is all it takes: the
//! listing does not show what starts with one, so an unzip that dies halfway
//! leaves nothing on the screen to explain.
//!
//! `console_files::unzipping` is what decides the names and whether the folder
//! inside the archive is a wrapper to lift away. This is the part that touches
//! the machine.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_core_external_programs::Program;
use console_files::places::Is;
use console_files::unzipping::{self, Lift};
use console_core_never::Never;
use console_panel::running::say;

const KIND: &str = "files-unzip";

const FULL: &str = "There is nowhere left to unzip this: the folders beside it are all taken.";

fn main() -> ExitCode {
    let said = match std::env::args().nth(1) {
        Some(said) => said,
        None => {
            eprintln!("usage: files-unzip ARCHIVE");
            return ExitCode::from(2);
        }
    };

    let archive = PathBuf::from(said);

    let holding = match archive.parent().map(Path::to_path_buf) {
        Some(holding) => holding,
        None => {
            eprintln!("files-unzip: {}: nothing holds this", archive.display());
            return ExitCode::FAILURE;
        }
    };

    let named = archive
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();

    let Ok(into) = unzipping::named_for(&named);

    let Ok(free) = unzipping::beside(&into, |tried| holding.join(tried).exists());

    let into = match free {
        Some(into) => into,
        None => {
            let Ok(()) = say(KIND, &named, FULL);

            return ExitCode::FAILURE;
        }
    };

    let Ok(while_) = unzipping::while_unpacking(&named);
    let unpacking = holding.join(while_);

    let Ok(gone) = unpacked(&archive, &named, &unpacking, &holding.join(into));

    gone
}

fn unpacked(
    archive: &Path,
    named: &str,
    unpacking: &Path,
    into: &Path,
) -> Result<ExitCode, Never> {
    let _ = std::fs::remove_dir_all(unpacking);

    match std::fs::create_dir_all(unpacking) {
        Ok(()) => {},
        Err(fault) => {
            let Ok(()) = told(named, &format!("{}: {fault}", unpacking.display()));

            return Ok(ExitCode::FAILURE);
        }
    }

    let Ok(ran) = ran(archive, unpacking);

    match ran {
        Ran::Badly(why) => {
            let _ = std::fs::remove_dir_all(unpacking);

            let Ok(()) = told(named, &why);

            return Ok(ExitCode::FAILURE);
        }
        Ran::Fine => {},
    }

    let Ok(inside) = inside(unpacking);

    let Ok(lift) = unzipping::lifting(&inside);

    let out_of = match lift {
        Lift::TheFolderInside(name) => unpacking.join(name),
        Lift::Nothing => unpacking.to_path_buf(),
    };

    match std::fs::rename(&out_of, into) {
        Ok(()) => {},
        Err(fault) => {
            let Ok(()) = told(named, &format!("{}: {fault}", into.display()));

            return Ok(ExitCode::FAILURE);
        }
    }

    let _ = std::fs::remove_dir_all(unpacking);

    Ok(ExitCode::SUCCESS)
}

enum Ran {
    Fine,
    Badly(String),
}

fn ran(archive: &Path, unpacking: &Path) -> Result<Ran, Never> {
    let Ok(mut seven) = Program::SevenZip.command();

    let out = format!("-o{}", unpacking.display());

    let started =
        seven.args(["x", "-y", "-bso0", "-bsp0", "-p"]).arg(out).arg("--").arg(archive).output();

    let done = match started {
        Ok(done) => done,
        Err(fault) => return Ok(Ran::Badly(format!("{fault}"))),
    };

    match done.status.success() {
        true => Ok(Ran::Fine),
        false => {
            let said = String::from_utf8_lossy(&done.stderr);
            let last = said.lines().rev().find(|line| !line.trim().is_empty());

            Ok(Ran::Badly(match last {
                Some(said) => said.trim().to_string(),
                None => "7z could not unpack this".to_string(),
            }))
        }
    }
}

fn inside(unpacking: &Path) -> Result<Vec<(String, Is)>, Never> {
    let reading = match std::fs::read_dir(unpacking) {
        Ok(reading) => reading,
        Err(_fault) => return Ok(Vec::new()),
    };

    Ok(reading
        .flatten()
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let is = match entry.path().is_dir() {
                true => Is::AFolder,
                false => Is::AFile,
            };

            (name, is)
        })
        .collect())
}

fn told(named: &str, why: &str) -> Result<(), Never> {
    say(KIND, &format!("{named} was not unzipped"), why)
}
