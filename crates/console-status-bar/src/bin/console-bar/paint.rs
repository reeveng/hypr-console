use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};

use console_concurrency::Latest;
use console_core_geometry::{Point, Size};
use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};
use console_draw_surface::{
  Hover, PointerCursor, RepaintHover, Scale, Surface, SurfaceError, Visible,
};
use console_status_bar::preferences::Preferences;
use console_status_bar::reading::StatusItem;
use console_status_bar::retained::{Scene, Volume, VolumeTiles, pixels};
use console_status_bar::settings::SettingsTab;
use console_status_bar::showing::{
  BarAction, Filling, Fitting, HitRegion, Rendered, Slot, Wearing,
};
use console_status_bar::state::BarState;
use console_waiting::latch;

use super::waking;

#[derive(Clone)]
struct Snapshot {
  held: BarState,
  preferences: Preferences,
  slots: Vec<Slot>,
  wearing: Wearing,
  fitting: Fitting,
  filling: Filling,
  room: Size<u32>,
  focus: Option<BarAction>,
  scale: Scale,
}

impl Snapshot {
  fn of(scene: Scene<'_>, scale: Scale) -> Result<Self, Never> {
    Ok(Self {
      held: scene.held.clone(),
      preferences: scene.preferences.clone(),
      slots: scene.slots.to_vec(),
      wearing: scene.wearing,
      fitting: scene.fitting,
      filling: scene.filling,
      room: scene.room,
      focus: scene.focus,
      scale,
    })
  }

  fn scene(&self) -> Result<Scene<'_>, Never> {
    Ok(Scene {
      held: &self.held,
      preferences: &self.preferences,
      slots: &self.slots,
      wearing: self.wearing,
      fitting: self.fitting,
      filling: self.filling,
      room: self.room,
      focus: self.focus,
    })
  }

  fn compare(&self, scene: Scene<'_>, scale: Scale) -> Result<SceneChange, Never> {
    let Ok(model) = compare_model(&self.held, scene.held);

    Ok(
      match (self.scale == scale
        && self.wearing == scene.wearing
        && self.room == scene.room
        && self.fitting == scene.fitting
        && self.filling == scene.filling
        && self.focus == scene.focus
        && &self.preferences == scene.preferences
        && self.slots == scene.slots
        && model == SceneChange::Same)
        .then_some(())
      {
        Some(()) => SceneChange::Same,
        None => SceneChange::Changed,
      },
    )
  }
}

#[derive(PartialEq, Eq)]
enum SceneChange {
  Same,
  Changed,
}

fn compare_model(before: &BarState, after: &BarState) -> Result<SceneChange, Never> {
  let readings = |(item, _): &&(StatusItem, _)| *item != StatusItem::Sound;
  let same = before.clock == after.clock
    && before.front == after.front
    && before.panels == after.panels
    && before.bell == after.bell
    && before.music == after.music
    && before.workspaces == after.workspaces
    && before
      .readings
      .iter()
      .filter(readings)
      .eq(after.readings.iter().filter(readings));

  Ok(match same.then_some(()) {
    Some(()) => SceneChange::Same,
    None => SceneChange::Changed,
  })
}

struct Job {
  generation: u64,
  scene: Box<Snapshot>,
  device: Size<u32>,
}

struct Answer {
  generation: u64,
  tiles: io::Result<Option<VolumeTiles>>,
}

enum State {
  Idle,
  Preparing { current: u64, pending: Option<Job> },
  Stopped,
}

enum Request {
  Invalidate(Job),
  Finished(Answer),
  Stopped,
}

enum Effect {
  Prepare(Job),
  Install(Answer),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Prepare(_) => &[Service::Compute],
      Self::Install(_) => &[],
    })
  }
}

impl SingleEffectMachine for State {
  type Input = ();
  type State = Self;
  type Request = Request;
  type Effect = Effect;

  fn initialize(
    _input: &(),
    _previous: Option<Self>,
  ) -> Result<SingleTransition<Self, Effect>, Never> {
    Ok(SingleTransition {
      state: Self::Idle,
      effect: None,
    })
  }

