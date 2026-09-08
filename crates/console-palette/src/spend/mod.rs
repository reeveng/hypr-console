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
pub mod mako;
pub mod paper;
pub mod shell;

use console_core_colour::Short;
use console_core_never::Never;
use std::path::{Path, PathBuf};

use crate::palette::Palette;
use crate::terminal::Terminal;

pub const ROLES: [&str; 17] = [
    "night", "ground", "panel", "fill", "edge", "text", "soft", "pink", "rose", "mauve", "lilac",
    "sky", "mint", "leaf", "butter", "peach", "coral",
];

pub fn widest<const N: usize>(names: [&str; N]) -> Result<usize, Never> {
    Ok(names.iter().map(|name| name.len()).max().unwrap_or(0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    Whole,
    Region,
}

#[derive(Debug, Clone)]
pub struct Written {
    pub path: PathBuf,
    pub how: How,
    pub body: String,
}

pub fn everywhere(
    files: &Path,
    palette: &Palette,
    terminal: &Terminal,
) -> Result<Vec<Written>, Short> {
    let home = files.join("home/@user@");
    let Ok(ours) = console_core_places::Base::Config.ours_under(&home);
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

    let css = gtk::spend(palette)?;
    let stylesheet = librewolf::stylesheet(palette)?;
    let sh = shell::spend(palette)?;
    let kdeglobals = kde::spend(palette)?;
    let mako = mako::spend(palette)?;
    let prefs = librewolf::prefs(palette)?;
    let hypr = hyprland::spend(palette)?;
    let paper = paper::spend(palette)?;
    let icon = icon::spend(palette)?;
    let Ok(colours) = alacritty::spend(terminal);

    Ok(vec![
        whole(ours.join("palette.css"), css),
        whole(ours.join("palette.toml"), colours),
        whole(chrome.join("palette.css"), stylesheet),
        whole(files.join("usr/local/lib/console/palette.sh"), sh),
        region(home.join(".config/kdeglobals"), kdeglobals),
        region(home.join(".config/mako/config"), mako),
        region(home.join(".librewolf/console/user.js"), prefs),
        region(home.join(".config/hypr/hyprland.lua"), hypr),
        region(files.join("etc/systemd/user/console-paper.service"), paper),
        whole(files.join("usr/share/icons/console-placeholder.svg"), icon),
    ])
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::spec::Spec;

    const DECLARED: &str = include_str!("../../../../theme/palette.toml");

    pub fn palette_spec() -> Spec {
        toml::from_str(DECLARED).expect("theme/palette.toml parses")
    }

    pub fn blossom() -> Palette {
        crate::palette::resolve(&palette_spec().colour).expect("it resolves")
    }

    fn spent() -> Vec<Written> {
        let (spec, palette) = (palette_spec(), blossom());
        let terminal = Terminal::of(&spec, &palette).expect("the terminal table is declared");
        everywhere(Path::new("files"), &palette, &terminal).expect("every colour it spends is declared")
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
        let Ok(ours) = console_core_places::Base::Config.ours_under(std::path::Path::new(""));

        for wanted in [
            ours.join("palette.css").display().to_string(),
            ours.join("palette.toml").display().to_string(),
            "chrome/palette.css".to_string(),
            "usr/local/lib/console/palette.sh".to_string(),
        ] {
            assert!(
                paths.iter().any(|p| p.contains(&wanted)),
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
