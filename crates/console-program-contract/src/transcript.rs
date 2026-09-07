//! Words in, doings out, written down.
//!
//! This is what makes the contract worth having. A trait with no way to press
//! it is a shape: what settles a program is a list of the words it was told
//! and a list of what it decided, held side by side, going red the moment the
//! program changes its mind about either.
//!
//! It keeps the doings *per word* rather than in one heap. A heap says that
//! somewhere in nine words the daemon ran the menu; a transcript says it ran
//! the menu on the release and not on the press, which is the half of the
//! button contract that was wrong for a year.

use console_core_never::Never;

use crate::argv::Argv;
use crate::doing::Doing;
use crate::program::{Opening, Program, Turn};
use crate::wants::Wants;
use crate::word::Word;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turned<Hears, Does> {
    pub word: Word<Hears>,
    pub doings: Vec<Doing<Does>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said<State, Hears, Does> {
    pub now: State,
    pub wants: Vec<Wants>,
    pub turns: Vec<Turned<Hears, Does>>,
}

impl<State, Hears, Does> Said<State, Hears, Does>
where
    Does: Clone,
{
    pub fn doings(&self) -> Result<Vec<Doing<Does>>, Never> {
        Ok(self.turns.iter().flat_map(|turned| turned.doings.iter().cloned()).collect())
    }

    pub fn on(&self, turn: usize) -> Result<Option<&[Doing<Does>]>, Never> {
        Ok(self.turns.get(turn).map(|turned| turned.doings.as_slice()))
    }
}

pub type Transcript<P> =
    Result<Said<<P as Program>::State, <P as Program>::Hears, <P as Program>::Does>, Never>;

pub fn walk<P: Program>(from: &P::State, words: &[Word<P::Hears>]) -> Transcript<P> {
    said::<P>(from.clone(), Vec::new(), words)
}

pub fn told<P: Program>(argv: &Argv, words: &[Word<P::Hears>]) -> Transcript<P> {
    let Opening { state, wants } = P::opening(argv);

    said::<P>(state, wants, words)
}

fn said<P: Program>(
    from: P::State,
    wants: Vec<Wants>,
    words: &[Word<P::Hears>],
) -> Transcript<P> {
    let mut state = from;
    let mut turns: Vec<Turned<P::Hears, P::Does>> = Vec::new();

    for word in words {
        let Turn { now, doings } = P::heard(&state, word);

        state = now;
        turns.push(Turned { word: word.clone(), doings });
    }

    Ok(Said { now: state, wants, turns })
}