  fn handle(state: Self, request: Request) -> Result<SingleTransition<Self, Effect>, Never> {
    let (next, effect) = match (state, request) {
      (Self::Idle, Request::Invalidate(job)) => {
        let Ok(started) = starting(job);
        started
      }
      (Self::Preparing { current, .. }, Request::Invalidate(job)) => (
        Self::Preparing {
          current,
          pending: Some(job),
        },
        None,
      ),
      (Self::Preparing { current, pending }, Request::Finished(answer)) => {
        match ((current == answer.generation).then_some(()), pending) {
          (Some(()), Some(job)) => {
            let Ok(started) = starting(job);
            started
          }
          (Some(()), None) => (Self::Idle, Some(Effect::Install(answer))),
          (None, queued) => (
            Self::Preparing {
              current,
              pending: queued,
            },
            None,
          ),
        }
      }
      (Self::Idle, Request::Finished(_)) => (Self::Idle, None),
      (Self::Stopped, _) | (_, Request::Stopped) => (Self::Stopped, None),
    };

    Ok(SingleTransition {
      state: next,
      effect,
    })
  }
}

fn starting(job: Job) -> Result<(State, Option<Effect>), Never> {
  let state = State::Preparing {
    current: job.generation,
    pending: None,
  };

  Ok((state, Some(Effect::Prepare(job))))
}

struct Last {
  scene: Snapshot,
  drawn: Rendered,
  actions: Vec<BarAction>,
  hover: Hover<HitRegion>,
  volume: Option<Volume>,
  tiles: Option<VolumeTiles>,
  path: Presentation,
}

#[derive(Clone, Copy)]
enum Presentation {
  Frame,
  WaitingForFrame,
  Tile,
}

impl Last {
  fn draw(
    &mut self,
    surface: &mut Surface,
    scene: Scene<'_>,
    volume: Option<Volume>,
  ) -> Result<(), SurfaceError> {
    let unchanged = match (self.volume, volume) {
      (Some(before), Some(after)) => (before == after).then_some(()),
      (None, None) => (self.scene.held.readings == scene.held.readings).then_some(()),
      (Some(_), None) | (None, Some(_)) => None,
    };
    let Ok(pointer) = surface.pointer_pixel();
    let Ok(target) = self.drawn.hit_at(pointer);
    let Ok(feedback) = self.hover.observe(target.copied());
    let over_sound = target.filter(|hit| hit.action == BarAction::Settings(SettingsTab::Sound));
    let tiles = match over_sound {
      Some(_) => None,
      None => self.tiles.as_ref(),
    };

    match (feedback, unchanged, tiles, self.volume, volume) {
      (None, Some(()), _, _, _) => self.resume(surface)?,
      (None, None, Some(volume_tiles), Some(_), Some(after)) => {
        let presented = present_volume(surface, self.path, volume_tiles, after)?;
        self.path = presented;
      }
      (Some(RepaintHover), _, _, _, _) | (None, None, _, _, _) => {
        let Ok(drawn) = rendered(scene, pointer, &mut self.hover);
        let presented = redraw(surface, &drawn)?;
        self.path = presented;
        self.actions = drawn.touching.iter().map(|region| region.action).collect();
        self.drawn = drawn;
        self.scene.held.readings.clone_from(&scene.held.readings);
      }
    }

    self.volume = volume;
    Ok(())
  }

  fn resume(&mut self, surface: &mut Surface) -> Result<(), SurfaceError> {
    match self.path {
      Presentation::Tile | Presentation::Frame => {}
      Presentation::WaitingForFrame => {
        let visible = surface.draw_retained(|_, _| Ok(()))?;
        let Ok(path) = frame_visibility(surface, visible);
        self.path = path;
      }
    }

    Ok(())
  }
}

