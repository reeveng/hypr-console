//! The words this desktop decided on, and the heading each one is under.
//!
//! `words.conf` is at the top of the tree beside `desktop.conf` and for the
//! same reason: what a machine holds is one file, and what a machine says is
//! another. A word is one line, and the heading above it is the whole argument
//! for it being here -- `press` under `[input]` is a thing someone does to a
//! button, and the day something else spells `press` at a thing that is not a
//! button the heading is what says so.
//!
//! The headings are the point rather than decoration, which is why this reads
//! the file with `console-core-ini-files` and keeps which heading a word came
//! from. A flat list of allowed words would answer the gate and tell a reader
//! nothing; a list under headings is where a family of words too close together
//! is visible as a family.
//!
//! Nothing here is about spelling. A word under a heading is a word this tree
//! may write as often as it likes, and how often is [`crate::measured`]'s
//! question.
//!
//! A heading is an argument for a word and not for the whole tree writing it.
//! `backlight` is the kernel's name for one file under `/sys`, and a heading
//! that let every crate say it would be the vocabulary working against itself:
//! the word arrives declared, and nothing asks why drawing or music is spelling
//! the kernel's word at all. So a line may name where its word belongs --
//! `backlight = crates/console-settings` -- and then the word is this tree's in
//! that place and undeclared everywhere else. What matches is the front of the
//! path, so a crate, a directory or one file all say themselves; a word with
//! nothing after it is a word the whole tree decided on, which is most of them.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use console_core_ini_files::{Under, lines, without_a_comment};
use console_core_never::Never;

pub const WORDS: &str = "words.conf";

pub const ONLY_IN: char = '=';

const AND: char = ',';

#[derive(Debug)]
pub enum Unread {
    Reading(PathBuf, std::io::Error),
    Twice(String, String, String),
}

impl fmt::Display for Unread {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unread::Reading(at, fault) => write!(to, "{}: {fault}", at.display()),
            Unread::Twice(word, under, again) => {
                write!(to, "{word} is declared under [{under}] and again under [{again}]")
            },
        }
    }
}

impl std::error::Error for Unread {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Configuration {
    Under(String),
    Nowhere,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Anywhere,
    Only(Vec<String>),
}

#[derive(Debug, Clone, Default)]
pub struct Declared {
    under: BTreeMap<String, String>,

    scope: BTreeMap<String, Vec<String>>,
}

impl Declared {
    pub fn read(at: &Path) -> Result<Declared, Unread> {
        let said = std::fs::read_to_string(at)
            .map_err(|fault| Unread::Reading(at.to_path_buf(), fault))?;
        let Ok(headings) = console_core_ini_files::headings(&said);
        let mut held: BTreeMap<String, String> = BTreeMap::new();
        let mut scope: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for heading in headings {
            let Ok(under) = lines(&said, Under(heading));

            for line in under {
                let Ok(said) = without_a_comment(line);
                let (word, places) = match said.split_once(ONLY_IN) {
                    Some((word, places)) => {
                        let Ok(places) = where_it_belongs(places);

                        (word.trim(), places)
                    },
                    None => (said, Vec::new()),
                };

                match places.is_empty() {
                    true => {},
                    false => {
                        let _ = scope.insert(word.to_lowercase(), places);
                    },
                }

                match held.insert(word.to_lowercase(), heading.to_string()) {
                    Some(already) => {
                        return Err(Unread::Twice(
                            word.to_string(),
                            already,
                            heading.to_string(),
                        ));
                    },
                    None => {},
                }
            }
        }

        Ok(Declared { under: held, scope })
    }

    pub fn scope(&self, word: &str) -> Result<Scope, Never> {
        Ok(match self.scope.get(word) {
            Some(places) => Scope::Only(places.clone()),
            None => Scope::Anywhere,
        })
    }

    pub fn knows(&self, word: &str) -> Result<Configuration, Never> {
        Ok(match self.under.get(word) {
            Some(heading) => Configuration::Under(heading.clone()),
            None => Configuration::Nowhere,
        })
    }

    pub fn every(&self) -> Result<&BTreeMap<String, String>, Never> {
        Ok(&self.under)
    }
}

fn where_it_belongs(said: &str) -> Result<Vec<String>, Never> {
    Ok(said
        .split(AND)
        .map(str::trim)
        .filter(|place| !place.is_empty())
        .map(str::to_string)
        .collect())
}
