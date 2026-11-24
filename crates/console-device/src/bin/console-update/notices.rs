//! An update tag is remembered only after the desktop accepts its notice.

use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};

use super::UpdateError;

pub(super) enum Eligibility {
  Suppressed,
  Offer,
}

pub(super) enum Delivery {
  Posted,
  Deferred,
}

pub(super) enum State {
  Inspecting,
  Posting,
  Remembering,
  Finished(Result<(), UpdateError>),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Effect {
  Inspect,
  Post,
  Remember,
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Inspect => &[Service::Process, Service::FileSystem],
      Self::Post => &[Service::Notifications],
      Self::Remember => &[Service::FileSystem],
    })
  }
}

pub(super) enum Report {
  Inspected(Result<Eligibility, UpdateError>),
  Posted(Result<Delivery, UpdateError>),
  Remembered(Result<(), UpdateError>),
}

pub(super) struct Notice;

impl SingleEffectMachine for Notice {
  type Input = ();
  type State = State;
  type Request = Report;
  type Effect = Effect;

  fn initialize(
    _input: &(),
    _previous: Option<State>,
  ) -> Result<SingleTransition<State, Effect>, Never> {
    Ok(SingleTransition {
      state: State::Inspecting,
      effect: Some(Effect::Inspect),
    })
  }

  fn handle(state: State, report: Report) -> Result<SingleTransition<State, Effect>, Never> {
    let (next, effect) = match (state, report) {
      (State::Inspecting, Report::Inspected(Ok(Eligibility::Offer))) => {
        (State::Posting, Some(Effect::Post))
      }
      (State::Posting, Report::Posted(Ok(Delivery::Posted))) => {
        (State::Remembering, Some(Effect::Remember))
      }
      (State::Inspecting, Report::Inspected(Ok(Eligibility::Suppressed)))
      | (State::Posting, Report::Posted(Ok(Delivery::Deferred))) => {
        (State::Finished(Ok(())), None)
      }
      (State::Inspecting, Report::Inspected(Err(fault)))
      | (State::Posting, Report::Posted(Err(fault))) => (State::Finished(Err(fault)), None),
      (State::Remembering, Report::Remembered(result)) => (State::Finished(result), None),
      (unchanged, _) => (unchanged, None),
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
  fn the_tag_is_written_only_after_a_posted_reply() {
    let Ok(inspecting) = Notice::initialize(&(), None);
    assert_eq!(inspecting.effect, Some(Effect::Inspect));
    let Ok(posting) = Notice::handle(inspecting.state, Report::Inspected(Ok(Eligibility::Offer)));
    assert_eq!(posting.effect, Some(Effect::Post));
    let Ok(remembering) = Notice::handle(posting.state, Report::Posted(Ok(Delivery::Posted)));
    assert_eq!(remembering.effect, Some(Effect::Remember));
    let Ok(finished) = Notice::handle(remembering.state, Report::Remembered(Ok(())));
    assert!(matches!(finished.state, State::Finished(Ok(()))));
    assert!(finished.effect.is_none());
  }
}
