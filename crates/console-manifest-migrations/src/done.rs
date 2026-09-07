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

use console_core_never::Never;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const KEPT: &str = "/var/lib/console/migrations";

pub fn pending(
    every: &[crate::sweeping::Migration],
    done: &BTreeSet<String>,
) -> Result<Vec<String>, Never> {
    Ok(every
        .iter()
        .map(|one| one.name.clone())
        .filter(|name| !done.contains(name))
        .collect())
}

pub fn already(at: &Path) -> Result<BTreeSet<String>, Never> {
    let Ok(entries) = std::fs::read_dir(at) else { return Ok(BTreeSet::new()) };

    Ok(entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect())
}

pub fn marker(at: &Path, name: &str) -> Result<PathBuf, Never> {
    Ok(at.join(name))
}

pub fn remember(at: &Path, name: &str) -> Result<(), String> {
    std::fs::create_dir_all(at).map_err(|fault| format!("{}: {fault}", at.display()))?;

    let Ok(marker) = marker(at, name);

    std::fs::write(&marker, b"").map_err(|fault| format!("{}: {fault}", marker.display()))
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

    fn done(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    #[test]
    fn a_machine_that_has_run_nothing_has_all_of_them_pending() {
        let every = [one("1780294774.sh"), one("1784767406.sh")];

        let Ok(pending) = pending(&every, &done(&[]));

        assert_eq!(pending, vec!["1780294774.sh", "1784767406.sh"]);
    }

    #[test]
    fn one_already_run_is_not_run_again() {
        let every = [one("1780294774.sh"), one("1784767406.sh")];

        let Ok(pending) = pending(&every, &done(&["1780294774.sh"]));

        assert_eq!(pending, vec!["1784767406.sh"]);
    }

    #[test]
    fn what_is_pending_comes_back_oldest_first() {
        let every = [one("1780294774.sh"), one("1784767406.sh"), one("1787618700.sh")];

        let Ok(pending) = pending(&every, &done(&["1784767406.sh"]));

        assert_eq!(pending, vec!["1780294774.sh", "1787618700.sh"]);
    }

    #[test]
    fn a_machine_that_has_run_them_all_has_nothing_pending() {
        let every = [one("1780294774.sh")];

        let Ok(pending) = pending(&every, &done(&["1780294774.sh"]));

        assert!(pending.is_empty());
    }

    #[test]
    fn a_marker_directory_that_is_not_there_is_a_machine_that_has_run_nothing() {
        let Ok(already) = already(Path::new("/nowhere/console/migrations"));

        assert!(already.is_empty());
    }
}
