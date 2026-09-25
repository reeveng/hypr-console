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
//! A line that names a path under `files/home/@user@` is not read by `saying`,
//! because what is under `files/` is a path in this repository and these rules
//! are about paths on a machine. `include_str!` is the case that forced it: the
//! one crate that reads the compositor's file out of the tree has to spell the
//! whole name, a macro cannot be handed a directory someone asked for, and the
//! alternative was excusing a crate from a rule it keeps everywhere else.
//!
//! `tests/` is deliberately not read. What these rules are about is a program
//! that stays up for days on a handheld with no one watching it; a test is
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

pub const TREE: &str = "files/home/@user@";

pub fn saying(said: &str, excused: &[&str]) -> Vec<String> {
    sources(excused)
        .into_iter()
        .filter(|(_, held)| {
            without_the_tree(&without_comments(&without_tests(held))).contains(said)
        })
        .map(|(at, _)| at.display().to_string())
        .collect()
}

fn without_the_tree(said: &str) -> String {
    said.lines().filter(|line| !line.contains(TREE)).collect::<Vec<&str>>().join("\n")
}

pub fn naming(word: &str, excused: &[&str]) -> Vec<String> {
    sources(excused)
        .into_iter()
        .filter(|(_, said)| {
            console_repository::sources::spells(&without_comments(&without_tests(said)), console_repository::sources::Word(word))
                == Ok(console_repository::sources::Spelled::Yes)
        })
        .map(|(at, _)| at.display().to_string())
        .collect()
}

fn sources(excused: &[&str]) -> Vec<(PathBuf, String)> {
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

        let Ok(inside) = console_repository::sources::under(&crate_.join("src"));

        found.extend(inside);
    }

    found.sort();
    found
        .into_iter()
        .filter_map(|at| std::fs::read_to_string(&at).ok().map(|said| (at, said)))
        .collect()
}

pub fn section(held: &str, wanted: &str) -> Vec<String> {
    use console_manifest_engine::manifest::{Manifest, Section};

    let Ok(named) = Section::from_name(wanted);
    let read = Manifest::read(held).expect("the manifest reads");

    match named {
        Some(section) => {
            let Ok(entries) = read.of(section);

            entries.to_vec()
        }
        None => panic!("{wanted} is not a section the manifest has"),
    }
}

const FIXTURES: &str = "#[cfg(test)]\nmod tests {";

fn without_tests(said: &str) -> String {
    let mut kept = String::new();
    let mut rest = said;

    loop {
        match rest.find(FIXTURES).map(|at| rest.split_at(at)) {
            Some((before, from)) => {
                kept.push_str(before);

                rest = past_the_body(from);
            }
            None => {
                kept.push_str(rest);

                return kept;
            }
        }
    }
}

fn past_the_body(said: &str) -> &str {
    let mut depth: u32 = 0;
    let mut letters = said.chars();

    while let Some(letter) = letters.next() {
        let now = match letter {
            '{' => depth.saturating_add(1),
            '}' => depth.saturating_sub(1),
            _ => depth,
        };

        match (letter, now) {
            ('}', 0) => return letters.as_str(),
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

