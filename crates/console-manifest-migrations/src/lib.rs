//! What a machine has to be told, because the manifest cannot say it.
//!
//! `desktop.conf` says what must be on this device. It has never said what must
//! not be, and the engine has no state for a program in `/usr/local/bin` that
//! the manifest no longer names -- so a name that leaves the manifest stays on
//! every machine that ever applied it. Seven `legion-*` units left `[services]`
//! in one commit, and the only thing that stopped that device booting two
//! controller daemons both wanting the pad was someone writing a sweep by hand
//! for that one rename. What has fallen through since happened to be one-shot
//! commands rather than daemons, which is luck rather than a property.
//!
//! A migration is that sweep, written down. One module per change, named for
//! the moment of the commit that needs it, run once per machine and remembered
//! there. The shape is borrowed from omarchy, which has run it over eighty
//! times: a directory of numbered scripts, a marker per script on the machine
//! that ran it, and a runner that walks them in order. What is different here
//! is that the need can be *derived*. omarchy's authors have to remember to
//! write one; this manifest is a file in git, so the tree can be asked what
//! left it and hold someone to sweeping it.
//!
//! That is the whole reason this exists as a crate rather than a script:
//! [`unswept`] is the rule, it is arithmetic over sets, and
//! `tests/every_removal_is_swept.rs` is what puts a repository's real history
//! into it.
//!
//! The rule now covers less than it did. Since [`RECORDED_SINCE`] every apply
//! is written down with the commit it came from, and the engine takes back by
//! itself what those commits placed and today's manifest does not name -- its
//! `pruning` module is that. So what still needs a sweep written by hand is a
//! name that left before any machine kept the record, and whatever a removal
//! costs beyond the thing the line named, which no manifest says.
//!
//! # What a migration looks like
//!
//! ```text
//! pub const MIGRATION: Migration = Migration {
//!     moment: Moment(1790200080),
//!     says: "sweeping the thumbnail maker under its old name",
//!     steps: &[Step::Attic("/usr/local/bin/files-thumbs")],
//! };
//! ```
//!
//! What the steps move and disable is the claim, and it is what the gate reads:
//! each names a thing the machine was left holding, in the words [`holds`] puts
//! it in. The engine carries the steps out. Nothing is deleted --
//! `console-migrate` set that precedent for the rename and its attic is still on
//! the device, which is how anyone can still tell that sweep did what it said.

pub mod done;
pub mod history;
pub mod sweeping;

use console_core_never::Never;
use console_core_internal_programs::EXECUTABLE_DIRECTORY;
use console_core_words::Words;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Undone {
    Listing(PathBuf, std::io::Error),
    Holding(PathBuf, std::io::Error),
    Marking(console_core_atomic_writes::Unwritten),
}

