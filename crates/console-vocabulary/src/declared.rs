//! The words this desktop decided on, and the heading each one is under.
//!
//! `words.conf` is at the top of the tree beside `desktop.conf` and for the
//! same reason: what a machine holds is one file, and what a machine says is
//! another. A word is one line, and the heading above it is the whole argument
//! for it being here -- `press` under `[input]` is a thing somebody does to a
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

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use console_core_ini_files::{Under, lines, without_a_comment};
use console_core_never::Never;

pub const WORDS: &str = "words.conf";

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
pub enum Declaration {
    Under(String),
    Nowhere,
}

#[derive(Debug, Clone, Default)]
pub struct Declared {
    under: BTreeMap<String, String>,
}

impl Declared {
    pub fn read(at: &Path) -> Result<Declared, Unread> {
        let said = std::fs::read_to_string(at)
            .map_err(|fault| Unread::Reading(at.to_path_buf(), fault))?;
        let Ok(headings) = console_core_ini_files::headings(&said);
        let mut held: BTreeMap<String, String> = BTreeMap::new();

        for heading in headings {
            let Ok(under) = lines(&said, Under(heading));

            for line in under {
                let Ok(word) = without_a_comment(line);

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

        Ok(Declared { under: held })
    }

    pub fn knows(&self, word: &str) -> Result<Declaration, Never> {
        Ok(match self.under.get(word) {
            Some(heading) => Declaration::Under(heading.clone()),
            None => Declaration::Nowhere,
        })
    }

    pub fn every(&self) -> Result<&BTreeMap<String, String>, Never> {
        Ok(&self.under)
    }
}
