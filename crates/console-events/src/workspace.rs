//! One compositor subscription and one capture in flight for workspace clients.

use std::fs::File;
use std::io::{self, Read, Write};
use std::num::NonZeroU16;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::{Duration, Instant};

use console_actor::{Actor, Booted, Completions, Scope, boot_buffered};
use console_compositor::socket::{self, Socket};
use console_compositor::workspace_snapshot::{self, QueryPlan, Snapshot};
use console_concurrency::Latest;
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::fixed_index;
use console_core_reconnect::{Round, keep};
use console_core_state_machine::{EffectDependencies, Machine, Queue, Service};
use console_waiting::latch;
use rustix::event::{PollFd, PollFlags, Timespec, poll};

use crate::ipc::{self, Workspace};

const INPUT_BYTES: u64 = 16 << 10;
const TRANSITION_TIME: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ticket {
  generation: u64,
  operation: u64,
}

#[derive(Debug, PartialEq)]
enum Phase {
  Offline,
  Opening,
  Listening,
  Fetching { ticket: Ticket, changed: Changed },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Changed {
  Yes,
  No,
}

struct State {
  phase: Phase,
  generation: u64,
  revision: u64,
  operation: u64,
}

enum Request {
  Retry,
  Subscribed,
  Changed,
  Captured(Ticket, Snapshot),
  CaptureFailed(Ticket),
  Lost,
}

#[derive(Debug, PartialEq)]
enum Effect {
  Connect,
  Capture(Ticket),
  Publish(Workspace),
  Reconnect,
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Connect | Self::Capture(_) | Self::Reconnect => &[Service::Compositor],
      Self::Publish(_) => &[Service::Events],
    })
  }
}

struct Source;

impl State {
  fn fetch(&mut self, effects: &mut Queue<Effect>) -> Result<(), Never> {
    self.operation = self.operation.saturating_add(1);
    let ticket = Ticket {
      generation: self.generation,
      operation: self.operation,
    };
    self.phase = Phase::Fetching {
      ticket,
      changed: Changed::No,
    };

    effects.offer(Effect::Capture(ticket))
  }
}

impl Machine for Source {
  type Input = ();
  type State = State;
  type Request = Request;
  type Effect = Effect;

  fn initialize(_: &(), _: Option<State>, _: &mut Queue<Effect>) -> Result<State, Never> {
    Ok(State {
      phase: Phase::Offline,
      generation: 0,
      revision: 0,
      operation: 0,
    })
  }

  fn handle(state: State, request: Request, effects: &mut Queue<Effect>) -> Result<State, Never> {
    state.advance(request, effects)
  }
}

impl State {
  fn advance(mut self, request: Request, effects: &mut Queue<Effect>) -> Result<Self, Never> {
    match (&mut self.phase, request) {
      (Phase::Offline, Request::Retry) => {
        self.phase = Phase::Opening;
        let Ok(()) = effects.offer(Effect::Connect);
      }
      (Phase::Opening, Request::Subscribed) => {
        self.generation = self.generation.saturating_add(1);
        self.revision = 0;
        let Ok(()) = self.fetch(effects);
      }
      (Phase::Listening, Request::Changed) => {
        let Ok(()) = self.fetch(effects);
      }
      (Phase::Fetching { changed, .. }, Request::Changed) => *changed = Changed::Yes,
      (Phase::Fetching { ticket, changed }, Request::Captured(receipt, snapshot)) => {
        let current = (*ticket == receipt).then_some(*changed);

        match current {
          Some(changed_2) => {
            let Ok(()) = self.captured(snapshot, changed_2, effects);
          }
          None => {}
        }
      }
      (Phase::Fetching { ticket, .. }, Request::CaptureFailed(receipt)) => {
        match (*ticket == receipt).then_some(()) {
          Some(()) => {
            let Ok(()) = self.lost(effects);
            let Ok(()) = effects.offer(Effect::Reconnect);
          }
          None => {}
        }
      }
      (Phase::Opening | Phase::Listening | Phase::Fetching { .. }, Request::Lost) => {
        let Ok(()) = self.lost(effects);
      }
      (_, _) => {}
    }

    Ok(self)
  }

