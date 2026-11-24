//! A release is downloaded and verified before the live checkout can change.

use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};

use super::UpdateError;

pub(super) enum State {
  Downloading,
  Verifying,
  Installing,
  Finished(Result<(), UpdateError>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Effect {
  Download,
  Verify,
  Install,
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Download | Self::Verify => &[
        Service::Console,
        Service::FileSystem,
        Service::Network,
        Service::Notifications,
        Service::Process,
      ],
      Self::Install => &[
        Service::Console,
        Service::FileSystem,
        Service::Notifications,
        Service::Process,
      ],
    })
  }
}

pub(super) struct Installation;

impl SingleEffectMachine for Installation {
  type Input = ();
  type State = State;
  type Request = Result<(), UpdateError>;
  type Effect = Effect;

  fn initialize(
    _input: &(),
    _previous: Option<State>,
  ) -> Result<SingleTransition<State, Effect>, Never> {
    Ok(SingleTransition {
      state: State::Downloading,
      effect: Some(Effect::Download),
    })
  }

  fn handle(
    state: State,
    result: Result<(), UpdateError>,
  ) -> Result<SingleTransition<State, Effect>, Never> {
    let (next, effect) = match (state, result) {
      (State::Finished(result_2), _) => (State::Finished(result_2), None),
      (_, Err(fault)) => (State::Finished(Err(fault)), None),
      (State::Downloading, Ok(())) => (State::Verifying, Some(Effect::Verify)),
      (State::Verifying, Ok(())) => (State::Installing, Some(Effect::Install)),
      (State::Installing, Ok(())) => (State::Finished(Ok(())), None),
    };

    Ok(SingleTransition {
      state: next,
      effect,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn downloading_and_verifying_are_separate_effects_before_installing() {
    let Ok(download) = Installation::initialize(&(), None);
    assert_eq!(download.effect, Some(Effect::Download));
    let Ok(verify) = Installation::handle(download.state, Ok(()));
    assert_eq!(verify.effect, Some(Effect::Verify));
    let Ok(install) = Installation::handle(verify.state, Ok(()));
    assert_eq!(install.effect, Some(Effect::Install));
    let Ok(finished) = Installation::handle(install.state, Ok(()));
    assert!(matches!(finished.state, State::Finished(Ok(()))));
    assert!(finished.effect.is_none());
  }

  #[test]
  fn a_failed_download_or_verification_cannot_start_installing() {
    for state in [State::Downloading, State::Verifying] {
      let Ok(failed) = Installation::handle(state, Err(UpdateError::from("bundle failed")));
      assert!(
        matches!(failed.state, State::Finished(Err(fault)) if fault.to_string() == "bundle failed")
      );
      assert!(failed.effect.is_none());
    }
  }
}
