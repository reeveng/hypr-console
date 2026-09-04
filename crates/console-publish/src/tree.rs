//! What is carried, and what the manifest says once it is.

use crate::papers::NOT_PUBLISHED;

/// The programs the copy does not carry, at the paths the manifest names.
///
/// Each is somebody else's program with our changes in it, and a binary
/// published without its source is a licence somebody else wrote being broken
/// on their behalf.
pub const FORKS: [&str; 2] = ["/usr/local/bin/hyprsession", "/usr/local/bin/kew"];

/// The forks whose source is in this tree.
///
/// A fork source is kept under the tree so the binary can be rebuilt, and
/// excluded from the public copy the way the binary is. The exclusion is not
/// about licence -- GPL-3 forbids relicensing, not carrying -- it is a
/// decision not to make this adaptation public: an adaptation for one device,
/// which published would carry an obligation to keep it level with upstream
/// and answer for it. Each entry is the source directory's path under the
/// repository root, with no trailing slash.
///
/// There are none. The keyboard's C source was the only one: the device
/// compiles the Rust keyboard now, so the source it was ported from was no
/// longer the way back to anything and went with the port. The list stays
/// because the next vendored source wants excluding the same way, and the
/// tests over it are written against the list rather than against that entry.
pub const FORK_SOURCES: [&str; 0] = [];

/// Whether a tracked file is one of the forks.
///
/// A file is a fork if it is a built fork at its installed path, or if it
/// lives under a fork's source. The binary is asked of the path both as the
/// manifest writes it and as the tree holds it, which is the same path with
/// `files` in front; the source is asked of the path the tree holds and
/// nothing else, because a source directory has only the one name.
/// Whether a path belongs to a fork rather than to this desktop's own code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fork {
    /// It came from somewhere else, and the copy leaves it out.
    Yes,
    /// It is this repository's own, and the copy carries it.
    No,
}

pub fn is_fork(name: &str) -> Fork {
    let built = FORKS
        .iter()
        .any(|fork| name == fork.trim_start_matches('/') || name.ends_with(fork));
    let source = FORK_SOURCES
        .iter()
        .any(|source| name == *source || name.starts_with(&format!("{source}/")));

    match built || source {
        true => Fork::Yes,
        false => Fork::No,
    }
}

/// Everything carried into the copy.
pub fn carried(tracked: impl IntoIterator<Item = String>) -> Vec<String> {
    tracked.into_iter().filter(|name| is_fork(name) == Fork::No).collect()
}

/// The manifest with the forks taken out of `[files]` and said elsewhere.
///
/// Listed rather than left out, so that a unit starting a program nothing
/// installs stays the failure it should be everywhere else.
pub fn manifest(held: &str) -> String {
    let kept: Vec<&str> = held
        .lines()
        .filter(|line| !FORKS.contains(&line.trim()))
        .collect();
    format!("{}\n\n\n{NOT_PUBLISHED}", kept.join("\n").trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fork_is_known_by_either_path_it_is_named_at() {
        assert_eq!(is_fork("files/usr/local/bin/hyprsession"), Fork::Yes);
        assert_eq!(is_fork("usr/local/bin/hyprsession"), Fork::Yes);
        assert_eq!(is_fork("files/usr/local/bin/launcher"), Fork::No);
    }

    /// The player is a fork the same way the session restorer is.
    ///
    /// It arrived later and by a different road -- the manifest carries it so
    /// that the machine ends up with the fork rather than the packaged
    /// program -- so it is asked for by name here, and not left to the
    /// hyprsession case standing in for both.
    #[test]
    fn the_player_is_held_back_the_way_the_other_fork_is() {
        assert_eq!(is_fork("files/usr/local/bin/kew"), Fork::Yes);
        assert_eq!(is_fork("usr/local/bin/kew"), Fork::Yes);
        assert_eq!(is_fork("crates/console-music/src/player.rs"), Fork::No);
    }

    /// The keyboard is this repository's own on both halves now.
    ///
    /// It used to be two things held back: a compiled binary carried at the
    /// path the unit starts, and the vendored C it was built from. The device
    /// compiles the Rust one, and the C went when it stopped being a way back
    /// to anything, so neither half is excluded and the copy carries the lot.
    #[test]
    fn the_keyboard_is_not_a_fork_on_either_half() {
        assert_eq!(is_fork("files/usr/local/bin/virtual-keyboard"), Fork::No);
        assert_eq!(is_fork("usr/local/bin/virtual-keyboard"), Fork::No);
        assert_eq!(is_fork("crates/keyboard/src/palette.rs"), Fork::No);
        assert_eq!(is_fork("crates/keyboard/Cargo.toml"), Fork::No);
    }

    #[test]
    fn the_forks_are_not_carried() {
        let tracked = [
            "justfile",
            "files/usr/local/bin/hyprsession",
            "files/usr/local/bin/kew",
            "crates/keyboard/src/palette.rs",
            "docs/checks.md",
        ];
        assert_eq!(
            carried(tracked.map(String::from)),
            [
                "justfile".to_string(),
                "crates/keyboard/src/palette.rs".to_string(),
                "docs/checks.md".to_string(),
            ]
        );
    }

    #[test]
    fn the_manifest_drops_the_forks_and_says_where_they_went() {
        let held = "[files]\n/usr/local/bin/launcher\n/usr/local/bin/hyprsession\n";
        let written = manifest(held);
        assert!(!written.contains("[files]\n/usr/local/bin/launcher\n/usr/local/bin/hyprsession"));
        assert!(written.contains("/usr/local/bin/launcher\n"));
        assert!(written.contains("[elsewhere]"));
        // Named in the note, so a unit starting one still fails loudly.
        assert!(written.contains("/usr/local/bin/hyprsession"));
    }

    /// The keyboard is named in the note the same way, so a device applying
    /// the published manifest is told what it has not got.
    #[test]
    fn a_program_the_copy_builds_for_itself_is_left_where_it_is() {
        // The keyboard is in `[build]` now and not in `[files]`, so the filter
        // has nothing to take out and must not invent anything: a copy that
        // moved it to the fork list would tell a reader to go and find a
        // program that is right there in the tree.
        let held = "[build]\nlauncher\nvirtual-keyboard\n\n[files]\n/usr/local/bin/launcher\n";
        let written = manifest(held);
        assert!(written.contains("[build]\nlauncher\nvirtual-keyboard"));
        assert!(!written.contains("not carried"), "nothing here is a fork:\n{written}");
    }
}