  fn captured(
    &mut self,
    snapshot: Snapshot,
    changed: Changed,
    effects: &mut Queue<Effect>,
  ) -> Result<(), Never> {
    self.phase = Phase::Listening;
    self.revision = self.revision.saturating_add(1);
    let Ok(()) = effects.offer(Effect::Publish(Workspace::Fresh {
      generation: self.generation,
      revision: self.revision,
      snapshot,
    }));

    match changed {
      Changed::Yes => self.fetch(effects),
      Changed::No => Ok(()),
    }
  }

  fn lost(&mut self, effects: &mut Queue<Effect>) -> Result<(), Never> {
    self.phase = Phase::Offline;

    effects.offer(Effect::Publish(Workspace::Stale))
  }
}

struct Capture {
  at: PathBuf,
  ticket: Ticket,
}

struct Captured {
  ticket: Ticket,
  result: Result<Snapshot, workspace_snapshot::Error>,
}

pub(crate) struct Subscription {
  pub(crate) updates: Receiver<Published>,
  _running: File,
}

pub(crate) struct Published {
  pub(crate) payload: String,
  pub(crate) fresh_at: Option<Instant>,
}

struct Work {
  capture: SyncSender<Capture>,
  captured: Receiver<Captured>,
  ready: OwnedFd,
  worker: Latest,
}

struct Driver {
  actor: Actor<Source>,
  _scope: Scope,
  completions: Completions<(), Effect>,
  work: Work,
  updates: SyncSender<Published>,
  wake: File,
  stopping: OwnedFd,
}

impl Subscription {
  pub(crate) fn start(wake: File) -> Result<Self, Box<dyn std::error::Error>> {
    let (tell, updates) = sync_channel(4);
    let stop = latch::pipe()?;
    let Booted {
      actor,
      scope,
      effects: _initial,
    } = boot_buffered::<Source>((), NonZeroU16::MIN)?;
    let Ok(completions) = Completions::new(NonZeroU16::MIN);
    let work = Work::start()?;
    let mut driver = Driver {
      actor,
      _scope: scope,
      completions,
      work,
      updates: tell,
      wake,
      stopping: stop.waiting,
    };
    let Ok(()) = keep(move || {
      let Ok(round) = driver.round();

      round
    });

    Ok(Self {
      updates,
      _running: stop.saying,
    })
  }
}

impl Work {
  fn start() -> io::Result<Self> {
    let ready = latch::pipe()?;
    let (capture, tasks) = sync_channel::<Capture>(1);
    let (tell, captured) = sync_channel(2);
    let mut wake = ready.saying;
    let worker = Latest::start("workspace snapshot capture".to_owned(), move || {
      let task = match tasks.try_recv() {
        Ok(task) => task,
        Err(_no_capture_waits) => return Ok(()),
      };
      let result = workspace_snapshot::capture_at(&task.at, QueryPlan::Batch)
        .map(|(snapshot, _timing)| snapshot);

      match tell.send(Captured {
        ticket: task.ticket,
        result,
      }) {
        Ok(()) => {
          let _notified_or_already_pending = wake.write(&[1]);
        }
        Err(_source_has_ended) => return Ok(()),
      }

      Ok(())
    })?;

    Ok(Self {
      capture,
      captured,
      worker,
      ready: ready.waiting,
    })
  }

  fn request(&self, at: &Path, ticket: Ticket) -> io::Result<()> {
    self
      .capture
      .try_send(Capture {
        at: at.to_path_buf(),
        ticket,
      })
      .map_err(|error| io::Error::other(error.to_string()))?;
    self
      .worker
      .schedule()
      .map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))
  }
}

type Failure = Box<dyn std::error::Error>;

enum Turn {
  Continue,
  Retry,
  Stop,
}

