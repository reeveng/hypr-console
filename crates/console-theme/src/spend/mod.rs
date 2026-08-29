//! One module per language the palette has to be spoken in.
//!
//! A stylesheet, a Lua table, a TOML file, an ini file, a shell script and a
//! browser cannot share a variable with each other, but most of them can
//! import a file written in their own language. So each module here writes one
//! small palette file, and the rest of the desktop imports whichever of them
//! speaks its own.
//!
//! Every function here is the same shape: a palette in, the text of a file
//! out. None of them touches the disk, so all of them can be read and tested
//! without a machine to write to.

pub mod alacritty;
pub mod breeze;
pub mod gtk;
pub mod hyprland;
pub mod icon;
pub mod kde;
pub mod librewolf;
pub mod paper;
pub mod shell;

use std::path::{Path, PathBuf};

use crate::palette::Palette;
use crate::terminal::Terminal;

/// The colours every palette file writes out, in the order they are written.
pub const ROLES: [&str; 16] = [
    "night", "ground", "panel", "edge", "text", "soft", "pink", "rose", "mauve", "lilac", "sky",
    "mint", "leaf", "butter", "peach", "coral",
];

/// The column a list of names is aligned into.
pub fn widest<const N: usize>(names: [&str; N]) -> usize {
    names.iter().map(|name| name.len()).max().unwrap_or(0)
}

/// How much of a file is ours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// A file that is nothing but colour, written from end to end.
    Whole,
    /// A file somebody else owns, where only what lies between the markers is
    /// written.
    Region,
}

/// One file the palette reaches, and what it should hold.
#[derive(Debug, Clone)]
pub struct Written {
    pub path: PathBuf,
    pub how: How,
    pub body: String,
}

/// Every file the palette reaches.
///
/// Five of these are the palette itself, one per language that has to be
/// spoken. The rest of the desktop imports whichever of them speaks its own,
/// so this list is nearly the whole of where a colour appears on the machine.
pub fn everywhere(files: &Path, palette: &Palette, terminal: &Terminal) -> Vec<Written> {
    let home = files.join("home/@user@");
    let chrome = home.join(".librewolf/console/chrome");
    let whole = |path: PathBuf, body: String| Written {
        path,
        how: How::Whole,
        body,
    };
    let region = |path: PathBuf, body: String| Written {
        path,
        how: How::Region,
        body,
    };

    vec![
        // The palette, once per language.
        whole(home.join(".config/console/palette.css"), gtk::spend(palette)),
        whole(
            home.join(".config/console/palette.toml"),
            alacritty::spend(terminal),
        ),
        whole(chrome.join("palette.css"), librewolf::stylesheet(palette)),
        whole(
            files.join("usr/local/lib/console/palette.sh"),
            shell::spend(palette),
        ),
        // The two that cannot import anything. KDE's ini format has no
        // include, and a user.js is a list of literals.
        region(home.join(".config/kdeglobals"), kde::spend(palette)),
        region(
            home.join(".librewolf/console/user.js"),
            librewolf::prefs(palette),
        ),
        // The compositor. Written rather than imported: a Lua file that fails
        // to load takes the whole session with it, and the session is the
        // thing the person holding this device is standing on.
        region(
            home.join(".config/hypr/hyprland.lua"),
            hyprland::spend(palette),
        ),
        // The unit that starts the wallpaper daemon, which sets the colour
        // behind everything for the moment before `console-sky` has chosen.
        region(
            files.join("etc/systemd/user/console-paper.service"),
            paper::spend(palette),
        ),
        // Drawn.
        whole(
            files.join("usr/share/icons/console-placeholder.svg"),
            icon::spend(palette),
        ),
    ]
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::spec::Spec;

    const DECLARED: &str = include_str!("../../../../theme/palette.toml");

    /// The palette this desktop actually wears.
    pub fn palette_spec() -> Spec {
        toml::from_str(DECLARED).expect("theme/palette.toml parses")
    }

    pub fn blossom() -> Palette {
        crate::palette::resolve(&palette_spec().colour).expect("it resolves")
    }

    fn spent() -> Vec<Written> {
        let (spec, palette) = (palette_spec(), blossom());
        let terminal = Terminal::of(&spec, &palette);
        everywhere(Path::new("files"), &palette, &terminal)
    }

    #[test]
    fn no_file_is_written_twice() {
        let written = spent();
        let mut paths: Vec<&PathBuf> = written.iter().map(|w| &w.path).collect();
        paths.sort();
        let once = {
            let mut seen = paths.clone();
            seen.dedup();
            seen
        };
        assert_eq!(paths.len(), once.len(), "a file is written twice");
    }

    #[test]
    fn nothing_written_whole_is_empty() {
        for written in spent().iter().filter(|w| w.how == How::Whole) {
            assert!(
                !written.body.trim().is_empty(),
                "{:?} is empty",
                written.path
            );
        }
    }

    #[test]
    fn a_whole_file_ends_in_a_newline_and_a_spliced_block_does_not() {
        for written in spent() {
            match written.how {
                How::Whole => assert!(
                    written.body.ends_with('\n'),
                    "{:?} is a whole file and does not end in a newline",
                    written.path
                ),
                How::Region => assert!(
                    !written.body.ends_with('\n'),
                    "{:?} is a block to splice and ends in a newline",
                    written.path
                ),
            }
        }
    }

    #[test]
    fn every_language_the_desktop_speaks_gets_a_palette() {
        let paths: Vec<String> = spent()
            .iter()
            .map(|w| w.path.display().to_string())
            .collect();
        for wanted in [
            ".config/console/palette.css",      // GTK, and everything that imports it
            ".config/console/palette.toml",     // the terminal
            "chrome/palette.css",              // the browser
            "usr/local/lib/console/palette.sh", // the keyboard
        ] {
            assert!(
                paths.iter().any(|p| p.contains(wanted)),
                "{wanted} is not written"
            );
        }
    }

    #[test]
    fn nothing_is_written_outside_the_tree_it_was_given() {
        for written in spent() {
            assert!(
                written.path.starts_with("files"),
                "{:?} is written outside files/",
                written.path
            );
        }
    }
}
