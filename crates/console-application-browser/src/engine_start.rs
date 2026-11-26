//! Start one renderer at a time, using the available alternatives when an
//! automatically selected worker cannot start.

use std::io;

use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};

use crate::renderer::Renderer;

pub(crate) const PREFERENCE: [Renderer; 3] =
  [Renderer::Servo, Renderer::WebKit, Renderer::Chromium];

pub(crate) struct Launch {
  pub(crate) first: Renderer,
  pub(crate) retry_with: &'static [Renderer],
}

pub(crate) struct Start {
  pub(crate) renderer: Renderer,
  pub(crate) previous_failure: Option<(Renderer, io::Error)>,
}

impl EffectDependencies for Start {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(&[Service::Process])
  }
}

pub(crate) enum State {
  Starting(Launch),
  Opened(Renderer),
  Failed(io::Error),
}

type Turn = SingleTransition<State, Start>;

pub(crate) struct EngineStart;

impl EngineStart {
  pub(crate) fn open<Context>(
    launch: &Launch,
    context: &mut Context,
    perform: impl Fn(&mut Context, Start) -> io::Result<()>,
  ) -> io::Result<Renderer> {
    let Ok(first) = Self::initialize(launch, None);

    match Self::run_effects(first, 3, context, perform) {
      Ok(State::Opened(renderer)) => Ok(renderer),
      Ok(State::Failed(fault)) => Err(fault),
      Ok(State::Starting(_)) | Err(_) => {
        Err(io::Error::other("browser engine selection did not settle"))
      }
    }
  }
}

impl SingleEffectMachine for EngineStart {
  type Input = Launch;
  type State = State;
  type Request = io::Result<()>;
  type Effect = Start;

  fn initialize(input: &Launch, _previous: Option<State>) -> Result<Turn, Never> {
    starting(input.first, input.retry_with, None)
  }

  fn handle(state: State, reply: io::Result<()>) -> Result<Turn, Never> {
    match (state, reply) {
      (State::Starting(launch), Ok(())) => Ok(Turn {
        state: State::Opened(launch.first),
        effect: None,
      }),
      (State::Starting(Launch { first, retry_with }), Err(fault)) => {
        let next = PREFERENCE
          .into_iter()
          .skip_while(|renderer| *renderer != first)
          .skip(1)
          .find(|candidate| retry_with.contains(candidate));

        match next {
          Some(renderer) => starting(renderer, retry_with, Some((first, fault))),
          None => Ok(Turn {
            state: State::Failed(fault),
            effect: None,
          }),
        }
      }
      (settled @ (State::Opened(_) | State::Failed(_)), _) => Ok(Turn {
        state: settled,
        effect: None,
      }),
    }
  }
}

fn starting(
  renderer: Renderer,
  retry_with: &'static [Renderer],
  previous_failure: Option<(Renderer, io::Error)>,
) -> Result<Turn, Never> {
  Ok(Turn {
    state: State::Starting(Launch {
      first: renderer,
      retry_with,
    }),
    effect: Some(Start {
      renderer,
      previous_failure,
    }),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn failed_automatic_choices_start_the_next_available_engine() {
    let input = Launch {
      first: Renderer::Servo,
      retry_with: &PREFERENCE,
    };
    let Ok(first) = EngineStart::initialize(&input, None);
    let started = EngineStart::run_effects(first, 3, &mut Vec::new(), |attempts, effect| {
      attempts.push(effect.renderer);

      match effect.renderer {
        Renderer::Servo | Renderer::WebKit => Err(io::Error::other("worker failed")),
        Renderer::Chromium => {
          assert_eq!(
            attempts,
            &[Renderer::Servo, Renderer::WebKit, Renderer::Chromium]
          );
          Ok(())
        }
      }
    });

    assert!(matches!(started, Ok(State::Opened(Renderer::Chromium))));
  }

  #[test]
  fn an_explicit_choice_reports_its_start_error() {
    let input = Launch {
      first: Renderer::WebKit,
      retry_with: &[],
    };
    let Ok(start) = EngineStart::initialize(&input, None);
    let started = EngineStart::run_effects(start, 1, &mut (), |(), effect| {
      assert_eq!(effect.renderer, Renderer::WebKit);
      Err(io::Error::from(io::ErrorKind::NotFound))
    });

    assert!(matches!(started, Ok(State::Failed(_))));
  }
}