impl Driver {
  fn round(&mut self) -> Result<Round, Never> {
    let mut stop = [PollFd::new(&self.stopping, PollFlags::IN)];
    let immediately = Timespec {
      tv_sec: 0,
      tv_nsec: 0,
    };

    match poll(&mut stop, Some(&immediately)) {
      Ok(0) => {}
      Ok(_stopped) => return Ok(Round::Finished),
      Err(_stop_descriptor_failed) => return Ok(Round::Finished),
    }

    let ended = self.listen();

    match ended {
      Ok(Round::Finished) => return Ok(Round::Finished),
      Ok(Round::Another) => {}
      Err(fault) => console_core_logging::debug!("workspace snapshot source: {fault}"),
    }

    Ok(match self.run_effects(Request::Lost, None) {
      Ok(_no_new_connection) => Round::Another,
      Err(_cannot_report_the_loss) => Round::Finished,
    })
  }

  fn listen(&mut self) -> Result<Round, Failure> {
    let opened = self.run_effects(Request::Retry, None)?;
    let stream = opened.ok_or("missing connection")?;
    let at = socket::socket(Socket::Requests)?;
    let _no_new_connection = self.run_effects(Request::Subscribed, Some(&at))?;

    let listened = iterate(
      (self, stream, Vec::new()),
      |(driver, mut stream_2, mut frame)| {
        let turn = match driver.turn(&mut stream_2, &mut frame, &at) {
          Ok(turn) => turn,
          Err(fault) => return Ok(Step::Halt(Err(fault))),
        };

        Ok(match turn {
          Turn::Continue => Step::Again((driver, stream_2, frame)),
          Turn::Retry => Step::Halt(Ok(Round::Another)),
          Turn::Stop => Step::Halt(Ok(Round::Finished)),
        })
      },
    );
    let round = listened.map_err(|Endless| Failure::from(Endless))?;

    round
  }

