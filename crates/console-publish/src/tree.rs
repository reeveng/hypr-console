//! What is carried, and what the manifest says once it is.

use crate::papers::NOT_PUBLISHED;

/// The two programs the copy does not carry, at the paths the manifest names.
///
/// Each is somebody else's program with our changes in it, and a binary
/// published without its source is a licence somebody else wrote being broken
/// on their behalf.
pub const FORKS: [&str; 2] = ["/usr/local/bin/wvkbd-mobintl", "/usr/local/bin/hyprsession"];

/// Whether a tracked file is one of the forks.
///
/// Asked of the path both as the manifest writes it and as the tree holds it,
/// which is the same path with `files` in front.
pub fn is_fork(name: &str) -> bool {
    FORKS
        .iter()
        .any(|fork| name == fork.trim_start_matches('/') || name.ends_with(fork))
}

/// Everything carried into the copy.
pub fn carried(tracked: impl IntoIterator<Item = String>) -> Vec<String> {
    tracked.into_iter().filter(|name| !is_fork(name)).collect()
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
        assert!(is_fork("files/usr/local/bin/hyprsession"));
        assert!(is_fork("usr/local/bin/hyprsession"));
        assert!(!is_fork("files/usr/local/bin/launcher"));
    }

    #[test]
    fn the_forks_are_not_carried() {
        let tracked = [
            "Makefile",
            "files/usr/local/bin/wvkbd-mobintl",
            "docs/checks.md",
        ];
        assert_eq!(
            carried(tracked.map(String::from)),
            ["Makefile".to_string(), "docs/checks.md".to_string()]
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
}
