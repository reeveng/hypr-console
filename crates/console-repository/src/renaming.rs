//! Changing a name everywhere it is written, in one pass that cannot half-do it.
//!
//! A name here is never in one place. `console-music` was a directory under
//! `crates/`, a package and a lib in two `Cargo.toml` files, an import in every
//! crate that used it, a line of prose in three documents, and a `.desktop`
//! file whose *filename* nine lines of `mimeapps.list` pointed at. Renaming it
//! by hand found seven of those and missed the eighth, and the eighth was the
//! one that would have left two Music rows in the menu on every machine that
//! applied it.
//!
//! Two things make that easy to get wrong and both are handled here rather than
//! remembered.
//!
//! The first is that names contain names. Sweeping `console-music` into
//! `console-music-panel` across the tree turns `console-music-player` into
//! `console-music-panel-player`, and the damage reads like a typo somebody made
//! on purpose. So a match only counts when what follows it cannot continue a
//! name: a letter, a digit, `-` and `_` all stop it, and everything else --
//! a dot, a slash, a quote, the end of a line -- lets it through. That is what
//! keeps `console-music.desktop` in and `console-music-player` out.
//!
//! The second is that a path installed on the device is not a name that can
//! simply change. An apply installs what the manifest names and has never
//! removed what it stopped naming, so a renamed file arrives beside the old one
//! rather than instead of it. Every moved path under `files/` is therefore
//! collected, and a migration is written claiming each of them, because the
//! sweep gate reads those claims and a rename with no migration is exactly what
//! it is there to catch.
//!
//! What is deliberately not written is the reason. A migration in this tree
//! argues for itself -- what was left behind, why it cannot stay, what happens
//! to a machine that misses it -- and no tool can write that. So the stub
//! carries [`UNSAID`] where the argument goes, and
//! `console-manifest-migrations` refuses a migration still saying it. The tool
//! does the part that is mechanical and refuses to pretend it did the part that
//! is not.

use console_core_external_programs::Program;
use console_core_never::Never;
use std::path::{Path, PathBuf};

pub const UNSAID: &str = "WHY THIS CANNOT BE LEFT ON THE DEVICE IS NOT WRITTEN YET";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    pub from: PathBuf,
    pub to: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Continues {
    Yes,
    No,
}

pub fn continues(after: Option<char>) -> Result<Continues, Never> {
    Ok(match after {
        None => Continues::No,
        Some(held) => match held.is_alphanumeric() || held == '-' || held == '_' {
            true => Continues::Yes,
            false => Continues::No,
        },
    })
}

pub fn swept(said: &str, old: &str, new: &str) -> Result<String, Never> {
    let mut out = String::new();
    let mut rest = said;

    while let Some(at) = rest.find(old) {
        let before = rest.get(..at).unwrap_or("");
        let from = at.saturating_add(old.len());
        let after = rest.get(from..).unwrap_or("");
        let Ok(continues) = continues(after.chars().next());

        out.push_str(before);

        match continues {
            Continues::Yes => out.push_str(old),
            Continues::No => out.push_str(new),
        }

        rest = after;
    }

    out.push_str(rest);

    Ok(out)
}

pub fn spellings(old: &str, new: &str) -> Result<Vec<(String, String)>, Never> {
    let under = old.replace('-', "_");
    let mut held = vec![(old.to_string(), new.to_string())];

    match under == old {
        true => {},
        false => held.push((under, new.replace('-', "_"))),
    }

    Ok(held)
}

pub fn through(said: &str, old: &str, new: &str) -> Result<String, Never> {
    let Ok(spellings) = spellings(old, new);
    let mut held = said.to_string();

    for (old, new) in spellings {
        let Ok(swept) = swept(&held, &old, &new);

        held = swept;
    }

    Ok(held)
}

pub fn tracked(root: &Path) -> Result<Vec<PathBuf>, Never> {
    let Ok(mut asking) = Program::Git.command();

    asking.arg("-C").arg(root).args(["ls-files", "-z"]);

    let done = match asking.output() {
        Ok(done) => done,
        Err(fault) => {
            eprintln!("console-rename: asking git what is tracked: {fault}");

            return Ok(Vec::new());
        },
    };

    let said = String::from_utf8_lossy(&done.stdout).to_string();

    Ok(said
        .split('\0')
        .filter(|name| !name.is_empty())
        .map(|name| root.join(name))
        .collect())
}