  fn turn(
    &mut self,
    stream: &mut UnixStream,
    frame: &mut Vec<u8>,
    at: &Path,
  ) -> Result<Turn, Failure> {
    let mut sockets = [
      PollFd::new(&self.stopping, PollFlags::IN),
      PollFd::new(&*stream, PollFlags::IN),
      PollFd::new(&self.work.ready, PollFlags::IN),
    ];

    match poll(&mut sockets, None) {
      Ok(_ready) => {}
      Err(rustix::io::Errno::INTR) => return Ok(Turn::Continue),
      Err(error) => return Err(Failure::from(error)),
    }

    let [stop, events, ready] = sockets.map(|fd| fd.revents());

    match stop.is_empty().then_some(()) {
      Some(()) => {}
      None => return Ok(Turn::Stop),
    }

    match events
      .intersects(PollFlags::HUP | PollFlags::ERR | PollFlags::NVAL)
      .then_some(())
    {
      Some(()) => return Ok(Turn::Retry),
      None => {}
    }

    match events.contains(PollFlags::IN).then_some(()) {
      Some(()) => {
        let changed = self.read_changes(stream, frame, at)?;

        match changed {
          Turn::Continue => {}
          Turn::Retry => return Ok(Turn::Retry),
          Turn::Stop => return Ok(Turn::Stop),
        }
      }
      None => {}
    }

    match ready.is_empty().then_some(()) {
      Some(()) => {}
      None => self.finish_captures(at)?,
    }

    Ok(Turn::Continue)
  }

  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "the driver switched this stream to nonblocking before registering it"
    )
  )]
  fn read_changes(
    &mut self,
    stream: &mut UnixStream,
    frame: &mut Vec<u8>,
    at: &Path,
  ) -> Result<Turn, Failure> {
    let mut buffer = [0_u8; 8192];
    let observed = match stream.read(&mut buffer) {
      Ok(0) => return Ok(Turn::Retry),
      Ok(bytes) => buffer.get(..bytes).ok_or("read beyond buffer")?,
      Err(fault) => {
        let kind = fault.kind();

        return match (kind == io::ErrorKind::Interrupted || kind == io::ErrorKind::WouldBlock)
          .then_some(())
        {
          Some(()) => Ok(Turn::Continue),
          None => Err(Failure::from(fault)),
        };
      }
    };
    let update = changed(frame, observed)?;

    match update {
      Changed::Yes => {
        let _no_new_connection = self.run_effects(Request::Changed, Some(at))?;
      }
      Changed::No => {}
    }

    Ok(Turn::Continue)
  }

  fn finish_captures(&mut self, at: &Path) -> Result<(), Failure> {
    let Ok(()) = latch::drain(&self.work.ready);

    for _result in 0..2 {
      let completed = match self.work.captured.try_recv() {
        Ok(completed) => completed,
        Err(std::sync::mpsc::TryRecvError::Empty) => break,
        Err(fault @ std::sync::mpsc::TryRecvError::Disconnected) => {
          return Err(Failure::from(fault));
        }
      };
      let request = match completed.result {
        Ok(snapshot) => Request::Captured(completed.ticket, snapshot),
        Err(fault) => {
          console_core_logging::debug!("capturing workspace snapshot: {fault}");

          Request::CaptureFailed(completed.ticket)
        }
      };
      let _no_new_connection = self.run_effects(request, Some(at))?;
    }

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    expect(
      explicit039_no_reading_the_clock,
      reason = "publication records the observation time at the driver's I/O edge"
    )
  )]
  fn run_effects(
    &mut self,
    request: Request,
    at: Option<&Path>,
  ) -> Result<Option<UnixStream>, Box<dyn std::error::Error>> {
    self
      .actor
      .try_submit(&mut self.completions, (), request)
      .map_err(|fault| io::Error::other(fault.to_string()))?;
    let completion = self.completions.recv_timeout(TRANSITION_TIME)?;
    let effects = completion
      .result
      .map_err(|fault| io::Error::other(fault.to_string()))?;
    let mut connected = None;

    for effect in effects {
      match effect {
        Effect::Connect => {
          let path = socket::socket(Socket::Events)?;
          let stream = ipc::connected(&path)?;
          stream.set_nonblocking(true)?;
          connected = Some(stream);
        }
        Effect::Capture(ticket) => {
          let at_2 = at.ok_or("missing compositor request socket")?;
          self.work.request(at_2, ticket)?;
        }
        Effect::Publish(workspace) => {
          let fresh_at = match workspace {
            Workspace::Stale => None,
            Workspace::Fresh { .. } => Some(Instant::now()),
          };
          let payload = serde_json::to_string(&workspace)?;
          self.updates.send(Published { payload, fresh_at })?;

          match self.wake.write(&[1]) {
            Ok(_notified) => {}
            Err(fault) => match ::core::primitive::bool::then_some(
              fault.kind() == io::ErrorKind::WouldBlock,
              (),
            ) {
              ::core::option::Option::Some(()) => continue,
              ::core::option::Option::None => return Err(Failure::from(fault)),
            },
          }
        }
        Effect::Reconnect => return Err(Failure::from("a workspace capture failed")),
      }
    }

    Ok(connected)
  }
}

fn changed(frame: &mut Vec<u8>, bytes: &[u8]) -> io::Result<Changed> {
  let mut update = Changed::No;

  for part in bytes.split_inclusive(|byte| *byte == b'\n') {
    let Ok(total) = fixed_index(frame.len().saturating_add(part.len()));

    match (total > INPUT_BYTES).then_some(()) {
      Some(()) => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "compositor event too large",
        ));
      }
      None => frame.extend_from_slice(part),
    }

    match part.last() {
      Some(b'\n') => {
        let Ok(observed) = event_changed(frame);
        update = match (update, observed) {
          (Changed::Yes, _) | (_, Changed::Yes) => Changed::Yes,
          (Changed::No, Changed::No) => Changed::No,
        };
        frame.clear();
      }
      Some(_) | None => {}
    }
  }

  Ok(update)
}

