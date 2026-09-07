//! The colours as the machine spends them.
//!
//! Read out of the file every themed surface is themed from, so a palette that
//! moves moves its checks with it. A check carrying its own copy of a colour is
//! a check that goes red for somebody else's good reason, or worse, stays green
//! against a colour nothing uses any more.

use std::collections::BTreeMap;

pub use console_core_colour::spent::read;

pub const SPENT: &str = "files/usr/local/lib/console/palette.sh";

pub fn palette() -> Result<BTreeMap<String, String>, console_core_never::Never> {
    let Ok(root) = crate::root();
    let at = root.join(SPENT);

    let said = match std::fs::read_to_string(&at) {
        Ok(said) => said,
        Err(fault) => {
            eprintln!("console-test-stages: {}: {fault}", at.display());

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
        let Ok(found) = palette();

        assert!(!found.is_empty(), "no colours in {SPENT}");
    }
}
