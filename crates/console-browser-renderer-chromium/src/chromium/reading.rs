//! Correlates the three DevTools replies needed for one reader request.

use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};
use serde_json::Value;

#[derive(Clone, Copy)]
enum Stage {
  MainFrame,
  IsolatedWorld,
  Text,
}

struct Pending {
  id: u64,
  stage: Stage,
  command: Option<u64>,
}

#[derive(Default)]
pub(super) struct State(Option<Pending>);

impl State {
  #[must_use]
  pub(super) fn awaiting(&self) -> Result<Option<u64>, Never> {
    Ok(self.0.as_ref().and_then(|pending| pending.command))
  }
}

pub(super) enum Request {
  Start(u64),
  Sent(u64),
  Reply { command: u64, result: Option<Value> },
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Effect {
  FindMainFrame,
  CreateWorld(Value),
  ReadText(Value),
  Publish { id: u64, text: String },
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::FindMainFrame | Self::CreateWorld(_) | Self::ReadText(_) => &[Service::Process],
      Self::Publish { .. } => &[Service::Events],
    })
  }
}

pub(super) struct Reading;

impl SingleEffectMachine for Reading {
  type Input = ();
  type State = State;
  type Request = Request;
  type Effect = Effect;

  fn initialize(_: &(), _: Option<State>) -> Result<SingleTransition<State, Effect>, Never> {
    Ok(SingleTransition {
      state: State::default(),
      effect: None,
    })
  }

  fn handle(
    mut state: State,
    request: Request,
  ) -> Result<SingleTransition<State, Effect>, Never> {
    match request {
      Request::Start(id) => sending(id, Stage::MainFrame, Effect::FindMainFrame),
      Request::Sent(command) => {
        match state.0.as_mut() {
          Some(Pending {
            command: slot @ None,
            ..
          }) => *slot = Some(command),
          Some(Pending {
            command: Some(_), ..
          })
          | None => {}
        }

        Ok(SingleTransition {
          state,
          effect: None,
        })
      }
      Request::Reply { command, result } => {
        let Ok(awaiting) = state.awaiting();

        match (state.0, awaiting == Some(command)) {
          (Some(pending), true) => replied(pending, result.as_ref()),
          (pending, false) | (pending @ None, true) => Ok(SingleTransition {
            state: State(pending),
            effect: None,
          }),
        }
      }
    }
  }
}

fn replied(
  pending: Pending,
  result: Option<&Value>,
) -> Result<SingleTransition<State, Effect>, Never> {
  let id = pending.id;

  match pending.stage {
    Stage::MainFrame => {
      match result.and_then(|value| value.pointer("/frameTree/frame/id")) {
        Some(frame) => {
          return sending(id, Stage::IsolatedWorld, Effect::CreateWorld(frame.clone()));
        }
        ::core::option::Option::None => {}
      };
    }
    Stage::IsolatedWorld => {
      match result.and_then(|value| value.get("executionContextId")) {
        Some(context) => {
          return sending(id, Stage::Text, Effect::ReadText(context.clone()));
        }
        ::core::option::Option::None => {}
      };
    }
    Stage::Text => {
      let text = match result
        .and_then(|value| value.pointer("/result/value"))
        .and_then(Value::as_str)
      {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => "",
      };
      return published(id, text.to_owned());
    }
  }

  published(id, String::new())
}

fn sending(
  id: u64,
  stage: Stage,
  effect: Effect,
) -> Result<SingleTransition<State, Effect>, Never> {
  Ok(SingleTransition {
    state: State(Some(Pending {
      id,
      stage,
      command: None,
    })),
    effect: Some(effect),
  })
}

fn published(id: u64, text: String) -> Result<SingleTransition<State, Effect>, Never> {
  Ok(SingleTransition {
    state: State::default(),
    effect: Some(Effect::Publish { id, text }),
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn a_late_reply_cannot_advance_a_new_reader_request() {
    let Ok(first) = Reading::handle(State::default(), Request::Start(1));
    let Ok(sent) = Reading::handle(first.state, Request::Sent(10));
    let Ok(second) = Reading::handle(sent.state, Request::Start(2));
    let Ok(sent_2) = Reading::handle(second.state, Request::Sent(11));
    let Ok(late) = Reading::handle(
      sent_2.state,
      Request::Reply {
        command: 10,
        result: None,
      },
    );

    assert_eq!(late.effect, None);
    assert_eq!(late.state.awaiting(), Ok(Some(11)));
    let Ok(failed) = Reading::handle(
      late.state,
      Request::Reply {
        command: 11,
        result: None,
      },
    );
    assert_eq!(
      failed.effect,
      Some(Effect::Publish {
        id: 2,
        text: String::new()
      })
    );
    assert_eq!(failed.state.awaiting(), Ok(None));
  }

  #[test]
  fn the_reader_publishes_text_after_all_three_replies() {
    let Ok(started) = Reading::handle(State::default(), Request::Start(7));
    assert_eq!(started.effect, Some(Effect::FindMainFrame));
    let mut state = started.state;

    for (command, result, effect) in [
      (
        10,
        json!({ "frameTree": { "frame": { "id": "page" } }}),
        Effect::CreateWorld(json!("page")),
      ),
      (
        11,
        json!({ "executionContextId": 42 }),
        Effect::ReadText(json!(42)),
      ),
      (
        12,
        json!({ "result": { "value": "article" }}),
        Effect::Publish {
          id: 7,
          text: <::std::string::String as ::core::convert::From<&::core::primitive::str>>::from(
            "article",
          ),
        },
      ),
    ] {
      let Ok(sent) = Reading::handle(state, Request::Sent(command));
      let Ok(next) = Reading::handle(
        sent.state,
        Request::Reply {
          command,
          result: Some(result),
        },
      );
      assert_eq!(next.effect, Some(effect));
      state = next.state;
    }

    assert_eq!(state.awaiting(), Ok(None));
  }
}
