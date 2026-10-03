//! A machine: a state, one request handled, the next state, and what the world
//! has to be asked to do, said as values on a queue whoever drives it owns.
//!
//! The words are effect's, from `@effect/experimental/Machine`: a machine is
//! `initialize`d from its input and, when it was saved, from the state it had
//! before; it `handle`s one request at a time; one that can be written down
//! and read back is a `SerializableMachine`; an `Actor` is one running on a
//! thread of its own, and that is `console-actor`. What is not effect's is
//! that nothing here performs anything. A handler in effect forks the work it
//! wants; a handler here offers an `Effect` to a `Queue`, and whatever drives the
//! machine carries it out. That is what lets a recorded run replay on a laptop
//! with no compositor and reach the state the device reached, and it is the
//! split `console-program-contract` was already written on.
//!
//! **A handler cannot fail.** A request that means nothing in the state it
//! arrives in is a transition back to the same state, written as an arm; a
//! fault met while carrying an effect out comes back as a request. An error a
//! handler could return would be a third outcome beside the state and the
//! effects, and the one nobody draws on the map.
//!
//! **The state is moved, not borrowed.** `handle` takes the state and gives one
//! back, so a large state is never copied to change a small part of it, and a
//! machine made of machines hands each child its own part and puts back what
//! the child returns. An earlier draft asked every state, request and effect to
//! be `Copy`, which nothing that holds a path, a name or a list can be.
//!
//! **Saving is a machine's to choose.** Every machine can implement
//! [`SerializableMachine`] and none has to. `initialize` is handed the state a
//! saved machine had and decides what of it to keep: what a person owns -- the
//! page they were on, the workspace an app was on, a half-typed line -- comes
//! back, and a reading of the world -- which panel was up, how full the battery
//! was -- is asked again, because a reading restored after a reboot is one that
//! is confidently wrong.
//!
//! `docs/state-machines.md` is the rest of the argument.

mod schema;
mod snapshot;
mod transcript;

pub use schema::{Decoder, Encoder, ParseError, Serializable};
pub use snapshot::{SerializableMachine, Version, restore, snapshot};
pub use transcript::{Step, Trace, Transcript, run, run_from};

pub use console_core_never::Never;

pub trait Machine {
    type Input;
    type State;
    type Request;
    type Effect;

    fn initialize(
        input: &Self::Input,
        previous: Option<Self::State>,
        effects: &mut Queue<Self::Effect>,
    ) -> Result<Self::State, Never>;

    fn handle(
        state: Self::State,
        request: Self::Request,
        effects: &mut Queue<Self::Effect>,
    ) -> Result<Self::State, Never>;

    fn initial_transition(
        input: &Self::Input,
        previous: Option<Self::State>,
    ) -> Result<Transition<Self::State, Self::Effect>, Never> {
        let Ok(mut effects) = Queue::unbounded();
        let Ok(state) = Self::initialize(input, previous, &mut effects);
        let Ok(effects) = effects.take_all();

        Ok(Transition { state, effects })
    }

    fn transition(state: Self::State, request: Self::Request) -> Result<Transition<Self::State, Self::Effect>, Never> {
        let Ok(mut effects) = Queue::unbounded();
        let Ok(state) = Self::handle(state, request, &mut effects);
        let Ok(effects) = effects.take_all();

        Ok(Transition { state, effects })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition<S, E> {
    pub state: S,
    pub effects: Vec<E>,
}

impl<S, E> Transition<S, E> {
    pub fn new(state: S, effects: Vec<E>) -> Result<Self, Never> {
        Ok(Transition { state, effects })
    }

    pub fn without_effects(state: S) -> Result<Self, Never> {
        Ok(Transition { state, effects: Vec::new() })
    }

    pub fn offered(self, to: &mut Queue<E>) -> Result<S, Never> {
        let Transition { state, effects } = self;
        let Ok(()) = to.offer_all(effects);

        Ok(state)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queue<T> {
    items: Vec<T>,
}

impl<T> Queue<T> {
    pub fn unbounded() -> Result<Self, Never> {
        Ok(Queue { items: Vec::new() })
    }

    pub fn offer(&mut self, item: T) -> Result<(), Never> {
        self.items.push(item);

        Ok(())
    }

    pub fn offer_all(&mut self, items: impl IntoIterator<Item = T>) -> Result<(), Never> {
        self.items.extend(items);

        Ok(())
    }

    pub fn take_all(&mut self) -> Result<Vec<T>, Never> {
        Ok(std::mem::take(&mut self.items))
    }
}
