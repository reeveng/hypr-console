//! Which tabs there are, and the order the shoulders walk them in.

use std::path::{Path, PathBuf};

use console_never::Never;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Place {
    pub title: String,
    pub path: PathBuf,
}

impl Place {
    pub fn new(title: &str, path: PathBuf) -> Result<Self, Never> {
        Ok(Place { title: title.to_string(), path })
    }
}

pub const WANTED: [&str; 6] = ["Home", "Documents", "Downloads", "Music", "Pictures", "Videos"];

pub fn wanted_at(home: &Path, said: &[(&str, Option<PathBuf>)]) -> Result<Vec<Place>, Never> {
    let mut places: Vec<Place> = Vec::new();

    for (title, path) in said {
        let path = path.clone().unwrap_or_else(|| home.join(title));
        let place = Place::new(title, path)?;

        places.push(place);
    }

    Ok(places)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Leading {
    pub place: usize,
    pub steps: Vec<String>,
    pub stand_on: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Is {
    AFolder,
    AFile,
}

pub fn leading_to(places: &[Place], path: &Path, folder: Is) -> Result<Option<Leading>, Never> {
    let (into, stand_on) = match folder {
        Is::AFolder => (path.to_path_buf(), None),
        Is::AFile => {
            let Some(holding) = path.parent() else { return Ok(None) };

            (
                holding.to_path_buf(),
                path.file_name().map(|name| name.to_string_lossy().to_string()),
            )
        }
    };

    let under = places
        .iter()
        .enumerate()
        .filter_map(|(at, place)| {
            let Ok(within) = into.strip_prefix(&place.path) else { return None };

            Some((at, within))
        })
        .max_by_key(|(at, _)| {
            places.get(*at).map_or(0, |place| place.path.components().count())
        });

    let Some((place, within)) = under else { return Ok(None) };

    Ok(Some(Leading {
        place,
        steps: within
            .components()
            .map(|step| step.as_os_str().to_string_lossy().to_string())
            .collect(),
        stand_on,
    }))
}

pub fn kept(places: Vec<Place>, there: impl Fn(&Path) -> bool) -> Result<Vec<Place>, Never> {
    Ok(places.into_iter().filter(|place| there(&place.path)).collect())
}

pub fn said_at(held: &str, name: &str, home: &Path) -> Result<Option<PathBuf>, Never> {
    let wanted = format!("{name}=");

    let found = held
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .find_map(|line| line.strip_prefix(&wanted));

    let Some(found) = found else { return Ok(None) };

    let said = found.trim().trim_matches('"');

    Ok(match said {
        "" => None,
        said => Some(match said.strip_prefix("$HOME/") {
            Some(rest) => home.join(rest),
            None => PathBuf::from(said),
        }),
    })
}

pub fn user_dirs(home: &Path) -> Result<PathBuf, Never> {
    Ok(home.join(".config/user-dirs.dirs"))
}

pub fn folder(home: &Path, name: &str, plain: &str) -> Result<PathBuf, Never> {
    let at = user_dirs(home)?;

    let held = match std::fs::read_to_string(&at) {
        Ok(held) => held,

        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => String::new(),

        Err(fault) => {
            eprintln!("console: {}: reading where this account keeps its folders: {fault}", at.display());
            String::new()
        }
    };

    let said = said_at(&held, name, home)?;

    Ok(said.unwrap_or_else(|| home.join(plain)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn at(path: &str) -> Option<PathBuf> {
        Some(Path::new(path).to_path_buf())
    }

    fn home() -> PathBuf {
        Path::new("/home/ada").to_path_buf()
    }

    fn titles(places: &[Place]) -> Vec<&str> {
        places.iter().map(|place| place.title.as_str()).collect()
    }

    fn place(title: &str, path: PathBuf) -> Place {
        let Ok(place) = Place::new(title, path);

        place
    }

    fn wanted(home: &Path, said: &[(&str, Option<PathBuf>)]) -> Vec<Place> {
        let Ok(places) = wanted_at(home, said);

        places
    }

    fn there(places: Vec<Place>, there: impl Fn(&Path) -> bool) -> Vec<Place> {
        let Ok(places) = kept(places, there);

        places
    }

    fn leading(places: &[Place], path: &Path, folder: Is) -> Option<Leading> {
        let Ok(leading) = leading_to(places, path, folder);

        leading
    }

    #[test]
    fn a_place_that_is_not_there_is_not_a_tab() {
        let places = wanted(&home(), &[
            ("Home", at("/home/ada")),
            ("Documents", at("/home/ada/Documents")),
            ("Pictures", at("/home/ada/Pictures")),
        ]);
        let kept = there(places, |path| path != Path::new("/home/ada/Documents"));

        assert_eq!(titles(&kept), ["Home", "Pictures"]);
    }

    #[test]
    fn the_tabs_come_out_in_the_order_they_were_asked_for() {
        let said: Vec<(&str, Option<PathBuf>)> =
            WANTED.iter().map(|title| (*title, at("/home/ada"))).collect();
        let places = there(wanted(&home(), &said), |_| true);

        assert_eq!(titles(&places), WANTED);
    }

    #[test]
    fn a_place_the_machine_says_nothing_about_is_looked_for_under_its_own_name() {
        let places = wanted(&home(), &[("Downloads", at("/data/downloads")), ("Pictures", None)]);

        assert_eq!(places[0].path, Path::new("/data/downloads"));
        assert_eq!(places[1].path, Path::new("/home/ada/Pictures"));
    }

    #[test]
    fn home_is_the_first_of_them() {
        assert_eq!(WANTED[0], "Home");
    }

    #[test]
    fn the_rest_are_in_the_alphabet_everybody_already_knows() {
        let mut rest = WANTED[1..].to_vec();
        rest.sort_by_key(|title| title.to_lowercase());
        assert_eq!(rest, WANTED[1..]);
    }

    fn two_places() -> Vec<Place> {
        vec![place("Home", home()), place("Music", home().join("Music"))]
    }

    #[test]
    fn a_path_arrives_in_the_most_particular_place_that_holds_it() {
        let song = home().join("Music/Nujabes/aruarian dance.mp3");
        let leading = leading(&two_places(), &song, Is::AFile).expect("the way to it");

        assert_eq!(leading.place, 1);
        assert_eq!(leading.steps, ["Nujabes"]);
        assert_eq!(leading.stand_on.as_deref(), Some("aruarian dance.mp3"));
    }

    #[test]
    fn a_thing_at_the_top_of_a_place_is_a_walk_of_no_steps() {
        let song = home().join("Music/505.opus");
        let leading = leading(&two_places(), &song, Is::AFile).expect("the way to it");

        assert_eq!(leading.place, 1);
        assert!(leading.steps.is_empty());
        assert_eq!(leading.stand_on.as_deref(), Some("505.opus"));
    }

    #[test]
    fn a_folder_is_the_place_arrived_at_rather_than_the_row_stood_on() {
        let leading =
            leading(&two_places(), &home().join("Music/Nujabes"), Is::AFolder).expect("the way");

        assert_eq!(leading.steps, ["Nujabes"]);
        assert_eq!(leading.stand_on, None);
    }

    #[test]
    fn a_path_under_none_of_the_places_leads_nowhere() {
        assert_eq!(leading(&two_places(), Path::new("/etc/fstab"), Is::AFile), None);
        assert_eq!(leading(&two_places(), Path::new("/"), Is::AFolder), None);
    }

    #[test]
    fn where_the_home_directory_says_its_pictures_are() {
        let held = "XDG_PICTURES_DIR=\"$HOME/Bilder\"\n";
        assert_eq!(
            said_at(held, "XDG_PICTURES_DIR", Path::new("/home/ada")),
            Ok(Some(PathBuf::from("/home/ada/Bilder")))
        );
    }

    #[test]
    fn a_path_that_is_not_under_the_home_directory_is_taken_as_it_is() {
        let held = "XDG_PICTURES_DIR=\"/data/pictures\"\n";
        assert_eq!(
            said_at(held, "XDG_PICTURES_DIR", Path::new("/home/ada")),
            Ok(Some(PathBuf::from("/data/pictures")))
        );
    }

    #[test]
    fn a_folder_the_file_says_nothing_about_is_nothing() {
        let held = "XDG_MUSIC_DIR=\"$HOME/Music\"\n";
        assert_eq!(said_at(held, "XDG_PICTURES_DIR", Path::new("/home/ada")), Ok(None));
        assert_eq!(said_at("", "XDG_PICTURES_DIR", Path::new("/home/ada")), Ok(None));
    }

    #[test]
    fn what_is_commented_out_is_not_read() {
        let held = "# XDG_PICTURES_DIR=\"$HOME/Wrong\"\nXDG_PICTURES_DIR=\"$HOME/Right\"\n";
        assert_eq!(
            said_at(held, "XDG_PICTURES_DIR", Path::new("/home/ada")),
            Ok(Some(PathBuf::from("/home/ada/Right")))
        );
    }

    #[test]
    fn a_folder_said_to_be_nothing_is_nothing() {
        assert_eq!(
            said_at("XDG_PICTURES_DIR=\"\"", "XDG_PICTURES_DIR", Path::new("/home/ada")),
            Ok(None),
        );
    }
}
