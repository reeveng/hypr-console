//! Events in, effects out, written down.
//!
//! This is what makes the contract worth having. A trait with no way to press
//! it is a shape: what settles a program is a list of the events it was told
//! and a list of what it decided, held side by side, going red the moment the
//! program changes its mind about either.
//!
//! It keeps the effects *per event* rather than in one heap. A heap says that
//! somewhere in nine events the daemon ran the menu; a transcript says it ran
//! the menu on the release and not on the press, which is the half of the
//! button contract that was wrong for a year.

use console_core_never::Never;
use console_core_number_conversion::index;

use crate::arguments::Arguments;
use crate::effect::Effect;
use crate::program::{Initial, Program, Update};
use crate::subscription::Subscription;
use crate::event::Event;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step<E, F> {
    pub event: Event<E>,
    pub effects: Vec<Effect<F>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace<State, E, F> {
    pub state: State,
    pub subscriptions: Vec<Subscription>,
    pub steps: Vec<Step<E, F>>,
}

impl<State, E, F> Trace<State, E, F>
where
    F: Clone,
{
    pub fn effects(&self) -> Result<Vec<Effect<F>>, Never> {
        Ok(self.steps.iter().flat_map(|step| step.effects.iter().cloned()).collect())
    }

    pub fn on(&self, step: u32) -> Result<Option<&[Effect<F>]>, Never> {
        let Ok(step) = index(step);

        Ok(self.steps.get(step).map(|step| step.effects.as_slice()))
    }
}

pub type Transcript<P> =
    Result<Trace<<P as Program>::State, <P as Program>::Event, <P as Program>::Effect>, Never>;

pub fn run_from<P: Program>(from: &P::State, events: &[Event<P::Event>]) -> Transcript<P> {
    trace::<P>(from.clone(), Vec::new(), events)
}

pub fn run<P: Program>(arguments: &Arguments, events: &[Event<P::Event>]) -> Transcript<P> {
    let Initial { state, subscriptions } = P::init(arguments);

    trace::<P>(state, subscriptions, events)
}

fn trace<P: Program>(
    from: P::State,
    subscriptions: Vec<Subscription>,
    events: &[Event<P::Event>],
) -> Transcript<P> {
    let mut state = from;
    let mut steps: Vec<Step<P::Event, P::Effect>> = Vec::new();

    for event in events {
        let Update { state: next, effects } = P::update(&state, event);

        state = next;
        steps.push(Step { event: event.clone(), effects });
    }

    Ok(Trace { state, subscriptions, steps })
}
