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


use console_number::{fitted, toward_zero_u64};
use crate::moon::Moon;
use crate::press::Stir;
use crate::sun::{Season, Sky};
use crate::weather::Weather;

/// How long one picture holds before the next of the same standing takes over.
///
/// Two hours: long enough that a picture is a picture rather than a slideshow,
/// short enough that three pictures for a clear afternoon are three pictures by
/// the evening. The daemon looks again every five minutes, so a turn ending is
/// noticed within five minutes of ending.
pub const HOLD_SECONDS: f64 = 2.0 * 60.0 * 60.0;

/// Whose turn it is, among the pictures that answer an outside equally well.
///
/// A number off the clock rather than anything written down, so the daemon and
/// `--now` and a test all say the same thing about the same moment without any
/// of them keeping a place. It is counted from the epoch rather than from when
/// the machine came up, so a machine that is turned off for an hour comes back
/// to the picture the hour asks for rather than to the one it was showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Turn(pub u64);

impl Turn {
    /// The turn at a moment, in seconds since the epoch.
    pub fn at(seconds: f64) -> Turn {
        Turn(toward_zero_u64(seconds.max(0.0) / HOLD_SECONDS))
    }
}

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

/// Whether a picture answers what is outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answers {
    /// It names this, or names nothing and so names everything.
    Yes,
    /// It names something else.
    No,
}

/// Whether a list of words names a thing, where naming nothing names everything.
fn names(list: &[String], word: Option<&str>) -> Answers {
    if list.is_empty() {
        return Answers::Yes;
    }

    // A picture that names a weather cannot be chosen when the weather is not
    // known. Guessing would put a snowy picture up in a heatwave the one day
    // the network was down.
    match word.is_some_and(|word| list.iter().any(|held| held.trim().to_lowercase() == word)) {
        true => Answers::Yes,
        false => Answers::No,
    }
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
    fn answers(&self, outside: &Outside) -> Answers {
        let every = self
            .against(outside)
            .iter()
            .all(|(list, word)| names(list, *word) == Answers::Yes);

        match every {
            true => Answers::Yes,
            false => Answers::No,
        }
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

/// Every picture that answers this outside as well as any of them does.
///
/// Nothing here says which of them is up, only which are in the running, and
/// that is the whole of what particularity decides. A set where no two pictures
/// answer the same outside gives back one picture and the turn has nothing to
/// choose between; a set grown sideways gives back several.
fn standing<'a>(pictures: &'a [Picture], outside: &Outside) -> Vec<&'a Picture> {
    let answering =
        || pictures.iter().filter(|picture| picture.answers(outside) == Answers::Yes);

    let Some(best) = answering().map(|picture| picture.particular(outside)).max() else {
        return Vec::new();
    };

    answering().filter(|picture| picture.particular(outside) == best).collect()
}

/// The picture for an outside, if the set holds one.
///
/// The most particular picture wins, and the ones that are equally particular
/// take turns, which is what `turn` is for: two hours of one, then two hours of
/// the next.
pub fn choose<'a>(
    pictures: &'a [Picture],
    outside: &Outside,
    turn: Turn,
) -> Option<&'a Picture> {
    let standing = standing(pictures, outside);
    // `max(1)` is for the empty set, where the get below answers nothing anyway
    // and all this has to do is not divide by zero.
    // Taken as a remainder before the width changes, so the index is inside
    // the list and the conversion cannot be the thing that decides it.
    let count = fitted::<usize, u64>(standing.len().max(1));
    standing.get(fitted::<u64, usize>(turn.0 % count)).copied()
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
        match toml::from_str(held) {
            Ok(set) => Some(set),
            Err(fault) => {
                eprintln!("console-sky: the picture table will not parse: {fault}");

                None
            }
        }
    }
}

impl Wanted {
    /// Read, forgivingly. A file somebody has edited by hand into nonsense is
    /// not a reason for the screen to have no wallpaper on it.
    pub fn read(held: &str) -> Self {
        match toml::from_str(held) {
            Ok(wanted) => wanted,
            // Forgiving, as the doc says, and no longer silent about it: the
            // screen still gets a wallpaper and the journal gets the reason it
            // is not the one somebody asked for.
            Err(fault) => {
                eprintln!("console-sky: what was asked of the wallpaper will not parse: {fault}");

                Wanted::default()
            }
        }
    }

    /// What was asked for, off the machine.
    ///
    /// Here rather than in each of the two programs that wants it, because the
    /// settings panel writes this file and the daemon reads it, and a file read
    /// two ways is a panel that says one thing while the screen shows another.
    /// A file nobody has written is following the weather, which is what this
    /// desktop does until it is told otherwise.
    pub fn asked() -> Self {
        let Some(at) = crate::place::asked() else { return Wanted::default() };

        // Nobody has written the file, which is this desktop following the
        // weather until it is told otherwise. A file that is there and will not
        // be read is not that, and it used to arrive here as the same answer.
        match std::fs::read_to_string(&at) {
            Ok(held) => Wanted::read(&held),
            Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Wanted::default(),
            Err(fault) => {
                eprintln!("console-sky: {}: {fault}", at.display());

                Wanted::default()
            }
        }
    }

