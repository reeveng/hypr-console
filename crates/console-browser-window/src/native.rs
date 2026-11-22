//! A Console toplevel presents frames and sends Console input to the selected renderer.

#![forbid(unsafe_code)]

use std::error::Error;
use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::os::fd::AsFd;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use console_browser_renderer::{MAX_SIDE, invalid, validate};
use console_core_arguments::{Command, Flag, Operands, Presence, Takes};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_shapes::{Picture, Pixels};
use console_core_web_engine::{Event, FrameLayout, Request, decoded_event};
use console_default_applications::engines;
use console_draw_painting::{self as painting, Frame};
use console_draw_surface::{
  Closed, KeyPress, LayerSupport, Part, PointerEvent, Surface, Visible, Window,
};
use console_input_key_symbols::Keysym;

use crate::reader::{Article, Mode, Reader, TextSize};
use crate::renderer::{Renderer, Worker};
use crate::toolbar::{
  Action, ButtonAvailability, Direction, HEIGHT as TOOLBAR_HEIGHT, MAX_TABS, PageLoad, Status,
  TabLabel, Toolbar, ToolbarChange,
};

const WINDOW: Size<u32> = Size {
  width: 1100,
  height: 750,
};
const BLANK: &str = "about:blank";
const APP_ID: &str = "console-browser-window";
const TITLE: &str = "Console Browser";
const SERVO_WORKER: Flag = Flag {
  spelling: "--servo-worker",
  takes: Takes::Value("PATH"),
  about: "enable the Servo renderer with the given worker",
};
const WEBKIT_WORKER: Flag = Flag {
  spelling: "--webkit-worker",
  takes: Takes::Value("PATH"),
  about: "enable the WebKit renderer with the given worker",
};
const ENGINE: Flag = Flag {
  spelling: "--engine",
  takes: Takes::Value("servo|chromium|webkit"),
  about: "choose the initial engine for new tabs",
};
const SMOKE: Flag = Flag {
  spelling: "--smoke-test",
  takes: Takes::None,
  about: "check Chromium HTML, CSS and JavaScript in the Console window",
};
const SERVO_TEST: Flag = Flag {
  spelling: "--servo-test",
  takes: Takes::None,
  about: "reload a test page in Chromium, Servo, and Chromium again",
};
const WEBKIT_TEST: Flag = Flag {
  spelling: "--webkit-test",
  takes: Takes::None,
  about: "reload a test page in Chromium, WebKit, and Chromium again",
};
const COMMAND: Command = Command {
  name: APP_ID,
  about: "open a page in the Console browser window",
  flags: &[
    SERVO_WORKER,
    WEBKIT_WORKER,
    ENGINE,
    SMOKE,
    SERVO_TEST,
    WEBKIT_TEST,
  ],
  operands: Operands::Optional("ADDRESS"),
};

struct Browser {
  surface: Surface,
  toolbar: Toolbar,
  programs: RendererPrograms,
  default_engine: Renderer,
  active: Tab,
  background: Vec<Tab>,
  next_tab: u32,
  viewport: Size<u32>,
  desktop_font: console_core_fonts::Font,
  clipboard: String,
  bar: ToolbarFrame,
}

struct Tab {
  id: u32,
  renderer: Renderer,
  worker: Option<Worker>,
  address: String,
  title: String,
  page: PageFrame,
  loading: PageLoad,
  reader_id: u64,
  reader_text_size: u32,
}

struct RendererPrograms {
  executable: PathBuf,
  servo: Option<PathBuf>,
  webkit: Option<PathBuf>,
}

impl RendererPrograms {
  fn preferred(&self) -> Renderer {
    match self.servo {
      Some(_) => Renderer::Servo,
      None => Renderer::Chromium,
    }
  }

  fn available(&self) -> Result<&'static [Renderer], Never> {
    use Renderer::{Chromium, Servo, WebKit};

    Ok(match (self.webkit.as_ref(), self.servo.as_ref()) {
      (None, None) => &[Chromium],
      (Some(_), None) => &[Chromium, WebKit],
      (None, Some(_)) => &[Chromium, Servo],
      (Some(_), Some(_)) => &[Chromium, WebKit, Servo],
    })
  }

  fn for_renderer(&self, renderer: Renderer) -> io::Result<&Path> {
    match renderer {
      Renderer::Chromium => Ok(&self.executable),
      Renderer::Servo => self
        .servo
        .as_deref()
        .ok_or_else(|| io::Error::other("Servo worker is not configured")),
      Renderer::WebKit => self
        .webkit
        .as_deref()
        .ok_or_else(|| io::Error::other("WebKit worker is not configured")),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Redraw {
  Requested,
  Current,
}

struct PageFrame {
  picture: Option<Pixels>,
  receiving: Vec<u8>,
  redraw: Redraw,
  reader: Reader,
}

struct ToolbarFrame {
  visible: Option<PaintedToolbar>,
  redraw: Redraw,
}

struct PaintedToolbar {
  frame: Frame,
  pixels: Vec<u8>,
}

const MAX_RGBA_BYTES: u64 = 16_777_216;

fn launch_arguments(mut args: Vec<OsString>) -> Result<Vec<String>, OsString> {
  let operand = match args.as_mut_slice() {
    [one] => Some(one),
    [separator, one] => (separator == "--").then_some(one),
    _ => None,
  };

  match operand {
    Some(operand_2) => match Path::new(&*operand_2).is_absolute().then_some(()) {
      Some(()) => {
        let Ok(url) = console_core_file_urls::url(Path::new(&*operand_2));
        *operand_2 = OsString::from(url);
      }
      None => {}
    },
    None => {}
  }

  args.into_iter().map(OsString::into_string).collect()
}

fn resolve(typed: &str) -> Result<Option<String>, Never> {
  let path = Path::new(typed);

  match path.is_absolute().then_some(()) {
    Some(()) => return console_core_file_urls::url(path).map(Some),
    None => {}
  }

  let address = typed.trim();

  match (address == BLANK || address.starts_with("data:")).then_some(()) {
    Some(()) => return Ok(Some(address.to_owned())),
    None => {}
  }

  let file = address
    .split_once(':')
    .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("file"));

  match file {
    Some((_, rest)) => {
      let absolute = match rest.strip_prefix("//") {
        Some(authority_and_path) => authority_and_path
          .split_once('/')
          .is_some_and(|(host, _)| host.is_empty() || host.eq_ignore_ascii_case("localhost")),
        None => rest.starts_with('/'),
      };

      return Ok(absolute.then(|| address.to_owned()));
    }
    None => {}
  }

  let Ok(current) = engines::current();
  let Ok(engine) = engines::one(&current);
  let engine_3 = match engine {
    Some(engine_2) => engine_2,
    None => return Ok(None),
  };
  let Ok(address_2) = engines::address(address, engine_3);

  Ok(address_2.filter(|address_3| {
    address_3.split_once("://").is_some_and(|(scheme, _)| {
      scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    })
  }))
}

fn nearby_worker(executable: &Path, name: &str) -> Result<Option<PathBuf>, Never> {
  available_worker(executable.with_file_name(name))
}

fn available_worker(worker: PathBuf) -> Result<Option<PathBuf>, Never> {
  Ok(match std::fs::metadata(&worker) {
    Ok(metadata) => metadata.is_file().then_some(worker),
    Err(fault) => {
      console_core_logging::debug!(
        "Browser worker unavailable at {}: {fault}",
        worker.display()
      );

      None
    }
  })
}

fn page_key(key: Keysym) -> Option<&'static str> {
  Some(match key {
    Keysym::Return | Keysym::KP_Enter => "Return",
    Keysym::Tab | Keysym::ISO_Left_Tab => "Tab",
    Keysym::BackSpace => "BackSpace",
    Keysym::Delete => "Delete",
    Keysym::Escape => "Escape",
    Keysym::Home => "Home",
    Keysym::End => "End",
    Keysym::Page_Up => "Page_Up",
    Keysym::Page_Down => "Page_Down",
    Keysym::Left => "Left",
    Keysym::Right => "Right",
    Keysym::Up => "Up",
    Keysym::Down => "Down",
    _ => return None,
  })
}

