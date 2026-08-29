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

/// The ones that are actually there, in the order they were asked for.
///
/// A folder that does not exist is left out rather than shown empty. Nothing on
/// this device makes a Videos folder until something is put in one, and a tab
/// that can only ever say it is empty is a tab the shoulders have to be pressed
/// past.
pub fn kept(places: Vec<Place>, there: impl Fn(&Path) -> bool) -> Vec<Place> {
    places.into_iter().filter(|place| there(&place.path)).collect()
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
}