fn rendered(
  scene: Scene<'_>,
  pointer: Option<Point<i32>>,
  hover: &mut Hover<HitRegion>,
) -> Result<Rendered, Never> {
  let Ok(mut drawn) = scene.render();
  let Ok(target) = drawn.show_hover(pointer, scene.wearing);
  let Ok(_painted_in_this_frame) = hover.observe(target);

  Ok(drawn)
}

fn present_volume(
  surface: &mut Surface,
  presentation: Presentation,
  tiles: &VolumeTiles,
  volume: Volume,
) -> Result<Presentation, SurfaceError> {
  let Ok(tile) = tiles.tile(volume);

  match (presentation, tile) {
    (Presentation::Frame | Presentation::Tile, Some((part, pixels))) => {
      let visible = surface.draw_packed_layer(part, pixels)?;

      match visible {
        Visible::Yes => return Ok(Presentation::Tile),
        Visible::NotYet => {}
      }
    }
    (Presentation::WaitingForFrame, _) | (_, None) => {}
  }

  let visible = surface.draw_retained(|retained, _| tiles.update(volume, retained))?;
  let Ok(path) = frame_visibility(surface, visible);

  Ok(path)
}

fn frame_visibility(surface: &mut Surface, visible: Visible) -> Result<Presentation, Never> {
  Ok(match visible {
    Visible::Yes => {
      let Ok(()) = surface.clear_layer();
      Presentation::Frame
    }
    Visible::NotYet => Presentation::WaitingForFrame,
  })
}

fn redraw(surface: &mut Surface, drawn: &Rendered) -> Result<Presentation, SurfaceError> {
  let visible = surface.draw_retained(|retained, size| {
    let rendered = pixels(drawn, size)?;
    retained.replace(size, rendered)
  })?;
  let Ok(path) = frame_visibility(surface, visible);

  Ok(path)
}

pub(super) struct Painter {
  last: Option<Last>,
  generation: u64,
  state: State,
  jobs: SyncSender<Job>,
  worker: Latest,
  answers: Receiver<Answer>,
  wake: OwnedFd,
}

impl Painter {
  pub(super) fn start() -> io::Result<Self> {
    let pipe = latch::pipe()?;
    let (jobs, work) = sync_channel::<Job>(1);
    let (send, answers) = sync_channel(1);
    let worker = Latest::start("bar device pixels".to_owned(), move || {
      prepare(&work, &send, &pipe.saying)
    })?;
    let Ok(initial) = State::initialize(&(), None);

    Ok(Self {
      last: None,
      generation: 0,
      state: initial.state,
      jobs,
      worker,
      answers,
      wake: pipe.waiting,
    })
  }

