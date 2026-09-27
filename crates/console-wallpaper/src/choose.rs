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

use console_core_atomic_writes::Stored;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_u64};
use crate::moon::Moon;
use crate::render::Stir;
use crate::sun::{Season, Sky};
use console_weather::conditions::Weather;
use console_weather::here::Where;

const NONE_OF_THEM: u64 = 0;


pub const HOLD_SECONDS: f64 = 2.0 * 60.0 * 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Turn(pub u64);

impl Turn {
    pub fn at(seconds: f64) -> Result<Turn, Never> {
        let Ok(turn) = toward_zero_u64(seconds.max(0.0) / HOLD_SECONDS);

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
    pub fn at(here: &Where, seconds: f64, weather: Option<Weather>) -> Result<Outside, Never> {
        let Ok(moon) = crate::moon::moon(seconds);
        let Ok(season) = crate::sun::season(here, seconds);
        let Ok(sky) = crate::sun::sky(here, seconds);

        Ok(Outside { sky, season, moon, weather })
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Picture {
    pub name: String,
    pub says: String,
    #[serde(default)]
    pub by: String,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub grade: Option<crate::grade::Grade>,
    #[serde(default)]
    pub sky: Vec<String>,
    #[serde(default)]
    pub weather: Vec<String>,
    #[serde(default)]
    pub season: Vec<String>,
    #[serde(default)]
    pub moon: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answers {
    Yes,
    No,
}

fn names(list: &[String], word: Option<&str>) -> Result<Answers, Never> {
    match list.is_empty() {
        true => return Ok(Answers::Yes),
        false => {},
    }

    Ok(
        match word.is_some_and(|word| list.iter().any(|held| held.trim().to_lowercase() == word)) {
            true => Answers::Yes,
            false => Answers::No,
        },
    )
}

type Against<'a> = [(&'a Vec<String>, Option<&'static str>); 4];

impl Picture {
    fn against(&self, outside: &Outside) -> Result<Against<'_>, Never> {
        let moon = outside.moon.word()?;

        let season = outside.season.word()?;

        let sky = outside.sky.word()?;

        let weather = match outside.weather {
            Some(weather) => {
                let word = weather.word()?;

                Some(word)
            }
            None => None,
        };

        Ok([
            (&self.moon, Some(moon)),
            (&self.season, Some(season)),
            (&self.sky, Some(sky)),
            (&self.weather, weather),
        ])
    }

    fn answers(&self, outside: &Outside) -> Result<Answers, Never> {
        let against = self.against(outside)?;

        for (list, word) in against {
            let names = names(list, word)?;

            match names {
                Answers::Yes => {},
                Answers::No => return Ok(Answers::No),
            }
        }

        Ok(Answers::Yes)
    }

    fn particular(&self, outside: &Outside) -> Result<u32, Never> {
        let against = self.against(outside)?;

        fitted(against.iter().filter(|(list, _)| !list.is_empty()).count())
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Set {
    #[serde(default)]
    pub stir: Stir,
    #[serde(default, rename = "picture")]
    pub pictures: Vec<Picture>,
}

fn suitable<'a>(pictures: &'a [Picture], outside: &Outside) -> Result<Vec<&'a Picture>, Never> {
    let mut answering: Vec<(&Picture, u32)> = Vec::new();

    for picture in pictures {
        let answers = picture.answers(outside)?;

        match answers {
            Answers::Yes => {
                let particular = picture.particular(outside)?;

                answering.push((picture, particular));
            }
            Answers::No => {},
        }
    }

    let best = match answering.iter().map(|(_, particular)| *particular).max() {
        Some(best) => best,
        None => return Ok(Vec::new()),
    };

    Ok(answering
        .into_iter()
        .filter(|(_, particular)| *particular == best)
        .map(|(picture, _)| picture)
        .collect())
}

pub fn choose<'a>(
    pictures: &'a [Picture],
    outside: &Outside,
    turn: Turn,
) -> Result<Option<&'a Picture>, Never> {
    let standing = suitable(pictures, outside)?;
    let Ok(count) = fitted::<_, u64>(standing.len().max(1));
    let round = match turn.0.checked_rem(count) {
        Some(round) => round,
        None => NONE_OF_THEM,
    };

    let Ok(at) = index(round);

    Ok(standing.get(at).copied())
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Wanted {
    pub follow: bool,
    pub picture: String,
}

impl Default for Wanted {
    fn default() -> Self {
        Wanted { follow: true, picture: String::new() }
    }
}

impl Set {
    pub fn read(held: &str) -> Result<Option<Set>, Never> {
        Ok(match toml::from_str(held) {
            Ok(set) => Some(set),
            Err(fault) => {
                eprintln!("console-wallpaper: the picture table will not parse: {fault}");

                None
            }
        })
    }
}

impl Wanted {
    pub fn read(held: &str) -> Result<Self, Never> {
        Ok(match toml::from_str(held) {
            Ok(wanted) => wanted,
            Err(fault) => {
                eprintln!("console-wallpaper: what was asked of the wallpaper will not parse: {fault}");

                Wanted::default()
            }
        })
    }

    pub fn load() -> Result<Self, Never> {
        let at = crate::place::config_path()?;

        let at = match at {
            Some(at) => at,
            None => return Ok(Wanted::default()),
        };

        let Ok(said) = console_core_atomic_writes::read(&at);

        match said {
            Stored::Text(held) => Wanted::read(&held),
            Stored::Absent => Ok(Wanted::default()),
            Stored::Failed(fault) => {
                eprintln!("console-wallpaper: {}: {fault}", at.display());

                Ok(Wanted::default())
            }
        }
    }

    pub fn serialize(&self) -> Result<String, Never> {
        Ok(match toml::to_string(self) {
            Ok(written) => written,
            Err(fault) => {
                eprintln!("console-wallpaper: writing down what was asked: {fault}");

                String::new()
            }
        })
    }
}

pub fn pinned<'a>(pictures: &'a [Picture], asked: &Wanted) -> Result<Option<&'a Picture>, Never> {
    Ok(match asked.follow {
        true => None,
        false => pictures.iter().find(|picture| picture.name == asked.picture),
    })
}