impl Tab {
  fn new(id: u32, renderer: Renderer, address: String) -> Self {
    Self {
      id,
      renderer,
      worker: None,
      address,
      title: TITLE.to_owned(),
      page: PageFrame {
        picture: None,
        receiving: Vec::new(),
        redraw: Redraw::Requested,
        reader: Reader::Hidden,
      },
      loading: PageLoad::Settled,
      reader_id: 0,
      reader_text_size: crate::reader::DEFAULT_TEXT_SIZE,
    }
  }

  fn switch(
    &mut self,
    renderer: Renderer,
    executable: &Path,
    viewport: Size<u32>,
  ) -> io::Result<()> {
    let worker = Worker::start(renderer, executable)?;
    worker.request(&Request::Resize(viewport))?;
    worker.request(&Request::Open(self.address.clone()))?;
    self.worker = Some(worker);
    self.renderer = renderer;
    self.page.picture = None;
    self.hide_reader();
    self.loading = PageLoad::Loading;
    self.title = TITLE.to_owned();
    Ok(())
  }

  fn request(&self, request: Request) -> io::Result<()> {
    match &self.worker {
      Some(worker) => worker.request(&request),
      None => Ok(()),
    }
  }

  fn label(&self) -> TabLabel {
    let title = match (self.title.is_empty() || self.title == TITLE).then_some(()) {
      Some(()) => match self.address.as_str() {
        BLANK => "New tab",
        address => address,
      },
      None => &self.title,
    };
    TabLabel {
      id: self.id,
      title: title.chars().take(160).collect(),
    }
  }

  fn hide_reader(&mut self) {
    self.page.reader = Reader::Hidden;
    self.page.redraw = Redraw::Requested;
  }

  fn read(&mut self, mode: Mode) -> io::Result<()> {
    match (self.page.reader.mode() == Some(mode)).then_some(()) {
      Some(()) => {
        self.hide_reader();
        return Ok(());
      }
      None => {}
    }
    match &mut self.page.reader {
      Reader::Open { mode: current, .. } | Reader::Pending { mode: current, .. } => {
        *current = mode;
      }
      Reader::Hidden => {
        self.reader_id = self
          .reader_id
          .checked_add(1)
          .ok_or_else(|| io::Error::other("reader request numbers exhausted"))?;
        match self.worker {
          Some(_) => {
            self.request(Request::Read(self.reader_id))?;
            self.page.reader = Reader::Pending {
              id: self.reader_id,
              mode,
            };
          }
          None => self.show_article(mode, String::new()),
        }
      }
    }
    self.page.redraw = Redraw::Requested;
    Ok(())
  }

  fn show_article(&mut self, mode: Mode, text: String) {
    let text = match text.trim() {
      "" => "No readable text on this page.".to_owned(),
      _ => text,
    };
    let article = Article::new(self.title.clone(), text);
    self.page.reader = Reader::Open { mode, article };
    self.page.redraw = Redraw::Requested;
  }

  fn stopped(&mut self, fault: io::Error) {
    console_core_logging::debug!("{} tab {} stopped: {fault}", self.renderer, self.id);
    self.worker = None;
    self.loading = PageLoad::Settled;
    match self.page.reader {
      Reader::Pending { mode, .. } => self.show_article(mode, String::new()),
      Reader::Hidden | Reader::Open { .. } => {}
    }
    self.title = format!("{} stopped. Reload to reopen the page.", self.renderer);
  }

  fn receive(&mut self) -> io::Result<bool> {
    let mut changed = false;
    for _ in 0..32 {
      let worker = match &self.worker {
        Some(worker) => worker,
        None => break,
      };
      let packet = match worker.receive() {
        Ok(packet) => packet,
        Err(error) => match error.kind() {
          io::ErrorKind::WouldBlock => break,
          _ => return Err(error),
        },
      };
      let Ok(event) = decoded_event(&packet.line);
      match (event, packet.frame) {
        (Some(Event::Frame(size)), Some(fd)) => {
          match self.address.as_str() {
            BLANK => {}
            _ => self.page.receive(size, fd)?,
          }
          worker.release_frame()?;
        }
        (Some(Event::Address(address)), None) => {
          match (address != self.address).then_some(()) {
            Some(()) => self.cancel_pending_reader(),
            None => {}
          }
          self.clear_blank_page(&address);
          self.address = address;
          changed = true;
        }
        (Some(Event::Title(title)), None) => {
          self.title = title;
          changed = true;
        }
        (Some(Event::Started), None) => {
          self.cancel_pending_reader();
          self.loading = PageLoad::Loading;
          changed = true;
        }
        (Some(Event::Finished), None) => {
          self.loading = PageLoad::Settled;
          changed = true;
        }
        (Some(Event::Reader(id, text)), None) => match self.page.reader {
          Reader::Pending { id: pending, mode } => match (pending == id).then_some(()) {
            Some(()) => {
              self.show_article(mode, text);
              changed = true;
            }
            None => {}
          },
          Reader::Hidden | Reader::Open { .. } => {}
        },
        _ => return Err(invalid("unexpected renderer message")),
      }
    }
    Ok(changed)
  }

