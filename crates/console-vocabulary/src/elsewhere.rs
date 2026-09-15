//! The words that are a list somewhere else already.
//!
//! `hyprctl` is written a hundred times here and is not a word this desktop
//! chose: it is a variant of `console_core_external_programs::Program`, with the
//! package it comes out of beside it, and that enum is the list of every program
//! this desktop runs and did not write. A heading in `words.conf` repeating
//! those names would be the second list the manifest rule denies, and the two
//! would disagree the first time a program left.
//!
//! The crate names are the same argument pointed inward. What a crate may be
//! called is `console-repository`'s families test, and every `use` line in the
//! tree spells one, so a vocabulary that asked for them again would be asking a
//! question somebody else answers -- and answering it differently on the day a
//! crate is renamed.
//!
//! So both are read where they live, and what is left for `words.conf` is what
//! nothing else in the tree writes down: the words this desktop says in its own
//! voice.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use console_core_external_programs::{EVERY, Origin};
use console_core_never::Never;

use crate::counting::{CRATES, split};

#[derive(Debug)]
pub enum Unlisted {
    Listing(PathBuf, std::io::Error),
}

impl fmt::Display for Unlisted {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unlisted::Listing(at, fault) => write!(to, "{}: {fault}", at.display()),
        }
    }
}

impl std::error::Error for Unlisted {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Name {
    ACrate,
    AProgram,
    APackage,
}

impl Name {
    pub fn spelled(self) -> Result<&'static str, Never> {
        Ok(match self {
            Name::ACrate => "a crate in this tree",
            Name::AProgram => "a program this desktop runs",
            Name::APackage => "a package this desktop installs",
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Elsewhere {
    named: BTreeMap<String, Name>,
}

impl Elsewhere {
    pub fn read(root: &Path) -> Result<Elsewhere, Unlisted> {
        let at = root.join(CRATES);
        let held = std::fs::read_dir(&at).map_err(|fault| Unlisted::Listing(at, fault))?;
        let mut named = BTreeMap::new();

        for entry in held.flatten() {
            let Ok(words) = split(&entry.file_name().to_string_lossy().replace('-', "_"));

            for word in words {
                let _ = named.insert(word, Name::ACrate);
            }
        }

        for program in EVERY {
            let Ok(spelled) = program.name();
            let Ok(origin) = program.origin();
            let Ok(words) = split(&spelled.replace('-', "_"));

            for word in words {
                let _ = named.insert(word, Name::AProgram);
            }

            match origin {
                Origin::Package(package) => {
                    let Ok(words) = split(&package.replace('-', "_"));

                    for word in words {
                        let _ = named.entry(word).or_insert(Name::APackage);
                    }
                },
                Origin::Arch => {},
                Origin::Developing => {},
            }
        }

        Ok(Elsewhere { named })
    }

    pub fn names(&self, word: &str) -> Result<Option<Name>, Never> {
        Ok(self.named.get(word).copied())
    }
}
