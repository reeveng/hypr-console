//! The Script property of Unicode Standard Annex #24, over Unicode 17.0: the writing system a
//! character belongs to.
//!
//! `UnicodeScript::script` answers the script Scripts.txt gives a character, the range in
//! `tables::SCRIPTS` it falls in, found by the last range that starts at or before it; a character
//! after the end of that range is in none, and is `Unknown`. `Script::short_name` answers the four
//! letters ISO 15924 knows a script by. That is what the text layout asks, to choose a font that
//! has a run's script, and what the vector graphics renderer asks, to know whether a script may be
//! letter-spaced.
//!
//! The crate began as unicode-script 0.5.8, by Manish Goregaokar. What it left behind nothing here
//! asked for: the Script_Extensions property and the sets of scripts it is answered in, a script's
//! full name and the way back to a script from it, its ISO 15924 tag as a number, whether UAX #31
//! recommends it, and its `Display`. The variants lost the numbers only those sets read, and the
//! `non_exhaustive` that made every match over them end in a wildcard. Every character's script
//! and every script's short name are what they were, and `tests/the_scripts.rs` holds both to the
//! Character Database's own files.
//!
//! <https://www.unicode.org/reports/tr24/>

#![no_std]

mod script;
mod tables;

use console_core_never::Never;
use core::cmp::Ordering;

pub use crate::script::Script;
use crate::tables::SCRIPTS;

pub trait UnicodeScript {
  fn script(&self) -> Result<Script, Never>;
}

impl UnicodeScript for char {
  fn script(&self) -> Result<Script, Never> {
    let character = *self;
    let found = SCRIPTS
      .partition_point(|&(first, _, _)| first <= character)
      .checked_sub(1)
      .and_then(|last| SCRIPTS.get(last));

    Ok(match found {
      Some(&(_, last, script)) => match character.cmp(&last) {
        Ordering::Less | Ordering::Equal => script,
        Ordering::Greater => Script::Unknown,
      },
      None => Script::Unknown,
    })
  }
}
