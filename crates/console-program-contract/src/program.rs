//! The contract itself.

use std::fmt::Debug;

use console_never::Never;

use crate::argv::Argv;
use crate::doing::Doing;
use crate::wants::Wants;
use crate::word::Word;

pub trait Program {
    type State: Clone + Debug + PartialEq;

    type Hears: Clone + Debug + PartialEq;

    type Does: Clone + Debug + PartialEq;

    fn opening(argv: &Argv) -> Opening<Self::State>;

    fn heard(state: &Self::State, word: &Word<Self::Hears>) -> Turn<Self::State, Self::Does>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opening<State> {
    pub state: State,
    pub wants: Vec<Wants>,
}

impl<State> Opening<State> {
    pub fn holding(state: State) -> Result<Self, Never> {
        Ok(Opening { state, wants: Vec::new() })
    }

    pub fn listening(state: State, wants: Vec<Wants>) -> Result<Self, Never> {
        Ok(Opening { state, wants })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn<State, Does> {
    pub now: State,
    pub doings: Vec<Doing<Does>>,
}

impl<State, Does> Turn<State, Does> {
    pub fn nothing(now: State) -> Result<Self, Never> {
        Ok(Turn { now, doings: Vec::new() })
    }

    pub fn doing(now: State, doings: Vec<Doing<Does>>) -> Result<Self, Never> {
        Ok(Turn { now, doings })
    }
}
