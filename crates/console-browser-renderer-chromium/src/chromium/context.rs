//! A tab must dispose of any context it acquired, even if attaching fails.

use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};

pub(super) struct Lifecycle;

pub(super) enum State {
  Closed,
  Creating,
  Attaching(String),
  Open(String),
  Disposing,
}

impl State {
  #[must_use]
  pub(super) fn id(&self) -> core::result::Result<Option<&str>, console_core_never::Never> {
    Ok(match self {
      Self::Open(id) => Some(id),
      Self::Closed | Self::Creating | Self::Attaching(_) | Self::Disposing => None,
    })
  }
}

pub(super) enum Request {
  Open,
  Created(String),
  Attached(bool),
  Close,
  Disposed,
}

pub(super) enum Effect {
  Create,
  Attach(String),
  Dispose(String),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(&[Service::Process])
  }
}

impl SingleEffectMachine for Lifecycle {
  type Input = ();
  type State = State;
  type Request = Request;
  type Effect = Effect;

  fn initialize(_: &(), _: Option<State>) -> Result<SingleTransition<State, Effect>, Never> {
    Ok(SingleTransition {
      state: State::Closed,
      effect: None,
    })
  }

  fn handle(state: State, request: Request) -> Result<SingleTransition<State, Effect>, Never> {
    let (state_3, effect) = match (state, request) {
      (State::Closed, Request::Open) => (State::Creating, Some(Effect::Create)),
      (State::Creating, Request::Created(id)) => {
        (State::Attaching(id.clone()), Some(Effect::Attach(id)))
      }
      (State::Attaching(id), Request::Attached(true)) => (State::Open(id), None),
      (State::Attaching(id), Request::Attached(false)) | (State::Open(id), Request::Close) => {
        (State::Disposing, Some(Effect::Dispose(id)))
      }
      (State::Disposing, Request::Disposed) => (State::Closed, None),
      (state_2, _) => (state_2, None),
    };
    Ok(SingleTransition {
      state: state_3,
      effect,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[must_use]
  fn attaching() -> core::result::Result<State, console_core_never::Never> {
    let Ok(start) = Lifecycle::initialize(&(), None);
    let Ok(create) = Lifecycle::handle(start.state, Request::Open);
    assert!(matches!(create.effect, Some(Effect::Create)));
    let Ok(attach) = Lifecycle::handle(create.state, Request::Created("private".to_owned()));
    assert!(matches!(attach.effect, Some(Effect::Attach(id)) if id == "private"));

    Ok(attach.state)
  }

  #[test]
  fn closing_disposes_only_the_owned_context() {
    let Ok(open) = Lifecycle::handle(
      {
        let Ok(explicit_value) = attaching();
        explicit_value
      },
      Request::Attached(true),
    );
    let Ok(open_id) = open.state.id();
    assert_eq!(open_id, Some("private"));
    let Ok(close) = Lifecycle::handle(open.state, Request::Close);
    assert!(matches!(close.effect, Some(Effect::Dispose(id)) if id == "private"));
    let Ok(closed) = Lifecycle::handle(close.state, Request::Disposed);
    assert!(matches!(closed.state, State::Closed));
  }

  #[test]
  fn a_failed_attach_still_disposes_the_context() {
    let Ok(failed) = Lifecycle::handle(
      {
        let Ok(explicit_value) = attaching();
        explicit_value
      },
      Request::Attached(false),
    );
    assert!(matches!(failed.effect, Some(Effect::Dispose(id)) if id == "private"));
  }
}
