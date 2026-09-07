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
//! nobody has covered yet can be added without a line of any other picture
//! changing.
//!
//! Pictures that are equally particular take turns. The turn is the clock cut
//! into two hour lengths, so one of them holds for a couple of hours and the
//! next of the same standing takes it from there, and the set of them comes
//! round again by the end of the day. That is what makes a set worth growing
//! sideways as well as downwards: a second picture for a clear summer day is
//! half of the clear summer days rather than a picture nobody ever sees.
//!
//! Which of them goes first is the order they are written down in. It is
//! arbitrary, and it is arbitrary in a way somebody can see and reorder, which
//! is more than picking at random would give them.


use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_u64};
use crate::moon::Moon;
use crate::press::Stir;
use crate::sun::{Season, Sky};
use crate::weather::Weather;

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

    fn particular(&self, outside: &Outside) -> Result<usize, Never> {
        let against = self.against(outside)?;

        Ok(against.iter().filter(|(list, _)| !list.is_empty()).count())
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Set {
    #[serde(default)]
    pub stir: Stir,
    #[serde(default, rename = "picture")]
    pub pictures: Vec<Picture>,
}

fn standing<'a>(pictures: &'a [Picture], outside: &Outside) -> Result<Vec<&'a Picture>, Never> {
    let mut answering: Vec<(&Picture, usize)> = Vec::new();

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

    let Some(best) = answering.iter().map(|(_, particular)| *particular).max() else {
        return Ok(Vec::new());
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
    let standing = standing(pictures, outside)?;
    let Ok(count) = fitted::<usize, u64>(standing.len().max(1));
    let Ok(at) = fitted::<u64, usize>(turn.0.checked_rem(count).unwrap_or(0));

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
                eprintln!("console-sky: the picture table will not parse: {fault}");

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
                eprintln!("console-sky: what was asked of the wallpaper will not parse: {fault}");

                Wanted::default()
            }
        })
    }

    pub fn asked() -> Result<Self, Never> {
        let Some(at) = crate::place::asked()? else { return Ok(Wanted::default()) };

        match std::fs::read_to_string(&at) {
            Ok(held) => Wanted::read(&held),
            Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Ok(Wanted::default()),
            Err(fault) => {
                eprintln!("console-sky: {}: {fault}", at.display());

                Ok(Wanted::default())
            }
        }
    }

    pub fn written(&self) -> Result<String, Never> {
        Ok(match toml::to_string(self) {
            Ok(written) => written,
            Err(fault) => {
                eprintln!("console-sky: writing down what was asked: {fault}");

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

pub fn wanted<'a>(
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

    fn chosen<'a>(pictures: &'a [Picture], outside: &Outside, turn: Turn) -> Option<&'a Picture> {
        let Ok(chosen) = choose(pictures, outside, turn);

        chosen
    }

    fn pin<'a>(pictures: &'a [Picture], asked: &Wanted) -> Option<&'a Picture> {
        let Ok(pinned) = pinned(pictures, asked);

        pinned
    }

    fn asked_for<'a>(
        pictures: &'a [Picture],
        asked: &Wanted,
        outside: &Outside,
        turn: Turn,
    ) -> Option<&'a Picture> {
        let Ok(wanted) = wanted(pictures, asked, outside, turn);

        wanted
    }

    fn read(held: &str) -> Wanted {
        let Ok(wanted) = Wanted::read(held);

        wanted
    }

    fn written(asked: &Wanted) -> String {
        let Ok(written) = asked.written();

        written
    }

    fn turn_at(seconds: f64) -> Turn {
        let Ok(turn) = Turn::at(seconds);

        turn
    }

    fn picture(name: &str, sky: &[&str], weather: &[&str]) -> Picture {
        let words = |list: &[&str]| list.iter().map(|word| (*word).to_string()).collect();
        Picture {
            name: name.to_string(),
            says: name.to_string(),
            by: String::new(),
            from: String::new(),
            sha256: String::new(),
            grade: None,
            sky: words(sky),
            weather: words(weather),
            season: Vec::new(),
            moon: Vec::new(),
        }
    }

    fn set() -> Vec<Picture> {
        vec![
            picture("terrarium", &[], &[]),
            picture("star-ride", &["night"], &[]),
            picture("cozy-winter", &["night"], &["snow"]),
            picture("lazy-river", &["day"], &["clear"]),
        ]
    }

    fn outside(sky: Sky, weather: Option<Weather>) -> Outside {
        Outside { sky, weather, season: Season::Winter, moon: Moon::New }
    }

    const FIRST: Turn = Turn(0);

    #[test]
    fn the_most_particular_picture_wins() {
        let set = set();
        let chosen =
            chosen(&set, &outside(Sky::Night, Some(Weather::Snow)), FIRST).expect("a picture");
        assert_eq!(chosen.name, "cozy-winter");
    }

    #[test]
    fn a_picture_for_a_part_of_the_day_beats_one_for_anything() {
        let set = set();
        let chosen =
            chosen(&set, &outside(Sky::Night, Some(Weather::Rain)), FIRST).expect("a picture");
        assert_eq!(chosen.name, "star-ride");
    }

    #[test]
    fn an_outside_nothing_answers_falls_to_the_picture_that_names_nothing() {
        let set = set();
        let chosen =
            chosen(&set, &outside(Sky::Dusk, Some(Weather::Fog)), FIRST).expect("a picture");
        assert_eq!(chosen.name, "terrarium");
    }

    #[test]
    fn a_picture_naming_a_weather_is_not_chosen_when_there_is_none_to_read() {
        let set = set();
        let chosen = chosen(&set, &outside(Sky::Night, None), FIRST).expect("a picture");
        assert_eq!(chosen.name, "star-ride");
    }

    #[test]
    fn a_picture_may_be_chosen_by_the_season_and_by_the_moon() {
        let mut winter = picture("first-snow", &[], &[]);
        winter.season = vec!["winter".to_string()];
        let mut full = picture("moonlit", &[], &[]);
        full.moon = vec!["full".to_string()];
        let set = vec![picture("terrarium", &[], &[]), winter, full];

        let snowy = Outside {
            sky: Sky::Day,
            weather: None,
            season: Season::Winter,
            moon: Moon::Waning,
        };
        assert_eq!(chosen(&set, &snowy, FIRST).expect("a picture").name, "first-snow");

        let moonlit = Outside { moon: Moon::Full, season: Season::Summer, ..snowy };
        assert_eq!(chosen(&set, &moonlit, FIRST).expect("a picture").name, "moonlit");
    }

    #[test]
    fn a_sunset_and_the_dusk_after_it_are_different_outsides() {
        let mut golden = picture("golden", &["sunrise", "sunset"], &[]);
        golden.by = "nobody".to_string();
        let set = vec![picture("terrarium", &[], &[]), golden];
        assert_eq!(
            chosen(&set, &outside(Sky::Sunset, None), FIRST).expect("a picture").name,
            "golden"
        );
        assert_eq!(
            chosen(&set, &outside(Sky::Dusk, None), FIRST).expect("a picture").name,
            "terrarium"
        );
    }

    #[test]
    fn a_set_holding_nothing_chooses_nothing() {
        assert!(chosen(&[], &outside(Sky::Day, Some(Weather::Clear)), FIRST).is_none());
    }

    #[test]
    fn pictures_of_the_same_standing_take_turns() {
        let set = vec![
            picture("terrarium", &[], &[]),
            picture("star-ride", &["night"], &[]),
            picture("dancing-frogs", &["night"], &[]),
        ];
        let night = outside(Sky::Night, Some(Weather::Rain));
        let name = |turn: u64| chosen(&set, &night, Turn(turn)).expect("a picture").name.clone();
        assert_eq!(name(0), "star-ride");
        assert_eq!(name(1), "dancing-frogs");
        assert_eq!(name(2), "star-ride");
        assert_eq!(name(3), "dancing-frogs");
    }

    #[test]
    fn a_more_particular_picture_does_not_take_turns_with_a_less_particular_one() {
        let set = vec![
            picture("star-ride", &["night"], &[]),
            picture("dancing-frogs", &["night"], &[]),
            picture("cozy-winter", &["night"], &["snow"]),
        ];
        let snowing = outside(Sky::Night, Some(Weather::Snow));
        for turn in 0..6 {
            let chosen = chosen(&set, &snowing, Turn(turn)).expect("a picture");
            assert_eq!(chosen.name, "cozy-winter", "turn {turn}");
        }
    }

    #[test]
    fn a_set_where_nothing_ties_says_the_same_thing_all_day() {
        let set = set();
        let night = outside(Sky::Night, Some(Weather::Snow));
        for turn in 0..12 {
            assert_eq!(chosen(&set, &night, Turn(turn)).expect("a picture").name, "cozy-winter");
        }
    }

    #[test]
    fn a_turn_is_two_hours_of_the_clock() {
        let hour = 60.0 * 60.0;
        assert_eq!(turn_at(0.0), Turn(0));
        assert_eq!(turn_at(hour), Turn(0));
        assert_eq!(turn_at(2.0 * hour), Turn(1));
        assert_eq!(turn_at(2.0 * hour - 1.0), Turn(0));
        assert_eq!(turn_at(24.0 * hour), Turn(12));
    }

    #[test]
    fn a_pinned_picture_is_the_one_that_is_up() {
        let set = set();
        let asked = Wanted { follow: false, picture: "lazy-river".to_string() };
        let chosen = asked_for(&set, &asked, &outside(Sky::Night, Some(Weather::Snow)), FIRST)
            .expect("a picture");
        assert_eq!(chosen.name, "lazy-river");
    }

    #[test]
    fn a_pinned_picture_that_is_gone_goes_back_to_following_the_weather() {
        let set = set();
        let asked = Wanted { follow: false, picture: "sledding".to_string() };
        let chosen = asked_for(&set, &asked, &outside(Sky::Night, Some(Weather::Snow)), FIRST)
            .expect("a picture");
        assert_eq!(chosen.name, "cozy-winter");
    }

    #[test]
    fn a_pinned_picture_is_known_without_anything_being_asked_of_the_weather() {
        let set = set();
        let pinned_on = Wanted { follow: false, picture: "lazy-river".to_string() };
        assert_eq!(pin(&set, &pinned_on).expect("a picture").name, "lazy-river");

        let following = Wanted { follow: true, picture: "lazy-river".to_string() };
        assert!(pin(&set, &following).is_none());
        let gone = Wanted { follow: false, picture: "sledding".to_string() };
        assert!(pin(&set, &gone).is_none());
    }

    #[test]
    fn a_pinned_picture_does_not_take_turns_with_anything() {
        let set = vec![
            picture("star-ride", &["night"], &[]),
            picture("dancing-frogs", &["night"], &[]),
        ];
        let asked = Wanted { follow: false, picture: "star-ride".to_string() };
        let night = outside(Sky::Night, None);
        for turn in 0..6 {
            let chosen = asked_for(&set, &asked, &night, Turn(turn)).expect("a picture");
            assert_eq!(chosen.name, "star-ride", "turn {turn}");
        }
    }

    #[test]
    fn what_was_asked_for_is_written_and_read_back_the_same() {
        let asked = Wanted { follow: false, picture: "star-ride".to_string() };
        assert_eq!(read(&written(&asked)), asked);
    }

    fn shipped() -> Set {
        let Ok(at) = crate::place::table();

        let held = std::fs::read_to_string(&at)
            .unwrap_or_else(|fault| panic!("{} could not be read: {fault}", at.display()));

        let Ok(set) = Set::read(&held);

        set.unwrap_or_else(|| panic!("{} is not a table this can read", at.display()))
    }

    #[test]
    fn no_two_pictures_in_the_shipped_table_are_called_the_same_thing() {
        let set = shipped();
        let mut seen = std::collections::BTreeSet::new();
        for picture in &set.pictures {
            assert!(seen.insert(picture.name.clone()), "two pictures called {}", picture.name);
        }
    }

    #[test]
    fn the_shipped_table_shows_more_than_one_picture_over_a_clear_day() {
        let set = shipped();
        let clear = Outside {
            sky: Sky::Day,
            weather: Some(Weather::Clear),
            season: Season::Summer,
            moon: Moon::New,
        };
        let over_a_day: Vec<&str> = (0..12)
            .map(|turn| {
                chosen(&set.pictures, &clear, Turn(turn)).expect("a picture").name.as_str()
            })
            .collect();
        let how_many: std::collections::BTreeSet<&str> = over_a_day.iter().copied().collect();
        assert!(how_many.len() > 1, "a whole clear day showed only {over_a_day:?}");
    }

    #[test]
    fn a_file_that_is_not_one_reads_as_following_the_weather() {
        assert_eq!(read("follow = maybe"), Wanted::default());
        assert!(read("").follow);
    }
}
