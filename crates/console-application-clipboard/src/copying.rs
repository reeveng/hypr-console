//! Publish an explicit copy before accepting it into session history.

use console_core_state_machine::{EffectDependencies, Machine, Never, Queue, Service};

use crate::history::{Capture, Contents, Format};
use crate::ipc::{MAX_PNG_REQUEST, Reply};

pub(crate) enum Request {
  Copy(Contents),
  Published(Reply),
}

pub(crate) enum State {
  Ready(Capture),
  Publishing(Option<Contents>),
  Finished(Reply),
}

pub(crate) enum Effect {
  Publish(Contents),
  Remember(Contents),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Publish(_) => &[Service::Clipboard],
      Self::Remember(_) => &[],
    })
  }
}

pub(crate) struct Copying;

impl Machine for Copying {
  type Input = Capture;
  type State = State;
  type Request = Request;
  type Effect = Effect;

  fn initialize(
    capture: &Capture,
    _: Option<State>,
    _: &mut Queue<Effect>,
  ) -> Result<State, Never> {
    Ok(State::Ready(*capture))
  }

  fn handle(state: State, request: Request, effects: &mut Queue<Effect>) -> Result<State, Never> {
    match (state, request) {
      (State::Ready(capture), Request::Copy(contents)) => {
        let Ok(length) = contents.length();
        let Ok(history_limit) = contents.format.limit();
        let limit = match contents.format {
          Format::Text => history_limit,
          Format::Image(_) => MAX_PNG_REQUEST,
        };

        match { ::core::primitive::bool::then_some(length == 0 || length > limit, ()) } {
          ::core::option::Option::Some(()) => {
            return Ok(State::Finished(Reply::Unavailable));
          }
          ::core::option::Option::None => {}
        }

        let remembered = match (capture, length <= history_limit) {
          (Capture::Recording, true) => Some(contents.clone()),
          (Capture::Recording, false) | (Capture::Paused, _) => None,
        };
        let Ok(()) = effects.offer(Effect::Publish(contents));
        Ok(State::Publishing(remembered))
      }
      (State::Publishing(remembered), Request::Published(reply)) => {
        match (&reply, remembered) {
          (Reply::Finished, Some(contents)) => {
            let Ok(()) = effects.offer(Effect::Remember(contents));
          }
          (Reply::Finished, None)
          | (Reply::Text(_) | Reply::List { .. } | Reply::Unavailable, _) => {}
        }

        Ok(State::Finished(reply))
      }
      (state_2, _) => Ok(state_2),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_bytes::Bytes;

  #[test]
  fn only_a_successful_publication_during_recording_enters_history() {
    for (capture, reply, remembers) in [
      (Capture::Recording, Reply::Finished, true),
      (Capture::Recording, Reply::Unavailable, false),
      (Capture::Paused, Reply::Finished, false),
    ] {
      let Ok(initial) = Copying::initial_transition(&capture, None);
      let Ok(copying) = Copying::transition(
        initial.state,
        Request::Copy(Contents {
          format: Format::Text,
          bytes: Bytes::from(b"sample".to_vec()),
        }),
      );
      assert!(matches!(copying.effects.as_slice(), [Effect::Publish(_)]));
      let Ok(services) = copying.services();
      assert_eq!(
        services.into_iter().collect::<Vec<_>>(),
        [Service::Clipboard]
      );
      let Ok(finished) = Copying::transition(copying.state, Request::Published(reply));
      assert_eq!(
        matches!(finished.effects.as_slice(), [Effect::Remember(_)]),
        remembers,
      );
    }
  }
}
