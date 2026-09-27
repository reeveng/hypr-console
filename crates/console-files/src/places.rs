//! Which tabs there are, and the order the shoulders walk them in.

use std::path::{Path, PathBuf};

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;

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
        let path = match path.clone() {
            Some(path) => path,
            None => home.join(title),
        };
        let place = Place::new(title, path)?;

        places.push(place);
    }

    Ok(places)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Leading {
    pub place: u32,
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
            let holding = match path.parent() {
                Some(holding) => holding,
                None => return Ok(None),
            };

            (
                holding.to_path_buf(),
                path.file_name().map(|name| name.to_string_lossy().to_string()),
            )
        }
    };

    let under = (0_u32..)
        .zip(places)
        .filter_map(|(at, place)| {
            let within = match into.strip_prefix(&place.path) {
                Ok(within) => within,
                Err(_outside_the_folder) => return None,
            };

            Some((at, place, within))
        })
        .max_by_key(|(_, place, _)| place.path.components().count());

    let (place, within) = match under {
        Some((place, _, within)) => (place, within),
        None => return Ok(None),
    };

    Ok(Some(Leading {
        place,
        steps: within
            .components()
            .map(|step| step.as_os_str().to_string_lossy().to_string())
            .collect(),
        stand_on,
    }))
}

pub const MOUNTS: &str = "/proc/mounts";

pub const PLUGGED_IN: [&str; 3] = ["/run/media", "/media", "/mnt"];

pub fn plugged_in(said: &str) -> Result<Vec<Place>, Never> {
    let mut places: Vec<Place> = Vec::new();

    for line in said.lines() {
        let said = match line.split_whitespace().nth(1) {
            Some(said) => said,
            None => continue,
        };

        let said = unescaped(said)?;
        let at = PathBuf::from(said);

        let under = PLUGGED_IN.iter().any(|root| at.starts_with(root) && at != Path::new(root));

        match under {
            true => {},
            false => continue,
        }

        let title = match at.file_name() {
            Some(title) => title.to_string_lossy().to_string(),
            None => continue,
        };

        let place = Place::new(&title, at)?;

        places.push(place);
    }

    Ok(places)
}

fn unescaped(said: &str) -> Result<String, Never> {
    let unescaped = iterate((said, String::new()), |(rest, mut written)| {
        let (before, after) = match rest.split_once('\\') {
            Some(halves) => halves,
            None => {
                written.push_str(rest);

                return Ok(Step::Halt(written));
            }
        };

        written.push_str(before);

        let read = match after.get(..3) {
            Some(three) => match u8::from_str_radix(three, 8) {
                Ok(byte) => Some(byte),
                Err(_that_was_not_three_octal_digits) => None,
            },
            None => None,
        };

        let rest = match read {
            Some(byte) => {
                written.push(char::from(byte));

                match after.get(3..) {
                    Some(rest) => rest,
                    None => "",
                }
            }
            None => {
                written.push('\\');

                after
            }
        };

        Ok(Step::Again((rest, written)))
    });

    Ok(match unescaped {
        Ok(unescaped) => unescaped,
        Err(Endless) => said.to_string(),
    })
}

