//! The palette as the machine spends it, read back.
//!
//! `console-theme` writes every colour this desktop uses into one file per
//! language that has to be spoken, and one of them is a plain list of names and
//! six hex digits. Two things read it back: the keyboard, which is handed its
//! colours as arguments because it has no configuration file, and the checks,
//! which look at the screen and have to know what colour a thing should have
//! been.
//!
//! One reader, because those two agreeing by coincidence is exactly the fault
//! this desktop keeps having with colours: a check carrying its own copy of one
//! is a check that goes red for somebody else's good reason, or worse, stays
//! green against a colour nothing uses any more.

use std::collections::BTreeMap;

/// Where the colours are spent, under the tree.
pub const SPENT: &str = "usr/local/lib/console/palette.sh";

/// Every colour a spent palette names, by the word it is spent as.
///
/// Anything that is not a name and six hex digits is not a colour: the file
/// carries a comment or two, and a line that has been commented out is not an
/// answer.
pub fn read(said: &str) -> BTreeMap<String, String> {
    said.lines()
        .filter_map(|line| line.trim_end().split_once('='))
        .filter(|(name, _)| !name.is_empty() && name.chars().all(|l| l.is_alphanumeric() || l == '_'))
        .filter(|(_, colour)| colour.len() == 6 && colour.chars().all(|l| l.is_ascii_hexdigit()))
        .map(|(name, colour)| (name.to_string(), colour.to_lowercase()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_is_a_word_and_six_digits() {
        let found = read("pink=FF7BAC\nnight=191724\nsomething=else\n# ground=000000\n");
        assert_eq!(found["pink"], "ff7bac");
        assert_eq!(found["night"], "191724");
        assert!(!found.contains_key("something"), "that is not a colour");
    }

    /// A hash begins a comment in the file this is written as, and a name with
    /// one in front of it has been taken out rather than left in.
    #[test]
    fn what_has_been_commented_out_is_not_a_colour() {
        assert!(read("# pink=ff7bac\n").is_empty());
    }

    #[test]
    fn nothing_in_an_empty_file_is_a_colour() {
        assert!(read("").is_empty());
    }
}
