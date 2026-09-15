//! Which migrations this machine has already run.
//!
//! The record is on the machine and not in the tree, because the question it
//! answers is about the machine. Two devices a release apart share a history
//! and share every migration in it, and what separates them is only how far
//! each has got. A tree that tried to remember would be remembering on behalf
//! of a machine it cannot see.
//!
//! One empty file per migration, named for it. A file rather than a list,
//! because a list is a thing to rewrite and a rewrite that is interrupted is a
//! machine that has run a migration and forgotten, or not run one and thinks it
//! has. Of the two, forgetting is much the worse: a migration is written to be
//! safe to run and the sweep it does is idempotent, so a marker written twice
//! costs nothing and a marker lost silently un-sweeps a device.
//!
//! Under `/var/lib` and not a home, because what these move is `/usr/local/bin`
//! and `/etc`. The engine that runs them is root and the machine is what has
//! run them, not a person.
//!
//! A machine with no record at all is not a machine that has run none of them:
//! it is a machine that has never applied. [`Applied::Never`] is that, and
//! every migration on such a machine is remembered rather than run. A sweep
//! moves what an older manifest left behind, and an apply that never happened
//! left nothing -- so there is nothing for any of them to find, and running
//! them would be reaching into paths this desktop has never written. On a
//! machine where it is installed beside another desktop those paths belong to
//! that one: the first thing these scripts would have done on this laptop is
//! put the other compositor's own config in the attic.
//!
//! The two are told apart by the directory being absent rather than empty,
//! which is the same reading `Held` makes of a file: a record that cannot be
//! read is a fault and says so, because a permission error taken for a fresh
//! machine would mark every migration done on a machine holding everything
//! they sweep.

use crate::Undone;
use console_core_never::Never;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const KEPT: &str = "/var/lib/console/migrations";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    Before(BTreeSet<String>),
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outstanding {
    Run(Vec<String>),
    Remember(Vec<String>),
}

pub fn pending(
    every: &[crate::sweeping::Migration],
    applied: &Applied,
) -> Result<Outstanding, Never> {
    let names: Vec<String> = every.iter().map(|one| one.name.clone()).collect();

    Ok(match applied {
        Applied::Never => Outstanding::Remember(names),
        Applied::Before(done) => {
            Outstanding::Run(names.into_iter().filter(|name| !done.contains(name)).collect())
        }
    })
}

pub fn already(at: &Path) -> Result<Applied, Undone> {
    let entries = match std::fs::read_dir(at) {
        Ok(entries) => entries,
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => return Ok(Applied::Never),
        Err(fault) => return Err(Undone::Listing(at.to_path_buf(), fault)),
    };

    Ok(Applied::Before(
        entries
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect(),
    ))
}

pub fn marker(at: &Path, name: &str) -> Result<PathBuf, Never> {
    Ok(at.join(name))
}

pub fn remember(at: &Path, name: &str) -> Result<(), Undone> {
    std::fs::create_dir_all(at).map_err(|fault| Undone::Holding(at.to_path_buf(), fault))?;

    let Ok(marker) = marker(at, name);

    console_core_atomic_writes::whole(&marker, b"").map_err(Undone::Marking)
}

pub fn attic(when: &str) -> Result<PathBuf, Never> {
    Ok(PathBuf::from(format!("/var/tmp/console-migration-{when}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sweeping::Migration;

    fn one(name: &str) -> Migration {
        Migration { name: name.to_string(), sweeps: BTreeSet::new() }
    }

    fn done(names: &[&str]) -> Applied {
        Applied::Before(names.iter().map(|name| (*name).to_string()).collect())
    }

    fn runs(names: &[&str]) -> Outstanding {
        Outstanding::Run(names.iter().map(|name| (*name).to_string()).collect())
    }

    #[test]
    fn a_machine_that_has_applied_and_run_nothing_has_all_of_them_pending() {
        let every = [one("1780294774.sh"), one("1784767406.sh")];

        let Ok(outstanding) = pending(&every, &done(&[]));

        assert_eq!(outstanding, runs(&["1780294774.sh", "1784767406.sh"]));
    }

    #[test]
    fn one_already_run_is_not_run_again() {
        let every = [one("1780294774.sh"), one("1784767406.sh")];

        let Ok(outstanding) = pending(&every, &done(&["1780294774.sh"]));

        assert_eq!(outstanding, runs(&["1784767406.sh"]));
    }

    #[test]
    fn what_is_pending_comes_back_oldest_first() {
        let every = [one("1780294774.sh"), one("1784767406.sh"), one("1787618700.sh")];

        let Ok(outstanding) = pending(&every, &done(&["1784767406.sh"]));

        assert_eq!(outstanding, runs(&["1780294774.sh", "1787618700.sh"]));
    }

    #[test]
    fn a_machine_that_has_run_them_all_has_nothing_pending() {
        let every = [one("1780294774.sh")];

        let Ok(outstanding) = pending(&every, &done(&["1780294774.sh"]));

        assert_eq!(outstanding, runs(&[]));
    }

    #[test]
    fn a_machine_that_has_never_applied_remembers_every_one_and_runs_none() {
        let every = [one("1780294774.sh"), one("1784767406.sh")];

        let Ok(outstanding) = pending(&every, &Applied::Never);

        assert_eq!(
            outstanding,
            Outstanding::Remember(vec!["1780294774.sh".to_string(), "1784767406.sh".to_string()])
        );
    }

    #[test]
    fn a_marker_directory_that_is_not_there_is_a_machine_that_has_never_applied() {
        assert_eq!(
            already(Path::new("/nowhere/console/migrations")).expect("never applied"),
            Applied::Never
        );
    }

    #[test]
    fn a_marker_directory_that_is_there_and_empty_is_a_machine_that_has_applied() {
        let at = std::env::temp_dir().join(format!("console-done-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&at);

        assert_eq!(
            already(&at).expect("applied"),
            Applied::Before(BTreeSet::new())
        );

        let _ = std::fs::remove_dir_all(&at);
    }
}
