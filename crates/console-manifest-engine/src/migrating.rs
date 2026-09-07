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
//! install over a machine whose state nobody now knows, and `console apply` is
//! the thing people reach for when something is wrong.

use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_external_programs::Program;
use console_manifest_migrations::done::{self, KEPT};
use console_manifest_migrations::sweeping;

fn attic() -> Result<PathBuf, String> {
    let Ok(date) = Program::Date.name();

    let said = Command::new(date)
        .args(["+%Y%m%d-%H%M%S"])
        .output()
        .map_err(|fault| format!("what time it is: {fault}"))?;

    let when = String::from_utf8_lossy(&said.stdout).trim().to_string();

    let Ok(attic) = done::attic(&when);

    Ok(attic)
}

pub fn outstanding(root: &Path) -> Result<Vec<String>, String> {
    let Ok(under) = sweeping::beside(root);
    let every = sweeping::every(&under)?;
    let Ok(already) = done::already(Path::new(KEPT));
    let Ok(pending) = done::pending(&every, &already);

    Ok(pending)
}

pub fn run(root: &Path, user: &str) -> Result<(), String> {
    let outstanding = outstanding(root)?;

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

        let ran = Command::new(bash)
            .args(["-euo", "pipefail", "-c", ". \"$CONSOLE_HELPERS\"; . \"$CONSOLE_MIGRATION\""])
            .env("CONSOLE_HELPERS", &helpers)
            .env("CONSOLE_MIGRATION", under.join(name))
            .env("CONSOLE_ATTIC", &attic)
            .env("CONSOLE_HOME", format!("/home/{user}"))
            .status()
            .map_err(|fault| format!("migration {name}: {fault}"))?;

        match ran.success() {
            true => done::remember(Path::new(KEPT), name)?,
            false => {
                return Err(format!(
                    "migration {name} stopped, so nothing after it has run and \
                     nothing has been installed over it"
                ));
            }
        }
    }

    match attic.exists() {
        true => println!("what was swept is under {}", attic.display()),
        false => {},
    }

    Ok(())
}
