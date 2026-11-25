//! The subprocesses and accepted results owned by one open Downloads card.

use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::thread::{self, JoinHandle};

use console_actor::Actor;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_state_machine::Machine;
use console_operating_system_linux::program_lifetime::{Forked, ProcessIdentity};
use console_operating_system_linux::program_runtime::commands::{self, Linux, Timing};
use console_program_contract::{Command, Effect, Event, ExitStatus};
use console_program_lifetime::Insistence;
use console_program_runtime::commands::{self as execution, Completion, Streams};

use crate::looking::{self, Looked};
use crate::standing::{
  Downloads, DownloadsEffect, DownloadsEvent, FIND, Search, SearchFailure, SearchId,
};
use crate::store::{self, Kind, Scratch};

mod lifecycle;

use lifecycle::{
  Destination, Effect as SearchEffect, Key, Lifecycle, Request as SearchRequest, State,
};

#[derive(Clone)]
pub(crate) struct Searches(SyncSender<Request>);

pub(crate) struct SearchOwner {
  sending: SyncSender<Request>,
  worker: Option<JoinHandle<()>>,
}

struct Start {
  panel: Actor<Downloads>,
  tab: u32,
  kind: Kind,
  query: Search,
}

enum Request {
  Start(Start),
  Cancel { tab: u32, id: SearchId },
  Ready,
  Close,
}

#[derive(Default)]
struct Workers {
  jobs: Vec<Task>,
}

struct Task {
  key: Key,
  identity: ProcessIdentity,
  worker: JoinHandle<()>,
  received: Receiver<Result<Looked, SearchFailure>>,
}

impl Task {
  fn cancel(&self) -> Result<(), Never> {
    let Ok(_reached) = self.identity.interrupt(Insistence::Insist);

    Ok(())
  }
}

impl Searches {
  pub(crate) fn open() -> Result<(Self, SearchOwner), Never> {
    let (sending, requests) = sync_channel(16);
    let wake = sending.clone();
    let worker = thread::spawn(move || {
      let Ok(()) = boot(requests, &wake);
    });

    Ok((
      Searches(sending.clone()),
      SearchOwner {
        sending,
        worker: Some(worker),
      },
    ))
  }

  pub(crate) fn start(
    &self,
    panel: &Actor<Downloads>,
    tab: u32,
    kind: Kind,
    query: Search,
  ) -> Result<(), Never> {
    let Ok(()) = requested(
      &self.0,
      Request::Start(Start {
        panel: panel.clone(),
        tab,
        kind,
        query,
      }),
    );

    console_panel::frames::tell(console_panel::frames::Notice::Rows)
  }

  pub(crate) fn cancel(&self, tab: u32, id: SearchId) -> Result<(), Never> {
    requested(&self.0, Request::Cancel { tab, id })
  }
}

impl SearchOwner {
  pub(crate) fn close(mut self) -> Result<(), Never> {
    self.stop_worker()
  }

  fn stop_worker(&mut self) -> Result<(), Never> {
    match self.worker.take() {
      Some(worker) => {
        let Ok(()) = requested(&self.sending, Request::Close);

        joined(worker)
      }
      None => Ok(()),
    }
  }
}

impl Drop for SearchOwner {
  fn drop(&mut self) {
    let Ok(()) = self.stop_worker();
  }
}

fn requested(sending: &SyncSender<Request>, request: Request) -> Result<(), Never> {
  match sending.send(request) {
    Ok(()) => {}
    Err(_closed) => console_core_logging::debug!("downloads: the search manager has stopped"),
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit055_no_unbounded_loop",
  expect(
    explicit055_no_unbounded_loop,
    reason = "one manager owns the jobs for an open Downloads card. The card owner sends Close and joins this worker before disposing of the surface"
  )
)]
fn boot(requests: Receiver<Request>, wake: &SyncSender<Request>) -> Result<(), Never> {
  let Ok(initial) = Lifecycle::initial_transition(&(), None);
  let mut state = initial.state;
  let mut workers = Workers::default();

  for request in requests.iter() {
    let input = match request {
      Request::Start(Start {
        panel,
        tab,
        kind,
        query,
      }) => Some(SearchRequest::Start {
        key: Key { tab, id: query.id },
        destination: Destination { panel, kind },
        word: query.word,
      }),
      Request::Cancel { tab, id } => Some(SearchRequest::Cancel(Key { tab, id })),
      Request::Ready => None,
      Request::Close => break,
    };

    match input {
      Some(command) => {
        let Ok(next) = drive(state, command, &mut workers, wake);

        state = next;
      }
      None => {}
    }

    let Ok(finished) = workers.finished();

    for report in finished {
      let Ok(next) = drive(state, report, &mut workers, wake);

      state = next;
    }
  }

  let Ok(_closed) = drive(state, SearchRequest::Close, &mut workers, wake);

  Ok(())
}