pub fn still(outside: &Outside, turn: Turn) -> Result<Option<PathBuf>, Never> {
    let Ok(table) = crate::place::table();
    let Ok(held) = console_core_atomic_writes::read(&table);

    let set = match held {
        Stored::Text(held) => Set::read(&held)?,
        Stored::Absent => None,
        Stored::Failed(fault) => {
            eprintln!("console-wallpaper: {}: {fault}", table.display());

            None
        }
    };

    let set = match set {
        Some(set) => set,
        None => return Ok(None),
    };

    let chosen = choose(&set.pictures, outside, turn)?;

    let found = match chosen.or_else(|| set.pictures.first()) {
        Some(picture) => crate::place::picture(&picture.name)?,
        None => None,
    };

    Ok(found.map(|(moving, still)| match still.is_file() {
        true => still,
        false => moving,
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
        Some(pinned) => return Ok(Some(pinned)),
        None => {},
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

    const ANYTHING: Entry = Entry { name: "", sky: &[], weather: &[], season: &[], moon: &[] };

    const TERRARIUM: Entry = Entry { name: "terrarium", ..ANYTHING };
    const STAR_RIDE: Entry = Entry { name: "star-ride", sky: &["night"], ..ANYTHING };
    const DANCING_FROGS: Entry = Entry { name: "dancing-frogs", sky: &["night"], ..ANYTHING };
    const COZY_WINTER: Entry = Entry { name: "cozy-winter", sky: &["night"], weather: &["snow"], ..ANYTHING };
    const LAZY_RIVER: Entry = Entry { name: "lazy-river", sky: &["day"], weather: &["clear"], ..ANYTHING };

    const SET: [Entry; 4] = [TERRARIUM, STAR_RIDE, COZY_WINTER, LAZY_RIVER];

    const WINTER: Outside = Outside { sky: Sky::Day, weather: None, season: Season::Winter, moon: Moon::New };

    const FIRST: Turn = Turn(0);

    fn pictures(entries: &[Entry]) -> Result<Vec<Picture>, Never> {
        let words = |list: &[&str]| list.iter().map(|word| (*word).to_string()).collect();

        Ok(entries
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
            .collect())
    }

    fn chosen<'a>(pictures: &'a [Picture], outside: &Outside, turn: Turn) -> Result<Option<&'a str>, Never> {
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
        let snowing = Outside { sky: Sky::Night, weather: Some(Weather::Snow), ..WINTER };

        assert_eq!(chosen(&set, &snowing, FIRST), Ok(Some("cozy-winter")));
    }

    #[test]
    fn a_picture_for_a_part_of_the_day_beats_one_for_anything() {
        let Ok(set) = pictures(&SET);
        let raining = Outside { sky: Sky::Night, weather: Some(Weather::Rain), ..WINTER };

        assert_eq!(chosen(&set, &raining, FIRST), Ok(Some("star-ride")));
    }

    #[test]
    fn an_outside_nothing_answers_falls_to_the_picture_that_names_nothing() {
        let Ok(set) = pictures(&SET);
        let foggy = Outside { sky: Sky::Dusk, weather: Some(Weather::Fog), ..WINTER };

        assert_eq!(chosen(&set, &foggy, FIRST), Ok(Some("terrarium")));
    }

    #[test]
    fn a_picture_naming_a_weather_is_not_chosen_when_there_is_none_to_read() {
        let Ok(set) = pictures(&SET);
        let night = Outside { sky: Sky::Night, ..WINTER };

        assert_eq!(chosen(&set, &night, FIRST), Ok(Some("star-ride")));
    }

    #[test]
    fn a_picture_may_be_chosen_by_the_season_and_by_the_moon() {
        let Ok(set) = pictures(&[
            TERRARIUM,
            Entry { name: "first-snow", season: &["winter"], ..ANYTHING },
            Entry { name: "moonlit", moon: &["full"], ..ANYTHING },
        ]);
        let snowy = Outside { moon: Moon::Waning, ..WINTER };
        let moonlit = Outside { moon: Moon::Full, season: Season::Summer, ..snowy };

        assert_eq!(chosen(&set, &snowy, FIRST), Ok(Some("first-snow")));
        assert_eq!(chosen(&set, &moonlit, FIRST), Ok(Some("moonlit")));
    }

    #[test]
    fn a_sunset_and_the_dusk_after_it_are_different_outsides() {
        let Ok(set) = pictures(&[TERRARIUM, Entry { name: "golden", sky: &["sunrise", "sunset"], ..ANYTHING }]);
        let sunset = Outside { sky: Sky::Sunset, ..WINTER };
        let dusk = Outside { sky: Sky::Dusk, ..WINTER };

        assert_eq!(chosen(&set, &sunset, FIRST), Ok(Some("golden")));
        assert_eq!(chosen(&set, &dusk, FIRST), Ok(Some("terrarium")));
    }

    #[test]
    fn a_set_holding_nothing_chooses_nothing() {
        let clear = Outside { weather: Some(Weather::Clear), ..WINTER };

        assert_eq!(chosen(&[], &clear, FIRST), Ok(None));
    }

    #[test]
    fn pictures_of_the_same_standing_take_turns() {
        let Ok(set) = pictures(&[TERRARIUM, STAR_RIDE, DANCING_FROGS]);
        let night = Outside { sky: Sky::Night, weather: Some(Weather::Rain), ..WINTER };
        let name = |turn: u64| chosen(&set, &night, Turn(turn));

        assert_eq!(name(0), Ok(Some("star-ride")));
        assert_eq!(name(1), Ok(Some("dancing-frogs")));
        assert_eq!(name(2), Ok(Some("star-ride")));
        assert_eq!(name(3), Ok(Some("dancing-frogs")));
    }

    #[test]
    fn a_more_particular_picture_does_not_take_turns_with_a_less_particular_one() {
        let Ok(set) = pictures(&[STAR_RIDE, DANCING_FROGS, COZY_WINTER]);
        let snowing = Outside { sky: Sky::Night, weather: Some(Weather::Snow), ..WINTER };

        for turn in 0..6 {
            assert_eq!(chosen(&set, &snowing, Turn(turn)), Ok(Some("cozy-winter")), "turn {turn}");
        }
    }

    #[test]
    fn a_set_where_nothing_ties_says_the_same_thing_all_day() {
        let Ok(set) = pictures(&SET);
        let night = Outside { sky: Sky::Night, weather: Some(Weather::Snow), ..WINTER };

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
        let asked = Wanted { follow: false, picture: "lazy-river".to_string() };
        let snowing = Outside { sky: Sky::Night, weather: Some(Weather::Snow), ..WINTER };

        assert_eq!(asked_for(&set, &asked, &snowing, FIRST), Ok(Some("lazy-river")));
    }

    #[test]
    fn a_pinned_picture_that_is_gone_goes_back_to_following_the_weather() {
        let Ok(set) = pictures(&SET);
        let asked = Wanted { follow: false, picture: "sledding".to_string() };
        let snowing = Outside { sky: Sky::Night, weather: Some(Weather::Snow), ..WINTER };

        assert_eq!(asked_for(&set, &asked, &snowing, FIRST), Ok(Some("cozy-winter")));
    }

    #[test]
    fn a_pinned_picture_is_known_without_anything_being_asked_of_the_weather() {
        let Ok(set) = pictures(&SET);
        let pinned_on = Wanted { follow: false, picture: "lazy-river".to_string() };
        let following = Wanted { follow: true, picture: "lazy-river".to_string() };
        let gone = Wanted { follow: false, picture: "sledding".to_string() };

        assert_eq!(pin(&set, &pinned_on), Ok(Some("lazy-river")));
        assert_eq!(pin(&set, &following), Ok(None));
        assert_eq!(pin(&set, &gone), Ok(None));
    }

    #[test]
    fn a_pinned_picture_does_not_take_turns_with_anything() {
        let Ok(set) = pictures(&[STAR_RIDE, DANCING_FROGS]);
        let asked = Wanted { follow: false, picture: "star-ride".to_string() };
        let night = Outside { sky: Sky::Night, ..WINTER };

        for turn in 0..6 {
            assert_eq!(asked_for(&set, &asked, &night, Turn(turn)), Ok(Some("star-ride")), "turn {turn}");
        }
    }

    #[test]
    fn what_was_asked_for_is_written_and_read_back_the_same() {
        let asked = Wanted { follow: false, picture: "star-ride".to_string() };
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
    fn no_two_pictures_in_the_shipped_table_are_called_the_same_thing() -> Result<(), Box<dyn Error>> {
        let set = shipped()?;
        let mut seen = std::collections::BTreeSet::new();

        for picture in &set.pictures {
            assert!(seen.insert(picture.name.clone()), "two pictures called {}", picture.name);
        }

        Ok(())
    }

    #[test]
    fn the_shipped_table_shows_more_than_one_picture_over_a_clear_day() -> Result<(), Box<dyn Error>> {
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

        assert!(how_many.len() > 1, "a whole clear day showed only {over_a_day:?}");

        Ok(())
    }

    #[test]
    fn a_file_that_is_not_one_reads_as_following_the_weather() {
        assert_eq!(Wanted::read("follow = maybe"), Ok(Wanted::default()));
        assert!(matches!(Wanted::read(""), Ok(Wanted { follow: true, .. })));
    }
}
