//! What is carried, and what the manifest says once it is.
//!
//! A fork is held back when it travels as somebody else's build. Nothing does
//! now. `kew` was the last of them and is gone -- what plays music is
//! `console-music-player`, built here like everything else -- so [`FORKS`] is
//! empty, and it is empty rather than deleted because the rule it carries is
//! the one thing that would have to be rewritten from memory the next time a
//! binary somebody else built has to ride along. Whether an empty list earns
//! its machinery is a fair question and the answer is not obvious enough to
//! settle by deleting it in the same commit that emptied it.
//!
//! `FORK_SOURCES` was the other half of this and is gone. It held back
//! `crates/console-resume` for the few days that crate was a fork's source
//! sitting in the workspace, and it was the right shape for exactly as long as
//! the question was "whose release is this". It stopped being: the port is most
//! of that crate now, and a workspace whose `members` and `Cargo.lock` name a
//! crate the copy does not carry is a copy that will not resolve, let alone
//! build. Nothing published between the crate landing and the exclusion going,
//! so that never happened to anybody.
//!
//! `VENDORED` is what stayed behind, and it asks a different question. Not
//! "is this published" -- it is -- but "does the licence it came with travel
//! with it", which is the one thing GPL-3.0 actually asks of a copy. It is the
//! list `the_forks.rs` walks to check that.
//!
//! An empty list is a rule nothing presses, so the list is handed in rather
//! than read: [`is_fork`], [`carried`] and [`manifest`] each pass [`FORKS`] to
//! the arithmetic under them, and the checks pass kew's old path. What the next
//! binary will need is the matching -- a path named with a leading slash in the
//! manifest and without one under `files/` -- and that is the half that would
//! otherwise be believed rather than known.
//!
//! What is left of the holding back is a rule about binaries, which is the rule
//! it always was.
//! `crates/console-manifest-publish/src/lib.rs` argues for the change and
//! `papers/forks.md` names upstream, which is the part that actually matters to
//! whoever wrote it.

use crate::papers::NOT_PUBLISHED;
use console_core_never::Never;

pub const FORKS: [&str; 0] = [];

pub const VENDORED: [&str; 1] = ["crates/console-resume"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fork {
    Yes,
    No,
}

pub fn is_fork(name: &str) -> Result<Fork, Never> {
    among(&FORKS, name)
}

fn among(forks: &[&str], name: &str) -> Result<Fork, Never> {
    let built = forks
        .iter()
        .any(|fork| name == fork.trim_start_matches('/') || name.ends_with(fork));

    Ok(match built {
        true => Fork::Yes,
        false => Fork::No,
    })
}

pub fn carried(tracked: impl IntoIterator<Item = String>) -> Result<Vec<String>, Never> {
    carrying(&FORKS, tracked)
}

fn carrying(
    forks: &[&str],
    tracked: impl IntoIterator<Item = String>,
) -> Result<Vec<String>, Never> {
    Ok(tracked
        .into_iter()
        .filter(|name| {
            let Ok(fork) = among(forks, name);

            fork == Fork::No
        })
        .collect())
}

pub fn manifest(held: &str) -> Result<String, Never> {
    written(&FORKS, held)
}

fn written(forks: &[&str], held: &str) -> Result<String, Never> {
    let kept: Vec<&str> = held
        .lines()
        .filter(|line| !forks.contains(&line.trim()))
        .collect();
    Ok(format!("{}\n\n\n{NOT_PUBLISHED}", kept.join("\n").trim_end()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: [&str; 1] = ["/usr/local/bin/kew"];

    #[test]
    fn a_fork_is_known_by_either_path_it_is_named_at() {
        let Ok(under_files) = among(&ONE, "files/usr/local/bin/kew");
        let Ok(installed) = among(&ONE, "usr/local/bin/kew");
        let Ok(ours) = among(&ONE, "files/usr/local/bin/launcher");

        assert_eq!(under_files, Fork::Yes);
        assert_eq!(installed, Fork::Yes);
        assert_eq!(ours, Fork::No);
    }

    #[test]
    fn nothing_this_tree_holds_is_somebody_elses_build_today() {
        let Ok(kew) = is_fork("files/usr/local/bin/kew");
        let Ok(keyboard) = is_fork("files/usr/local/bin/virtual-keyboard");

        assert_eq!(kew, Fork::No, "kew is a crate now and the list should be empty");
        assert_eq!(keyboard, Fork::No);
    }

    #[test]
    fn the_crate_that_talks_to_the_player_is_not_the_player() {
        let Ok(source) = is_fork("crates/console-music-panel/src/player.rs");

        assert_eq!(source, Fork::No);
    }

    #[test]
    fn the_keyboard_is_not_a_fork_on_either_half() {
        let Ok(under_files) = is_fork("files/usr/local/bin/virtual-keyboard");
        let Ok(installed) = is_fork("usr/local/bin/virtual-keyboard");
        let Ok(source) = is_fork("crates/console-input-keyboard/src/palette.rs");
        let Ok(manifest) = is_fork("crates/console-input-keyboard/Cargo.toml");

        assert_eq!(under_files, Fork::No);
        assert_eq!(installed, Fork::No);
        assert_eq!(source, Fork::No);
        assert_eq!(manifest, Fork::No);
    }

    #[test]
    fn a_fork_the_workspace_builds_from_source_is_carried_like_any_other_crate() {
        let Ok(manifest) = is_fork("crates/console-resume/Cargo.toml");
        let Ok(source) = is_fork("crates/console-resume/src/session.rs");
        let Ok(licence) = is_fork("crates/console-resume/LICENSE");

        assert_eq!(manifest, Fork::No);
        assert_eq!(source, Fork::No);
        assert_eq!(
            licence,
            Fork::No,
            "the licence most of all: a GPL crate published without it is the one thing that \
             would be somebody else's to complain about"
        );
    }

    #[test]
    fn the_forks_are_not_carried() {
        let tracked = [
            "justfile",
            "files/usr/local/bin/kew",
            "crates/console-resume/src/session.rs",
            "crates/console-input-keyboard/src/palette.rs",
            "docs/checks.md",
        ];
        let Ok(carried) = carrying(&ONE, tracked.map(String::from));

        assert_eq!(
            carried,
            [
                "justfile".to_string(),
                "crates/console-resume/src/session.rs".to_string(),
                "crates/console-input-keyboard/src/palette.rs".to_string(),
                "docs/checks.md".to_string(),
            ]
        );
    }

    #[test]
    fn the_manifest_drops_the_forks_and_says_where_they_went() {
        let held = "[files]\n/usr/local/bin/launcher\n/usr/local/bin/kew\n";
        let Ok(written) = written(&ONE, held);
        assert!(!written.contains("[files]\n/usr/local/bin/launcher\n/usr/local/bin/kew"));
        assert!(written.contains("/usr/local/bin/launcher\n"));
        assert!(written.contains("[elsewhere]"));
        assert!(written.contains("/usr/local/bin/kew"));
    }

    #[test]
    fn a_program_the_copy_builds_for_itself_is_left_where_it_is() {
        let held = "[build]\nlauncher\nvirtual-keyboard\n\n[files]\n/usr/local/bin/launcher\n";
        let Ok(written) = manifest(held);
        assert!(written.contains("[build]\nlauncher\nvirtual-keyboard"));
        assert!(!written.contains("not carried"), "nothing here is a fork:\n{written}");
    }
}