fn drive(
  current: State,
  input: SearchRequest,
  running: &mut Workers,
  wake: &SyncSender<Request>,
) -> Result<State, Never> {
  let driven =
    console_core_iteration::iterate((current, input, running), |(state, request, workers)| {
      let Ok(turn) = Lifecycle::transition(state, request);

      for effect in turn.effects {
        let Ok(reply) = workers.perform(effect, wake);

        match reply {
          Some(report) => return Ok(Step::Again((turn.state, report, workers))),
          None => {}
        }
      }

      Ok(Step::Halt(turn.state))
    });

  Ok(match driven {
    Ok(next) => next,
    Err(_endless) => State::Closed,
  })
}

impl Workers {
  fn perform(
    &mut self,
    effect: SearchEffect,
    wake: &SyncSender<Request>,
  ) -> Result<Option<SearchRequest>, Never> {
    Ok(match effect {
      SearchEffect::Launch { key, kind, word } => match self.start(key, kind, word, wake) {
        Ok(()) => None,
        Err(fault) => Some(SearchRequest::Finished {
          key,
          result: Err(SearchFailure::Process(fault.to_string())),
        }),
      },
      SearchEffect::Interrupt(key) => {
        let Ok(()) = self.cancel(key);

        None
      }
      SearchEffect::Publish {
        key,
        destination,
        result,
      } => {
        let Ok(()) = publish(
          &destination.panel,
          key.tab,
          destination.kind,
          key.id,
          result,
        );

        None
      }
      SearchEffect::ReapAll => {
        let Ok(()) = self.close();

        None
      }
    })
  }

  fn start(
    &mut self,
    key: Key,
    kind: Kind,
    word: String,
    wake: &SyncSender<Request>,
  ) -> std::io::Result<()> {
    let (child, scratch) = start_finder(kind, &word)?;
    let Ok(identity) = child.identity();
    let (completed, received) = sync_channel(1);
    let wake_2 = wake.clone();
    let worker = thread::Builder::new().spawn(move || {
      let _scratch = scratch;
      let completion = commands::finished(child, None);
      let result = reviewed(completion, &word);

      match completed.send(result) {
        Ok(()) => {
          let Ok(()) = ready(&wake_2);
        }
        Err(_closed) => {
          console_core_logging::trace!("downloads: closed before the search finished")
        }
      }
    })?;

    self.jobs.push(Task {
      key,
      identity,
      worker,
      received,
    });

    Ok(())
  }

  fn cancel(&self, key: Key) -> Result<(), Never> {
    match self.jobs.iter().find(|job| job.key == key) {
      Some(job) => job.cancel(),
      None => Ok(()),
    }
  }

  fn finished(&mut self) -> Result<Vec<SearchRequest>, Never> {
    let mut reports = Vec::new();

    for job in std::mem::take(&mut self.jobs) {
      let result = match job.received.try_recv() {
        Ok(result) => result,
        Err(TryRecvError::Empty) => {
          self.jobs.push(job);

          continue;
        }
        Err(TryRecvError::Disconnected) => Err(SearchFailure::Process(
          "the search worker stopped without an answer".to_string(),
        )),
      };
      let Ok(()) = joined(job.worker);
      reports.push(SearchRequest::Finished {
        key: job.key,
        result,
      });
    }

    Ok(reports)
  }

  fn close(&mut self) -> Result<(), Never> {
    for job in &self.jobs {
      let Ok(()) = job.cancel();
    }

    for job in std::mem::take(&mut self.jobs) {
      let Ok(()) = joined(job.worker);
    }

    Ok(())
  }
}

