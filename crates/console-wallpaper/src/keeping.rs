//! What the wallpaper daemon does about what it found, and when it wakes next.
//!
//! Three of the four things this program holds between one look and the next
//! are here, and the fourth -- what the sky outside is doing -- only ever
//! arrives from somewhere else. What is decided is the settling: a wallpaper
//! that has just been covered is not put away at once, because the thing in
//! front of it may be a menu somebody is about to close, and swapping a moving
//! picture for a still one and back again is worse than leaving it moving for
//! a few seconds.
//!
//! The still picture is put up before the moving one rather than after,
//! whenever the moving one is going up for the first time. A frame is what the
//! wallpaper daemon holds while anything is in front of it, so the first frame
//! of an animation somebody has not seen yet is a picture rather than whatever
//! the decoder happened to hand over.
//!
//! Waking is a doing here rather than a wait in the loop. Between the three
//! reasons it wakes this program is asleep, and that is the whole of what it
//! costs a handheld -- so how long to sleep for is a decision like any other,
//! and one a test can hold against the clock rather than against a stopwatch.

use std::path::PathBuf;
use std::time::Duration;

use console_never::Never;
use console_program_contract::{Argv, Doing, Ending, Given, Opening, Program, Turn, Word};

use crate::covered::Covered;
use crate::weather::Weather;

pub const NOW: &str = "--now";

pub const LOOK_AGAIN: Duration = Duration::from_secs(300);

pub const TRY_AGAIN: Duration = Duration::from_secs(2);

pub const SETTLE: Duration = Duration::from_secs(15);

pub const SOONEST: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Going {
    Once,
    KeepGoing,
}

