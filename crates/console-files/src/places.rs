//! Which tabs there are, and the order the shoulders walk them in.

use std::path::{Path, PathBuf};

/// One tab: a name on the strip, and where it starts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Place {
    pub title: String,
    pub path: PathBuf,
}

impl Place {
    pub fn new(title: &str, path: PathBuf) -> Self {
        Place { title: title.to_string(), path }
    }
}

/// The places a tab is offered for, in the order they are offered.
///
/// Home first because it is the one that holds the others, and because it is
/// where anything nobody filed ends up. The rest by name, which is the rule
/// everywhere here: an order somebody thought was most useful is an order the
/// next person has to learn, and the alphabet is one everybody already knows.
///
/// These are the names on the strip. Where each one actually is is the
/// machine's answer rather than this file's, because a home directory can have
/// its folders anywhere and in any language, and a Pictures tab that opens a
/// folder called Pictures that nothing else on the machine writes into is a tab
/// that is always empty.
pub const WANTED: [&str; 6] = ["Home", "Documents", "Downloads", "Music", "Pictures", "Videos"];

/// Where each place would be, before asking whether it is there.
///
/// The machine's answer comes out of a file the home directory keeps, and that
/// file names only the folders something has once had a reason to write down.
/// This device's names three of these and the rest are sitting in the home
/// directory anyway, so a place the file says nothing about is looked for under
/// its own name before it is given up on.
///
/// The plain name is the fallback rather than the answer, so a home directory
/// that keeps its pictures somewhere else is still believed. Asked the other
/// way round, the Pictures tab would open a folder that happens to be spelt
/// that way while the camera wrote into another one.
pub fn wanted_at(home: &Path, said: &[(&str, Option<PathBuf>)]) -> Vec<Place> {
    said.iter()
        .map(|(title, path)| {
            let path = path.clone().unwrap_or_else(|| home.join(title));
            Place::new(title, path)
        })
        .collect()
}

/// What a path asked for on the command line comes to.
///
/// Something else on the desktop has a thing in mind and wants the files
/// opened standing on it: the music panel has a song, and everything a person
/// does to a song that is not playing it -- renaming it, throwing it away --
/// lives here rather than there.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Leading {
    /// Which place it is in, as a number into the places offered.
    pub place: usize,
    /// The folders to walk into from the top of that place, in order.
    pub steps: Vec<String>,
    /// What to stand on once the walk is done, where the thing asked for was a
    /// file rather than a folder.
    pub stand_on: Option<String>,
}

/// Which place a path is in, and the way down to it from the top of that place.
///
/// The most particular place wins. A song is under Home as surely as it is
/// under Music, and arriving at Home three folders up from the thing asked for
/// is arriving somewhere nobody asked to be.
///
/// Whether it is a folder is handed in rather than asked of the disk, because
/// this is the sort of thing that is easier to be sure of with a table than
/// with a temporary directory.
pub fn leading_to(places: &[Place], path: &Path, folder: bool) -> Option<Leading> {
    let (into, stand_on) = match folder {
        true => (path.to_path_buf(), None),
        false => (
            path.parent()?.to_path_buf(),
            path.file_name().map(|name| name.to_string_lossy().to_string()),
        ),
    };
    let (place, within) = places
        .iter()
        .enumerate()
        .filter_map(|(at, place)| Some((at, into.strip_prefix(&place.path).ok()?)))
        .max_by_key(|(at, _)| places[*at].path.components().count())?;

    Some(Leading {
        place,
        steps: within
            .components()
            .map(|step| step.as_os_str().to_string_lossy().to_string())
            .collect(),
        stand_on,
    })
}

/// The ones that are actually there, in the order they were asked for.
///
/// A folder that does not exist is left out rather than shown empty. Nothing on
/// this device makes a Videos folder until something is put in one, and a tab
/// that can only ever say it is empty is a tab the shoulders have to be pressed
/// past.
pub fn kept(places: Vec<Place>, there: impl Fn(&Path) -> bool) -> Vec<Place> {
    places.into_iter().filter(|place| there(&place.path)).collect()
}

