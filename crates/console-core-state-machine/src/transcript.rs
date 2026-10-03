//! Requests in, effects out, written down.
//!
//! A machine with no way to press it is a shape. What settles one is a list of
//! the requests it was sent and a list of what it decided, held side by side,
//! going red the moment the machine changes its mind about either. The
//! effects are kept per request rather than in one heap: a heap says that
//! somewhere in nine requests the menu was opened, and a transcript says it
//! was opened on the release and not on the press.
//!
//! `console-program-contract` had this first, for programs; it is here because
//! every machine is pressed the same way.

use console_core_never::Never;

use crate::{Machine, Queue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step<R, E> {
    pub request: R,
    pub effects: Vec<E>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace<S, R, E> {
    pub state: S,
    pub initialized: Vec<E>,
    pub steps: Vec<Step<R, E>>,
}

impl<S, R, E: Clone> Trace<S, R, E> {
    pub fn effects(&self) -> Result<Vec<E>, Never> {
        Ok(self.steps.iter().flat_map(|step| step.effects.iter().cloned()).collect())
    }

    pub fn on(&self, step: u32) -> Result<Option<&[E]>, Never> {
        let found = self.steps.iter().zip(0_u32..).find(|(_, at)| *at == step);

        Ok(found.map(|(step, _)| step.effects.as_slice()))
    }
}

pub type Transcript<M> =
    Result<Trace<<M as Machine>::State, <M as Machine>::Request, <M as Machine>::Effect>, Never>;

pub fn run<M: Machine<Request: Clone>>(input: &M::Input, requests: &[M::Request]) -> Transcript<M> {
    let Ok(mut effects) = Queue::unbounded();
    let Ok(state) = M::initialize(input, None, &mut effects);
    let Ok(initialized) = effects.take_all();

    trace::<M>(state, initialized, requests)
}

pub fn run_from<M: Machine<Request: Clone>>(from: M::State, requests: &[M::Request]) -> Transcript<M> {
    trace::<M>(from, Vec::new(), requests)
}

fn trace<M: Machine<Request: Clone>>(
    from: M::State,
    initialized: Vec<M::Effect>,
    requests: &[M::Request],
) -> Transcript<M> {
    let Ok(mut effects) = Queue::unbounded();
    let mut state = from;
    let mut steps = Vec::new();

    for request in requests {
        let Ok(next) = M::handle(state, request.clone(), &mut effects);
        let Ok(decided) = effects.take_all();

        state = next;
        steps.push(Step { request: request.clone(), effects: decided });
    }

    Ok(Trace { state, initialized, steps })
}
