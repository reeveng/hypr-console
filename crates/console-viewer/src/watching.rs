//! What a press on the card decides, and what it forgets.
//!
//! Every piece of this was already testable on its own -- which things in a
//! folder can be shown is `reel`, how far through a film you are is `playing`,
//! whether the card has gone quiet is `waking` -- and what was not was the
//! composition. Stepping to the next thing in the folder is four of those
//! pieces moving together, and getting it wrong leaves the second film playing
//! from where the first one was, at the first one's speed, with the first
//! one's subtitles on.
//!
//! So the forgetting is one turn here and is what the tests are about. A press
//! that changes which thing is on the screen rewinds every setting that
//! belonged to the last one; a press that only moves inside the same thing
//! keeps all of it.
//!
//! Waking is the other half. The card hides everything but the picture after a
//! few quiet seconds, and any press wakes it: the first press after that is
//! spent on the waking rather than on what it landed on, which is why
//! `stirred` answers what the card was rather than what it is now.

use console_never::Never;
use console_program_contract::{Argv, Doing, Opening, Program, Turn, Word};

use crate::kinds::Kind;
use crate::playing::{self, Along, Captions, Running};
use crate::reel::{Reel, Shot, Stood};
use crate::waking::{self, Awake};

#[derive(Debug, Clone, PartialEq)]
pub struct Watching {
    pub reel: Reel,
    pub along: Along,
    pub running: Running,
    pub sought: Option<u64>,
    pub speed: usize,
    pub captions: Captions,
    pub tracks: usize,
    pub stirred: Since,
}

pub type Since = std::time::Duration;

impl Watching {
    pub fn of(reel: Reel, since: Since) -> Result<Self, Never> {
        let Ok(ordinary) = playing::ordinary();

        Ok(Watching {
            reel,
            along: Along::default(),
            running: Running::default(),
            sought: None,
            speed: ordinary,
            captions: Captions::default(),
            tracks: 0,
            stirred: since,
        })
    }

    pub fn showing(&self) -> Result<&Shot, Never> {
        self.reel.showing()
    }

