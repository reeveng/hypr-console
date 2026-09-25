//! Which migrations this machine has already run.
//!
//! The record is on the machine and not in the tree, because the question it
//! answers is about the machine. Two devices a release apart share a history
//! and share every migration in it, and what separates them is only how far
//! each has got. A tree that tried to remember would be remembering on behalf
//! of a machine it cannot see.
//!
//! One empty file per migration, named for its moment. A migration was a
//! script once and the markers written then carry its `.sh`, so a marker is
//! read as the moment it begins with and a device that ran the scripts does
//! not run them again as steps. A file rather than a list,
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
use crate::sweeping::{Migration, Moment};
use console_core_never::Never;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const KEPT: &str = "/var/lib/console/migrations";

const SCRIPT: &str = ".sh";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    Before(BTreeSet<Moment>),
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outstanding {
    Run(Vec<Migration>),
    Remember(Vec<Migration>),
}

pub fn pending(every: &[Migration], applied: &Applied) -> Result<Outstanding, Never> {
    Ok(match applied {
        Applied::Never => Outstanding::Remember(every.to_vec()),
        Applied::Before(done) => {
            Outstanding::Run(every.iter().filter(|one| !done.contains(&one.moment)).copied().collect())
        }
    })
}

pub fn moment(marker: &str) -> Result<Option<Moment>, Never> {
    let named = match marker.strip_suffix(SCRIPT) {
        Some(named) => named,
        None => marker,
    };

    Ok(match named.parse::<u64>() {
        Ok(moment) => Some(Moment(moment)),
        Err(_not_a_moment) => None,
    })
}

pub fn already(at: &Path) -> Result<Applied, Undone> {
    let entries = match std::fs::read_dir(at) {
        Ok(entries) => entries,
        Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
            true => return Ok(Applied::Never),
            false => return Err(Undone::Listing(at.to_path_buf(), fault)),
        },
    };

    Ok(Applied::Before(
        entries
            .flatten()
            .filter_map(|entry| {
                let Ok(moment) = moment(&entry.file_name().to_string_lossy());

                moment
            })
            .collect(),
    ))
}

pub fn marker(at: &Path, moment: Moment) -> Result<PathBuf, Never> {
    Ok(at.join(moment.to_string()))
}

pub fn remember(at: &Path, moment: Moment) -> Result<(), Undone> {
    std::fs::create_dir_all(at).map_err(|fault| Undone::Holding(at.to_path_buf(), fault))?;

    let Ok(marker) = marker(at, moment);

    console_core_atomic_writes::whole(&marker, b"").map_err(Undone::Marking)
}

pub fn attic(when: &str) -> Result<PathBuf, Never> {
    Ok(PathBuf::from(format!("/var/tmp/console-migration-{when}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(moment: u64) -> Migration {
        Migration { moment: Moment(moment), says: "", steps: &[] }
    }

    fn done(moments: &[u64]) -> Applied {
        Applied::Before(moments.iter().map(|moment| Moment(*moment)).collect())
    }

    fn runs(moments: &[u64]) -> Outstanding {
        Outstanding::Run(moments.iter().map(|moment| one(*moment)).collect())
    }

    #[test]
    fn a_machine_that_has_applied_and_run_nothing_has_all_of_them_pending() {
        let every = [one(1780294774), one(1784767406)];

        let Ok(outstanding) = pending(&every, &done(&[]));

        assert_eq!(outstanding, runs(&[1780294774, 1784767406]));
    }

    #[test]
    fn one_already_run_is_not_run_again() {
        let every = [one(1780294774), one(1784767406)];

        let Ok(outstanding) = pending(&every, &done(&[1780294774]));

        assert_eq!(outstanding, runs(&[1784767406]));
    }

    #[test]
    fn what_is_pending_comes_back_oldest_first() {
        let every = [one(1780294774), one(1784767406), one(1787618700)];

        let Ok(outstanding) = pending(&every, &done(&[1784767406]));

        assert_eq!(outstanding, runs(&[1780294774, 1787618700]));
    }

    #[test]
    fn a_machine_that_has_run_them_all_has_nothing_pending() {
        let every = [one(1780294774)];

        let Ok(outstanding) = pending(&every, &done(&[1780294774]));

        assert_eq!(outstanding, runs(&[]));
    }

    #[test]
    fn a_machine_that_has_never_applied_remembers_every_one_and_runs_none() {
        let every = [one(1780294774), one(1784767406)];

        let Ok(outstanding) = pending(&every, &Applied::Never);

        assert_eq!(outstanding, Outstanding::Remember(vec![one(1780294774), one(1784767406)]));
    }

    #[test]
    fn a_marker_the_scripts_wrote_is_the_same_migration_as_one_written_now() {
        let Ok(script) = moment("1788609965.sh");
        let Ok(step) = moment("1788609965");

        assert_eq!(script, Some(Moment(1788609965)));
        assert_eq!(step, script);
    }

    #[test]
    fn a_marker_that_is_not_a_moment_is_not_a_migration() {
        let Ok(helper) = moment("attic.sh");
        let Ok(longer) = moment("1788609965.sh.bak");

        assert_eq!(helper, None);
        assert_eq!(longer, None);
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
