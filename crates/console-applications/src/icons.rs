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

use console_core_directory_listing::Descend;
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

pub fn serialize(index: &BTreeMap<String, String>) -> Result<String, Never> {
    Ok(index.iter().map(|(name, path)| format!("{name}\t{path}\n")).collect())
}

pub fn read(said: &str) -> Result<BTreeMap<String, String>, Never> {
    Ok(said
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(name, path)| (name.to_string(), path.to_string()))
        .collect())
}

fn index_entry(root: &Path, path: &Path) -> Result<Option<(String, Best)>, Never> {
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
            let Ok(indexed) = index_entry(root, &path);

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
    let Ok(listing) = console_core_directory_listing::recursive(root, |_| Descend::Into);

    Ok(listing
        .filter_map(|entry| match entry {
            Ok(path) => match path.is_dir() {
                true => None,
                false => Some(path),
            },
            Err(_unreadable) => None,
        })
        .collect())
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
    use std::error::Error;

    fn parts(said: &str) -> Result<Vec<String>, Never> {
        Ok(said.split('/').map(str::to_string).collect())
    }

    fn ranked(icon: Icon<'_>) -> Result<Score, Box<dyn Error>> {
        let Ok(rank) = rank(icon);
        let score = rank.ok_or("an icon worth nothing")?;

        Ok(score)
    }

    #[test]
    fn the_theme_this_machine_is_dressed_in_wins() -> Result<(), Box<dyn Error>> {
        let papirus = ranked(Icon { theme: "Papirus-Dark", size: "64", suffix: "svg" })?;
        let breeze = ranked(Icon { theme: "breeze", size: "64", suffix: "svg" })?;

        assert!(papirus < breeze);

        Ok(())
    }

    #[test]
    fn something_drawn_beats_something_pixelated() -> Result<(), Box<dyn Error>> {
        let drawn = ranked(Icon { theme: "hicolor", size: "64", suffix: "svg" })?;
        let pixelated = ranked(Icon { theme: "hicolor", size: "64", suffix: "png" })?;

        assert!(drawn < pixelated);

        Ok(())
    }

    #[test]
    fn the_nearest_to_the_size_a_row_is_wins() -> Result<(), Box<dyn Error>> {
        let sixty_four = ranked(Icon { theme: "hicolor", size: "64", suffix: "png" })?;
        let one_twenty_eight = ranked(Icon { theme: "hicolor", size: "128", suffix: "png" })?;
        let forty_eight = ranked(Icon { theme: "hicolor", size: "48", suffix: "png" })?;
        let two_fifty_six = ranked(Icon { theme: "hicolor", size: "256", suffix: "png" })?;
        let scalable = ranked(Icon { theme: "hicolor", size: "scalable", suffix: "svg" })?;
        let drawn = ranked(Icon { theme: "hicolor", size: "64", suffix: "svg" })?;

        assert!(sixty_four < one_twenty_eight);
        assert!(forty_eight < two_fifty_six);
        assert_eq!(scalable, drawn);

        Ok(())
    }

    #[test]
    fn something_too_small_or_saying_nothing_is_no_use() {
        let tiny = Icon { theme: "hicolor", size: "16", suffix: "png" };
        let unsaid = Icon { theme: "hicolor", size: "symbolic", suffix: "svg" };

        assert_eq!(rank(tiny), Ok(None));
        assert_eq!(rank(unsaid), Ok(None));
    }

    #[test]
    fn whichever_part_of_the_path_looks_like_a_size_is_the_size() {
        for (path, size) in [
            ("Papirus/64x64/apps", "64x64"),
            ("hicolor/apps/48x48", "48x48"),
            ("hicolor/scalable/apps", "scalable"),
            ("pixmaps", "48"),
        ] {
            let Ok(parts) = parts(path);

            assert_eq!(said_size(&parts), Ok(size.to_string()), "{path}");
        }
    }

    #[test]
    fn what_is_written_is_what_is_read() {
        let said = "firefox\t/usr/share/icons/Papirus/64x64/apps/firefox.svg\n";

        let Ok(index) = read(said);

        assert_eq!(serialize(&index), Ok(said.to_string()));
    }

    #[test]
    fn a_steam_icon_is_named_by_the_game_it_is_for() {
        assert_eq!(steam_appid("steam_icon_620"), Ok(Some("620")));
        assert_eq!(steam_appid("firefox"), Ok(None));
        assert_eq!(steam_appid("steam_icon_"), Ok(None));
    }
}
