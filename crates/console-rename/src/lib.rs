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
//! `console-music` across the tree turns `console-music-player` into
//! `console-music-panel-player`, and the damage reads like a typo someone made
//! on purpose. So a match only counts when what follows it cannot continue a
//! name: a letter, a digit, `-` and `_` all stop it, and everything else --
//! a dot, a slash, a quote, the end of a line -- lets it through. That is what
//! keeps `console-music.desktop` in and `console-music-player` out.
//!
//! The second is that a path installed on the device is not a name that can
//! simply change. An apply installs what the manifest names and has never
//! removed what it stopped naming, so a renamed file arrives beside the old one
//! rather than instead of it. Every moved path under `files/` is therefore
//! collected, and a migration is written moving each of them to the attic,
//! because the sweep gate reads what a migration moves as what it claims and a
//! rename with no migration is exactly what it is there to catch. The module is
//! written and not listed: listing it in `history.rs` is the moment somebody
//! has read it, and a module left unlisted fails a test there.
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
use console_manifest_migrations::history::{CRATE, DIRECTORY, PREFIX};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Renaming<'a> {
    pub old: &'a str,
    pub new: &'a str,
}

pub fn swept(said: &str, renaming: Renaming<'_>) -> Result<String, Never> {
    let Renaming { old, new } = renaming;
    let mut out = String::new();
    let mut rest = said;

    while let Some((before, after)) = rest.split_once(old) {
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

pub fn spellings(renaming: Renaming<'_>) -> Result<Vec<(String, String)>, Never> {
    let Renaming { old, new } = renaming;
    let under = old.replace('-', "_");
    let mut held = vec![(old.to_string(), new.to_string())];

    match under == old {
        true => {},
        false => held.push((under, new.replace('-', "_"))),
    }

    Ok(held)
}

pub fn through(said: &str, renaming: Renaming<'_>) -> Result<String, Never> {
    let Ok(spellings) = spellings(renaming);
    let mut held = said.to_string();

    for (old, new) in spellings {
        let Ok(swept) = swept(&held, Renaming { old: &old, new: &new });

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

pub fn renamed(at: &Path, renaming: Renaming<'_>) -> Result<Option<PathBuf>, Never> {
    let name = match at.file_name().and_then(|name| name.to_str()) {
        Some(name) => name,
        None => return Ok(None),
    };

    let Ok(swept) = through(name, renaming);

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

pub fn stub(sweeping: &[String], when: u64) -> Result<String, Never> {
    let attic: String =
        sweeping.iter().map(|path| format!("        Step::Attic(\"{path}\"),\n")).collect();

    Ok(format!(
        "//! {UNSAID}\n\
         //!\n\
         //! A path below left the manifest under one name and arrived under another.\n\
         //! An apply installs what it is told and removes nothing, so both are on\n\
         //! every machine that applied the old one. Say here what that costs -- what\n\
         //! reads the old name, what a person would see with two of them, and why the\n\
         //! one being swept cannot simply be left.\n\
         \n\
         use crate::sweeping::{{Migration, Moment, Step}};\n\
         \n\
         pub const MIGRATION: Migration = Migration {{\n\
         \x20   moment: Moment({when}),\n\
         \x20   says: \"sweeping what the rename left behind\",\n\
         \x20   steps: &[\n\
         {attic}\
         \x20   ],\n\
         }};\n"
    ))
}

pub fn written_at(root: &Path, when: u64) -> Result<PathBuf, Never> {
    Ok(root.join(CRATE).join(DIRECTORY).join(format!("{PREFIX}{when}.rs")))
}

#[cfg(test)]
mod tests {
    const MUSIC: Renaming<'static> =
        Renaming { old: "console-music", new: "console-music" };

    use super::*;

    #[test]
    fn a_name_inside_a_longer_name_is_left_alone() {
        let Ok(said) = swept("console-music-player", MUSIC);

        assert_eq!(said, "console-music-player");
    }

    #[test]
    fn a_name_that_ends_is_swept_however_it_ends() {
        let Ok(file) = swept("console-music.desktop", MUSIC);
        let Ok(path) = swept("crates/console-music/src", MUSIC);
        let Ok(quoted) = swept("\"console-music\"", MUSIC);
        let Ok(ends) = swept("uses console-music", MUSIC);

        assert_eq!(file, "console-music.desktop");
        assert_eq!(path, "crates/console-music/src");
        assert_eq!(quoted, "\"console-music\"");
        assert_eq!(ends, "uses console-music");
    }

    #[test]
    fn both_spellings_of_a_name_move_together() {
        let Ok(said) = through("use console_music::player; // console-music", MUSIC);

        assert_eq!(said, "use console_music::player; // console-music");
    }

    #[test]
    fn the_underscore_spelling_of_a_longer_name_is_left_alone_too() {
        let Ok(said) = through("console_music_player", MUSIC);

        assert_eq!(said, "console_music_player");
    }

    #[test]
    fn a_name_with_no_underscore_spelling_is_swept_once() {
        let Ok(spellings) = spellings(Renaming { old: "kew", new: "music-player" });

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
        let Ok(said) = stub(&["/usr/share/applications/a.desktop".to_string()], 1790300000);

        assert!(said.contains("Step::Attic(\"/usr/share/applications/a.desktop\")"));
        assert!(said.contains("moment: Moment(1790300000)"));
        assert!(said.contains(UNSAID));
    }
}
