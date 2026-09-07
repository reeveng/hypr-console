//! What the notices panel is looking at, and what a press does about it.
//!
//! The panel used to hold this in an actor whose whole job was to be reachable
//! from every closure that draws a row. What it holds is one of two things, so
//! the actor was the shape rather than the state, and it is here as a
//! `console_program_contract::Program`: a press goes in, a place and a list of
//! doings comes out, and none of it needs a mako or a screen.
//!
//! Dismissing goes back to the list before the row it dismissed has gone,
//! because mako answers the dismissal before it answers a fresh `list`, and a
//! panel that stayed would be drawing a notice nobody can act on.

use console_core_external_programs::Program as Theirs;
use console_core_never::Never;
use console_program_contract::{Argv, Doing, Opening, Program, Runs, Turn, Word};

pub const UP: usize = 0;

pub const DEEPER: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Onto {
    List,
    One(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard {
    Chose(u32),
    Back,
    Dismissed(u32),
    ClearAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Its {
    Replace(usize),
    Refresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closes {
    Yes,
    No,
}

pub struct Notices;

impl Program for Notices {
    type State = Onto;
    type Hears = Heard;
    type Does = Its;

    fn opening(_argv: &Argv) -> Opening<Onto> {
        let Ok(opening) = Opening::holding(Onto::List);

        opening
    }

    fn heard(state: &Onto, word: &Word<Heard>) -> Turn<Onto, Its> {
        let Ok(turn) = match word {
            Word::Its(Heard::Chose(id)) => {
                Turn::doing(Onto::One(*id), vec![Doing::Its(Its::Replace(DEEPER))])
            }

            Word::Its(Heard::Back) => match state {
                Onto::One(_) => Turn::doing(Onto::List, vec![Doing::Its(Its::Replace(UP))]),
                Onto::List => Turn::nothing(*state),
            },

            Word::Its(Heard::Dismissed(id)) => {
                let Ok(dismiss) =
                    Runs::theirs(Theirs::Makoctl, &["dismiss", "-n", &id.to_string()]);

                Turn::doing(
                    Onto::List,
                    vec![Doing::Ask(dismiss), Doing::Its(Its::Replace(UP))],
                )
            }

            Word::Its(Heard::ClearAll) => {
                let Ok(clear) = Runs::theirs(Theirs::Makoctl, &["dismiss", "--all"]);

                Turn::doing(Onto::List, vec![Doing::Ask(clear), Doing::Its(Its::Refresh)])
            }

            Word::Opened | Word::Changed(_) | Word::CameRound(_, _) | Word::Answered(_)
            | Word::Chose(_) | Word::Stopping => Turn::nothing(*state),
        };

        turn
    }
}

pub fn closes(state: &Onto) -> Result<Closes, Never> {
    Ok(match state {
        Onto::List => Closes::Yes,
        Onto::One(_) => Closes::No,
    })
}

#[cfg(test)]
mod tests {
    use console_program_contract::told;

    use super::*;

    fn dismissing(id: u32) -> Doing<Its> {
        let Ok(runs) = Runs::theirs(Theirs::Makoctl, &["dismiss", "-n", &id.to_string()]);

        Doing::Ask(runs)
    }

    #[test]
    fn a_row_opens_onto_the_notice_it_names() {
        let Ok(said) = told::<Notices>(&Argv::default(), &[Word::Its(Heard::Chose(7))]);

        assert_eq!(said.now, Onto::One(7));
        assert_eq!(said.on(0), Ok(Some([Doing::Its(Its::Replace(DEEPER))].as_slice())));
    }

    #[test]
    fn back_out_of_a_notice_is_the_list_and_back_out_of_the_list_is_the_way_out() {
        let Ok(one) = told::<Notices>(
            &Argv::default(),
            &[Word::Its(Heard::Chose(7)), Word::Its(Heard::Back)],
        );

        assert_eq!(one.now, Onto::List);
        assert_eq!(one.on(1), Ok(Some([Doing::Its(Its::Replace(UP))].as_slice())));
        assert_eq!(closes(&one.now), Ok(Closes::Yes));
        assert_eq!(closes(&Onto::One(7)), Ok(Closes::No));
    }

    #[test]
    fn dismissing_one_asks_mako_and_comes_back_to_the_list() {
        let Ok(said) = told::<Notices>(
            &Argv::default(),
            &[Word::Its(Heard::Chose(7)), Word::Its(Heard::Dismissed(7))],
        );

        assert_eq!(said.now, Onto::List);
        assert_eq!(
            said.on(1),
            Ok(Some([dismissing(7), Doing::Its(Its::Replace(UP))].as_slice()))
        );
    }

    #[test]
    fn clearing_them_all_stays_where_it_is_and_draws_again() {
        let Ok(said) = told::<Notices>(&Argv::default(), &[Word::Its(Heard::ClearAll)]);
        let Ok(clear) = Runs::theirs(Theirs::Makoctl, &["dismiss", "--all"]);

        assert_eq!(
            said.on(0),
            Ok(Some([Doing::Ask(clear), Doing::Its(Its::Refresh)].as_slice()))
        );
    }
}
