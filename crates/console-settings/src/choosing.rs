//! Where the Configuration tab is standing, and how the home grid steps.
//!
//! Eight of the nine tabs here hold nothing between one drawing and the next:
//! they ask the machine what the sound, the radios and the battery are doing
//! and draw the answer. The ninth goes deeper -- into which browser search,
//! which language dictation, and what opens each kind of file -- and the rule
//! worth writing down is where B lands. Backing out of a choice puts the thumb
//! on the row that opened it rather than at the top, because the row that
//! opened it is what somebody was looking at.
//!
//! The home grid is here for a different reason. Stepping it is arithmetic on
//! somebody's screen -- one column fewer, one size up the ladder -- and it was
//! written twice inside two closures with no test on either. The file it ends
//! up in is written by the binary, because where somebody's home is is not
//! something this may look up.

use console_home_screen::shape::{self, Shape, Size};
use console_core_never::Never;
use console_program_contract::{Argv, Doing, Opening, Program, Turn, Word};

pub const SEARCH: usize = 0;

pub const DICTATION: usize = 1;

pub const FIRST_KIND: usize = 4;

pub const DEEPER: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Onto {
    Settings,
    Search,
    Dictation,
    Kind(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard {
    Opened(Onto),
    Back,
    Across { shape: Shape, step: i32 },
    Down { shape: Shape, step: i32 },
    Sized { shape: Shape, step: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Its {
    Replace(usize),
    Home(Shape),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closes {
    Yes,
    No,
}

pub struct Settings;

impl Program for Settings {
    type State = Onto;
    type Hears = Heard;
    type Does = Its;

    fn opening(_argv: &Argv) -> Opening<Onto> {
        let Ok(opening) = Opening::holding(Onto::Settings);

        opening
    }

    fn heard(state: &Onto, word: &Word<Heard>) -> Turn<Onto, Its> {
        let Word::Its(heard) = word else {
            let Ok(nothing) = Turn::nothing(*state);

            return nothing;
        };

        let Ok(turn) = match heard {
            Heard::Opened(onto) => Turn::doing(*onto, vec![Doing::Its(Its::Replace(DEEPER))]),

            Heard::Back => {
                let Ok(row) = row_of(*state);

                Turn::doing(Onto::Settings, vec![Doing::Its(Its::Replace(row))])
            }

            Heard::Across { shape, step } => {
                let Ok(columns) = stepped(shape.columns, *step);

                let Ok(across) = shape.across(columns);

                grid(*state, across)
            }

            Heard::Down { shape, step } => {
                let Ok(rows) = stepped(shape.rows, *step);

                let Ok(down) = shape.down(rows);

                grid(*state, down)
            }

            Heard::Sized { shape, step } => {
                let Ok(size) = rung(shape.size, *step);

                let Ok(sized) = shape.sized(size);

                grid(*state, sized)
            }
        };

        turn
    }
}

fn grid(state: Onto, shape: Shape) -> Result<Turn<Onto, Its>, Never> {
    Turn::doing(state, vec![Doing::Its(Its::Home(shape))])
}

pub fn row_of(onto: Onto) -> Result<usize, Never> {
    match onto {
        Onto::Dictation => Ok(DICTATION),
        Onto::Kind(at) => Ok(FIRST_KIND.saturating_add(at)),
        Onto::Settings | Onto::Search => Ok(SEARCH),
    }
}

pub fn closes(onto: Onto) -> Result<Closes, Never> {
    match onto {
        Onto::Settings => Ok(Closes::Yes),
        Onto::Search | Onto::Dictation | Onto::Kind(_) => Ok(Closes::No),
    }
}

fn stepped(now: usize, step: i32) -> Result<usize, Never> {
    match step > 0 {
        true => Ok(now.saturating_add(1)),
        false => Ok(now.saturating_sub(1)),
    }
}

fn rung(now: Size, step: i32) -> Result<Size, Never> {
    let at = shape::EVERY.iter().position(|size| *size == now).unwrap_or(0);

    let went = match step > 0 {
        true => at.saturating_add(1),
        false => at.saturating_sub(1),
    };

    Ok(shape::EVERY.get(went).copied().unwrap_or(now))
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Said, told};

    use super::*;

    fn said(heard: &[Heard]) -> Said<Onto, Heard, Its> {
        let words: Vec<Word<Heard>> = heard.iter().copied().map(Word::Its).collect();

        let Ok(told) = told::<Settings>(&Argv::default(), &words);

        told
    }

    fn doings(said: &Said<Onto, Heard, Its>) -> Vec<Doing<Its>> {
        let Ok(doings) = said.doings();

        doings
    }

    #[test]
    fn backing_out_lands_on_the_row_that_opened_it() {
        let out = said(&[Heard::Opened(Onto::Kind(3)), Heard::Back]);

        assert_eq!(out.now, Onto::Settings);
        assert_eq!(doings(&out).last(), Some(&Doing::Its(Its::Replace(FIRST_KIND + 3))));

        let dictation = said(&[Heard::Opened(Onto::Dictation), Heard::Back]);

        assert_eq!(doings(&dictation).last(), Some(&Doing::Its(Its::Replace(DICTATION))));
    }

    #[test]
    fn opening_one_stands_the_thumb_where_the_choices_start() {
        assert_eq!(doings(&said(&[Heard::Opened(Onto::Search)])), vec![Doing::Its(Its::Replace(
            DEEPER
        ))]);
    }

    #[test]
    fn b_leaves_the_panel_only_from_the_top() {
        assert_eq!(closes(Onto::Settings), Ok(Closes::Yes));
        assert_eq!(closes(Onto::Search), Ok(Closes::No));
        assert_eq!(closes(Onto::Kind(0)), Ok(Closes::No));
    }

    #[test]
    fn the_grid_stops_at_the_narrowest_and_the_shallowest_it_is_allowed() {
        let Ok(narrow) = Shape::USUAL.across(*Shape::COLUMNS.start());

        let Ok(least) = narrow.down(*Shape::ROWS.start());

        assert_eq!(
            doings(&said(&[Heard::Across { shape: least, step: -1 }])),
            vec![Doing::Its(Its::Home(least))]
        );
        assert_eq!(
            doings(&said(&[Heard::Down { shape: least, step: -1 }])),
            vec![Doing::Its(Its::Home(least))]
        );
    }

    #[test]
    fn the_grid_steps_one_at_a_time_whichever_way_it_is_pushed() {
        let usual = Shape::USUAL;

        let Ok(wider) = usual.across(usual.columns + 1);

        let Ok(shallower) = usual.down(usual.rows - 1);

        assert_eq!(
            doings(&said(&[Heard::Across { shape: usual, step: 1 }])),
            vec![Doing::Its(Its::Home(wider))]
        );
        assert_eq!(
            doings(&said(&[Heard::Down { shape: usual, step: -1 }])),
            vec![Doing::Its(Its::Home(shallower))]
        );
    }

    #[test]
    fn the_size_stops_at_both_ends_of_the_ladder() {
        let Ok(smallest) = Shape::USUAL.sized(Size::Tiny);

        let Ok(biggest) = Shape::USUAL.sized(Size::Huge);

        assert_eq!(
            doings(&said(&[Heard::Sized { shape: smallest, step: -1 }])),
            vec![Doing::Its(Its::Home(smallest))]
        );
        assert_eq!(
            doings(&said(&[Heard::Sized { shape: biggest, step: 1 }])),
            vec![Doing::Its(Its::Home(biggest))]
        );
    }
}
