//! What the wallpaper daemon does about what it found, and when it wakes next.
//!
//! Three of the four things this program holds between one look and the next
//! are here, and the fourth -- what the sky outside is doing -- only ever
//! arrives from somewhere else. What is decided is the settling: a wallpaper
//! that has just been covered is not put away at once, because the thing in
//! front of it may be a menu someone is about to close, and swapping a moving
//! picture for a still one and back again is worse than leaving it moving for
//! a few seconds.
//!
//! The still picture is put up before the moving one rather than after,
//! whenever the moving one is going up for the first time. A frame is what the
//! wallpaper daemon holds while anything is in front of it, so the first frame
//! of an animation someone has not seen yet is a picture rather than whatever
//! the decoder happened to hand over.
//!
//! A weather that did not arrive is not a weather. Asking is over a network on
//! a handheld that is carried out of range of one, and a curl that timed out
//! says nothing about the sky -- so the last answer stays where it is and the
//! picture goes on being the one for the rain it was raining, rather than
//! falling back to the no-weather picture every time the wifi drops and
//! climbing back out of it a minute later.
//!
//! Waking is an effect here rather than a wait in the loop. Between the three
//! reasons it wakes this program is asleep, and that is the whole of what it
//! costs a handheld -- so how long to sleep for is a decision like any other,
//! and one a test can hold against the clock rather than against a stopwatch.

use std::path::PathBuf;
use std::time::Duration;

use console_core_never::Never;
use console_program_contract::{Arguments, Effect, Exit, Flag, Initial, Program, Update, Event};