/// Where the home directory says one of its folders is.
///
/// The file names only the folders something has once had a reason to write
/// down, and it writes them as shell: a name, an equals sign, and a quoted path
/// that usually begins with `$HOME`. Nothing here runs a shell to read it --
/// the one expansion that file ever uses is the one expanded below.
///
/// This is the answer `wanted_at` is given by the machine, read without a
/// desktop library to ask. `console-screenshot` needs it and has none: a
/// screenshot written to a folder called Pictures on a machine whose pictures
/// are somewhere else is a screenshot nobody finds again.
pub fn said_at(held: &str, name: &str, home: &Path) -> Option<PathBuf> {
    let wanted = format!("{name}=");
    let said = held
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .find_map(|line| line.strip_prefix(&wanted))?
        .trim()
        .trim_matches('"');
    match said {
        "" => None,
        said => Some(match said.strip_prefix("$HOME/") {
            Some(rest) => home.join(rest),
            None => PathBuf::from(said),
        }),
    }
}

/// The file the answer is in.
pub fn user_dirs(home: &Path) -> PathBuf {
    home.join(".config/user-dirs.dirs")
}

/// One of the home directory's folders, wherever it is, made if it is not there.
///
/// The plain name under the home directory is the fallback and never the
/// answer, so a machine that keeps its pictures somewhere else is believed.
pub fn folder(home: &Path, name: &str, plain: &str) -> PathBuf {
    let held = std::fs::read_to_string(user_dirs(home)).unwrap_or_default();
    said_at(&held, name, home).unwrap_or_else(|| home.join(plain))
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

    #[test]
    fn a_place_that_is_not_there_is_not_a_tab() {
        let places = wanted_at(&home(), &[
            ("Home", at("/home/ada")),
            ("Documents", at("/home/ada/Documents")),
            ("Pictures", at("/home/ada/Pictures")),
        ]);
        let kept = kept(places, |path| path != Path::new("/home/ada/Documents"));
        assert_eq!(titles(&kept), ["Home", "Pictures"]);
    }

    #[test]
    fn the_tabs_come_out_in_the_order_they_were_asked_for() {
        let said: Vec<(&str, Option<PathBuf>)> =
            WANTED.iter().map(|title| (*title, at("/home/ada"))).collect();
        let places = kept(wanted_at(&home(), &said), |_| true);
        assert_eq!(titles(&places), WANTED);
    }

    /// The file a home directory keeps names the folders something once had a
    /// reason to write down, which on this device is three of the six. The
    /// other three are sitting there and were tabs nobody could reach.
    #[test]
    fn a_place_the_machine_says_nothing_about_is_looked_for_under_its_own_name() {
        let places = wanted_at(&home(), &[("Downloads", at("/data/downloads")), ("Pictures", None)]);
        assert_eq!(places[0].path, Path::new("/data/downloads"));
        assert_eq!(places[1].path, Path::new("/home/ada/Pictures"));
    }

    /// Home is the one that holds the others, so it is the one the panel opens
    /// on when nothing said otherwise.
    #[test]
    fn home_is_the_first_of_them() {
        assert_eq!(WANTED[0], "Home");
    }

    /// Everything after Home, which is the rule the rest of this repository
    /// keeps too.
    #[test]
    fn the_rest_are_in_the_alphabet_everybody_already_knows() {
        let mut rest = WANTED[1..].to_vec();
        rest.sort_by_key(|title| title.to_lowercase());
        assert_eq!(rest, WANTED[1..]);
    }

    fn two_places() -> Vec<Place> {
        vec![
            Place::new("Home", home()),
            Place::new("Music", home().join("Music")),
        ]
    }

    /// A song is under Home as surely as it is under Music, and the tab worth
    /// opening is the one that lands nearest the thing asked for.
    #[test]
    fn a_path_arrives_in_the_most_particular_place_that_holds_it() {
        let song = home().join("Music/Nujabes/aruarian dance.mp3");
        let leading = leading_to(&two_places(), &song, false).expect("the way to it");
        assert_eq!(leading.place, 1);
        assert_eq!(leading.steps, ["Nujabes"]);
        assert_eq!(leading.stand_on.as_deref(), Some("aruarian dance.mp3"));
    }

    /// Straight in the place itself: nothing to walk into, and the highlight
    /// still lands on the song.
    #[test]
    fn a_thing_at_the_top_of_a_place_is_a_walk_of_no_steps() {
        let song = home().join("Music/505.opus");
        let leading = leading_to(&two_places(), &song, false).expect("the way to it");
        assert_eq!(leading.place, 1);
        assert!(leading.steps.is_empty());
        assert_eq!(leading.stand_on.as_deref(), Some("505.opus"));
    }

    /// A folder is walked into rather than stood on, because what was asked
    /// for is what is in it.
    #[test]
    fn a_folder_is_the_place_arrived_at_rather_than_the_row_stood_on() {
        let leading =
            leading_to(&two_places(), &home().join("Music/Nujabes"), true).expect("the way");
        assert_eq!(leading.steps, ["Nujabes"]);
        assert_eq!(leading.stand_on, None);
    }

    /// Which is what makes a path nothing here holds fall back to being read
    /// as the name of a tab.
    #[test]
    fn a_path_under_none_of_the_places_leads_nowhere() {
        assert_eq!(leading_to(&two_places(), Path::new("/etc/fstab"), false), None);
        assert_eq!(leading_to(&two_places(), Path::new("/"), true), None);
    }

    #[test]
    fn where_the_home_directory_says_its_pictures_are() {
        let held = "XDG_PICTURES_DIR=\"$HOME/Bilder\"\n";
        assert_eq!(
            said_at(held, "XDG_PICTURES_DIR", Path::new("/home/ada")),
            Some(PathBuf::from("/home/ada/Bilder"))
        );
    }

    /// A folder somewhere else entirely, which is a stick or another disk.
    #[test]
    fn a_path_that_is_not_under_the_home_directory_is_taken_as_it_is() {
        let held = "XDG_PICTURES_DIR=\"/data/pictures\"\n";
        assert_eq!(
            said_at(held, "XDG_PICTURES_DIR", Path::new("/home/ada")),
            Some(PathBuf::from("/data/pictures"))
        );
    }

    /// The file names only what something has had a reason to write down, so
    /// most of these are missing on most machines.
    #[test]
    fn a_folder_the_file_says_nothing_about_is_nothing() {
        let held = "XDG_MUSIC_DIR=\"$HOME/Music\"\n";
        assert_eq!(said_at(held, "XDG_PICTURES_DIR", Path::new("/home/ada")), None);
        assert_eq!(said_at("", "XDG_PICTURES_DIR", Path::new("/home/ada")), None);
    }

    /// The file is written with a comment at the top of it by the tool that
    /// generates it, and a commented-out line is not an answer.
    #[test]
    fn what_is_commented_out_is_not_read() {
        let held = "# XDG_PICTURES_DIR=\"$HOME/Wrong\"\nXDG_PICTURES_DIR=\"$HOME/Right\"\n";
        assert_eq!(
            said_at(held, "XDG_PICTURES_DIR", Path::new("/home/ada")),
            Some(PathBuf::from("/home/ada/Right"))
        );
    }

    /// An empty value means the person has said this folder is their home
    /// directory itself, which is not a folder to write screenshots into.
    #[test]
    fn a_folder_said_to_be_nothing_is_nothing() {
        assert_eq!(said_at("XDG_PICTURES_DIR=\"\"", "XDG_PICTURES_DIR", Path::new("/home/ada")), None);
    }
}
