//! A machine whose initializer and each request can emit at most one effect.
//! The driver receives that effect inline with the next state.

use crate::{EffectDependencies, Never};

pub trait SingleEffectMachine {
  type Input;
  type State;
  type Request;
  type Effect: EffectDependencies;

  fn initialize(
    input: &Self::Input,
    previous: Option<Self::State>,
  ) -> Result<SingleTransition<Self::State, Self::Effect>, Never>;

  fn handle(
    state: Self::State,
    request: Self::Request,
  ) -> Result<SingleTransition<Self::State, Self::Effect>, Never>;

  fn run_effects<Context>(
    mut turn: SingleTransition<Self::State, Self::Effect>,
    limit: u32,
    context: &mut Context,
    perform: impl Fn(&mut Context, Self::Effect) -> Self::Request,
  ) -> Result<Self::State, SingleTransition<Self::State, Self::Effect>> {
    for _ in 0..limit {
      let SingleTransition { state, effect } = turn;
      let pending = match effect {
        Some(effect_2) => effect_2,
        None => return Ok(state),
      };
      let reply = perform(context, pending);
      let Ok(next) = Self::handle(state, reply);
      turn = next;
    }

    match turn.effect {
      Some(_) => Err(turn),
      None => Ok(turn.state),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingleTransition<S, E> {
  pub state: S,
  pub effect: Option<E>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Service;

  struct Countdown;

  #[derive(Debug, PartialEq, Eq)]
  struct Decrement(u8);

  impl EffectDependencies for Decrement {
    fn services(&self) -> Result<&'static [Service], Never> {
      Ok(&[])
    }
  }

  impl SingleEffectMachine for Countdown {
    type Input = u8;
    type State = u8;
    type Request = u8;
    type Effect = Decrement;

    fn initialize(
      input: &u8,
      _previous: Option<u8>,
    ) -> Result<SingleTransition<u8, Decrement>, Never> {
      next(*input)
    }

    fn handle(_state: u8, reply: u8) -> Result<SingleTransition<u8, Decrement>, Never> {
      next(reply)
    }
  }

  fn next(left: u8) -> Result<SingleTransition<u8, Decrement>, Never> {
    let effect = match left {
      0 => None,
      1..=u8::MAX => Some(Decrement(left)),
    };

    Ok(SingleTransition {
      state: left,
      effect,
    })
  }

  #[test]
  fn a_limited_run_keeps_the_pending_effect_and_resumes_in_order() -> Result<(), &'static str> {
    let Ok(first) = Countdown::initialize(&2, None);
    let mut performed = 0_u8;
    let decrement = |count: &mut u8, Decrement(left)| {
      *count = count.saturating_add(1);
      left.saturating_sub(1)
    };
    let pending = match Countdown::run_effects(first, 1, &mut performed, decrement) {
      Err(pending) => pending,
      Ok(_) => return Err("countdown stopped early"),
    };

    assert_eq!(performed, 1);
    assert_eq!(pending.state, 1);
    assert_eq!(pending.effect, Some(Decrement(1)));

    let settled = Countdown::run_effects(pending, 1, &mut performed, decrement)
      .map_err(|_| "countdown did not settle")?;
    assert_eq!(performed, 2);
    assert_eq!(settled, 0);

    let Ok(finished) = Countdown::initialize(&0, None);
    let finished_state = Countdown::run_effects(finished, 0, &mut performed, decrement)
      .map_err(|_| "finished countdown requested an effect")?;
    assert_eq!(finished_state, 0);
    assert_eq!(performed, 2);

    Ok(())
  }
}
