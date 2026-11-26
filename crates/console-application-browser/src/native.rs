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
use console_core_state_machine::Machine;
use console_core_web_engine::{DownloadState, Event, FrameLayout, Request, decoded_event};
use console_default_applications::engines;
use console_input_key_symbols::Keysym;
use console_panel::ui::painting::{self, Frame};
use console_panel::ui::shapes::{Picture, Pixels};
use console_panel::ui::surface::retained::{Area, Patch};
use console_panel::ui::surface::{
  Closed, KeyPress, LayerSupport, Part, PointerEvent, Surface, Visible, Window,
};

use crate::download_catalog::{self, FileAction, Scanner, WatchChange};
use crate::download_refresh::{self, DownloadRefresh};
use crate::download_view::{self, FileProgress};
use crate::downloads::{self, open_in_desktop};
use crate::engine_start::{self, EngineStart};
use crate::file_picker::{self, Picker, VerticalOffset};
use crate::presentation::{self, Backdrop, Presentation, Redraw};
use crate::reader::{Article, Mode, Reader, ReaderText, ScrollPixels, TextSize};
use crate::renderer::{PrivateChromium, Renderer, Worker};
use crate::settings;
use crate::toolbar::{
  self, Action, ButtonAvailability, Direction, DownloadsPanel, EntryMode, MAX_TABS, PageLoad,
  Status, TabLabel, TextPrompt, Toolbar, ToolbarChange,
};