  fn cancel_pending_reader(&mut self) {
    match self.page.reader {
      Reader::Pending { .. } => self.hide_reader(),
      Reader::Hidden | Reader::Open { .. } => {}
    }
  }

  fn clear_blank_page(&mut self, address: &str) {
    match address {
      BLANK => {
        self.page.picture = None;
        self.page.redraw = Redraw::Requested;
      }
      _ => {}
    }
  }

  fn update(&mut self) -> bool {
    match self.receive() {
      Ok(changed) => changed,
      Err(fault) => {
        self.stopped(fault);
        true
      }
    }
  }
}

impl Browser {
  fn dirty_toolbar(&mut self) -> Result<(), Never> {
    self.bar.redraw = Redraw::Requested;
    Ok(())
  }

  fn controls(&mut self) -> Result<(), Never> {
    let navigation = match self.active.worker {
      Some(_) => ButtonAvailability::Enabled,
      None => ButtonAvailability::Disabled,
    };
    let Ok(status) = self.toolbar.status(Status {
      loading: self.active.loading,
      back: navigation,
      forward: navigation,
      renderer: self.active.renderer,
      progress: 0,
      reader: self.active.page.reader.mode(),
    });
    let Ok(address) = self.toolbar.address(&self.active.address);
    let mut labels: Vec<_> = self.background.iter().map(Tab::label).collect();
    labels.push(self.active.label());
    labels.sort_unstable_by_key(|tab| tab.id);
    let tabs = self.toolbar.tabs(labels, self.active.id);
    match (status, address, tabs) {
      (ToolbarChange::Unchanged, ToolbarChange::Unchanged, ToolbarChange::Unchanged) => {}
      _ => {
        let Ok(()) = self.dirty_toolbar();
      }
    }
    self.surface.set_window_title(&self.active.title)
  }

  fn switch(&mut self, renderer: Renderer) -> io::Result<()> {
    let executable = self.programs.for_renderer(renderer)?;
    self.active.switch(renderer, executable, self.viewport)?;
    let Ok(()) = self.controls();
    Ok(())
  }

  fn request(&mut self, request: Request) -> io::Result<()> {
    let result = self.active.request(request);
    self.handle_worker_result(result)
  }

  fn handle_worker_result(&mut self, result: io::Result<()>) -> io::Result<()> {
    match result {
      Ok(()) => Ok(()),
      Err(fault) => match fault.kind() {
        io::ErrorKind::BrokenPipe
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::UnexpectedEof => {
          self.active.stopped(fault);
          let Ok(()) = self.controls();
          Ok(())
        }
        _ => Err(fault),
      },
    }
  }

  fn receive(&mut self) {
    let mut changed = self.active.update();
    for tab in &mut self.background {
      changed |= tab.update();
    }
    match changed.then_some(()) {
      Some(()) => {
        let Ok(()) = self.controls();
      }
      None => {}
    }
  }

  fn new_tab(&mut self) -> io::Result<()> {
    match self.background.len().saturating_add(1).cmp(&MAX_TABS) {
      std::cmp::Ordering::Less => {}
      std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => return Ok(()),
    }
    let next = self
      .next_tab
      .checked_add(1)
      .ok_or_else(|| io::Error::other("tab numbers exhausted"))?;
    let mut tab = Tab::new(self.next_tab, self.default_engine, BLANK.to_owned());
    let executable = self.programs.for_renderer(self.default_engine)?;
    tab.switch(self.default_engine, executable, self.viewport)?;
    self.next_tab = next;
    self
      .background
      .push(std::mem::replace(&mut self.active, tab));
    let Ok(()) = self.tab_changed();
    let Ok(()) = self.toolbar.edit();
    Ok(())
  }

  fn select_tab(&mut self, id: u32, keep_current: bool) {
    let index = match self.background.iter().position(|tab| tab.id == id) {
      Some(index) => index,
      None => return,
    };
    let mut tab = self.background.swap_remove(index);
    // A disconnected background tab can still be selected and reloaded.
    match tab.request(Request::Resize(self.viewport)) {
      Ok(()) => {}
      Err(fault) => tab.stopped(fault),
    }
    let previous = std::mem::replace(&mut self.active, tab);
    match keep_current.then_some(previous) {
      Some(previous) => self.background.push(previous),
      None => {}
    }
    let Ok(()) = self.tab_changed();
  }

  fn adjacent_tab(&self, direction: Direction) -> Option<u32> {
    let id = self.active.id;
    let ids = self.background.iter().map(|tab| tab.id);
    match direction {
      Direction::Previous => ids
        .clone()
        .filter(|candidate| *candidate < id)
        .max()
        .or_else(|| ids.max()),
      Direction::Next => ids
        .clone()
        .filter(|candidate| *candidate > id)
        .min()
        .or_else(|| ids.min()),
    }
  }

  fn tab_changed(&mut self) -> Result<(), Never> {
    self.active.page.redraw = Redraw::Requested;
    let Ok(()) = self.toolbar.blur(&self.active.address);
    self.controls()
  }

  fn action(&mut self, action: Action) -> io::Result<Closed> {
    match action {
      Action::Pass => return Ok(Closed::No),
      Action::Close => return Ok(Closed::Yes),
      Action::CloseTab => match self.adjacent_tab(Direction::Next) {
        Some(id) => self.select_tab(id, false),
        None => return Ok(Closed::Yes),
      },
      Action::NewTab => match self.new_tab() {
        Ok(()) => {}
        Err(fault) => self.report_action(fault),
      },
      Action::SelectTab(id) => self.select_tab(id, true),
      Action::CycleTab(direction) => match self.adjacent_tab(direction) {
        Some(id) => self.select_tab(id, true),
        None => {}
      },
      Action::Reader(mode) => {
        match mode {
          Mode::Desktop => {
            self.desktop_font = crate::reader::desktop_font();
            self.toolbar.refresh_palette();
          }
          Mode::Dark => {}
        }
        let result = self.active.read(mode);
        self.handle_worker_result(result)?;
        let Ok(()) = self.toolbar.blur(&self.active.address);
      }
      Action::PageView => self.active.hide_reader(),
      Action::SettingsChanged => self.active.page.redraw = Redraw::Requested,
      Action::ReaderText(size) => {
        let height = self.active.reader_text_size;
        self.active.reader_text_size = match size {
          TextSize::Smaller => height.saturating_sub(2).max(14),
          TextSize::Larger => height.saturating_add(2).min(34),
        };
        self.active.page.redraw = Redraw::Requested;
      }
      Action::Handled | Action::FocusPage | Action::Compose => {}
      Action::Back | Action::Forward | Action::Reload => self.navigate(action)?,
      Action::Stop => self.request(Request::Stop)?,
      Action::Switch(renderer) => {
        match (self.active.renderer == renderer)
          .then_some(self.active.worker.as_ref())
          .flatten()
        {
          Some(_) => {}
          None => match self.switch(renderer) {
            Ok(()) => {}
            Err(fault) => self.report_action(fault),
          },
        }
      }
      Action::Open(text) => self.open(&text)?,
      Action::Copy(text) => self.clipboard = text,
      Action::Paste => {
        let Ok(()) = self.toolbar.text(&self.clipboard);
      }
    }

    let Ok(()) = self.controls();
    let Ok(()) = self.dirty_toolbar();
    Ok(Closed::No)
  }

