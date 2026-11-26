//! Workspace switches and panel visibility use independent compositor workers.
//! A newer workspace tap or panel refresh supersedes an older reply.
//! Each worker handles one request at a time and coalesces bursts into one follow-up.

use std::fs::File;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};

use console_compositor::{Action, Carrying, DispatchResult, Layer, Workspace, WorkspaceTarget};
use console_concurrency::Latest;
use console_core_state_machine::{
  EffectDependencies, Machine, Never, Queue, Service, SingleEffectMachine, SingleTransition,
  Transition,
};
use console_events::ipc::{Trace, Watch};
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

pub(super) struct PanelSnapshot {
  pub(super) layers: Vec<Layer>,
  pub(super) tab: Result<Option<String>, console_onscreen::Error>,
}

#[derive(Clone, Copy)]
enum ObservationState {
  Idle,
  Reading,
  RefreshPending,
  Stopped,
}

enum ObservationRequest<T> {
  Refresh,
  Answered(Option<T>),
  Stopped,
}

enum ObservationEffect<T> {
  Read(&'static [Service]),
  Publish(T),
}

impl<T> EffectDependencies for ObservationEffect<T> {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Read(services) => services,
      Self::Publish(_) => &[],
    })
  }
}

struct Observe<T>(std::marker::PhantomData<T>);

impl<T> SingleEffectMachine for Observe<T> {
  type Input = &'static [Service];
  type State = ObservationState;
  type Request = (&'static [Service], ObservationRequest<T>);
  type Effect = ObservationEffect<T>;

  fn initialize(
    services: &&'static [Service],
    _previous: Option<ObservationState>,
  ) -> Result<SingleTransition<ObservationState, ObservationEffect<T>>, Never> {
    Self::handle(
      ObservationState::Idle,
      (*services, ObservationRequest::Refresh),
    )
  }

  fn handle(
    state: ObservationState,
    (services, request): (&'static [Service], ObservationRequest<T>),
  ) -> Result<SingleTransition<ObservationState, ObservationEffect<T>>, Never> {
    let (next, effect) = match (state, request) {
      (ObservationState::Idle, ObservationRequest::Refresh)
      | (ObservationState::RefreshPending, ObservationRequest::Answered(_)) => (
        ObservationState::Reading,
        Some(ObservationEffect::Read(services)),
      ),
      (
        ObservationState::Reading | ObservationState::RefreshPending,
        ObservationRequest::Refresh,
      ) => (ObservationState::RefreshPending, None),
      (ObservationState::Reading, ObservationRequest::Answered(Some(snapshot))) => (
        ObservationState::Idle,
        Some(ObservationEffect::Publish(snapshot)),
      ),
      (ObservationState::Reading, ObservationRequest::Answered(None))
      | (ObservationState::Idle, ObservationRequest::Answered(_)) => {
        (ObservationState::Idle, None)
      }
      (ObservationState::Stopped, _) | (_, ObservationRequest::Stopped) => {
        (ObservationState::Stopped, None)
      }
    };

    Ok(SingleTransition {
      state: next,
      effect,
    })
  }
}

pub(super) struct Observer<T> {
  state: ObservationState,
  worker: Latest,
  answers: Receiver<Option<T>>,
  services: &'static [Service],
  wake: Option<OwnedFd>,
}

pub(super) fn panels() -> io::Result<Observer<PanelSnapshot>> {
  Observer::start(
    "bar panel visibility",
    &[Service::Compositor, Service::FileSystem],
    panel_snapshot,
  )
}

impl<T: Send + 'static> Observer<T> {
  pub(super) fn start(
    name: &'static str,
    services: &'static [Service],
    mut observe: impl FnMut() -> Result<Option<T>, Never> + Send + 'static,
  ) -> io::Result<Self> {
    let pipe = latch::pipe()?;
    let (replies, answers) = sync_channel(1);
    let worker = Latest::start(name.to_owned(), move || {
      let Ok(answer) = observe();

      match deliver(answer, &replies, &pipe.saying) {
        Ok(()) => {}
        Err(fault) => {
          console_core_logging::debug!("console-bar: {name} observation delivery: {fault}")
        }
      }

      Ok(())
    })?;
    let mut watcher = Self {
      state: ObservationState::Idle,
      worker,
      answers,
      services,
      wake: Some(pipe.waiting),
    };
    let Ok(initial) = Observe::initialize(&services, None);
    let Ok(_no_snapshot_yet) = watcher.perform(initial);

    Ok(watcher)
  }

  pub(super) fn refresh(&mut self) -> Result<(), Never> {
    let Ok(transition) =
      Observe::handle(self.state, (self.services, ObservationRequest::Refresh));
    let Ok(_no_snapshot_yet) = self.perform(transition);

    Ok(())
  }

  pub(super) fn wake(&self) -> Result<Option<BorrowedFd<'_>>, Never> {
    Ok(self.wake.as_ref().map(AsFd::as_fd))
  }