const WINDOW: Size<u32> = Size {
  width: 1100,
  height: 750,
};
const BLANK: &str = "about:blank";
const APP_ID: &str = "console-browser-window";
const TITLE: &str = "Console Browser";
const VIEWPORT_BUCKET: Size<u32> = Size {
  width: 200,
  height: 100,
};
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
const CHROMIUM_WORKER: Flag = Flag {
  spelling: "--chromium-worker",
  takes: Takes::Value("PATH"),
  about: "enable the Chromium renderer with the given worker",
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
const RESUME: Flag = Flag {
  spelling: "--resume-window",
  takes: Takes::Value("STATE"),
  about: "reopen tabs kept by the desktop session",
};
const COMMAND: Command = Command {
  name: APP_ID,
  about: "open a page in the Console browser window",
  flags: &[
    CHROMIUM_WORKER,
    SERVO_WORKER,
    WEBKIT_WORKER,
    ENGINE,
    SMOKE,
    SERVO_TEST,
    WEBKIT_TEST,
    RESUME,
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
  prompt: Option<Prompt>,
  file_picker: Option<Picker>,
  download_notice: Option<DownloadNotice>,
  download_view: download_view::View,
  download_scanner: Option<Scanner>,
  download_refresh: download_refresh::State,
  bar: ToolbarFrame,
  backdrop: Backdrop,
}

enum Prompt {
  Find,
  Upload { tab: u32, chooser: u64 },
}

struct DownloadNotice {
  tab: u32,
  state: DownloadState,
  name: String,
}

impl DownloadNotice {
  fn interrupt(&mut self, tab: u32) -> Result<(), Never> {
    match (self.tab == tab && self.state == DownloadState::Started).then_some(()) {
      Some(()) => self.state = DownloadState::Failed,
      None => {}
    }

    Ok(())
  }
}

fn interrupt_download(notice: &mut Option<DownloadNotice>, tab: u32) -> Result<(), Never> {
  match notice {
    Some(current) => current.interrupt(tab),
    None => Ok(()),
  }
}

#[derive(Default)]
enum ClipboardSelection {
  #[default]
  Idle,
  Waiting,
  Ready(String),
}

enum FindStatus {
  Unreported,
  Found,
  NoMatches,
}

enum TabUpdate {
  Unchanged,
  Changed,
}

enum Shortcut {
  Handled,
  Unclaimed,
}

enum PreviousTab {
  Keep,
  Close,
}

struct Tab {
  id: u32,
  renderer: Renderer,
  opener: Option<String>,
  upload: Option<u64>,
  selection: ClipboardSelection,
  find: String,
  find_status: FindStatus,
  zoom: u16,
  worker: Option<Worker>,
  address: String,
  title: String,
  page: PageFrame,
  loading: PageLoad,
  reader_id: u64,
  reader_text_size: u32,
}

struct RendererPrograms {
  chromium: Option<PathBuf>,
  servo: Option<PathBuf>,
  webkit: Option<PathBuf>,
  private: PrivateChromium,
}

impl RendererPrograms {
  fn start(&mut self, renderer: Renderer) -> io::Result<Worker> {
    match renderer {
      Renderer::Chromium => {
        let executable = self
          .chromium
          .as_deref()
          .ok_or_else(|| io::Error::other("Chromium worker is not configured"))?;
        self.private.tab(executable)
      }
      Renderer::Servo | Renderer::WebKit => Worker::start({
        let checked_value_2 = self.for_renderer(renderer)?;
        checked_value_2
      }),
    }
  }

  fn preferred(&self) -> io::Result<Renderer> {
    for renderer in engine_start::PREFERENCE {
      match self.path(renderer) {
        Ok(Some(_path)) => return Ok(renderer),
        Ok(None) => {}
      }
    }

    Err(io::Error::other("no browser engine worker is configured"))
  }

  fn available(&self) -> Result<&'static [Renderer], Never> {
    use Renderer::{Chromium, Servo, WebKit};

    Ok(
      match (
        self.chromium.is_some(),
        self.webkit.is_some(),
        self.servo.is_some(),
      ) {
        (false, false, false) => &[],
        (true, false, false) => &[Chromium],
        (false, true, false) => &[WebKit],
        (false, false, true) => &[Servo],
        (true, true, false) => &[Chromium, WebKit],
        (true, false, true) => &[Chromium, Servo],
        (false, true, true) => &[WebKit, Servo],
        (true, true, true) => &[Chromium, WebKit, Servo],
      },
    )
  }

  fn for_renderer(&self, renderer: Renderer) -> io::Result<&Path> {
    let Ok(path) = self.path(renderer);
    path.ok_or_else(|| io::Error::other(format!("{renderer} worker is not configured")))
  }

  fn path(&self, renderer: Renderer) -> Result<Option<&Path>, Never> {
    Ok(match renderer {
      Renderer::Chromium => self.chromium.as_deref(),
      Renderer::Servo => self.servo.as_deref(),
      Renderer::WebKit => self.webkit.as_deref(),
    })
  }
}

struct PageFrame {
  picture: Option<Pixels>,
  receiving: Vec<u8>,
  redraw: Redraw,
  reader: Reader,
}

struct ToolbarFrame {
  visible: Option<PaintedToolbar>,
  spare: Vec<u8>,
  redraw: Redraw,
}

struct PaintedToolbar {
  frame: Frame,
  pixels: Vec<u8>,
  changed: Option<Patch>,
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

#[must_use]
fn page_key(
  key: Keysym,
) -> core::result::Result<Option<&'static str>, console_core_never::Never> {
  Ok(Some(match key {
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
    _ => return Ok(None),
  }))
}

impl Tab {
  #[must_use]
  fn new(
    id: u32,
    renderer: Renderer,
    address: String,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      id,
      renderer,
      opener: None,
      upload: None,
      selection: ClipboardSelection::Idle,
      find: String::new(),
      find_status: FindStatus::Unreported,
      zoom: 100,
      worker: None,
      address,
      title: TITLE.to_owned(),
      page: PageFrame {
        picture: None,
        receiving: Vec::new(),
        redraw: Redraw::Requested,
        reader: Reader::Hidden,
      },
      loading: PageLoad::Ready,
      reader_id: 0,
      reader_text_size: crate::reader::DEFAULT_TEXT_SIZE,
    })
  }

  fn switch(
    &mut self,
    renderer: Renderer,
    programs: &mut RendererPrograms,
    viewport: Size<u32>,
  ) -> io::Result<()> {
    let worker = programs.start(renderer)?;
    let Ok(content) = content_viewport(viewport);
    worker.request(&Request::Resize(content))?;
    let address = match &self.opener {
      Some(opener) => opener.clone(),
      None => self.address.clone(),
    };
    worker.request(&Request::Open(address.clone()))?;

    match self.zoom {
      100 => {}
      _ => {
        worker.request(&Request::Zoom(self.zoom))?;
      }
    }

    self.worker = Some(worker);
    self.address = address;
    self.renderer = renderer;
    self.page.picture = None;
    self.opener = None;
    self.upload = None;
    self.selection = ClipboardSelection::Idle;
    self.find_status = FindStatus::Unreported;
    let Ok(()) = self.hide_reader();
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

  #[must_use]
  fn label(&self) -> core::result::Result<TabLabel, console_core_never::Never> {
    let title = match (self.title.is_empty() || self.title == TITLE).then_some(()) {
      Some(()) => match self.address.as_str() {
        BLANK => "New tab",
        address => address,
      },
      None => &self.title,
    };
    let title_2: String = title.chars().take(160).collect();
    let title_3 = match self.find_status {
      FindStatus::NoMatches => format!("No matches · {title}", title = title_2),
      FindStatus::Unreported | FindStatus::Found => title_2,
    };
    let title_4 = match { ::core::primitive::bool::then_some(self.opener.is_some(), ()) } {
      ::core::option::Option::Some(()) => {
        format!("Popup · {title}", title = title_3)
      }
      ::core::option::Option::None => title_3,
    };

    Ok(TabLabel {
      id: self.id,
      title: title_4,
    })
  }

  fn hide_reader(&mut self) -> core::result::Result<(), console_core_never::Never> {
    {
      self.page.reader = Reader::Hidden;
      self.page.redraw = Redraw::Requested;
    };

    Ok(())
  }

  fn read(&mut self, mode: Mode) -> io::Result<()> {
    let Ok(own_mode) = self.page.reader.mode();

    match (own_mode == Some(mode)).then_some(()) {
      Some(()) => {
        let Ok(()) = self.hide_reader();
        return Ok(());
      }
      None => {}
    }

    match &mut self.page.reader {
      Reader::Open { mode: current, .. } | Reader::Pending { mode: current, .. } => {
        *current = mode;
      }
      Reader::Hidden => {
        let checked_value = self
          .reader_id
          .checked_add(1)
          .ok_or_else(|| io::Error::other("reader request numbers exhausted"))?;
        self.reader_id = checked_value;

        match self.worker {
          Some(_) => {
            self.request(Request::Read(self.reader_id))?;
            self.page.reader = Reader::Pending {
              id: self.reader_id,
              mode,
            };
          }
          None => {
            let Ok(show_article) = self.show_article(mode, String::new());

            show_article
          }
        }
      }
    }

    self.page.redraw = Redraw::Requested;
    Ok(())
  }

  fn show_article(
    &mut self,
    mode: Mode,
    text: String,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let text_2 = match text.trim() {
        "" => "No readable text on this page.".to_owned(),
        _ => text,
      };
      let Ok(article) = Article::new(ReaderText {
        title: self.title.clone(),
        body: text_2,
      });
      self.page.reader = Reader::Open { mode, article };
      self.page.redraw = Redraw::Requested;
    };

    Ok(())
  }

  fn stopped(&mut self, fault: io::Error) -> core::result::Result<(), console_core_never::Never> {
    {
      console_core_logging::debug!("{} tab {} stopped: {fault}", self.renderer, self.id);
      self.worker = None;
      self.loading = PageLoad::Ready;

      match self.page.reader {
        Reader::Pending { mode, .. } => {
          let Ok(show_article) = self.show_article(mode, String::new());

          show_article
        }
        Reader::Hidden | Reader::Open { .. } => {}
      }

      self.title = format!("{} stopped. Reload to reopen the page.", self.renderer);
    };

    Ok(())
  }

  fn receive(&mut self, notice: &mut Option<DownloadNotice>) -> io::Result<TabUpdate> {
    let mut changed = TabUpdate::Unchanged;

    for _ in 0..32 {
      let worker = match &self.worker {
        Some(worker) => worker,
        None => break,
      };
      let packet = match worker.receive() {
        Ok(packet) => packet,
        Err(error) => {
          match ::core::primitive::bool::then_some(error.kind() == io::ErrorKind::WouldBlock, ())
          {
            ::core::option::Option::Some(()) => break,
            ::core::option::Option::None => return Err(error),
          }
        }
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
            Some(()) => {
              let Ok(cancel_pending_reader) = self.cancel_pending_reader();

              cancel_pending_reader
            }
            None => {}
          }

          let Ok(()) = self.clear_blank_page(&address);
          self.address = address;
          changed = TabUpdate::Changed;
        }
        (Some(Event::Title(title)), None) => {
          self.title = title;
          changed = TabUpdate::Changed;
        }
        (Some(Event::Started), None) => {
          self.upload = None;

          match self.selection {
            ClipboardSelection::Waiting => self.selection = ClipboardSelection::Idle,
            ClipboardSelection::Ready(_) | ClipboardSelection::Idle => {}
          }

          self.find_status = FindStatus::Unreported;
          let Ok(()) = self.cancel_pending_reader();
          self.loading = PageLoad::Loading;
          changed = TabUpdate::Changed;
        }
        (Some(Event::Finished), None) => {
          self.loading = PageLoad::Ready;
          changed = TabUpdate::Changed;
        }
        (Some(Event::Download(state, name)), None) => {
          *notice = Some(DownloadNotice {
            tab: self.id,
            state,
            name,
          });
          changed = TabUpdate::Changed;
        }
        (Some(Event::FileChooser(chooser)), None) => {
          self.upload = Some(chooser);
          changed = TabUpdate::Changed;
        }
        (Some(Event::Popup(open)), None) => {
          match ::core::primitive::bool::then_some(open, ()) {
            ::core::option::Option::Some(()) => {
              self.opener.get_or_insert_with(|| self.address.clone());
            }
            ::core::option::Option::None => {
              self.opener = None;
            }
          }

          let Ok(()) = self.hide_reader();
          self.page.picture = None;
          changed = TabUpdate::Changed;
        }
        (Some(Event::Selection(text)), None) => {
          match (&self.selection, text.is_empty().then_some(())) {
            (ClipboardSelection::Waiting, Some(())) => self.selection = ClipboardSelection::Idle,
            (ClipboardSelection::Waiting, None) => {
              self.selection = ClipboardSelection::Ready(text)
            }
            (ClipboardSelection::Idle | ClipboardSelection::Ready(_), _) => {}
          }
        }
        (Some(Event::FindResult(found)), None) => {
          self.find_status = match found.then_some(()) {
            Some(()) => FindStatus::Found,
            None => FindStatus::NoMatches,
          };
          changed = TabUpdate::Changed;
        }
        (Some(Event::Reader(id, text)), None) => match self.page.reader {
          Reader::Pending { id: pending, mode } => match (pending == id).then_some(()) {
            Some(()) => {
              let Ok(()) = self.show_article(mode, text);
              changed = TabUpdate::Changed;
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

  fn cancel_pending_reader(&mut self) -> core::result::Result<(), console_core_never::Never> {
    {
      match self.page.reader {
        Reader::Pending { .. } => {
          let Ok(hide_reader) = self.hide_reader();

          hide_reader
        }
        Reader::Hidden | Reader::Open { .. } => {}
      }
    };

    Ok(())
  }

  fn clear_blank_page(
    &mut self,
    address: &str,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      match address {
        BLANK => {
          self.page.picture = None;
          self.page.redraw = Redraw::Requested;
        }
        _ => {}
      }
    };

    Ok(())
  }

  fn selected_text(&mut self) -> Result<Option<String>, Never> {
    match &mut self.selection {
      ClipboardSelection::Ready(text) => {
        let selected = std::mem::take(text);
        self.selection = ClipboardSelection::Idle;
        Ok(Some(selected))
      }
      ClipboardSelection::Waiting | ClipboardSelection::Idle => Ok(None),
    }
  }

  fn update(&mut self, notice: &mut Option<DownloadNotice>) -> Result<TabUpdate, Never> {
    match self.receive(notice) {
      Ok(changed) => Ok(changed),
      Err(fault) => {
        let Ok(()) = self.stopped(fault);
        let Ok(()) = interrupt_download(notice, self.id);
        Ok(TabUpdate::Changed)
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
    let workers = std::iter::once(&self.active)
      .chain(&self.background)
      .filter_map(|tab| tab.worker.as_ref());
    let Ok(()) = self.programs.private.retain(workers);
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
      reader: ({
        let Ok(explicit_value) = self.active.page.reader.mode();
        explicit_value
      }),
    });
    let Ok(address) = self.toolbar.address(&self.active.address);
    let Ok(downloads) = self.toolbar.download_notice(
      self
        .download_notice
        .as_ref()
        .map(|notice| (notice.state, notice.name.as_str())),
    );
    let mut labels: Vec<_> = self
      .background
      .iter()
      .map(|explicit_argument_0| {
        let Ok(label) = Tab::label(explicit_argument_0);

        label
      })
      .collect();
    let Ok(label) = self.active.label();
    labels.push(label);
    labels.sort_unstable_by_key(|tab| tab.id);
    let Ok(tabs) = self.toolbar.tabs(labels, self.active.id);

    match (status, address, tabs, downloads) {
      (
        ToolbarChange::Unchanged,
        ToolbarChange::Unchanged,
        ToolbarChange::Unchanged,
        ToolbarChange::Unchanged,
      ) => {}
      _ => {
        let Ok(()) = self.dirty_toolbar();
      }
    }

    self.surface.set_window_title(&self.active.title)
  }

  fn switch(&mut self, renderer: Renderer) -> io::Result<()> {
    self
      .active
      .switch(renderer, &mut self.programs, self.viewport)?;
    let Ok(()) = interrupt_download(&mut self.download_notice, self.active.id);
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
      Err(fault) => match matches!(
        fault.kind(),
        io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset | io::ErrorKind::UnexpectedEof
      )
      .then_some(())
      {
        Some(()) => {
          let Ok(()) = self.active.stopped(fault);
          let Ok(()) = interrupt_download(&mut self.download_notice, self.active.id);
          let Ok(()) = self.controls();
          Ok(())
        }
        None => Err(fault),
      },
    }
  }

  fn receive(&mut self) -> core::result::Result<(), console_core_never::Never> {
    {
      let previous = self.active.address.clone();
      let Ok(mut changed) = self.active.update(&mut self.download_notice);

      for tab in &mut self.background {
        match tab.update(&mut self.download_notice) {
          Ok(TabUpdate::Changed) => changed = TabUpdate::Changed,
          Ok(TabUpdate::Unchanged) => {}
        }

        let Ok(_background_selection) = tab.selected_text();
      }

      let Ok(selected) = self.active.selected_text();

      match selected {
        Some(text) => {
          let Ok(()) = self.copy(text);
        }
        ::core::option::Option::None => {}
      }

      match { ::core::primitive::bool::then_some(previous != self.active.address, ()) } {
        ::core::option::Option::Some(()) => {
          self.file_picker = None;

          match self.prompt {
            Some(Prompt::Upload { .. }) => {
              self.prompt = None;
              let Ok(()) = self.toolbar.blur(&self.active.address);
              let Ok(()) = self.dirty_toolbar();
            }
            Some(Prompt::Find) | None => {}
          }
        }
        ::core::option::Option::None => {}
      }

      match self.active.upload.take() {
        Some(chooser) => {
          let Ok(()) = self.close_downloads();
          let Ok(()) = self.toolbar.dismiss_settings();
          let Ok(()) = self.toolbar.blur(&self.active.address);
          let Ok(()) = self.dirty_toolbar();

          match Picker::new(self.active.id, chooser) {
            Ok(picker) => self.file_picker = Some(picker),
            Err(fault) => {
              console_core_logging::debug!("opening file chooser: {fault}");
              let Ok(()) = self.upload_prompt(self.active.id, chooser);
            }
          }

          self.active.page.redraw = Redraw::Requested;
        }
        ::core::option::Option::None => {}
      }

      match changed {
        TabUpdate::Changed => {
          let Ok(()) = self.controls();
        }
        TabUpdate::Unchanged => {}
      }
    };

    Ok(())
  }

  fn new_tab(&mut self) -> io::Result<()> {
    let Ok(background) = console_core_number_conversion::fixed_index(self.background.len());

    match background.saturating_add(1).cmp(&MAX_TABS) {
      std::cmp::Ordering::Less => {}
      std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => return Ok(()),
    }

    let next = self
      .next_tab
      .checked_add(1)
      .ok_or_else(|| io::Error::other("tab numbers exhausted"))?;
    let Ok(mut tab) = Tab::new(self.next_tab, self.default_engine, BLANK.to_owned());
    tab.switch(self.default_engine, &mut self.programs, self.viewport)?;
    self.next_tab = next;
    let previous = std::mem::replace(&mut self.active, tab);
    let Ok(()) = self.background_tab(previous);
    let Ok(()) = self.tab_changed();
    let Ok(()) = self.toolbar.edit();
    Ok(())
  }

  fn background_tab(
    &mut self,
    mut tab: Tab,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      match tab.request(Request::Visible(false)) {
        Err(fault) => {
          let Ok(()) = tab.stopped(fault);
          let Ok(()) = interrupt_download(&mut self.download_notice, tab.id);
        }
        ::core::result::Result::Ok(_) => {}
      }

      self.background.push(tab);
    };

    Ok(())
  }

  fn select_tab(
    &mut self,
    id: u32,
    disposition: PreviousTab,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let ::core::result::Result::Ok(index) = ::console_core_number_conversion::fixed_index(
        match self.background.iter().position(|tab| tab.id == id) {
          Some(index) => index,
          None => return Ok(()),
        },
      );
      let mut tab = match self.background.pop() {
        Some(last) => {
          match self
            .background
            .get_mut(match ::console_core_number_conversion::index(index) {
              ::core::result::Result::Ok(native_position) => native_position,
            }) {
            Some(slot) => std::mem::replace(slot, last),
            None => last,
          }
        }
        None => return Ok(()),
      };

      let ready = match { ::core::primitive::bool::then_some(tab.worker.is_some(), ()) } {
        ::core::option::Option::Some(()) => {
          let Ok(content) = content_viewport(self.viewport);
          tab
            .request(Request::Resize(content))
            .and_then(|()| tab.request(Request::Visible(true)))
        }
        ::core::option::Option::None => {
          tab.switch(tab.renderer, &mut self.programs, self.viewport)
        }
      };

      match ready {
        Ok(()) => {}
        Err(fault) => {
          let Ok(()) = tab.stopped(fault);
          let Ok(()) = interrupt_download(&mut self.download_notice, tab.id);
        }
      }

      let previous = std::mem::replace(&mut self.active, tab);

      match disposition {
        PreviousTab::Keep => {
          let Ok(()) = self.background_tab(previous);
        }
        PreviousTab::Close => {}
      }

      let Ok(()) = self.tab_changed();
    };

    Ok(())
  }

  #[must_use]
  fn adjacent_tab(
    &self,
    direction: Direction,
  ) -> core::result::Result<Option<u32>, console_core_never::Never> {
    let id = self.active.id;
    let ids = self.background.iter().map(|tab| tab.id);

    Ok(match direction {
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
    })
  }

  fn tab_changed(&mut self) -> Result<(), Never> {
    self.prompt = None;
    self.file_picker = None;
    self.active.page.redraw = Redraw::Requested;
    let Ok(()) = self.toolbar.blur(&self.active.address);
    self.controls()
  }

  fn action(&mut self, action: Action) -> io::Result<Closed> {
    match {
      ::core::primitive::bool::then_some(
        matches!(
          self.download_view.visibility(),
          Ok(download_view::Visibility::Visible)
        ),
        (),
      )
    } {
      ::core::option::Option::Some(()) => match action {
        Action::Back | Action::Downloads => {
          return self.download_action(download_view::Action::Dismiss);
        }
        Action::Reload => return self.download_action(download_view::Action::Refresh),
        Action::CloseTab
        | Action::NewTab
        | Action::SelectTab(_)
        | Action::CycleTab(_)
        | Action::Open(_)
        | Action::SettingsChanged => {
          let Ok(()) = self.close_downloads();
        }
        Action::Pass
        | Action::Handled
        | Action::Compose
        | Action::Forward
        | Action::Stop
        | Action::Close
        | Action::Reader(_)
        | Action::PageView
        | Action::ReaderText(_)
        | Action::Switch(_)
        | Action::FocusPage
        | Action::Copy(_)
        | Action::Paste => {}
      },
      ::core::option::Option::None => {}
    }

    match action {
      Action::Pass => return Ok(Closed::No),
      Action::Close => return Ok(Closed::Yes),
      Action::CloseTab => {
        match { ::core::primitive::bool::then_some(self.active.opener.is_some(), ()) } {
          ::core::option::Option::Some(()) => {
            self.request(Request::ClosePopup)?;
            return Ok(Closed::No);
          }
          ::core::option::Option::None => {}
        }

        let Ok(()) = interrupt_download(&mut self.download_notice, self.active.id);

        let Ok(adjacent_tab) = self.adjacent_tab(Direction::Next);

        match adjacent_tab {
          Some(id) => {
            let Ok(select_tab) = self.select_tab(id, PreviousTab::Close);

            select_tab
          }
          None => return Ok(Closed::Yes),
        }
      }
      Action::NewTab => match self.new_tab() {
        Ok(()) => {}
        Err(fault) => {
          let Ok(()) = self.report_action(fault);
        }
      },
      Action::Downloads => {
        let Ok(()) = self.show_downloads();
      }
      Action::SelectTab(id) => {
        let Ok(select_tab) = self.select_tab(id, PreviousTab::Keep);

        select_tab
      }
      Action::CycleTab(direction) => {
        let Ok(adjacent_tab) = self.adjacent_tab(direction);

        match adjacent_tab {
          Some(id) => {
            let Ok(select_tab) = self.select_tab(id, PreviousTab::Keep);

            select_tab
          }
          None => {}
        }
      }
      Action::Reader(mode) => {
        match mode {
          Mode::Desktop => {
            let Ok(font) = crate::reader::desktop_font();
            self.desktop_font = font;
            let Ok(()) = self.toolbar.refresh_palette();
          }
          Mode::Dark => {}
        }

        let result = self.active.read(mode);
        self.handle_worker_result(result)?;
        let Ok(()) = self.toolbar.blur(&self.active.address);
      }
      Action::PageView => {
        let Ok(hide_reader) = self.active.hide_reader();

        hide_reader
      }
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
            Err(fault) => {
              let Ok(()) = self.report_action(fault);
            }
          },
        }
      }
      Action::Open(text) => self.submit(&text)?,
      Action::Copy(text) => {
        let Ok(copy) = self.copy(text);

        copy
      }
      Action::Paste => {
        let text = match crate::clipboard::paste() {
          Ok(Some(text)) => text,
          Ok(None) => String::new(),
          Err(fault) => {
            console_core_logging::debug!("reading the system clipboard: {}", fault.kind());
            self.clipboard.clone()
          }
        };
        let Ok(()) = self.toolbar.text(&text);
      }
    }

    let Ok(()) = self.controls();
    let Ok(()) = self.dirty_toolbar();
    Ok(Closed::No)
  }

  fn navigate(&mut self, action: Action) -> io::Result<()> {
    let Ok(()) = self.active.hide_reader();

    match (action, &self.active.worker) {
      (Action::Back, _) => self.request(Request::Back),
      (Action::Forward, _) => self.request(Request::Forward),
      (Action::Reload, Some(_)) => self.request(Request::Reload),
      (Action::Reload, None) => {
        match self.switch(self.active.renderer) {
          Ok(()) => {}
          Err(fault) => {
            let Ok(()) = self.report_action(fault);
          }
        }

        Ok(())
      }
      _ => Ok(()),
    }
  }

  fn report_action(&mut self, fault: io::Error) -> Result<(), Never> {
    console_core_logging::warn!("browser action failed: {fault}");
    self.active.title = fault.to_string();

    Ok(())
  }

  fn show_downloads(&mut self) -> Result<(), Never> {
    let Ok(()) = self.toolbar.dismiss_settings();
    let Ok(()) = self.toolbar.blur(&self.active.address);
    let Ok(()) = self.toolbar.downloads_open(DownloadsPanel::Open);
    let Ok(()) = self.download_view.open();
    self.refresh_downloads()
  }

  fn close_downloads(&mut self) -> Result<(), Never> {
    let Ok(()) = self.toolbar.downloads_open(DownloadsPanel::Closed);
    let Ok(()) = self.dirty_toolbar();
    let Ok(()) = self.download_view.close();
    self.active.page.redraw = Redraw::Requested;

    Ok(())
  }

  fn refresh_downloads(&mut self) -> Result<(), Never> {
    self.advance_download_refresh(download_refresh::Request::Refresh)
  }

  fn advance_download_refresh(
    &mut self,
    request: download_refresh::Request,
  ) -> Result<(), Never> {
    let Ok(turn) = DownloadRefresh::transition(self.download_refresh, request);
    self.download_refresh = turn.state;

    for effect in turn.effects {
      match effect {
        download_refresh::Effect::Scan => {
          let Ok(()) = self.start_download_scan();
        }
        download_refresh::Effect::Present(Ok(catalog)) => {
          let Ok(replace) = self.download_view.replace(catalog);

          replace
        }
        download_refresh::Effect::Present(Err(fault)) => {
          console_core_logging::warn!("reading browser download catalog: {fault}");
          let Ok(()) = self.download_view.report("Couldn’t refresh downloads");
        }
      }
    }

    Ok(())
  }

  fn start_download_scan(&mut self) -> Result<(), Never> {
    let result = match &self.download_scanner {
      Some(scanner) => scanner.refresh(),
      None => match started_scanner() {
        Ok(scanner) => {
          self.download_scanner = Some(scanner);
          Ok(())
        }
        Err(fault) => Err(fault),
      },
    };

    match result {
      Ok(()) => self.download_view.begin(),
      Err(fault) => {
        console_core_logging::warn!("starting download catalog refresh: {fault}");
        self.download_scanner = None;
        let Ok(recovered) = DownloadRefresh::transition(
          self.download_refresh,
          download_refresh::Request::QueueFailed,
        );
        self.download_refresh = recovered.state;
        self.download_view.report("Couldn’t refresh downloads")
      }
    }
  }

  fn receive_downloads(&mut self) -> core::result::Result<(), console_core_never::Never> {
    {
      let scanner = match self.download_scanner.as_mut() {
        Some(scanner) => scanner,
        None => return Ok(()),
      };
      let Ok(changed) = scanner.changed();
      let Ok(update) = scanner.result();
      let trashed = matches!(
        update,
        Some(download_catalog::Update::File {
          action: FileAction::Trash,
          result: Ok(()),
          ..
        })
      );

      match ::core::primitive::bool::then_some(
        trashed
          || (changed == WatchChange::Pending
            && matches!(
              self.download_view.visibility(),
              Ok(download_view::Visibility::Visible)
            )),
        (),
      ) {
        Some(()) => {
          let Ok(()) = self.refresh_downloads();
        }
        None => {}
      }

      match update {
        Some(download_catalog::Update::Catalog(result)) => {
          let Ok(()) = self.advance_download_refresh(download_refresh::Request::Scanned(result));
        }
        Some(download_catalog::Update::File { id, action, result }) => {
          let Ok(message) = file_action_message(&id, action, result);
          let Ok(()) = self.download_view.file_finished(message);
        }
        None => {}
      }
    };

    Ok(())
  }

  fn queue_file_action(
    &mut self,
    id: downloads::Id,
    action: FileAction,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      match {
        ::core::primitive::bool::then_some(
          matches!(self.download_view.file_busy(), Ok(FileProgress::Working)),
          (),
        )
      } {
        ::core::option::Option::Some(()) => {
          return Ok(());
        }
        ::core::option::Option::None => {}
      }

      let result = self
        .download_scanner
        .as_ref()
        .ok_or_else(|| io::Error::other("the download catalog is unavailable"))
        .and_then(|scanner| scanner.file_action(id, action));

      match result {
        Ok(()) => {
          let Ok(()) = self.download_view.file_started(action);
        }
        Err(fault) => {
          let message = match (fault.kind() == io::ErrorKind::WouldBlock).then_some(()) {
            Some(()) => "Downloads are busy. Try again.",
            None => {
              console_core_logging::warn!("starting download file action: {fault}");
              "Couldn’t start this file action"
            }
          };
          let Ok(()) = self.download_view.file_finished(Some(message));
        }
      }
    };

    Ok(())
  }

  fn download_action(&mut self, action: download_view::Action) -> io::Result<Closed> {
    match action {
      download_view::Action::Handled => {}
      download_view::Action::Dismiss => {
        let Ok(()) = self.close_downloads();
      }
      download_view::Action::Refresh => {
        let Ok(()) = self.refresh_downloads();
      }
      download_view::Action::File {
        id,
        action: action_2,
      } => {
        let Ok(queue_file_action) = self.queue_file_action(id, action_2);

        queue_file_action
      }
      download_view::Action::Folder => {
        match downloads::folder().and_then(open_in_desktop) {
          Err(fault) => {
            console_core_logging::warn!("could not open the downloads folder: {fault}");
            let Ok(()) = self
              .download_view
              .report("Couldn’t open the downloads folder");
          }
          ::core::result::Result::Ok(_) => {}
        };
      }
      download_view::Action::Open(id) => {
        match self.download_view.file(&id).and_then(open_in_desktop) {
          Ok(()) => {}
          Err(fault) => {
            let Ok(id_text) = id.as_str();
            console_core_logging::warn!("opening download {id_text}: {fault}");
            let message = match ::core::primitive::bool::then_some(
              fault.kind() == io::ErrorKind::NotFound,
              (),
            ) {
              ::core::option::Option::Some(()) => "The file or its default app is unavailable",
              ::core::option::Option::None => "Couldn’t open this download",
            };
            let Ok(()) = self.download_view.report(message);
          }
        }
      }
    }

    Ok(Closed::No)
  }

  fn open(&mut self, typed: &str) -> io::Result<()> {
    let Ok(address) = resolve(typed);

    match address {
      Some(address_2) => {
        let Ok(()) = self.active.hide_reader();
        let Ok(()) = self.active.clear_blank_page(&address_2);
        self.active.address = address_2;
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

  fn submit(&mut self, text: &str) -> io::Result<()> {
    let prompt = self
      .prompt
      .take()
      .filter(|_| matches!(self.toolbar.entry_mode(), Ok(EntryMode::Prompt)));

    match prompt {
      None => self.open(text),
      Some(Prompt::Find) => {
        self.active.find = text.to_owned();
        let Ok(()) = self.toolbar.blur(&self.active.address);
        self.find(Direction::Next)
      }
      Some(Prompt::Upload { tab, chooser }) => self.submit_upload(tab, chooser, text),
    }
  }

  fn submit_upload(&mut self, tab: u32, chooser: u64, text: &str) -> io::Result<()> {
    match (self.active.id == tab).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    let Ok(url) = resolve(text);
    let path = url.as_deref().and_then(|address| {
      let Ok(path) = console_core_file_urls::path(address);
      path.filter(|candidate| candidate.is_absolute())
    });
    let usable = path.filter(|candidate| match candidate.metadata() {
      Ok(metadata) => metadata.is_file(),
      Err(fault) => {
        console_core_logging::debug!("cannot select {candidate:?} for upload: {fault}");
        false
      }
    });

    let file = match usable {
      Some(known) => known,
      None => {
        self.prompt = Some(Prompt::Upload { tab, chooser });
        let Ok(()) = self.toolbar.prompt(TextPrompt {
          placeholder: "Choose an existing local file",
          value: "",
        });
        return Ok(());
      }
    };
    let Ok(location) = console_core_file_urls::url(&file);
    let Ok(()) = self.toolbar.blur(&self.active.address);
    self.request(Request::Upload { chooser, location })
  }

  fn upload_prompt(&mut self, tab: u32, chooser: u64) -> Result<(), Never> {
    self.prompt = Some(Prompt::Upload { tab, chooser });
    let Ok(()) = self.toolbar.prompt(TextPrompt {
      placeholder: "File path to upload",
      value: "",
    });
    self.dirty_toolbar()
  }

  fn choose(&mut self, choice: io::Result<file_picker::Choice>) -> io::Result<Closed> {
    self.active.page.redraw = Redraw::Requested;

    match choice {
      Err(fault) => {
        let Ok(()) = self.report_action(fault);
      }
      Ok(file_picker::Choice::Waiting) => {}
      Ok(choice_2) => {
        let picker = {
          let optional_picker = self.file_picker.take();

          match optional_picker {
            Some(matched_picker) => matched_picker,
            ::core::option::Option::None => return Ok(Closed::No),
          }
        };

        let Ok(request) = picker.request();

        match bool::then_some(request.tab == self.active.id, ()) {
          Some(()) => {}
          None => return Ok(Closed::No),
        }

        match choice_2 {
          file_picker::Choice::PathEntry => {
            let Ok(()) = self.upload_prompt(request.tab, request.chooser);
          }
          file_picker::Choice::Selected(path) => {
            let Ok(location) = console_core_file_urls::url(&path);
            self.request(Request::Upload {
              chooser: request.chooser,
              location,
            })?;
          }
          file_picker::Choice::Waiting | file_picker::Choice::Cancelled => {}
        }
      }
    }

    Ok(Closed::No)
  }

  fn find(&mut self, direction: Direction) -> io::Result<()> {
    match { ::core::primitive::bool::then_some(!self.active.find.is_empty(), ()) } {
      ::core::option::Option::Some(()) => {
        self.request(Request::Find {
          text: self.active.find.clone(),
          backward: matches!(direction, Direction::Previous),
        })?;
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  fn copy(&mut self, text: String) -> core::result::Result<(), console_core_never::Never> {
    {
      match crate::clipboard::copy(&text) {
        Err(fault) => {
          console_core_logging::debug!("writing the system clipboard: {}", fault.kind());
        }
        ::core::result::Result::Ok(_) => {}
      }

      self.clipboard = text;
    };

    Ok(())
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
    let mods = control | shift | alt | super_key;

    match &mut self.file_picker {
      Some(picker) => {
        let choice = match {
          ::core::primitive::bool::then_some(
            (mods, press.key) == (crate::toolbar::CONTROL, Keysym::l),
            (),
          )
        } {
          ::core::option::Option::Some(()) => Ok(file_picker::Choice::PathEntry),
          ::core::option::Option::None => picker.key(press.key, self.viewport.height),
        };
        return self.choose(choice);
      }
      ::core::option::Option::None => {}
    }

    match {
      ::core::primitive::bool::then_some(
        (mods, press.key) == (crate::toolbar::CONTROL, Keysym::l)
          && matches!(self.toolbar.entry_mode(), Ok(EntryMode::Prompt)),
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.toolbar.blur(&self.active.address);
        self.prompt = None;
      }
      ::core::option::Option::None => {}
    }

    let shortcut = self.page_shortcut(press.key, mods)?;

    match shortcut {
      Shortcut::Handled => return Ok(Closed::No),
      Shortcut::Unclaimed => {}
    }

    match {
      ::core::primitive::bool::then_some(
        matches!(
          self.download_view.visibility(),
          Ok(download_view::Visibility::Visible)
        ),
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        const CONTROL_SHIFT: u32 = crate::toolbar::CONTROL | crate::toolbar::SHIFT;

        match (mods, press.key) {
          (crate::toolbar::CONTROL, Keysym::l | Keysym::t | Keysym::Tab)
          | (CONTROL_SHIFT, Keysym::Tab | Keysym::ISO_Left_Tab) => {
            let Ok(()) = self.close_downloads();
          }
          _ => {
            let Ok(action) = self.download_view.key(press.key, mods);
            return self.download_action(action);
          }
        }
      }
      ::core::option::Option::None => {}
    }

    let Ok(action) = self.toolbar.key(press.key, mods, &self.active.address);

    match action {
      Action::Compose => {
        match character_2 {
          Some(character_3) => {
            let Ok(()) = self.toolbar.text(&character_3.to_string());
          }
          None => {}
        }

        let Ok(()) = self.dirty_toolbar();
        Ok(Closed::No)
      }
      Action::Pass => self.page_keyboard(press.key, mods, character_2),
      remaining @ (Action::Handled
      | Action::Back
      | Action::Forward
      | Action::Reload
      | Action::Stop
      | Action::Close
      | Action::CloseTab
      | Action::NewTab
      | Action::Downloads
      | Action::SelectTab(_)
      | Action::CycleTab(_)
      | Action::Reader(_)
      | Action::PageView
      | Action::ReaderText(_)
      | Action::SettingsChanged
      | Action::Open(_)
      | Action::Switch(_)
      | Action::FocusPage
      | Action::Copy(_)
      | Action::Paste) => self.action(remaining),
    }
  }

  fn page_shortcut(&mut self, key: Keysym, mods: u32) -> io::Result<Shortcut> {
    let blocked = matches!(
      self.download_view.visibility(),
      Ok(download_view::Visibility::Visible)
    ) || matches!(
      self.toolbar.settings_visibility(),
      Ok(settings::Visibility::Visible)
    ) || matches!(self.toolbar.entry_mode(), Ok(EntryMode::Prompt));

    match blocked.then_some(()) {
      Some(()) => return Ok(Shortcut::Unclaimed),
      None => {}
    }

    match (mods, key) {
      (crate::toolbar::CONTROL, Keysym::f) => {
        let Ok(()) = self.active.hide_reader();
        self.active.find_status = FindStatus::Unreported;
        self.prompt = Some(Prompt::Find);
        let Ok(()) = self.toolbar.prompt(TextPrompt {
          placeholder: "Find in page",
          value: &self.active.find,
        });
        let Ok(()) = self.dirty_toolbar();
      }
      (0 | crate::toolbar::SHIFT, Keysym::F3) => {
        let direction = match mods {
          crate::toolbar::SHIFT => Direction::Previous,
          _ => Direction::Next,
        };
        self.find(direction)?;
      }
      (crate::toolbar::CONTROL, Keysym::minus | Keysym::equal | Keysym::_0) => {
        match self.active.page.reader {
          Reader::Hidden => {}
          Reader::Pending { .. } | Reader::Open { .. } => return Ok(Shortcut::Unclaimed),
        }

        self.active.zoom = match key {
          Keysym::minus => self.active.zoom.saturating_sub(10).max(50),
          Keysym::equal => self.active.zoom.saturating_add(10).min(200),
          _ => 100,
        };
        self.request(Request::Zoom(self.active.zoom))?;
      }
      _ => return Ok(Shortcut::Unclaimed),
    }

    Ok(Shortcut::Handled)
  }

  fn page_keyboard(
    &mut self,
    key: Keysym,
    mods: u32,
    character: Option<char>,
  ) -> io::Result<Closed> {
    match &mut self.active.page.reader {
      Reader::Open { article, .. } => {
        let Ok(()) = article.key(key, self.viewport.height);
        self.active.page.redraw = Redraw::Requested;
      }
      Reader::Pending { .. } => {}
      Reader::Hidden => self.renderer_keyboard(key, mods, character)?,
    }

    Ok(Closed::No)
  }

  fn renderer_keyboard(
    &mut self,
    key: Keysym,
    mods: u32,
    character: Option<char>,
  ) -> io::Result<()> {
    match (mods, key, self.active.renderer) {
      (crate::toolbar::CONTROL, Keysym::v, _) => {
        match crate::clipboard::paste() {
          Ok(Some(text)) => self.request(Request::Paste(text))?,
          Ok(None) => {}
          Err(fault) => {
            console_core_logging::debug!("reading the system clipboard: {}", fault.kind());
          }
        }

        return Ok(());
      }
      (crate::toolbar::CONTROL, Keysym::c, Renderer::Chromium) => {
        self.active.selection = ClipboardSelection::Waiting;
        return self.request(Request::CopySelection);
      }
      _ => {}
    }

    let page_mods = mods & !crate::toolbar::SHIFT;
    let Ok(special) = page_key(key);

    match (page_mods, character, special) {
      (0, Some(text), _) => self.request(Request::Text(text.to_string())),
      (0, None, Some(known_key)) => self.request(Request::Key(known_key.to_owned())),
      _ => Ok(()),
    }
  }

  fn pointer(&mut self, pointer: PointerEvent) -> io::Result<Closed> {
    let toolbar_height = f64::from(toolbar::HEIGHT);

    match &mut self.file_picker {
      Some(picker) => {
        return match picker_pointer(picker, self.viewport.height, pointer) {
          Ok(PickerPointer::Interacted(choice)) => self.choose(Ok(choice)),
          Ok(PickerPointer::Ignored) => Ok(Closed::No),
          Err(fault) => self.choose(Err(fault)),
        };
      }
      None => {}
    }

    match (self.download_view.visibility(), pointer) {
      (Ok(download_view::Visibility::Visible), PointerEvent::Down { at: (x, y) }) => {
        match (y >= toolbar_height).then_some(()) {
          Some(()) => {
            let Ok(at) = toolbar_point(Point {
              x,
              y: y - toolbar_height,
            });
            let Ok(action) = self.download_view.click(at);
            return self.download_action(action);
          }
          None => {}
        }
      }
      (Ok(download_view::Visibility::Visible), PointerEvent::Scrolled { by }) => {
        let Ok(()) = self.download_view.scroll(ScrollPixels(by));
        return Ok(Closed::No);
      }
      (Ok(download_view::Visibility::Hidden), _)
      | (
        Ok(download_view::Visibility::Visible),
        PointerEvent::Moved { .. }
        | PointerEvent::Pinched { .. }
        | PointerEvent::Up
        | PointerEvent::Left,
      ) => {}
    }

    match pointer {
      PointerEvent::Down { at: (x, y) } => {
        match (y < toolbar_height
          || matches!(
            self.toolbar.settings_visibility(),
            Ok(settings::Visibility::Visible)
          ))
        .then_some(())
        {
          Some(()) => self.press_toolbar(Point { x, y }),
          None => self.press_page(Point { x, y }),
        }
      }
      PointerEvent::Scrolled { by } => {
        match matches!(
          self.toolbar.settings_visibility(),
          Ok(settings::Visibility::Visible)
        )
        .then_some(())
        {
          Some(()) => return Ok(Closed::No),
          None => {}
        }

        match &mut self.active.page.reader {
          Reader::Open { article, .. } => {
            let Ok(()) = article.scroll(ScrollPixels(by), self.viewport.height);
            self.active.page.redraw = Redraw::Requested;
          }
          Reader::Pending { .. } => {}
          Reader::Hidden => self.request(Request::Scroll(Point { x: 0.0, y: by }))?,
        }

        Ok(Closed::No)
      }
      PointerEvent::Moved { .. }
      | PointerEvent::Left
      | PointerEvent::Pinched { .. }
      | PointerEvent::Up => Ok(Closed::No),
    }
  }

  fn press_page(&mut self, at: Point<f64>) -> io::Result<Closed> {
    let Ok(()) = self.toolbar.blur(&self.active.address);
    let Ok(()) = self.dirty_toolbar();

    match self.active.page.reader {
      Reader::Hidden => {}
      Reader::Pending { .. } | Reader::Open { .. } => return Ok(Closed::No),
    }

    let Ok(content) = content_viewport(self.viewport);
    let Ok(visible) = fitted_page(self.viewport, content);
    let x = at.x - f64::from(visible.at.x);
    let y = at.y - f64::from(toolbar::HEIGHT) - f64::from(visible.at.y);
    let size = Size {
      width: f64::from(visible.size.width),
      height: f64::from(visible.size.height),
    };
    let within = x >= 0.0 && x < size.width && y >= 0.0 && y < size.height;

    match within.then_some(()) {
      Some(()) => {
        self.request(Request::Click(Point {
          x: x * f64::from(content.width) / size.width,
          y: y * f64::from(content.height) / size.height,
        }))?;
      }
      None => {}
    }

    Ok(Closed::No)
  }

  fn press_toolbar(&mut self, at: Point<f64>) -> io::Result<Closed> {
    let Ok(action) = self.toolbar.tap(
      {
        let Ok(explicit_value) = toolbar_point(at);
        explicit_value
      },
      &self.active.address,
    );

    match action {
      Action::Pass => {
        let Ok(()) = self.surface.move_window();

        Ok(Closed::No)
      }
      remaining @ (Action::Handled
      | Action::Compose
      | Action::Back
      | Action::Forward
      | Action::Reload
      | Action::Stop
      | Action::Close
      | Action::CloseTab
      | Action::NewTab
      | Action::Downloads
      | Action::SelectTab(_)
      | Action::CycleTab(_)
      | Action::Reader(_)
      | Action::PageView
      | Action::ReaderText(_)
      | Action::SettingsChanged
      | Action::Open(_)
      | Action::Switch(_)
      | Action::FocusPage
      | Action::Copy(_)
      | Action::Paste) => self.action(remaining),
    }
  }

  fn hover_toolbar(&mut self, at: Option<Point<i32>>) -> Result<(), Never> {
    let Ok(hover) = self.toolbar.hover(at);

    match hover {
      ToolbarChange::Different => self.dirty_toolbar(),
      ToolbarChange::Unchanged => Ok(()),
    }
  }

  fn toolbar_frame(&self, room: Size<u32>) -> Result<Option<(Frame, FrameLayout)>, Never> {
    let Ok(scale) = self.surface.scale();
    let points = Size {
      width: room.width,
      height: room.height.min(toolbar::HEIGHT),
    };
    let Ok(device) = scale.device(points);
    let frame = Frame { points, device };
    let layout = match FrameLayout::rgba(device) {
      Ok(layout) => layout,
      Err(fault) => {
        console_core_logging::debug!(
          "browser toolbar waiting for a drawable viewport: {fault:?}"
        );

        return Ok(None);
      }
    };
    Ok(Some((frame, layout)))
  }

  fn refresh(&mut self) -> Result<(), Box<dyn Error>> {
    let Ok(logical) = self.surface.logical();
    let logical_3 = match logical {
      Some(logical_2) => logical_2,
      None => return Ok(()),
    };
    let Ok(pointer) = self.surface.pointer_pixel();
    let Ok(()) = self.hover_toolbar(pointer.filter(|_| self.file_picker.is_none()));
    let bar_height = logical_3.height.min(toolbar::HEIGHT);
    let viewport = Size {
      width: logical_3.width.max(1),
      height: logical_3.height.saturating_sub(bar_height).max(1),
    };

    match (viewport == self.viewport).then_some(()) {
      Some(()) => {}
      None => {
        let Ok(previous) = content_viewport(self.viewport);
        let Ok(content) = content_viewport(viewport);
        self.viewport = viewport;
        self.active.page.redraw = Redraw::Requested;

        match { ::core::primitive::bool::then_some(previous != content, ()) } {
          ::core::option::Option::Some(()) => {
            self.request(Request::Resize(content))?;
          }
          ::core::option::Option::None => {}
        }

        let Ok(()) = self.dirty_toolbar();
      }
    }

    match &mut self.active.page.reader {
      Reader::Open { mode, article } => {
        let (family, size) = match mode {
          Mode::Dark => (console_core_fonts::SERIF, self.active.reader_text_size),
          Mode::Desktop => (self.desktop_font.family.as_str(), self.desktop_font.height),
        };
        let Ok(()) = article.prepare(viewport, family, size);
      }
      Reader::Hidden | Reader::Pending { .. } => {}
    }

    let Ok(desktop_colors) = self.toolbar.reader_colors();
    let Ok(bar_frame) = self.toolbar_frame(logical_3);
    let (frame, layout) = match bar_frame {
      Some(ready) => ready,
      None => return Ok(()),
    };
    let Ok(geometry) = self.bar.geometry(frame);
    let bar_changed = self.bar.redraw == Redraw::Requested || geometry == Redraw::Requested;
    let page_changed = match self.download_view.visibility() {
      Ok(download_view::Visibility::Visible) => {
        matches!(
          self.download_view.drawing(),
          Ok(download_view::Drawing::Pending)
        )
      }
      Ok(download_view::Visibility::Hidden) => self.active.page.redraw == Redraw::Requested,
    } || geometry == Redraw::Requested;

    match bar_changed.then_some(()) {
      Some(()) => {
        self.bar.paint(&mut self.toolbar, frame, layout)?;
        let Ok(()) = self.toolbar_text_input();
      }
      None => {}
    }

    match (bar_changed || page_changed).then_some(()) {
      Some(()) => {}
      None => {
        let Ok(cursor) = self.toolbar.pointer_cursor();
        let Ok(()) = self.surface.set_pointer_cursor(cursor);

        return Ok(());
      }
    }

    let Ok(layers) = self.surface.layer_support();

    let overlay = (self.file_picker.is_some()
      || matches!(
        self.download_view.visibility(),
        Ok(download_view::Visibility::Visible)
      )
      || matches!(
        self.toolbar.settings_visibility(),
        Ok(settings::Visibility::Visible)
      ))
    .then_some(());

    let page_layout = PageLayout {
      viewport,
      entire: logical_3,
      bar_height,
    };
    let repaint = presentation::Repaint {
      before: self.backdrop,
      geometry: presentation::Geometry {
        window: logical_3,
        toolbar: frame,
      },
      layers: match (layers, overlay) {
        (LayerSupport::Available, None) => presentation::Layers::Available,
        (LayerSupport::Available, Some(())) | (LayerSupport::Unavailable, _) => {
          presentation::Layers::Combined
        }
      },
      toolbar: match bar_changed.then_some(()) {
        Some(()) => Redraw::Requested,
        None => Redraw::Current,
      },
      page: match page_changed.then_some(()) {
        Some(()) => Redraw::Requested,
        None => Redraw::Current,
      },
    };
    let backdrop = Presentation::paint(&repaint, self, |browser, effect| {
      browser.present(effect, page_layout, desktop_colors)
    })?;
    self.backdrop = backdrop;

    match self.download_view.visibility() {
      Ok(download_view::Visibility::Visible) => {}
      Ok(download_view::Visibility::Hidden) => self.active.page.redraw = Redraw::Current,
    }

    let Ok(cursor) = self.toolbar.pointer_cursor();
    let Ok(()) = self.surface.set_pointer_cursor(cursor);

    Ok(())
  }

  fn present(
    &mut self,
    effect: presentation::Effect,
    layout: PageLayout,
    colors: ReaderColors,
  ) -> Result<Visible, Box<dyn Error>> {
    let bar = self
      .bar
      .visible
      .as_ref()
      .ok_or_else(|| invalid("missing browser toolbar"))?;

    match effect {
      presentation::Effect::DrawPageLayer => {
        draw_page_layer(&mut self.surface, &self.active.page, layout, colors)
      }
      presentation::Effect::PatchToolbar => match bar.changed.as_ref() {
        Some(patch) => self.surface.draw_patch(patch).map_err(Into::into),
        None => Ok(Visible::Yes),
      },
      presentation::Effect::DrawCombined => {
        draw_combined(
          &mut self.surface,
          bar,
          &self.active.page,
          layout,
          colors,
          &mut self.toolbar,
          &mut self.download_view,
          self.file_picker.as_ref(),
        )?;
        Ok(Visible::Yes)
      }
      presentation::Effect::ClearPageLayer => {
        let Ok(()) = self.surface.clear_layer();
        Ok(Visible::Yes)
      }
    }
  }
}

type ReaderColors = (console_core_color::Oklch, console_core_color::Oklch);

fn file_action_message(
  id: &downloads::Id,
  action: FileAction,
  result: io::Result<()>,
) -> Result<Option<&'static str>, Never> {
  Ok(match (action, result) {
    (FileAction::Reveal, Ok(())) => None,
    (FileAction::Trash, Ok(())) => Some("Moved to Trash"),
    (FileAction::Reveal, Err(fault)) => {
      let Ok(id_text) = id.as_str();
      console_core_logging::warn!("download {id_text} Reveal: {fault}");
      Some("Couldn’t reveal this download")
    }
    (FileAction::Trash, Err(fault)) => {
      let Ok(id_text) = id.as_str();
      console_core_logging::warn!("download {id_text} Trash: {fault}");
      Some("Couldn’t move this download to Trash")
    }
  })
}

#[must_use]
fn started_scanner() -> io::Result<Scanner> {
  let folder = downloads::folder()?;
  let scanner = Scanner::new(folder)?;
  scanner.refresh()?;
  Ok(scanner)
}

enum PickerPointer {
  Ignored,
  Interacted(file_picker::Choice),
}

fn picker_pointer(
  picker: &mut Picker,
  viewport_height: u32,
  pointer: PointerEvent,
) -> io::Result<PickerPointer> {
  let origin = f64::from(toolbar::HEIGHT);

  match pointer {
    PointerEvent::Down { at: (_, y) } => match (y >= origin).then_some(()) {
      Some(()) => {
        let Ok(row) = console_core_number_conversion::toward_zero_u32(y - origin);
        picker
          .click(VerticalOffset(row), viewport_height)
          .map(PickerPointer::Interacted)
      }
      None => Ok(PickerPointer::Ignored),
    },
    PointerEvent::Scrolled { by } => match by.classify() {
      core::num::FpCategory::Zero => Ok(PickerPointer::Ignored),
      core::num::FpCategory::Nan
      | core::num::FpCategory::Infinite
      | core::num::FpCategory::Subnormal
      | core::num::FpCategory::Normal => {
        let key = match (by > 0.0).then_some(()) {
          Some(()) => Keysym::Down,
          None => Keysym::Up,
        };
        picker
          .key(key, viewport_height)
          .map(PickerPointer::Interacted)
      }
    },
    PointerEvent::Moved { .. }
    | PointerEvent::Pinched { .. }
    | PointerEvent::Up
    | PointerEvent::Left => Ok(PickerPointer::Ignored),
  }
}

fn toolbar_point(at: Point<f64>) -> core::result::Result<Point<i32>, console_core_never::Never> {
  let Ok(x) = console_core_number_conversion::toward_zero_i32(at.x.floor());
  let Ok(y) = console_core_number_conversion::toward_zero_i32(at.y.floor());

  Ok(Point { x, y })
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
        let Ok(()) = paint_page(pixels, frame, picture, desktop_colors.0);
        return Ok(());
      }
      (Reader::Hidden | Reader::Pending { .. }, None) => {
        let Ok(background) = crate::reader::background(frame.points, crate::reader::DARK.0);
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
    let mut pixels = std::mem::take(&mut self.spare);
    resize_pixels(&mut pixels, layout)?;
    toolbar.paint(&mut pixels, frame)?;
    let changing = match self.visible.as_ref().filter(|prior| prior.frame == frame) {
      Some(previous) => {
        let Ok(area) = Area::between(frame.device, &previous.pixels, &pixels);
        area
      }
      None => None,
    };
    let changed = match changing {
      Some(area) => {
        let patch = Patch::extract(frame.device, &pixels, area)?;
        Some(patch)
      }
      None => None,
    };

    match self.visible.replace(PaintedToolbar {
      frame,
      pixels,
      changed,
    }) {
      Some(previous) => self.spare = previous.pixels,
      None => {}
    }

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

#[derive(Clone, Copy)]
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
  downloads: &mut download_view::View,
  file_picker: Option<&Picker>,
) -> Result<(), Box<dyn Error>> {
  surface.draw_with(
    &mut (toolbar, downloads),
    |(bar_controls, download_overlay), pixels, device, _| {
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

          match (
            file_picker,
            matches!(
              download_overlay.visibility(),
              Ok(download_view::Visibility::Visible)
            ),
          ) {
            (Some(picker), _) => {
              let Ok((palette, font)) = bar_controls.style();

              {
                match picker.paint(page_pixels, frame, palette, font) {
                  Err(fault) => {
                    console_core_logging::error!("could not paint the upload chooser: {fault}");
                  }
                  ::core::result::Result::Ok(_) => {}
                };
              }
            }
            (None, true) => {
              let Ok((palette, font)) = bar_controls.style();

              {
                match download_overlay.paint(page_pixels, frame, palette, font) {
                  Err(fault) => {
                    console_core_logging::error!("could not paint browser downloads: {fault}");
                  }
                  ::core::result::Result::Ok(_) => {}
                };
              }
            }
            (None, false) => {
              let Ok(()) = page.paint(page_pixels, frame, desktop_colors);
            }
          }
        }
        (None, _) | (Some(_), 0) => {}
      }

      bar_controls.paint_settings(
        pixels,
        Frame {
          points: layout.entire,
          device,
        },
      )
    },
  )?;
  Ok(())
}

fn paint_page(
  pixels: &mut [u8],
  frame: Frame,
  picture: &Pixels,
  background: console_core_color::Oklch,
) -> Result<(), Never> {
  let Ok(content) = picture.size();
  let Ok(visible) = fitted_page(frame.points, content);

  match { ::core::primitive::bool::then_some(visible.size != frame.points, ()) } {
    ::core::option::Option::Some(()) => {
      let Ok(backdrop) = crate::reader::background(frame.points, background);

      {
        match painting::onto(pixels, frame, &[backdrop]) {
          Err(fault) => {
            console_core_logging::error!("could not paint browser background: {fault}");
          }
          ::core::result::Result::Ok(_) => {}
        };
      }
    }
    ::core::option::Option::None => {}
  }

  let Ok(image) = Picture::rectangular(visible.at, visible.size, picture);

  match painting::over_picture(pixels, frame, image, &[]) {
    Ok(()) => {}
    Err(fault) => console_core_logging::error!("could not paint browser page: {fault}"),
  }

  Ok(())
}

fn content_viewport(available: Size<u32>) -> Result<Size<u32>, Never> {
  #[must_use]
  fn bucket<const STEP: u32>(dimension: u32) -> Result<u32, Never> {
    let bounded = dimension.min(MAX_SIDE);
    let remainder = match bounded.checked_rem(STEP) {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => 0,
    };

    Ok(bounded.saturating_sub(remainder).max(STEP))
  }

  let Ok(width) = bucket::<{ VIEWPORT_BUCKET.width }>(available.width);
  let Ok(height) = bucket::<{ VIEWPORT_BUCKET.height }>(available.height);

  Ok(Size { width, height })
}

fn fitted_page(available: Size<u32>, content: Size<u32>) -> Result<Part, Never> {
  let scale = (f64::from(available.width) / f64::from(content.width))
    .min(f64::from(available.height) / f64::from(content.height));
  let Ok(width) =
    console_core_number_conversion::toward_zero_u32((f64::from(content.width) * scale).round());
  let Ok(height) =
    console_core_number_conversion::toward_zero_u32((f64::from(content.height) * scale).round());
  let size = Size {
    width: width.clamp(1, available.width.max(1)),
    height: height.clamp(1, available.height.max(1)),
  };
  let Ok(x) = console_core_number_conversion::fitted(::core::primitive::u32::wrapping_div(
    available.width.saturating_sub(size.width),
    2,
  ));
  let Ok(y) = console_core_number_conversion::fitted(::core::primitive::u32::wrapping_div(
    available.height.saturating_sub(size.height),
    2,
  ));
  Ok(Part {
    at: Point { x, y },
    size,
  })
}

enum CheckProgress {
  Pending,
  Complete,
}

enum DownloadCheck {
  Render,
  Link(String),
  Address(String),
  Catalog,
  Trashing {
    removed: downloads::Id,
    kept: downloads::Id,
  },
}

struct CheckedDownloads {
  first: downloads::Id,
  second: downloads::Id,
  paths: [PathBuf; 2],
}

fn open_download(view: &mut download_view::View) -> io::Result<downloads::Id> {
  let Ok(_focused) = view.key(Keysym::Down, 0);
  let Ok(action) = view.key(Keysym::Return, 0);

  match action {
    download_view::Action::Open(id) => Ok(id),
    download_view::Action::Handled
    | download_view::Action::Dismiss
    | download_view::Action::Refresh
    | download_view::Action::Folder
    | download_view::Action::File { .. } => {
      Err(invalid("download catalog did not open the focused file"))
    }
  }
}

fn checked_downloads(browser: &mut Browser) -> io::Result<CheckedDownloads> {
  for character in "report 127.0.0.1".chars() {
    let Ok(key) = Keysym::from_char(character);
    let Ok(press) = KeyPress::unmodified(key);
    browser.keyboard(press)?;
  }

  let view = &mut browser.download_view;
  let Ok(count) = view.count();

  match count {
    2 => {}
    _ => {
      return Err(invalid(
        "catalog search did not find both fixture downloads",
      ));
    }
  }

  let first = open_download(view)?;
  let second = open_download(view)?;

  match (first == second).then_some(()) {
    Some(()) => {
      return Err(invalid(
        "download catalog did not resolve separate files by ID",
      ));
    }
    None => {}
  }

  let first_path = view.file(&first)?;
  let second_path = view.file(&second)?;
  let expected = first_path.file_name() == Some(std::ffi::OsStr::new("report.txt"))
    && second_path.file_name() == first_path.file_name();

  match expected.then_some(()) {
    Some(()) => {}
    None => return Err(invalid("catalog did not open the expected fixture paths")),
  }

  let Ok(_menu) = view.key(Keysym::Right, 0);
  let Ok(reveal) = view.key(Keysym::Return, 0);
  let correct = matches!(reveal, download_view::Action::File { id, action: FileAction::Reveal } if id == second);

  match correct.then_some(()) {
    Some(()) => {}
    None => return Err(invalid("Reveal in Folder did not select the focused ID")),
  }

  println!("The native download catalog found and resolved both IDs.");
  Ok(CheckedDownloads {
    first,
    second,
    paths: [first_path, second_path],
  })
}

impl DownloadCheck {
  fn begin(&mut self, browser: &mut Browser) -> io::Result<()> {
    let page = browser.active.address.clone();
    browser.pointer({
      let Ok(explicit_value) = page_press(browser.viewport, Point { x: 40, y: 96 });
      explicit_value
    })?;
    *self = Self::Link(page);
    Ok(())
  }

  fn advance(
    &mut self,
    browser: &mut Browser,
    trash_root: Option<&Path>,
  ) -> io::Result<CheckProgress> {
    match self {
      Self::Catalog => return self.catalog(browser, trash_root),
      Self::Trashing { removed, kept } => {
        let view = &browser.download_view;

        match {
          ::core::primitive::bool::then_some(
            matches!(view.file_busy(), Ok(FileProgress::Working))
              || !matches!(view.availability(), Ok(download_view::Availability::Ready)),
            (),
          )
        } {
          ::core::option::Option::Some(()) => {
            return Ok(CheckProgress::Pending);
          }
          ::core::option::Option::None => {}
        }

        let Ok(count) = view.count();
        let consistent = match count {
          1 => {
            matches!(view.file(removed), Err(_removed)) && matches!(view.file(kept), Ok(_kept))
          }
          _wrong_count => false,
        };

        match bool::then_some(!consistent, ()) {
          ::core::option::Option::Some(()) => {
            return Err(invalid(
              "moving one fixture download to Trash affected the wrong IDs",
            ));
          }
          ::core::option::Option::None => {}
        }

        return Self::verify_icon(browser);
      }
      Self::Render | Self::Link(_) | Self::Address(_) => {}
    }

    match browser.download_notice.as_ref() {
      Some(DownloadNotice {
        state: DownloadState::Failed,
        name,
        ..
      }) => {
        return Err(io::Error::other(format!("fixture download failed: {name}")));
      }
      Some(DownloadNotice {
        state: DownloadState::Saved,
        name,
        ..
      }) => match name.as_str() {
        "report.txt" => {}
        _ => return Ok(CheckProgress::Pending),
      },
      Some(DownloadNotice {
        state: DownloadState::Started,
        ..
      })
      | None => {
        return Ok(CheckProgress::Pending);
      }
    }

    match self {
      Self::Link(page) => {
        let host = page
          .strip_suffix("/page")
          .ok_or_else(|| invalid("unexpected fixture URL"))?;
        browser.download_notice = None;
        browser.open(&format!("{host}/download/second"))?;
        *self = Self::Address(std::mem::take(page));
      }
      Self::Address(page) => {
        let ready = page == &browser.active.address && browser.active.loading == PageLoad::Ready;

        match ready.then_some(()) {
          Some(()) => {
            let Ok(press) = KeyPress::with_modifiers(Keysym::j, KeyPress::CONTROL);
            browser.keyboard(press)?;
            *self = Self::Catalog;
          }
          None => {}
        }
      }
      Self::Render | Self::Catalog | Self::Trashing { .. } => {}
    }

    Ok(CheckProgress::Pending)
  }

  fn catalog(
    &mut self,
    browser: &mut Browser,
    trash_root: Option<&Path>,
  ) -> io::Result<CheckProgress> {
    match {
      ::core::primitive::bool::then_some(
        !matches!(
          browser.download_view.visibility(),
          Ok(download_view::Visibility::Visible)
        ),
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        return Err(invalid("Ctrl+J did not open the download catalog"));
      }
      ::core::option::Option::None => {}
    }

    match {
      ::core::primitive::bool::then_some(
        !matches!(
          browser.download_view.availability(),
          Ok(download_view::Availability::Ready)
        ),
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        return Ok(CheckProgress::Pending);
      }
      ::core::option::Option::None => {}
    }

    let CheckedDownloads {
      first,
      second,
      paths,
    } = checked_downloads(browser)?;

    let root = match trash_root {
      Some(path) => path,
      None => return Self::verify_icon(browser),
    };

    for path in paths {
      match {
        ::core::primitive::bool::then_some(path.parent().and_then(Path::parent) != Some(root), ())
      } {
        ::core::option::Option::Some(()) => {
          return Err(invalid("trash check needs paths in its private fixture"));
        }
        ::core::option::Option::None => {}
      }
    }

    let view = &mut browser.download_view;
    let Ok(_menu) = view.key(Keysym::Right, 0);
    let Ok(_select) = view.key(Keysym::Down, 0);
    let Ok(trash) = view.key(Keysym::Return, 0);

    match {
      ::core::primitive::bool::then_some(
        !matches!(&trash, download_view::Action::File { id, action: FileAction::Trash } if id == &second),
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        return Err(invalid("Move to Trash did not select the focused ID"));
      }
      ::core::option::Option::None => {}
    }

    browser.download_action(trash)?;

    match {
      ::core::primitive::bool::then_some(
        !matches!(browser.download_view.file_busy(), Ok(FileProgress::Working)),
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        return Err(invalid("the catalog did not queue Move to Trash"));
      }
      ::core::option::Option::None => {}
    }

    *self = Self::Trashing {
      removed: second,
      kept: first,
    };
    Ok(CheckProgress::Pending)
  }

  fn verify_icon(browser: &mut Browser) -> io::Result<CheckProgress> {
    let icon = (f64::from(browser.viewport.width.saturating_sub(27)), 64.0);
    browser.pointer(PointerEvent::Down { at: icon })?;

    match {
      ::core::primitive::bool::then_some(
        matches!(
          browser.download_view.visibility(),
          Ok(download_view::Visibility::Visible)
        ),
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        return Err(invalid("the download icon did not close the catalog"));
      }
      ::core::option::Option::None => {}
    }

    browser.pointer(PointerEvent::Down { at: icon })?;

    match {
      ::core::primitive::bool::then_some(
        !matches!(
          browser.download_view.visibility(),
          Ok(download_view::Visibility::Visible)
        ),
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        return Err(invalid("the download icon did not open the catalog"));
      }
      ::core::option::Option::None => {}
    }

    Ok(CheckProgress::Complete)
  }
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
        browser.pointer({
          let Ok(explicit_value) = page_press(browser.viewport, Point { x: 40, y: 40 });
          explicit_value
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

#[must_use]
fn page_press(
  viewport: Size<u32>,
  at: Point<u32>,
) -> core::result::Result<PointerEvent, console_core_never::Never> {
  let Ok(content) = content_viewport(viewport);
  let Ok(visible) = fitted_page(viewport, content);

  Ok(PointerEvent::Down {
    at: (
      f64::from(visible.at.x)
        + f64::from(at.x) * f64::from(visible.size.width) / f64::from(content.width),
      f64::from(toolbar::HEIGHT)
        + f64::from(visible.at.y)
        + f64::from(at.y) * f64::from(visible.size.height) / f64::from(content.height),
    ),
  })
}

struct SmokeCheck {
  engine: Renderer,
  deadline: Instant,
  completed: u32,
  webkit: WebKitCheck,
  downloads: DownloadCheck,
  trash_root: Option<PathBuf>,
}

impl SmokeCheck {
  fn new(engine: Renderer) -> io::Result<Self> {
    let Ok(duration) = smoke_duration(engine);
    let Ok(trash_root) = smoke_trash_root();
    let deadline = Instant::now()
      .checked_add(duration)
      .ok_or_else(|| io::Error::other("browser check deadline is too far away"))?;

    Ok(Self {
      engine,
      deadline,
      completed: 0,
      webkit: WebKitCheck::Page,
      downloads: DownloadCheck::Render,
      trash_root,
    })
  }

  fn advance(&mut self, browser: &mut Browser) -> io::Result<CheckProgress> {
    match browser.active.worker {
      Some(_) => {}
      None => return Err(io::Error::other(browser.active.title.clone())),
    }

    match self.downloads {
      DownloadCheck::Render => {}
      DownloadCheck::Link(_)
      | DownloadCheck::Address(_)
      | DownloadCheck::Catalog
      | DownloadCheck::Trashing { .. } => {
        return self.downloads.advance(browser, self.trash_root.as_deref());
      }
    }

    let Ok(first_pixel) = browser.first_pixel();
    let rendered = matches!(
      (browser.active.loading, first_pixel),
      (PageLoad::Ready, Some([12, 34, 56, 255]))
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
      (Renderer::Chromium, 2) | (Renderer::WebKit | Renderer::Servo, 3) => {
        let address = browser.active.address.clone();
        browser.default_engine = Renderer::Chromium;
        browser.new_tab()?;
        browser.open(&address)?;
      }
      _ => self.downloads.begin(browser)?,
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
    reason = "the smoke-test driver reads its private download fixture directory once at startup"
  )
)]
fn smoke_trash_root() -> Result<Option<PathBuf>, Never> {
  Ok(std::env::var_os("CONSOLE_BROWSER_CHECK_TRASH_ROOT").map(PathBuf::from))
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

  fn toolbar_text_input(&mut self) -> Result<(), Never> {
    let obstructed = self.file_picker.is_some()
      || matches!(
        self.download_view.visibility(),
        Ok(download_view::Visibility::Visible)
      )
      || matches!(
        self.toolbar.settings_visibility(),
        Ok(settings::Visibility::Visible)
      );
    let Ok(state) = match obstructed.then_some(()) {
      Some(()) => Ok(None),
      None => self.toolbar.text_input_state(),
    };
    self.surface.set_text_input(state)
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

    let Ok(changes) = self.surface.text_input_changes();

    for change in changes {
      let Ok(()) = self.toolbar.text_input(change);
      let Ok(()) = self.dirty_toolbar();
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
  remembered: Option<String>,
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

    let Ok(()) = self.browser.receive();
    let Ok(()) = self.browser.receive_downloads();

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

    match { ::core::primitive::bool::then_some(self.check.is_none(), ()) } {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.remember();
      }
      ::core::option::Option::None => {}
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
      .chain(
        self
          .browser
          .download_scanner
          .iter()
          .flat_map(|explicit_argument_0| {
            let Ok(descriptors) = Scanner::descriptors(explicit_argument_0);

            descriptors
          }),
      )
      .collect();
    self.browser.surface.wait(&channels, remaining)?;

    Ok(Closed::No)
  }

  fn remember(&mut self) -> core::result::Result<(), console_core_never::Never> {
    {
      let browser = &self.browser;
      let tabs = std::iter::once(&browser.active)
        .chain(&browser.background)
        .map(|tab| {
          let address = match tab.opener.as_deref() {
            Some(opener) => opener,
            None => &tab.address,
          };
          (tab.id, tab.renderer, address)
        })
        .collect();

      let text = {
        let Ok(optional_text) =
          crate::session::encode(browser.default_engine, browser.active.id, tabs);

        match optional_text {
          Some(matched_text) => matched_text,
          ::core::option::Option::None => return Ok(()),
        }
      };

      match { ::core::primitive::bool::then_some(self.remembered.as_ref() == Some(&text), ()) } {
        ::core::option::Option::Some(()) => {
          return Ok(());
        }
        ::core::option::Option::None => {}
      }

      let mut arguments = vec![RESUME.spelling, &text];

      for (flag, worker) in [
        (SERVO_WORKER, &browser.programs.servo),
        (WEBKIT_WORKER, &browser.programs.webkit),
      ] {
        {
          match worker.as_ref().and_then(|worker_2| worker_2.to_str()) {
            Some(path) => {
              arguments.extend([flag.spelling, path]);
            }
            ::core::option::Option::None => {}
          };
        }
      }

      match console_core_places::session::publish(env!("CARGO_BIN_NAME"), &arguments) {
        Ok(()) => self.remembered = Some(text),
        Err(fault) => {
          console_core_logging::warn!(
            "console-application-browser: saving the open tabs: {fault}"
          )
        }
      }
    };

    Ok(())
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
  retry_with: &'static [Renderer],
) -> Result<(), Box<dyn Error>> {
  let initial = browser.active.renderer;
  let launched = EngineStart::open(
    &engine_start::Launch {
      first: initial,
      retry_with,
    },
    &mut browser,
    |window, effect| {
      match effect.previous_failure {
        Some((previous, fault)) => {
          let renderer = effect.renderer;
          console_core_logging::warn!("could not start {previous}, opening {renderer}: {fault}");
        }
        None => {}
      }

      window.switch(effect.renderer)
    },
  )?;

  match (launched != initial).then_some(()) {
    Some(()) => browser.default_engine = launched,
    None => {}
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
    remembered: None,
  }) {
    step?;
  }

  Ok(())
}

struct RendererOverrides<'a> {
  chromium: Option<&'a str>,
  servo: Option<&'a str>,
  webkit: Option<&'a str>,
}

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  expect(
    explicit026_env_read_once,
    reason = "the Browser reads this deployment override once when it discovers its renderer executables"
  )
)]
fn staged_servo_worker() -> Result<Option<PathBuf>, Never> {
  Ok(std::env::var_os("CONSOLE_BROWSER_SERVO_WORKER").map(PathBuf::from))
}

impl RendererPrograms {
  fn discover(overrides: RendererOverrides<'_>) -> io::Result<Self> {
    let executable = std::env::current_exe()?;
    let Ok(staged_servo) = staged_servo_worker();
    let candidates = [
      (
        overrides.chromium,
        "console-browser-renderer-chromium",
        None,
      ),
      (
        overrides.servo,
        "console-browser-renderer-servo",
        staged_servo.as_deref(),
      ),
      (overrides.webkit, "console-browser-renderer-webkit", None),
    ];
    let paths =
      console_concurrency::map_with_threads(3, &candidates, |&(requested, name, staged)| {
        match requested {
          Some(path) => Some(PathBuf::from(path)),
          None => staged
            .and_then(|path| {
              let Ok(found) = available_worker(path.to_owned());
              found
            })
            .or_else(|| {
              let Ok(nearby) = nearby_worker(&executable, name);
              nearby
            }),
        }
      })
      .map_err(|_worker_panicked| io::Error::other("browser worker discovery failed"))?;
    let [chromium, servo, webkit] = <[Option<PathBuf>; 3]>::try_from(paths)
      .map_err(|_| io::Error::other("incomplete browser worker discovery"))?;

    Ok(Self {
      chromium,
      servo,
      webkit,
      private: PrivateChromium::default(),
    })
  }

  fn initial_engine(&self, requested: Option<&str>) -> io::Result<Renderer> {
    match requested {
      Some("servo") => Ok(Renderer::Servo),
      Some("chromium") => Ok(Renderer::Chromium),
      Some("webkit") => Ok(Renderer::WebKit),
      Some(_) => Err(invalid("use --engine servo, chromium, or webkit")),
      None => self.preferred(),
    }
  }
}

struct BrowserLaunch<'a> {
  address: String,
  restored: Option<crate::session::Window>,
  engine: Option<&'a str>,
  test: Option<Renderer>,
  workers: RendererOverrides<'a>,
}

struct OpenTabs {
  active: Tab,
  background: Vec<Tab>,
  next_tab: u32,
}

impl OpenTabs {
  fn restore(
    window: crate::session::Window,
    programs: &RendererPrograms,
    default_engine: Renderer,
  ) -> io::Result<Self> {
    let Ok(next_tab) =
      console_core_number_conversion::fitted::<_, u32>(window.tabs.len().saturating_add(1));
    let mut background = Vec::new();
    let mut selected = None;

    for (position, (tab, id)) in (0u64..).zip(window.tabs.into_iter().zip(1..)) {
      let Ok(program) = programs.path(tab.renderer);
      let renderer = match program {
        Some(_) => tab.renderer,
        None => default_engine,
      };
      let Ok(restored) = Tab::new(id, renderer, tab.address);

      match (position == window.active).then_some(()) {
        Some(()) => selected = Some(restored),
        None => background.push(restored),
      }
    }

    let active = selected.ok_or_else(|| invalid("saved window has no active tab"))?;
    Ok(Self {
      active,
      background,
      next_tab,
    })
  }

  fn single(address: String, renderer: Renderer) -> Result<Self, Never> {
    let Ok(active) = Tab::new(1, renderer, address);
    Ok(Self {
      active,
      background: Vec::new(),
      next_tab: 2,
    })
  }
}

fn start_browser(
  BrowserLaunch {
    address,
    restored,
    engine,
    test,
    workers,
  }: BrowserLaunch<'_>,
) -> Result<(), Box<dyn Error>> {
  let programs = RendererPrograms::discover(workers)?;
  let chosen = programs.initial_engine(engine)?;
  let Ok(available) = programs.available();
  let saved_engine = restored
    .as_ref()
    .map(|window| window.default_engine)
    .filter(|renderer| available.contains(renderer));
  let default_engine = match saved_engine {
    Some(saved) => saved,
    None => chosen,
  };
  let _executable = programs.for_renderer(default_engine)?;

  let tabs = match (restored, test) {
    (_, Some(checking)) => {
      let _worker = programs.for_renderer(checking)?;
      let Ok(tabs) = OpenTabs::single(address, Renderer::Chromium);
      tabs
    }
    (Some(window), None) => OpenTabs::restore(window, &programs, default_engine)?,
    (None, None) => {
      let Ok(tabs) = OpenTabs::single(address, default_engine);
      tabs
    }
  };
  let browser = open_window(programs, default_engine, tabs)?;
  let retry_with = match (engine, test) {
    (None, None) => available,
    _ => &[],
  };

  show(browser, test, retry_with)
}

fn open_window(
  programs: RendererPrograms,
  default_engine: Renderer,
  OpenTabs {
    active,
    background,
    next_tab,
  }: OpenTabs,
) -> Result<Browser, Box<dyn Error>> {
  let Ok(available) = programs.available();
  let mut toolbar = Toolbar::with_renderers(available)?;
  let Ok(_) = toolbar.address(&active.address);
  let mut surface = Surface::connect()?;
  surface.show_window_sized(
    &Window {
      app_id: APP_ID.to_owned(),
      title: TITLE.to_owned(),
    },
    WINDOW,
  )?;
  let Ok(download_refresh) = DownloadRefresh::initial_transition(&(), None);
  let Ok(desktop_font) = crate::reader::desktop_font();
  Ok(Browser {
    surface,
    toolbar,
    default_engine,
    active,
    programs,
    background,
    next_tab,
    desktop_font,
    viewport: Size {
      width: WINDOW.width,
      height: WINDOW.height.saturating_sub(toolbar::HEIGHT),
    },
    clipboard: String::new(),
    prompt: None,
    file_picker: None,
    download_notice: None,
    download_view: download_view::View::default(),
    download_scanner: None,
    download_refresh: download_refresh.state,
    bar: ToolbarFrame {
      visible: None,
      spare: Vec::new(),
      redraw: Redraw::Requested,
    },
    backdrop: Backdrop::Combined,
  })
}

pub fn run(arguments: Vec<OsString>) -> Result<ExitCode, Never> {
  let args = match launch_arguments(arguments) {
    Ok(args) => args,
    Err(argument) => {
      console_core_logging::error!(
        "console-application-browser: expected a UTF-8 address or local file path, received {argument:?}"
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
  let Ok(saved) = line.value(RESUME);
  let saved_2 = saved
    .and_then(|explicit_argument_0| {
      let Ok(parse) = crate::session::Window::parse(explicit_argument_0);

      parse
    })
    .filter(|_| operands.is_empty());

  let typed = match operands.first() {
    Some(typed) => typed.as_str(),
    None => BLANK,
  };
  let Ok(address) = resolve(typed);
  let address_3 = match address {
    Some(address_2) => address_2,
    None => {
      eprintln!(
        "console-application-browser: enter a web address, local file path, or search term"
      );

      return Ok(ExitCode::FAILURE);
    }
  };

  let Ok(engine) = line.value(ENGINE);
  let Ok(servo_worker) = line.value(SERVO_WORKER);
  let Ok(chromium_worker) = line.value(CHROMIUM_WORKER);
  let Ok(webkit_worker) = line.value(WEBKIT_WORKER);
  let Ok(smoke) = line.presence(SMOKE);
  let Ok(servo_test) = line.presence(SERVO_TEST);
  let Ok(webkit_test) = line.presence(WEBKIT_TEST);
  let test = match (webkit_test, servo_test, smoke) {
    (Presence::Present, _, _) => Some(Renderer::WebKit),
    (_, Presence::Present, _) => Some(Renderer::Servo),
    (_, _, Presence::Present) => Some(Renderer::Chromium),
    (Presence::Absent, Presence::Absent, Presence::Absent) => None,
  };
  let result = start_browser(BrowserLaunch {
    address: address_3,
    restored: saved_2,
    engine,
    test,
    workers: RendererOverrides {
      chromium: chromium_worker,
      servo: servo_worker,
      webkit: webkit_worker,
    },
  });
  let Ok(()) = console_core_places::session::forget();
  Ok(match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("console-application-browser: {error}");
      ExitCode::FAILURE
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::os::unix::ffi::OsStringExt;

  #[test]
  fn nearby_window_sizes_share_a_viewport_and_keep_the_page_proportions() {
    let first = Size {
      width: 1100,
      height: 675,
    };
    let second = Size {
      width: 1199,
      height: 600,
    };
    let Ok(content) = content_viewport(first);
    assert_eq!(
      content,
      Size {
        width: 1000,
        height: 600
      }
    );
    assert_eq!(content_viewport(second), Ok(content));

    let Ok(tall) = fitted_page(first, content);
    assert_eq!(
      tall.size,
      Size {
        width: 1100,
        height: 660
      }
    );
    assert_eq!(tall.at, Point { x: 0, y: 7 });

    let Ok(wide) = fitted_page(second, content);
    assert_eq!(wide.size, content);
    assert_eq!(wide.at, Point { x: 99, y: 0 });
  }

  #[test]
  fn installed_alternatives_are_preferred_over_chromium() {
    let mut programs = RendererPrograms {
      chromium: Some(PathBuf::from("chromium-worker")),
      servo: None,
      webkit: None,
      private: PrivateChromium::default(),
    };
    assert!(matches!(programs.preferred(), Ok(Renderer::Chromium)));
    programs.webkit = Some(PathBuf::from("webkit-worker"));
    assert!(matches!(programs.preferred(), Ok(Renderer::WebKit)));
    assert!(matches!(
      programs.initial_engine(Some("chromium")),
      Ok(Renderer::Chromium)
    ));
    programs.servo = Some(PathBuf::from("servo-worker"));
    assert!(matches!(programs.preferred(), Ok(Renderer::Servo)));
    programs.chromium = None;
    assert!(matches!(programs.preferred(), Ok(Renderer::Servo)));
    programs.servo = None;
    assert!(matches!(programs.preferred(), Ok(Renderer::WebKit)));
  }

  #[test]
  fn page_keys_carry_logical_input() {
    assert_eq!(
      ({
        let Ok(explicit_value) = page_key(Keysym::KP_Enter);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = page_key(Keysym::Return);
        explicit_value
      })
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = page_key(Keysym::XF86Launch9);
        explicit_value
      }),
      None
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = page_key(Keysym::F9);
        explicit_value
      }),
      None
    );
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
