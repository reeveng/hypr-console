//! Running the migrations this machine has not run.
//! `console_manifest_migrations` decides what a migration is and which are
//! outstanding; this is the half that is allowed to touch the machine. The
//! split is the same one the rest of the tree keeps: the crate is arithmetic
//! over names and can be asked twice, and the answer only becomes a `mv` here.
//! It runs before anything else an apply does. A migration exists because the
//! manifest stopped naming something, so the machine it runs on is the one
//! about to be brought to a manifest that does not name it -- and a sweep that
//! ran afterwards would be sweeping beside a freshly installed set of the very
//! files it is deciding about. The rename went in this order for the same
//! reason and said so: what the old names left behind goes to the attic after
//! the new ones are installed *there* only because that migration was also
//! doing the install.  A migration that fails stops the apply. That is not the
//! cautious choice, it is the only honest one: the next thing the apply does is
//! install over a machine whose state no one now knows, and `console apply` is
//! the thing people reach for when something is wrong.
//!
//! On a machine that has never applied none of them is run and all of them are
//! remembered, which is `done::Applied::Never` and argued for there.

use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_external_programs::Program;
use console_manifest_migrations::done::{self, KEPT, Outstanding};
use console_manifest_migrations::sweeping;

use crate::unapplied::Unapplied;

fn attic() -> Result<PathBuf, Unapplied> {
    let Ok(date) = Program::Date.name();

    let said = Command::new(date)
        .args(["+%Y%m%d-%H%M%S"])
        .output()
        .map_err(Unapplied::WhatTimeItIs)?;

    let when = String::from_utf8_lossy(&said.stdout).trim().to_string();

    let Ok(attic) = done::attic(&when);

    Ok(attic)
}

pub fn outstanding(root: &Path) -> Result<Outstanding, Unapplied> {
    let Ok(under) = sweeping::beside(root);
    let every = sweeping::every(&under)?;
    let applied = done::already(Path::new(KEPT))?;
    let Ok(pending) = done::pending(&every, &applied);

    Ok(pending)
}

pub fn run(root: &Path, user: &str) -> Result<(), Unapplied> {
    let pending = outstanding(root)?;

    let outstanding = match pending {
        Outstanding::Run(names) => names,
        Outstanding::Remember(names) => return remembered(&names),
    };

    match outstanding.is_empty() {
        true => return Ok(()),
        false => {},
    }

    let attic = attic()?;
    let Ok(under) = sweeping::beside(root);

    let helpers = under.join("attic.sh");

    for name in &outstanding {
        println!("migration {name}");

        let Ok(bash) = Program::Bash.name();

        #[cfg_attr(
            dylint_lib = "explicit029_no_asking_per_item",
            allow(
                explicit029_no_asking_per_item,
                reason = "a migration is a shell script someone wrote, and each is remembered as done on its own: one of them failing has to leave the ones after it unrun, which one process for the lot could not do"
            )
        )]
        let ran = Command::new(bash)
            .args(["-euo", "pipefail", "-c", ". \"$CONSOLE_HELPERS\"; . \"$CONSOLE_MIGRATION\""])
            .env("CONSOLE_HELPERS", &helpers)
            .env("CONSOLE_MIGRATION", under.join(name))
            .env("CONSOLE_ATTIC", &attic)
            .env("CONSOLE_HOME", format!("/home/{user}"))
            .status()
            .map_err(|fault| Unapplied::Migration(name.clone(), fault))?;

        match ran.success() {
            true => done::remember(Path::new(KEPT), name)?,
            false => {
                return Err(Unapplied::MigrationStopped(name.clone()));
            }
        }
    }

    match attic.exists() {
        true => println!("what was swept is under {}", attic.display()),
        false => {},
    }

    Ok(())
}

fn remembered(names: &[String]) -> Result<(), Unapplied> {
    match names.is_empty() {
        true => return Ok(()),
        false => {},
    }

    println!(
        "this machine has never applied, so the {} migrations in the history are marked done \
         rather than run: each of them moves aside what an older manifest installed here, and \
         nothing here was installed by one",
        names.len()
    );

    for name in names {
        done::remember(Path::new(KEPT), name)?;
    }

    Ok(())
}