fn ready(wake: &SyncSender<Request>) -> Result<(), Never> {
  match wake.try_send(Request::Ready) {
    Ok(()) | Err(TrySendError::Full(_)) => return Ok(()),
    Err(TrySendError::Disconnected(_)) => {
      console_core_logging::trace!("downloads: the search manager has already stopped");
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the card joins its supervisor after Close. The supervisor joins only completed work or finder process groups it has cancelled, and workers publish once into their own one-result channels"
  )
)]
fn joined(worker: JoinHandle<()>) -> Result<(), Never> {
  match worker.join() {
    Ok(()) => {}
    Err(_panicked) => {
      console_core_logging::error!("downloads: a search worker stopped unexpectedly")
    }
  }

  Ok(())
}

fn start_finder(kind: Kind, word: &str) -> std::io::Result<(Forked, Scratch)> {
  let scratch = Scratch::make("download-search")?;
  let Ok(folder) = scratch.path();
  let at = folder
    .to_str()
    .ok_or_else(|| std::io::Error::other("the search scratch path is not UTF-8"))?;
  let Ok(flag) = kind.flag();
  let Ok(command) = Command::internal(FIND, &[flag, store::REPORT.spelling, at, "--", word]);
  let Ok(mut platform) = Linux::new(Timing::Background);
  let invocation = execution::resolve(&mut platform, &command)?;
  let child = platform.fork(&invocation, Streams::COLLECT)?;

  Ok((child, scratch))
}

fn reviewed(
  completion: std::io::Result<Completion>,
  word: &str,
) -> Result<Looked, SearchFailure> {
  let completed = completion.map_err(|fault| SearchFailure::Process(fault.to_string()))?;

  match completed.status {
    ExitStatus::Success => {
      let Ok(looked) = looking::parse(&completed.output);

      match ::core::primitive::bool::then_some(looked.asked == word, ()) {
        ::core::option::Option::Some(()) => Ok(looked),
        ::core::option::Option::None => Err(SearchFailure::InvalidReply),
      }
    }
    ExitStatus::Failure(_) | ExitStatus::Interrupt => {
      let detail = completed.error_output.trim();

      match ::core::primitive::bool::then_some(detail.is_empty(), ()) {
        ::core::option::Option::Some(()) => {
          Err(SearchFailure::Process("the finder stopped".to_string()))
        }
        ::core::option::Option::None => Err(SearchFailure::Process(detail.to_string())),
      }
    }
  }
}

fn publish(
  panel: &Actor<Downloads>,
  tab: u32,
  kind: Kind,
  id: SearchId,
  result: Result<Looked, SearchFailure>,
) -> Result<(), Never> {
  let effects = match panel.send(Event::Custom(DownloadsEvent::Landed { tab, id, result })) {
    Ok(effects) => effects,
    Err(_closed) => {
      console_core_logging::debug!("downloads: the card closed before its search was accepted");

      return Ok(());
    }
  };

  for effect in effects {
    match effect {
      Effect::Custom(DownloadsEffect::Retain(looked)) => {
        let Ok(()) = keep_result(kind, &looked);
      }
      Effect::Custom(DownloadsEffect::Refresh) => {
        let Ok(()) = console_panel::frames::tell(console_panel::frames::Notice::Rows);
      }
      Effect::Custom(
        DownloadsEffect::Replace(_)
        | DownloadsEffect::Notify(_)
        | DownloadsEffect::ForgetTyping
        | DownloadsEffect::Search { .. }
        | DownloadsEffect::Cancel { .. },
      )
      | Effect::Run(_)
      | Effect::Stream(_)
      | Effect::Fork(_)
      | Effect::Interrupt(_)
      | Effect::Prompt(_)
      | Effect::Spawn(_)
      | Effect::Subscribe(_)
      | Effect::Unsubscribe(_)
      | Effect::Write(_)
      | Effect::Notify(_)
      | Effect::Print(_)
      | Effect::Stop(_) => {}
    }
  }

  Ok(())
}

