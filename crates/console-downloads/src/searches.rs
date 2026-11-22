//! The subprocesses and accepted results owned by one open Downloads card.

use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::thread::{self, JoinHandle};

use console_actor::Actor;
use console_core_never::Never;
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
struct Running {
  jobs: Vec<Job>,
}

struct Job {
  panel: Actor<Downloads>,
  tab: u32,
  kind: Kind,
  id: SearchId,
  identity: ProcessIdentity,
  worker: JoinHandle<()>,
  received: Receiver<Result<Looked, SearchFailure>>,
}

impl Job {
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
      let Ok(()) = supervise(requests, &wake);
    });

    Ok((
      Searches(sending.clone()),
      SearchOwner { sending, worker: Some(worker) },
    ))
  }

  pub(crate) fn start(
    &self,
    panel: &Actor<Downloads>,
    tab: u32,
    kind: Kind,
    query: Search,
  ) -> Result<(), Never> {
    let Ok(()) = requested(&self.0, Request::Start(Start {
      panel: panel.clone(),
      tab,
      kind,
      query,
    }));

    console_panel::frames::tell(console_panel::frames::Notice::Rows)
  }

  pub(crate) fn cancel(&self, tab: u32, id: SearchId) -> Result<(), Never> {
    requested(&self.0, Request::Cancel { tab, id })
  }
}

impl SearchOwner {
  pub(crate) fn close(mut self) -> Result<(), Never> {
    self.shutdown()
  }

  fn shutdown(&mut self) -> Result<(), Never> {
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
    let Ok(()) = self.shutdown();
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
fn supervise(requests: Receiver<Request>, wake: &SyncSender<Request>) -> Result<(), Never> {
  let mut running = Running::default();

  for request in requests.iter() {
    match request {
      Request::Start(start) => {
        let Ok(()) = running.start(start, wake);
      }
      Request::Cancel { tab, id } => {
        let Ok(()) = running.cancel(tab, id);
      }
      Request::Ready => {}
      Request::Close => return running.close(),
    }

    let Ok(()) = running.deliver();
  }

  running.close()
}

impl Running {
  fn start(&mut self, start: Start, wake: &SyncSender<Request>) -> Result<(), Never> {
    let Start { panel, tab, kind, query } = start;
    let id = query.id;
    let (child, scratch) = match start_finder(kind, &query.word) {
      Ok(started) => started,
      Err(fault) => {
        return publish(&panel, tab, kind, id, Err(SearchFailure::Process(fault.to_string())));
      }
    };
    let Ok(identity) = child.identity();
    let (completed, received) = sync_channel(1);
    let wake_2 = wake.clone();
    let worker = thread::Builder::new().spawn(move || {
      let _scratch = scratch;
      let completion = commands::finished(child, None);
      let result = reviewed(completion, &query.word);

      match completed.send(result) {
        Ok(()) => {
          let Ok(()) = ready(&wake_2);
        }
        Err(_closed) => console_core_logging::trace!("downloads: closed before the search finished"),
      }
    });
    let waiting = match worker {
      Ok(waiting) => waiting,
      Err(fault) => {
        return publish(&panel, tab, kind, id, Err(SearchFailure::Process(fault.to_string())));
      }
    };

    self.jobs.push(Job { panel, tab, kind, id, identity, worker: waiting, received });

    Ok(())
  }

  fn cancel(&self, tab: u32, id: SearchId) -> Result<(), Never> {
    match self.jobs.iter().find(|job| job.tab == tab && job.id == id) {
      Some(job) => job.cancel(),
      None => Ok(()),
    }
  }

  fn deliver(&mut self) -> Result<(), Never> {
    for job in std::mem::take(&mut self.jobs) {
      let result = match job.received.try_recv() {
        Ok(result) => result,
        Err(TryRecvError::Empty) => {
          self.jobs.push(job);

          continue;
        }
        Err(TryRecvError::Disconnected) => {
          Err(SearchFailure::Process("the search worker stopped without an answer".to_string()))
        }
      };
      let Ok(()) = joined(job.worker);
      let Ok(()) = publish(&job.panel, job.tab, job.kind, job.id, result);
    }

    Ok(())
  }

  fn close(self) -> Result<(), Never> {
    for job in &self.jobs {
      let Ok(()) = job.cancel();
    }

    for job in self.jobs {
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
    Err(_panicked) => console_core_logging::error!("downloads: a search worker stopped unexpectedly"),
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
  let platform = Linux {
    timing: Timing::Background,
  };
  let invocation = execution::resolve(&platform, &command)?;
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
        ::core::option::Option::Some(()) => Err(SearchFailure::Process("the finder stopped".to_string())),
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
      Effect::Custom(DownloadsEffect::Keep(looked)) => {
        let Ok(()) = keep_result(kind, &looked);
      }
      Effect::Custom(DownloadsEffect::Refresh) => {
        let Ok(()) = console_panel::frames::tell(console_panel::frames::Notice::Rows);
      }
      Effect::Custom(
        DownloadsEffect::Replace(_)
        | DownloadsEffect::Note(_)
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

  fn waiting(
    panel: Actor<Downloads>,
    tab: u32,
    id: SearchId,
  ) -> std::io::Result<(Job, Receiver<ExitStatus>)> {
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
      Job {
        panel,
        tab,
        kind: Kind::Sound,
        id,
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
    let (first, heard_first) = waiting(booted.actor.clone(), 0, SearchId(1))?;
    let (fresh, heard_fresh) = waiting(booted.actor.clone(), 0, SearchId(2))?;
    let (other, heard_other) = waiting(booted.actor, 1, SearchId(1))?;
    let searches = Running { jobs: vec![first, fresh, other] };

    searches.cancel(0, SearchId(1))?;
    let cancelled = heard_first.recv_timeout(Duration::from_secs(2))?;

    assert_eq!(cancelled, ExitStatus::Failure(None));
    assert!(matches!(heard_fresh.try_recv(), Err(TryRecvError::Empty)));
    assert!(matches!(heard_other.try_recv(), Err(TryRecvError::Empty)));

    searches.close()?;
    let followed = heard_fresh.try_recv()?;
    let independent = heard_other.try_recv()?;

    assert_eq!(followed, ExitStatus::Failure(None));
    assert_eq!(independent, ExitStatus::Failure(None));

    Ok(())
  }
}
