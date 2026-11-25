//! Workspace switches and panel visibility use independent compositor workers.
//! A newer workspace tap supersedes an older reply, and bursts share a follow-up read.

use std::fs::File;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard};

use console_compositor::{Action, Carrying, DispatchResult, Layer, Workspace, WorkspaceTarget};
use console_concurrency::Latest;
use console_core_state_machine::{
  EffectDependencies, Machine, Never, Queue, Service, Transition,
};
use console_status_bar::state::FocusUpdate;
use console_waiting::latch;

use super::waking;

enum Reply {
  Focus(Option<Workspace>),
  Workspaces(Option<Vec<Workspace>>),
}

#[derive(Default)]
pub(super) struct Observations {
  pub(super) front: FocusUpdate,
  pub(super) workspaces: Option<Vec<Workspace>>,
}

#[derive(Default)]
struct LayerReading {
  turn: u64,
  layers: Option<Vec<Layer>>,
}

impl LayerReading {
  fn invalidate(&mut self) -> Result<(), Never> {
    self.turn = self.turn.wrapping_add(1);
    self.layers = None;

    Ok(())
  }

  fn answered(&mut self, turn: u64, layers: Vec<Layer>) -> Result<(), Never> {
    match (self.turn == turn).then_some(()) {
      Some(()) => self.layers = Some(layers),
      None => {}
    }

    Ok(())
  }
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "the drawing loop and the latest worker exchange one generation and one replaceable snapshot in this mailbox"
  )
)]
pub(super) struct LayerWorker {
  reading: Arc<Mutex<LayerReading>>,
  worker: Latest,
  wake: OwnedFd,
}

impl LayerWorker {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the drawing loop and the latest worker exchange one generation and one replaceable snapshot in this mailbox"
    )
  )]
  pub(super) fn start() -> io::Result<Self> {
    let pipe = latch::pipe()?;
    let reading = Arc::new(Mutex::new(LayerReading::default()));
    let latest = Arc::clone(&reading);
    let worker = Latest::start("bar panel visibility".to_owned(), move || {
      observe_layers(&latest, &pipe.saying)
    })?;
    let watcher = Self {
      reading,
      worker,
      wake: pipe.waiting,
    };
    let Ok(()) = watcher.refresh();

    Ok(watcher)
  }

  pub(super) fn refresh(&self) -> Result<(), Never> {
    match self.worker.schedule() {
      Ok(()) => {}
      Err(_stopped) => {
        console_core_logging::warn!("console-bar: panel visibility worker stopped")
      }
    }

    Ok(())
  }

  pub(super) fn invalidate(&self) -> Result<(), Never> {
    let Ok(mut reading) = locked(&self.reading);

    reading.invalidate()
  }

  pub(super) fn wake(&self) -> Result<BorrowedFd<'_>, Never> {
    Ok(self.wake.as_fd())
  }

  pub(super) fn observed(&self) -> Result<Option<Vec<Layer>>, Never> {
    let Ok(()) = latch::drain(&self.wake);
    let Ok(mut reading) = locked(&self.reading);

    Ok(reading.layers.take())
  }
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the mailbox lock covers only ownership of the snapshot and its generation; no holder performs I/O or takes another lock"
  )
)]
fn locked(reading: &Mutex<LayerReading>) -> Result<MutexGuard<'_, LayerReading>, Never> {
  Ok(match reading.lock() {
    Ok(ready) => ready,
    Err(poisoned) => poisoned.into_inner(),
  })
}

