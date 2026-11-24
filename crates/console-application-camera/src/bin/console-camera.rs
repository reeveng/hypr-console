use std::convert::Infallible;
use std::error::Error;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use console_core_color::palette::Wearing;
use console_core_geometry::Size;
use console_core_internal_programs::InternalProgram;
use console_core_iteration::{Step, iterate};
use console_core_number_conversion::index;
use console_core_shapes::Pixels;
use console_draw_surface::{
  Closed, KeyboardEvent, Keysym, PointerEvent, Scale, Surface, SurfaceError, Window,
};

#[path = "console-camera/source.rs"]
mod source;
#[path = "console-camera/view.rs"]
mod view;

use source::{Source, Stream};
use view::{Action, Layout};

struct Camera {
  sources: Vec<Source>,
  selected: u32,
  stream: Option<Stream>,
  frame: Option<Pixels>,
  message: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Refresh {
  Changed,
  Unchanged,
}

enum Direction {
  Previous,
  Next,
}

impl Camera {
  fn selected_source(&self) -> Result<Option<&Source>, Infallible> {
    let Ok(position) = index(self.selected);
    Ok(self.sources.get(position))
  }

  fn discover(&mut self) -> Result<(), Infallible> {
    self.stream = None;
    self.frame = None;
    let Ok(selected) = self.selected_source();
    let previous = selected.map(|source| source.path.clone());
    self.sources = match source::discover() {
      Ok(sources) => sources,
      Err(fault) => {
        self.sources.clear();
        self.selected = 0;
        return self.failed(
          "Couldn’t access a camera. Check its connection and permissions.",
          &fault,
        );
      }
    };
    self.selected = match self
      .sources
      .iter()
      .position(|source| Some(&source.path) == previous.as_ref())
    {
      Some(found) => match u32::try_from(found) {
        Ok(index) => index,
        Err(_too_many_devices) => 0,
      },
      None => 0,
    };
    self.start()
  }

  fn start(&mut self) -> Result<(), Infallible> {
    self.stream = None;
    self.frame = None;
    self.message.clear();

    let Ok(selected) = self.selected_source();
    let source = match selected {
      Some(source) => source,
      None => {
        self.message = "No camera found. Connect a USB camera, then press Retry.".to_owned();
        return Ok(());
      }
    };

    match Stream::open(source) {
      Ok(stream) => {
        self.stream = Some(stream);
        self.message = "Starting camera…".to_owned();
        Ok(())
      }
      Err(fault) => self.failed("Couldn’t start the camera.", &fault),
    }
  }

  fn poll(&mut self, now: Instant) -> Result<Refresh, Infallible> {
    let stream = match &mut self.stream {
      Some(matched_stream) => matched_stream,
      None => return Ok(Refresh::Unchanged),
    };

    match stream.frame(now) {
      Ok(Some(frame)) => {
        match { ::core::primitive::bool::then_some(self.frame.is_none(), ()) } {
          ::core::option::Option::Some(()) => {
            self.message.clear();
          }
          ::core::option::Option::None => {}
        }

        self.frame = Some(frame);
        Ok(Refresh::Changed)
      }
      Ok(None) => Ok(Refresh::Unchanged),
      Err(fault) => {
        let Ok(()) = self.failed(
          "Camera stopped. Check the connection or close other camera apps.",
          &fault,
        );
        Ok(Refresh::Changed)
      }
    }
  }

  fn failed(&mut self, message: &str, fault: &std::io::Error) -> Result<(), Infallible> {
    console_core_logging::warn!("camera: {fault}");
    self.stream = None;
    self.frame = None;
    self.message = message.to_owned();
    Ok(())
  }

  fn act(&mut self, action: Action) -> Result<(), Infallible> {
    match action {
      Action::Previous => self.switch_source(Direction::Previous),
      Action::Next => self.switch_source(Direction::Next),
      Action::Retry => self.discover(),
      Action::Take => self.take_photo(),
      Action::Close => Ok(()),
    }
  }

  fn switch_source(&mut self, direction: Direction) -> Result<(), Infallible> {
    let count = match u32::try_from(self.sources.len()) {
      Ok(0 | 1) => return Ok(()),
      Ok(count) => count,
      Err(_too_many_devices) => return Ok(()),
    };

    self.selected = match direction {
      Direction::Previous => match self.selected.checked_sub(1) {
        Some(previous) => previous,
        None => count.saturating_sub(1),
      },
      Direction::Next => {
        let next = self.selected.saturating_add(1);

        match next.cmp(&count) {
          std::cmp::Ordering::Less => next,
          std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => 0,
        }
      }
    };
    self.start()
  }

  fn take_photo(&mut self) -> Result<(), Infallible> {
    let frame = match &self.frame {
      Some(frame) => frame,
      None => return Ok(()),
    };
    self.message = match console_application_camera::save_photo(frame) {
      Ok(path) => match path.file_name().and_then(|name| name.to_str()) {
        Some(name) => format!("Saved to Pictures · {name}"),
        None => "Saved to Pictures".to_owned(),
      },
      Err(fault) => {
        console_core_logging::warn!("saving photo: {fault}");
        "Couldn’t save the photo".to_owned()
      }
    };
    Ok(())
  }
}

fn main() -> ExitCode {
  match open() {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      console_core_logging::error!("console-application-camera: {fault}");
      ExitCode::FAILURE
    }
  }
}

