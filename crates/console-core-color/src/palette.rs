//! The palette as the machine spends it, read back.  `console-palette` writes
//! every color this desktop uses into one file per language that has to be
//! spoken, and one of them is a plain list of names and six hex digits. Two
//! things read it back: the keyboard, which is handed its colors as arguments
//! because it has no configuration file, and the checks, which look at the
//! screen and have to know what color a thing should have been.  One reader,
//! because those two agreeing by coincidence is exactly the fault this desktop
//! keeps having with colors: a check carrying its own copy of one is a check
//! that goes red for someone else's good reason, or worse, stays green against
//! a color nothing uses any more.  **Why the path is relative, and found from
//! the program asking.** `SPENT` is written without a leading slash on purpose.
//! There is more than one of these trees: the device has one at `/`, and the
//! nested desktop stages a whole copy of it under a directory of its own so
//! that a machine which is not the device can run the device's software without
//! being allowed to write to `/usr`. A program that joined `SPENT` onto `/`
//! read the wrong tree in the second one -- the keyboard in the staged session
//! found no palette at all, kept the colors it was compiled with, and drew a
//! keyboard nothing on this desktop is the color of. It said so on stderr,
//! where nothing was reading.  So the tree is the one the running program is
//! installed in, and `beside` is how it is asked. Both answers are the same
//! file on the device, which is the point: the stage differs from the device in
//! where it is and in nothing else.
//!
//! **Which names a surface wants is the surface's, and what a missing one means
//! is not.** Two things draw themselves out of this file now and both wanted
//! the same three lines -- look the name up, read the six digits, say which
//! name was missing rather than drawing in black. That is here rather than
//! twice, because a surface that quietly defaults one color is a surface that
//! comes up looking nearly right, which is worse than one that says what the
//! palette does not spend.
//!
//! **Finding the file is here too.** `worn` is the whole walk -- where the
//! program is, the file beside it, the colors in it -- because eight surfaces
//! had each written it, and the ones that folded a missing file into
//! `PaletteError` told the journal the palette spent "palette" as a color
//! rather than that there was no file to read.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use console_core_never::Never;

use crate::{Oklch, Rgba};

pub const SPENT: &str = "usr/local/lib/console/palette.sh";

pub const BIN: &str = "usr/local/bin";

pub fn beside(program: &Path) -> Result<PathBuf, Never> {
    let mut root = program.parent();

    for _ in Path::new(BIN).components() {
        root = root.and_then(Path::parent);
    }

    Ok(match root {
        Some(root) => root.join(SPENT),
        None => Path::new("/").join(SPENT),
    })
}

pub fn read(said: &str) -> Result<BTreeMap<String, String>, Never> {
    Ok(said.lines()
        .filter_map(|line| line.trim_end().split_once('='))
        .filter(|(name, _)| !name.is_empty() && name.chars().all(|l| l.is_alphanumeric() || l == '_'))
        .filter(|(_, color)| color.len() == 6 && color.chars().all(|l| l.is_ascii_hexdigit()))
        .map(|(name, color)| (name.to_string(), color.to_lowercase()))
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteError {
    Absent(&'static str),
    Invalid { color: &'static str, value: String },
}

impl std::fmt::Display for PaletteError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaletteError::Absent(color) => {
                write!(to, "the palette this desktop spends has no {color}")
            }
            PaletteError::Invalid { color, value } => {
                write!(to, "the palette spends {color} as {value}, which is not a color")
            }
        }
    }
}

impl std::error::Error for PaletteError {}

#[derive(Debug)]
pub enum WearingError {
    Lost(std::io::Error),
    Unread { at: PathBuf, why: std::io::Error },
    Palette(PaletteError),
}

impl std::fmt::Display for WearingError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WearingError::Lost(why) => write!(to, "this program cannot find itself: {why}"),
            WearingError::Unread { at, why } => write!(to, "no palette at {}: {why}", at.display()),
            WearingError::Palette(why) => write!(to, "{why}"),
        }
    }
}

impl std::error::Error for WearingError {}

impl From<PaletteError> for WearingError {
    fn from(why: PaletteError) -> WearingError {
        WearingError::Palette(why)
    }
}

pub fn spent() -> Result<BTreeMap<String, String>, WearingError> {
    let me = std::env::current_exe().map_err(WearingError::Lost)?;
    let Ok(at) = beside(&me);
    let held = std::fs::read_to_string(&at).map_err(|why| WearingError::Unread { at: at.clone(), why })?;
    let Ok(spent) = read(&held);

    Ok(spent)
}

pub const SPENDS: [&str; 7] = ["panel", "text", "edge", "soft", "coral", "ground", "fill"];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wearing {
    pub panel: Oklch,
    pub text: Oklch,
    pub edge: Oklch,
    pub soft: Oklch,
    pub coral: Oklch,
    pub ground: Oklch,
    pub fill: Oklch,
    pub night: Oklch,
    pub pink: Oklch,
}

impl Wearing {
    pub fn out_of(spent: &BTreeMap<String, String>) -> Result<Wearing, PaletteError> {
        let panel = named(spent, "panel")?;
        let text = named(spent, "text")?;
        let edge = named(spent, "edge")?;
        let soft = named(spent, "soft")?;
        let coral = named(spent, "coral")?;
        let ground = named(spent, "ground")?;
        let fill = named(spent, "fill")?;
        let night = named(spent, "night")?;
        let pink = named(spent, "pink")?;

        Ok(Wearing { panel, text, edge, soft, coral, ground, fill, night, pink })
    }

    pub fn worn() -> Result<Wearing, WearingError> {
        let spent = spent()?;
        let wearing = Wearing::out_of(&spent)?;

        Ok(wearing)
    }
}

pub fn named(
    spent: &BTreeMap<String, String>,
    what: &'static str,
) -> Result<Oklch, PaletteError> {
    let said = match spent.get(what) {
        Some(said) => said,
        None => return Err(PaletteError::Absent(what)),
    };
    let channels = Rgba::of(said)
        .map_err(|_| PaletteError::Invalid { color: what, value: said.clone() })?;
    let Ok(oklch) = Oklch::of(channels);

    Ok(oklch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_color_is_a_word_and_six_digits() {
        let Ok(found) = read("pink=FF7BAC\nnight=191724\nsomething=else\n# ground=000000\n");

        assert_eq!(found["pink"], "ff7bac");
        assert_eq!(found["night"], "191724");
        assert!(!found.contains_key("something"), "that is not a color");
    }

    #[test]
    fn what_has_been_commented_out_is_not_a_color() {
        let Ok(found) = read("# pink=ff7bac\n");

        assert!(found.is_empty());
    }

    #[test]
    fn nothing_in_an_empty_file_is_a_color() {
        let Ok(found) = read("");

        assert!(found.is_empty());
    }

    #[test]
    fn a_program_installed_at_the_root_spends_the_palette_at_the_root() {
        assert_eq!(
            beside(Path::new("/usr/local/bin/console-keyboard")),
            Ok(PathBuf::from("/usr/local/lib/console/palette.sh"))
        );
    }

    #[test]
    fn a_program_installed_in_a_staged_tree_spends_that_trees_palette() {
        assert_eq!(
            beside(Path::new("/s/session-1/usr/local/bin/console-keyboard")),
            Ok(PathBuf::from("/s/session-1/usr/local/lib/console/palette.sh"))
        );
    }

    #[test]
    fn a_program_nowhere_near_a_tree_falls_back_to_the_root() {
        assert_eq!(
            beside(Path::new("console-keyboard")),
            Ok(PathBuf::from("/usr/local/lib/console/palette.sh"))
        );
    }
}