fn event_changed(frame: &[u8]) -> Result<Changed, Never> {
  let word = frame.split(|byte| *byte == b'>').next();

  Ok(match word {
    Some(
      b"workspacev2"
      | b"focusedmon"
      | b"createworkspacev2"
      | b"destroyworkspacev2"
      | b"moveworkspacev2"
      | b"renameworkspace"
      | b"activespecial"
      | b"openwindow"
      | b"closewindow"
      | b"movewindowv2"
      | b"windowtitlev2"
      | b"pin"
      | b"changefloatingmode"
      | b"fullscreen"
      | b"monitoraddedv2"
      | b"monitorremovedv2"
      | b"configreloaded",
    ) => Changed::Yes,
    Some(_) | None => Changed::No,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[must_use]
  fn sample() -> Result<Snapshot, Never> {
    Ok(Snapshot {
      front: None,
      workspaces: Vec::new(),
      clients: Vec::new(),
      monitors: Vec::new(),
    })
  }

  #[test]
  fn only_the_current_generation_can_publish_after_reconnect() {
    let Ok(snapshot) = sample();
    let Ok(initial) = Source::initial_transition(&(), None);
    let Ok(opening) = Source::transition(initial.state, Request::Retry);
    assert_eq!(opening.effects, [Effect::Connect]);
    let Ok(first) = Source::transition(opening.state, Request::Subscribed);
    let first_id = Ticket {
      generation: 1,
      operation: 1,
    };
    assert_eq!(first.effects, [Effect::Capture(first_id)]);
    let Ok(lost) = Source::transition(first.state, Request::Lost);
    assert_eq!(lost.effects, [Effect::Publish(Workspace::Stale)]);

    let Ok(reopening) = Source::transition(lost.state, Request::Retry);
    let Ok(second) = Source::transition(reopening.state, Request::Subscribed);
    let second_id = Ticket {
      generation: 2,
      operation: 2,
    };
    assert_eq!(second.effects, [Effect::Capture(second_id)]);
    let Ok(too_late) =
      Source::transition(second.state, Request::Captured(first_id, snapshot.clone()));
    assert!(too_late.effects.is_empty());
    let Ok(old_fault) = Source::transition(too_late.state, Request::CaptureFailed(first_id));
    assert!(old_fault.effects.is_empty());

    let Ok(ready) = Source::transition(
      old_fault.state,
      Request::Captured(second_id, snapshot.clone()),
    );
    assert_eq!(
      ready.effects,
      [Effect::Publish(Workspace::Fresh {
        generation: 2,
        revision: 1,
        snapshot: snapshot.clone()
      })]
    );
  }

  #[test]
  fn changes_during_capture_request_one_more_capture() {
    let Ok(snapshot) = sample();
    let Ok(initial) = Source::initial_transition(&(), None);
    let Ok(opening) = Source::transition(initial.state, Request::Retry);
    let Ok(first) = Source::transition(opening.state, Request::Subscribed);
    let Ok(changed) = Source::transition(first.state, Request::Changed);
    let Ok(changed_again) = Source::transition(changed.state, Request::Changed);
    assert!(changed.effects.is_empty() && changed_again.effects.is_empty());
    let first_id = Ticket {
      generation: 1,
      operation: 1,
    };
    let Ok(ready) = Source::transition(
      changed_again.state,
      Request::Captured(first_id, snapshot.clone()),
    );
    assert_eq!(
      ready.effects,
      [
        Effect::Publish(Workspace::Fresh {
          generation: 1,
          revision: 1,
          snapshot: snapshot.clone()
        }),
        Effect::Capture(Ticket {
          generation: 1,
          operation: 2
        })
      ]
    );
  }

  #[test]
  fn a_split_compositor_event_is_consumed_once() -> io::Result<()> {
    let mut partial = Vec::new();
    let first = changed(&mut partial, b"work")?;
    assert_eq!(first, Changed::No);
    let second = changed(&mut partial, b"spacev2>>2,terminal\nopenlayer>>bar\n")?;
    assert_eq!(second, Changed::Yes);
    assert!(partial.is_empty());

    Ok(())
  }
}