  pub(super) fn wake(&self) -> Result<Option<BorrowedFd<'_>>, Never> {
    Ok(match self.state {
      State::Stopped => None,
      State::Idle | State::Preparing { .. } => Some(self.wake.as_fd()),
    })
  }

  pub(super) fn clear(&mut self) -> Result<(), Never> {
    self.last = None;
    self.generation = self.generation.wrapping_add(1);

    Ok(())
  }

  pub(super) fn picture(&self) -> Result<Option<(&Rendered, &[BarAction])>, Never> {
    Ok(
      self
        .last
        .as_ref()
        .map(|last| (&last.drawn, last.actions.as_slice())),
    )
  }

  pub(super) fn draw(
    &mut self,
    surface: &mut Surface,
    scene: Scene<'_>,
  ) -> Result<(), SurfaceError> {
    self.draw_scene(surface, scene)?;
    let hovered = self.last.as_ref().and_then(|last| {
      let Ok(target) = last.hover.target();

      target
    });
    let cursor = match hovered {
      Some(_) => PointerCursor::Hand,
      None => PointerCursor::Default,
    };
    let Ok(()) = surface.set_pointer_cursor(cursor);

    Ok(())
  }

  fn draw_scene(&mut self, surface: &mut Surface, scene: Scene<'_>) -> Result<(), SurfaceError> {
    let Ok(scale) = surface.scale();
    let Ok(volume) = Volume::observed(scene.held);

    let change = match self.last.as_ref() {
      Some(last) => {
        let Ok(change) = last.scene.compare(scene, scale);
        change
      }
      None => SceneChange::Changed,
    };

    match change {
      SceneChange::Same => {
        let Ok(()) = self.finished();

        match self.last.as_mut() {
          Some(last) => return last.draw(surface, scene, volume),
          None => {}
        }
      }
      SceneChange::Changed => {}
    }

    self.rebuild(surface, scene, scale, volume)
  }

  fn rebuild(
    &mut self,
    surface: &mut Surface,
    scene: Scene<'_>,
    scale: Scale,
    volume: Option<Volume>,
  ) -> Result<(), SurfaceError> {
    let Ok(configured) = surface.logical();
    let logical = match configured {
      Some(size) => size,
      None => return Ok(()),
    };
    let Ok(device) = scale.device(logical);
    let Ok(pointer) = surface.pointer_pixel();
    let mut hover = Hover::default();
    let Ok(drawn) = rendered(scene, pointer, &mut hover);
    let actions = drawn.touching.iter().map(|region| region.action).collect();
    let path = redraw(surface, &drawn)?;

    let Ok(snapshot) = Snapshot::of(scene, scale);
    self.generation = self.generation.wrapping_add(1);
    let generation = self.generation;
    let job = Job {
      generation,
      scene: Box::new(snapshot.clone()),
      device,
    };
    self.last = Some(Last {
      scene: snapshot,
      drawn,
      actions,
      hover,
      volume,
      tiles: None,
      path,
    });
    let Ok(()) = self.request(Request::Invalidate(job));
    let Ok(()) = self.finished();

    Ok(())
  }

  fn finished(&mut self) -> Result<(), Never> {
    let Ok(()) = latch::drain(&self.wake);

    match self.answers.try_recv() {
      Ok(answer) => self.request(Request::Finished(answer)),
      Err(TryRecvError::Empty) => Ok(()),
      Err(TryRecvError::Disconnected) => self.request(Request::Stopped),
    }
  }

  fn request(&mut self, request: Request) -> Result<(), Never> {
    let Ok(next) = State::handle(std::mem::replace(&mut self.state, State::Idle), request);
    self.state = next.state;

    match next.effect {
      Some(Effect::Prepare(job)) => match self.jobs.try_send(job) {
        Ok(()) => match self.worker.schedule() {
          Ok(()) => {}
          Err(_stopped) => {
            console_core_logging::warn!("console-bar: device asset worker stopped");
            self.state = State::Stopped;
          }
        },
        Err(fault) => {
          console_core_logging::warn!("console-bar: scheduling device assets: {fault}");
          self.state = State::Stopped;
        }
      },
      Some(Effect::Install(answer)) => match (
        self.last.as_mut(),
        answer.tiles,
        (answer.generation == self.generation).then_some(()),
      ) {
        (Some(last), Ok(tiles), Some(())) => last.tiles = tiles,
        (Some(_), Ok(_), None) | (None, Ok(_), _) => {}
        (_, Err(fault), _) => {
          console_core_logging::warn!("console-bar: preparing device assets: {fault}")
        }
      },
      None => {}
    }

    Ok(())
  }
}

fn prepare(
  work: &Receiver<Job>,
  send: &SyncSender<Answer>,
  wake: &std::fs::File,
) -> Result<(), Never> {
  let job = match work.try_recv() {
    Ok(job) => job,
    Err(TryRecvError::Empty | TryRecvError::Disconnected) => return Ok(()),
  };
  let Ok(scene) = job.scene.scene();
  let answer = Answer {
    generation: job.generation,
    tiles: VolumeTiles::prepare(scene, job.device),
  };

  match send.try_send(answer) {
    Ok(()) => match waking::pulse(wake) {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::debug!("console-bar: waking after preparing device assets: {fault}")
      }
    },
    Err(fault) => console_core_logging::debug!("console-bar: delivering device assets: {fault}"),
  }

  Ok(())
}
