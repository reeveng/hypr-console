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

use console_never::Never;

pub const THEMES: [&str; 5] = ["Papirus-Dark", "Papirus", "hicolor", "breeze-dark", "breeze"];

pub const WANTED: i64 = 64;

pub const SMALLEST: i64 = 24;

pub const PLACEHOLDER: &str = "/usr/share/icons/console-placeholder.svg";

pub fn rank(theme: &str, size: &str, suffix: &str) -> Result<Option<(usize, u8, i64)>, Never> {
    let theme = THEMES.iter().position(|known| *known == theme).unwrap_or(THEMES.len());

    let said = digits(size)?;

    let pixels = match (size == "scalable", said) {
        (true, _) => WANTED,
        (false, Some(pixels)) => pixels,
        (false, None) => return Ok(None),
    };

    match pixels < SMALLEST {
        true => return Ok(None),
        false => {},
    }

    let drawn = u8::from(suffix != "svg");

    Ok(Some((theme, drawn, pixels.saturating_sub(WANTED).saturating_abs())))
}

fn digits(said: &str) -> Result<Option<i64>, Never> {
    let front: String = said.chars().take_while(char::is_ascii_digit).collect();

    let Ok(number) = front.parse() else { return Ok(None) };

    Ok(Some(number))
}

pub fn said_size(parts: &[String]) -> Result<String, Never> {
    Ok(parts
        .iter()
        .take(3)
        .skip(1)
        .find(|part| part == &"scalable" || part.starts_with(|first: char| first.is_ascii_digit()))
        .cloned()
        .unwrap_or_else(|| "48".to_string()))
}

pub fn written(index: &BTreeMap<String, String>) -> Result<String, Never> {
    Ok(index.iter().map(|(name, path)| format!("{name}\t{path}\n")).collect())
}

pub fn read(said: &str) -> Result<BTreeMap<String, String>, Never> {
    Ok(said
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(name, path)| (name.to_string(), path.to_string()))
        .collect())
}

pub fn built(roots: &[PathBuf]) -> Result<BTreeMap<String, String>, Never> {
    let mut best: BTreeMap<String, ((usize, u8, i64), String)> = BTreeMap::new();

    for root in roots {
        let under = under(root)?;

        for path in under {
            let Ok(inside) = path.parent().unwrap_or(root).strip_prefix(root) else { continue };

            let parts: Vec<String> =
                inside.components().map(|part| part.as_os_str().to_string_lossy().to_string()).collect();
            let theme = parts.first().cloned().unwrap_or_default();
            let suffix = path.extension().map(|kind| kind.to_string_lossy().to_string());

            let Some(suffix) = suffix else { continue };

            match ["png", "svg", "xpm"].contains(&suffix.as_str()) {
                true => {},
                false => continue,
            }

            let size = said_size(&parts)?;

            let Some(score) = rank(&theme, &size, &suffix)? else { continue };

            let Some(stem) = path.file_stem().map(|stem| stem.to_string_lossy().to_string()) else {
                continue;
            };

            let found = (score, path.to_string_lossy().to_string());

            match best.get(&stem) {
                Some((already, _)) if *already <= score => (),
                Some(_) | None => {
                    best.insert(stem, found);
                }
            }
        }
    }

    Ok(best.into_iter().map(|(name, (_, path))| (name, path)).collect())
}

fn under(root: &Path) -> Result<Vec<PathBuf>, Never> {
    let Ok(reading) = std::fs::read_dir(root) else { return Ok(Vec::new()) };

    let mut found = Vec::new();

    for entry in reading.filter_map(Result::ok) {
        let path = entry.path();

        match path.is_dir() {
            true => {
                let deeper = under(&path)?;

                found.extend(deeper);
            }
            false => found.push(path),
        }
    }

    Ok(found)
}

pub fn steam_appid(name: &str) -> Result<Option<&str>, Never> {
    let Some(rest) = name.strip_prefix("steam_icon_") else { return Ok(None) };

    Ok((!rest.is_empty() && rest.chars().all(|letter| letter.is_ascii_digit())).then_some(rest))
}

pub const FALLBACKS: [&str; 3] = ["library_600x900.jpg", "logo.png", "library_header.jpg"];

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    #[test]
    fn the_theme_this_machine_is_dressed_in_wins() {
        let papirus = ok(rank("Papirus-Dark", "64", "svg")).expect("a rank");
        let breeze = ok(rank("breeze", "64", "svg")).expect("a rank");
        assert!(papirus < breeze);
    }

    #[test]
    fn something_drawn_beats_something_pixelated() {
        assert!(ok(rank("hicolor", "64", "svg")) < ok(rank("hicolor", "64", "png")));
    }

    #[test]
    fn the_nearest_to_the_size_a_row_is_wins() {
        assert!(ok(rank("hicolor", "64", "png")) < ok(rank("hicolor", "128", "png")));
        assert!(ok(rank("hicolor", "48", "png")) < ok(rank("hicolor", "256", "png")));
        assert_eq!(ok(rank("hicolor", "scalable", "svg")), ok(rank("hicolor", "64", "svg")));
    }

    #[test]
    fn something_too_small_or_saying_nothing_is_no_use() {
        assert_eq!(ok(rank("hicolor", "16", "png")), None);
        assert_eq!(ok(rank("hicolor", "symbolic", "svg")), None);
    }

    #[test]
    fn whichever_part_of_the_path_looks_like_a_size_is_the_size() {
        let parts = |said: &str| said.split('/').map(str::to_string).collect::<Vec<String>>();
        assert_eq!(ok(said_size(&parts("Papirus/64x64/apps"))), "64x64");
        assert_eq!(ok(said_size(&parts("hicolor/apps/48x48"))), "48x48");
        assert_eq!(ok(said_size(&parts("hicolor/scalable/apps"))), "scalable");
        assert_eq!(ok(said_size(&parts("pixmaps"))), "48", "a tree with no sizes in it");
    }

    #[test]
    fn what_is_written_is_what_is_read() {
        let said = "firefox\t/usr/share/icons/Papirus/64x64/apps/firefox.svg\n";
        assert_eq!(ok(written(&ok(read(said)))), said);
    }

    #[test]
    fn a_steam_icon_is_named_by_the_game_it_is_for() {
        assert_eq!(ok(steam_appid("steam_icon_620")), Some("620"));
        assert_eq!(ok(steam_appid("firefox")), None);
        assert_eq!(ok(steam_appid("steam_icon_")), None);
    }
}