    pub fn written(&self) -> String {
        match toml::to_string(self) {
            Ok(written) => written,
            // A bool and a string: there is no value of this that toml cannot
            // write. If that ever stops being true, an empty file is what the
            // reader above takes as "follow the weather", so the machine lands
            // somewhere sensible and the journal says how it got there.
            Err(fault) => {
                eprintln!("console-sky: writing down what was asked: {fault}");

                String::new()
            }
        }
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
    turn: Turn,
) -> Option<&'a Picture> {
    pinned(pictures, asked).or_else(|| choose(pictures, outside, turn).or_else(|| pictures.first()))
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

    /// The turn most of these do not care about. A set where no two pictures
    /// answer the same outside says the same thing whichever turn it is, and
    /// the tests that are about the turn say so by naming another one.
    const FIRST: Turn = Turn(0);

    #[test]
    fn the_most_particular_picture_wins() {
        let set = set();
        let chosen =
            choose(&set, &outside(Sky::Night, Some(Weather::Snow)), FIRST).expect("a picture");
        assert_eq!(chosen.name, "cozy-winter");
    }

    #[test]
    fn a_picture_for_a_part_of_the_day_beats_one_for_anything() {
        let set = set();
        let chosen =
            choose(&set, &outside(Sky::Night, Some(Weather::Rain)), FIRST).expect("a picture");
        assert_eq!(chosen.name, "star-ride");
    }

    /// The whole reason a picture is allowed to name nothing: something has to
    /// be on the screen on a foggy afternoon nobody wrote a picture for.
    #[test]
    fn an_outside_nothing_answers_falls_to_the_picture_that_names_nothing() {
        let set = set();
        let chosen =
            choose(&set, &outside(Sky::Dusk, Some(Weather::Fog)), FIRST).expect("a picture");
        assert_eq!(chosen.name, "terrarium");
    }

    /// A picture that names a weather cannot be chosen when the weather is not
    /// known. Guessing would put a snowy picture up in a heatwave the one day
    /// the network was down.
    #[test]
    fn a_picture_naming_a_weather_is_not_chosen_when_there_is_none_to_read() {
        let set = set();
        let chosen = choose(&set, &outside(Sky::Night, None), FIRST).expect("a picture");
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
        assert_eq!(choose(&set, &snowy, FIRST).expect("a picture").name, "first-snow");

        let moonlit = Outside { moon: Moon::Full, season: Season::Summer, ..snowy };
        assert_eq!(choose(&set, &moonlit, FIRST).expect("a picture").name, "moonlit");
    }

    /// The finer bands are what makes the golden hour worth having: a picture
    /// for a sunset is not a picture for the night that follows it.
    #[test]
    fn a_sunset_and_the_dusk_after_it_are_different_outsides() {
        let mut golden = picture("golden", &["sunrise", "sunset"], &[]);
        golden.by = "nobody".to_string();
        let set = vec![picture("terrarium", &[], &[]), golden];
        assert_eq!(
            choose(&set, &outside(Sky::Sunset, None), FIRST).expect("a picture").name,
            "golden"
        );
        assert_eq!(
            choose(&set, &outside(Sky::Dusk, None), FIRST).expect("a picture").name,
            "terrarium"
        );
    }

    #[test]
    fn a_set_holding_nothing_chooses_nothing() {
        assert!(choose(&[], &outside(Sky::Day, Some(Weather::Clear)), FIRST).is_none());
    }

    /// The whole point of a second picture for an outside somebody already has
    /// a picture for: both of them are up, one after the other.
    #[test]
    fn pictures_of_the_same_standing_take_turns() {
        let set = vec![
            picture("terrarium", &[], &[]),
            picture("star-ride", &["night"], &[]),
            picture("dancing-frogs", &["night"], &[]),
        ];
        let night = outside(Sky::Night, Some(Weather::Rain));
        let name = |turn: u64| choose(&set, &night, Turn(turn)).expect("a picture").name.clone();
        assert_eq!(name(0), "star-ride");
        assert_eq!(name(1), "dancing-frogs");
        assert_eq!(name(2), "star-ride");
        assert_eq!(name(3), "dancing-frogs");
    }

