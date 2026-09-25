//! Where each tab is standing, what it is carrying, and what a press decides.
//!
//! One tab per place -- home, the folders under it, whatever is plugged in --
//! and each one walks on its own. Turning to another tab and back is not a
//! reset: the folder you were in, the row you were on and what you had typed
//! are all still there, which is why every field here is a list with one entry
//! to a tab.
//!
//! What is carried is not. A thing picked up in one tab is put down in another
//! -- that is most of what carrying is for -- so `holding` is one, and the row
//! offering to put it down appears in whichever folder is in front of you.
//!
//! Going up puts the thumb back on the folder you came out of, which `Walk`
//! remembers, and the row that lands on depends on what else is drawn above
//! the things: the line, the way up if there is one, and the thing being
//! carried if there is one. That arithmetic is `first_thing`, and it is the
//! reason a panel that grew a row at the top once put every thumb one row
//! wrong.
//!
//! The disk is not read here. Standing on a thing by name needs to know where
//! that name is in the folder, so the folder's names are handed in.

use std::path::PathBuf;

use console_core_never::Never;
use console_program_contract::{Arguments, Effect, Initial, Program, Update, Event};

use crate::doing::Holding;
use crate::listing::Entry;
use crate::places::Place;
use crate::walk::{self, Walk};

const NOTHING_TYPED: &str = "";

const THE_FIRST_ROW: u32 = 0;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line(pub u32);

pub const LINE: u32 = 1;

pub const HERE_START: u32 = 1;

pub const WAYS_START: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    Folder,
    Here { from: u32 },
    Programs { thing: Entry, from: u32 },
    Ways { thing: Entry, from: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    pub holding: Option<Holding>,
    pub onto: Vec<Destination>,
    pub places: Vec<Place>,
    pub stand_on: Option<(u32, String)>,
    pub typed: Vec<String>,
    pub walks: Vec<Walk>,
}

