//! How often English writes a word, read once from the list beside this crate.
//!
//! The rate is what is kept rather than the count. A count means nothing on its
//! own -- it is a count out of a corpus this tree cannot see the size of -- and
//! turning it into a rate at the point where the list is written means nothing
//! downstream divides by a number it would have to be told.
//!
//! A word the list does not have is not a fault and not a zero. It is a word
//! English writes less often than the twenty-thousandth most common one, which is
//! about three times in a million, and [`Norm::of`] says so with a `None` that
//! every caller has to meet.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::PerMillion;

pub const ENGLISH: &str = "fixtures/english.tsv";

const COMMENT: char = '#';

const BETWEEN: char = '\t';

#[derive(Debug)]
pub enum Unread {
    Reading(PathBuf, std::io::Error),
    Rate(PathBuf, String),
}

impl fmt::Display for Unread {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unread::Reading(at, fault) => write!(to, "{}: {fault}", at.display()),
            Unread::Rate(at, line) => write!(to, "{}: no rate on {line}", at.display()),
        }
    }
}

impl std::error::Error for Unread {}

#[derive(Debug, Clone, Default)]
pub struct Norm {
    said: BTreeMap<String, PerMillion>,
}

impl Norm {
    pub fn read(at: &Path) -> Result<Norm, Unread> {
        let said = std::fs::read_to_string(at)
            .map_err(|fault| Unread::Reading(at.to_path_buf(), fault))?;
        let mut held = BTreeMap::new();

        for line in said.lines().filter(|line| !line.starts_with(COMMENT)) {
            let (word, rate) = line
                .split_once(BETWEEN)
                .ok_or_else(|| Unread::Rate(at.to_path_buf(), line.to_string()))?;
            let rate = rate
                .trim()
                .parse::<f64>()
                .map_err(|_a_line_that_is_not_a_rate| {
                    Unread::Rate(at.to_path_buf(), line.to_string())
                })?;

            let _ = held.insert(word.trim().to_string(), PerMillion(rate));
        }

        Ok(Norm { said: held })
    }

    pub fn of(&self, word: &str) -> Result<Option<PerMillion>, Never> {
        Ok(self.said.get(word).copied())
    }

    pub fn words(&self) -> Result<u32, Never> {
        fitted(self.said.len())
    }
}

pub fn beside() -> Result<PathBuf, Never> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR")).join(ENGLISH))
}