impl fmt::Display for Undone {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Undone::Listing(at, fault) => write!(to, "{}: {fault}", at.display()),
            Undone::Holding(at, fault) => write!(to, "{}: {fault}", at.display()),
            Undone::Marking(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for Undone {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unswept {
    pub holds: String,

    pub section: Section,
}

pub fn unswept(
    ever: &BTreeMap<String, Section>,
    now: &BTreeSet<String>,
    swept: &BTreeSet<String>,
    on_purpose: &BTreeSet<String>,
    recorded: &BTreeSet<String>,
) -> Result<Vec<Unswept>, Never> {
    Ok(ever
        .iter()
        .filter(|(holds, _)| !now.contains(*holds))
        .filter(|(holds, _)| !swept.contains(*holds))
        .filter(|(holds, _)| !on_purpose.contains(*holds))
        .filter(|(holds, _)| !recorded.contains(*holds))
        .map(|(holds, section)| Unswept { holds: holds.clone(), section: *section })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Words)]
pub enum Section {
    #[words(name = "packages")]
    Packages,
    #[words(name = "build")]
    Build,
    #[words(name = "files")]
    Files,
    #[words(name = "services")]
    Services,
    #[words(name = "masked")]
    Masked,
    #[words(name = "elsewhere")]
    Elsewhere,
}

impl Section {
    pub const EVERY: [Section; 5] = [
        Section::Packages,
        Section::Build,
        Section::Files,
        Section::Services,
        Section::Masked,
    ];
}

pub fn holds(section: Section, entry: &str) -> Result<Option<String>, Never> {
    Ok(match section {
        Section::Build => Some(format!("{EXECUTABLE_DIRECTORY}/{entry}")),
        Section::Files => {
            let Ok(whoevers) = whoevers(entry);

            Some(whoevers)
        },
        Section::Services => Some(format!("enabled {entry}")),
        Section::Masked => Some(format!("masked {entry}")),
        Section::Packages | Section::Elsewhere => None,
    })
}

pub fn whoevers(path: &str) -> Result<String, Never> {
    let rest = match path.strip_prefix("/home/") {
        Some(rest) => rest,
        None => return Ok(path.to_string()),
    };

    let (_taken, under) = match rest.split_once('/') {
        Some((_taken, under)) => (_taken, under),
        None => return Ok(path.to_string()),
    };

    Ok(format!("/home/{USER}/{under}"))
}

pub const USER: &str = "@user@";

pub const RECORDED_SINCE: u64 = 1_790_123_959;

pub fn recording_state(committed: u64) -> Result<Recorded, Never> {
    Ok(match committed >= RECORDED_SINCE {
        true => Recorded::Yes,
        false => Recorded::No,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    Yes,
    No,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ever(entries: &[(Section, &str)]) -> Result<BTreeMap<String, Section>, Never> {
        Ok(entries
            .iter()
            .filter_map(|(section, entry)| {
                let Ok(holds) = holds(*section, entry);

                holds.map(|holds| (holds, *section))
            })
            .collect())
    }

    fn now(entries: &[(Section, &str)]) -> Result<BTreeSet<String>, Never> {
        Ok(entries
            .iter()
            .filter_map(|(section, entry)| {
                let Ok(holds) = holds(*section, entry);

                holds
            })
            .collect())
    }

    fn names(said: &[&str]) -> Result<BTreeSet<String>, Never> {
        Ok(said.iter().map(|word| (*word).to_string()).collect())
    }

    #[test]
    fn a_name_that_left_and_nothing_sweeps_is_the_answer() {
        let Ok(ever) = ever(&[(Section::Build, "console-poke"), (Section::Build, "launcher")]);
        let Ok(now) = now(&[(Section::Build, "launcher")]);
        let Ok(left) = unswept(&ever, &now, &BTreeSet::new(), &BTreeSet::new(), &BTreeSet::new());

        assert_eq!(left.len(), 1);
        assert_eq!(left.first().map(|one| one.holds.as_str()), Some("/usr/local/bin/console-poke"));
    }

    #[test]
    fn a_migration_that_names_it_answers_for_it() {
        let Ok(ever) = ever(&[(Section::Build, "console-poke")]);
        let Ok(swept) = names(&["/usr/local/bin/console-poke"]);

        let Ok(left) = unswept(&ever, &BTreeSet::new(), &swept, &BTreeSet::new(), &BTreeSet::new());

        assert!(left.is_empty());
    }

    #[test]
    fn a_name_left_on_purpose_is_answered_for_too() {
        let Ok(ever) = ever(&[(Section::Build, "console-timings")]);
        let Ok(said) = names(&["/usr/local/bin/console-timings"]);

        let Ok(left) = unswept(&ever, &BTreeSet::new(), &BTreeSet::new(), &said, &BTreeSet::new());

        assert!(left.is_empty());
    }

    #[test]
    fn a_rename_is_the_old_name_and_not_the_new_one() {
        let Ok(ever) = ever(&[(Section::Build, "legion-sky"), (Section::Build, "console-wallpaper")]);
        let Ok(now) = now(&[(Section::Build, "console-wallpaper")]);
        let Ok(left) = unswept(&ever, &now, &BTreeSet::new(), &BTreeSet::new(), &BTreeSet::new());

        assert_eq!(left.first().map(|one| one.holds.as_str()), Some("/usr/local/bin/legion-sky"));
        assert_eq!(left.len(), 1);
    }

    #[test]
    fn a_name_carried_since_the_generations_began_is_the_engines_to_take() {
        let Ok(ever) = ever(&[(Section::Build, "files-panel")]);
        let Ok(recorded) = names(&["/usr/local/bin/files-panel"]);

        let Ok(left) = unswept(&ever, &BTreeSet::new(), &BTreeSet::new(), &BTreeSet::new(), &recorded);

        assert!(left.is_empty());
    }

    #[test]
    fn a_commit_before_the_generations_began_is_one_no_machine_remembers() {
        let Ok(before) = recording_state(RECORDED_SINCE.saturating_sub(1));
        let Ok(since) = recording_state(RECORDED_SINCE);

        assert_eq!(before, Recorded::No);
        assert_eq!(since, Recorded::Yes);
    }

    #[test]
    fn a_program_that_became_a_crate_left_nothing_behind() {
        let Ok(ever) = ever(&[(Section::Files, "/usr/local/bin/launcher"), (Section::Build, "launcher")]);
        let Ok(now) = now(&[(Section::Build, "launcher")]);
        let Ok(left) = unswept(&ever, &now, &BTreeSet::new(), &BTreeSet::new(), &BTreeSet::new());

        assert!(left.is_empty());
    }

    #[test]
    fn a_name_that_came_back_is_not_left_anywhere() {
        let Ok(ever) = ever(&[(Section::Build, "files-panel")]);
        let Ok(now) = now(&[(Section::Build, "files-panel")]);
        let Ok(left) = unswept(&ever, &now, &BTreeSet::new(), &BTreeSet::new(), &BTreeSet::new());

        assert!(left.is_empty());
    }

    #[test]
    fn a_path_under_a_home_is_the_same_file_whoever_the_home_belongs_to() {
        let Ok(theirs) = holds(Section::Files, "/home/ada/.config/console/palette.css");
        let Ok(whoevers) = holds(Section::Files, "/home/@user@/.config/console/palette.css");

        assert_eq!(theirs, whoevers);
    }

    #[test]
    fn a_path_outside_a_home_is_left_alone() {
        let Ok(outside) = whoevers("/usr/local/bin/console-say");
        let Ok(a_home) = whoevers("/home/ada");

        assert_eq!(outside, "/usr/local/bin/console-say");
        assert_eq!(a_home, "/home/ada");
    }

    #[test]
    fn a_unit_enabled_and_a_unit_masked_are_two_things_to_hold() {
        let Ok(enabled) = holds(Section::Services, "mako.service");
        let Ok(masked) = holds(Section::Masked, "mako.service");

        assert_ne!(enabled, masked);
    }

    #[test]
    fn what_pacman_already_collects_is_not_swept_here() {
        let Ok(nothing) = holds(Section::Packages, "grim");

        assert_eq!(nothing, None);
    }
}