fn observe_layers(latest: &Mutex<LayerReading>, waking: &File) -> Result<(), Never> {
  let turn = {
    let Ok(reading) = locked(latest);

    reading.turn
  };
  let Ok(layers) = reading(console_compositor::Layers);

  match layers {
    Some(observed) => {
      {
        let Ok(mut reading) = locked(latest);
        let Ok(()) = reading.answered(turn, observed);
      }

      match waking::pulse(waking) {
        Ok(()) => {}
        Err(rustix::io::Errno::PIPE) => {
          console_core_logging::trace!("console-bar: panel visibility receiver closed")
        }
        Err(fault) => {
          console_core_logging::warn!("console-bar: signalling panel visibility: {fault}")
        }
      }
    }
    None => {}
  }

  Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Work {
  Read,
  Switch(i64),
}

impl EffectDependencies for Work {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(&[Service::Compositor])
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum State {
  #[default]
  Idle,
  Reading,
  RefreshPending,
  SwitchPending(i64),
}

enum Request {
  Refresh,
  Switch(i64),
  Answered,
  Unavailable,
}

impl State {
  fn start(work: Work, effects: &mut Queue<Work>) -> Result<Self, Never> {
    let Ok(()) = effects.offer(work);

    Ok(Self::Reading)
  }
}

impl Machine for State {
  type Input = ();
  type State = Self;
  type Request = Request;
  type Effect = Work;

  fn initialize(
    _input: &(),
    _previous: Option<Self>,
    effects: &mut Queue<Work>,
  ) -> Result<Self, Never> {
    Self::start(Work::Read, effects)
  }

  fn handle(state: Self, request: Request, effects: &mut Queue<Work>) -> Result<Self, Never> {
    match (state, request) {
      (Self::Idle, Request::Refresh) => Self::start(Work::Read, effects),
      (Self::Reading | Self::RefreshPending, Request::Refresh) => Ok(Self::RefreshPending),
      (Self::SwitchPending(_), Request::Refresh) => Ok(state),
      (Self::Idle, Request::Switch(id)) => Self::start(Work::Switch(id), effects),
      (Self::Reading | Self::RefreshPending | Self::SwitchPending(_), Request::Switch(id)) => {
        Ok(Self::SwitchPending(id))
      }
      (Self::RefreshPending, Request::Answered) => Self::start(Work::Read, effects),
      (Self::SwitchPending(id), Request::Answered) => Self::start(Work::Switch(id), effects),
      (Self::Idle | Self::Reading, Request::Answered) | (_, Request::Unavailable) => {
        Ok(Self::Idle)
      }
    }
  }
}

pub(super) struct Worker {
  state: State,
  asking: SyncSender<Work>,
  answers: Receiver<Reply>,
  waking: Option<OwnedFd>,
}

impl Worker {
  pub(super) fn start() -> io::Result<Self> {
    let pipe = latch::pipe()?;
    let (asking, asked) = sync_channel(1);
    let (replies, answers) = sync_channel(2);
    let thread = std::thread::Builder::new().spawn(move || {
      let Ok(()) = serve(asked, replies, pipe.saying);
    })?;
    let Ok(()) = console_program_lifetime::threads::let_go(thread);
    let mut worker = Self {
      state: State::Idle,
      asking,
      answers,
      waking: Some(pipe.waiting),
    };
    let Ok(initial) = State::initial_transition(&(), None);
    let Ok(()) = worker.perform(initial);

    Ok(worker)
  }

  pub(super) fn refresh(&mut self) -> Result<(), Never> {
    self.request(Request::Refresh)
  }

  pub(super) fn switch(&mut self, id: i64) -> Result<(), Never> {
    self.request(Request::Switch(id))
  }

  pub(super) fn wake(&self) -> Result<Option<BorrowedFd<'_>>, Never> {
    Ok(self.waking.as_ref().map(AsFd::as_fd))
  }

  pub(super) fn observed(&mut self) -> Result<Observations, Never> {
    let mut observed = Observations::default();
    let waking = match &self.waking {
      Some(waking) => waking,
      None => return Ok(observed),
    };
    let Ok(()) = latch::drain(waking);

    for _ in 0..2 {
      let answer = match self.answers.try_recv() {
        Ok(answer) => answer,
        Err(TryRecvError::Empty) => break,
        Err(TryRecvError::Disconnected) => {
          self.waking = None;
          let Ok(()) = self.request(Request::Unavailable);

          break;
        }
      };
      let fresh = (self.state == State::Reading).then_some(());

      match answer {
        Reply::Focus(front) => match (fresh, front) {
          (Some(()), Some(workspace)) => observed.front = FocusUpdate::Workspace(workspace),
          (Some(()), None) => observed.front = FocusUpdate::NoWorkspace,
          (None, _) => {}
        },
        Reply::Workspaces(workspaces) => {
          observed.workspaces = fresh.and(workspaces);
          let Ok(()) = self.request(Request::Answered);
        }
      }
    }

    Ok(observed)
  }

  fn request(&mut self, request: Request) -> Result<(), Never> {
    let Ok(transition) = State::transition(self.state, request);

    self.perform(transition)
  }

  fn perform(&mut self, transition: Transition<State, Work>) -> Result<(), Never> {
    self.state = transition.state;

    for work in transition.effects {
      match self.asking.try_send(work) {
        Ok(()) => {}
        Err(fault) => {
          console_core_logging::warn!(
            "console-bar: could not request a compositor refresh: {fault}"
          );
          let Ok(reset) = State::transition(self.state, Request::Unavailable);

          self.state = reset.state;
        }
      }
    }

    Ok(())
  }
}

#[cfg_attr(
  dylint_lib = "explicit055_no_unbounded_loop",
  expect(
    explicit055_no_unbounded_loop,
    reason = "the bar owns the sole request sender; dropping it ends this worker without polling while the bar is idle"
  )
)]
fn serve(asked: Receiver<Work>, replies: SyncSender<Reply>, waking: File) -> Result<(), Never> {
  for work in asked {
    match observe(work, &replies, &waking) {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::debug!(
          "console-bar: compositor observation was not delivered: {fault}"
        );

        break;
      }
    }
  }

  Ok(())
}

