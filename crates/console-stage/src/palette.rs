//! The colours as the machine spends them.
//!
//! Read out of the file every themed surface is themed from, so a palette that
//! moves moves its checks with it. A check carrying its own copy of a colour is
//! a check that goes red for somebody else's good reason, or worse, stays green
//! against a colour nothing uses any more.

use std::collections::BTreeMap;

/// Where the colours are spent, under the tree.
pub const SPENT: &str = "files/usr/local/lib/console/palette.sh";

/// Every colour it names, by the word it is spent as.
pub fn read(said: &str) -> BTreeMap<String, String> {
    said.lines()
        .filter_map(|line| line.trim_end().split_once('='))
        .filter(|(name, _)| !name.is_empty() && name.chars().all(|l| l.is_alphanumeric() || l == '_'))
        .filter(|(_, colour)| colour.len() == 6 && colour.chars().all(|l| l.is_ascii_hexdigit()))
        .map(|(name, colour)| (name.to_string(), colour.to_lowercase()))
        .collect()
}

/// The palette this repository spends.
pub fn palette() -> BTreeMap<String, String> {
    let said = std::fs::read_to_string(crate::root().join(SPENT)).unwrap_or_default();
    read(&said)
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

    #[test]
    fn the_palette_this_machine_spends_is_read_off_the_file_that_spends_it() {
        let found = palette();
        assert!(!found.is_empty(), "no colours in {SPENT}");
    }
}
