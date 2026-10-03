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
//!
//! Y on a row of the Media page is the files panel, standing on that file.
//! Renaming a photograph, copying it to a stick and throwing it away all live
//! there already, behind the same button the music panel puts them behind, and
//! none of it is worth teaching this panel twice. The card page has its own Y
//! -- what this picture is, how fast the film runs, which words are on it --
//! so the offer is the list's alone.

use console_core_internal_programs::InternalProgram;
use std::path::PathBuf;

use console_core_never::Never;
use console_core_state_machine::{Machine, Queue, Transition};
use console_program_contract::{Arguments, Effect, Command, Event};

use crate::kinds::Kind;
use crate::playing::{self, Along, Captions, Running};
use crate::reel::{Reel, Shot, Stood};
use crate::waking::{self, Woken};

pub const FILES: InternalProgram = InternalProgram::Files;

#[derive(Debug, Clone, PartialEq)]
pub struct Watching {
    pub reel: Reel,
    pub along: Along,
    pub running: Running,
    pub sought: Option<u64>,
    pub speed: u32,
    pub captions: Captions,
    pub tracks: u32,
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

    pub fn current(&self) -> Result<&Shot, Never> {
        self.reel.current()
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
pub enum ViewerEvent {
    Stepped { by: i32, at: Since },
    StoodOn { name: String, at: Since },
    Listed { listing: Vec<(String, String)>, at: Since },
    Scrubbed { by: i32, at: Since },
    SoughtTo { fraction: f64, at: Since },
    Running(Since),
    Speed { which: u32, at: Since },
    Text { which: u32, at: Since },
    Tracks(u32),
    Where { at: u64, whole: u64 },
    WakeOutcome(Since),
    Shown(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewerEffect {
    Refresh,
    TurnToTheCard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeOutcome {
    AlreadyAwake,
    Woke,
}

pub struct Watch;

impl Machine for Watch {
    type Input = Arguments;
    type State = Watching;
    type Request = Event<ViewerEvent>;
    type Effect = Effect<ViewerEffect>;

    fn initialize(arguments: &Arguments, _previous: Option<Watching>, effects: &mut Effects) -> Result<Watching, Never> {
        let Ok(opening) = initial(arguments);

        opening.offered(effects)
    }

    fn handle(state: Watching, event: Event<ViewerEvent>, effects: &mut Effects) -> Result<Watching, Never> {
        let Ok(decided) = decide(&state, &event);

        decided.offered(effects)
    }
}

type Effects = Queue<Effect<ViewerEffect>>;

fn initial(_argv: &Arguments) -> Result<Transition<Watching, Effect<ViewerEffect>>, Never> {
    let Ok(watching) = Watching::of(Reel::default(), Since::ZERO);
    let Ok(opening) = Transition::without_effects(watching);

    Ok(opening)
}

fn decide(state: &Watching, event: &Event<ViewerEvent>) -> Result<Transition<Watching, Effect<ViewerEffect>>, Never> {
    let heard = match event {
        Event::Custom(heard) => heard,
        Event::Opened
        | Event::Changed(_)
        | Event::Tick(_, _)
        | Event::Replied(_)
        | Event::Chosen(_)
        | Event::Stopping => {
            let Ok(nothing) = Transition::without_effects(state.clone());

            return Ok(nothing);
        }
    };

    let Ok(turn) = match heard {
        ViewerEvent::Stepped { by, at } => {
            let mut reel = state.reel.clone();

            let Ok(()) = reel.step(*by);
            let Ok(rewound) = state.rewound();

            Transition::new(
                Watching { reel, stirred: *at, ..rewound },
                vec![Effect::Custom(ViewerEffect::Refresh)],
            )
        }

        ViewerEvent::StoodOn { name, at } => {
            let mut reel = state.reel.clone();

            let Ok(_) = reel.stand_on(name);
            let Ok(rewound) = state.rewound();

            Transition::new(
                Watching { reel, stirred: *at, ..rewound },
                vec![Effect::Custom(ViewerEffect::TurnToTheCard)],
            )
        }

        ViewerEvent::Listed { listing, at } => relisted(state, listing, *at),

        ViewerEvent::Scrubbed { by, at } => {
            let Ok(step) = console_core_number_conversion::fitted(playing::STEP);
            let step = i64::from(*by).saturating_mul(step);
            let Ok(along) = state.along.moved(step);

            Transition::without_effects(Watching {
                along,
                sought: Some(along.at),
                stirred: *at,
                ..state.clone()
            })
        }

        ViewerEvent::SoughtTo { fraction, at } => {
            let Ok(along) = state.along.sought(*fraction);

            Transition::new(
                Watching { along, sought: Some(along.at), stirred: *at, ..state.clone() },
                vec![Effect::Custom(ViewerEffect::Refresh)],
            )
        }

        ViewerEvent::Running(at) => {
            let Ok(other) = state.running.other();

            Transition::new(
                Watching { running: other, stirred: *at, ..state.clone() },
                vec![Effect::Custom(ViewerEffect::Refresh)],
            )
        }

        ViewerEvent::Speed { which, at } => {
            Transition::without_effects(Watching { speed: *which, stirred: *at, ..state.clone() })
        }

        ViewerEvent::Text { which, at } => {
            let Ok(chosen) = Captions::from_track(*which);

            Transition::without_effects(Watching { captions: chosen, stirred: *at, ..state.clone() })
        }

        ViewerEvent::Tracks(tracks) => {
            Transition::without_effects(Watching { tracks: *tracks, ..state.clone() })
        }

        ViewerEvent::Where { at, whole } => {
            let along = Along { at: *at, whole: *whole };

            Transition::without_effects(Watching { along, ..state.clone() })
        }

        ViewerEvent::WakeOutcome(at) => Transition::without_effects(Watching { stirred: *at, ..state.clone() }),

        ViewerEvent::Shown(at) => {
            let Ok(files) = Command::internal(FILES, &[&at.to_string_lossy()]);

            Transition::new(state.clone(), vec![Effect::Spawn(files)])
        }
    };

    Ok(turn)
}

fn relisted(
    state: &Watching,
    listing: &[(String, String)],
    at: Since,
) -> Result<Transition<Watching, Effect<ViewerEffect>>, Never> {
    let Ok(showing) = state.current();
    let name = showing.name.clone();
    let Ok(found) = Reel::of(listing, &name);

    let mut reel = match found {
        Some(reel) => reel,
        None => return Transition::without_effects(state.clone()),
    };

    let Ok(stood) = reel.stand_on(&name);

    match stood {
        Stood::OnIt => Transition::without_effects(Watching { reel, ..state.clone() }),
        Stood::NotThere => {
            let Ok(rewound) = state.rewound();

            Transition::without_effects(Watching { reel, stirred: at, ..rewound })
        },
    }
}

pub fn stirred(state: &Watching, now: Since) -> Result<WakeOutcome, Never> {
    let Ok(awake) = waking::awake(now.saturating_sub(state.stirred));

    Ok(match awake {
        Woken::Yes => WakeOutcome::AlreadyAwake,
        Woken::No => WakeOutcome::Woke,
    })
}

pub fn awake(state: &Watching, now: Since) -> Result<Woken, Never> {
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
    let Ok(showing) = state.current();

    Ok(showing.kind)
}

#[cfg(test)]
mod tests {
    use console_core_state_machine::{Trace, run_from};

    use super::*;

    type Failure = Box<dyn std::error::Error>;

    fn folder() -> Result<Vec<(String, String)>, Never> {
        Ok(vec![
            (String::from("beach.jpg"), String::from("image/jpeg")),
            (String::from("holiday.mp4"), String::from("video/mp4")),
            (String::from("sunset.png"), String::from("image/png")),
        ])
    }

    fn sample_watching() -> Result<Watching, Failure> {
        let Ok(folder) = folder();
        let Ok(reel) = Reel::of(&folder, "holiday.mp4");
        let reel = reel.ok_or("a folder with a film in it is a reel")?;
        let Ok(watching) = Watching::of(reel, Since::ZERO);

        Ok(watching)
    }

    fn current_name(state: &Watching) -> Result<&str, Never> {
        let Ok(shot) = state.current();

        Ok(&shot.name)
    }

    fn run_events(from: &Watching, heard: &[ViewerEvent]) -> Result<Trace<Watching, Event<ViewerEvent>, Effect<ViewerEffect>>, Never> {
        let events: Vec<Event<ViewerEvent>> = heard.iter().cloned().map(Event::Custom).collect();

        run_from::<Watch>(from.clone(), &events)
    }

    fn a_film_part_way_through() -> Result<Watching, Failure> {
        let watching = sample_watching()?;
        let Ok(after) = run_events(&watching, &[
            ViewerEvent::Where { at: 100, whole: 600 },
            ViewerEvent::Running(Since::ZERO),
            ViewerEvent::Speed { which: 3, at: Since::ZERO },
            ViewerEvent::Text { which: 1, at: Since::ZERO },
            ViewerEvent::Tracks(2),
        ]);

        Ok(after.state)
    }

    #[test]
    fn moving_inside_the_same_film_keeps_everything_about_it() -> Result<(), Failure> {
        let was = a_film_part_way_through()?;
        let Ok(after) = run_events(&was, &[ViewerEvent::Scrubbed { by: 1, at: Since::from_secs(9) }]);

        assert_eq!(after.state.speed, was.speed);
        assert_eq!(after.state.captions, was.captions);
        assert_eq!(after.state.running, was.running);
        assert_eq!(after.state.tracks, was.tracks);

        Ok(())
    }

    #[test]
    fn stepping_to_the_next_thing_forgets_what_belonged_to_the_last_one() -> Result<(), Failure> {
        let was = a_film_part_way_through()?;
        let Ok(after) = run_events(&was, &[ViewerEvent::Stepped { by: 1, at: Since::from_secs(9) }]);
        let Ok(name) = current_name(&after.state);

        assert_eq!(name, "sunset.png");
        assert_eq!(after.state.along, Along::default());
        assert_eq!(after.state.running, Running::default());
        assert_eq!(after.state.speed, was.speed, "the speed is the person's, not the film's");
        assert_eq!(after.state.captions, Captions::default());
        assert_eq!(after.state.sought, None);
        assert_eq!(after.state.tracks, 0);

        Ok(())
    }

    #[test]
    fn standing_on_one_from_the_folder_turns_back_to_the_card() -> Result<(), Failure> {
        let watching = sample_watching()?;
        let Ok(after) = run_events(&watching, &[ViewerEvent::StoodOn {
            name: String::from("beach.jpg"),
            at: Since::from_secs(2),
        }]);
        let Ok(name) = current_name(&after.state);

        assert_eq!(name, "beach.jpg");
        assert_eq!(after.effects(), Ok(vec![Effect::Custom(ViewerEffect::TurnToTheCard)]));

        Ok(())
    }

    #[test]
    fn a_folder_that_still_holds_it_leaves_the_card_alone() -> Result<(), Failure> {
        let was = a_film_part_way_through()?;
        let Ok(listing) = folder();
        let Ok(after) = run_events(&was, &[ViewerEvent::Listed {
            listing,
            at: Since::from_secs(9),
        }]);
        let Ok(name) = current_name(&after.state);

        assert_eq!(name, "holiday.mp4");
        assert_eq!(after.state.along, was.along);
        assert_eq!(after.state.captions, was.captions);

        Ok(())
    }

    #[test]
    fn a_thing_that_has_gone_leaves_nothing_of_itself_on_the_next_one() -> Result<(), Failure> {
        let was = a_film_part_way_through()?;
        let Ok(folder) = folder();
        let gone: Vec<(String, String)> =
            folder.into_iter().filter(|(name, _)| name != "holiday.mp4").collect();

        let Ok(after) = run_events(&was, &[ViewerEvent::Listed { listing: gone, at: Since::from_secs(9) }]);
        let Ok(name) = current_name(&after.state);

        assert_ne!(name, "holiday.mp4");
        assert_eq!(after.state.along, Along::default());
        assert_eq!(after.state.captions, Captions::default());

        Ok(())
    }

    #[test]
    fn the_card_goes_quiet_and_any_press_wakes_it() -> Result<(), Failure> {
        let quiet = waking::QUIET.saturating_add(Since::from_secs(1));
        let watching = sample_watching()?;

        assert_eq!(awake(&watching, quiet), Ok(Woken::No));
        assert_eq!(stirred(&watching, quiet), Ok(WakeOutcome::Woke));
        assert_eq!(stirred(&watching, Since::from_secs(1)), Ok(WakeOutcome::AlreadyAwake));

        let Ok(after) = run_events(&watching, &[ViewerEvent::WakeOutcome(quiet)]);

        assert_eq!(awake(&after.state, quiet), Ok(Woken::Yes));

        Ok(())
    }

    #[test]
    fn one_from_the_media_page_is_handed_to_the_files_panel() -> Result<(), Failure> {
        let at = std::path::Path::new("/home/someone/Pictures/beach.jpg");
        let watching = sample_watching()?;
        let Ok(after) = run_events(&watching, &[ViewerEvent::Shown(at.to_path_buf())]);
        let Ok(files) = Command::internal(FILES, &[&at.to_string_lossy()]);
        let Ok(name) = current_name(&after.state);

        assert_eq!(after.effects(), Ok(vec![Effect::Spawn(files)]));
        assert_eq!(name, "holiday.mp4", "it stands where it stood");

        Ok(())
    }

    #[test]
    fn where_the_film_is_does_not_wake_the_card() -> Result<(), Failure> {
        let quiet = waking::QUIET.saturating_add(Since::from_secs(1));
        let watching = sample_watching()?;
        let Ok(after) = run_events(&watching, &[ViewerEvent::Where { at: 100, whole: 600 }]);

        assert_eq!(awake(&after.state, quiet), Ok(Woken::No));

        Ok(())
    }
}