  fn navigate(&mut self, action: Action) -> io::Result<()> {
    self.active.hide_reader();
    match (action, &self.active.worker) {
      (Action::Back, _) => self.request(Request::Back),
      (Action::Forward, _) => self.request(Request::Forward),
      (Action::Reload, Some(_)) => self.request(Request::Reload),
      (Action::Reload, None) => {
        match self.switch(self.active.renderer) {
          Ok(()) => {}
          Err(fault) => self.report_action(fault),
        }
        Ok(())
      }
      _ => Ok(()),
    }
  }

  fn report_action(&mut self, fault: io::Error) {
    console_core_logging::warn!("browser action failed: {fault}");
    self.active.title = fault.to_string();
  }

  fn open(&mut self, typed: &str) -> io::Result<()> {
    let Ok(address) = resolve(typed);
    match address {
      Some(address) => {
        self.active.hide_reader();
        self.active.clear_blank_page(&address);
        self.active.address = address;
        let Ok(()) = self.toolbar.blur(&self.active.address);
        match self.active.worker {
          Some(_) => self.request(Request::Open(self.active.address.clone())),
          None => self.navigate(Action::Reload),
        }
      }
      None => {
        self.active.title = "Enter a web address or search term".to_owned();
        Ok(())
      }
    }
  }

  fn keyboard(&mut self, press: KeyPress) -> io::Result<Closed> {
    let control =
      u32::from(press.modifiers & ::core::primitive::u32::wrapping_shl(1, 2_u32) != 0);
    let shift = ::core::primitive::u32::wrapping_shl(u32::from(press.modifiers & 1 != 0), 1_u32);
    let alt = ::core::primitive::u32::wrapping_shl(
      u32::from(press.modifiers & ::core::primitive::u32::wrapping_shl(1, 3_u32) != 0),
      2_u32,
    );
    let super_key = ::core::primitive::u32::wrapping_shl(
      u32::from(press.modifiers & ::core::primitive::u32::wrapping_shl(1, 6_u32) != 0),
      3_u32,
    );
    let Ok(character) = press.key.char();
    let character_2 = character.filter(|letter| !letter.is_control());

    let Ok(action) = self.toolbar.key(
      press.key,
      control | shift | alt | super_key,
      &self.active.address,
    );

    match action {
      Action::Compose => {
        match character_2 {
          Some(character) => {
            let Ok(()) = self.toolbar.text(&character.to_string());
          }
          None => {}
        }
        let Ok(()) = self.dirty_toolbar();
        Ok(Closed::No)
      }
      Action::Pass => {
        match (&mut self.active.page.reader, press.key) {
          (Reader::Open { article, .. }, key) => {
            article.key(key, self.viewport.height);
            self.active.page.redraw = Redraw::Requested;
          }
          (Reader::Pending { .. }, _) => {}
          (Reader::Hidden, key) => {
            match (control | alt | super_key, character_2, page_key(key)) {
              (0, Some(text), _) => self.request(Request::Text(text.to_string()))?,
              (0, None, Some(key)) => self.request(Request::Key(key.to_owned()))?,
              _ => {}
            }
          }
        }
        Ok(Closed::No)
      }
      action => self.action(action),
    }
  }

  fn pointer(&mut self, pointer: PointerEvent) -> io::Result<Closed> {
    let toolbar_height = f64::from(TOOLBAR_HEIGHT);

    match pointer {
      PointerEvent::Down { at: (x, y) } => {
        match (y < toolbar_height || self.toolbar.settings_visible()).then_some(()) {
          Some(()) => self.press_toolbar(Point { x, y }),
          None => {
            let Ok(()) = self.toolbar.blur(&self.active.address);
            let Ok(()) = self.dirty_toolbar();
            match self.active.page.reader {
              Reader::Hidden => {}
              Reader::Pending { .. } | Reader::Open { .. } => return Ok(Closed::No),
            }
            let placed = Point {
              x: x * f64::from(self.viewport.width.min(MAX_SIDE))
                / f64::from(self.viewport.width),
              y: (y - toolbar_height) * f64::from(self.viewport.height.min(MAX_SIDE))
                / f64::from(self.viewport.height),
            };
            self.request(Request::Click(placed))?;

            Ok(Closed::No)
          }
        }
      }
      PointerEvent::Scrolled { by } => {
        match self.toolbar.settings_visible().then_some(()) {
          Some(()) => return Ok(Closed::No),
          None => {}
        }
        match &mut self.active.page.reader {
          Reader::Open { article, .. } => {
            article.scroll(by, self.viewport.height);
            self.active.page.redraw = Redraw::Requested;
          }
          Reader::Pending { .. } => {}
          Reader::Hidden => self.request(Request::Scroll(Point { x: 0.0, y: by }))?,
        }

        Ok(Closed::No)
      }
      PointerEvent::Moved { at: (x, y) } => {
        let Ok(()) = self.hover_toolbar(Some(toolbar_point(Point { x, y })));
        Ok(Closed::No)
      }
      PointerEvent::Left => {
        let Ok(()) = self.hover_toolbar(None);
        Ok(Closed::No)
      }
      PointerEvent::Pinched { .. } | PointerEvent::Up => Ok(Closed::No),
    }
  }

  fn press_toolbar(&mut self, at: Point<f64>) -> io::Result<Closed> {
    let Ok(action) = self.toolbar.tap(toolbar_point(at), &self.active.address);

    match action {
      Action::Pass => {
        let Ok(()) = self.surface.move_window();

        Ok(Closed::No)
      }
      action => self.action(action),
    }
  }

  fn hover_toolbar(&mut self, at: Option<Point<i32>>) -> Result<(), Never> {
    match self.toolbar.hover(at) {
      ToolbarChange::Changed => self.dirty_toolbar(),
      ToolbarChange::Unchanged => Ok(()),
    }
  }

