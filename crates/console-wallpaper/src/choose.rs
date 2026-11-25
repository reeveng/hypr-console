//! Which picture is up.
//!
//! Nothing that draws a picture is in this file and nothing in this file draws
//! one. What is here is the table: a set of pictures, each saying what outside
//! it answers, and the rule for picking one when the outside has been read.
//!
//! A picture may name four things, and any of them it does not name it answers
//! all of. The part of the day, from the sun. The weather, from a service. The
//! season, from where the sun is on the ecliptic, which gets the southern
//! hemisphere right without being told about it. And the moon, which needs
//! nothing but a clock.
//!
//! The rule is that the most particular picture wins: the one naming the most
//! things that are true. A picture for a full-moon winter night beats one for
//! any winter night, which beats one for any night, which beats one that names
//! nothing at all. That makes a set easy to grow, because a picture for a case
//! no one has covered yet can be added without a line of any other picture
//! changing.
//!
//! Pictures that are equally particular take turns. The turn is the clock cut
//! into two hour lengths, so one of them holds for a couple of hours and the
//! next of the same standing takes it from there, and the set of them comes
//! round again by the end of the day. That is what makes a set worth growing
//! sideways as well as downwards: a second picture for a clear summer day is
//! half of the clear summer days rather than a picture no one ever sees.
//!
//! Which of them goes first is the order they are written down in. It is
//! arbitrary, and it is arbitrary in a way someone can see and reorder, which
//! is more than picking at random would give them.

use std::path::PathBuf;

use crate::moon::Moon;
use crate::render::Stir;
use crate::sun::{Season, Sky};
use crate::units::UnixSeconds;
use console_core_atomic_writes::Stored;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_u64};
use console_location::Coordinates;
use console_weather::conditions::Weather;

pub const HOLD_SECONDS: f64 = 2.0 * 60.0 * 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Turn(pub u64);

impl Turn {
  pub fn at(seconds: impl Into<UnixSeconds>) -> Result<Turn, Never> {
    let Ok(turn) = toward_zero_u64(
      <_ as ::core::convert::Into<crate::units::UnixSeconds>>::into(seconds)
        .0
        .max(0.0)
        / HOLD_SECONDS,
    );

    Ok(Turn(turn))
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outside {
  pub sky: Sky,
  pub season: Season,
  pub moon: Moon,
  pub weather: Option<Weather>,
}

impl Outside {
  pub fn at(
    here: &Coordinates,
    seconds: impl Into<UnixSeconds>,
    weather: Option<Weather>,
  ) -> Result<Outside, Never> {
    let when = <_ as ::core::convert::Into<crate::units::UnixSeconds>>::into(seconds);
    let Ok(moon) = crate::moon::moon(when);
    let Ok(season) = crate::sun::season(here, when);
    let Ok(sky) = crate::sun::sky(here, when);

    Ok(Outside {
      sky,
      season,
      moon,
      weather,
    })
  }
}

#[derive(Debug, Clone)]

pub struct Picture {
  pub name: String,

  pub says: String,

  pub by: String,

  pub from: String,

  pub sha256: String,

  pub grade: Option<crate::grade::Grade>,

  pub sky: Vec<String>,

  pub weather: Vec<String>,

  pub season: Vec<String>,

  pub moon: Vec<String>,
}

pub(super) fn specificity(tags: [&[String]; 4], outside: &Outside) -> Result<Option<u32>, Never> {
  let Ok(sky) = outside.sky.word();
  let Ok(weather) = outside.weather.map(Weather::word).transpose();
  let Ok(season) = outside.season.word();
  let Ok(moon) = outside.moon.word();
  let mut score: u32 = 0;

  for (list, word) in tags
    .into_iter()
    .zip([Some(sky), weather, Some(season), Some(moon)])
  {
    match list {
      [] => {
        continue;
      }
      [_, ..] => {}
    }

    let matches = word.is_some_and(|tag| {
      list
        .iter()
        .any(|candidate| candidate.trim().eq_ignore_ascii_case(tag))
    });

    match bool::then_some(matches, ()) {
      None => return Ok(None),
      Some(()) => {}
    }

    score = score.saturating_add(1);
  }

  Ok(Some(score))
}

#[derive(Debug, Clone)]

pub struct Set {
  pub stir: Stir,

  pub pictures: Vec<Picture>,
}

pub(super) fn rotate<T: Copy>(
  matches: impl Iterator<Item = (T, u32)>,
  turn: Turn,
) -> Result<Option<T>, Never> {
  let mut standing = Vec::new();
  let mut best = 0;

  for (picture, score) in matches {
    match score.cmp(&best) {
      std::cmp::Ordering::Greater => {
        standing.clear();
        best = score;
        standing.push(picture);
      }
      std::cmp::Ordering::Equal => standing.push(picture),
      std::cmp::Ordering::Less => {}
    }
  }

  let Ok(count) = fitted::<_, u64>(standing.len());
  let round = match turn.0.checked_rem(count) {
    Some(round) => round,
    None => return Ok(None),
  };
  let Ok(at) = index(round);

  Ok(standing.get(at).copied())
}

pub fn choose<'a>(
  pictures: &'a [Picture],
  outside: &Outside,
  turn: Turn,
) -> Result<Option<&'a Picture>, Never> {
  choose_from(pictures.iter(), outside, turn)
}