fn open() -> Result<(), Box<dyn Error>> {
  let wearing = Wearing::worn()?;
  let mut surface = Surface::connect()?;
  let Ok(name) = InternalProgram::Camera.name();
  surface.show_window_sized(
    &Window {
      app_id: name.to_owned(),
      title: "Camera".to_owned(),
    },
    Size {
      width: 780,
      height: 560,
    },
  )?;

  let mut camera = Camera {
    sources: Vec::new(),
    selected: 0,
    stream: None,
    frame: None,
    message: String::new(),
  };
  let Ok(()) = camera.discover();
  run(&mut surface, &wearing, &mut camera)?;
  Ok(())
}

fn run(
  surface: &mut Surface,
  wearing: &Wearing,
  camera: &mut Camera,
) -> Result<(), Box<dyn Error>> {
  let drawing = WindowState {
    pressed: None,
    shown: Drawing::Pending,
  };
  let result = iterate(
    (surface, camera, drawing),
    |(target, camera_2, mut window)| {
      Ok(match window.turn(target, camera_2, wearing) {
        Ok(WindowChange::Continue) => Step::Again((target, camera_2, window)),
        Ok(WindowChange::Close) => Step::Halt(Ok(())),
        Err(fault) => Step::Halt(Err(fault)),
      })
    },
  )?;
  result.map_err(Box::from)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Drawing {
  Pending,
  Ready { points: Size<u32>, scale: Scale },
}

enum WindowChange {
  Continue,
  Close,
}

struct WindowState {
  pressed: Option<Action>,
  shown: Drawing,
}

impl WindowState {
  fn turn(
    &mut self,
    surface: &mut Surface,
    camera: &mut Camera,
    wearing: &Wearing,
  ) -> Result<WindowChange, SurfaceError> {
    let Ok(closed) = surface.closed();

    match closed {
      Closed::Yes => return Ok(WindowChange::Close),
      Closed::No => {}
    }

    let Ok(points) = surface.logical();
    let Ok(scale) = surface.scale();
    let layout = points.map(|window_size| {
      let Ok(window_layout) = Layout::new(window_size, camera);
      window_layout
    });

    match &layout {
      Some(visible) => {
        let current = Drawing::Ready {
          points: visible.points,
          scale,
        };

        match (self.shown != current).then_some(()) {
          Some(()) => {
            view::paint(surface, visible, camera, wearing)?;
            self.shown = current;
          }
          None => {}
        }
      }
      None => {}
    }

    let wait = camera.stream.as_ref().map(|_| Duration::from_millis(33));
    surface.wait(&[], wait)?;
    let Ok(events) = surface.pointer_events();
    let Ok(keys) = surface.keyboard_events();

    for event in events {
      let Ok(change) = self.pointer(camera, layout.as_ref(), event);

      match change {
        WindowChange::Continue => {}
        WindowChange::Close => return Ok(WindowChange::Close),
      }
    }

    'keypress: for KeyboardEvent::Down { key } in keys {
      let action = match key {
        Keysym::Escape | Keysym::q => return Ok(WindowChange::Close),
        Keysym::Left => Action::Previous,
        Keysym::Right => Action::Next,
        Keysym::r => Action::Retry,
        Keysym::space | Keysym::Return | Keysym::KP_Enter => match &camera.stream {
          Some(_stream) => Action::Take,
          None => Action::Retry,
        },
        _ => continue 'keypress,
      };
      let Ok(()) = camera.act(action);
      self.shown = Drawing::Pending;
    }

    let now = Instant::now();
    let Ok(refresh) = camera.poll(now);

    match refresh {
      Refresh::Changed => self.shown = Drawing::Pending,
      Refresh::Unchanged => {}
    }

    Ok(WindowChange::Continue)
  }

  fn pointer(
    &mut self,
    camera: &mut Camera,
    layout: Option<&Layout>,
    event: PointerEvent,
  ) -> Result<WindowChange, Infallible> {
    let at = |point| {
      layout.and_then(|window_layout| {
        let Ok(action) = window_layout.action(point);
        action
      })
    };

    match event {
      PointerEvent::Down { at: point } => self.pressed = at(point),
      PointerEvent::Moved { at: point } => {
        self.pressed = self.pressed.filter(|action| at(point) == Some(*action));
      }
      PointerEvent::Up => match self.pressed.take() {
        Some(Action::Close) => return Ok(WindowChange::Close),
        Some(action) => {
          let Ok(()) = camera.act(action);
          self.shown = Drawing::Pending;
        }
        None => {}
      },
      PointerEvent::Left | PointerEvent::Pinched { .. } => self.pressed = None,
      PointerEvent::Scrolled { .. } => {}
    }

    Ok(WindowChange::Continue)
  }
}