impl Standing {
    pub fn of(places: Vec<Place>) -> Result<Self, Never> {
        let mut walks: Vec<Walk> = Vec::new();

        for place in &places {
            let walk = Walk::of(&place.path)?;

            walks.push(walk);
        }

        let onto = places.iter().map(|_| Destination::Folder).collect();
        let typed = places.iter().map(|_| String::new()).collect();

        Ok(Standing { holding: None, onto, places, stand_on: None, typed, walks })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesEvent {
    Typed { tab: u32, word: String },
    LookingFor { tab: u32, word: String },
    Opened { tab: u32, onto: Destination, row: u32 },
    Entered { tab: u32, name: String, at: u32 },
    Walked { tab: u32, steps: Vec<String> },
    Up { tab: u32 },
    PickedUp(Holding),
    PutDown,
    Arrived { tab: u32, names: Vec<String> },
    Back { tab: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesEffect {
    Replace(u32),
    WantingPictures(PathBuf),
    ForgetTyping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closes {
    Yes,
    No,
}

pub struct Files;

impl Program for Files {
    type State = Standing;
    type Event = FilesEvent;
    type Effect = FilesEffect;

    fn init(_argv: &Arguments) -> Initial<Standing> {
        let Ok(standing) = Standing::of(Vec::new());
        let Ok(opening) = Initial::new(standing);

        opening
    }

    fn update(state: &Standing, event: &Event<FilesEvent>) -> Update<Standing, FilesEffect> {
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
            FilesEvent::Typed { tab, word } => {
                let Ok(typed) = typed(state, *tab);

                let Ok(with) = with_typed(state, *tab, word);

                match typed == *word {
                    true => Update::none(state.clone()),
                    false => Update::new(with, vec![Effect::Custom(FilesEffect::Replace(0))]),
                }
            }

            FilesEvent::LookingFor { tab, word } => {
                let Ok(with) = with_typed(state, *tab, word);

                Update::none(with)
            }

            FilesEvent::Opened { tab, onto, row } => {
                let Ok(with) = with_onto(state, *tab, onto.clone());

                Update::new(with, vec![Effect::Custom(FilesEffect::Replace(*row))])
            }

            FilesEvent::Entered { tab, name, at } => {
                let Ok(row) = first_thing(state, *tab);

                let Ok(held) = walked(state, *tab, std::slice::from_ref(name), Line(*at));

                let Ok(here) = here(&held, *tab);

                Update::new(held, vec![
                    Effect::Custom(FilesEffect::Replace(row)),
                    Effect::Custom(FilesEffect::WantingPictures(here)),
                ])
            }

            FilesEvent::Walked { tab, steps } => {
                let Ok(row) = first_thing(state, *tab);

                let Ok(held) = walked(state, *tab, steps, Line(row));

                let Ok(here) = here(&held, *tab);

                Update::new(held, vec![
                    Effect::Custom(FilesEffect::Replace(row)),
                    Effect::Custom(FilesEffect::WantingPictures(here)),
                ])
            }

            FilesEvent::Up { tab } => went_up(state, *tab),

            FilesEvent::PickedUp(holding) => {
                Update::none(Standing { holding: Some(holding.clone()), ..state.clone() })
            }

            FilesEvent::PutDown => Update::new(
                Standing { holding: None, ..state.clone() },
                vec![Effect::Custom(FilesEffect::Replace(LINE))],
            ),

            FilesEvent::Arrived { tab, names } => arrived(state, *tab, names),

            FilesEvent::Back { tab } => back(state, *tab),
        };

        turn
    }
}

fn back(state: &Standing, tab: u32) -> Result<Update<Standing, FilesEffect>, Never> {
    let word = typed(state, tab)?;
    let onto = onto(state, tab)?;

    match (onto, word.trim().is_empty()) {
        (Destination::Folder, false) => {
            let with = with_typed(state, tab, "")?;

            Update::new(with, vec![
                Effect::Custom(FilesEffect::ForgetTyping),
                Effect::Custom(FilesEffect::Replace(LINE)),
            ])
        }

        (Destination::Folder, true) => {
            let at_top = at_top(state, tab)?;

            match at_top {
                Top::Yes => Update::none(state.clone()),
                Top::No => went_up(state, tab),
            }
        }

        (Destination::Here { from }, _) | (Destination::Ways { from, .. }, _) => {
            let with = with_onto(state, tab, Destination::Folder)?;

            Update::new(with, vec![Effect::Custom(FilesEffect::Replace(from))])
        }

        (Destination::Programs { thing, from }, _) => {
            let with = with_onto(state, tab, Destination::Ways { thing, from })?;

            Update::new(with, vec![Effect::Custom(FilesEffect::Replace(WAYS_START))])
        }
    }
}

fn went_up(state: &Standing, tab: u32) -> Result<Update<Standing, FilesEffect>, Never> {
    let mut walks = state.walks.clone();

    let Ok(slot) = console_core_number_conversion::index(tab);

    let walk = match walks.get_mut(slot) {
        Some(walk) => walk,
        None => return Update::none(state.clone()),
    };

    let back_to = walk.up()?;
    let held = Standing { walks, ..state.clone() };
    let here = here(&held, tab)?;

    match back_to {
        Some(back_to) => Update::new(held, vec![
            Effect::Custom(FilesEffect::Replace(back_to)),
            Effect::Custom(FilesEffect::WantingPictures(here)),
        ]),
        None => Update::none(held),
    }
}

fn arrived(state: &Standing, tab: u32, names: &[String]) -> Result<Update<Standing, FilesEffect>, Never> {
    let asked = state.stand_on.clone();

    match asked {
        Some((at, name)) => match at == tab {
            true => {
                let row = row_of(state, tab, &name, names)?;

                Update::new(
                    Standing { stand_on: None, ..state.clone() },
                    vec![Effect::Custom(FilesEffect::Replace(row))],
                )
            }
            false => Update::none(state.clone()),
        },
        None => Update::none(state.clone()),
    }
}

fn walked(
    state: &Standing,
    tab: u32,
    steps: &[String],
    from: Line,
) -> Result<Standing, Never> {
    let mut walks = state.walks.clone();

    let Ok(slot) = console_core_number_conversion::index(tab);

    let walk = match walks.get_mut(slot) {
        Some(walk) => walk,
        None => return Ok(state.clone()),
    };

    for step in steps {
        walk.enter(step, from.0)?;
    }

    Ok(Standing { walks, ..state.clone() })
}

fn with_typed(state: &Standing, tab: u32, word: &str) -> Result<Standing, Never> {
    let Ok(slot) = console_core_number_conversion::index(tab);

    let typed = state
        .typed
        .iter()
        .enumerate()
        .map(|(at, was)| match at == slot {
            true => word.to_string(),
            false => was.clone(),
        })
        .collect();

    Ok(Standing { typed, ..state.clone() })
}

fn with_onto(state: &Standing, tab: u32, onto: Destination) -> Result<Standing, Never> {
    let Ok(slot) = console_core_number_conversion::index(tab);

    let every = state
        .onto
        .iter()
        .enumerate()
        .map(|(at, was)| match at == slot {
            true => onto.clone(),
            false => was.clone(),
        })
        .collect();

    Ok(Standing { onto: every, ..state.clone() })
}

pub fn here(state: &Standing, tab: u32) -> Result<PathBuf, Never> {
    let Ok(slot) = console_core_number_conversion::index(tab);

    let walk = match state.walks.get(slot) {
        Some(walk) => walk,
        None => return Ok(PathBuf::new()),
    };

    let here = walk.here()?;

    Ok(here.to_path_buf())
}

pub fn onto(state: &Standing, tab: u32) -> Result<Destination, Never> {
    let Ok(slot) = console_core_number_conversion::index(tab);

    Ok(match state.onto.get(slot).cloned() {
        Some(onto) => onto,
        None => Destination::Folder,
    })
}

pub fn typed(state: &Standing, tab: u32) -> Result<String, Never> {
    let Ok(slot) = console_core_number_conversion::index(tab);

    Ok(match state.typed.get(slot).cloned() {
        Some(typed) => typed,
        None => NOTHING_TYPED.to_string(),
    })
}

fn opened(state: &Standing, tab: u32) -> Result<Option<(&Walk, &Place)>, Never> {
    let Ok(slot) = console_core_number_conversion::index(tab);

    Ok(state.walks.get(slot).zip(state.places.get(slot)))
}

pub fn called(state: &Standing, tab: u32) -> Result<String, Never> {
    let Ok(opened) = opened(state, tab);

    match opened {
        Some((walk, place)) => walk.called(&place.title),
        None => Ok(String::new()),
    }
}

pub fn above(state: &Standing, tab: u32) -> Result<Option<String>, Never> {
    let Ok(opened) = opened(state, tab);

    match opened {
        Some((walk, place)) => walk.above(&place.title),
        None => Ok(None),
    }
}

pub fn titles(state: &Standing) -> Result<Vec<String>, Never> {
    Ok(state.places.iter().map(|place| place.title.clone()).collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Top {
    Yes,
    No,
}

pub fn at_top(state: &Standing, tab: u32) -> Result<Top, Never> {
    let Ok(slot) = console_core_number_conversion::index(tab);

    let walk = match state.walks.get(slot) {
        Some(walk) => walk,
        None => return Ok(Top::Yes),
    };

    let at_top = walk.at_top()?;

    Ok(match at_top {
        walk::Top::Yes => Top::Yes,
        walk::Top::No => Top::No,
    })
}

pub fn first_thing(state: &Standing, tab: u32) -> Result<u32, Never> {
    let above = above(state, tab)?;
    let below_top = above.is_some();

    Ok(LINE
        .saturating_add(2u32.saturating_mul(u32::from(below_top)))
        .saturating_add(u32::from(state.holding.is_some())))
}

pub fn row_of(
    state: &Standing,
    tab: u32,
    name: &str,
    names: &[String],
) -> Result<u32, Never> {
    let first = first_thing(state, tab)?;
    let Ok(at) = match names.iter().position(|held| held == name) {
        Some(at) => console_core_number_conversion::fitted::<_, u32>(at),
        None => Ok(THE_FIRST_ROW),
    };

    Ok(first.saturating_add(at))
}

pub fn closes(state: &Standing, tab: u32) -> Result<Closes, Never> {
    let onto = onto(state, tab)?;
    let typed = typed(state, tab)?;

    match (onto, typed.trim().is_empty()) {
        (Destination::Folder, true) => {
            let at_top = at_top(state, tab)?;

            Ok(match at_top {
                Top::Yes => Closes::Yes,
                Top::No => Closes::No,
            })
        }
        (Destination::Folder, false)
        | (Destination::Here { .. }, _)
        | (Destination::Programs { .. }, _)
        | (Destination::Ways { .. }, _) => Ok(Closes::No),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use console_program_contract::{Trace, run_from};

    use crate::doing::Carrying;

    use super::*;

    fn places() -> Vec<Place> {
        vec![
            Place { title: "Home".to_string(), path: PathBuf::from("/home/someone") },
            Place { title: "Stick".to_string(), path: PathBuf::from("/run/media/stick") },
        ]
    }

    fn standing() -> Standing {
        let Ok(standing) = Standing::of(places());

        standing
    }

    fn at(state: &Standing, tab: u32) -> PathBuf {
        let Ok(here) = here(state, tab);

        here
    }

    fn word(state: &Standing, tab: u32) -> String {
        let Ok(typed) = typed(state, tab);

        typed
    }

    fn first(state: &Standing, tab: u32) -> u32 {
        let Ok(first) = first_thing(state, tab);

        first
    }

    fn said(from: &Standing, heard: &[FilesEvent]) -> Trace<Standing, FilesEvent, FilesEffect> {
        let events: Vec<Event<FilesEvent>> = heard.iter().cloned().map(Event::Custom).collect();

        let Ok(said) = run_from::<Files>(from, &events);

        said
    }

    fn thing(name: &str) -> Entry {
        Entry { name: name.to_string(), ..Entry::default() }
    }

    #[test]
    fn each_tab_walks_on_its_own() {
        let after = said(&standing(), &[
            FilesEvent::Entered { tab: 0, name: "Music".to_string(), at: 3 },
            FilesEvent::Entered { tab: 1, name: "DCIM".to_string(), at: 3 },
        ]);

        assert_eq!(at(&after.state, 0), Path::new("/home/someone/Music"));
        assert_eq!(at(&after.state, 1), Path::new("/run/media/stick/DCIM"));
    }

    #[test]
    fn what_is_typed_in_one_tab_is_still_there_after_the_other_one() {
        let after = said(&standing(), &[
            FilesEvent::Typed { tab: 0, word: "beach".to_string() },
            FilesEvent::Typed { tab: 1, word: "holiday".to_string() },
        ]);

        assert_eq!(word(&after.state, 0), "beach");
        assert_eq!(word(&after.state, 1), "holiday");
    }

    #[test]
    fn going_up_puts_the_thumb_back_on_the_folder_it_came_out_of() {
        let down = said(&standing(), &[FilesEvent::Entered { tab: 0, name: "Music".to_string(), at: 7 }]);
        let up = said(&down.state, &[FilesEvent::Up { tab: 0 }]);

        assert_eq!(at(&up.state, 0), Path::new("/home/someone"));
        let Ok(effects) = up.effects();

        assert_eq!(effects.first(), Some(&Effect::Custom(FilesEffect::Replace(7))));
    }

    #[test]
    fn the_top_of_a_place_is_where_back_leaves_the_panel() {
        assert_eq!(closes(&standing(), 0), Ok(Closes::Yes));

        let down = said(&standing(), &[FilesEvent::Entered { tab: 0, name: "Music".to_string(), at: 3 }]);

        assert_eq!(closes(&down.state, 0), Ok(Closes::No));
    }

    #[test]
    fn back_out_of_a_thing_lands_on_the_row_it_was_opened_from() {
        let opened = said(&standing(), &[FilesEvent::Opened {
            tab: 0,
            onto: Destination::Ways { thing: thing("beach.jpg"), from: 5 },
            row: WAYS_START,
        }]);

        assert_eq!(opened.effects(), Ok(vec![Effect::Custom(FilesEffect::Replace(WAYS_START))]));

        let out = said(&opened.state, &[FilesEvent::Back { tab: 0 }]);

        assert_eq!(onto(&out.state, 0), Ok(Destination::Folder));
        assert_eq!(out.effects(), Ok(vec![Effect::Custom(FilesEffect::Replace(5))]));
    }

    #[test]
    fn back_out_of_the_programs_goes_to_the_ways_and_not_to_the_folder() {
        let opened = said(&standing(), &[FilesEvent::Opened {
            tab: 0,
            onto: Destination::Programs { thing: thing("beach.jpg"), from: 5 },
            row: 0,
        }]);

        let out = said(&opened.state, &[FilesEvent::Back { tab: 0 }]);

        assert_eq!(onto(&out.state, 0), Ok(Destination::Ways { thing: thing("beach.jpg"), from: 5 }));
    }

    #[test]
    fn what_is_carried_is_carried_between_tabs() {
        let holding = Holding {
            name: "song.opus".to_string(),
            paths: vec![PathBuf::from("/home/someone/Music/song.opus")],
            moving: Carrying::ToMove,
        };
        let after = said(&standing(), &[FilesEvent::PickedUp(holding.clone())]);

        assert_eq!(after.state.holding, Some(holding));

        let down = said(&after.state, &[FilesEvent::PutDown]);

        assert_eq!(down.state.holding, None);
        assert_eq!(down.effects(), Ok(vec![Effect::Custom(FilesEffect::Replace(LINE))]));
    }

    #[test]
    fn the_first_thing_moves_down_for_the_way_up_and_for_what_is_carried() {
        let top = standing();

        assert_eq!(first(&top, 0), LINE);

        let down = said(&top, &[FilesEvent::Entered { tab: 0, name: "Music".to_string(), at: 3 }]);

        assert_eq!(first(&down.state, 0), LINE + 2);

        let carrying = said(&down.state, &[FilesEvent::PickedUp(Holding {
            name: "else".to_string(),
            paths: vec![PathBuf::from("/somewhere/else")],
            moving: Carrying::ToCopy,
        })]);

        assert_eq!(first(&carrying.state, 0), LINE + 3);
    }

    #[test]
    fn a_panel_opened_on_a_thing_stands_on_it_once_and_then_forgets() {
        let asked = Standing {
            stand_on: Some((0, "song.opus".to_string())),
            ..said(&standing(), &[FilesEvent::Entered { tab: 0, name: "Music".to_string(), at: 3 }]).state
        };
        let names = vec!["another.opus".to_string(), "song.opus".to_string()];

        let after = said(&asked, &[FilesEvent::Arrived { tab: 0, names: names.clone() }]);

        assert_eq!(after.effects(), Ok(vec![Effect::Custom(FilesEffect::Replace(first(&asked, 0) + 1))]));
        assert_eq!(after.state.stand_on, None);

        let again = said(&after.state, &[FilesEvent::Arrived { tab: 0, names }]);

        let Ok(effects) = again.effects();

        assert!(effects.is_empty());
    }

    #[test]
    fn arriving_in_a_tab_no_one_asked_for_stands_nowhere_and_keeps_the_asking() {
        let asked = Standing { stand_on: Some((1, "song.opus".to_string())), ..standing() };
        let after = said(&asked, &[FilesEvent::Arrived { tab: 0, names: Vec::new() }]);

        let Ok(effects) = after.effects();

        assert!(effects.is_empty());
        assert_eq!(after.state.stand_on, asked.stand_on);
    }
}
