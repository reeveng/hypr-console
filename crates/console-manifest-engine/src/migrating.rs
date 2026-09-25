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
//! A step that stops or disables a unit is allowed to find nothing to stop:
//! a unit systemd has never heard of is a machine that already does not start
//! it, which is the state being asked for. A path that is not there is the
//! same, since every sweep has to be safe to run twice.
//!
//! On a machine that has never applied none of them is run and all of them are
//! remembered, which is `done::Applied::Never` and argued for there.

use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_external_programs::Program;
use console_manifest_migrations::done::{self, KEPT, Outstanding};
use console_manifest_migrations::history::EVERY;
use console_manifest_migrations::sweeping::{self, CopySetting, Migration, Rewrite, Step};

use crate::install::{self, User};
use crate::machine::{self, Ran};
use crate::pruning;
use crate::unapplied::Unapplied;

pub fn attic() -> Result<PathBuf, Unapplied> {
    let Ok(date) = Program::Date.name();

    let said = Command::new(date)
        .args(["+%Y%m%d-%H%M%S"])
        .output()
        .map_err(Unapplied::WhatTimeItIs)?;

    let when = String::from_utf8_lossy(&said.stdout).trim().to_string();

    let Ok(attic) = done::attic(&when);

    Ok(attic)
}

pub fn outstanding() -> Result<Outstanding, Unapplied> {
    let applied = done::already(Path::new(KEPT))?;
    let Ok(pending) = done::pending(EVERY, &applied);

    Ok(pending)
}

pub fn run(user: User<'_>) -> Result<(), Unapplied> {
    let pending = outstanding()?;

    let outstanding = match pending {
        Outstanding::Run(migrations) => migrations,
        Outstanding::Remember(migrations) => return remembered(&migrations),
    };

    match outstanding.is_empty() {
        true => return Ok(()),
        false => {},
    }

    let attic = attic()?;

    for migration in &outstanding {
        println!("migration {}: {}", migration.moment, migration.says);

        for step in migration.steps {
            carried_out(step, &attic, user)
                .map_err(|fault| Unapplied::MigrationStopped(migration.moment, Box::new(fault)))?;
        }

        done::remember(Path::new(KEPT), migration.moment)?;
    }

    match attic.exists() {
        true => println!("what was swept is under {}", attic.display()),
        false => {},
    }

    Ok(())
}

fn carried_out(step: &Step, attic: &Path, user: User<'_>) -> Result<(), Unapplied> {
    match step {
        Step::Attic(declared) => {
            let Ok(on) = install::on_machine(declared, user);

            into_the_attic(Path::new(&on), attic)
        }
        Step::AtticEach(each) => every_one(Path::new(each.under), each.named, attic),
        Step::Stop(unit) => {
            let Ok(said) = machine::user_systemctl(&["stop", unit]);

            match said.ran {
                Ran::Fine => println!("  stopped {unit}"),
                Ran::Badly => println!("  {unit} was not running"),
            }

            Ok(())
        }
        Step::Disable(unit) => {
            let Ok(said) = machine::user_systemctl(&["disable", "--now", unit]);

            match said.ran {
                Ran::Fine => println!("  stopped and disabled {unit}"),
                Ran::Badly => println!("  {unit} was not enabled"),
            }

            Ok(())
        }
        Step::DisableGlobally(unit) => {
            let Ok(systemctl) = Program::Systemctl.name();
            let Ok(said) = machine::answered(&[systemctl, "--user", "--global", "disable", unit]);

            match said.ran {
                Ran::Fine => println!("  disabled {unit}"),
                Ran::Badly => println!("  {unit} was not enabled"),
            }

            Ok(())
        }
        Step::Terminate(program) => {
            let Ok(pkill) = Program::Pkill.name();
            let Ok(said) = machine::answered(&[pkill, "-x", program]);

            match said.ran {
                Ran::Fine => println!("  ended {program}"),
                Ran::Badly => {},
            }

            Ok(())
        }
        Step::RemoveIfEmpty(declared) => {
            let Ok(on) = install::on_machine(declared, user);

            match std::fs::remove_dir(&on) {
                Ok(()) => println!("  {on} was left empty"),
                Err(_not_there_or_not_empty) => {},
            }

            Ok(())
        }
        Step::Rewrite(rewrite) => rewritten(rewrite),
        Step::CopySetting(copy) => copied(copy, user),
    }
}

