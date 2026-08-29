//! Where an icon named in a .desktop file is actually kept.
//!
//! Guessing the path does not work: icons sit under apps, devices or
//! preferences depending on what they are, in whatever sizes the theme happened
//! to ship, and a dark theme inherits most of its icons from the light one it
//! is built from.
//!
//! So the whole tree is indexed once and the answer kept. Walking it takes
//! about a second, which is a second too long to spend every time the menu
//! opens, and installing something is the only thing that changes the answer.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The themes worth having, best first.
pub const THEMES: [&str; 5] = ["Papirus-Dark", "Papirus", "hicolor", "breeze-dark", "breeze"];

/// The size a row in the menu is drawn at.
pub const WANTED: i64 = 64;

/// Smaller than this and it is an icon for something else.
pub const SMALLEST: i64 = 24;

/// Drawn for the applications that ship no icon, so their names still line up
/// with everything else.
pub const PLACEHOLDER: &str = "/usr/share/icons/console-placeholder.svg";

/// How good a file is for one name. Lower is better: the right theme, drawn
/// rather than pixelated, and near the size a row is.
///
/// Nothing at all where the file is no use: too small to read, or in a
/// directory that says nothing about how big it is.
pub fn rank(theme: &str, size: &str, suffix: &str) -> Option<(usize, u8, i64)> {
    let theme = THEMES.iter().position(|known| *known == theme).unwrap_or(THEMES.len());
    let pixels = match size == "scalable" {
        true => WANTED,
        false => digits(size)?,
    };
    if pixels < SMALLEST {
        return None;
    }
    let drawn = u8::from(suffix != "svg");
    Some((theme, drawn, (pixels - WANTED).abs()))
}

fn digits(said: &str) -> Option<i64> {
    let front: String = said.chars().take_while(char::is_ascii_digit).collect();
    front.parse().ok()
}

/// Which part of a path is the size.
///
/// One layout puts the size before the category and the other after, so
/// whichever part looks like a size is the size.
pub fn said_size(parts: &[String]) -> String {
    parts
        .iter()
        .take(3)
        .skip(1)
        .find(|part| part == &"scalable" || part.starts_with(|first: char| first.is_ascii_digit()))
        .cloned()
        .unwrap_or_else(|| "48".to_string())
}

/// The index, as it is kept between openings.
pub fn written(index: &BTreeMap<String, String>) -> String {
    index.iter().map(|(name, path)| format!("{name}\t{path}\n")).collect()
}

pub fn read(said: &str) -> BTreeMap<String, String> {
    said.lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(name, path)| (name.to_string(), path.to_string()))
        .collect()
}

/// The best file for every icon name, over every theme on the machine.
pub fn built(roots: &[PathBuf]) -> BTreeMap<String, String> {
    let mut best: BTreeMap<String, ((usize, u8, i64), String)> = BTreeMap::new();
    for root in roots {
        for path in under(root) {
            let Ok(inside) = path.parent().unwrap_or(root).strip_prefix(root) else { continue };
            let parts: Vec<String> =
                inside.components().map(|part| part.as_os_str().to_string_lossy().to_string()).collect();
            let theme = parts.first().cloned().unwrap_or_default();
            let suffix = path.extension().map(|kind| kind.to_string_lossy().to_string());
            let Some(suffix) = suffix else { continue };
            if !["png", "svg", "xpm"].contains(&suffix.as_str()) {
                continue;
            }
            let Some(score) = rank(&theme, &said_size(&parts), &suffix) else { continue };
            let Some(stem) = path.file_stem().map(|stem| stem.to_string_lossy().to_string()) else {
                continue;
            };
            let found = (score, path.to_string_lossy().to_string());
            match best.get(&stem) {
                Some((already, _)) if *already <= score => (),
                _ => {
                    best.insert(stem, found);
                }
            }
        }
    }
    best.into_iter().map(|(name, (_, path))| (name, path)).collect()
}

/// Every file under one directory, however deep.
fn under(root: &Path) -> Vec<PathBuf> {
    let Ok(reading) = std::fs::read_dir(root) else { return Vec::new() };
    let mut found = Vec::new();
    for entry in reading.filter_map(Result::ok) {
        let path = entry.path();
        match path.is_dir() {
            true => found.extend(under(&path)),
            false => found.push(path),
        }
    }
    found
}

/// The appid of a Steam icon, where the name is one.
///
/// Steam writes an icon named steam_icon_<appid> into the .desktop file but
/// does not always put a file of that name anywhere.
pub fn steam_appid(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("steam_icon_")?;
    (!rest.is_empty() && rest.chars().all(|letter| letter.is_ascii_digit())).then_some(rest)
}

/// The art Steam keeps is named by content hash, so a game's icon is found by
/// shape instead: it is the square one. The rest is cover art and banners,
/// which are the wrong shape for a row in a list, and are used only if there is
/// no icon at all.
pub const FALLBACKS: [&str; 3] = ["library_600x900.jpg", "logo.png", "library_header.jpg"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_theme_this_machine_is_dressed_in_wins() {
        let papirus = rank("Papirus-Dark", "64", "svg").expect("a rank");
        let breeze = rank("breeze", "64", "svg").expect("a rank");
        assert!(papirus < breeze);
    }

    #[test]
    fn something_drawn_beats_something_pixelated() {
        assert!(rank("hicolor", "64", "svg") < rank("hicolor", "64", "png"));
    }

    #[test]
    fn the_nearest_to_the_size_a_row_is_wins() {
        assert!(rank("hicolor", "64", "png") < rank("hicolor", "128", "png"));
        assert!(rank("hicolor", "48", "png") < rank("hicolor", "256", "png"));
        assert_eq!(rank("hicolor", "scalable", "svg"), rank("hicolor", "64", "svg"));
    }

    /// A 16-pixel icon drawn at 64 is a smear, and a directory that says
    /// nothing about size is a directory of anything at all.
    #[test]
    fn something_too_small_or_saying_nothing_is_no_use() {
        assert_eq!(rank("hicolor", "16", "png"), None);
        assert_eq!(rank("hicolor", "symbolic", "svg"), None);
    }

    #[test]
    fn whichever_part_of_the_path_looks_like_a_size_is_the_size() {
        let parts = |said: &str| said.split('/').map(str::to_string).collect::<Vec<String>>();
        assert_eq!(said_size(&parts("Papirus/64x64/apps")), "64x64");
        assert_eq!(said_size(&parts("hicolor/apps/48x48")), "48x48");
        assert_eq!(said_size(&parts("hicolor/scalable/apps")), "scalable");
        assert_eq!(said_size(&parts("pixmaps")), "48", "a tree with no sizes in it");
    }

    #[test]
    fn what_is_written_is_what_is_read() {
        let said = "firefox\t/usr/share/icons/Papirus/64x64/apps/firefox.svg\n";
        assert_eq!(written(&read(said)), said);
    }

    #[test]
    fn a_steam_icon_is_named_by_the_game_it_is_for() {
        assert_eq!(steam_appid("steam_icon_620"), Some("620"));
        assert_eq!(steam_appid("firefox"), None);
        assert_eq!(steam_appid("steam_icon_"), None);
    }
}