  fn refresh(&mut self) -> Result<(), Box<dyn Error>> {
    let Ok(logical) = self.surface.logical();
    let logical_3 = match logical {
      Some(logical_2) => logical_2,
      None => return Ok(()),
    };
    let bar_height = logical_3.height.min(TOOLBAR_HEIGHT);
    let viewport = Size {
      width: logical_3.width.max(1),
      height: logical_3.height.saturating_sub(bar_height).max(1),
    };

    match (viewport == self.viewport).then_some(()) {
      Some(()) => {}
      None => {
        self.viewport = viewport;
        self.active.page.redraw = Redraw::Requested;
        self.request(Request::Resize(viewport))?;
        let Ok(()) = self.dirty_toolbar();
      }
    }

    match &mut self.active.page.reader {
      Reader::Open { mode, article } => {
        let (family, size) = match mode {
          Mode::Dark => (console_core_fonts::SERIF, self.active.reader_text_size),
          Mode::Desktop => (self.desktop_font.family.as_str(), self.desktop_font.height),
        };
        article.prepare(viewport, family, size);
      }
      Reader::Hidden | Reader::Pending { .. } => {}
    }
    let desktop_colors = self.toolbar.reader_colors();
    let Ok(scale) = self.surface.scale();
    let points = Size {
      width: logical_3.width,
      height: bar_height,
    };
    let Ok(device) = scale.device(points);
    let frame = Frame { points, device };
    let layout = match FrameLayout::rgba(device) {
      Ok(layout) => layout,
      Err(fault) => {
        console_core_logging::debug!(
          "browser toolbar waiting for a drawable viewport: {fault:?}"
        );

        return Ok(());
      }
    };
    let Ok(geometry) = self.bar.geometry(frame);
    let bar_changed = self.bar.redraw == Redraw::Requested || geometry == Redraw::Requested;
    let page_changed =
      self.active.page.redraw == Redraw::Requested || geometry == Redraw::Requested;

    match bar_changed.then_some(()) {
      Some(()) => self.bar.paint(&mut self.toolbar, frame, layout)?,
      None => {}
    }

    match (bar_changed || page_changed).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    let bar = match self.bar.visible.as_ref() {
      Some(bar) => bar,
      None => return Ok(()),
    };
    let Ok(layers) = self.surface.layer_support();

    let settings = self.toolbar.settings_visible().then_some(());
    match (layers, settings) {
      (LayerSupport::Available, None) => {
        match bar_changed.then_some(()) {
          Some(()) => draw_background(&mut self.surface, bar)?,
          None => {}
        }

        match page_changed.then_some(()) {
          Some(()) => {}
          None => return Ok(()),
        }

        let shown = draw_page_layer(
          &mut self.surface,
          &self.active.page,
          PageLayout {
            viewport,
            entire: logical_3,
            bar_height,
          },
          desktop_colors,
        )?;

        match shown {
          Visible::Yes => {
            self.active.page.redraw = Redraw::Current;

            return Ok(());
          }
          Visible::NotYet => {}
        }

        let Ok(()) = self.surface.clear_layer();
      }
      (LayerSupport::Available, Some(())) => {
        let Ok(()) = self.surface.clear_layer();
      }
      (LayerSupport::Unavailable, _) => {}
    }

    draw_combined(
      &mut self.surface,
      bar,
      &self.active.page,
      PageLayout {
        viewport,
        entire: logical_3,
        bar_height,
      },
      desktop_colors,
      &mut self.toolbar,
    )?;
    self.active.page.redraw = Redraw::Current;

    Ok(())
  }
}

type ReaderColors = (console_core_color::Oklch, console_core_color::Oklch);

fn toolbar_point(at: Point<f64>) -> Point<i32> {
  let Ok(x) = console_core_number_conversion::toward_zero_i32(at.x.floor());
  let Ok(y) = console_core_number_conversion::toward_zero_i32(at.y.floor());
  Point { x, y }
}

impl PageFrame {
  fn paint(
    &self,
    pixels: &mut [u8],
    frame: Frame,
    desktop_colors: ReaderColors,
  ) -> Result<(), Never> {
    let result = match (&self.reader, &self.picture) {
      (Reader::Open { mode, article }, _) => {
        let colors = match mode {
          Mode::Dark => crate::reader::DARK,
          Mode::Desktop => desktop_colors,
        };
        article.paint(pixels, frame, colors)
      }
      (Reader::Hidden | Reader::Pending { .. }, Some(picture)) => {
        let Ok(()) = paint_page(pixels, frame, picture);
        return Ok(());
      }
      (Reader::Hidden | Reader::Pending { .. }, None) => {
        let background = crate::reader::background(frame.points, crate::reader::DARK.0);
        painting::onto(pixels, frame, &[background])
      }
    };
    match result {
      Ok(()) => {}
      Err(fault) => console_core_logging::error!("could not paint browser page: {fault}"),
    }
    Ok(())
  }

  fn receive(&mut self, size: Size<u32>, descriptor: std::os::fd::OwnedFd) -> io::Result<()> {
    match (size.width <= MAX_SIDE && size.height <= MAX_SIDE).then_some(()) {
      Some(()) => {}
      None => return Err(invalid("oversized viewport frame")),
    }

    let layout =
      FrameLayout::rgba(size).map_err(|_invalid_layout| invalid("invalid viewport frame"))?;
    validate(&descriptor, layout)?;
    resize_pixels(&mut self.receiving, layout)?;
    File::from(descriptor).read_exact_at(&mut self.receiving, 0)?;
    let bytes = std::mem::take(&mut self.receiving);
    let Ok(picture) = Pixels::packed(size, bytes);

    match self.picture.replace(picture) {
      Some(previous) => self.receiving = previous.bytes,
      None => {}
    }

    match self.reader {
      Reader::Open { .. } => {}
      Reader::Hidden | Reader::Pending { .. } => self.redraw = Redraw::Requested,
    }

    Ok(())
  }
}

impl ToolbarFrame {
  fn geometry(&self, requested: Frame) -> Result<Redraw, Never> {
    Ok(match self.visible.as_ref() {
      Some(previous) => match (requested == previous.frame).then_some(()) {
        Some(()) => Redraw::Current,
        None => Redraw::Requested,
      },
      None => Redraw::Requested,
    })
  }