fn into_the_attic(on: &Path, attic: &Path) -> Result<(), Unapplied> {
    let there = found(on)?;

    match there {
        Found::There => {
            let under = pruning::take(on, attic)?;

            println!("  {} -> {}", on.display(), under.display());

            Ok(())
        }
        Found::Absent => Ok(()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Found {
    There,
    Absent,
}

fn found(at: &Path) -> Result<Found, Unapplied> {
    match std::fs::symlink_metadata(at) {
        Ok(_metadata) => Ok(Found::There),
        Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
            true => Ok(Found::Absent),
            false => Err(Unapplied::Read(at.to_path_buf(), fault)),
        },
    }
}

fn every_one(under: &Path, named: &str, attic: &Path) -> Result<(), Unapplied> {
    let entries = match std::fs::read_dir(under) {
        Ok(entries) => entries,
        Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
            true => return Ok(()),
            false => return Err(Unapplied::Read(under.to_path_buf(), fault)),
        },
    };

    for entry in entries.flatten() {
        into_the_attic(&entry.path().join(named), attic)?;
    }

    Ok(())
}

fn read(at: &Path) -> Result<Option<String>, Unapplied> {
    match std::fs::read_to_string(at) {
        Ok(said) => Ok(Some(said)),
        Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
            true => Ok(None),
            false => Err(Unapplied::Read(at.to_path_buf(), fault)),
        },
    }
}

fn rewritten(rewrite: &Rewrite) -> Result<(), Unapplied> {
    let at = Path::new(rewrite.at);

    let held = read(at)?;

    let said = match held {
        Some(said) => said,
        None => return Ok(()),
    };

    let Ok(becomes) = sweeping::rewritten(&said, rewrite);

    match becomes {
        Some(becomes) => {
            console_core_atomic_writes::whole(at, becomes.as_bytes())?;

            println!("  {} now says {}", at.display(), rewrite.becomes);

            Ok(())
        }
        None => Ok(()),
    }
}

fn copied(copy: &CopySetting, user: User<'_>) -> Result<(), Unapplied> {
    let Ok(from) = install::on_machine(copy.from, user);
    let Ok(into) = install::on_machine(copy.into, user);
    let into = Path::new(&into);

    let already = found(into)?;

    let held = read(Path::new(&from))?;

    let said = match (already, held) {
        (Found::Absent, Some(said)) => said,
        (Found::There, _) | (Found::Absent, None) => return Ok(()),
    };

    let Ok(setting) = sweeping::setting(&said, copy);

    let setting = match setting {
        Some(setting) => setting,
        None => return Ok(()),
    };

    match into.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| Unapplied::Making(holding.to_path_buf(), fault))?,
        None => {},
    }

    console_core_atomic_writes::whole(into, format!("{setting}\n").as_bytes())?;
    machine::handed_over(into)?;

    println!("  {} now holds what {from} said: {setting}", into.display());

    Ok(())
}

fn remembered(migrations: &[Migration]) -> Result<(), Unapplied> {
    match migrations.is_empty() {
        true => return Ok(()),
        false => {},
    }

    println!(
        "this machine has never applied, so the {} migrations in the history are marked done \
         rather than run: each of them moves aside what an older manifest installed here, and \
         nothing here was installed by one",
        migrations.len()
    );

    for migration in migrations {
        done::remember(Path::new(KEPT), migration.moment)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pruning::tests::{a_machine, placed};

    #[test]
    fn a_path_that_is_there_goes_to_the_attic_and_one_that_is_not_is_nothing() {
        let machine = a_machine("attic");
        let on = machine.join("usr/local/bin/files-thumbs");
        let gone = machine.join("usr/local/bin/never-here");
        let attic = machine.join("attic");

        placed(&on, b"old\n");

        match (into_the_attic(&on, &attic), into_the_attic(&gone, &attic)) {
            (Ok(()), Ok(())) => {},
            (Err(fault), _) | (_, Err(fault)) => panic!("{fault}"),
        }

        let inside = match on.strip_prefix("/") {
            Ok(inside) => inside,
            Err(_relative) => on.as_path(),
        };

        assert!(!on.exists(), "{} is still where it was", on.display());
        assert!(attic.join(inside).exists(), "nothing reached the attic");

        let _ = std::fs::remove_dir_all(&machine);
    }

    #[test]
    fn every_directory_under_the_one_named_gives_up_its_copy() {
        let machine = a_machine("each");
        let run = machine.join("run/user");
        let attic = machine.join("attic");

        placed(&run.join("1000/console/notices.json"), b"old\n");
        placed(&run.join("1001/console/notices.json"), b"old\n");
        placed(&run.join("1001/console/notifications.json"), b"old\n");

        match every_one(&run, "console/notices.json", &attic) {
            Ok(()) => {},
            Err(fault) => panic!("{fault}"),
        }

        assert!(!run.join("1000/console/notices.json").exists());
        assert!(!run.join("1001/console/notices.json").exists());
        assert!(run.join("1001/console/notifications.json").exists(), "the live store went too");

        let _ = std::fs::remove_dir_all(&machine);
    }

    #[test]
    fn a_directory_to_look_under_that_is_not_there_is_nothing_to_sweep() {
        let machine = a_machine("nowhere");

        let swept = every_one(&machine.join("run/user"), "console/notices.json", &machine.join("attic"));

        assert!(swept.is_ok());

        let _ = std::fs::remove_dir_all(&machine);
    }
}
