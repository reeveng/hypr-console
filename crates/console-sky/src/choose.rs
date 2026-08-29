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
//! Ties go to the first one written down. It is arbitrary, and it is arbitrary
//! in a way somebody can see and reorder, which is more than picking at random
//! would give them.

use crate::moon::Moon;
use crate::press::Stir;
use crate::sun::{Season, Sky};
use crate::weather::Weather;

/// What it is like outside, as far as a picture is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outside {
    pub sky: Sky,
    pub season: Season,
    pub moon: Moon,
    /// What the weather is, when it could be found out. Everything else here
    /// is arithmetic and cannot fail; this is the one thing that needs a
    /// network, so it is the one thing that can be missing.
    pub weather: Option<Weather>,
}

/// One picture: where it came from, how it is graded, and what it answers.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Picture {
    /// What it is called on the disk, without an extension.
    pub name: String,
    /// What it is called in the settings.
    pub says: String,
    /// Who drew it. Every picture here is somebody's work and the settings
    /// says so beside it.
    #[serde(default)]
    pub by: String,
    /// Where the source is fetched from, when it is not one of hers.
    #[serde(default)]
    pub from: String,
    /// What the source was when it was written down here, so a picture that
    /// changes upstream is noticed rather than quietly swapped.
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub grade: Option<crate::grade::Grade>,
    /// Which parts of the day it answers. Empty answers all of them, and the
    /// same goes for each of the three below.
    #[serde(default)]
    pub sky: Vec<String>,
    #[serde(default)]
    pub weather: Vec<String>,
    #[serde(default)]
    pub season: Vec<String>,
    #[serde(default)]
    pub moon: Vec<String>,
}

/// Whether a list of words names a thing, where naming nothing names everything.
fn names(list: &[String], word: Option<&str>) -> bool {
    if list.is_empty() {
        return true;
    }
    // A picture that names a weather cannot be chosen when the weather is not
    // known. Guessing would put a snowy picture up in a heatwave the one day
    // the network was down.
    word.is_some_and(|word| list.iter().any(|held| held.trim().to_lowercase() == word))
}

impl Picture {
    /// What this picture names, against what is outside.
    fn against(&self, outside: &Outside) -> [(&Vec<String>, Option<&'static str>); 4] {
        [
            (&self.moon, Some(outside.moon.word())),
            (&self.season, Some(outside.season.word())),
            (&self.sky, Some(outside.sky.word())),
            (&self.weather, outside.weather.map(|weather| weather.word())),
        ]
    }

    /// Whether it answers this outside at all.
    fn answers(&self, outside: &Outside) -> bool {
        self.against(outside).iter().all(|(list, word)| names(list, *word))
    }

    /// How particular it is, which is how many things it names.
    fn particular(&self, outside: &Outside) -> usize {
        self.against(outside).iter().filter(|(list, _)| !list.is_empty()).count()
    }
}

/// The whole set, as `theme/sky.toml` holds it.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Set {
    #[serde(default)]
    pub stir: Stir,
    #[serde(default, rename = "picture")]
    pub pictures: Vec<Picture>,
}

/// The picture for an outside, if the set holds one.
pub fn choose<'a>(pictures: &'a [Picture], outside: &Outside) -> Option<&'a Picture> {
    pictures
        .iter()
        .filter(|picture| picture.answers(outside))
        .max_by_key(|picture| picture.particular(outside))
}

/// What somebody asked for, over what the weather asked for.
///
/// Written by the settings panel and read here. Following the weather is the
/// point of all this, so it is what a file that does not exist means; a person
/// who has picked one picture and wants it left alone has said so, and saying
/// so is what writes the file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Wanted {
    /// Whether the picture changes with the sky, the weather and the year.
    pub follow: bool,
    /// The one that is up when it does not.
    pub picture: String,
}

impl Default for Wanted {
    fn default() -> Self {
        Wanted { follow: true, picture: String::new() }
    }
}

impl Set {
    /// The table, out of what a file holds. A table that will not parse is a
    /// panel with no pictures on it rather than a panel that will not open.
    pub fn read(held: &str) -> Option<Set> {
        toml::from_str(held).ok()
    }
}

impl Wanted {
    /// Read, forgivingly. A file somebody has edited by hand into nonsense is
    /// not a reason for the screen to have no wallpaper on it.
    pub fn read(held: &str) -> Self {
        toml::from_str(held).unwrap_or_default()
    }

    /// What was asked for, off the machine.
    ///
    /// Here rather than in each of the two programs that wants it, because the
    /// settings panel writes this file and the daemon reads it, and a file read
    /// two ways is a panel that says one thing while the screen shows another.
    /// A file nobody has written is following the weather, which is what this
    /// desktop does until it is told otherwise.
    pub fn asked() -> Self {
        crate::place::asked()
            .and_then(|at| std::fs::read_to_string(at).ok())
            .map(|held| Wanted::read(&held))
            .unwrap_or_default()
    }

    pub fn written(&self) -> String {
        toml::to_string(self).unwrap_or_default()
    }
}

/// The picture somebody chose and asked to be left alone, if the set still
/// holds it.
///
/// Its own function because it is the one question that can be answered without
/// knowing anything about the outside: a pinned picture is that picture in any
/// weather. `console-sky --now` asks this before it asks the sky, so that
/// choosing a picture does not wait on a web service that has nothing to say
/// about the answer.
///
/// A picture pinned and then deleted answers nothing, and what that leaves is
/// following, rather than a bare screen.
pub fn pinned<'a>(pictures: &'a [Picture], asked: &Wanted) -> Option<&'a Picture> {
    match asked.follow {
        true => None,
        false => pictures.iter().find(|picture| picture.name == asked.picture),
    }
}

