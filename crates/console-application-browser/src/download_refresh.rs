//! One scan is outstanding at a time. A change observed before it completes
//! calls for one follow-up scan, even if several changes arrive together.

use std::io;

use console_core_state_machine::{EffectDependencies, Machine, Never, Queue, Service};

use crate::download_catalog::Catalog;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
  Idle,
  Scanning,
  RescanNeeded,
}

pub(crate) enum Request {
  Refresh,
  Scanned(io::Result<Catalog>),
  QueueFailed,
}

pub(crate) enum Effect {
  Scan,
  Present(io::Result<Catalog>),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Scan => &[Service::FileSystem],
      Self::Present(_) => &[Service::Display],
    })
  }
}

pub(crate) struct DownloadRefresh;

impl Machine for DownloadRefresh {
  type Input = ();
  type State = State;
  type Request = Request;
  type Effect = Effect;

  fn initialize(
    _input: &(),
    _previous: Option<State>,
    _effects: &mut Queue<Effect>,
  ) -> Result<State, Never> {
    Ok(State::Idle)
  }

  fn handle(state: State, request: Request, effects: &mut Queue<Effect>) -> Result<State, Never> {
    Ok(match (state, request) {
      (State::Idle, Request::Refresh) => {
        let Ok(()) = effects.offer(Effect::Scan);

        State::Scanning
      }
      (State::Scanning | State::RescanNeeded, Request::Refresh) => State::RescanNeeded,
      (State::Scanning, Request::Scanned(result)) => {
        let Ok(()) = effects.offer(Effect::Present(result));

        State::Idle
      }
      (State::RescanNeeded, Request::Scanned(result)) => {
        let Ok(()) = effects.offer_all([Effect::Present(result), Effect::Scan]);

        State::Scanning
      }
      (State::Idle, Request::Scanned(_)) | (_, Request::QueueFailed) => State::Idle,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn complete() -> Result<Request, Never> {
    Ok(Request::Scanned(Err(io::Error::other(
      "sample scan result",
    ))))
  }

  #[test]
  fn multiple_changes_during_a_scan_queue_one_follow_up() {
    let Ok(initial) = DownloadRefresh::initial_transition(&(), None);
    let Ok(scanning) = DownloadRefresh::transition(initial.state, Request::Refresh);
    assert!(matches!(scanning.effects.as_slice(), [Effect::Scan]));
    let Ok(changed) = DownloadRefresh::transition(scanning.state, Request::Refresh);
    let Ok(changed_again) = DownloadRefresh::transition(changed.state, Request::Refresh);
    assert!(changed.effects.is_empty());
    assert!(changed_again.effects.is_empty());

    let Ok(report) = complete();
    let Ok(follow_up) = DownloadRefresh::transition(changed_again.state, report);
    assert!(matches!(
      follow_up.effects.as_slice(),
      [Effect::Present(_), Effect::Scan]
    ));
    let Ok(report_2) = complete();
    let Ok(finished) = DownloadRefresh::transition(follow_up.state, report_2);
    assert!(matches!(finished.effects.as_slice(), [Effect::Present(_)]));
    assert_eq!(finished.state, State::Idle);
  }

  #[test]
  fn a_queue_failure_leaves_the_next_refresh_free_to_start() {
    let Ok(failed) = DownloadRefresh::transition(State::Scanning, Request::QueueFailed);
    assert_eq!(failed.state, State::Idle);
    let Ok(report) = complete();
    let Ok(stale) = DownloadRefresh::transition(failed.state, report);
    assert!(stale.effects.is_empty());
    let Ok(retried) = DownloadRefresh::transition(stale.state, Request::Refresh);
    assert!(matches!(retried.effects.as_slice(), [Effect::Scan]));
  }
}
