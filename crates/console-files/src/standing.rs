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
use console_program_contract::{Argv, Doing, Opening, Program, Turn, Word};

use crate::doing::Holding;
use crate::listing::Entry;
use crate::places::Place;
use crate::walk::{self, Walk};

pub const LINE: usize = 1;

pub const HERE_START: usize = 1;

pub const WAYS_START: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Onto {
    Folder,
    Here { from: usize },
    Programs { thing: Entry, from: usize },
    Ways { thing: Entry, from: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    pub holding: Option<Holding>,
    pub onto: Vec<Onto>,
    pub places: Vec<Place>,
    pub stand_on: Option<(usize, String)>,
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

        let onto = places.iter().map(|_| Onto::Folder).collect();
        let typed = places.iter().map(|_| String::new()).collect();

        Ok(Standing { holding: None, onto, places, stand_on: None, typed, walks })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    Typed { tab: usize, word: String },
    LookingFor { tab: usize, word: String },
    Opened { tab: usize, onto: Onto, row: usize },
    Entered { tab: usize, name: String, at: usize },
    Walked { tab: usize, steps: Vec<String> },
    Up { tab: usize },
    Held(Holding),
    PutDown,
    Arrived { tab: usize, names: Vec<String> },
    Back { tab: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Its {
    Replace(usize),
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
    type Hears = Heard;
    type Does = Its;

    fn opening(_argv: &Argv) -> Opening<Standing> {
        let Ok(standing) = Standing::of(Vec::new());
        let Ok(opening) = Opening::holding(standing);

        opening
    }

    fn heard(state: &Standing, word: &Word<Heard>) -> Turn<Standing, Its> {
        let Word::Its(heard) = word else {
            let Ok(nothing) = Turn::nothing(state.clone());

            return nothing;
        };

        let Ok(turn) = match heard {
            Heard::Typed { tab, word } => {
                let Ok(typed) = typed(state, *tab);

                let Ok(with) = with_typed(state, *tab, word);

                match typed == *word {
                    true => Turn::nothing(state.clone()),
                    false => Turn::doing(with, vec![Doing::Its(Its::Replace(0))]),
                }
            }

            Heard::LookingFor { tab, word } => {
                let Ok(with) = with_typed(state, *tab, word);

                Turn::nothing(with)
            }

            Heard::Opened { tab, onto, row } => {
                let Ok(with) = with_onto(state, *tab, onto.clone());

                Turn::doing(with, vec![Doing::Its(Its::Replace(*row))])
            }

            Heard::Entered { tab, name, at } => {
                let Ok(row) = first_thing(state, *tab);

                let Ok(held) = walked(state, *tab, std::slice::from_ref(name), *at);

                let Ok(here) = here(&held, *tab);

                Turn::doing(held, vec![
                    Doing::Its(Its::Replace(row)),
                    Doing::Its(Its::WantingPictures(here)),
                ])
            }

            Heard::Walked { tab, steps } => {
                let Ok(row) = first_thing(state, *tab);

                let Ok(held) = walked(state, *tab, steps, row);

                let Ok(here) = here(&held, *tab);

                Turn::doing(held, vec![
                    Doing::Its(Its::Replace(row)),
                    Doing::Its(Its::WantingPictures(here)),
                ])
            }

            Heard::Up { tab } => went_up(state, *tab),

            Heard::Held(holding) => {
                Turn::nothing(Standing { holding: Some(holding.clone()), ..state.clone() })
            }

            Heard::PutDown => Turn::doing(
                Standing { holding: None, ..state.clone() },
                vec![Doing::Its(Its::Replace(LINE))],
            ),

            Heard::Arrived { tab, names } => arrived(state, *tab, names),

            Heard::Back { tab } => back(state, *tab),
        };

        turn
    }
}

fn back(state: &Standing, tab: usize) -> Result<Turn<Standing, Its>, Never> {
    let word = typed(state, tab)?;
    let onto = onto(state, tab)?;

    match (onto, word.trim().is_empty()) {
        (Onto::Folder, false) => {
            let with = with_typed(state, tab, "")?;

            Turn::doing(with, vec![
                Doing::Its(Its::ForgetTyping),
                Doing::Its(Its::Replace(LINE)),
            ])
        }

        (Onto::Folder, true) => {
            let at_top = at_top(state, tab)?;

            match at_top {
                Top::Yes => Turn::nothing(state.clone()),
                Top::No => went_up(state, tab),
            }
        }

        (Onto::Here { from }, _) | (Onto::Ways { from, .. }, _) => {
            let with = with_onto(state, tab, Onto::Folder)?;

            Turn::doing(with, vec![Doing::Its(Its::Replace(from))])
        }

        (Onto::Programs { thing, from }, _) => {
            let with = with_onto(state, tab, Onto::Ways { thing, from })?;

            Turn::doing(with, vec![Doing::Its(Its::Replace(WAYS_START))])
        }
    }
}

fn went_up(state: &Standing, tab: usize) -> Result<Turn<Standing, Its>, Never> {
    let mut walks = state.walks.clone();

    let Some(walk) = walks.get_mut(tab) else {
        return Turn::nothing(state.clone());
    };

    let back_to = walk.up()?;
    let held = Standing { walks, ..state.clone() };
    let here = here(&held, tab)?;

    match back_to {
        Some(back_to) => Turn::doing(held, vec![
            Doing::Its(Its::Replace(back_to)),
            Doing::Its(Its::WantingPictures(here)),
        ]),
        None => Turn::nothing(held),
    }
}

fn arrived(state: &Standing, tab: usize, names: &[String]) -> Result<Turn<Standing, Its>, Never> {
    let asked = state.stand_on.clone();

    match asked {
        Some((at, name)) if at == tab => {
            let row = row_of(state, tab, &name, names)?;

            Turn::doing(
                Standing { stand_on: None, ..state.clone() },
                vec![Doing::Its(Its::Replace(row))],
            )
        }
        Some(_) | None => Turn::nothing(state.clone()),
    }
}

fn walked(
    state: &Standing,
    tab: usize,
    steps: &[String],
    from: usize,
) -> Result<Standing, Never> {
    let mut walks = state.walks.clone();

    let Some(walk) = walks.get_mut(tab) else {
        return Ok(state.clone());
    };

    for step in steps {
        walk.enter(step, from)?;
    }

    Ok(Standing { walks, ..state.clone() })
}

fn with_typed(state: &Standing, tab: usize, word: &str) -> Result<Standing, Never> {
    let typed = state
        .typed
        .iter()
        .enumerate()
        .map(|(at, was)| match at == tab {
            true => word.to_string(),
            false => was.clone(),
        })
        .collect();

    Ok(Standing { typed, ..state.clone() })
}

fn with_onto(state: &Standing, tab: usize, onto: Onto) -> Result<Standing, Never> {
    let every = state
        .onto
        .iter()
        .enumerate()
        .map(|(at, was)| match at == tab {
            true => onto.clone(),
            false => was.clone(),
        })
        .collect();

    Ok(Standing { onto: every, ..state.clone() })
}

pub fn here(state: &Standing, tab: usize) -> Result<PathBuf, Never> {
    let Some(walk) = state.walks.get(tab) else { return Ok(PathBuf::new()) };

    let here = walk.here()?;

    Ok(here.to_path_buf())
}

pub fn onto(state: &Standing, tab: usize) -> Result<Onto, Never> {
    Ok(state.onto.get(tab).cloned().unwrap_or(Onto::Folder))
}

pub fn typed(state: &Standing, tab: usize) -> Result<String, Never> {
    Ok(state.typed.get(tab).cloned().unwrap_or_default())
}

pub fn called(state: &Standing, tab: usize) -> Result<String, Never> {
    let (Some(walk), Some(place)) = (state.walks.get(tab), state.places.get(tab)) else {
        return Ok(String::new());
    };

    walk.called(&place.title)
}

pub fn above(state: &Standing, tab: usize) -> Result<Option<String>, Never> {
    let (Some(walk), Some(place)) = (state.walks.get(tab), state.places.get(tab)) else {
        return Ok(None);
    };

    walk.above(&place.title)
}

pub fn titles(state: &Standing) -> Result<Vec<String>, Never> {
    Ok(state.places.iter().map(|place| place.title.clone()).collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Top {
    Yes,
    No,
}

pub fn at_top(state: &Standing, tab: usize) -> Result<Top, Never> {
    let Some(walk) = state.walks.get(tab) else { return Ok(Top::Yes) };

    let at_top = walk.at_top()?;

    Ok(match at_top {
        walk::Top::Yes => Top::Yes,
        walk::Top::No => Top::No,
    })
}

pub fn first_thing(state: &Standing, tab: usize) -> Result<usize, Never> {
    let above = above(state, tab)?;
    let below_top = above.is_some();

    Ok(LINE
        .saturating_add(2usize.saturating_mul(usize::from(below_top)))
        .saturating_add(usize::from(state.holding.is_some())))
}

pub fn row_of(
    state: &Standing,
    tab: usize,
    name: &str,
    names: &[String],
) -> Result<usize, Never> {
    let first = first_thing(state, tab)?;
    let at = names.iter().position(|held| held == name);

    Ok(first.saturating_add(at.unwrap_or_default()))
}

pub fn closes(state: &Standing, tab: usize) -> Result<Closes, Never> {
    let onto = onto(state, tab)?;
    let typed = typed(state, tab)?;

    match (onto, typed.trim().is_empty()) {
        (Onto::Folder, true) => {
            let at_top = at_top(state, tab)?;

            Ok(match at_top {
                Top::Yes => Closes::Yes,
                Top::No => Closes::No,
            })
        }
        (Onto::Folder, false)
        | (Onto::Here { .. }, _)
        | (Onto::Programs { .. }, _)
        | (Onto::Ways { .. }, _) => Ok(Closes::No),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use console_program_contract::{Said, walk as walked_through};

    use crate::doing::Carrying;

    use super::*;

    fn places() -> Vec<Place> {
        vec![
            Place { title: "Home".to_string(), path: PathBuf::from("/home/somebody") },
            Place { title: "Stick".to_string(), path: PathBuf::from("/run/media/stick") },
        ]
    }

    fn standing() -> Standing {
        let Ok(standing) = Standing::of(places());

        standing
    }

    fn at(state: &Standing, tab: usize) -> PathBuf {
        let Ok(here) = here(state, tab);

        here
    }

    fn word(state: &Standing, tab: usize) -> String {
        let Ok(typed) = typed(state, tab);

        typed
    }

    fn first(state: &Standing, tab: usize) -> usize {
        let Ok(first) = first_thing(state, tab);

        first
    }

    fn said(from: &Standing, heard: &[Heard]) -> Said<Standing, Heard, Its> {
        let words: Vec<Word<Heard>> = heard.iter().cloned().map(Word::Its).collect();

        let Ok(said) = walked_through::<Files>(from, &words);

        said
    }

    fn thing(name: &str) -> Entry {
        Entry { name: name.to_string(), ..Entry::default() }
    }

    #[test]
    fn each_tab_walks_on_its_own() {
        let after = said(&standing(), &[
            Heard::Entered { tab: 0, name: "Music".to_string(), at: 3 },
            Heard::Entered { tab: 1, name: "DCIM".to_string(), at: 3 },
        ]);

        assert_eq!(at(&after.now, 0), Path::new("/home/somebody/Music"));
        assert_eq!(at(&after.now, 1), Path::new("/run/media/stick/DCIM"));
    }

    #[test]
    fn what_is_typed_in_one_tab_is_still_there_after_the_other_one() {
        let after = said(&standing(), &[
            Heard::Typed { tab: 0, word: "beach".to_string() },
            Heard::Typed { tab: 1, word: "holiday".to_string() },
        ]);

        assert_eq!(word(&after.now, 0), "beach");
        assert_eq!(word(&after.now, 1), "holiday");
    }

    #[test]
    fn going_up_puts_the_thumb_back_on_the_folder_it_came_out_of() {
        let down = said(&standing(), &[Heard::Entered { tab: 0, name: "Music".to_string(), at: 7 }]);
        let up = said(&down.now, &[Heard::Up { tab: 0 }]);

        assert_eq!(at(&up.now, 0), Path::new("/home/somebody"));
        let Ok(doings) = up.doings();

        assert_eq!(doings.first(), Some(&Doing::Its(Its::Replace(7))));
    }

    #[test]
    fn the_top_of_a_place_is_where_back_leaves_the_panel() {
        assert_eq!(closes(&standing(), 0), Ok(Closes::Yes));

        let down = said(&standing(), &[Heard::Entered { tab: 0, name: "Music".to_string(), at: 3 }]);

        assert_eq!(closes(&down.now, 0), Ok(Closes::No));
    }

    #[test]
    fn back_out_of_a_thing_lands_on_the_row_it_was_opened_from() {
        let opened = said(&standing(), &[Heard::Opened {
            tab: 0,
            onto: Onto::Ways { thing: thing("beach.jpg"), from: 5 },
            row: WAYS_START,
        }]);

        assert_eq!(opened.doings(), Ok(vec![Doing::Its(Its::Replace(WAYS_START))]));

        let out = said(&opened.now, &[Heard::Back { tab: 0 }]);

        assert_eq!(onto(&out.now, 0), Ok(Onto::Folder));
        assert_eq!(out.doings(), Ok(vec![Doing::Its(Its::Replace(5))]));
    }

    #[test]
    fn back_out_of_the_programs_goes_to_the_ways_and_not_to_the_folder() {
        let opened = said(&standing(), &[Heard::Opened {
            tab: 0,
            onto: Onto::Programs { thing: thing("beach.jpg"), from: 5 },
            row: 0,
        }]);

        let out = said(&opened.now, &[Heard::Back { tab: 0 }]);

        assert_eq!(onto(&out.now, 0), Ok(Onto::Ways { thing: thing("beach.jpg"), from: 5 }));
    }

    #[test]
    fn what_is_carried_is_carried_between_tabs() {
        let holding = Holding {
            name: "song.opus".to_string(),
            path: PathBuf::from("/home/somebody/Music/song.opus"),
            moving: Carrying::ToMove,
        };
        let after = said(&standing(), &[Heard::Held(holding.clone())]);

        assert_eq!(after.now.holding, Some(holding));

        let down = said(&after.now, &[Heard::PutDown]);

        assert_eq!(down.now.holding, None);
        assert_eq!(down.doings(), Ok(vec![Doing::Its(Its::Replace(LINE))]));
    }

    #[test]
    fn the_first_thing_moves_down_for_the_way_up_and_for_what_is_carried() {
        let top = standing();

        assert_eq!(first(&top, 0), LINE);

        let down = said(&top, &[Heard::Entered { tab: 0, name: "Music".to_string(), at: 3 }]);

        assert_eq!(first(&down.now, 0), LINE + 2);

        let carrying = said(&down.now, &[Heard::Held(Holding {
            name: "else".to_string(),
            path: PathBuf::from("/somewhere/else"),
            moving: Carrying::ToCopy,
        })]);

        assert_eq!(first(&carrying.now, 0), LINE + 3);
    }

    #[test]
    fn a_panel_opened_on_a_thing_stands_on_it_once_and_then_forgets() {
        let asked = Standing {
            stand_on: Some((0, "song.opus".to_string())),
            ..said(&standing(), &[Heard::Entered { tab: 0, name: "Music".to_string(), at: 3 }]).now
        };
        let names = vec!["another.opus".to_string(), "song.opus".to_string()];

        let after = said(&asked, &[Heard::Arrived { tab: 0, names: names.clone() }]);

        assert_eq!(after.doings(), Ok(vec![Doing::Its(Its::Replace(first(&asked, 0) + 1))]));
        assert_eq!(after.now.stand_on, None);

        let again = said(&after.now, &[Heard::Arrived { tab: 0, names }]);

        let Ok(doings) = again.doings();

        assert!(doings.is_empty());
    }

    #[test]
    fn arriving_in_a_tab_nobody_asked_for_stands_nowhere_and_keeps_the_asking() {
        let asked = Standing { stand_on: Some((1, "song.opus".to_string())), ..standing() };
        let after = said(&asked, &[Heard::Arrived { tab: 0, names: Vec::new() }]);

        let Ok(doings) = after.doings();

        assert!(doings.is_empty());
        assert_eq!(after.now.stand_on, asked.stand_on);
    }
}