pub fn renamed(at: &Path, old: &str, new: &str) -> Result<Option<PathBuf>, Never> {
    let name = match at.file_name().and_then(|name| name.to_str()) {
        Some(name) => name,
        None => return Ok(None),
    };

    let Ok(swept) = through(name, old, new);

    Ok(match swept == name {
        true => None,
        false => Some(at.with_file_name(swept)),
    })
}

pub fn installed(root: &Path, at: &Path) -> Result<Option<String>, Never> {
    let under = root.join("files");

    Ok(match at.strip_prefix(&under) {
        Ok(rest) => Some(format!("/{}", rest.display())),
        Err(_it_is_not_installed_anywhere) => None,
    })
}

pub fn stub(sweeping: &[String]) -> Result<String, Never> {
    let claims: String =
        sweeping.iter().map(|path| format!("# sweeps: {path}\n")).collect();
    let attic: String =
        sweeping.iter().map(|path| format!("console-attic {path}\n")).collect();

    Ok(format!(
        "# {UNSAID}\n\
         #\n\
         # A path below left the manifest under one name and arrived under another.\n\
         # An apply installs what it is told and removes nothing, so both are on\n\
         # every machine that applied the old one. Say here what that costs -- what\n\
         # reads the old name, what a person would see with two of them, and why the\n\
         # one being swept cannot simply be left.\n\
         #\n\
         {claims}\n\
         echo \"sweeping what the rename left behind\"\n\
         \n\
         {attic}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_inside_a_longer_name_is_left_alone() {
        let Ok(said) = swept("console-music-player", "console-music", "console-music-panel");

        assert_eq!(said, "console-music-player");
    }

    #[test]
    fn a_name_that_ends_is_swept_however_it_ends() {
        let Ok(file) = swept("console-music.desktop", "console-music", "console-music-panel");
        let Ok(path) = swept("crates/console-music/src", "console-music", "console-music-panel");
        let Ok(quoted) = swept("\"console-music\"", "console-music", "console-music-panel");
        let Ok(ends) = swept("uses console-music", "console-music", "console-music-panel");

        assert_eq!(file, "console-music-panel.desktop");
        assert_eq!(path, "crates/console-music-panel/src");
        assert_eq!(quoted, "\"console-music-panel\"");
        assert_eq!(ends, "uses console-music-panel");
    }

    #[test]
    fn both_spellings_of_a_name_move_together() {
        let Ok(said) = through(
            "use console_music::player; // console-music",
            "console-music",
            "console-music-panel",
        );

        assert_eq!(said, "use console_music_panel::player; // console-music-panel");
    }

    #[test]
    fn the_underscore_spelling_of_a_longer_name_is_left_alone_too() {
        let Ok(said) = through("console_music_player", "console-music", "console-music-panel");

        assert_eq!(said, "console_music_player");
    }

    #[test]
    fn a_name_with_no_underscore_spelling_is_swept_once() {
        let Ok(spellings) = spellings("kew", "music-player");

        assert_eq!(spellings.len(), 1);
    }

    #[test]
    fn a_file_under_files_says_where_it_lands_on_the_device() {
        let root = Path::new("/tree");
        let Ok(said) =
            installed(root, Path::new("/tree/files/usr/share/applications/a.desktop"));
        let Ok(elsewhere) = installed(root, Path::new("/tree/crates/a/src/lib.rs"));

        assert_eq!(said.as_deref(), Some("/usr/share/applications/a.desktop"));
        assert_eq!(elsewhere, None);
    }

    #[test]
    fn a_stub_claims_every_path_it_moved_and_says_the_reason_is_missing() {
        let Ok(said) = stub(&["/usr/share/applications/a.desktop".to_string()]);

        assert!(said.contains("# sweeps: /usr/share/applications/a.desktop"));
        assert!(said.contains("console-attic /usr/share/applications/a.desktop"));
        assert!(said.contains(UNSAID));
    }
}
