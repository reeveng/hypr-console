//! Reading the tree's own sources, for the rules a compiler will not hold.
//!
//! Some of what this workspace holds itself to is about where a thing is
//! written rather than what it does: a `Child` is named in one crate, and where
//! a person's home is is asked in one. Neither is a type error anywhere, so
//! each is a test that reads the tree, and they read it the same way -- every
//! `src` file in every crate but the ones excused, with the comments taken off
//! so that arguing about a rule in prose cannot break it. That reading is here
//! and each test spells only its own word and its own excuses.
//!
//! `naming` is for a word and `saying` is for a path. A word has an edge:
//! `Child` is not `Children`, so what is on either side of it is looked at. A
//! path has none -- what follows `.config/console/` is the filename, which is
//! exactly what the rule does not care about -- so the two spellings that end
//! one are looked for whole instead, a directory that goes on and a directory
//! that stops.
//!
//! `tests/` is deliberately not read. What these rules are about is a program
//! that stays up for days on a handheld with nobody watching it; a test is
//! bounded by the `cargo test` run that started it, and stands a fixture up on
//! purpose. A `mod tests` inside a `src` file is the same fixture in a
//! different place, so it is taken out here for the same reason -- and taking
//! it out is what lets a test say the whole path of the file it is about,
//! which is the most useful thing such a test can say.
//!
//! It is `#[cfg(test)]` followed by `mod tests {` that is taken out, and the
//! braces are counted to find the end of it. The attribute alone is not enough
//! to go on: it also stands over a `use` and over one variant of an enum, and
//! neither of those has a body to count.

#![allow(
    dead_code,
    reason = "every test binary compiles this whole module and each one asks it for a different half"
)]

use std::path::{Path, PathBuf};

pub fn root() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}

pub fn saying(said: &str, excused: &[&str]) -> Vec<String> {
    sources(excused)
        .into_iter()
        .filter(|(_, held)| without_comments(&without_tests(held)).contains(said))
        .map(|(at, _)| at.display().to_string())
        .collect()
}

pub fn naming(word: &str, excused: &[&str]) -> Vec<String> {
    sources(excused)
        .into_iter()
        .filter(|(_, said)| says_the_word(&without_comments(&without_tests(said)), word))
        .map(|(at, _)| at.display().to_string())
        .collect()
}

fn sources(excused: &[&str]) -> Vec<(PathBuf, String)> {
    fn walk(at: &Path, into: &mut Vec<PathBuf>) {
        let entries = match std::fs::read_dir(at) {
            Ok(entries) => entries,
            Err(_fault) => return,
        };

        for path in entries.flatten().map(|entry| entry.path()) {
            match path {
                path if path.is_dir() => walk(&path, into),
                path if path.extension().is_some_and(|end| end == "rs") => into.push(path),
                _ => {}
            }
        }
    }

    let mut found = Vec::new();
    let crates = match std::fs::read_dir(root().join("crates")) {
        Ok(crates) => crates,
        Err(_fault) => return Vec::new(),
    };

    for crate_ in crates.flatten().map(|entry| entry.path()) {
        let named = crate_.file_name().and_then(|name| name.to_str()).unwrap_or("").to_string();

        match excused.contains(&named.as_str()) {
            true => continue,
            false => {}
        }

        walk(&crate_.join("src"), &mut found);
    }

    found.sort();
    found
        .into_iter()
        .filter_map(|at| std::fs::read_to_string(&at).ok().map(|said| (at, said)))
        .collect()
}

const FIXTURES: &str = "#[cfg(test)]\nmod tests {";

fn without_tests(said: &str) -> String {
    let mut kept = String::new();
    let mut rest = said;

    loop {
        let at = match rest.find(FIXTURES) {
            Some(at) => at,
            None => {
                kept.push_str(rest);

                return kept;
            }
        };

        match (rest.get(..at), rest.get(at..)) {
            (Some(before), Some(from)) => {
                kept.push_str(before);

                rest = past_the_body(from);
            }
            _ => return kept,
        }
    }
}

fn past_the_body(said: &str) -> &str {
    let mut depth: usize = 0;

    for (at, letter) in said.char_indices() {
        let now = match letter {
            '{' => depth.saturating_add(1),
            '}' => depth.saturating_sub(1),
            _ => depth,
        };

        match (letter, now) {
            ('}', 0) => return said.get(at.saturating_add(1)..).unwrap_or(""),
            _ => {}
        }

        depth = now;
    }

    ""
}

fn without_comments(said: &str) -> String {
    said.lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<&str>>()
        .join("\n")
}

fn says_the_word(said: &str, word: &str) -> bool {
    said.match_indices(word).any(|(at, _)| {
        let before = said
            .get(..at)
            .and_then(|earlier| earlier.chars().next_back())
            .is_none_or(|letter| !letter.is_alphanumeric() && letter != '_');
        let after = said
            .get(at.saturating_add(word.len())..)
            .and_then(|rest| rest.chars().next())
            .is_none_or(|letter| !letter.is_alphanumeric() && letter != '_');

        before && after
    })
}