pub fn existing(places: Vec<Place>, there: impl Fn(&Path) -> bool) -> Result<Vec<Place>, Never> {
    Ok(places.into_iter().filter(|place| there(&place.path)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::path::Path;

    const HOME: &str = "/home/ada";

    fn titles(places: &[Place]) -> Result<Vec<&str>, Never> {
        Ok(places.iter().map(|place| place.title.as_str()).collect())
    }

    fn two_places() -> Result<Vec<Place>, Never> {
        let Ok(home) = Place::new("Home", PathBuf::from(HOME));
        let Ok(music) = Place::new("Music", Path::new(HOME).join("Music"));

        Ok(vec![home, music])
    }

    #[test]
    fn a_place_that_is_not_there_is_not_a_tab() {
        let Ok(places) = wanted_at(Path::new(HOME), &[
            ("Home", Some(PathBuf::from("/home/ada"))),
            ("Documents", Some(PathBuf::from("/home/ada/Documents"))),
            ("Pictures", Some(PathBuf::from("/home/ada/Pictures"))),
        ]);
        let Ok(kept) = existing(places, |path| path != Path::new("/home/ada/Documents"));
        let Ok(titles) = titles(&kept);

        assert_eq!(titles, ["Home", "Pictures"]);
    }

    #[test]
    fn the_tabs_come_out_in_the_order_they_were_asked_for() {
        let said: Vec<(&str, Option<PathBuf>)> =
            WANTED.iter().map(|title| (*title, Some(PathBuf::from(HOME)))).collect();
        let Ok(wanted) = wanted_at(Path::new(HOME), &said);
        let Ok(places) = existing(wanted, |_| true);
        let Ok(titles) = titles(&places);

        assert_eq!(titles, WANTED);
    }

    #[test]
    fn a_place_the_machine_says_nothing_about_is_looked_for_under_its_own_name() -> Result<(), Box<dyn Error>> {
        let Ok(places) =
            wanted_at(Path::new(HOME), &[("Downloads", Some(PathBuf::from("/data/downloads"))), ("Pictures", None)]);
        let [downloads, pictures] = places.first_chunk::<2>().ok_or("two places")?;

        assert_eq!(downloads.path, Path::new("/data/downloads"));
        assert_eq!(pictures.path, Path::new("/home/ada/Pictures"));

        Ok(())
    }

    #[test]
    fn home_is_the_first_of_them() {
        assert_eq!(WANTED.first(), Some(&"Home"));
    }

    #[test]
    fn the_rest_are_in_the_alphabet_everyone_already_knows() -> Result<(), Box<dyn Error>> {
        let (_home, rest) = WANTED.split_first().ok_or("a first place")?;
        let mut sorted = rest.to_vec();

        sorted.sort_by_key(|title| title.to_lowercase());

        assert_eq!(sorted, rest);

        Ok(())
    }

    #[test]
    fn a_path_arrives_in_the_most_particular_place_that_holds_it() -> Result<(), Box<dyn Error>> {
        let song = Path::new(HOME).join("Music/Nujabes/aruarian dance.mp3");
        let Ok(two_places) = two_places();
        let Ok(leading) = leading_to(&two_places, &song, Is::AFile);
        let leading = leading.ok_or("the way to it")?;

        assert_eq!(leading.place, 1);
        assert_eq!(leading.steps, ["Nujabes"]);
        assert_eq!(leading.stand_on.as_deref(), Some("aruarian dance.mp3"));

        Ok(())
    }

    #[test]
    fn a_thing_at_the_top_of_a_place_is_a_walk_of_no_steps() -> Result<(), Box<dyn Error>> {
        let song = Path::new(HOME).join("Music/505.opus");
        let Ok(two_places) = two_places();
        let Ok(leading) = leading_to(&two_places, &song, Is::AFile);
        let leading = leading.ok_or("the way to it")?;

        assert_eq!(leading.place, 1);
        assert!(leading.steps.is_empty());
        assert_eq!(leading.stand_on.as_deref(), Some("505.opus"));

        Ok(())
    }

    #[test]
    fn a_folder_is_the_place_arrived_at_rather_than_the_row_stood_on() -> Result<(), Box<dyn Error>> {
        let Ok(two_places) = two_places();
        let Ok(leading) = leading_to(&two_places, &Path::new(HOME).join("Music/Nujabes"), Is::AFolder);
        let leading = leading.ok_or("the way")?;

        assert_eq!(leading.steps, ["Nujabes"]);
        assert_eq!(leading.stand_on, None);

        Ok(())
    }

    #[test]
    fn what_is_plugged_in_is_what_is_mounted_where_a_stick_lands() {
        let held = concat!(
            "/dev/nvme0n1p2 / btrfs rw,relatime 0 0\n",
            "tmpfs /run tmpfs rw 0 0\n",
            "/dev/sda1 /run/media/someone/Field\\040Notes vfat rw 0 0\n",
            "/dev/sdb1 /mnt/films ext4 rw 0 0\n",
        );

        let Ok(places) = plugged_in(held);
        let Ok(titles) = titles(&places);

        assert_eq!(titles, ["Field Notes", "films"]);
        assert_eq!(
            places.first().map(|place| place.path.clone()),
            Some(PathBuf::from("/run/media/someone/Field Notes"))
        );
    }

    #[test]
    fn the_root_of_where_sticks_land_is_not_itself_a_stick() {
        let Ok(places) = plugged_in("tmpfs /run/media tmpfs rw 0 0\n");

        assert!(places.is_empty());
    }

    #[test]
    fn a_path_under_none_of_the_places_leads_nowhere() {
        let Ok(two_places) = two_places();

        assert_eq!(leading_to(&two_places, Path::new("/etc/fstab"), Is::AFile), Ok(None));
        assert_eq!(leading_to(&two_places, Path::new("/"), Is::AFolder), Ok(None));
    }
}
