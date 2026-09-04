//! The colours as the machine spends them.
//!
//! Read out of the file every themed surface is themed from, so a palette that
//! moves moves its checks with it. A check carrying its own copy of a colour is
//! a check that goes red for somebody else's good reason, or worse, stays green
//! against a colour nothing uses any more.

use std::collections::BTreeMap;

pub use console_colour::spent::read;

/// Where the colours are spent, under the tree.
pub const SPENT: &str = "files/usr/local/lib/console/palette.sh";

/// The palette this repository spends.
pub fn palette() -> BTreeMap<String, String> {
    let at = crate::root().join(SPENT);

    // No colours at all is what an empty file reads as, and it is what every
    // check that spends this treats as "the palette is not here". A file that
    // is there and will not be read is a different fact and used to arrive as
    // the same empty table.
    let said = match std::fs::read_to_string(&at) {
        Ok(said) => said,
        Err(fault) => {
            eprintln!("console-stage: {}: {fault}", at.display());

            String::new()
        }
    };

    read(&said)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_palette_this_machine_spends_is_read_off_the_file_that_spends_it() {
        let found = palette();
        assert!(!found.is_empty(), "no colours in {SPENT}");
    }
}
