//! A state stepped again and again until a step says it is done.
//!
//! EXPLICIT053 and EXPLICIT054 took `while` and `loop`, and most of what they
//! found was a list or a sequence of steps, which an iterator already says the
//! end of. What was left was the program that runs for as long as something
//! hands it events -- a daemon waiting on a descriptor, a panel's queue --
//! and those had each written a `loop` whose end was a `return` somewhere in
//! its body. This is that shape written once: the state goes in, a step hands
//! back `Again` with the next state or `Halt` with what the program ends on,
//! and the end is the one variant that says so.
//!
//! The state is handed to the step and handed back rather than captured,
//! which is EXPLICIT047's argument: a closure that holds what it writes is a
//! closure whose writes nobody can see at the call.
//!
//! Underneath it is `try_fold` over `repeat`, which has no end of its own. So
//! the one answer it cannot give -- the repetition ending without a step
//! saying `Halt` -- is still an answer here, `Endless`, rather than a value
//! somebody made up for it.

use console_core_never::Never;
use std::fmt;
use std::ops::ControlFlow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step<State, Exit> {
    Again(State),
    Halt(Exit),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endless;

impl fmt::Display for Endless {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(to, "the steps ran out without one of them saying it was done")
    }
}

impl std::error::Error for Endless {}

pub fn iterate<State, Exit>(
    state: State,
    step: impl Fn(State) -> Result<Step<State, Exit>, Never>,
) -> Result<Exit, Endless> {
    let flow = std::iter::repeat(()).try_fold(state, |state, ()| {
        let Ok(stepped) = step(state);

        match stepped {
            Step::Again(next) => ControlFlow::Continue(next),
            Step::Halt(exit) => ControlFlow::Break(exit),
        }
    });

    match flow {
        ControlFlow::Break(exit) => Ok(exit),
        ControlFlow::Continue(_never_reached) => Err(Endless),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_steps_until_a_step_says_done_and_ends_on_what_that_step_said() {
        let ended = iterate((3_u8, Vec::new()), |(at, mut seen)| {
            seen.push(at);

            Ok(match at.checked_sub(1) {
                Some(next) => Step::Again((next, seen)),
                None => Step::Halt(seen),
            })
        });

        assert_eq!(ended, Ok(vec![3, 2, 1, 0]));
    }

    #[test]
    fn a_first_step_that_is_done_is_the_only_step() {
        let ended = iterate(0_u32, |steps| Ok(Step::<u32, u32>::Halt(steps.saturating_add(1))));

        assert_eq!(ended, Ok(1));
    }
}
