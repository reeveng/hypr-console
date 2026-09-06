//! What is carried, and what the manifest says once it is.

use crate::papers::NOT_PUBLISHED;
use console_never::Never;

pub const FORKS: [&str; 2] = ["/usr/local/bin/hyprsession", "/usr/local/bin/kew"];

pub const FORK_SOURCES: [&str; 0] = [];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fork {
    Yes,
    No,
}

pub fn is_fork(name: &str) -> Result<Fork, Never> {
    let built = FORKS
        .iter()
        .any(|fork| name == fork.trim_start_matches('/') || name.ends_with(fork));
    let source = FORK_SOURCES
        .iter()
        .any(|source| name == *source || name.starts_with(&format!("{source}/")));

    Ok(match built || source {
        true => Fork::Yes,
        false => Fork::No,
    })
}

pub fn carried(tracked: impl IntoIterator<Item = String>) -> Result<Vec<String>, Never> {
    Ok(tracked
        .into_iter()
        .filter(|name| {
            let Ok(fork) = is_fork(name);

            fork == Fork::No
        })
        .collect())
}

pub fn manifest(held: &str) -> Result<String, Never> {
    let kept: Vec<&str> = held
        .lines()
        .filter(|line| !FORKS.contains(&line.trim()))
        .collect();
    Ok(format!("{}\n\n\n{NOT_PUBLISHED}", kept.join("\n").trim_end()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fork_is_known_by_either_path_it_is_named_at() {
        let Ok(under_files) = is_fork("files/usr/local/bin/hyprsession");
        let Ok(installed) = is_fork("usr/local/bin/hyprsession");
        let Ok(ours) = is_fork("files/usr/local/bin/launcher");

        assert_eq!(under_files, Fork::Yes);
        assert_eq!(installed, Fork::Yes);
        assert_eq!(ours, Fork::No);
    }

    #[test]
    fn the_player_is_held_back_the_way_the_other_fork_is() {
        let Ok(under_files) = is_fork("files/usr/local/bin/kew");
        let Ok(installed) = is_fork("usr/local/bin/kew");
        let Ok(source) = is_fork("crates/console-music/src/player.rs");

        assert_eq!(under_files, Fork::Yes);
        assert_eq!(installed, Fork::Yes);
        assert_eq!(source, Fork::No);
    }

    #[test]
    fn the_keyboard_is_not_a_fork_on_either_half() {
        let Ok(under_files) = is_fork("files/usr/local/bin/virtual-keyboard");
        let Ok(installed) = is_fork("usr/local/bin/virtual-keyboard");
        let Ok(source) = is_fork("crates/console-keyboard/src/palette.rs");
        let Ok(manifest) = is_fork("crates/console-keyboard/Cargo.toml");

        assert_eq!(under_files, Fork::No);
        assert_eq!(installed, Fork::No);
        assert_eq!(source, Fork::No);
        assert_eq!(manifest, Fork::No);
    }

    #[test]
    fn the_forks_are_not_carried() {
        let tracked = [
            "justfile",
            "files/usr/local/bin/hyprsession",
            "files/usr/local/bin/kew",
            "crates/console-keyboard/src/palette.rs",
            "docs/checks.md",
        ];
        let Ok(carried) = carried(tracked.map(String::from));

        assert_eq!(
            carried,
            [
                "justfile".to_string(),
                "crates/console-keyboard/src/palette.rs".to_string(),
                "docs/checks.md".to_string(),
            ]
        );
    }

    #[test]
    fn the_manifest_drops_the_forks_and_says_where_they_went() {
        let held = "[files]\n/usr/local/bin/launcher\n/usr/local/bin/hyprsession\n";
        let Ok(written) = manifest(held);
        assert!(!written.contains("[files]\n/usr/local/bin/launcher\n/usr/local/bin/hyprsession"));
        assert!(written.contains("/usr/local/bin/launcher\n"));
        assert!(written.contains("[elsewhere]"));
        assert!(written.contains("/usr/local/bin/hyprsession"));
    }

    #[test]
    fn a_program_the_copy_builds_for_itself_is_left_where_it_is() {
        let held = "[build]\nlauncher\nvirtual-keyboard\n\n[files]\n/usr/local/bin/launcher\n";
        let Ok(written) = manifest(held);
        assert!(written.contains("[build]\nlauncher\nvirtual-keyboard"));
        assert!(!written.contains("not carried"), "nothing here is a fork:\n{written}");
    }
}
