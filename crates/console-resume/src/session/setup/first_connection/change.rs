//! A preferred mode is accepted only after verification. Restore the previous
//! mode when needed, while the same set of displays is still connected.

use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};

use super::Live;
use crate::Unresumed;

pub(super) struct ModeChange;

pub(super) enum State {
  Applying,
  Verifying,
  Restoring,
  VerifyingRestore,
  Finished(Result<Option<Live>, Unresumed>),
}

pub(super) enum Reply {
  Applied(Result<(), Unresumed>),
  Settled(Result<Option<Live>, Unresumed>),
}

pub(super) enum Effect {
  ApplyPreferred,
  VerifyPreferred,
  Restore(Option<Unresumed>),
  VerifyRestore,
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::ApplyPreferred | Self::Restore(None) => &[Service::Compositor],
      Self::Restore(Some(_)) => &[Service::Compositor, Service::Console],
      Self::VerifyPreferred | Self::VerifyRestore => &[Service::Compositor, Service::Clock],
    })
  }
}

impl SingleEffectMachine for ModeChange {
  type Input = ();
  type State = State;
  type Request = Reply;
  type Effect = Effect;

  fn initialize(_: &(), _: Option<State>) -> Result<SingleTransition<State, Effect>, Never> {
    Ok(SingleTransition {
      state: State::Applying,
      effect: Some(Effect::ApplyPreferred),
    })
  }

  fn handle(state: State, reply: Reply) -> Result<SingleTransition<State, Effect>, Never> {
    Ok(match (state, reply) {
      (State::Applying, Reply::Applied(Ok(()))) => SingleTransition {
        state: State::Verifying,
        effect: Some(Effect::VerifyPreferred),
      },
      (State::Verifying, Reply::Settled(Ok(Some(live)))) => return finished(Ok(Some(live))),
      (State::Applying, Reply::Applied(Err(Unresumed::DisplaysChanged)))
      | (State::Verifying, Reply::Settled(Err(Unresumed::DisplaysChanged))) => {
        return finished(Err(Unresumed::DisplaysChanged));
      }
      (State::Applying, Reply::Applied(Err(fault)))
      | (State::Verifying, Reply::Settled(Err(fault))) => SingleTransition {
        state: State::Restoring,
        effect: Some(Effect::Restore(Some(fault))),
      },
      (State::Verifying, Reply::Settled(Ok(None))) => SingleTransition {
        state: State::Restoring,
        effect: Some(Effect::Restore(None)),
      },
      (State::Restoring, Reply::Applied(Ok(()))) => SingleTransition {
        state: State::VerifyingRestore,
        effect: Some(Effect::VerifyRestore),
      },
      (State::Restoring, Reply::Applied(Err(fault)))
      | (State::VerifyingRestore, Reply::Settled(Err(fault))) => return finished(Err(fault)),
      (State::VerifyingRestore, Reply::Settled(Ok(Some(_)))) => return finished(Ok(None)),
      (State::VerifyingRestore, Reply::Settled(Ok(None))) => {
        return finished(Err(Unresumed::ApplyingSetup(
          "could not restore the display's original mode".to_owned(),
        )));
      }
      (state_2, _) => SingleTransition {
        state: state_2,
        effect: None,
      },
    })
  }
}

fn finished(
  result: Result<Option<Live>, Unresumed>,
) -> Result<SingleTransition<State, Effect>, Never> {
  Ok(SingleTransition {
    state: State::Finished(result),
    effect: None,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_timeout_restores_and_verifies_the_original_mode() {
    let Ok(started) = ModeChange::initialize(&(), None);
    assert!(matches!(started.effect, Some(Effect::ApplyPreferred)));
    let Ok(applied) = ModeChange::handle(started.state, Reply::Applied(Ok(())));
    assert!(matches!(applied.effect, Some(Effect::VerifyPreferred)));
    let Ok(timed_out) = ModeChange::handle(applied.state, Reply::Settled(Ok(None)));
    assert!(matches!(timed_out.effect, Some(Effect::Restore(None))));
    let Ok(restored) = ModeChange::handle(timed_out.state, Reply::Applied(Ok(())));
    assert!(matches!(restored.effect, Some(Effect::VerifyRestore)));
    let Ok(live) = Live::from_outputs(Vec::new());
    let Ok(confirmed) = ModeChange::handle(restored.state, Reply::Settled(Ok(Some(live))));
    assert!(matches!(confirmed.state, State::Finished(Ok(None))));
    assert!(confirmed.effect.is_none());
  }

  #[test]
  fn a_changed_display_set_stops_the_transaction_before_rollback() {
    let Ok(applied) = ModeChange::handle(State::Applying, Reply::Applied(Ok(())));
    let Ok(changed) = ModeChange::handle(
      applied.state,
      Reply::Settled(Err(Unresumed::DisplaysChanged)),
    );
    assert!(matches!(
      changed.state,
      State::Finished(Err(Unresumed::DisplaysChanged))
    ));
    assert!(changed.effect.is_none());
  }
}
