//! The palette as the machine spends it, read back.  `console-palette` writes
//! every colour this desktop uses into one file per language that has to be
//! spoken, and one of them is a plain list of names and six hex digits. Two
//! things read it back: the keyboard, which is handed its colours as arguments
//! because it has no configuration file, and the checks, which look at the
//! screen and have to know what colour a thing should have been.  One reader,
//! because those two agreeing by coincidence is exactly the fault this desktop
//! keeps having with colours: a check carrying its own copy of one is a check
//! that goes red for somebody else's good reason, or worse, stays green against
//! a colour nothing uses any more.  **Why the path is relative, and found from
//! the program asking.** `SPENT` is written without a leading slash on purpose.
//! There is more than one of these trees: the device has one at `/`, and the
//! nested desktop stages a whole copy of it under a directory of its own so
//! that a machine which is not the device can run the device's software without
//! being allowed to write to `/usr`. A program that joined `SPENT` onto `/`
//! read the wrong tree in the second one -- the keyboard in the staged session
//! found no palette at all, kept the colours it was compiled with, and drew a
//! keyboard nothing on this desktop is the colour of. It said so on stderr,
//! where nothing was reading.  So the tree is the one the running program is
//! installed in, and `beside` is how it is asked. Both answers are the same
//! file on the device, which is the point: the stage differs from the device in
//! where it is and in nothing else.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use console_core_never::Never;

pub const SPENT: &str = "usr/local/lib/console/palette.sh";

pub const BIN: &str = "usr/local/bin";

pub fn beside(program: &Path) -> Result<PathBuf, Never> {
    let deep = Path::new(BIN).components().count().saturating_add(1);

    Ok(match program.ancestors().nth(deep) {
        Some(root) => root.join(SPENT),
        None => Path::new("/").join(SPENT),
    })
}

pub fn read(said: &str) -> Result<BTreeMap<String, String>, Never> {
    Ok(said.lines()
        .filter_map(|line| line.trim_end().split_once('='))
        .filter(|(name, _)| !name.is_empty() && name.chars().all(|l| l.is_alphanumeric() || l == '_'))
        .filter(|(_, colour)| colour.len() == 6 && colour.chars().all(|l| l.is_ascii_hexdigit()))
        .map(|(name, colour)| (name.to_string(), colour.to_lowercase()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_is_a_word_and_six_digits() {
        let Ok(found) = read("pink=FF7BAC\nnight=191724\nsomething=else\n# ground=000000\n");

        assert_eq!(found["pink"], "ff7bac");
        assert_eq!(found["night"], "191724");
        assert!(!found.contains_key("something"), "that is not a colour");
    }

    #[test]
    fn what_has_been_commented_out_is_not_a_colour() {
        let Ok(found) = read("# pink=ff7bac\n");

        assert!(found.is_empty());
    }

    #[test]
    fn nothing_in_an_empty_file_is_a_colour() {
        let Ok(found) = read("");

        assert!(found.is_empty());
    }

    #[test]
    fn a_program_installed_at_the_root_spends_the_palette_at_the_root() {
        assert_eq!(
            beside(Path::new("/usr/local/bin/virtual-keyboard")),
            Ok(PathBuf::from("/usr/local/lib/console/palette.sh"))
        );
    }

    #[test]
    fn a_program_installed_in_a_staged_tree_spends_that_trees_palette() {
        assert_eq!(
            beside(Path::new("/s/session-1/usr/local/bin/virtual-keyboard")),
            Ok(PathBuf::from("/s/session-1/usr/local/lib/console/palette.sh"))
        );
    }

    #[test]
    fn a_program_nowhere_near_a_tree_falls_back_to_the_root() {
        assert_eq!(
            beside(Path::new("virtual-keyboard")),
            Ok(PathBuf::from("/usr/local/lib/console/palette.sh"))
        );
    }
}