fn keep_result(kind: Kind, looked: &Looked) -> Result<(), Never> {
  let Ok(cache) = store::cache();

  match cache {
    Some(at) => match store::keep(&at, kind, looked) {
      Ok(()) => {}
      Err(fault) => console_core_logging::warn!("downloads: keeping the last search: {fault}"),
    },
    None => {}
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_external_programs::Program;
  use console_operating_system_linux::program_lifetime::forked;
  use std::time::Duration;

  fn waiting(key: Key) -> std::io::Result<(Task, Receiver<ExitStatus>)> {
    let Ok(mut command) = Program::Sleep.command();
    command.arg("5");
    let child = forked(&mut command)?;
    let Ok(identity) = child.identity();
    let (send, receive) = sync_channel(1);
    let (_unused, received) = sync_channel(1);
    let worker = thread::spawn(move || match commands::finished(child, None) {
      Ok(done) => match send.send(done.status) {
        Ok(()) => {}
        Err(_ended) => eprintln!("the fixture ended before its job reported"),
      },
      Err(fault) => eprintln!("the cancellation fixture failed: {fault}"),
    });

    Ok((
      Task {
        key,
        identity,
        worker,
        received,
      },
      receive,
    ))
  }

  #[test]
  fn cancellation_is_exact_and_closing_reaps_the_other_searches()
  -> Result<(), Box<dyn std::error::Error>> {
    let booted = console_actor::boot::<Downloads>(())?;
    let first_key = Key {
      tab: 0,
      id: SearchId(1),
    };
    let fresh_key = Key {
      tab: 0,
      id: SearchId(2),
    };
    let other_key = Key {
      tab: 1,
      id: SearchId(1),
    };
    let (first, heard_first) = waiting(first_key)?;
    let (fresh, heard_fresh) = waiting(fresh_key)?;
    let (other, heard_other) = waiting(other_key)?;
    let mut workers = Workers {
      jobs: vec![first, fresh, other],
    };
    let Ok(initial) = Lifecycle::initial_transition(&(), None);
    let mut state = initial.state;
    let (wake, _requests) = sync_channel(1);

    for key in [first_key, fresh_key, other_key] {
      let Ok(launched) = Lifecycle::transition(
        state,
        SearchRequest::Start {
          key,
          destination: Destination {
            panel: booted.actor.clone(),
            kind: Kind::Sound,
          },
          word: "fixture".to_string(),
        },
      );
      assert!(matches!(
        launched.effects.as_slice(),
        [SearchEffect::Launch { .. }]
      ));
      state = launched.state;
    }

    let Ok(cancelling) = drive(state, SearchRequest::Cancel(first_key), &mut workers, &wake);
    let cancelled = heard_first.recv_timeout(Duration::from_secs(2))?;

    assert_eq!(cancelled, ExitStatus::Failure(None));
    assert!(matches!(heard_fresh.try_recv(), Err(TryRecvError::Empty)));
    assert!(matches!(heard_other.try_recv(), Err(TryRecvError::Empty)));

    let Ok(closed) = drive(cancelling, SearchRequest::Close, &mut workers, &wake);
    assert!(matches!(closed, State::Closed));
    assert!(workers.jobs.is_empty());
    let followed = heard_fresh.try_recv()?;
    let independent = heard_other.try_recv()?;

    assert_eq!(followed, ExitStatus::Failure(None));
    assert_eq!(independent, ExitStatus::Failure(None));

    Ok(())
  }

  #[test]
  fn only_the_pending_search_id_can_publish_a_result() -> Result<(), Box<dyn std::error::Error>> {
    let booted = console_actor::boot::<Downloads>(())?;
    let key = Key {
      tab: 0,
      id: SearchId(2),
    };
    let Ok(initial) = Lifecycle::initial_transition(&(), None);
    let Ok(launched) = Lifecycle::transition(
      initial.state,
      SearchRequest::Start {
        key,
        destination: Destination {
          panel: booted.actor,
          kind: Kind::Sound,
        },
        word: "query".to_string(),
      },
    );
    let Ok(stale) = Lifecycle::transition(
      launched.state,
      SearchRequest::Finished {
        key: Key {
          id: SearchId(1),
          ..key
        },
        result: Err(SearchFailure::InvalidReply),
      },
    );
    assert!(stale.effects.is_empty());

    let Ok(first) = Lifecycle::transition(
      stale.state,
      SearchRequest::Finished {
        key,
        result: Err(SearchFailure::InvalidReply),
      },
    );
    assert!(
      matches!(first.effects.as_slice(), [SearchEffect::Publish { key: published, .. }] if *published == key)
    );

    let Ok(duplicate) = Lifecycle::transition(
      first.state,
      SearchRequest::Finished {
        key,
        result: Err(SearchFailure::InvalidReply),
      },
    );
    assert!(duplicate.effects.is_empty());

    Ok(())
  }
}