    fn rewound(&self) -> Result<Self, Never> {
        Ok(Watching {
            along: Along::default(),
            running: Running::default(),
            sought: None,
            captions: Captions::default(),
            tracks: 0,
            ..self.clone()
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Heard {
    Stepped { by: isize, at: Since },
    StoodOn { name: String, at: Since },
    Listed { listing: Vec<(String, String)>, at: Since },
    Scrubbed { by: i32, at: Since },
    SoughtTo { fraction: f64, at: Since },
    Running(Since),
    Speed { which: usize, at: Since },
    Words { which: usize, at: Since },
    Tracks(usize),
    Where { at: u64, whole: u64 },
    Stirred(Since),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Its {
    Refresh,
    TurnToTheCard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stirred {
    Awake,
    Woke,
}

pub struct Watch;

impl Program for Watch {
    type State = Watching;
    type Hears = Heard;
    type Does = Its;

    fn opening(_argv: &Argv) -> Opening<Watching> {
        let Ok(watching) = Watching::of(Reel::default(), Since::ZERO);
        let Ok(opening) = Opening::holding(watching);

        opening
    }

    fn heard(state: &Watching, word: &Word<Heard>) -> Turn<Watching, Its> {
        let Word::Its(heard) = word else {
            let Ok(nothing) = Turn::nothing(state.clone());

            return nothing;
        };

        let Ok(turn) = match heard {
            Heard::Stepped { by, at } => {
                let mut reel = state.reel.clone();

                let Ok(()) = reel.step(*by);
                let Ok(rewound) = state.rewound();

                Turn::doing(
                    Watching { reel, stirred: *at, ..rewound },
                    vec![Doing::Its(Its::Refresh)],
                )
            }

            Heard::StoodOn { name, at } => {
                let mut reel = state.reel.clone();

                let Ok(_) = reel.stand_on(name);
                let Ok(rewound) = state.rewound();

                Turn::doing(
                    Watching { reel, stirred: *at, ..rewound },
                    vec![Doing::Its(Its::TurnToTheCard)],
                )
            }

            Heard::Listed { listing, at } => relisted(state, listing, *at),

            Heard::Scrubbed { by, at } => {
                let Ok(step) = console_number_conversion::fitted(playing::STEP);
                let step = i64::from(*by).saturating_mul(step);
                let Ok(along) = state.along.moved(step);

                Turn::nothing(Watching {
                    along,
                    sought: Some(along.at),
                    stirred: *at,
                    ..state.clone()
                })
            }

            Heard::SoughtTo { fraction, at } => {
                let Ok(along) = state.along.sought(*fraction);

                Turn::doing(
                    Watching { along, sought: Some(along.at), stirred: *at, ..state.clone() },
                    vec![Doing::Its(Its::Refresh)],
                )
            }

            Heard::Running(at) => {
                let Ok(other) = state.running.other();

                Turn::doing(
                    Watching { running: other, stirred: *at, ..state.clone() },
                    vec![Doing::Its(Its::Refresh)],
                )
            }

            Heard::Speed { which, at } => {
                Turn::nothing(Watching { speed: *which, stirred: *at, ..state.clone() })
            }

            Heard::Words { which, at } => {
                let Ok(chosen) = Captions::chosen(*which);

                Turn::nothing(Watching { captions: chosen, stirred: *at, ..state.clone() })
            }

            Heard::Tracks(tracks) => {
                Turn::nothing(Watching { tracks: *tracks, ..state.clone() })
            }

            Heard::Where { at, whole } => {
                let Ok(along) = Along::new(*at, *whole);

                Turn::nothing(Watching { along, ..state.clone() })
            }

            Heard::Stirred(at) => Turn::nothing(Watching { stirred: *at, ..state.clone() }),
        };

        turn
    }
}

fn relisted(
    state: &Watching,
    listing: &[(String, String)],
    at: Since,
) -> Result<Turn<Watching, Its>, Never> {
    let Ok(showing) = state.showing();
    let name = showing.name.clone();
    let Ok(found) = Reel::of(listing, &name);

    let Some(mut reel) = found else {
        return Turn::nothing(state.clone());
    };

    let Ok(stood) = reel.stand_on(&name);

    match stood {
        Stood::OnIt => Turn::nothing(Watching { reel, ..state.clone() }),
        Stood::NotThere => {
            let Ok(rewound) = state.rewound();

            Turn::nothing(Watching { reel, stirred: at, ..rewound })
        },
    }
}

pub fn stirred(state: &Watching, now: Since) -> Result<Stirred, Never> {
    let Ok(awake) = waking::awake(now.saturating_sub(state.stirred));

    Ok(match awake {
        Awake::Yes => Stirred::Awake,
        Awake::No => Stirred::Woke,
    })
}

pub fn awake(state: &Watching, now: Since) -> Result<Awake, Never> {
    waking::awake(now.saturating_sub(state.stirred))
}

pub fn alone(state: &Watching) -> Result<Alone, Never> {
    let Ok(many) = state.reel.many();

    Ok(match many > 1 {
        true => Alone::No,
        false => Alone::Yes,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alone {
    Yes,
    No,
}

pub fn plays(state: &Watching) -> Result<Kind, Never> {
    let Ok(showing) = state.showing();

    Ok(showing.kind)
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Said, walk};

    use super::*;

    fn folder() -> Vec<(String, String)> {
        vec![
            ("beach.jpg".to_string(), "image/jpeg".to_string()),
            ("holiday.mp4".to_string(), "video/mp4".to_string()),
            ("sunset.png".to_string(), "image/png".to_string()),
        ]
    }

    fn watching() -> Watching {
        let Ok(reel) = Reel::of(&folder(), "holiday.mp4");
        let Ok(watching) = Watching::of(reel.unwrap_or_default(), Since::ZERO);

        watching
    }

    fn showing(state: &Watching) -> &Shot {
        let Ok(shot) = state.showing();

        shot
    }

    fn said(from: &Watching, heard: &[Heard]) -> Said<Watching, Heard, Its> {
        let words: Vec<Word<Heard>> = heard.iter().cloned().map(Word::Its).collect();

        let Ok(said) = walk::<Watch>(from, &words);

        said
    }

    fn a_film_part_way_through() -> Watching {
        let after = said(&watching(), &[
            Heard::Where { at: 100, whole: 600 },
            Heard::Running(Since::ZERO),
            Heard::Speed { which: 3, at: Since::ZERO },
            Heard::Words { which: 1, at: Since::ZERO },
            Heard::Tracks(2),
        ]);

        after.now
    }

    #[test]
    fn moving_inside_the_same_film_keeps_everything_about_it() {
        let was = a_film_part_way_through();
        let after = said(&was, &[Heard::Scrubbed { by: 1, at: Since::from_secs(9) }]);

        assert_eq!(after.now.speed, was.speed);
        assert_eq!(after.now.captions, was.captions);
        assert_eq!(after.now.running, was.running);
        assert_eq!(after.now.tracks, was.tracks);
    }

    #[test]
    fn stepping_to_the_next_thing_forgets_what_belonged_to_the_last_one() {
        let was = a_film_part_way_through();
        let after = said(&was, &[Heard::Stepped { by: 1, at: Since::from_secs(9) }]);

        assert_eq!(showing(&after.now).name, "sunset.png");
        assert_eq!(after.now.along, Along::default());
        assert_eq!(after.now.running, Running::default());
        assert_eq!(after.now.speed, was.speed, "the speed is the person's, not the film's");
        assert_eq!(after.now.captions, Captions::default());
        assert_eq!(after.now.sought, None);
        assert_eq!(after.now.tracks, 0);
    }

    #[test]
    fn standing_on_one_from_the_folder_turns_back_to_the_card() {
        let after = said(&watching(), &[Heard::StoodOn {
            name: "beach.jpg".to_string(),
            at: Since::from_secs(2),
        }]);

        assert_eq!(showing(&after.now).name, "beach.jpg");
        assert_eq!(after.doings(), Ok(vec![Doing::Its(Its::TurnToTheCard)]));
    }

    #[test]
    fn a_folder_that_still_holds_it_leaves_the_card_alone() {
        let was = a_film_part_way_through();
        let after = said(&was, &[Heard::Listed {
            listing: folder(),
            at: Since::from_secs(9),
        }]);

        assert_eq!(showing(&after.now).name, "holiday.mp4");
        assert_eq!(after.now.along, was.along);
        assert_eq!(after.now.captions, was.captions);
    }

    #[test]
    fn a_thing_that_has_gone_leaves_nothing_of_itself_on_the_next_one() {
        let was = a_film_part_way_through();
        let gone: Vec<(String, String)> =
            folder().into_iter().filter(|(name, _)| name != "holiday.mp4").collect();

        let after = said(&was, &[Heard::Listed { listing: gone, at: Since::from_secs(9) }]);

        assert_ne!(showing(&after.now).name, "holiday.mp4");
        assert_eq!(after.now.along, Along::default());
        assert_eq!(after.now.captions, Captions::default());
    }

    #[test]
    fn the_card_goes_quiet_and_any_press_wakes_it() {
        let quiet = waking::QUIET.saturating_add(Since::from_secs(1));

        assert_eq!(awake(&watching(), quiet), Ok(Awake::No));
        assert_eq!(stirred(&watching(), quiet), Ok(Stirred::Woke));
        assert_eq!(stirred(&watching(), Since::from_secs(1)), Ok(Stirred::Awake));

        let after = said(&watching(), &[Heard::Stirred(quiet)]);

        assert_eq!(awake(&after.now, quiet), Ok(Awake::Yes));
    }

    #[test]
    fn where_the_film_is_does_not_wake_the_card() {
        let quiet = waking::QUIET.saturating_add(Since::from_secs(1));
        let after = said(&watching(), &[Heard::Where { at: 100, whole: 600 }]);

        assert_eq!(awake(&after.now, quiet), Ok(Awake::No));
    }
}
