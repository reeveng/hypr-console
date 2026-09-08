//! Change a name everywhere this tree writes it.
//!
//!     console-rename console-music console-music-panel
//!
//! Sweeps both spellings through every tracked file, moves the directories and
//! files whose own names carry it, and writes a migration claiming any path
//! that moved under `files/` -- because an apply installs a name and has never
//! removed one, so a renamed file lands beside the old one rather than instead
//! of it.
//!
//! It does not commit and it does not write the migration's argument. Read the
//! diff: a sweep through prose is a sweep through sentences that were true
//! about the old name, and some of those want rewriting rather than renaming.
//! `crates/console-repository/src/renaming.rs` argues for the rest.

use console_core_external_programs::Program;
use console_core_never::Never;
use console_repository::renaming::{Moved, installed, renamed, stub, through, tracked};
use std::path::{Path, PathBuf};

fn main() -> Result<(), Never> {
    let old = std::env::args().nth(1).unwrap_or_default();
    let new = std::env::args().nth(2).unwrap_or_default();

    match old.is_empty() || new.is_empty() {
        true => {
            eprintln!("console-rename: two names, the one it is and the one it should be");

            return Ok(());
        },
        false => {},
    }

    let root = match console_repository::root() {
        Ok(root) => root,
        Err(said) => {
            eprintln!("console-rename: {said}");

            return Ok(());
        },
    };

    let Ok(written) = sweeping(&root, &old, &new);
    let Ok(moved) = moving(&root, &old, &new);
    let Ok(sweeping) = claimed(&root, &moved);

    println!("{written} files say the new name");

    for held in &moved {
        println!("moved {} to {}", held.from.display(), held.to.display());
    }

    match sweeping.is_empty() {
        true => Ok(()),
        false => wrote(&root, &sweeping),
    }
}

fn sweeping(root: &Path, old: &str, new: &str) -> Result<usize, Never> {
    let Ok(files) = tracked(root);
    let mut written: usize = 0;

    for at in files {
        let said = match std::fs::read_to_string(&at) {
            Ok(said) => said,
            Err(_it_is_not_text_or_it_is_gone) => continue,
        };

        let Ok(swept) = through(&said, old, new);

        match swept == said {
            true => continue,
            false => {},
        }

        match std::fs::write(&at, swept) {
            Ok(()) => written = written.saturating_add(1),
            Err(fault) => eprintln!("console-rename: {}: {fault}", at.display()),
        }
    }

    Ok(written)
}

fn moving(root: &Path, old: &str, new: &str) -> Result<Vec<Moved>, Never> {
    let Ok(files) = tracked(root);
    let mut moved = Vec::new();
    let mut done: Vec<PathBuf> = Vec::new();

    for at in files {
        for held in at.ancestors() {
            let Ok(landing) = renamed(held, old, new);

            let landing = match landing {
                Some(landing) => landing,
                None => continue,
            };

            match done.contains(&held.to_path_buf()) {
                true => continue,
                false => done.push(held.to_path_buf()),
            }

            let Ok(went) = git_mv(root, held, &landing);

            match went {
                Went::Yes => moved.push(Moved { from: held.to_path_buf(), to: landing }),
                Went::No => {},
            }
        }
    }

    Ok(moved)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Went {
    Yes,
    No,
}

fn git_mv(root: &Path, from: &Path, to: &Path) -> Result<Went, Never> {
    let Ok(mut asking) = Program::Git.command();

    asking.arg("-C").arg(root).arg("mv").arg(from).arg(to);

    match asking.status() {
        Ok(done) => Ok(match done.success() {
            true => Went::Yes,
            false => Went::No,
        }),
        Err(fault) => {
            eprintln!("console-rename: moving {}: {fault}", from.display());

            Ok(Went::No)
        },
    }
}

fn claimed(root: &Path, moved: &[Moved]) -> Result<Vec<String>, Never> {
    let mut claims = Vec::new();

    for held in moved {
        let Ok(was) = installed(root, &held.from);

        match was {
            Some(was) => claims.push(was),
            None => {},
        }
    }

    Ok(claims)
}

fn wrote(root: &Path, sweeping: &[String]) -> Result<(), Never> {
    let when = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(since) => since.as_secs(),
        Err(_the_clock_is_before_the_epoch) => 0,
    };

    let at = root.join(format!("migrations/{when}.sh"));
    let Ok(said) = stub(sweeping);

    match std::fs::write(&at, said) {
        Ok(()) => {
            println!("wrote {} -- the reason in it is yours to write", at.display());

            Ok(())
        },
        Err(fault) => {
            eprintln!("console-rename: {}: {fault}", at.display());

            Ok(())
        },
    }
}