  pub(super) fn observed(&mut self) -> Result<Option<T>, Never> {
    let wake = match &self.wake {
      Some(wake) => wake,
      None => return Ok(None),
    };
    let Ok(()) = latch::drain(wake);
    let request = match self.answers.try_recv() {
      Ok(snapshot) => ObservationRequest::Answered(snapshot),
      Err(TryRecvError::Empty) => return Ok(None),
      Err(TryRecvError::Disconnected) => ObservationRequest::Stopped,
    };
    let Ok(transition) = Observe::handle(self.state, (self.services, request));

    self.perform(transition)
  }

  fn perform(
    &mut self,
    transition: SingleTransition<ObservationState, ObservationEffect<T>>,
  ) -> Result<Option<T>, Never> {
    let previously = self.state;
    self.state = transition.state;

    match transition.effect {
      Some(ObservationEffect::Read(_)) => match self.worker.schedule() {
        Ok(()) => {}
        Err(_stopped) => {
          let Ok(stopped) =
            Observe::<T>::handle(self.state, (self.services, ObservationRequest::Stopped));
          self.state = stopped.state;
        }
      },
      Some(ObservationEffect::Publish(snapshot)) => return Ok(Some(snapshot)),
      None => {}
    }

    match (previously, self.state) {
      (
        ObservationState::Idle | ObservationState::Reading | ObservationState::RefreshPending,
        ObservationState::Stopped,
      ) => {
        self.wake = None;
        console_core_logging::warn!("console-bar: observation worker stopped");
      }
      (ObservationState::Stopped, _)
      | (
        _,
        ObservationState::Idle | ObservationState::Reading | ObservationState::RefreshPending,
      ) => {}
    }

    Ok(None)
  }
}

fn panel_snapshot() -> Result<Option<PanelSnapshot>, Never> {
  let Ok(observed) = reading(console_compositor::Layers);

  Ok(observed.map(|layers| PanelSnapshot {
    layers,
    tab: console_onscreen::tab(),
  }))
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
  workspace: Option<Watch>,
  source: WorkspaceSource,
}

#[derive(Clone, Copy)]
enum WorkspaceSource {
  Direct,
  Snapshot,
}

impl Worker {
  pub(super) fn start() -> io::Result<Self> {
    let pipe = latch::pipe()?;
    let Ok(workspace) = watching(&pipe.saying);
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
      workspace,
      source: WorkspaceSource::Direct,
    };
    let Ok(initial) = State::initial_transition(&(), None);
    let Ok(()) = worker.perform(initial);