  fn paint(
    &mut self,
    toolbar: &mut Toolbar,
    frame: Frame,
    layout: FrameLayout,
  ) -> Result<(), Box<dyn Error>> {
    let mut pixels = self
      .visible
      .take()
      .map_or_else(Vec::new, |previous| previous.pixels);
    resize_pixels(&mut pixels, layout)?;
    toolbar.paint(&mut pixels, frame)?;
    self.visible = Some(PaintedToolbar { frame, pixels });
    self.redraw = Redraw::Current;

    Ok(())
  }
}

#[cfg_attr(
  dylint_lib = "explicit088_no_arriving_length",
  expect(
    explicit088_no_arriving_length,
    reason = "the only allocation length below is checked against the explicit 16 MiB RGBA budget before converting to a native slice position"
  )
)]
fn resize_pixels(pixels: &mut Vec<u8>, layout: FrameLayout) -> io::Result<()> {
  match (layout.bytes <= MAX_RGBA_BYTES).then_some(()) {
    Some(()) => {}
    None => return Err(invalid("oversized framebuffer")),
  }

  let length =
    console_core_number_conversion::checked_index(layout.bytes).map_err(io::Error::other)?;
  pixels.resize(length, 0);

  Ok(())
}

fn fill_toolbar<'a>(
  pixels: &'a mut [u8],
  bar: &PaintedToolbar,
) -> Result<Option<&'a mut [u8]>, Never> {
  let (toolbar, page) = match pixels.split_at_mut_checked(bar.pixels.len()) {
    Some(parts) => parts,
    None => return Ok(None),
  };

  for (target, source) in toolbar.iter_mut().zip(&bar.pixels) {
    *target = *source;
  }

  Ok(Some(page))
}

fn draw_background(surface: &mut Surface, bar: &PaintedToolbar) -> Result<(), Box<dyn Error>> {
  surface.draw(|pixels, _, _| {
    let Ok(page) = fill_toolbar(pixels, bar);

    match page {
      Some(page_2) => page_2.fill(255),
      None => {}
    }

    Ok(())
  })?;

  Ok(())
}

struct PageLayout {
  viewport: Size<u32>,
  entire: Size<u32>,
  bar_height: u32,
}

fn draw_page_layer(
  surface: &mut Surface,
  page: &PageFrame,
  layout: PageLayout,
  desktop_colors: ReaderColors,
) -> Result<Visible, Box<dyn Error>> {
  match (layout.entire.height > layout.bar_height).then_some((&page.picture, &page.reader)) {
    Some((Some(_), _) | (_, Reader::Open { .. })) => {}
    Some((None, Reader::Hidden | Reader::Pending { .. })) | None => return Ok(Visible::NotYet),
  }
  let y = i32::try_from(layout.bar_height)?;
  let viewport = layout.viewport;
  let shown = surface.draw_layer(
    Part {
      at: Point { x: 0, y },
      size: viewport,
    },
    |pixels, device, _| {
      page.paint(
        pixels,
        Frame {
          points: viewport,
          device,
        },
        desktop_colors,
      )
    },
  )?;
  Ok(shown)
}

fn draw_combined(
  surface: &mut Surface,
  bar: &PaintedToolbar,
  page: &PageFrame,
  layout: PageLayout,
  desktop_colors: ReaderColors,
  toolbar: &mut Toolbar,
) -> Result<(), Box<dyn Error>> {
  surface.draw(|pixels, device, _| {
    let Ok(rest) = fill_toolbar(pixels, bar);
    let height = device.height.saturating_sub(bar.frame.device.height);
    match (rest, height) {
      (Some(page_pixels), 1..) => {
        let frame = Frame {
          points: layout.viewport,
          device: Size {
            width: device.width,
            height,
          },
        };
        let Ok(()) = page.paint(page_pixels, frame, desktop_colors);
      }
      (None, _) | (Some(_), 0) => {}
    }
    toolbar.paint_settings(
      pixels,
      Frame {
        points: layout.entire,
        device,
      },
    )
  })?;
  Ok(())
}

fn paint_page(pixels: &mut [u8], frame: Frame, picture: &Pixels) -> Result<(), Never> {
  let image = Picture {
    at: Point { x: 0, y: 0 },
    size: frame.points,
    pixels: picture,
  };

  match painting::over_picture(pixels, frame, image, &[]) {
    Ok(()) => {}
    Err(fault) => console_core_logging::error!("could not paint browser page: {fault}"),
  }

  Ok(())
}

enum CheckProgress {
  Pending,
  Complete,
}

enum WebKitCheck {
  Page,
  Click,
  Text,
  Scroll,
}

impl WebKitCheck {
  fn advance(&mut self, browser: &mut Browser) -> io::Result<CheckProgress> {
    match (&self, browser.active.title.as_str()) {
      (Self::Page, "Console browser test") => {
        browser.pointer(PointerEvent::Down {
          at: (40.0, f64::from(TOOLBAR_HEIGHT) + 40.0),
        })?;
        *self = Self::Click;
      }
      (Self::Click, "Console browser input focused") => {
        browser.request(Request::Text("z".to_owned()))?;
        *self = Self::Text;
      }
      (Self::Text, "Console browser input typed") => {
        browser.pointer(PointerEvent::Scrolled { by: 120.0 })?;
        *self = Self::Scroll;
      }
      (Self::Scroll, "Console browser scrolled down") => return Ok(CheckProgress::Complete),
      _ => {}
    }

    Ok(CheckProgress::Pending)
  }
}

struct SmokeCheck {
  engine: Renderer,
  deadline: Instant,
  completed: u32,
  webkit: WebKitCheck,
}

impl SmokeCheck {
  fn new(engine: Renderer) -> io::Result<Self> {
    let Ok(duration) = smoke_duration(engine);
    let deadline = Instant::now()
      .checked_add(duration)
      .ok_or_else(|| io::Error::other("browser check deadline is too far away"))?;

    Ok(Self {
      engine,
      deadline,
      completed: 0,
      webkit: WebKitCheck::Page,
    })
  }

