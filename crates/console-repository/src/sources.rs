//! The tree's own Rust sources, read for the rules a compiler will not hold.
//!
//! Some of what this workspace holds itself to is about where a thing is
//! written rather than what it does: an icon named only through the enum, a
//! program named only through its variant, a `Child` in one crate. None of it
//! is a type error anywhere, so each is a test that reads the tree, and those
//! tests had each written the same recursive walk to find the files and the
//! same look either side of a word to find it. The walk is here once.
//!
//! What a file is taken to be is the compiler's answer, not a guess: a file
//! ending `.rs`, in any of the directories cargo compiles a crate from. What
//! is left out is the caller's question: the test asking, and the one file or
//! crate that is allowed to say the word. A rule about a long-running program
//! reads `src` alone and walks it with `under`.

use std::path::{Path, PathBuf};

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Word<'a>(pub &'a str);

pub const COMPILED: [&str; 3] = ["src", "tests", "examples"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spelled {
    Yes,
    No,
}

pub fn under(at: &Path) -> Result<Vec<PathBuf>, Never> {
    let mut found = Vec::new();
    let mut waiting = vec![at.to_path_buf()];

    while let Some(here) = waiting.pop() {
        let entries = match std::fs::read_dir(&here) {
            Ok(entries) => entries,
            Err(_nothing_there) => continue,
        };

        for path in entries.flatten().map(|entry| entry.path()) {
            match (path.is_dir(), path.extension().and_then(|ending| ending.to_str())) {
                (true, _) => waiting.push(path),
                (false, Some("rs")) => found.push(path),
                (false, Some(_) | None) => {}
            }
        }
    }

    found.sort();

    Ok(found)
}

pub fn of_every_crate(root: &Path, leaving_out: &[&Path]) -> Result<Vec<PathBuf>, Never> {
    let mut found = Vec::new();

    let crates = match std::fs::read_dir(root.join("crates")) {
        Ok(crates) => crates,
        Err(_nothing_there) => return Ok(found),
    };

    for crate_ in crates.flatten().map(|entry| entry.path()) {
        for directory in COMPILED {
            let Ok(inside) = under(&crate_.join(directory));

            found.extend(inside);
        }
    }

    for out in leaving_out {
        found.retain(|at| !at.starts_with(out));
    }

    found.sort();

    Ok(found)
}

pub fn spells(said: &str, word: Word<'_>) -> Result<Spelled, Never> {
    let Word(word) = word;
    let is_a_name_letter = |letter: char| letter.is_alphanumeric() || letter == '_';

    let found = said.match_indices(word).any(|(at, _)| {
        let before = said.get(..at).and_then(|earlier| earlier.chars().next_back());
        let after = said.get(at.saturating_add(word.len())..).and_then(|rest| rest.chars().next());

        !before.is_some_and(is_a_name_letter) && !after.is_some_and(is_a_name_letter)
    });

    Ok(match found {
        true => Spelled::Yes,
        false => Spelled::No,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_inside_a_longer_name_is_not_that_word() {
        assert_eq!(spells("let children = 1;", Word("Child")), Ok(Spelled::No));
        assert_eq!(spells("Icon::HomeScreen", Word("Icon::Home")), Ok(Spelled::No));
        assert_eq!(spells("MyIcon::Home", Word("Icon::Home")), Ok(Spelled::No));
    }

    #[test]
    fn a_word_with_anything_but_a_name_either_side_is_spelled() {
        assert_eq!(spells("icons::Icon::Home)", Word("Icon::Home")), Ok(Spelled::Yes));
        assert_eq!(spells("Child", Word("Child")), Ok(Spelled::Yes));
    }

    #[test]
    fn the_walk_finds_this_file() {
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        let Ok(found) = under(&here.join("src"));

        assert!(found.contains(&here.join("src/sources.rs")));
    }
}