    /// Taking turns is only ever between equals. A picture that names the
    /// weather as well as the hour is up for the whole of the weather it names,
    /// however many pictures for that hour alone are waiting behind it.
    #[test]
    fn a_more_particular_picture_does_not_take_turns_with_a_less_particular_one() {
        let set = vec![
            picture("star-ride", &["night"], &[]),
            picture("dancing-frogs", &["night"], &[]),
            picture("cozy-winter", &["night"], &["snow"]),
        ];
        let snowing = outside(Sky::Night, Some(Weather::Snow));
        for turn in 0..6 {
            let chosen = choose(&set, &snowing, Turn(turn)).expect("a picture");
            assert_eq!(chosen.name, "cozy-winter", "turn {turn}");
        }
    }

    /// A set where nothing ties is the set this machine had before there was a
    /// second picture for anything, and the turn has to leave it alone.
    #[test]
    fn a_set_where_nothing_ties_says_the_same_thing_all_day() {
        let set = set();
        let night = outside(Sky::Night, Some(Weather::Snow));
        for turn in 0..12 {
            assert_eq!(choose(&set, &night, Turn(turn)).expect("a picture").name, "cozy-winter");
        }
    }

    /// Two hours, off the clock rather than off the machine's uptime, so two
    /// machines in a room agree and one machine agrees with itself across a
    /// suspend.
    #[test]
    fn a_turn_is_two_hours_of_the_clock() {
        let hour = 60.0 * 60.0;
        assert_eq!(Turn::at(0.0), Turn(0));
        assert_eq!(Turn::at(hour), Turn(0));
        assert_eq!(Turn::at(2.0 * hour), Turn(1));
        assert_eq!(Turn::at(2.0 * hour - 1.0), Turn(0));
        assert_eq!(Turn::at(24.0 * hour), Turn(12));
    }

    #[test]
    fn a_pinned_picture_is_the_one_that_is_up() {
        let set = set();
        let asked = Wanted { follow: false, picture: "lazy-river".to_string() };
        let chosen = wanted(&set, &asked, &outside(Sky::Night, Some(Weather::Snow)), FIRST)
            .expect("a picture");
        assert_eq!(chosen.name, "lazy-river");
    }

    /// Pinning a picture and then deleting it should not leave a bare screen.
    #[test]
    fn a_pinned_picture_that_is_gone_goes_back_to_following_the_weather() {
        let set = set();
        let asked = Wanted { follow: false, picture: "sledding".to_string() };
        let chosen = wanted(&set, &asked, &outside(Sky::Night, Some(Weather::Snow)), FIRST)
            .expect("a picture");
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

    /// Somebody who has pinned a picture has said they want that picture, and
    /// two hours later they still want that picture.
    #[test]
    fn a_pinned_picture_does_not_take_turns_with_anything() {
        let set = vec![
            picture("star-ride", &["night"], &[]),
            picture("dancing-frogs", &["night"], &[]),
        ];
        let asked = Wanted { follow: false, picture: "star-ride".to_string() };
        let night = outside(Sky::Night, None);
        for turn in 0..6 {
            let chosen = wanted(&set, &asked, &night, Turn(turn)).expect("a picture");
            assert_eq!(chosen.name, "star-ride", "turn {turn}");
        }
    }

    #[test]
    fn what_was_asked_for_is_written_and_read_back_the_same() {
        let asked = Wanted { follow: false, picture: "star-ride".to_string() };
        assert_eq!(Wanted::read(&asked.written()), asked);
    }

    /// The set the machine ships with, out of the tree rather than made up
    /// here. The two tests below are about that set and not about the rule.
    fn shipped() -> Set {
        let at = crate::place::table();
        let held = std::fs::read_to_string(&at)
            .unwrap_or_else(|fault| panic!("{} could not be read: {fault}", at.display()));
        Set::read(&held)
            .unwrap_or_else(|| panic!("{} is not a table this can read", at.display()))
    }

    /// Two pictures with one name are two pictures pressed over each other,
    /// because a picture is written to a file named after it.
    #[test]
    fn no_two_pictures_in_the_shipped_table_are_called_the_same_thing() {
        let set = shipped();
        let mut seen = std::collections::BTreeSet::new();
        for picture in &set.pictures {
            assert!(seen.insert(picture.name.clone()), "two pictures called {}", picture.name);
        }
    }

    /// Taking turns is a rule and the set is what makes it worth having. A set
    /// where nothing ever ties would pass every test above this one and still
    /// show one picture from one clear morning to the next.
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
                choose(&set.pictures, &clear, Turn(turn)).expect("a picture").name.as_str()
            })
            .collect();
        let how_many: std::collections::BTreeSet<&str> = over_a_day.iter().copied().collect();
        assert!(how_many.len() > 1, "a whole clear day showed only {over_a_day:?}");
    }

    /// A file edited by hand into nonsense is not a reason for a bare screen.
    #[test]
    fn a_file_that_is_not_one_reads_as_following_the_weather() {
        assert_eq!(Wanted::read("follow = maybe"), Wanted::default());
        assert!(Wanted::read("").follow);
    }
}
