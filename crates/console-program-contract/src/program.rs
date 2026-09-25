//! The contract itself.

use std::fmt::Debug;

use console_core_never::Never;

use crate::arguments::Arguments;
use crate::effect::Effect;
use crate::subscription::Subscription;
use crate::event::Event;

pub trait Program {
    type State: Clone + Debug + PartialEq;

    type Event: Clone + Debug + PartialEq;

    type Effect: Clone + Debug + PartialEq;

    fn init(arguments: &Arguments) -> Initial<Self::State>;

    fn update(state: &Self::State, event: &Event<Self::Event>) -> Update<Self::State, Self::Effect>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Initial<State> {
    pub state: State,
    pub subscriptions: Vec<Subscription>,
}

impl<State> Initial<State> {
    pub fn new(state: State) -> Result<Self, Never> {
        Ok(Initial { state, subscriptions: Vec::new() })
    }

    pub fn subscribed(state: State, subscriptions: Vec<Subscription>) -> Result<Self, Never> {
        Ok(Initial { state, subscriptions })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update<State, F> {
    pub state: State,
    pub effects: Vec<Effect<F>>,
}

impl<State, F> Update<State, F> {
    pub fn none(state: State) -> Result<Self, Never> {
        Ok(Update { state, effects: Vec::new() })
    }

    pub fn new(state: State, effects: Vec<Effect<F>>) -> Result<Self, Never> {
        Ok(Update { state, effects })
    }
}