use crate::covered::Covered;
use console_weather::conditions::Weather;

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
    pub fn of(arguments: &Arguments) -> Result<Self, Never> {
        let Ok(given) = arguments.given(NOW);

        Ok(match given {
            Flag::Present => Going::Once,
            Flag::Absent => Going::KeepGoing,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Away {
    PutIt,
    NotYet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rendered {
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
pub enum WallpaperEvent {
    Weather(Option<Weather>),
    Looked { seconds: f64, covered: Covered, chosen: Option<Chosen> },
    Rendered { at: PathBuf, went: Rendered },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    pub moving: PathBuf,
    pub still: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WallpaperEffect {
    Paint(PathBuf),
    Refresh(PathBuf),
    Again(Duration),
}

pub struct Sun;

impl Program for Sun {
    type State = Sky;
    type Event = WallpaperEvent;
    type Effect = WallpaperEffect;

    fn init(arguments: &Arguments) -> Initial<Sky> {
        let Ok(going) = Going::of(arguments);
        let Ok(opening) = Initial::new(Sky {
            showing: None,
            covered_since: None,
            weather: None,
            going,
        });

        opening
    }

    fn update(state: &Sky, event: &Event<WallpaperEvent>) -> Update<Sky, WallpaperEffect> {
        let heard = match event {
            Event::Custom(heard) => heard,
            Event::Opened
            | Event::Changed(_)
            | Event::Tick(_, _)
            | Event::Replied(_)
            | Event::Chosen(_)
            | Event::Stopping => {
                let Ok(nothing) = Update::none(state.clone());

                return nothing;
            }
        };

        let Ok(turn) = match heard {
            WallpaperEvent::Weather(weather) => {
                Update::none(Sky { weather: weather.or(state.weather), ..state.clone() })
            }

            WallpaperEvent::Rendered { at, went } => match went {
                Rendered::Yes => {
                    Update::none(Sky { showing: Some(at.clone()), ..state.clone() })
                }
                Rendered::No => Update::new(state.clone(), vec![Effect::Custom(WallpaperEffect::Again(TRY_AGAIN))]),
            },

            WallpaperEvent::Looked { seconds, covered, chosen } => {
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
) -> Result<Update<Sky, WallpaperEffect>, Never> {
    let covered_since = match covered {
        Covered::Yes => state.covered_since.or(Some(seconds)),
        Covered::No => None,
    };
    let settled = covered_since.map(|since| SETTLE.as_secs_f64() - (seconds - since));

    let away = match settled.is_some_and(|left| left <= 0.0) {
        true => Away::PutIt,
        false => Away::NotYet,
    };

    let mut effects = match chosen {
        Some(chosen) => putting(state, chosen, away)?,
        None => Vec::new(),
    };

    match state.going {
        Going::Once => {
            effects.push(Effect::Stop(Exit::Success));

            return Update::new(Sky { covered_since, ..state.clone() }, effects);
        }
        Going::KeepGoing => {},
    }

    let waiting = match settled.filter(|_| away == Away::NotYet) {
        Some(left) => LOOK_AGAIN.min(Duration::from_secs_f64(left.max(SOONEST.as_secs_f64()))),
        None => LOOK_AGAIN,
    };

    effects.push(Effect::Custom(WallpaperEffect::Again(waiting)));

    Update::new(Sky { covered_since, ..state.clone() }, effects)
}

fn putting(state: &Sky, chosen: &Chosen, away: Away) -> Result<Vec<Effect<WallpaperEffect>>, Never> {
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
    let mut effects = Vec::new();

    match (first, &chosen.still) {
        (true, Some(still)) => effects.push(Effect::Custom(WallpaperEffect::Paint(still.clone()))),
        (true, None) | (false, _) => {},
    }

    match resting {
        Resting::Yes => {},
        Resting::No => effects.push(Effect::Custom(WallpaperEffect::Refresh(chosen.moving.clone()))),
    }

    effects.push(Effect::Custom(WallpaperEffect::Paint(put_up)));

    Ok(effects)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Resting {
    Yes,
    No,
}

pub fn wake(effects: &[Effect<WallpaperEffect>]) -> Result<Option<Duration>, Never> {
    Ok(effects.iter().rev().find_map(|effect| match effect {
        Effect::Custom(WallpaperEffect::Again(waiting)) => Some(*waiting),

        Effect::Custom(WallpaperEffect::Paint(_))
        | Effect::Custom(WallpaperEffect::Refresh(_))
        | Effect::Run(_)
        | Effect::Stream(_)
        | Effect::Prompt(_)
        | Effect::Spawn(_)
        | Effect::Subscribe(_)
        | Effect::Unsubscribe(_)
        | Effect::Write(_)
        | Effect::Notify(_)
        | Effect::Print(_)
        | Effect::Stop(_) => None,
    }))
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Trace, run_from};

    use super::*;

    fn sky() -> Sky {
        Sun::init(&Arguments::default()).state
    }

    fn waking(effects: &[Effect<WallpaperEffect>]) -> Option<Duration> {
        let Ok(waking) = wake(effects);

        waking
    }

    fn chosen() -> Chosen {
        Chosen {
            moving: PathBuf::from("/pictures/rain.gif"),
            still: Some(PathBuf::from("/pictures/rain.png")),
        }
    }

    fn said(from: &Sky, heard: &[WallpaperEvent]) -> Trace<Sky, WallpaperEvent, WallpaperEffect> {
        let events: Vec<Event<WallpaperEvent>> = heard.iter().cloned().map(Event::Custom).collect();

        let Ok(said) = run_from::<Sun>(from, &events);

        said
    }

    fn looked(seconds: f64, covered: Covered) -> WallpaperEvent {
        WallpaperEvent::Looked { seconds, covered, chosen: Some(chosen()) }
    }

    fn painted(at: &str) -> WallpaperEvent {
        WallpaperEvent::Rendered { at: PathBuf::from(at), went: Rendered::Yes }
    }

    #[test]
    fn the_still_one_goes_up_before_the_moving_one_the_first_time() {
        let after = said(&sky(), &[looked(0.0, Covered::No)]);

        let Ok(effects) = after.effects();

        assert_eq!(effects.first(), Some(&Effect::Custom(WallpaperEffect::Paint(PathBuf::from(
            "/pictures/rain.png"
        )))));
        assert_eq!(
            effects.get(1),
            Some(&Effect::Custom(WallpaperEffect::Refresh(PathBuf::from("/pictures/rain.gif"))))
        );
        assert_eq!(
            effects.get(2),
            Some(&Effect::Custom(WallpaperEffect::Paint(PathBuf::from("/pictures/rain.gif"))))
        );
    }

    #[test]
    fn a_picture_already_up_is_not_put_up_again() {
        let up = said(&sky(), &[
            looked(0.0, Covered::No),
            painted("/pictures/rain.png"),
            painted("/pictures/rain.gif"),
        ]);
        let again = said(&up.state, &[looked(1.0, Covered::No)]);

        assert_eq!(again.effects(), Ok(vec![Effect::Custom(WallpaperEffect::Again(LOOK_AGAIN))]));
    }

    #[test]
    fn something_in_front_of_it_does_not_put_it_away_at_once() {
        let up = said(&sky(), &[
            looked(0.0, Covered::No),
            painted("/pictures/rain.png"),
            painted("/pictures/rain.gif"),
        ]);
        let covered = said(&up.state, &[looked(1.0, Covered::Yes)]);

        let Ok(effects) = covered.effects();

        assert!(
            !effects.iter().any(|effect| matches!(effect, Effect::Custom(WallpaperEffect::Paint(_)))),
            "it was put away the moment something covered it"
        );
        assert_eq!(covered.state.covered_since, Some(1.0));
    }

    #[test]
    fn it_wakes_for_the_end_of_the_settling_rather_than_for_the_sun() {
        let up = said(&sky(), &[looked(0.0, Covered::No), painted("/pictures/rain.gif")]);
        let covered = said(&up.state, &[looked(1.0, Covered::Yes)]);

        let Ok(effects) = covered.effects();

        assert_eq!(waking(&effects), Some(SETTLE));

        let later = said(&covered.state, &[looked(10.0, Covered::Yes)]);
        let Ok(after) = later.effects();

        assert_eq!(waking(&after), Some(Duration::from_secs_f64(6.0)));
    }

    #[test]
    fn once_it_has_settled_the_still_one_goes_up_and_it_sleeps_for_the_sun() {
        let up = said(&sky(), &[
            looked(0.0, Covered::No),
            painted("/pictures/rain.png"),
            painted("/pictures/rain.gif"),
        ]);
        let covered = said(&up.state, &[looked(1.0, Covered::Yes)]);
        let away = said(&covered.state, &[looked(17.0, Covered::Yes)]);

        let Ok(effects) = away.effects();

        assert!(effects.contains(&Effect::Custom(WallpaperEffect::Paint(PathBuf::from("/pictures/rain.png")))));
        assert!(
            !effects.iter().any(|effect| matches!(effect, Effect::Custom(WallpaperEffect::Refresh(_)))),
            "a still picture was refreshed, which is a moving one's word"
        );
        assert_eq!(waking(&effects), Some(LOOK_AGAIN));
    }

    #[test]
    fn uncovering_it_starts_the_settling_over() {
        let covered = said(&sky(), &[looked(0.0, Covered::Yes), looked(5.0, Covered::Yes)]);

        assert_eq!(covered.state.covered_since, Some(0.0));

        let seen = said(&covered.state, &[looked(6.0, Covered::No)]);

        assert_eq!(seen.state.covered_since, None);

        let again = said(&seen.state, &[looked(7.0, Covered::Yes)]);

        assert_eq!(again.state.covered_since, Some(7.0));
    }

    #[test]
    fn a_wallpaper_that_would_not_take_is_tried_again_sooner() {
        let after = said(&sky(), &[WallpaperEvent::Rendered {
            at: PathBuf::from("/pictures/rain.gif"),
            went: Rendered::No,
        }]);

        assert_eq!(after.effects(), Ok(vec![Effect::Custom(WallpaperEffect::Again(TRY_AGAIN))]));
        assert_eq!(after.state.showing, None);
    }

    #[test]
    fn now_puts_one_up_and_stops() {
        let Ok(arguments) = Arguments::of(&[NOW]);

        let once = Sun::init(&arguments).state;
        let after = said(&once, &[looked(0.0, Covered::No)]);
        let Ok(effects) = after.effects();

        assert_eq!(effects.last(), Some(&Effect::Stop(Exit::Success)));
        assert_eq!(waking(&effects), None);
    }

    #[test]
    fn nothing_chosen_is_nothing_done_and_it_waits_for_the_sun() {
        let after = said(&sky(), &[WallpaperEvent::Looked {
            seconds: 0.0,
            covered: Covered::No,
            chosen: None,
        }]);

        assert_eq!(after.effects(), Ok(vec![Effect::Custom(WallpaperEffect::Again(LOOK_AGAIN))]));
    }

    #[test]
    fn what_the_sky_is_doing_only_ever_arrives_from_somewhere_else() {
        let after = said(&sky(), &[WallpaperEvent::Weather(Some(Weather::Rain))]);
        let Ok(effects) = after.effects();

        assert_eq!(after.state.weather, Some(Weather::Rain));
        assert!(effects.is_empty());
    }

    #[test]
    fn a_weather_nobody_could_ask_for_leaves_the_last_one_where_it_was() {
        let after = said(&sky(), &[
            WallpaperEvent::Weather(Some(Weather::Rain)),
            WallpaperEvent::Weather(None),
        ]);

        assert_eq!(after.state.weather, Some(Weather::Rain));

        let after = said(&after.state, &[WallpaperEvent::Weather(Some(Weather::Snow))]);

        assert_eq!(after.state.weather, Some(Weather::Snow));
    }
}
