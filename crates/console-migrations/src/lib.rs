//! What a machine has to be told, because the manifest cannot say it.
//!
//! `desktop.conf` says what must be on this device. It has never said what must
//! not be, and the engine has no state for a program in `/usr/local/bin` that
//! the manifest no longer names -- so a name that leaves the manifest stays on
//! every machine that ever applied it. Seven `legion-*` units left `[services]`
//! in one commit, and the only thing that stopped that device booting two
//! controller daemons both wanting the pad was somebody writing a sweep by hand
//! for that one rename. What has fallen through since happened to be one-shot
//! commands rather than daemons, which is luck rather than a property.
//!
//! A migration is that sweep, written down. One file per change, named for the
//! moment of the commit that needs it, run once per machine and remembered
//! there. The shape is borrowed from omarchy, which has run it over eighty
//! times: a directory of numbered scripts, a marker per script on the machine
//! that ran it, and a runner that walks them in order. What is different here
//! is that the need can be *derived*. omarchy's authors have to remember to
//! write one; this manifest is a file in git, so the tree can be asked what
//! left it and hold somebody to sweeping it.
//!
//! That is the whole reason this exists as a crate rather than a script:
//! [`unswept`] is the rule, it is arithmetic over four sets, and
//! `tests/every_removal_is_swept.rs` is what puts a repository's real history
//! into it. The engine on the device never asks that question at all -- by then
//! the answer is a file somebody committed.
//!
//! # What a migration looks like
//!
//! ```text
//! # sweeps: /usr/local/bin/console-poke
//! # sweeps: enabled legion-bar.service
//!
//! attic /usr/local/bin/console-poke
//! ```
//!
//! The `sweeps:` lines are the claim, and they are what the gate reads: each
//! one names a thing the machine was left holding, in the words [`holds`] puts
//! it in -- a path for a file, and what holds a unit there for a unit. The rest
//! is a shell script and does the work. Nothing is deleted -- `console-migrate`
//! set that precedent for the rename and its attic is still on the device, which
//! is how anybody can still tell that sweep did what it said.

pub mod done;
pub mod sweeping;

use console_never::Never;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unswept {
    pub holds: String,

    pub section: String,
}

pub fn unswept(
    ever: &BTreeMap<String, String>,
    now: &BTreeSet<String>,
    swept: &BTreeSet<String>,
    on_purpose: &BTreeSet<String>,
) -> Result<Vec<Unswept>, Never> {
    Ok(ever
        .iter()
        .filter(|(holds, _)| !now.contains(*holds))
        .filter(|(holds, _)| !swept.contains(*holds))
        .filter(|(holds, _)| !on_purpose.contains(*holds))
        .map(|(holds, section)| Unswept { holds: holds.clone(), section: section.clone() })
        .collect())
}

pub fn holds(section: &str, entry: &str) -> Result<Option<String>, Never> {
    Ok(match section {
        "[build]" => Some(format!("/usr/local/bin/{entry}")),
        "[files]" => {
            let Ok(whoevers) = whoevers(entry);

            Some(whoevers)
        },
        "[services]" => Some(format!("enabled {entry}")),
        "[masked]" => Some(format!("masked {entry}")),
        _ => None,
    })
}

pub fn whoevers(path: &str) -> Result<String, Never> {
    let Some(rest) = path.strip_prefix("/home/") else { return Ok(path.to_string()) };

    let Some((_, under)) = rest.split_once('/') else { return Ok(path.to_string()) };

    Ok(format!("/home/{USER}/{under}"))
}

pub const USER: &str = "@user@";

pub const SWEPT: [&str; 4] = ["[build]", "[files]", "[services]", "[masked]"];

pub fn outlives(section: &str) -> Result<Outlives, Never> {
    Ok(match SWEPT.contains(&section) {
        true => Outlives::TheManifest,
        false => Outlives::Nothing,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outlives {
    TheManifest,
    Nothing,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ever(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .filter_map(|(section, entry)| {
                let Ok(holds) = holds(section, entry);

                holds.map(|holds| (holds, (*section).to_string()))
            })
            .collect()
    }

    fn now(entries: &[(&str, &str)]) -> BTreeSet<String> {
        entries
            .iter()
            .filter_map(|(section, entry)| {
                let Ok(holds) = holds(section, entry);

                holds
            })
            .collect()
    }

    fn names(said: &[&str]) -> BTreeSet<String> {
        said.iter().map(|word| (*word).to_string()).collect()
    }

    #[test]
    fn a_name_that_left_and_nothing_sweeps_is_the_answer() {
        let ever = ever(&[("[build]", "console-poke"), ("[build]", "launcher")]);
        let now = now(&[("[build]", "launcher")]);
        let Ok(left) = unswept(&ever, &now, &names(&[]), &names(&[]));

        assert_eq!(left.len(), 1);
        assert_eq!(left.first().map(|one| one.holds.as_str()), Some("/usr/local/bin/console-poke"));
    }

    #[test]
    fn a_migration_that_names_it_answers_for_it() {
        let ever = ever(&[("[build]", "console-poke")]);
        let swept = names(&["/usr/local/bin/console-poke"]);

        let Ok(left) = unswept(&ever, &now(&[]), &swept, &names(&[]));

        assert!(left.is_empty());
    }

    #[test]
    fn a_name_left_on_purpose_is_answered_for_too() {
        let ever = ever(&[("[build]", "console-timings")]);
        let said = names(&["/usr/local/bin/console-timings"]);

        let Ok(left) = unswept(&ever, &now(&[]), &names(&[]), &said);

        assert!(left.is_empty());
    }

    #[test]
    fn a_rename_is_the_old_name_and_not_the_new_one() {
        let ever = ever(&[("[build]", "legion-sky"), ("[build]", "console-sky")]);
        let now = now(&[("[build]", "console-sky")]);
        let Ok(left) = unswept(&ever, &now, &names(&[]), &names(&[]));

        assert_eq!(left.first().map(|one| one.holds.as_str()), Some("/usr/local/bin/legion-sky"));
        assert_eq!(left.len(), 1);
    }

    #[test]
    fn a_program_that_became_a_crate_left_nothing_behind() {
        let ever = ever(&[("[files]", "/usr/local/bin/launcher"), ("[build]", "launcher")]);
        let now = now(&[("[build]", "launcher")]);
        let Ok(left) = unswept(&ever, &now, &names(&[]), &names(&[]));

        assert!(left.is_empty());
    }

    #[test]
    fn a_name_that_came_back_is_not_left_anywhere() {
        let ever = ever(&[("[build]", "files-panel")]);
        let now = now(&[("[build]", "files-panel")]);
        let Ok(left) = unswept(&ever, &now, &names(&[]), &names(&[]));

        assert!(left.is_empty());
    }

    #[test]
    fn a_path_under_a_home_is_the_same_file_whoever_the_home_belongs_to() {
        let Ok(theirs) = holds("[files]", "/home/ada/.config/waybar/style.css");
        let Ok(whoevers) = holds("[files]", "/home/@user@/.config/waybar/style.css");

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
        let Ok(enabled) = holds("[services]", "mako.service");
        let Ok(masked) = holds("[masked]", "mako.service");

        assert_ne!(enabled, masked);
    }

    #[test]
    fn what_pacman_already_collects_is_not_swept_here() {
        let Ok(built) = outlives("[build]");
        let Ok(packaged) = outlives("[packages]");
        let Ok(nothing) = holds("[packages]", "grim");

        assert_eq!(built, Outlives::TheManifest);
        assert_eq!(packaged, Outlives::Nothing);
        assert_eq!(nothing, None);
    }
}
