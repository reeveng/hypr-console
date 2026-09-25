//! Every word this tree writes for itself, counted, with where to find it.
//!
//! A word arrives in three shapes and they are one vocabulary: an identifier,
//! the `//!` head above it, and the paragraph in `docs/` arguing for both. So
//! all three are read into the same count, and the split between a name and a
//! sentence is not a split this crate keeps.
//!
//! An identifier is taken apart the way a reader takes it apart -- on the
//! underscores and at the capitals -- because `unpressed_yet` is two words and
//! `ByAPress` is three, and a count of whole identifiers would say every one of
//! them exactly once and measure nothing.
//!
//! What is dropped is what someone else said. A string literal is stripped
//! before the identifiers are taken, because a `hyprctl` argument, a CSS
//! property and a `.desktop` key are quotations rather than this desktop's
//! words, and a vocabulary that counted them would be a list of every program
//! on the machine. The stripping is done a line at a time on the quote, so the
//! inside of a literal that spans lines is read as code; it is prose in there
//! either way, and the floor under [`crate::A_HABIT`] is what absorbs the rest.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_number_conversion::{Float, fitted};

use crate::{PerMillion, Uses};

#[derive(Debug)]
pub enum Unread {
    Listing(PathBuf, std::io::Error),
    Reading(PathBuf, std::io::Error),
}

impl fmt::Display for Unread {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unread::Listing(at, fault) => write!(to, "{}: {fault}", at.display()),
            Unread::Reading(at, fault) => write!(to, "{}: {fault}", at.display()),
        }
    }
}

impl std::error::Error for Unread {}

#[derive(Debug, Clone, Default)]
pub struct Written {
    pub uses: Uses,

