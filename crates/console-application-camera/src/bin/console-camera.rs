use std::convert::Infallible;
use std::error::Error;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use console_core_color::palette::Wearing;
use console_core_geometry::Size;
use console_core_internal_programs::InternalProgram;
use console_core_iteration::{Step, iterate};
use console_core_number_conversion::index;
use console_panel::keys::{self, Driving, Meaning};
use console_panel::strip::{self, Stop};
use console_panel::ui::shapes::Pixels;
use console_panel::ui::surface::{
  Closed, Hover, KeyboardEvent, Keysym, PointerCursor, PointerEvent, RepaintHover, Scale,
  Surface, SurfaceError, Window,
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
      Action::Source(index) => self.select_source(index),
      Action::Previous => self.switch_source(-1),
      Action::Next => self.switch_source(1),
      Action::Retry => self.discover(),
      Action::Take => self.take_photo(),
      Action::Close => Ok(()),
    }
  }

  fn switch_source(&mut self, step: i32) -> Result<(), Infallible> {
    let Ok(many) = console_core_number_conversion::fitted::<_, u32>(self.sources.len());
    let Ok(next) = strip::along(many, Stop::Tab(self.selected), step);

    match next {
      Stop::Tab(source) => self.select_source(source),
      Stop::Outside => Ok(()),
    }
  }

  fn select_source(&mut self, source: u32) -> Result<(), Infallible> {
    let Ok(index) = index(source);

    match (self.selected != source && self.sources.get(index).is_some()).then_some(()) {
      Some(()) => {
        self.selected = source;
        self.start()
      }
      None => Ok(()),
    }
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
    hover: Hover::default(),
    shown: Drawing::Pending,
    layout: None,
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
  hover: Hover<(console_panel::canvas::Area, Action)>,
  shown: Drawing,
  layout: Option<Layout>,
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

    match points {
      Some(size) => {
        match (self.layout.as_ref().map(|layout| layout.points) != Some(size)).then_some(()) {
          Some(()) => {
            let Ok(layout) = Layout::new(size, camera, wearing);
            self.layout = Some(layout);
          }
          None => {}
        }
      }
      None => self.layout = None,
    }

    match &self.layout {
      Some(visible) => {
        let Ok(pointer) = surface.pointer_position();
        let target = pointer.and_then(|point| {
          let Ok(hit) = visible.hit(point, camera);

          hit
        });
        let Ok(feedback) = self.hover.observe(target);
        let cursor = match target {
          Some(_) => PointerCursor::Hand,
          None => PointerCursor::Default,
        };
        let Ok(()) = surface.set_pointer_cursor(cursor);
        let current = Drawing::Ready {
          points: visible.points,
          scale,
        };

        match ((self.shown != current).then_some(()), feedback) {
          (Some(()), _) | (None, Some(RepaintHover)) => {
            view::paint(
              surface,
              visible,
              camera,
              (wearing, target.map(|(area, _)| area)),
            )?;
            self.shown = current;
          }
          (None, None) => {}
        }
      }
      None => {}
    }

    let wait = camera.stream.as_ref().map(|_| Duration::from_millis(33));
    surface.wait(&[], wait)?;
    let Ok(events) = surface.pointer_events();
    let Ok(keys) = surface.keyboard_events();

    for event in events {
      let Ok(change) = self.pointer(camera, event);

      match change {
        WindowChange::Continue => {}
        WindowChange::Close => return Ok(WindowChange::Close),
      }
    }

    'keypress: for KeyboardEvent::Down { key } in keys {
      let Ok(meaning) = keys::meaning(key, Driving::Panel);
      let action = match (meaning, key) {
        (Meaning::Close, _) | (_, Keysym::q) => return Ok(WindowChange::Close),
        (Meaning::Tab(-1) | Meaning::Nudge(-1), _) => Action::Previous,
        (Meaning::Tab(1) | Meaning::Nudge(1), _) => Action::Next,
        (_, Keysym::r) => Action::Retry,
        (Meaning::Choose, _) => match view::capture_action(camera) {
          Ok(Some(action)) => action,
          Ok(None) => continue 'keypress,
        },
        _ => continue 'keypress,
      };
      let Ok(()) = self.act(camera, action);
    }

    let now = Instant::now();
    let Ok(refresh) = camera.poll(now);

    match refresh {
      Refresh::Changed => self.shown = Drawing::Pending,
      Refresh::Unchanged => {}
    }

    Ok(WindowChange::Continue)
  }

  fn act(&mut self, camera: &mut Camera, action: Action) -> Result<(), Infallible> {
    let Ok(()) = camera.act(action);
    self.shown = Drawing::Pending;

    match action {
      Action::Retry | Action::Previous | Action::Next | Action::Source(_) => self.layout = None,
      Action::Take | Action::Close => {}
    }

    Ok(())
  }

  fn pointer(
    &mut self,
    camera: &mut Camera,
    event: PointerEvent,
  ) -> Result<WindowChange, Infallible> {
    let at = |point| {
      self.layout.as_ref().and_then(|window_layout| {
        let Ok(action) = window_layout.action(point, camera);
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
          let Ok(()) = self.act(camera, action);
        }
        None => {}
      },
      PointerEvent::Left | PointerEvent::Pinched { .. } => self.pressed = None,
      PointerEvent::Scrolled { .. } => {}
    }

    Ok(WindowChange::Continue)
  }
}