  fn advance(&mut self, browser: &mut Browser) -> io::Result<CheckProgress> {
    match browser.active.worker {
      Some(_) => {}
      None => return Err(io::Error::other(browser.active.title.clone())),
    }

    let Ok(first_pixel) = browser.first_pixel();
    let rendered = matches!(
      (browser.active.loading, first_pixel),
      (PageLoad::Settled, Some([12, 34, 56, 255]))
    );

    match rendered.then_some(()) {
      Some(()) => {}
      None => return Ok(CheckProgress::Pending),
    }

    let progress = match (self.engine, browser.active.renderer) {
      (Renderer::WebKit, Renderer::WebKit) => self.webkit.advance(browser)?,
      _ => match browser.active.title.as_str() {
        "Console browser test" => CheckProgress::Complete,
        _ => CheckProgress::Pending,
      },
    };

    match progress {
      CheckProgress::Pending => return Ok(CheckProgress::Pending),
      CheckProgress::Complete => {}
    }

    println!(
      "{} rendered HTML, CSS and JavaScript in the Console window",
      browser.active.renderer
    );
    self.completed = self.completed.saturating_add(1);

    match (self.engine, self.completed) {
      (_, 1) => browser.switch(self.engine)?,
      (Renderer::WebKit, 2) => {
        println!("WebKit handled a click, text input and scrolling down");
        browser.switch(Renderer::Chromium)?;
      }
      (Renderer::Servo, 2) => browser.switch(Renderer::Chromium)?,
      _ => return Ok(CheckProgress::Complete),
    }

    Ok(CheckProgress::Pending)
  }

  fn remaining(&self, browser: &Browser) -> io::Result<Duration> {
    let remaining = self.deadline.saturating_duration_since(Instant::now());

    match remaining {
      Duration::ZERO => {
        let Ok(first) = browser.first_pixel();

        Err(io::Error::other(format!(
          "{} check timed out after {} completed loads. Current title {:?}, pending load {}, first pixel {:?}",
          browser.active.renderer,
          self.completed,
          browser.active.title,
          browser.active.loading == PageLoad::Loading,
          first
        )))
      }
      _ => Ok(remaining),
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  expect(
    explicit026_env_read_once,
    reason = "the browser fixture passes one CONSOLE_BROWSER_CHECK_TIMEOUT value to this window and never changes it during a run"
  )
)]
fn smoke_duration(engine: Renderer) -> Result<Duration, Never> {
  let default = match engine {
    Renderer::WebKit => 100,
    Renderer::Chromium | Renderer::Servo => 40,
  };
  let seconds = match std::env::var("CONSOLE_BROWSER_CHECK_TIMEOUT") {
    Ok(text) => match text.parse::<u64>() {
      Ok(seconds) => seconds,
      Err(fault) => {
        console_core_logging::debug!("invalid browser check timeout {text:?}: {fault}");

        default
      }
    },
    Err(std::env::VarError::NotPresent) => default,
    Err(fault) => {
      console_core_logging::debug!("cannot read the browser check timeout: {fault}");

      default
    }
  };

  Ok(Duration::from_secs(seconds))
}

impl Browser {
  fn first_pixel(&self) -> Result<Option<[u8; 4]>, Never> {
    Ok(
      self
        .active
        .page
        .picture
        .as_ref()
        .and_then(|picture| picture.bytes.first_chunk::<4>().copied()),
    )
  }

  fn read_input(&mut self) -> io::Result<Closed> {
    let Ok(presses) = self.surface.key_presses();
    let Ok(pointers) = self.surface.pointer_events();

    for press in presses {
      let closed = self.keyboard(press)?;

      match closed {
        Closed::Yes => return Ok(Closed::Yes),
        Closed::No => {}
      }
    }

    for pointer in pointers {
      let closed = self.pointer(pointer)?;

      match closed {
        Closed::Yes => return Ok(Closed::Yes),
        Closed::No => {}
      }
    }

    Ok(Closed::No)
  }
}

enum WaitPlan {
  Finished,
  Waiting(Option<Duration>),
}

struct WindowRun {
  browser: Browser,
  check: Option<SmokeCheck>,
  closed: Closed,
}

impl WindowRun {
  fn step(&mut self) -> Result<Closed, Box<dyn Error>> {
    let Ok(closed) = self.browser.surface.closed();

    match (closed, self.check.as_ref()) {
      (Closed::Yes, Some(_)) => {
        return Err(Box::from(io::Error::other(
          "the browser window closed before its check finished",
        )));
      }
      (Closed::Yes, None) => return Ok(Closed::Yes),
      (Closed::No, _) => {}
    }

    self.browser.receive();

    match self.check.as_ref() {
      Some(_) => {
        let Ok(_) = self.browser.surface.key_presses();
        let Ok(_) = self.browser.surface.pointer_events();
      }
      None => {
        let input = self.browser.read_input()?;

        match input {
          Closed::Yes => return Ok(Closed::Yes),
          Closed::No => {}
        }
      }
    }

    self.browser.refresh()?;
    let plan = self.wait_plan()?;
    let remaining = match plan {
      WaitPlan::Finished => return Ok(Closed::Yes),
      WaitPlan::Waiting(remaining) => remaining,
    };

    let channels: Vec<_> = std::iter::once(&self.browser.active)
      .chain(&self.browser.background)
      .filter_map(|tab| tab.worker.as_ref().map(|worker| worker.channel.as_fd()))
      .collect();
    self.browser.surface.wait(&channels, remaining)?;

    Ok(Closed::No)
  }

  fn wait_plan(&mut self) -> io::Result<WaitPlan> {
    match self.check.as_mut() {
      Some(check) => {
        let progress = check.advance(&mut self.browser)?;

        match progress {
          CheckProgress::Complete => Ok(WaitPlan::Finished),
          CheckProgress::Pending => {
            let duration = check.remaining(&self.browser)?;

            Ok(WaitPlan::Waiting(Some(duration)))
          }
        }
      }
      None => Ok(WaitPlan::Waiting(None)),
    }
  }
}

impl Iterator for WindowRun {
  type Item = Result<(), Box<dyn Error>>;

