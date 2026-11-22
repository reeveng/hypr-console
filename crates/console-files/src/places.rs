//! Which tabs there are, and the order the shoulders walk them in.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;

use crate::paths::{self, ChildName, InvalidName, RelativePath};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Place {
  pub title: String,
  pub path: PathBuf,
}

impl Place {
  pub fn new(title: &str, path: PathBuf) -> Result<Self, Never> {
    Ok(Place {
      title: title.to_string(),
      path,
    })
  }
}

pub const WANTED: [&str; 6] = [
  "Home",
  "Documents",
  "Downloads",
  "Music",
  "Pictures",
  "Videos",
];

pub fn wanted_at(home: &Path, said: &[(&str, Option<PathBuf>)]) -> Result<Vec<Place>, Never> {
  let mut places: Vec<Place> = Vec::new();

  for (title, path) in said {
    let path_3 = match path.clone() {
      Some(path_2) => path_2,
      None => {
        let child = match ChildName::new(title) {
          Ok(child) => child,
          Err(InvalidName) => continue,
        };

        home.join(child)
      }
    };
    let place = Place::new(title, path_3)?;

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

      let named = match path.file_name().and_then(OsStr::to_str) {
        Some(named) => named.to_owned(),
        None => return Ok(None),
      };

      (holding.to_path_buf(), Some(named))
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

  let relative = match RelativePath::new(within) {
    Ok(relative) => relative,
    Err(InvalidName) => return Ok(None),
  };
  let steps = relative
    .as_ref()
    .components()
    .map(|step| step.as_os_str().to_str().map(str::to_owned))
    .collect::<Option<Vec<_>>>();

  Ok(steps.map(|hops| Leading {
    place,
    steps: hops,
    stand_on,
  }))
}

pub const MOUNTS: &str = "/proc/mounts";

pub const PLUGGED_IN: [&str; 3] = ["/run/media", "/media", "/mnt"];

pub fn plugged_in(said: &str) -> Result<Vec<Place>, Never> {
  let mut places: Vec<Place> = Vec::new();

  for line in said.lines() {
    let said_3 = match line.split_whitespace().nth(1) {
      Some(said_2) => said_2,
      None => continue,
    };

    let said_4 = unescaped(said_3)?;
    let at = PathBuf::from(said_4);

    let under = PLUGGED_IN
      .iter()
      .any(|root| at.starts_with(root) && at != Path::new(root));

    match ::core::primitive::bool::then_some(under, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => continue,
    }

    let Ok(title) = paths::display_name(&at);

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

    let rest_3 = match read {
      Some(byte) => {
        written.push(char::from(byte));

        match after.get(3..) {
          Some(rest_2) => rest_2,
          None => "",
        }
      }
      None => {
        written.push('\\');

        after
      }
    };

    Ok(Step::Again((rest_3, written)))
  });

  Ok(match unescaped {
    Ok(unescaped_2) => unescaped_2,
    Err(Endless) => said.to_string(),
  })
}

pub fn existing(places: Vec<Place>, there: impl Fn(&Path) -> bool) -> Result<Vec<Place>, Never> {
  Ok(
    places
      .into_iter()
      .filter(|place| there(&place.path))
      .collect(),
  )
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
    let Ok(places) = wanted_at(
      Path::new(HOME),
      &[
        ("Home", Some(PathBuf::from("/home/ada"))),
        ("Documents", Some(PathBuf::from("/home/ada/Documents"))),
        ("Pictures", Some(PathBuf::from("/home/ada/Pictures"))),
      ],
    );
    let Ok(kept) = existing(places, |path| path != Path::new("/home/ada/Documents"));
    let Ok(titles) = titles(&kept);

    assert_eq!(titles, ["Home", "Pictures"]);
  }

  #[test]
  fn the_tabs_come_out_in_the_order_they_were_asked_for() {
    let said: Vec<(&str, Option<PathBuf>)> = WANTED
      .iter()
      .map(|title| (*title, Some(PathBuf::from(HOME))))
      .collect();
    let Ok(wanted) = wanted_at(Path::new(HOME), &said);
    let Ok(places) = existing(wanted, |_| true);
    let Ok(titles) = titles(&places);

    assert_eq!(titles, WANTED);
  }

  #[test]
  fn a_place_the_machine_says_nothing_about_is_looked_for_under_its_own_name()
  -> Result<(), Box<dyn Error>> {
    let Ok(places) = wanted_at(
      Path::new(HOME),
      &[
        ("Downloads", Some(PathBuf::from("/data/downloads"))),
        ("Pictures", None),
      ],
    );
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
    let leading_2 = leading.ok_or("the way to it")?;

    assert_eq!(leading_2.place, 1);
    assert_eq!(leading_2.steps, ["Nujabes"]);
    assert_eq!(leading_2.stand_on.as_deref(), Some("aruarian dance.mp3"));

    Ok(())
  }

  #[test]
  fn a_thing_at_the_top_of_a_place_is_a_walk_of_no_steps() -> Result<(), Box<dyn Error>> {
    let song = Path::new(HOME).join("Music/505.opus");
    let Ok(two_places) = two_places();
    let Ok(leading) = leading_to(&two_places, &song, Is::AFile);
    let arrived = leading.ok_or("the way to it")?;

    assert_eq!(arrived.place, 1);
    assert!(arrived.steps.is_empty());
    assert_eq!(arrived.stand_on.as_deref(), Some("505.opus"));

    Ok(())
  }

  #[test]
  fn a_folder_is_the_place_arrived_at_rather_than_the_row_stood_on() -> Result<(), Box<dyn Error>>
  {
    let Ok(two_places) = two_places();
    let Ok(leading) = leading_to(
      &two_places,
      &Path::new(HOME).join("Music/Nujabes"),
      Is::AFolder,
    );
    let leading_2 = leading.ok_or("the way")?;

    assert_eq!(leading_2.steps, ["Nujabes"]);
    assert_eq!(leading_2.stand_on, None);

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

    assert_eq!(
      leading_to(&two_places, Path::new("/etc/fstab"), Is::AFile),
      Ok(None)
    );
    assert_eq!(
      leading_to(&two_places, Path::new("/"), Is::AFolder),
      Ok(None)
    );
  }
}
