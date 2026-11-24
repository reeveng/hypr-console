//! Search replies are routed by tab and search ID. Process handles stay with
//! the interpreter, which cancels and reaps them when the card closes.

use std::collections::{BTreeMap, btree_map::Entry};

use console_actor::Actor;
use console_core_state_machine::{EffectDependencies, Machine, Never, Queue, Service};

use crate::looking::Looked;
use crate::standing::{Downloads, SearchFailure, SearchId};
use crate::store::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Key {
  pub(super) tab: u32,
  pub(super) id: SearchId,
}

pub(super) struct Destination {
  pub(super) panel: Actor<Downloads>,
  pub(super) kind: Kind,
}

enum Status {
  Running,
  Cancelling,
}

pub(super) struct Pending {
  destination: Destination,
  status: Status,
}

pub(super) enum State {
  Open(BTreeMap<Key, Pending>),
  Closed,
}

pub(super) enum Request {
  Start {
    key: Key,
    destination: Destination,
    word: String,
  },
  Cancel(Key),
  Finished {
    key: Key,
    result: Result<Looked, SearchFailure>,
  },
  Close,
}

pub(super) enum Effect {
  Launch {
    key: Key,
    kind: Kind,
    word: String,
  },
  Interrupt(Key),
  Publish {
    key: Key,
    destination: Destination,
    result: Result<Looked, SearchFailure>,
  },
  ReapAll,
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Launch { .. } | Self::Interrupt(_) | Self::ReapAll => &[Service::Process],
      Self::Publish { .. } => &[],
    })
  }
}

pub(super) struct Lifecycle;

impl Machine for Lifecycle {
  type Input = ();
  type State = State;
  type Request = Request;
  type Effect = Effect;

  fn initialize(
    _input: &(),
    _previous: Option<State>,
    _effects: &mut Queue<Effect>,
  ) -> Result<State, Never> {
    Ok(State::Open(BTreeMap::new()))
  }

  fn handle(state: State, request: Request, effects: &mut Queue<Effect>) -> Result<State, Never> {
    match (state, request) {
      (State::Closed, _) => Ok(State::Closed),
      (State::Open(_), Request::Close) => {
        let Ok(()) = effects.offer(Effect::ReapAll);

        Ok(State::Closed)
      }
      (State::Open(mut pending), input) => {
        let Ok(effect) = update(&mut pending, input);
        let Ok(()) = effects.offer_all(effect);

        Ok(State::Open(pending))
      }
    }
  }
}

fn update(
  pending: &mut BTreeMap<Key, Pending>,
  request: Request,
) -> Result<Option<Effect>, Never> {
  Ok(match request {
    Request::Start {
      key,
      destination,
      word,
    } => match pending.entry(key) {
      Entry::Occupied(_) => None,
      Entry::Vacant(entry) => {
        let kind = destination.kind;
        entry.insert(Pending {
          destination,
          status: Status::Running,
        });

        Some(Effect::Launch { key, kind, word })
      }
    },
    Request::Cancel(key) => match pending.get_mut(&key) {
      Some(Pending {
        status: status @ Status::Running,
        ..
      }) => {
        *status = Status::Cancelling;

        Some(Effect::Interrupt(key))
      }
      Some(Pending {
        status: Status::Cancelling,
        ..
      })
      | None => None,
    },
    Request::Finished { key, result } => pending.remove(&key).map(|job| Effect::Publish {
      key,
      destination: job.destination,
      result,
    }),
    Request::Close => None,
  })
}