  fn next(&mut self) -> Option<Self::Item> {
    match self.closed {
      Closed::Yes => return None,
      Closed::No => {}
    }

    match self.step() {
      Ok(Closed::Yes) => {
        self.closed = Closed::Yes;

        None
      }
      Ok(Closed::No) => Some(Ok(())),
      Err(fault) => {
        self.closed = Closed::Yes;

        Some(Err(fault))
      }
    }
  }
}

fn show(
  mut browser: Browser,
  test: Option<Renderer>,
  fallback: Option<Renderer>,
) -> Result<(), Box<dyn Error>> {
  match browser.switch(browser.active.renderer) {
    Ok(()) => {}
    Err(fault) => match fallback {
      Some(renderer) => {
        console_core_logging::warn!("could not start Servo, opening {renderer}: {fault}");
        browser.switch(renderer)?;
        browser.default_engine = renderer;
      }
      None => return Err(fault.into()),
    },
  }
  let check = match test {
    Some(engine) => {
      let started = SmokeCheck::new(engine)?;

      Some(started)
    }
    None => None,
  };

  for step in (WindowRun {
    browser,
    check,
    closed: Closed::No,
  }) {
    step?;
  }

  Ok(())
}

pub fn run(arguments: Vec<OsString>) -> Result<ExitCode, Never> {
  let args = match launch_arguments(arguments) {
    Ok(args) => args,
    Err(argument) => {
      console_core_logging::error!(
        "console-browser-window: expected a UTF-8 address or local file path, received {argument:?}"
      );

      return Ok(ExitCode::FAILURE);
    }
  };
  let line = match console_core_arguments::read(&COMMAND, &args) {
    Ok(line) => line,
    Err(refusal) => {
      let Ok(code) = refusal.print();
      return Ok(ExitCode::from(code));
    }
  };
  let Ok(operands) = line.operands();

  let typed = match operands.first() {
    Some(typed) => typed.as_str(),
    None => BLANK,
  };
  let Ok(address) = resolve(typed);
  let address_3 = match address {
    Some(address_2) => address_2,
    None => {
      eprintln!("console-browser-window: enter a web address, local file path, or search term");

      return Ok(ExitCode::FAILURE);
    }
  };

  let Ok(engine) = line.value(ENGINE);
  let Ok(servo_worker) = line.value(SERVO_WORKER);
  let Ok(webkit_worker) = line.value(WEBKIT_WORKER);
  let Ok(smoke) = line.presence(SMOKE);
  let Ok(servo_test) = line.presence(SERVO_TEST);
  let Ok(webkit_test) = line.presence(WEBKIT_TEST);
  let test = match (webkit_test, servo_test, smoke) {
    (Presence::Present, _, _) => Some(Renderer::WebKit),
    (_, Presence::Present, _) => Some(Renderer::Servo),
    (_, _, Presence::Present) => Some(Renderer::Chromium),
    _ => None,
  };
  let boot = || -> Result<(), Box<dyn Error>> {
    let executable = std::env::current_exe()?;
    let locate = |requested: Option<&str>, name| {
      let Ok(nearby) = nearby_worker(&executable, name);
      requested.map(PathBuf::from).or(nearby)
    };
    let servo = servo_worker.map(PathBuf::from).or_else(|| {
      let staged = std::env::var_os("CONSOLE_BROWSER_SERVO_WORKER").and_then(|path| {
        let Ok(available) = available_worker(PathBuf::from(path));
        available
      });
      staged.or_else(|| locate(None, "console-browser-renderer-servo"))
    });
    let webkit = locate(webkit_worker, "console-browser-renderer-webkit");
    let programs = RendererPrograms {
      executable,
      servo,
      webkit,
    };
    let default_engine = match engine {
      Some("servo") => Renderer::Servo,
      Some("chromium") => Renderer::Chromium,
      Some("webkit") => Renderer::WebKit,
      Some(_) => return Err(invalid("use --engine servo, chromium, or webkit").into()),
      None => programs.preferred(),
    };
    let _executable = programs.for_renderer(default_engine)?;
    let Ok(available) = programs.available();

    match test {
      Some(engine) => match available.contains(&engine).then_some(()) {
        Some(()) => {}
        None => {
          return Err(Box::from(io::Error::other(format!(
            "{engine} worker is not configured"
          ))));
        }
      },
      None => {}
    }

    let mut toolbar = Toolbar::with_renderers(available)?;
    let Ok(_) = toolbar.address(&address_3);
    let mut surface = Surface::connect()?;
    surface.show_window_sized(
      &Window {
        app_id: APP_ID.to_owned(),
        title: TITLE.to_owned(),
      },
      WINDOW,
    )?;
    let browser = Browser {
      surface,
      toolbar,
      default_engine,
      active: Tab::new(
        1,
        match test {
          Some(_) => Renderer::Chromium,
          None => default_engine,
        },
        address_3,
      ),
      programs,
      background: Vec::new(),
      next_tab: 2,
      desktop_font: crate::reader::desktop_font(),
      viewport: Size {
        width: WINDOW.width,
        height: WINDOW.height.saturating_sub(TOOLBAR_HEIGHT),
      },
      clipboard: String::new(),
      bar: ToolbarFrame {
        visible: None,
        redraw: Redraw::Requested,
      },
    };
    let fallback = match (engine, test, default_engine) {
      (None, None, Renderer::Servo) => Some(Renderer::Chromium),
      _ => None,
    };
    show(browser, test, fallback)
  };

  Ok(match boot() {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("console-browser-window: {error}");
      ExitCode::FAILURE
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::os::unix::ffi::OsStringExt;

  #[test]
  fn servo_is_preferred_only_when_configured() {
    let mut programs = RendererPrograms {
      executable: PathBuf::from("console-browser-window"),
      servo: None,
      webkit: Some(PathBuf::from("webkit-worker")),
    };
    assert_eq!(programs.preferred(), Renderer::Chromium);
    programs.servo = Some(PathBuf::from("servo-worker"));
    assert_eq!(programs.preferred(), Renderer::Servo);
  }

  #[test]
  fn page_keys_carry_logical_input() {
    assert_eq!(page_key(Keysym::KP_Enter), page_key(Keysym::Return));
    assert_eq!(page_key(Keysym::XF86Launch9), None);
    assert_eq!(page_key(Keysym::F9), None);
  }

  #[test]
  fn desktop_links_keep_their_query_and_fragment() {
    for url in [
      "http://127.0.0.1:8080/start?next=%2Fa%20b&keep=yes#top",
      "https://example.org/path?key=%23#heading",
    ] {
      let Ok(resolved) = resolve(url);
      assert_eq!(resolved.as_deref(), Some(url));
    }
  }

  #[test]
  fn desktop_documents_accept_file_uris_and_native_paths() -> Result<(), Box<dyn Error>> {
    let file = OsString::from_vec(b"/documents/report #\xff.html".to_vec());
    let args = launch_arguments(vec![OsString::from("--"), file])
      .map_err(|_| invalid("could not read desktop arguments"))?;
    let line = console_core_arguments::read(&COMMAND, &args)?;
    let Ok(operands) = line.operands();
    let address = operands.first().and_then(|operand| {
      let Ok(address) = resolve(operand);

      address
    });
    assert_eq!(
      address.as_deref(),
      Some("file:///documents/report%20%23%FF.html")
    );
    let Ok(absolute) = resolve("/documents/report #1.html");
    assert_eq!(
      absolute.as_deref(),
      Some("file:///documents/report%20%231.html")
    );
    let url = "file://localhost/documents/report%20%231.html#heading";
    let Ok(resolved) = resolve(url);
    assert_eq!(resolved.as_deref(), Some(url));
    assert_eq!(resolve("file://remote/documents/report.html"), Ok(None));
    Ok(())
  }
}
