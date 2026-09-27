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
//! `naming` is for a word and `files_containing` is for a path. A word has an edge:
//! `Child` is not `Children`, so what is on either side of it is looked at. A
//! path has none -- what follows `.config/console/` is the filename, which is
//! exactly what the rule does not care about -- so the two spellings that end
//! one are looked for whole instead, a directory that goes on and a directory
//! that stops.
//!
//! A line that names a path under `files/home/@user@` is not read by `files_containing`,
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

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use console_core_iteration::Step;
use console_core_never::Never;
pub use console_manifest_engine::manifest::Section;
use console_repository::sources::{Spelled, Word, spells, under};

pub type Failure = Box<dyn std::error::Error>;

pub fn root() -> Result<PathBuf, Never> {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    match from.canonicalize() {
        Ok(found) => Ok(found),
        Err(_not_reached_from_here) => Ok(from),
    }
}

pub fn read(at: &str) -> Result<String, Failure> {
    let Ok(root) = root();
    let held = std::fs::read_to_string(root.join(at)).map_err(|fault| format!("{at}: {fault}"))?;

    Ok(held)
}

pub fn missing(excused: &[&'static str]) -> Result<Vec<&'static str>, Never> {
    let Ok(root) = root();
    let crates = root.join("crates");

    Ok(excused.iter().filter(|named| !crates.join(named).is_dir()).copied().collect())
}

pub fn section(held: &str, section: Section) -> Result<Vec<String>, Failure> {
    use console_manifest_engine::manifest::Manifest;

    let read = Manifest::read(held)?;
    let Ok(entries) = read.of(section);

    Ok(entries.to_vec())
}

pub const TREE: &str = "files/home/@user@";

pub fn files_containing(said: &str, excused: &[&str]) -> Result<Vec<String>, Failure> {
    let sources = sources(excused)?;

    Ok(sources
        .into_iter()
        .filter(|(_, held)| {
            let Ok(kept) = without_tests(held);
            let Ok(kept) = without_comments(&kept);
            let Ok(kept) = without_the_tree(&kept);

            kept.contains(said)
        })
        .map(|(at, _)| at.display().to_string())
        .collect())
}

fn without_the_tree(said: &str) -> Result<String, Never> {
    Ok(said.lines().filter(|line| !line.contains(TREE)).collect::<Vec<&str>>().join("\n"))
}

pub fn naming(word: &str, excused: &[&str]) -> Result<Vec<String>, Failure> {
    let sources = sources(excused)?;

    Ok(sources
        .into_iter()
        .filter(|(_, said)| {
            let Ok(kept) = without_tests(said);
            let Ok(kept) = without_comments(&kept);

            spells(&kept, Word(word)) == Ok(Spelled::Yes)
        })
        .map(|(at, _)| at.display().to_string())
        .collect())
}

fn sources(excused: &[&str]) -> Result<Vec<(PathBuf, String)>, Failure> {
    let Ok(root) = root();
    let excused: BTreeSet<&str> = excused.iter().copied().collect();
    let mut found = Vec::new();

    let entries = std::fs::read_dir(root.join("crates"))?;

    for entry in entries {
        let entry = entry?;
        let crate_ = entry.path();
        let named = crate_.file_name().and_then(|name| name.to_str()).ok_or("a crate named in UTF-8")?;

        match excused.contains(named) {
            true => continue,
            false => {},
        }

        let Ok(inside) = under(&crate_.join("src"));

        found.extend(inside);
    }

    found.sort();

    let mut read = Vec::new();

    for at in found {
        let said = std::fs::read_to_string(&at)?;

        read.push((at, said));
    }

    Ok(read)
}

const FIXTURES: &str = "#[cfg(test)]\nmod tests {";

fn without_tests(said: &str) -> Result<String, Never> {
    let kept = console_core_iteration::iterate((String::new(), said), |(mut kept, rest)| {
        Ok(match rest.find(FIXTURES).map(|at| rest.split_at(at)) {
            Some((before, from)) => {
                kept.push_str(before);

                let Ok(after) = past_the_body(from);

                Step::Again((kept, after))
            },
            None => {
                kept.push_str(rest);

                Step::Halt(kept)
            },
        })
    });

    Ok(match kept {
        Ok(kept) => kept,
        Err(_endless) => said.to_string(),
    })
}

fn past_the_body(said: &str) -> Result<&str, Never> {
    let past = console_core_iteration::iterate((said.chars(), 0_u32), |(mut letters, depth)| {
        let letter = match letters.next() {
            Some(letter) => letter,
            None => return Ok(Step::Halt("")),
        };

        let now = match letter {
            '{' => depth.saturating_add(1),
            '}' => depth.saturating_sub(1),
            _ => depth,
        };

        Ok(match (letter, now) {
            ('}', 0) => Step::Halt(letters.as_str()),
            _ => Step::Again((letters, now)),
        })
    });

    Ok(match past {
        Ok(past) => past,
        Err(_endless) => "",
    })
}

fn without_comments(said: &str) -> Result<String, Never> {
    Ok(said
        .lines()
        .map(|line| line.split_once("//").map_or(line, |(kept, _)| kept))
        .collect::<Vec<&str>>()
        .join("\n"))
}
