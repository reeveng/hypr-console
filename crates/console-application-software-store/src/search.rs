//! Debounces queries and keeps one worker outstanding per package manager.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::time::Instant;

use console_core_state_machine::{EffectDependencies, Machine, Never, Queue, Service};
use console_package_manager::catalogue::{Manager, Matches};

use crate::DEBOUNCE;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchId(u64);

#[derive(Default)]
pub struct Schedule {
  query: String,
  id: SearchId,
  queued: BTreeSet<Manager>,
  running: BTreeMap<Manager, SearchId>,
  deadline: Option<Instant>,
}

impl Schedule {
  #[must_use]
  pub fn status(&self) -> Result<super::SearchStatus, Never> {
    let active = !self.queued.is_empty() || self.running.values().any(|id| *id == self.id);
    Ok(match active.then_some(()) {
      Some(()) => super::SearchStatus::Searching,
      None => super::SearchStatus::Settled,
    })
  }

  fn ready(&mut self, now: Instant) -> Result<(), Never> {
    match (self.deadline.is_none() && !self.queued.is_empty()).then_some(()) {
      Some(()) => self.deadline = Some(now),
      None => {}
    }

    Ok(())
  }

  fn poll(&mut self, now: Instant, effects: &mut Queue<Effect>) -> Result<(), Never> {
    match self.deadline.filter(|at| now >= *at) {
      Some(_) => self.deadline = None,
      None => return Ok(()),
    }

    for manager in std::mem::take(&mut self.queued) {
      match self.running.entry(manager) {
        std::collections::btree_map::Entry::Occupied(_) => {
          let _queued = self.queued.insert(manager);
        }
        std::collections::btree_map::Entry::Vacant(slot) => {
          let _ticket = slot.insert(self.id);
          let Ok(()) = effects.offer(Effect::Query {
            manager,
            id: self.id,
            query: self.query.clone(),
          });
        }
      };
    }

    Ok(())
  }
}

pub enum Failure {
  Launch(io::Error),
  Search(io::Error),
}

pub enum Request {
  Edit {
    query: String,
    managers: BTreeSet<Manager>,
    now: Instant,
  },
  Sources {
    active: BTreeSet<Manager>,
    added: BTreeSet<Manager>,
    now: Instant,
  },
  Expedite(Instant),
  Poll(Instant),
  Finished {
    manager: Manager,
    id: SearchId,
    now: Instant,
    result: Result<Matches, Failure>,
  },
}

pub enum Effect {
  Query {
    manager: Manager,
    id: SearchId,
    query: String,
  },
  Present {
    manager: Manager,
    result: Result<Matches, Failure>,
  },
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Query { .. } => &[Service::Process],
      Self::Present { .. } => &[Service::Display],
    })
  }
}

pub struct Search;

impl Machine for Search {
  type Input = ();
  type State = Schedule;
  type Request = Request;
  type Effect = Effect;

  fn initialize(
    _input: &(),
    _previous: Option<Schedule>,
    _effects: &mut Queue<Effect>,
  ) -> Result<Schedule, Never> {
    Ok(Schedule::default())
  }

  fn handle(
    mut state: Schedule,
    event: Request,
    effects: &mut Queue<Effect>,
  ) -> Result<Schedule, Never> {
    let Ok(()) = match event {
      Request::Edit {
        query,
        managers,
        now,
      } => {
        state.id = SearchId(state.id.0.wrapping_add(1));
        state.query = query;
        state.queued = managers;
        state.deadline = match (!state.queued.is_empty()).then_some(()) {
          Some(()) => match now.checked_add(DEBOUNCE) {
            Some(at) => Some(at),
            None => Some(now),
          },
          None => None,
        };
        Ok(())
      }
      Request::Sources { active, added, now } => {
        state.queued.retain(|manager| active.contains(manager));

        match state.query.as_str() {
          "" => {}
          _ => state.queued.extend(added),
        }

        state.ready(now)
      }
      Request::Expedite(now) => {
        match (!state.queued.is_empty()).then_some(()) {
          Some(()) => state.deadline = Some(now),
          None => {}
        }

        Ok(())
      }
      Request::Poll(now) => state.poll(now, effects),
      Request::Finished {
        manager,
        id,
        now,
        result,
      } => finish(&mut state, manager, id, now, result, effects),
    };
    Ok(state)
  }
}

fn finish(
  state: &mut Schedule,
  manager: Manager,
  id: SearchId,
  now: Instant,
  result: Result<Matches, Failure>,
  effects: &mut Queue<Effect>,
) -> Result<(), Never> {
  match (state.running.get(&manager) == Some(&id)).then_some(()) {
    Some(()) => {
      let _removed = state.running.remove(&manager);
    }
    None => return Ok(()),
  }

  match state.queued.contains(&manager).then_some(()) {
    Some(()) => {
      let Ok(()) = state.ready(now);
    }
    None => {}
  }

  match (state.id == id).then_some(()) {
    Some(()) => effects.offer(Effect::Present { manager, result }),
    None => Ok(()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn an_old_reply_releases_only_its_manager_and_never_presents_results() {
    let now = Instant::now();
    let previous = SearchId(1);
    let current = SearchId(2);
    let state = Schedule {
      query: "zen".to_owned(),
      id: current,
      queued: BTreeSet::from([Manager::Aur]),
      running: BTreeMap::from([(Manager::Aur, previous), (Manager::Nix, current)]),
      deadline: None,
    };
    let Ok(done) = Search::transition(
      state,
      Request::Finished {
        manager: Manager::Aur,
        id: previous,
        now,
        result: Ok(Matches::default()),
      },
    );
    assert!(done.effects.is_empty());
    let Ok(next) = Search::transition(done.state, Request::Poll(now));
    assert!(
      matches!(next.effects.as_slice(), [Effect::Query { manager: Manager::Aur, id, query }] if *id == current && query == "zen")
    );
    assert_eq!(next.state.running.get(&Manager::Nix), Some(&current));
    assert_eq!(next.state.running.get(&Manager::Aur), Some(&current));
  }
}
