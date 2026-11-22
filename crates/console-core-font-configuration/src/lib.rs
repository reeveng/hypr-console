//! A machine's fontconfig files, read as fontconfig reads them, into what they say.
//!
//! `parse_config_parts` reads one file's text into its parts, in the order they
//! are written. `FontConfig::merge_config` reads a file and everything it
//! includes into one configuration: the aliases that say what a generic family
//! stands for, the directories fonts are kept in, the cache and remapped
//! directories, the rules that select and edit fonts, and which files were
//! read. What it is told about the machine -- where the home and the base
//! directories are -- it is handed as `Bases`, so it reads no variable of its
//! own; the files it is asked for and the files they include are the only
//! things it reads.
//!
//! Most of fontconfig's language is read into its parts and not acted on: a
//! match rule's tests and edits are kept as they were written, for whoever asks
//! next. This is fontconfig-parser, and the parts it does not read yet are the
//! parts it did not read upstream either.

#[macro_use]
mod util;

mod error;
mod parser;
mod types;

pub type Result<T> = core::result::Result<T, Error>;

pub use crate::error::Error;
pub use crate::types::*;

pub fn parse_config_parts(s: &str) -> Result<Vec<ConfigPart>> {
  let document = roxmltree::Document::parse_with_options(
    s,
    roxmltree::ParsingOptions {
      dtd: roxmltree::Dtd::Allow,
      ..Default::default()
    },
  )?;
  let parts = crate::parser::parse_config(&document)?;

  parts.collect()
}