    pub written_in: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Counted {
    pub words: BTreeMap<String, Written>,

    pub total: Uses,
}

impl Counted {
    pub fn per_million(&self, uses: Uses) -> Result<PerMillion, Never> {
        let Ok(many) = uses.0.float();
        let Ok(whole) = self.total.0.max(1).float();

        Ok(PerMillion(many / whole * A_MILLION))
    }
}

const A_MILLION: f64 = 1_000_000.0;

pub const CRATES: &str = "crates";

pub const DOCS: &str = "docs";

pub const README: &str = "README.md";

const RUST: &str = "rs";

const PROSE: &str = "md";

const TARGET: &str = "target";

const HEAD: &str = "//";

const FENCE: &str = "```";

const QUOTE: char = '"';

const SPAN: char = '`';

const UNDER: char = '_';

const SHORTEST: u32 = 3;

pub fn counted(root: &Path) -> Result<Counted, Unread> {
    let code = under(&root.join(CRATES), Ending::Rust)?;
    let prose = under(&root.join(DOCS), Ending::Prose)?;
    let mut held = Counted::default();

    for at in code {
        let said = read(&at)?;
        let Ok(words) = words_of_code(&said);
        let Ok(named) = naming(Top(root), &at);

        let Ok(()) = counting(&mut held, &words, &named);
    }

    for at in prose.into_iter().chain(std::iter::once(root.join(README))) {
        let said = read(&at)?;
        let Ok(words) = words_of_prose(&said);
        let Ok(named) = naming(Top(root), &at);

        let Ok(()) = counting(&mut held, &words, &named);
    }

    Ok(held)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Top<'a>(pub &'a Path);

fn naming(top: Top<'_>, at: &Path) -> Result<String, Never> {
    Ok(match at.strip_prefix(top.0) {
        Ok(under) => under.display().to_string(),
        Err(_a_path_from_outside_the_tree) => at.display().to_string(),
    })
}

fn read(at: &Path) -> Result<String, Unread> {
    std::fs::read_to_string(at).map_err(|fault| Unread::Reading(at.to_path_buf(), fault))
}

fn counting(into: &mut Counted, words: &[String], at: &str) -> Result<(), Never> {
    for word in words {
        let held = into.words.entry(word.clone()).or_default();

        held.uses = Uses(held.uses.0.saturating_add(1));

        let _ = held.written_in.insert(at.to_string());

        into.total = Uses(into.total.0.saturating_add(1));
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    Rust,
    Prose,
}

impl Ending {
    pub fn spelled(self) -> Result<&'static str, Never> {
        Ok(match self {
            Ending::Rust => RUST,
            Ending::Prose => PROSE,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Walk {
    Into,
    Past,
}

fn walking(at: &Path) -> Result<Walk, Never> {
    Ok(match at.file_name().and_then(OsStr::to_str) {
        Some(named) => match named == TARGET || named.starts_with('.') {
            true => Walk::Past,
            false => Walk::Into,
        },
        None => Walk::Past,
    })
}

pub fn under(root: &Path, ending: Ending) -> Result<Vec<PathBuf>, Unread> {
    let Ok(wanted) = ending.spelled();
    let mut found = Vec::new();
    let mut waiting = vec![root.to_path_buf()];

    while let Some(folder) = waiting.pop() {
        let held = std::fs::read_dir(&folder).map_err(|fault| Unread::Listing(folder.clone(), fault))?;

        for entry in held.flatten() {
            let at = entry.path();

            match at.is_dir() {
                true => {
                    let Ok(walk) = walking(&at);

                    match walk {
                        Walk::Into => waiting.push(at),
                        Walk::Past => {},
                    }
                },

                false => match at.extension().and_then(OsStr::to_str) == Some(wanted) {
                    true => found.push(at),
                    false => {},
                },
            }
        }
    }

    found.sort();

    Ok(found)
}

pub fn words_of_code(said: &str) -> Result<Vec<String>, Never> {
    let mut found = Vec::new();

    for line in said.lines() {
        match line.trim_start().starts_with(HEAD) {
            true => {
                let Ok(words) = words_of_a_sentence(line);

                found.extend(words);
            },

            false => {
                for part in line.split(QUOTE).step_by(2) {
                    let Ok(words) = identifiers(part);

                    found.extend(words);
                }
            },
        }
    }

    Ok(found)
}

pub fn words_of_prose(said: &str) -> Result<Vec<String>, Never> {
    let mut found = Vec::new();
    let mut fenced = Fenced::No;

    for line in said.lines() {
        match line.trim_start().starts_with(FENCE) {
            true => {
                fenced = match fenced {
                    Fenced::No => Fenced::Yes,
                    Fenced::Yes => Fenced::No,
                };
            },

            false => match fenced {
                Fenced::No => {
                    let Ok(words) = words_of_a_sentence(line);

                    found.extend(words);
                },
                Fenced::Yes => {},
            },
        }
    }

    Ok(found)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fenced {
    Yes,
    No,
}

pub fn words_of_a_sentence(line: &str) -> Result<Vec<String>, Never> {
    let mut found = Vec::new();

    for part in line.split(SPAN).step_by(2) {
        let said = part.split(|letter: char| !letter.is_alphabetic());

        found.extend(said.filter_map(|word| {
            let Ok(kept) = keeping(&word.to_lowercase());

            kept
        }));
    }

    Ok(found)
}

pub fn identifiers(said: &str) -> Result<Vec<String>, Never> {
    let mut found = Vec::new();
    let runs = said.split(|letter: char| !letter.is_ascii_alphanumeric() && letter != UNDER);

    for run in runs {
        let Ok(words) = split(run);

        found.extend(words);
    }

    Ok(found)
}

pub fn split(said: &str) -> Result<Vec<String>, Never> {
    let mut found = Vec::new();
    let mut held = String::new();

    for letter in said.chars() {
        let Ok(starting) = starting(&held, letter);

        match starting {
            Starting::Yes => {
                found.push(std::mem::take(&mut held));
            },
            Starting::No => {},
        }

        match letter == UNDER {
            true => {
                found.push(std::mem::take(&mut held));
            },
            false => held.push(letter.to_ascii_lowercase()),
        }
    }

    found.push(held);

    Ok(found
        .into_iter()
        .filter_map(|word| {
            let Ok(kept) = keeping(&word);

            kept
        })
        .collect())
}

fn keeping(word: &str) -> Result<Option<String>, Never> {
    let Ok(long) = fitted::<_, u32>(word.len());

    Ok(
        match long >= SHORTEST && word.chars().all(|letter| letter.is_ascii_alphabetic()) {
            true => Some(word.to_string()),
            false => None,
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Starting {
    Yes,
    No,
}

fn starting(held: &str, letter: char) -> Result<Starting, Never> {
    Ok(match letter.is_ascii_uppercase() {
        true => match held.chars().last() {
            Some(last) => match last.is_ascii_uppercase() {
                true => Starting::No,
                false => Starting::Yes,
            },
            None => Starting::No,
        },
        false => Starting::No,
    })
}