impl Going {
    pub fn of(argv: &Argv) -> Result<Self, Never> {
        let Ok(given) = argv.given(NOW);

        Ok(match given {
            Given::Yes => Going::Once,
            Given::No => Going::KeepGoing,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Away {
    PutIt,
    NotYet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Painted {
    Yes,
    No,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sky {
    pub showing: Option<PathBuf>,
    pub covered_since: Option<f64>,
    pub weather: Option<Weather>,
    pub going: Going,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Heard {
    Weather(Option<Weather>),
    Looked { seconds: f64, covered: Covered, chosen: Option<Chosen> },
    Painted { at: PathBuf, went: Painted },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    pub moving: PathBuf,
    pub still: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Its {
    Paint(PathBuf),
    Freshen(PathBuf),
    Again(Duration),
}

pub struct Sun;

impl Program for Sun {
    type State = Sky;
    type Hears = Heard;
    type Does = Its;

    fn opening(argv: &Argv) -> Opening<Sky> {
        let Ok(going) = Going::of(argv);
        let Ok(opening) = Opening::holding(Sky {
            showing: None,
            covered_since: None,
            weather: None,
            going,
        });

        opening
    }

    fn heard(state: &Sky, word: &Word<Heard>) -> Turn<Sky, Its> {
        let Word::Its(heard) = word else {
            let Ok(nothing) = Turn::nothing(state.clone());

            return nothing;
        };

        let Ok(turn) = match heard {
            Heard::Weather(weather) => {
                Turn::nothing(Sky { weather: *weather, ..state.clone() })
            }

            Heard::Painted { at, went } => match went {
                Painted::Yes => {
                    Turn::nothing(Sky { showing: Some(at.clone()), ..state.clone() })
                }
                Painted::No => Turn::doing(state.clone(), vec![Doing::Its(Its::Again(TRY_AGAIN))]),
            },

            Heard::Looked { seconds, covered, chosen } => {
                looked(state, *seconds, *covered, chosen.as_ref())
            }
        };

        turn
    }
}

fn looked(
    state: &Sky,
    seconds: f64,
    covered: Covered,
    chosen: Option<&Chosen>,
) -> Result<Turn<Sky, Its>, Never> {
    let covered_since = match covered {
        Covered::Yes => state.covered_since.or(Some(seconds)),
        Covered::No => None,
    };
    let settled = covered_since.map(|since| SETTLE.as_secs_f64() - (seconds - since));

    let away = match settled.is_some_and(|left| left <= 0.0) {
        true => Away::PutIt,
        false => Away::NotYet,
    };

    let mut doings = match chosen {
        Some(chosen) => putting(state, chosen, away)?,
        None => Vec::new(),
    };

    match state.going {
        Going::Once => {
            doings.push(Doing::Stop(Ending::Done));

            return Turn::doing(Sky { covered_since, ..state.clone() }, doings);
        }
        Going::KeepGoing => {},
    }

    let waiting = match settled.filter(|_| away == Away::NotYet) {
        Some(left) => LOOK_AGAIN.min(Duration::from_secs_f64(left.max(SOONEST.as_secs_f64()))),
        None => LOOK_AGAIN,
    };

    doings.push(Doing::Its(Its::Again(waiting)));

    Turn::doing(Sky { covered_since, ..state.clone() }, doings)
}

fn putting(state: &Sky, chosen: &Chosen, away: Away) -> Result<Vec<Doing<Its>>, Never> {
    let resting = match (away, &chosen.still) {
        (Away::PutIt, Some(_)) => Resting::Yes,
        (Away::PutIt, None) | (Away::NotYet, _) => Resting::No,
    };

    let put_up = match (resting, &chosen.still) {
        (Resting::Yes, Some(still)) => still.clone(),
        (Resting::Yes, None) | (Resting::No, _) => chosen.moving.clone(),
    };

    match state.showing.as_deref() == Some(put_up.as_path()) {
        true => return Ok(Vec::new()),
        false => {},
    }

    let first = resting == Resting::No && state.showing.as_deref() != chosen.still.as_deref();
    let mut doings = Vec::new();

    match (first, &chosen.still) {
        (true, Some(still)) => doings.push(Doing::Its(Its::Paint(still.clone()))),
        (true, None) | (false, _) => {},
    }

    match resting {
        Resting::Yes => {},
        Resting::No => doings.push(Doing::Its(Its::Freshen(chosen.moving.clone()))),
    }

    doings.push(Doing::Its(Its::Paint(put_up)));

    Ok(doings)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Resting {
    Yes,
    No,
}

pub fn wake(doings: &[Doing<Its>]) -> Result<Option<Duration>, Never> {
    Ok(doings.iter().rev().find_map(|doing| match doing {
        Doing::Its(Its::Again(waiting)) => Some(*waiting),

        Doing::Its(Its::Paint(_))
        | Doing::Its(Its::Freshen(_))
        | Doing::Ask(_)
        | Doing::Watch(_)
        | Doing::AskWhoever(_)
        | Doing::Start(_)
        | Doing::Listen(_)
        | Doing::Deafen(_)
        | Doing::Write(_)
        | Doing::Say(_)
        | Doing::Print(_)
        | Doing::Stop(_) => None,
    }))
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Said, walk};

    use super::*;

    fn sky() -> Sky {
        Sun::opening(&Argv::default()).state
    }

    fn waking(doings: &[Doing<Its>]) -> Option<Duration> {
        let Ok(waking) = wake(doings);

        waking
    }

    fn chosen() -> Chosen {
        Chosen {
            moving: PathBuf::from("/pictures/rain.gif"),
            still: Some(PathBuf::from("/pictures/rain.png")),
        }
    }

    fn said(from: &Sky, heard: &[Heard]) -> Said<Sky, Heard, Its> {
        let words: Vec<Word<Heard>> = heard.iter().cloned().map(Word::Its).collect();

        let Ok(said) = walk::<Sun>(from, &words);

        said
    }

    fn looked(seconds: f64, covered: Covered) -> Heard {
        Heard::Looked { seconds, covered, chosen: Some(chosen()) }
    }

    fn painted(at: &str) -> Heard {
        Heard::Painted { at: PathBuf::from(at), went: Painted::Yes }
    }

    #[test]
    fn the_still_one_goes_up_before_the_moving_one_the_first_time() {
        let after = said(&sky(), &[looked(0.0, Covered::No)]);

        let Ok(doings) = after.doings();

        assert_eq!(doings.first(), Some(&Doing::Its(Its::Paint(PathBuf::from(
            "/pictures/rain.png"
        )))));
        assert_eq!(
            doings.get(1),
            Some(&Doing::Its(Its::Freshen(PathBuf::from("/pictures/rain.gif"))))
        );
        assert_eq!(
            doings.get(2),
            Some(&Doing::Its(Its::Paint(PathBuf::from("/pictures/rain.gif"))))
        );
    }

    #[test]
    fn a_picture_already_up_is_not_put_up_again() {
        let up = said(&sky(), &[
            looked(0.0, Covered::No),
            painted("/pictures/rain.png"),
            painted("/pictures/rain.gif"),
        ]);
        let again = said(&up.now, &[looked(1.0, Covered::No)]);

        assert_eq!(again.doings(), Ok(vec![Doing::Its(Its::Again(LOOK_AGAIN))]));
    }

    #[test]
    fn something_in_front_of_it_does_not_put_it_away_at_once() {
        let up = said(&sky(), &[
            looked(0.0, Covered::No),
            painted("/pictures/rain.png"),
            painted("/pictures/rain.gif"),
        ]);
        let covered = said(&up.now, &[looked(1.0, Covered::Yes)]);

        let Ok(doings) = covered.doings();

        assert!(
            !doings.iter().any(|doing| matches!(doing, Doing::Its(Its::Paint(_)))),
            "it was put away the moment something covered it"
        );
        assert_eq!(covered.now.covered_since, Some(1.0));
    }

    #[test]
    fn it_wakes_for_the_end_of_the_settling_rather_than_for_the_sun() {
        let up = said(&sky(), &[looked(0.0, Covered::No), painted("/pictures/rain.gif")]);
        let covered = said(&up.now, &[looked(1.0, Covered::Yes)]);

        let Ok(doings) = covered.doings();

        assert_eq!(waking(&doings), Some(SETTLE));

        let later = said(&covered.now, &[looked(10.0, Covered::Yes)]);
        let Ok(after) = later.doings();

        assert_eq!(waking(&after), Some(Duration::from_secs_f64(6.0)));
    }

    #[test]
    fn once_it_has_settled_the_still_one_goes_up_and_it_sleeps_for_the_sun() {
        let up = said(&sky(), &[
            looked(0.0, Covered::No),
            painted("/pictures/rain.png"),
            painted("/pictures/rain.gif"),
        ]);
        let covered = said(&up.now, &[looked(1.0, Covered::Yes)]);
        let away = said(&covered.now, &[looked(17.0, Covered::Yes)]);

        let Ok(doings) = away.doings();

        assert!(doings.contains(&Doing::Its(Its::Paint(PathBuf::from("/pictures/rain.png")))));
        assert!(
            !doings.iter().any(|doing| matches!(doing, Doing::Its(Its::Freshen(_)))),
            "a still picture was freshened, which is a moving one's word"
        );
        assert_eq!(waking(&doings), Some(LOOK_AGAIN));
    }

    #[test]
    fn uncovering_it_starts_the_settling_over() {
        let covered = said(&sky(), &[looked(0.0, Covered::Yes), looked(5.0, Covered::Yes)]);

        assert_eq!(covered.now.covered_since, Some(0.0));

        let seen = said(&covered.now, &[looked(6.0, Covered::No)]);

        assert_eq!(seen.now.covered_since, None);

        let again = said(&seen.now, &[looked(7.0, Covered::Yes)]);

        assert_eq!(again.now.covered_since, Some(7.0));
    }

    #[test]
    fn a_wallpaper_that_would_not_take_is_tried_again_sooner() {
        let after = said(&sky(), &[Heard::Painted {
            at: PathBuf::from("/pictures/rain.gif"),
            went: Painted::No,
        }]);

        assert_eq!(after.doings(), Ok(vec![Doing::Its(Its::Again(TRY_AGAIN))]));
        assert_eq!(after.now.showing, None);
    }

    #[test]
    fn now_puts_one_up_and_stops() {
        let Ok(argv) = Argv::of(&[NOW]);

        let once = Sun::opening(&argv).state;
        let after = said(&once, &[looked(0.0, Covered::No)]);
        let Ok(doings) = after.doings();

        assert_eq!(doings.last(), Some(&Doing::Stop(Ending::Done)));
        assert_eq!(waking(&doings), None);
    }

    #[test]
    fn nothing_chosen_is_nothing_done_and_it_waits_for_the_sun() {
        let after = said(&sky(), &[Heard::Looked {
            seconds: 0.0,
            covered: Covered::No,
            chosen: None,
        }]);

        assert_eq!(after.doings(), Ok(vec![Doing::Its(Its::Again(LOOK_AGAIN))]));
    }

    #[test]
    fn what_the_sky_is_doing_only_ever_arrives_from_somewhere_else() {
        let after = said(&sky(), &[Heard::Weather(Some(Weather::Rain))]);
        let Ok(doings) = after.doings();

        assert_eq!(after.now.weather, Some(Weather::Rain));
        assert!(doings.is_empty());
    }
}
