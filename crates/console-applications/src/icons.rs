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

use console_core_never::Never;
use console_core_number_conversion::fitted;

pub const THEMES: [&str; 5] = ["Papirus-Dark", "Papirus", "hicolor", "breeze-dark", "breeze"];

const WHEN_NO_SIZE_IS_SAID: &str = "48";

pub const WANTED: i64 = 64;

pub const SMALLEST: i64 = 24;

pub const PLACEHOLDER: &str = "/usr/share/icons/console-placeholder.svg";

type Score = (u32, u8, i64);

type Best = (Score, String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icon<'a> {
    pub theme: &'a str,
    pub size: &'a str,
    pub suffix: &'a str,
}

pub fn rank(icon: Icon<'_>) -> Result<Option<Score>, Never> {
    let Icon { theme, size, suffix } = icon;
    let Ok(behind_every_theme_we_know) = fitted::<_, u32>(THEMES.len());
    let theme = match (0..).zip(THEMES).find(|(_, known)| *known == theme) {
        Some((at, _)) => at,
        None => behind_every_theme_we_know,
    };

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

    let number = match front.parse() {
        Ok(number) => number,
        Err(_not_a_number) => return Ok(None),
    };

    Ok(Some(number))
}

pub fn said_size(parts: &[String]) -> Result<String, Never> {
    let found = parts
        .iter()
        .take(3)
        .skip(1)
        .find(|part| part == &"scalable" || part.starts_with(|first: char| first.is_ascii_digit()));

    Ok(match found {
        Some(said) => said.clone(),
        None => WHEN_NO_SIZE_IS_SAID.to_string(),
    })
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

fn indexed(root: &Path, path: &Path) -> Result<Option<(String, Best)>, Never> {
    let holding = match path.parent() {
        Some(holding) => holding,
        None => root,
    };

    let inside = match holding.strip_prefix(root) {
        Ok(inside) => inside,
        Err(_outside_the_folder) => return Ok(None),
    };

    let parts: Vec<String> =
        inside.components().map(|part| part.as_os_str().to_string_lossy().to_string()).collect();

    let theme = match parts.first() {
        Some(theme) => theme.clone(),
        None => String::new(),
    };

    let suffix = match path.extension().map(|kind| kind.to_string_lossy().to_string()) {
        Some(suffix) => suffix,
        None => return Ok(None),
    };

    match ["png", "svg", "xpm"].contains(&suffix.as_str()) {
        true => {},
        false => return Ok(None),
    }

    let size = said_size(&parts)?;

    let score = rank(Icon { theme: &theme, size: &size, suffix: &suffix })?;

    let score = match score {
        Some(score) => score,
        None => return Ok(None),
    };

    let stem = match path.file_stem().map(|stem| stem.to_string_lossy().to_string()) {
        Some(stem) => stem,
        None => return Ok(None),
    };

    Ok(Some((stem, (score, path.to_string_lossy().to_string()))))
}

pub fn built(roots: &[PathBuf]) -> Result<BTreeMap<String, String>, Never> {
    let mut best: BTreeMap<String, Best> = BTreeMap::new();

    for root in roots {
        let under = under(root)?;

        let found = under.into_iter().filter_map(|path| {
            let Ok(indexed) = indexed(root, &path);

            indexed
        });

        for (stem, (score, at)) in found {
            let nearer = match best.get(&stem) {
                Some((already, _)) => score < *already,
                None => true,
            };

            match nearer {
                true => {
                    best.insert(stem, (score, at));
                }
                false => {},
            }
        }
    }

    Ok(best.into_iter().map(|(name, (_, path))| (name, path)).collect())
}

fn under(root: &Path) -> Result<Vec<PathBuf>, Never> {
    let mut found = Vec::new();
    let mut waiting = listed(root)?;

    while let Some(path) = waiting.pop() {
        match path.is_dir() {
            true => {
                let deeper = listed(&path)?;

                waiting.extend(deeper);
            }
            false => found.push(path),
        }
    }

    Ok(found)
}

fn listed(directory: &Path) -> Result<Vec<PathBuf>, Never> {
    let reading = match std::fs::read_dir(directory) {
        Ok(reading) => reading,
        Err(_unreadable) => return Ok(Vec::new()),
    };

    Ok(reading.filter_map(Result::ok).map(|entry| entry.path()).collect())
}

pub fn steam_appid(name: &str) -> Result<Option<&str>, Never> {
    let rest = match name.strip_prefix("steam_icon_") {
        Some(rest) => rest,
        None => return Ok(None),
    };

    Ok((!rest.is_empty() && rest.chars().all(|letter| letter.is_ascii_digit())).then_some(rest))
}

pub const FALLBACKS: [&str; 3] = ["library_600x900.jpg", "logo.png", "library_header.jpg"];

pub const UNPICTURED: &str = "application-x-executable";

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    #[test]
    fn the_theme_this_machine_is_dressed_in_wins() {
        let dressed = Icon { theme: "Papirus-Dark", size: "64", suffix: "svg" };
        let other = Icon { theme: "breeze", size: "64", suffix: "svg" };

        let papirus = ok(rank(dressed)).expect("a rank");
        let breeze = ok(rank(other)).expect("a rank");
        assert!(papirus < breeze);
    }

    #[test]
    fn something_drawn_beats_something_pixelated() {
        let drawn = Icon { theme: "hicolor", size: "64", suffix: "svg" };
        let pixelated = Icon { theme: "hicolor", size: "64", suffix: "png" };

        assert!(ok(rank(drawn)) < ok(rank(pixelated)));
    }

    #[test]
    fn the_nearest_to_the_size_a_row_is_wins() {
        let sized = |size| Icon { theme: "hicolor", size, suffix: "png" };
        let drawn = |size| Icon { theme: "hicolor", size, suffix: "svg" };

        assert!(ok(rank(sized("64"))) < ok(rank(sized("128"))));
        assert!(ok(rank(sized("48"))) < ok(rank(sized("256"))));
        assert_eq!(ok(rank(drawn("scalable"))), ok(rank(drawn("64"))));
    }

    #[test]
    fn something_too_small_or_saying_nothing_is_no_use() {
        let tiny = Icon { theme: "hicolor", size: "16", suffix: "png" };
        let unsaid = Icon { theme: "hicolor", size: "symbolic", suffix: "svg" };

        assert_eq!(ok(rank(tiny)), None);
        assert_eq!(ok(rank(unsaid)), None);
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