/// The picture that should be on the screen: what was asked for, or what the
/// outside asks for, or whatever the set holds.
pub fn wanted<'a>(
    pictures: &'a [Picture],
    asked: &Wanted,
    outside: &Outside,
) -> Option<&'a Picture> {
    pinned(pictures, asked).or_else(|| choose(pictures, outside).or_else(|| pictures.first()))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn the_most_particular_picture_wins() {
        let set = set();
        let chosen = choose(&set, &outside(Sky::Night, Some(Weather::Snow))).expect("a picture");
        assert_eq!(chosen.name, "cozy-winter");
    }

    #[test]
    fn a_picture_for_a_part_of_the_day_beats_one_for_anything() {
        let set = set();
        let chosen = choose(&set, &outside(Sky::Night, Some(Weather::Rain))).expect("a picture");
        assert_eq!(chosen.name, "star-ride");
    }

    /// The whole reason a picture is allowed to name nothing: something has to
    /// be on the screen on a foggy afternoon nobody wrote a picture for.
    #[test]
    fn an_outside_nothing_answers_falls_to_the_picture_that_names_nothing() {
        let set = set();
        let chosen = choose(&set, &outside(Sky::Dusk, Some(Weather::Fog))).expect("a picture");
        assert_eq!(chosen.name, "terrarium");
    }

    /// A picture that names a weather cannot be chosen when the weather is not
    /// known. Guessing would put a snowy picture up in a heatwave the one day
    /// the network was down.
    #[test]
    fn a_picture_naming_a_weather_is_not_chosen_when_there_is_none_to_read() {
        let set = set();
        let chosen = choose(&set, &outside(Sky::Night, None)).expect("a picture");
        assert_eq!(chosen.name, "star-ride");
    }

    /// The four things a picture may name, each on its own.
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
        assert_eq!(choose(&set, &snowy).expect("a picture").name, "first-snow");

        let moonlit = Outside { moon: Moon::Full, season: Season::Summer, ..snowy };
        assert_eq!(choose(&set, &moonlit).expect("a picture").name, "moonlit");
    }

    /// The finer bands are what makes the golden hour worth having: a picture
    /// for a sunset is not a picture for the night that follows it.
    #[test]
    fn a_sunset_and_the_dusk_after_it_are_different_outsides() {
        let mut golden = picture("golden", &["sunrise", "sunset"], &[]);
        golden.by = "nobody".to_string();
        let set = vec![picture("terrarium", &[], &[]), golden];
        assert_eq!(
            choose(&set, &outside(Sky::Sunset, None)).expect("a picture").name,
            "golden"
        );
        assert_eq!(
            choose(&set, &outside(Sky::Dusk, None)).expect("a picture").name,
            "terrarium"
        );
    }

    #[test]
    fn a_set_holding_nothing_chooses_nothing() {
        assert!(choose(&[], &outside(Sky::Day, Some(Weather::Clear))).is_none());
    }

    #[test]
    fn a_pinned_picture_is_the_one_that_is_up() {
        let set = set();
        let asked = Wanted { follow: false, picture: "lazy-river".to_string() };
        let chosen =
            wanted(&set, &asked, &outside(Sky::Night, Some(Weather::Snow))).expect("a picture");
        assert_eq!(chosen.name, "lazy-river");
    }

    /// Pinning a picture and then deleting it should not leave a bare screen.
    #[test]
    fn a_pinned_picture_that_is_gone_goes_back_to_following_the_weather() {
        let set = set();
        let asked = Wanted { follow: false, picture: "sledding".to_string() };
        let chosen =
            wanted(&set, &asked, &outside(Sky::Night, Some(Weather::Snow))).expect("a picture");
        assert_eq!(chosen.name, "cozy-winter");
    }

    /// What `console-sky --now` asks before it asks the sky. A press on the
    /// wallpaper tab used to wait on a web service that had nothing to say
    /// about the answer, which is up to eight seconds between choosing a
    /// picture and seeing it.
    #[test]
    fn a_pinned_picture_is_known_without_anything_being_asked_of_the_weather() {
        let set = set();
        let pinned_on = Wanted { follow: false, picture: "lazy-river".to_string() };
        assert_eq!(pinned(&set, &pinned_on).expect("a picture").name, "lazy-river");

        // Both of these want the sky before they can answer, so both are worth
        // waiting for the weather.
        let following = Wanted { follow: true, picture: "lazy-river".to_string() };
        assert!(pinned(&set, &following).is_none());
        let gone = Wanted { follow: false, picture: "sledding".to_string() };
        assert!(pinned(&set, &gone).is_none());
    }

    #[test]
    fn what_was_asked_for_is_written_and_read_back_the_same() {
        let asked = Wanted { follow: false, picture: "star-ride".to_string() };
        assert_eq!(Wanted::read(&asked.written()), asked);
    }

    /// A file edited by hand into nonsense is not a reason for a bare screen.
    #[test]
    fn a_file_that_is_not_one_reads_as_following_the_weather() {
        assert_eq!(Wanted::read("follow = maybe"), Wanted::default());
        assert!(Wanted::read("").follow);
    }
}