pub(super) fn choose_from<'a>(
  pictures: impl Iterator<Item = &'a Picture>,
  outside: &Outside,
  turn: Turn,
) -> Result<Option<&'a Picture>, Never> {
  rotate(
    pictures.filter_map(|picture| {
      let tags = [
        ::core::convert::AsRef::<[_]>::as_ref(&picture.sky),
        &picture.weather,
        &picture.season,
        &picture.moon,
      ];
      let Ok(matched) = specificity(tags, outside);
      matched.map(|score| (picture, score))
    }),
    turn,
  )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
  pub selection: Selection,
  pub picture: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
  Following,
  Still,
  Live(String),
}

impl Default for Wanted {
  fn default() -> Self {
    Wanted {
      selection: Selection::Following,
      picture: String::new(),
    }
  }
}

impl Set {
  pub fn load() -> Result<Option<Self>, Never> {
    let Ok(at) = crate::place::table();
    let Ok(stored) = console_core_atomic_writes::read(&at);

    match stored {
      Stored::Text(held) => Self::read(&held),
      Stored::Absent => Ok(None),
      Stored::Failed(fault) => {
        console_core_logging::warn!("{}: {fault}", at.display());
        Ok(None)
      }
    }
  }

  pub fn read(held: &str) -> Result<Option<Set>, Never> {
    Ok(match toml::from_str(held) {
      Ok(set) => Some(set),
      Err(fault) => {
        console_core_logging::warn!("the picture table will not parse: {fault}");

        None
      }
    })
  }
}

impl Wanted {
  pub fn scene(&self) -> Result<Option<&str>, Never> {
    Ok(match &self.selection {
      Selection::Following | Selection::Still => None,
      Selection::Live(name) => Some(name.as_str()),
    })
  }

  pub fn read(held: &str) -> Result<Self, Never> {
    Ok(match toml::from_str(held) {
      Ok(wanted) => wanted,
      Err(fault) => {
        console_core_logging::warn!("what was asked of the wallpaper will not parse: {fault}");

        Wanted::default()
      }
    })
  }

  pub fn load() -> Result<Self, Never> {
    let at = crate::place::config_path()?;

    let at_3 = match at {
      Some(at_2) => at_2,
      None => return Ok(Wanted::default()),
    };

    let Ok(said) = console_core_atomic_writes::read(&at_3);

    match said {
      Stored::Text(held) => Wanted::read(&held),
      Stored::Absent => Ok(Wanted::default()),
      Stored::Failed(fault) => {
        console_core_logging::warn!("{}: {fault}", at_3.display());

        Ok(Wanted::default())
      }
    }
  }

  pub fn serialize(&self) -> Result<String, Never> {
    Ok(match toml::to_string(self) {
      Ok(written) => written,
      Err(fault) => {
        console_core_logging::warn!("writing down what was asked: {fault}");

        String::new()
      }
    })
  }
}

pub fn pinned<'a>(pictures: &'a [Picture], asked: &Wanted) -> Result<Option<&'a Picture>, Never> {
  Ok(match asked.selection {
    Selection::Following => None,
    Selection::Still | Selection::Live(_) => pictures
      .iter()
      .find(|picture| picture.name == asked.picture),
  })
}

pub fn still(outside: &Outside, turn: Turn) -> Result<Option<PathBuf>, Never> {
  let Ok(set) = Set::load();
  let pictures: &[Picture] = match &set {
    Some(table) => &table.pictures,
    None => &[],
  };
  let Ok(rules) = crate::rules::installed();
  let Ok(picked) = crate::rules::pick(&rules, pictures, outside, turn);
  let found = match picked {
    Some(name) => crate::place::picture(name)?,
    None => None,
  };

  Ok(found.map(|(moving, still)| {
    let Ok(status) = crate::place::file_status(&still);

    match status {
      crate::place::FileStatus::File => still,
      crate::place::FileStatus::Other => moving,
    }
  }))
}