    Ok(worker)
  }

  pub(super) fn refresh(&mut self) -> Result<(), Never> {
    match self.source {
      WorkspaceSource::Direct => self.request(Request::Refresh),
      WorkspaceSource::Snapshot => Ok(()),
    }
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
          match &self.workspace {
            None => self.waking = None,
            Some(_watch) => {}
          }

          let Ok(()) = self.request(Request::Unavailable);

          break;
        }
      };
      let fresh = matches!(
        (self.state, self.source),
        (State::Reading, WorkspaceSource::Direct)
      )
      .then_some(());

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

    let update = self.workspace.as_ref().and_then(|workspace| {
      let Ok(update) = workspace.take();

      update
    });

    match update {
      Some(console_events::ipc::Workspace::Fresh { snapshot, .. }) => {
        self.source = WorkspaceSource::Snapshot;
        observed.front = match snapshot.front {
          Some(workspace) => FocusUpdate::Workspace(workspace),
          None => FocusUpdate::NoWorkspace,
        };
        observed.workspaces = Some(snapshot.workspaces);
      }
      Some(console_events::ipc::Workspace::Stale) => match self.source {
        WorkspaceSource::Snapshot => {
          self.source = WorkspaceSource::Direct;
          let Ok(()) = self.refresh();
        }
        WorkspaceSource::Direct => {}
      },
      None => {}
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

fn watching(waking: &File) -> Result<Option<Watch>, Never> {
  let watching = waking
    .try_clone()
    .map_err(console_events::ipc::Error::from)
    .and_then(|wake| Watch::session(wake, Trace::Quiet));

  Ok(match watching {
    Ok(watch) => Some(watch),
    Err(fault) => {
      console_core_logging::debug!("console-bar: workspace feed unavailable: {fault}");

      None
    }
  })
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

fn deliver<T>(observation: T, replies: &SyncSender<T>, waking: &File) -> io::Result<()> {
  replies.try_send(observation).map_err(|fault| match fault {
    TrySendError::Full(_) => io::Error::from(io::ErrorKind::WouldBlock),
    TrySendError::Disconnected(_) => io::Error::from(io::ErrorKind::BrokenPipe),
  })?;
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
  use console_status_bar::panels::{self, Panel, PanelEvent, Panels};
  use console_status_bar::showing::Lit;
  use rustix::event::{PollFd, PollFlags, Timespec, poll};

  #[test]
  fn a_newer_panel_event_discards_the_old_reply_and_coalesces_refreshes()
  -> Result<(), Box<dyn std::error::Error>> {
    let calendar_layer =
      r#"{"eDP-1":{"levels":{"3":[{"namespace":"calendar-panel","w":640,"h":480}]}}}"#;
    let layers = console_compositor::read(console_compositor::Layers, calendar_layer)?;
    let reopened_layers = console_compositor::read(console_compositor::Layers, calendar_layer)?;
    let mut replies = [
      Some(PanelSnapshot {
        layers,
        tab: Ok(None),
      }),
      Some(PanelSnapshot {
        layers: Vec::new(),
        tab: Ok(None),
      }),
      Some(PanelSnapshot {
        layers: reopened_layers,
        tab: Ok(None),
      }),
    ]
    .into_iter();
    let mut worker = Observer::start(
      "test bar panel observation",
      &[Service::Compositor, Service::FileSystem],
      move || Ok(replies.next().flatten()),
    )?;

    let Ok(()) = worker.refresh();
    let Ok(()) = worker.refresh();
    wait_for_layers(&worker)?;
    let Ok(discarded) = worker.observed();
    assert!(discarded.is_none());

    wait_for_layers(&worker)?;
    let Ok(fresh) = worker.observed();
    let closed = fresh.ok_or("missing coalesced panel observation")?;
    let Ok(up) = panels::on_screen(&closed.layers, None);
    let Ok(clocked) = Panels::transition(Panels::default(), PanelEvent::Shown(up));
    assert_eq!(clocked.state.lit(Panel::Calendar), Ok(Lit::No));

    let Ok(()) = worker.refresh();
    wait_for_layers(&worker)?;
    let Ok(reopened) = worker.observed();
    let opened = reopened.ok_or("missing reopened calendar")?;
    let Ok(visible) = panels::on_screen(&opened.layers, None);
    let Ok(lit) = Panels::transition(clocked.state, PanelEvent::Shown(visible));
    assert_eq!(lit.state.lit(Panel::Calendar), Ok(Lit::Yes));

    Ok(())
  }

  fn wait_for_layers(worker: &Observer<PanelSnapshot>) -> Result<(), Box<dyn std::error::Error>> {
    let Ok(wake) = worker.wake();
    let fd = wake.ok_or("the panel worker stopped")?;
    let mut watched = [PollFd::from_borrowed_fd(fd, PollFlags::IN)];
    let timeout = Timespec {
      tv_sec: 2,
      tv_nsec: 0,
    };

    match poll(&mut watched, Some(&timeout)) {
      Ok(1) => Ok(()),
      Ok(_) => Err(Box::from("panel observation did not wake the bar")),
      Err(fault) => Err(Box::from(fault)),
    }
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
      workspace: None,
      source: WorkspaceSource::Direct,
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