fn observe(work: Work, replies: &SyncSender<Reply>, waking: &File) -> io::Result<()> {
  match work {
    Work::Read => {}
    Work::Switch(id) => {
      let Ok(result) = Action::MoveWorkspace {
        target: WorkspaceTarget::Number(id),
        carrying: Carrying::None,
      }
      .dispatch();

      match result {
        DispatchResult::Success => {}
        DispatchResult::Failure(fault) => {
          console_core_logging::warn!("console-bar: switching workspace: {fault}");
        }
      }
    }
  }

  let Ok(front) = reading(console_compositor::ActiveWorkspace);

  match front {
    Some(answer) => {
      deliver(Reply::Focus(answer), replies, waking)?;
    }
    None => {}
  }

  let Ok(workspaces) = reading(console_compositor::Workspaces);

  deliver(Reply::Workspaces(workspaces), replies, waking)
}

fn deliver(observation: Reply, replies: &SyncSender<Reply>, waking: &File) -> io::Result<()> {
  replies.try_send(observation).map_err(io::Error::other)?;
  waking::pulse(waking)?;

  Ok(())
}

fn reading<Q: console_compositor::Question>(question: Q) -> Result<Option<Q::Reply>, Never> {
  Ok(match console_compositor::ask(question) {
    Ok(answer) => Some(answer),
    Err(fault) => {
      console_core_logging::warn!("console-bar: {fault}");

      None
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn an_observation_started_before_a_press_cannot_replace_new_layer_state() {
    let mut reading = LayerReading::default();
    let previous = reading.turn;
    let Ok(()) = reading.invalidate();
    let Ok(()) = reading.answered(previous, Vec::new());
    assert!(reading.layers.is_none());

    let current = reading.turn;
    let Ok(()) = reading.answered(current, Vec::new());
    assert!(reading.layers.is_some());
  }

  #[test]
  fn a_burst_during_an_observation_requests_one_follow_up() {
    let Ok(initial) = State::initial_transition(&(), None);
    assert_eq!(initial.effects, [Work::Read]);

    let Ok(first) = State::transition(initial.state, Request::Refresh);
    let Ok(second) = State::transition(first.state, Request::Refresh);
    assert!(first.effects.is_empty());
    assert!(second.effects.is_empty());

    let Ok(checked) = State::transition(second.state, Request::Answered);
    assert_eq!(checked.effects, [Work::Read]);
    let Ok(settled) = State::transition(checked.state, Request::Answered);
    assert_eq!(settled.state, State::Idle);
    assert!(settled.effects.is_empty());
  }

  #[test]
  fn an_older_reply_cannot_overwrite_the_latest_tap() -> Result<(), Box<dyn std::error::Error>> {
    let pipe = latch::pipe()?;
    let (asking, jobs) = sync_channel(1);
    let (replies, answers) = sync_channel(2);
    let mut worker = Worker {
      state: State::Reading,
      asking,
      answers,
      waking: Some(pipe.waiting),
    };
    let Ok(()) = worker.switch(2);
    let Ok(()) = worker.switch(9);
    let Ok(()) = worker.refresh();

    deliver(Reply::Focus(None), &replies, &pipe.saying)?;
    deliver(Reply::Workspaces(Some(Vec::new())), &replies, &pipe.saying)?;
    let Ok(superseded) = worker.observed();
    assert_eq!(superseded.front, FocusUpdate::Unchanged);
    assert!(superseded.workspaces.is_none());
    assert_eq!(jobs.try_recv(), Ok(Work::Switch(9)));

    let front = Workspace {
      id: 9,
      named: "9".to_owned(),
      windows: Some(0),
    };
    deliver(Reply::Focus(Some(front.clone())), &replies, &pipe.saying)?;
    let Ok(focused) = worker.observed();
    assert_eq!(focused.front, FocusUpdate::Workspace(front));
    assert!(
      focused.workspaces.is_none(),
      "focus arrives before the trailing snapshot"
    );

    Ok(())
  }
}
