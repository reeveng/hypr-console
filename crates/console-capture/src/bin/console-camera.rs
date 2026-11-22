use std::error::Error;
use std::process::ExitCode;
use std::time::Duration;

use console_core_color::palette::Wearing;
use console_core_geometry::Size;
use console_core_internal_programs::InternalProgram;
use console_core_shapes::Pixels;
use console_draw_surface::{
  Closed, KeyboardEvent, Keysym, PointerEvent, Surface, SurfaceError, Window,
};

#[path = "console-camera/source.rs"]
mod source;
#[path = "console-camera/view.rs"]
mod view;

use source::{Source, Stream};
use view::{Action, Layout};

struct Camera {
  sources: Vec<Source>,
  selected: usize,
  stream: Option<Stream>,
  frame: Option<Pixels>,
  message: String,
}

impl Camera {
  fn discover(&mut self) {
    self.stream = None;
    self.frame = None;
    let previous = self
      .sources
      .get(self.selected)
      .map(|source| source.path.clone());
    self.sources = match source::discover() {
      Ok(sources) => sources,
      Err(fault) => {
        self.sources.clear();
        self.selected = 0;
        self.failed(
          "Couldn’t access a camera. Check its connection and permissions.",
          &fault,
        );
        return;
      }
    };
    self.selected = self
      .sources
      .iter()
      .position(|source| Some(&source.path) == previous.as_ref())
      .unwrap_or(0);
    self.start();
  }

  fn start(&mut self) {
    self.stream = None;
    self.frame = None;
    self.message.clear();
    let Some(source) = self.sources.get(self.selected) else {
      self.message = "No camera found. Connect a USB camera, then press Retry.".to_owned();
      return;
    };

    match Stream::open(source) {
      Ok(stream) => {
        self.stream = Some(stream);
        self.message = "Starting camera…".to_owned();
      }
      Err(fault) => self.failed("Couldn’t start the camera.", &fault),
    }
  }

  fn poll(&mut self) -> bool {
    let Some(stream) = &mut self.stream else {
      return false;
    };
    match stream.frame() {
      Ok(Some(frame)) => {
        if self.frame.is_none() {
          self.message.clear();
        }
        self.frame = Some(frame);
        true
      }
      Ok(None) => false,
      Err(fault) => {
        self.failed(
          "Camera stopped. Check the connection or close other camera apps.",
          &fault,
        );
        true
      }
    }
  }

  fn failed(&mut self, message: &str, fault: &std::io::Error) {
    console_core_logging::warn!("camera: {fault}");
    self.stream = None;
    self.frame = None;
    self.message = message.to_owned();
  }

  fn act(&mut self, action: Action) {
    match action {
      Action::Previous if self.sources.len() > 1 => {
        self.selected = self
          .selected
          .checked_sub(1)
          .unwrap_or(self.sources.len() - 1);
        self.start();
      }
      Action::Next if self.sources.len() > 1 => {
        self.selected = (self.selected + 1) % self.sources.len();
        self.start();
      }
      Action::Retry => self.discover(),
      Action::Take => {
        if let Some(frame) = &self.frame {
          self.message = match console_capture::save_photo(frame) {
            Ok(path) => format!(
              "Saved to Pictures · {}",
              path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
            ),
            Err(fault) => {
              console_core_logging::warn!("saving photo: {fault}");
              "Couldn’t save the photo".to_owned()
            }
          };
        }
      }
      Action::Previous | Action::Next | Action::Close => {}
    }
  }
}

fn main() -> ExitCode {
  match open() {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      console_core_logging::error!("console-camera: {fault}");
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
  camera.discover();
  run(&mut surface, &wearing, &mut camera)?;
  Ok(())
}

fn run(
  surface: &mut Surface,
  wearing: &Wearing,
  camera: &mut Camera,
) -> Result<(), SurfaceError> {
  let mut pressed = None;
  let mut geometry = None;
  let mut changed = true;

  loop {
    if surface.closed() == Ok(Closed::Yes) {
      return Ok(());
    }
    let Ok(points) = surface.logical();
    let Ok(scale) = surface.scale();
    let layout = points.map(|points| Layout::new(points, camera));
    if let Some(layout) = &layout {
      let now = (layout.points, scale);
      if changed || geometry != Some(now) {
        view::paint(surface, layout, camera, wearing)?;
        geometry = Some(now);
      }
    }
    changed = false;

    let wait = camera.stream.as_ref().map(|_| Duration::from_millis(33));
    surface.wait(&[], wait)?;
    let Ok(events) = surface.pointer_events();
    let Ok(keys) = surface.keyboard_events();
    for event in events {
      let at = |point| layout.as_ref().and_then(|layout| layout.action(point));
      match event {
        PointerEvent::Down { at: point } => pressed = at(point),
        PointerEvent::Moved { at: point } => {
          if pressed != at(point) {
            pressed = None;
          }
        }
        PointerEvent::Up => {
          if let Some(action) = pressed.take() {
            if action == Action::Close {
              return Ok(());
            }
            camera.act(action);
            changed = true;
          }
        }
        PointerEvent::Left | PointerEvent::Pinched { .. } => pressed = None,
        PointerEvent::Scrolled { .. } => {}
      }
    }

    for KeyboardEvent::Down { key } in keys {
      let action = match key {
        Keysym::Escape | Keysym::q => return Ok(()),
        Keysym::Left => Action::Previous,
        Keysym::Right => Action::Next,
        Keysym::r => Action::Retry,
        Keysym::space | Keysym::Return | Keysym::KP_Enter if camera.stream.is_none() => {
          Action::Retry
        }
        Keysym::space | Keysym::Return | Keysym::KP_Enter => Action::Take,
        _ => continue,
      };
      camera.act(action);
      changed = true;
    }
    changed |= camera.poll();
  }
}