pub fn pick<'a>(
  pictures: &'a [Picture],
  asked: &Wanted,
  outside: &Outside,
  turn: Turn,
) -> Result<Option<&'a Picture>, Never> {
  let pinned = pinned(pictures, asked)?;

  match pinned {
    Some(pinned_2) => return Ok(Some(pinned_2)),
    None => {}
  }

  let chosen = choose(pictures, outside, turn)?;

  Ok(chosen.or_else(|| pictures.first()))
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;

  struct Entry {
    name: &'static str,
    sky: &'static [&'static str],
    weather: &'static [&'static str],
    season: &'static [&'static str],
    moon: &'static [&'static str],
  }

  const ANYTHING: Entry = Entry {
    name: "",
    sky: &[],
    weather: &[],
    season: &[],
    moon: &[],
  };

  const TERRARIUM: Entry = Entry {
    name: "terrarium",
    ..ANYTHING
  };
  const STAR_RIDE: Entry = Entry {
    name: "star-ride",
    sky: &["night"],
    ..ANYTHING
  };
  const DANCING_FROGS: Entry = Entry {
    name: "dancing-frogs",
    sky: &["night"],
    ..ANYTHING
  };
  const COZY_WINTER: Entry = Entry {
    name: "cozy-winter",
    sky: &["night"],
    weather: &["snow"],
    ..ANYTHING
  };
  const LAZY_RIVER: Entry = Entry {
    name: "lazy-river",
    sky: &["day"],
    weather: &["clear"],
    ..ANYTHING
  };

  const SET: [Entry; 4] = [TERRARIUM, STAR_RIDE, COZY_WINTER, LAZY_RIVER];

  const WINTER: Outside = Outside {
    sky: Sky::Day,
    weather: None,
    season: Season::Winter,
    moon: Moon::New,
  };

  const FIRST: Turn = Turn(0);

  fn pictures(entries: &[Entry]) -> Result<Vec<Picture>, Never> {
    let words = |list: &[&str]| list.iter().map(|word| (*word).to_string()).collect();

    Ok(
      entries
        .iter()
        .map(|entry| Picture {
          name: entry.name.to_string(),
          says: entry.name.to_string(),
          by: String::new(),
          from: String::new(),
          sha256: String::new(),
          grade: None,
          sky: words(entry.sky),
          weather: words(entry.weather),
          season: words(entry.season),
          moon: words(entry.moon),
        })
        .collect(),
    )
  }

  fn chosen<'a>(
    pictures: &'a [Picture],
    outside: &Outside,
    turn: Turn,
  ) -> Result<Option<&'a str>, Never> {
    let Ok(chosen) = choose(pictures, outside, turn);

    Ok(chosen.map(|picture| picture.name.as_str()))
  }

  fn asked_for<'a>(
    pictures: &'a [Picture],
    asked: &Wanted,
    outside: &Outside,
    turn: Turn,
  ) -> Result<Option<&'a str>, Never> {
    let Ok(wanted) = pick(pictures, asked, outside, turn);

    Ok(wanted.map(|picture| picture.name.as_str()))
  }

  fn pin<'a>(pictures: &'a [Picture], asked: &Wanted) -> Result<Option<&'a str>, Never> {
    let Ok(pinned) = pinned(pictures, asked);

    Ok(pinned.map(|picture| picture.name.as_str()))
  }

  #[test]
  fn the_most_particular_picture_wins() {
    let Ok(set) = pictures(&SET);
    let snowing = Outside {
      sky: Sky::Night,
      weather: Some(Weather::Snow),
      ..WINTER
    };

    assert_eq!(chosen(&set, &snowing, FIRST), Ok(Some("cozy-winter")));
  }

  #[test]
  fn a_picture_for_a_part_of_the_day_beats_one_for_anything() {
    let Ok(set) = pictures(&SET);
    let raining = Outside {
      sky: Sky::Night,
      weather: Some(Weather::Rain),
      ..WINTER
    };

    assert_eq!(chosen(&set, &raining, FIRST), Ok(Some("star-ride")));
  }

  #[test]
  fn an_outside_nothing_answers_falls_to_the_picture_that_names_nothing() {
    let Ok(set) = pictures(&SET);
    let foggy = Outside {
      sky: Sky::Dusk,
      weather: Some(Weather::Fog),
      ..WINTER
    };

    assert_eq!(chosen(&set, &foggy, FIRST), Ok(Some("terrarium")));
  }

  #[test]
  fn a_picture_naming_a_weather_is_not_chosen_when_there_is_none_to_read() {
    let Ok(set) = pictures(&SET);
    let night = Outside {
      sky: Sky::Night,
      ..WINTER
    };

    assert_eq!(chosen(&set, &night, FIRST), Ok(Some("star-ride")));
  }

  #[test]
  fn a_picture_may_be_chosen_by_the_season_and_by_the_moon() {
    let Ok(set) = pictures(&[
      TERRARIUM,
      Entry {
        name: "first-snow",
        season: &["winter"],
        ..ANYTHING
      },
      Entry {
        name: "moonlit",
        moon: &["full"],
        ..ANYTHING
      },
    ]);
    let snowy = Outside {
      moon: Moon::Waning,
      ..WINTER
    };
    let moonlit = Outside {
      moon: Moon::Full,
      season: Season::Summer,
      ..snowy
    };

    assert_eq!(chosen(&set, &snowy, FIRST), Ok(Some("first-snow")));
    assert_eq!(chosen(&set, &moonlit, FIRST), Ok(Some("moonlit")));
  }

  #[test]
  fn a_sunset_and_the_dusk_after_it_are_different_outsides() {
    let Ok(set) = pictures(&[
      TERRARIUM,
      Entry {
        name: "golden",
        sky: &["sunrise", "sunset"],
        ..ANYTHING
      },
    ]);
    let sunset = Outside {
      sky: Sky::Sunset,
      ..WINTER
    };
    let dusk = Outside {
      sky: Sky::Dusk,
      ..WINTER
    };

    assert_eq!(chosen(&set, &sunset, FIRST), Ok(Some("golden")));
    assert_eq!(chosen(&set, &dusk, FIRST), Ok(Some("terrarium")));
  }

  #[test]
  fn a_set_holding_nothing_chooses_nothing() {
    let clear = Outside {
      weather: Some(Weather::Clear),
      ..WINTER
    };

    assert_eq!(chosen(&[], &clear, FIRST), Ok(None));
  }

  #[test]
  fn pictures_of_the_same_standing_take_turns() {
    let Ok(set) = pictures(&[TERRARIUM, STAR_RIDE, DANCING_FROGS]);
    let night = Outside {
      sky: Sky::Night,
      weather: Some(Weather::Rain),
      ..WINTER
    };
    let name = |turn: u64| chosen(&set, &night, Turn(turn));

    assert_eq!(name(0), Ok(Some("star-ride")));
    assert_eq!(name(1), Ok(Some("dancing-frogs")));
    assert_eq!(name(2), Ok(Some("star-ride")));
    assert_eq!(name(3), Ok(Some("dancing-frogs")));
  }

  #[test]
  fn a_more_particular_picture_does_not_take_turns_with_a_less_particular_one() {
    let Ok(set) = pictures(&[STAR_RIDE, DANCING_FROGS, COZY_WINTER]);
    let snowing = Outside {
      sky: Sky::Night,
      weather: Some(Weather::Snow),
      ..WINTER
    };

    for turn in 0..6 {
      assert_eq!(
        chosen(&set, &snowing, Turn(turn)),
        Ok(Some("cozy-winter")),
        "turn {turn}"
      );
    }
  }

  #[test]
  fn a_set_where_nothing_ties_says_the_same_thing_all_day() {
    let Ok(set) = pictures(&SET);
    let night = Outside {
      sky: Sky::Night,
      weather: Some(Weather::Snow),
      ..WINTER
    };

    for turn in 0..12 {
      assert_eq!(chosen(&set, &night, Turn(turn)), Ok(Some("cozy-winter")));
    }
  }

  #[test]
  fn a_turn_is_two_hours_of_the_clock() {
    let hour = 60.0 * 60.0;

    assert_eq!(Turn::at(0.0), Ok(Turn(0)));
    assert_eq!(Turn::at(hour), Ok(Turn(0)));
    assert_eq!(Turn::at(2.0 * hour), Ok(Turn(1)));
    assert_eq!(Turn::at(2.0 * hour - 1.0), Ok(Turn(0)));
    assert_eq!(Turn::at(24.0 * hour), Ok(Turn(12)));
  }

  #[test]
  fn a_pinned_picture_is_the_one_that_is_up() {
    let Ok(set) = pictures(&SET);
    let asked = Wanted {
      selection: Selection::Still,
      picture: "lazy-river".to_string(),
    };
    let snowing = Outside {
      sky: Sky::Night,
      weather: Some(Weather::Snow),
      ..WINTER
    };

    assert_eq!(
      asked_for(&set, &asked, &snowing, FIRST),
      Ok(Some("lazy-river"))
    );
  }

  #[test]
  fn a_pinned_picture_that_is_gone_goes_back_to_following_the_weather() {
    let Ok(set) = pictures(&SET);
    let asked = Wanted {
      selection: Selection::Still,
      picture: "sledding".to_string(),
    };
    let snowing = Outside {
      sky: Sky::Night,
      weather: Some(Weather::Snow),
      ..WINTER
    };

    assert_eq!(
      asked_for(&set, &asked, &snowing, FIRST),
      Ok(Some("cozy-winter"))
    );
  }

  #[test]
  fn a_pinned_picture_is_known_without_anything_being_asked_of_the_weather() {
    let Ok(set) = pictures(&SET);
    let pinned_on = Wanted {
      selection: Selection::Still,
      picture: "lazy-river".to_string(),
    };
    let following = Wanted {
      selection: Selection::Following,
      picture: "lazy-river".to_string(),
    };
    let gone = Wanted {
      selection: Selection::Still,
      picture: "sledding".to_string(),
    };

    assert_eq!(pin(&set, &pinned_on), Ok(Some("lazy-river")));
    assert_eq!(pin(&set, &following), Ok(None));
    assert_eq!(pin(&set, &gone), Ok(None));
  }

  #[test]
  fn a_pinned_picture_does_not_take_turns_with_anything() {
    let Ok(set) = pictures(&[STAR_RIDE, DANCING_FROGS]);
    let asked = Wanted {
      selection: Selection::Still,
      picture: "star-ride".to_string(),
    };
    let night = Outside {
      sky: Sky::Night,
      ..WINTER
    };

    for turn in 0..6 {
      assert_eq!(
        asked_for(&set, &asked, &night, Turn(turn)),
        Ok(Some("star-ride")),
        "turn {turn}"
      );
    }
  }

  #[test]
  fn what_was_asked_for_is_written_and_read_back_the_same() {
    let asked = Wanted {
      selection: Selection::Still,
      picture: "star-ride".to_string(),
    };
    let Ok(written) = asked.serialize();

    assert_eq!(Wanted::read(&written), Ok(asked));
  }

  fn shipped() -> Result<Set, Box<dyn Error>> {
    let Ok(at) = crate::place::table();
    let held = std::fs::read_to_string(&at)?;
    let Ok(set) = Set::read(&held);

    set.ok_or_else(|| Box::from(format!("{} is not a table this can read", at.display())))
  }

  #[test]
  fn no_two_pictures_in_the_shipped_table_are_called_the_same_thing() -> Result<(), Box<dyn Error>>
  {
    let set = shipped()?;
    let mut seen = std::collections::BTreeSet::new();

    for picture in &set.pictures {
      assert!(
        seen.insert(picture.name.clone()),
        "two pictures called {}",
        picture.name
      );
    }

    Ok(())
  }

  #[test]
  fn the_shipped_table_shows_more_than_one_picture_over_a_clear_day() -> Result<(), Box<dyn Error>>
  {
    let set = shipped()?;
    let clear = Outside {
      sky: Sky::Day,
      weather: Some(Weather::Clear),
      season: Season::Summer,
      moon: Moon::New,
    };
    let over_a_day: Vec<Option<&str>> = (0..12)
      .map(|turn| {
        let Ok(chosen) = chosen(&set.pictures, &clear, Turn(turn));

        chosen
      })
      .collect();
    let how_many: std::collections::BTreeSet<Option<&str>> = over_a_day.iter().copied().collect();

    assert!(
      how_many.len() > 1,
      "a whole clear day showed only {over_a_day:?}"
    );

    Ok(())
  }

  #[test]
  fn a_file_that_is_not_one_reads_as_following_the_weather() {
    assert_eq!(Wanted::read("follow = maybe"), Ok(Wanted::default()));
    assert!(matches!(
      Wanted::read(""),
      Ok(Wanted {
        selection: Selection::Following,
        ..
      })
    ));
  }
}
