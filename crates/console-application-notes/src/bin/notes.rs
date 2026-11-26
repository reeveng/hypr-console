//! The notes, and the note open on them.
//!
//!     notes
//!     notes ~/Notes/Groceries.md
//!
//! The notes are a list the way the files are one: New Note at the top and
//! every note under it, newest first, each with how it begins under its title.
//! The d-pad walks it, A opens what it is standing on, Y offers to put a note
//! in the trash, and B closes the app.
//!
//! A note open is a page and little else, set the way a book is: the same
//! column, the same face and size, on the paper and in the ink chosen for the
//! books, because a note is a page somebody is writing rather than a form being
//! filled in. Its first line is its title, as it is in Apple's Notes, so a new
//! note is a page with the caret on it and nothing to name first. The arrows
//! walk the caret, Page Up and Page Down turn a page of rows, and a finger puts
//! the caret where it lands. Writing past the foot of the page turns it, and
//! the number at the foot says which page this is of how many. Every change is
//! kept as it is made, and B goes back to the list, where the note is named
//! after its title. What that means on the disk is `console_application_notes::folder`'s.
//!
//! Diagrams opens a preview of the note's fenced Mermaid blocks. F5 or Y
//! switches between preview and writing; arrows and shoulders walk the
//! diagrams, and B returns to writing. A finger can use the header and the
//! Previous and Next buttons. The source stays in the note, so another editor
//! can read and change it too.
//!
//! It is an app, and holds the lock an app holds: asked for again it is
//! brought forward, and asked for a note while it is up, the one up stands down
//! for it. It was a panel once, which is how a note came to be a list of
//! one-line questions: a panel's question is one line long, and that was the
//! only text a panel could take.

use std::cmp::Ordering;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

use console_application_books::appearance::{self, Appearance, READING_EDGE};
use console_application_books::pages::Style;
use console_application_notes::diagrams::{self, DiagramError};
use console_application_notes::folder::{self, Destination, Leave, Note, OnDisk, Record};
use console_application_notes::pages;
use console_core_arguments::{
  Command, CommandLine, Flag, NoSubcommand, Operands, Presence, Takes, read,
};
use console_core_atomic_writes::read_text;
use console_core_color::Oklch;
use console_core_color::palette::{Wearing, WearingError};
use console_core_fonts::{EM, LETTERS};
use console_core_geometry::{Point, Size};
use console_core_internal_programs::InternalProgram;
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_i32, whole_u32};
use console_input_text::area::row_at;
use console_input_text::{Row, TextArea, Typed, Way, between};
use console_panel::keys::{Driving, Meaning, meaning};
use console_panel::ui::canvas::{
  Area, AreaLayout, Canvas, Interaction, ROW_RADIUS, RowTone, Style as CanvasStyle,
};
use console_panel::ui::painting::Run;
use console_panel::ui::shapes::{Edge, Font, Panel, Picture, Pixels, Round, Shape, Text, Weight};
use console_panel::ui::surface::standing::{Closed, PointerEvent, SurfaceError, Window};
use console_panel::ui::surface::{Activation, KeyPress, Keysym, Surface};
use console_panel::ui::{list, strip, theme};
use console_panel::window::{self, Responder, Update, handle_input, merge};

const TITLE: &str = "Notes";

const NEW_NOTE: &str = "New Note";

const PLACEHOLDER: &str = "Title";

const BACK: &str = "\u{2039}  Notes";

const TRASHED: &str = "It goes to the trash";

const ROW: u32 = (EM * 4).unsigned_abs();

const HEADER: u32 = strip::CONTENT_TOP.unsigned_abs();

const WORDS: u32 = EM.unsigned_abs();

const SMALL: u32 = (EM * 5 / 6).unsigned_abs();

const INSIDE: u32 = (EM * 2 / 3).unsigned_abs();

const CARET: u32 = 2;

const MEASURING: u32 = 4096;

const CARET_AT: Flag = Flag {
  spelling: "--caret",
  takes: Takes::Value("POSITION"),
  about: "reopen the note with the caret at this character",
};

const AFTER_RENAME: Flag = Flag {
  spelling: "--renamed-to",
  takes: Takes::Value("PATH"),
  about: "look here if the previous filename is gone",
};

const DRAFT: Flag = Flag {
  spelling: "--draft",
  takes: Takes::None,
  about: "open an empty draft",
};
const SELECTED: Flag = Flag {
  spelling: "--selected-note",
  takes: Takes::Value("PATH"),
  about: "open the list with this note selected",
};
const DIAGRAM: Flag = Flag {
  spelling: "--diagram",
  takes: Takes::Value("INDEX"),
  about: "open the diagram preview at this item",
};

const COMMAND: Command = Command {
  name: env!("CARGO_BIN_NAME"),
  about: "the notes, and the note open on them; a note's first line is its title",
  flags: &[CARET_AT, AFTER_RENAME, DRAFT, SELECTED, DIAGRAM],
  operands: Operands::Optional("NOTE"),
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prompt {
  None,
  Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
  List,
  Page,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Choice {
  Stay(Update),
  Open(Option<PathBuf>),
  Exit,
}

#[derive(Clone, Copy)]
enum PageAction {
  Back,
  ToggleDiagrams,
  Step(i32),
}

struct List {
  folder: PathBuf,
  notes: Vec<Note>,
  selected: u32,
  first: u32,
  prompt: Prompt,
  pressed: Option<u32>,
}

#[derive(Clone, Copy)]
struct ListPosition {
  selected: u32,
  first: u32,
}

struct OpenNote {
  at: Option<PathBuf>,
  known: String,
  area: TextArea,
  mode: PageMode,
  list_position: ListPosition,
}

enum PageMode {
  Writing,
  Diagrams(Preview),
}

struct Preview {
  sources: Vec<String>,
  selected: u32,
  image: Result<Pixels, DiagramError>,
}

impl Preview {
  fn of(text: &str) -> Result<Preview, Never> {
    let Ok(sources) = diagrams::sources(text);
    let mut preview = Preview {
      sources,
      selected: 0,
      image: Err(DiagramError::NoDiagram),
    };
    let Ok(()) = preview.render();

    Ok(preview)
  }

  fn render(&mut self) -> Result<(), Never> {
    let Ok(at) = console_core_number_conversion::index(self.selected);

    self.image = match self.sources.get(at) {
      Some(source) => diagrams::drawn(
        source,
        Size {
          width: 2048,
          height: 1536,
        },
      ),
      None => Err(DiagramError::NoDiagram),
    };

    Ok(())
  }

  fn step(&mut self, by: i32) -> Result<Update, Never> {
    let Ok(count) = fitted::<_, u32>(self.sources.len());
    let selected = self
      .selected
      .saturating_add_signed(by)
      .min(count.saturating_sub(1));

    match ::core::primitive::bool::then_some(selected == self.selected, ()) {
      ::core::option::Option::Some(()) => Ok(Update::None),
      ::core::option::Option::None => {
        self.selected = selected;

        let Ok(()) = self.render();

        Ok(Update::Redraw)
      }
    }
  }
}

enum View {
  List(List),
  Page(Box<OpenNote>),
}

struct App {
  folder: PathBuf,
  wearing: Wearing,
  appearance: Appearance,
  view: View,
}

#[derive(Debug)]
enum NotesError {
  NoHome,
  Palette(WearingError),
  Surface(SurfaceError),
}

impl std::fmt::Display for NotesError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      NotesError::NoHome => write!(to, "HOME is not set, so there is no Notes folder"),
      NotesError::Palette(fault) => write!(to, "{fault}"),
      NotesError::Surface(fault) => write!(to, "{fault}"),
    }
  }
}

impl std::error::Error for NotesError {}

fn main() -> ExitCode {
  let arguments: Vec<String> = std::env::args().skip(1).collect();
  let Ok(code) = window::open(&arguments, InternalProgram::Notes, &COMMAND, run);
  let Ok(()) = console_core_places::session::forget();

  code
}

fn run(arguments: &[String]) -> Result<(), NotesError> {
  let Ok(home) = console_core_places::home();
  let home_2 = home.ok_or(NotesError::NoHome)?;
  let wearing = Wearing::worn().map_err(NotesError::Palette)?;
  let Ok(appearance) = Appearance::current();
  let folder = home_2.join(folder::FOLDER);
  let Ok(list) = List::of(&folder, None);
  let mut app = App {
    folder,
    wearing,
    appearance,
    view: View::List(list),
  };

  match read(&COMMAND, arguments) {
    Ok(line) => {
      let Ok(()) = app.restore(&line);
    }
    Err(fault) => console_core_logging::warn!("notes: restoring launch arguments: {fault}"),
  }

  let mut surface = Surface::connect().map_err(NotesError::Surface)?;
  let Ok(named) = InternalProgram::Notes.name();
  let window = Window {
    app_id: named.to_string(),
    title: TITLE.to_string(),
  };

  surface.show_window(&window).map_err(NotesError::Surface)?;

  let ended = event_loop(&mut app, &mut surface);
  let Ok(_left) = app.leave_page();

  ended
}

fn session_argument<T: FromStr>(
  line: &CommandLine<NoSubcommand>,
  flag: Flag,
) -> Result<Option<T>, Never> {
  match line.parsed(flag) {
    Ok(value) => Ok(value),
    Err(fault) => {
      console_core_logging::warn!("notes: restoring {}: {fault}", flag.spelling);
      Ok(None)
    }
  }
}

fn session_path(path: &Path) -> Result<Option<String>, Never> {
  match std::path::absolute(path) {
    Ok(at) => Ok(at.to_str().map(str::to_owned)),
    Err(fault) => {
      console_core_logging::warn!("notes: saving {}: {fault}", path.display());
      Ok(None)
    }
  }
}

fn event_loop(app: &mut App, surface: &mut Surface) -> Result<(), NotesError> {
  let looked = iterate(
    (app, surface, Update::Redraw, None::<Size<u32>>, None),
    |(app_2, surface_2, update, drawn_at, mut reported)| {
      let Ok(location) = app_2.location();
      let arguments = match location {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => ::std::vec::Vec::new(),
      };

      match { ::core::primitive::bool::then_some(reported.as_ref() != Some(&arguments), ()) } {
        ::core::option::Option::Some(()) => {
          let words: Vec<_> = arguments.iter().map(String::as_str).collect();

          match console_core_places::session::publish(COMMAND.name, &words) {
            Ok(()) => reported = Some(arguments),
            Err(fault) => console_core_logging::warn!("notes: saving the open document: {fault}"),
          }
        }
        ::core::option::Option::None => {}
      }

      let drawn_at_2 = match update {
        Update::Redraw => {
          let Ok(()) = draw(app_2, surface_2);
          let Ok(room) = logical_size(surface_2);

          Some(room)
        }
        Update::None => drawn_at,
        Update::Exit => return Ok(Step::Halt(Ok(()))),
      };

      match surface_2.wait(&[], None) {
        Ok(()) => {}
        Err(fault) => return Ok(Step::Halt(Err(NotesError::Surface(fault)))),
      }

      let Ok(closed) = surface_2.closed();

      match closed {
        Closed::Yes => return Ok(Step::Halt(Ok(()))),
        Closed::No => {}
      }

      let Ok(room) = logical_size(surface_2);
      let Ok(input) = handle_input(app_2, surface_2, room);

      let resized = match ::core::primitive::bool::then_some(drawn_at_2 == Some(room), ()) {
        ::core::option::Option::Some(()) => Update::None,
        ::core::option::Option::None => Update::Redraw,
      };

      let Ok(update_2) = merge(input, resized);

      Ok(Step::Again((
        app_2, surface_2, update_2, drawn_at_2, reported,
      )))
    },
  );

  match looked {
    Ok(looked_2) => looked_2,
    Err(Endless) => Ok(()),
  }
}

fn logical_size(surface: &Surface) -> Result<Size<u32>, Never> {
  let Ok(logical) = surface.logical();

  Ok(match logical {
    Some(logical_2) => logical_2,
    None => Size {
      width: 1280,
      height: 800,
    },
  })
}

fn point_of(at: (f64, f64)) -> Result<Point<i32>, Never> {
  let Ok(across) = toward_zero_i32(at.0);
  let Ok(down) = toward_zero_i32(at.1);

  Ok(Point { x: across, y: down })
}

fn show_keyboard() -> Result<(), Never> {
  let Ok(mut command) = InternalProgram::KeyboardShow.command();

  command.stdout(std::process::Stdio::null());

  match console_program_lifetime::let_go(&mut command) {
    Ok(_let_go) => {}
    Err(fault) => eprintln!("notes: the keyboard would not come up: {fault}"),
  }

  Ok(())
}

fn hide_keyboard() -> Result<(), Never> {
  use console_input_keyboard::remote::{self, Answer, Command};

  let Ok(answer) = remote::send(Command::Hide);

  match answer {
    Answer::Received | Answer::Sessionless => {}
    Answer::Failed { at, why } => eprintln!(
      "notes: the keyboard could not be hidden at {}: {why}",
      at.display()
    ),
  }

  Ok(())
}

impl App {
  fn screen(&self) -> Result<Screen, Never> {
    Ok(match self.view {
      View::List(_) => Screen::List,
      View::Page(_) => Screen::Page,
    })
  }

  fn list_key(&mut self, key: Keysym, room: Size<u32>) -> Result<Update, Never> {
    let Ok(meant) = meaning(key, Driving::Panel);

    self.on_list(room, |list, shown| list.meant(meant, shown))
  }

  fn list_pointer(&mut self, event: PointerEvent, room: Size<u32>) -> Result<Update, Never> {
    match event {
      PointerEvent::Down { at } => {
        let Ok(width) = fitted::<u32, i32>(room.width);
        let Ok(header) = strip::window_area(width);
        let Ok(close) = strip::close_area(header);

        match close.contains_point(Point::from(at)) {
          Ok(console_core_geometry::Containment::Inside) => return Ok(Update::Exit),
          Ok(console_core_geometry::Containment::Outside) => {}
        }
      }
      PointerEvent::Moved { .. }
      | PointerEvent::Scrolled { .. }
      | PointerEvent::Pinched { .. }
      | PointerEvent::Up
      | PointerEvent::Left => {}
    }

    self.on_list(room, |list, shown| list.pointed(event, shown))
  }

  fn on_list(
    &mut self,
    room: Size<u32>,
    answer: impl FnOnce(&mut List, Visible) -> Result<Choice, Never>,
  ) -> Result<Update, Never> {
    let Ok(shown) = shown(room);

    let choice = match &mut self.view {
      View::List(list) => answer(list, shown),
      View::Page(_) => return Ok(Update::None),
    };

    let Ok(choice_2) = choice;

    self.choose(choice_2)
  }

  fn choose(&mut self, choice: Choice) -> Result<Update, Never> {
    match choice {
      Choice::Stay(update) => Ok(update),
      Choice::Open(at) => self.open(at),
      Choice::Exit => Ok(Update::Exit),
    }
  }

  fn restore(&mut self, line: &CommandLine<NoSubcommand>) -> Result<(), Never> {
    let Ok(operands) = line.operands();
    let Ok(draft) = line.presence(DRAFT);
    let Ok(caret) = session_argument::<u32>(line, CARET_AT);

    match (operands.first(), draft) {
      (Some(path), _) => {
        let Ok(renamed) = session_argument::<PathBuf>(line, AFTER_RENAME);
        let Ok(()) = self.restore_path(PathBuf::from(path), renamed, caret);
      }
      (None, Presence::Present) => {
        let Ok(_opened) = self.open(None);
      }
      (None, Presence::Absent) => {
        let Ok(selected) = session_argument::<PathBuf>(line, SELECTED);

        match selected {
          Some(path) => {
            let Ok(list) = List::of(&self.folder, Some(&path));
            self.view = View::List(list);
          }
          None => {}
        }
      }
    }

    let page = match &mut self.view {
      View::Page(page) => page,
      View::List(_) => return Ok(()),
    };

    match caret {
      Some(position) => {
        let Ok(_moved) = page.area.placed(position);
      }
      None => {}
    }

    let Ok(diagram) = session_argument::<u32>(line, DIAGRAM);

    match diagram {
      Some(position) => {
        let Ok(_previewed) = page.toggle_diagrams();

        match &mut page.mode {
          PageMode::Diagrams(preview) => {
            let Ok(count) = fitted::<_, u32>(preview.sources.len());
            preview.selected = position.min(count.saturating_sub(1));
            let Ok(()) = preview.render();
          }
          PageMode::Writing => {}
        }
      }
      None => {}
    }

    Ok(())
  }

  fn restore_path(
    &mut self,
    at: PathBuf,
    renamed: Option<PathBuf>,
    caret: Option<u32>,
  ) -> Result<(), Never> {
    let fault = match read_text(&at) {
      Ok(file) => return self.show_note(Some(at), file),
      Err(cause) => cause,
    };

    match (fault.kind(), renamed) {
      (io::ErrorKind::NotFound, Some(candidate)) => match read_text(&candidate) {
        Ok(file) => return self.show_note(Some(candidate), file),
        Err(read_fault) => {
          let absent = matches!(
            read_fault.kind(),
            io::ErrorKind::NotFound | io::ErrorKind::IsADirectory
          );

          match absent.then_some(()) {
            Some(()) => {}
            None => return report_open_error(&candidate, &read_fault),
          }
        }
      },
      _ => {}
    }

    match (fault.kind(), caret) {
      (io::ErrorKind::NotFound, None) => self.show_note(Some(at), String::new()),
      (io::ErrorKind::NotFound | io::ErrorKind::IsADirectory, Some(_)) => Ok(()),
      _ => report_open_error(&at, &fault),
    }
  }

  fn location(&self) -> Result<Option<Vec<String>>, Never> {
    let page = match &self.view {
      View::Page(page) => page,
      View::List(list) => {
        let Ok(note) = list.note(list.selected);

        return match note {
          Some(selected) => {
            let Ok(path) = session_path(&selected.path);
            Ok(path.map(|selected_path| vec![SELECTED.spelling.to_owned(), selected_path]))
          }
          None => Ok(None),
        };
      }
    };
    let Ok(caret) = page.area.caret();
    let mut arguments = vec![CARET_AT.spelling.to_owned(), caret.to_string()];

    match &page.at {
      Some(path) => {
        let Ok(absolute) = session_path(path);
        let at = match absolute {
          Some(at) => at,
          None => return Ok(None),
        };

        let Ok(text) = page.area.text();
        let Ok(title) = folder::title(text);
        let Ok(rename) = self.rename_for_session(&title, Path::new(&at));

        match rename {
          Some(candidate) => arguments.extend([AFTER_RENAME.spelling.to_owned(), candidate]),
          None => {}
        }

        arguments.insert(0, at);
      }
      ::core::option::Option::None => {
        arguments.push(DRAFT.spelling.to_owned());
      }
    }

    match &page.mode {
      PageMode::Diagrams(preview) => {
        arguments.extend([DIAGRAM.spelling.to_owned(), preview.selected.to_string()]);
      }
      PageMode::Writing => {}
    }

    Ok(Some(arguments))
  }

  fn rename_for_session(&self, title: &str, current: &Path) -> Result<Option<String>, Never> {
    let Ok(renamed) = folder::path(&self.folder, title);
    let candidate = match renamed.to_str().filter(|name| Path::new(*name) != current) {
      Some(candidate) => candidate,
      None => return Ok(None),
    };

    match std::fs::symlink_metadata(&renamed) {
      Ok(_) => Ok(None),
      Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
        Some(()) => Ok(Some(candidate.to_owned())),
        None => {
          console_core_logging::warn!("notes: checking {}: {fault}", renamed.display());
          Ok(None)
        }
      },
    }
  }

  fn open(&mut self, at: Option<PathBuf>) -> Result<Update, Never> {
    let file = match at.as_deref() {
      Some(path) => match read_text(path) {
        Ok(file) => file,
        Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
          Some(()) => String::new(),
          None => {
            let Ok(()) = report_open_error(path, &fault);

            return Ok(Update::Redraw);
          }
        },
      },
      None => String::new(),
    };
    let Ok(()) = self.show_note(at, file);

    Ok(Update::Redraw)
  }

  fn show_note(&mut self, at: Option<PathBuf>, known: String) -> Result<(), Never> {
    let text = match &at {
      Some(path) => {
        let Ok(text) = folder::text(&known, path);

        text
      }
      None => String::new(),
    };
    let Ok(area) = TextArea::holding(&text);
    let list_position = match &self.view {
      View::List(list) => ListPosition {
        selected: list.selected,
        first: list.first,
      },
      View::Page(note) => note.list_position,
    };
    self.view = View::Page(Box::new(OpenNote {
      at,
      known,
      area,
      mode: PageMode::Writing,
      list_position,
    }));

    show_keyboard()
  }

  fn page_key(&mut self, press: KeyPress, room: Size<u32>) -> Result<Update, Never> {
    let key = press.key;

    match key {
      Keysym::F5 | Keysym::F18 | Keysym::XF86Launch9 => {
        return self.page_action(Activation::Focused, Some(PageAction::ToggleDiagrams));
      }
      _ => {}
    }

    match (&self.view, key) {
      (View::Page(writing), Keysym::Escape) => match writing.mode {
        PageMode::Writing => {
          return self.page_action(Activation::Focused, Some(PageAction::Back));
        }
        PageMode::Diagrams(_) => {
          return self.page_action(Activation::Focused, Some(PageAction::ToggleDiagrams));
        }
      },
      _ => {}
    }

    let appearance = self.appearance;

    let writing = match &mut self.view {
      View::Page(writing) => writing,
      View::List(_) => return Ok(Update::None),
    };

    match &mut writing.mode {
      PageMode::Diagrams(_) => {
        let action = match key {
          Keysym::Left | Keysym::Up | Keysym::Page_Up => Some(PageAction::Step(-1)),
          Keysym::Right | Keysym::Down | Keysym::Page_Down => Some(PageAction::Step(1)),
          Keysym::Return | Keysym::KP_Enter => Some(PageAction::ToggleDiagrams),
          _ => None,
        };

        return self.page_action(Activation::Focused, action);
      }
      PageMode::Writing => {}
    }

    let Ok(rows) = rows_of(&writing.area, appearance, room);

    let Ok(typed) = match key {
      Keysym::Page_Up | Keysym::Page_Down => {
        let Ok(by) = a_page(&writing.area, &rows, appearance, room);

        let way = match ::core::primitive::bool::then_some(key == Keysym::Page_Up, ()) {
          ::core::option::Option::Some(()) => Way::Up,
          ::core::option::Option::None => Way::Down,
        };

        writing.area.stepped(&rows, way, by)
      }
      _ => writing.area.typed_press(press, &rows),
    };

    match typed {
      Typed::Edited => {
        let Ok(()) = writing.kept(&self.folder);

        Ok(Update::Redraw)
      }
      Typed::Moved => Ok(Update::Redraw),
      Typed::None => Ok(Update::None),
    }
  }

  fn page_pointer(&mut self, event: PointerEvent, room: Size<u32>) -> Result<Update, Never> {
    match (&mut self.view, event) {
      (View::Page(writing), PointerEvent::Scrolled { by }) => match &mut writing.mode {
        PageMode::Diagrams(preview) => {
          let Ok(step) = toward_zero_i32(by.signum());

          return preview.step(step);
        }
        PageMode::Writing => {}
      },
      _ => {}
    }

    let at = match event {
      PointerEvent::Down { at } => at,
      PointerEvent::Moved { .. }
      | PointerEvent::Scrolled { .. }
      | PointerEvent::Pinched { .. }
      | PointerEvent::Up
      | PointerEvent::Left => return Ok(Update::None),
    };

    let Ok(at_2) = point_of(at);
    let Ok(edge) = fitted::<u32, i32>(READING_EDGE);

    match ::core::primitive::bool::then_some(at_2.y < edge, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(across) = fitted::<i32, u32>(at_2.x.max(0));

        let action =
          match ::core::primitive::bool::then_some(across >= room.width.saturating_div(2), ()) {
            ::core::option::Option::Some(()) => PageAction::ToggleDiagrams,
            ::core::option::Option::None => PageAction::Back,
          };

        return self.page_action(Activation::At(action), None);
      }
      ::core::option::Option::None => {}
    }

    let appearance = self.appearance;

    let writing = match &mut self.view {
      View::Page(writing) => writing,
      View::List(_) => return Ok(Update::None),
    };

    match &mut writing.mode {
      PageMode::Diagrams(_) => return self.diagram_tap(at_2, room),
      PageMode::Writing => {}
    }

    let Ok(caret) = caret_at(&writing.area, at_2, appearance, room);
    let Ok(_moved) = writing.area.placed(caret);
    let Ok(()) = show_keyboard();

    Ok(Update::Redraw)
  }

  fn diagram_tap(&mut self, at: Point<i32>, room: Size<u32>) -> Result<Update, Never> {
    let Ok(down) = fitted::<i32, u32>(at.y.max(0));
    let footer = room.height.saturating_sub(READING_EDGE);

    match down.cmp(&footer) {
      Ordering::Less => Ok(Update::None),
      Ordering::Equal | Ordering::Greater => {
        let Ok(across) = fitted::<i32, u32>(at.x.max(0));
        let step = match across.cmp(&room.width.saturating_div(2)) {
          Ordering::Less => -1,
          Ordering::Equal | Ordering::Greater => 1,
        };

        self.page_action(Activation::At(PageAction::Step(step)), None)
      }
    }
  }

  fn page_action(
    &mut self,
    activation: Activation<PageAction>,
    focused: Option<PageAction>,
  ) -> Result<Update, Never> {
    let Ok(action) = activation.resolve(|| focused, Some);

    match action {
      Some(PageAction::Back) => self.back(),
      Some(PageAction::ToggleDiagrams) => self.toggle_diagrams(),
      Some(PageAction::Step(by)) => match &mut self.view {
        View::Page(writing) => match &mut writing.mode {
          PageMode::Diagrams(preview) => preview.step(by),
          PageMode::Writing => Ok(Update::None),
        },
        View::List(_) => Ok(Update::None),
      },
      None => Ok(Update::None),
    }
  }

  fn toggle_diagrams(&mut self) -> Result<Update, Never> {
    match &mut self.view {
      View::Page(writing) => writing.toggle_diagrams(),
      View::List(_) => Ok(Update::None),
    }
  }

  fn back(&mut self) -> Result<Update, Never> {
    let position = match &self.view {
      View::Page(writing) => writing.list_position,
      View::List(_) => return Ok(Update::None),
    };
    let Ok(left) = self.leave_page();
    let Ok(mut list) = List::of(&self.folder, left.as_deref());
    let Ok(last) = list.last();

    match list.selected {
      0 => list.selected = position.selected.min(last),
      1.. => {}
    }

    list.first = position.first.min(last);

    self.view = View::List(list);

    Ok(Update::Redraw)
  }

  fn leave_page(&mut self) -> Result<Option<PathBuf>, Never> {
    let writing = match &self.view {
      View::Page(writing) => writing,
      View::List(_) => return Ok(None),
    };

    let at = match &writing.at {
      Some(at) => at.clone(),
      None => return Ok(None),
    };

    let Ok(text) = writing.area.text();

    let written = OnDisk {
      at: Some(&at),
      known: &writing.known,
    };

    Ok(match folder::leave(&self.folder, written, text) {
      Ok(Leave::Retain) => Some(at),
      Ok(Leave::Rename(to)) => Some(to),
      Ok(Leave::Remove) => None,
      Err(fault) => {
        eprintln!("notes: {fault}");

        Some(at)
      }
    })
  }
}

impl Responder for App {
  fn key_pressed(&mut self, key: Keysym, room: Size<u32>) -> Result<Update, Never> {
    let Ok(press) = KeyPress::unmodified(key);

    self.key_pressed_with_modifiers(press, room)
  }

  fn key_pressed_with_modifiers(
    &mut self,
    press: KeyPress,
    room: Size<u32>,
  ) -> Result<Update, Never> {
    let Ok(screen) = self.screen();

    match screen {
      Screen::List => self.list_key(press.key, room),
      Screen::Page => self.page_key(press, room),
    }
  }

  fn pointer_event(&mut self, event: PointerEvent, room: Size<u32>) -> Result<Update, Never> {
    let Ok(screen) = self.screen();

    match screen {
      Screen::List => self.list_pointer(event, room),
      Screen::Page => self.page_pointer(event, room),
    }
  }
}

impl OpenNote {
  fn toggle_diagrams(&mut self) -> Result<Update, Never> {
    self.mode = match self.mode {
      PageMode::Writing => {
        let Ok(text) = self.area.text();
        let Ok(preview) = Preview::of(text);
        let Ok(()) = hide_keyboard();

        PageMode::Diagrams(preview)
      }
      PageMode::Diagrams(_) => {
        let Ok(()) = show_keyboard();

        PageMode::Writing
      }
    };

    Ok(Update::Redraw)
  }

  fn kept(&mut self, folder: &Path) -> Result<(), Never> {
    let Ok(text) = self.area.text();

    match (self.at.as_ref(), text.trim().is_empty()) {
      (None, true) => return Ok(()),
      (None, false) | (Some(_), _) => {}
    }

    let written = OnDisk {
      at: self.at.as_deref(),
      known: &self.known,
    };

    match folder::save(folder, written, text) {
      Ok(Record {
        at,
        known,
        destination,
      }) => {
        match destination {
          Destination::Copy => {
            let message = format!(
              "This note changed somewhere else. Your copy was saved as {}.",
              at.display()
            );
            let Ok(()) = console_notifications::notify(
              console_notifications::Application("Notes"),
              "Saved a copy of the note",
              Some(&message),
            );
          }
          Destination::Original => {}
        }

        self.at = Some(at);
        self.known = known;
      }
      Err(fault) => {
        let Ok(()) = console_notifications::notify(
          console_notifications::Application("Notes"),
          "Could not save note",
          Some(&fault.to_string()),
        );
      }
    }

    Ok(())
  }
}

fn report_open_error(at: &Path, fault: &io::Error) -> Result<(), Never> {
  console_notifications::notify(
    console_notifications::Application("Notes"),
    "Could not open note",
    Some(&format!("{}: {fault}", at.display())),
  )
}

impl List {
  fn of(folder: &Path, standing: Option<&Path>) -> Result<List, Never> {
    let notes = match folder::listed(folder) {
      Ok(notes) => notes,
      Err(fault) => {
        let Ok(()) = console_notifications::notify(
          console_notifications::Application("Notes"),
          "Could not list notes",
          Some(&fault.to_string()),
        );

        Vec::new()
      }
    };

    let found = (1_u32..)
      .zip(&notes)
      .find(|(_, note)| Some(note.path.as_path()) == standing);

    let selected = match found {
      Some((at, _)) => at,
      None => 0,
    };

    Ok(List {
      folder: folder.to_path_buf(),
      notes,
      selected,
      first: 0,
      prompt: Prompt::None,
      pressed: None,
    })
  }

  fn last(&self) -> Result<u32, Never> {
    fitted::<_, u32>(self.notes.len())
  }

  fn note(&self, row: u32) -> Result<Option<&Note>, Never> {
    let Ok(at) = console_core_number_conversion::index(row.saturating_sub(1));

    Ok(match row {
      0 => None,
      _ => self.notes.get(at),
    })
  }

  fn meant(&mut self, meant: Meaning, shown: Visible) -> Result<Choice, Never> {
    Ok(match (meant, self.prompt) {
      (Meaning::Step(by), Prompt::None) => {
        let Ok(last) = self.last();
        let Ok(()) = self.stand_on(self.selected.saturating_add_signed(by).min(last), shown);

        Choice::Stay(Update::Redraw)
      }
      (Meaning::Choose, Prompt::None) => {
        let Ok(choice) = self.choice(Activation::Focused);

        choice
      }
      (Meaning::More, Prompt::None) => {
        let Ok(note) = self.note(self.selected);

        match note {
          Some(_) => {
            self.prompt = Prompt::Delete;

            Choice::Stay(Update::Redraw)
          }
          None => Choice::Stay(Update::None),
        }
      }
      (Meaning::Close | Meaning::Abandon, Prompt::None) => Choice::Exit,
      (Meaning::Choose, Prompt::Delete) => {
        let Ok(()) = self.trash();

        Choice::Stay(Update::Redraw)
      }
      (Meaning::Close | Meaning::Abandon, Prompt::Delete) => {
        self.prompt = Prompt::None;

        Choice::Stay(Update::Redraw)
      }
      (Meaning::None | Meaning::Nudge(_) | Meaning::Tab(_), Prompt::None)
      | (
        Meaning::Step(_) | Meaning::More | Meaning::None | Meaning::Nudge(_) | Meaning::Tab(_),
        Prompt::Delete,
      ) => Choice::Stay(Update::None),
    })
  }

  fn pointed(&mut self, event: PointerEvent, shown: Visible) -> Result<Choice, Never> {
    Ok(match event {
      PointerEvent::Down { at } => {
        let Ok(at_2) = point_of(at);
        let Ok(row) = self.row_under(at_2);

        self.pressed = row;

        match row {
          Some(row_2) => {
            self.prompt = Prompt::None;

            let Ok(()) = self.stand_on(row_2, shown);

            Choice::Stay(Update::Redraw)
          }
          None => Choice::Stay(Update::None),
        }
      }
      PointerEvent::Up => {
        let pressed = self.pressed.take();
        let Ok(choice) = self.choice(Activation::At(pressed));

        choice
      }
      PointerEvent::Scrolled { by } => {
        let Ok(last) = self.last();
        let Ok(step) = toward_zero_i32(by.signum());
        let Ok(()) = self.stand_on(self.selected.saturating_add_signed(step).min(last), shown);

        Choice::Stay(Update::Redraw)
      }
      PointerEvent::Left => {
        self.pressed = None;

        Choice::Stay(Update::None)
      }
      PointerEvent::Moved { .. } | PointerEvent::Pinched { .. } => Choice::Stay(Update::None),
    })
  }

  fn row_under(&self, at: Point<i32>) -> Result<Option<u32>, Never> {
    let Ok(down) = fitted::<i32, u32>(at.y.max(0));
    let Ok(last) = self.last();

    Ok(match down.checked_sub(HEADER) {
      Some(into) => {
        let row = self.first.saturating_add(into.saturating_div(ROW));

        match ::core::primitive::bool::then_some(row > last, ()) {
          ::core::option::Option::Some(()) => None,
          ::core::option::Option::None => Some(row),
        }
      }
      None => None,
    })
  }

  fn stand_on(&mut self, row: u32, shown: Visible) -> Result<(), Never> {
    let Visible(shown_2) = shown;

    self.selected = row;

    self.first = match (row < self.first, row >= self.first.saturating_add(shown_2)) {
      (true, _) => row,
      (false, true) => row.saturating_add(1).saturating_sub(shown_2),
      (false, false) => self.first,
    };

    Ok(())
  }

  fn choice(&self, activation: Activation<Option<u32>>) -> Result<Choice, Never> {
    let Ok(selected_row) = activation.resolve(
      || Some(self.selected),
      |pressed| pressed.filter(|row| *row == self.selected),
    );
    let row = match selected_row {
      Some(row) => row,
      None => return Ok(Choice::Stay(Update::None)),
    };

    let Ok(note) = self.note(row);

    Ok(Choice::Open(note.map(|note_2| note_2.path.clone())))
  }

  fn trash(&mut self) -> Result<(), Never> {
    let Ok(note) = self.note(self.selected);

    let at = match note {
      Some(note_2) => &note_2.path,
      None => return Ok(()),
    };

    match console_gio::trash(at) {
      Ok(()) => {}
      Err(fault) => {
        let Ok(()) = console_notifications::notify(
          console_notifications::Application("Notes"),
          "Could not trash note",
          Some(&fault.to_string()),
        );
      }
    }

    let Ok(listed) = List::of(&self.folder, None);
    let Ok(last) = listed.last();

    *self = List {
      selected: self.selected.min(last),
      first: self.first,
      ..listed
    };

    Ok(())
  }
}

#[cfg(test)]
mod navigation_tests {
  use super::*;

  #[test]
  fn back_from_a_note_restores_the_list_viewport() -> Result<(), Box<dyn std::error::Error>> {
    let folder = console_core_temporary_directories::fresh("notes-scroll")?;

    for row in 0_u32..8 {
      let Ok(at) = console_application_notes::folder::path(&folder, &format!("Note {row}"));
      console_core_atomic_writes::whole(&at, format!("# Note {row}\n").as_bytes())?;
    }

    let Ok(appearance) = Appearance::current();
    let Ok(area) = TextArea::holding("");
    let Ok(palette) = console_core_color::palette::read(include_str!(
      "../../../../files/usr/local/lib/console/palette.sh"
    ));
    let wearing = Wearing::out_of(&palette)?;
    let mut app = App {
      folder,
      wearing,
      appearance,
      view: View::Page(Box::new(OpenNote {
        at: None,
        known: String::new(),
        area,
        mode: PageMode::Writing,
        list_position: ListPosition {
          first: 3,
          selected: 5,
        },
      })),
    };
    let Ok(_) = app.back();
    let list = match app.view {
      View::List(list) => list,
      View::Page(_) => {
        return Err(Box::<dyn std::error::Error>::from(
          "Back did not return to the note list",
        ));
      }
    };

    assert_eq!((list.first, list.selected), (3, 5));
    Ok(())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Visible(u32);

fn shown(room: Size<u32>) -> Result<Visible, Never> {
  Ok(Visible(
    room
      .height
      .saturating_sub(HEADER)
      .saturating_div(ROW)
      .max(1),
  ))
}

fn style_of(paragraph: u32) -> Result<Style, Never> {
  Ok(match paragraph {
    0 => Style::Heading,
    _ => Style::Body,
  })
}

fn rows_of(area: &TextArea, appearance: Appearance, room: Size<u32>) -> Result<Vec<Row>, Never> {
  let Ok(column) = appearance::column(room);

  let wrap = |paragraph: u32, said: &str| -> Vec<String> {
    let Ok(style) = style_of(paragraph);
    let Ok((font, weight)) = appearance.styled(style);
    let Ok(lines) = console_panel::ui::painting::wrapped(
      Run {
        said,
        weight,
        width: column.width,
      },
      &font,
    );

    lines
  };

  area.rows(&wrap)
}

struct Heights {
  heading: u32,
  body: u32,
}

impl Heights {
  fn of(appearance: Appearance) -> Result<Heights, Never> {
    let Ok(heading) = appearance.line_height(Style::Heading);
    let Ok(body) = appearance.line_height(Style::Body);

    Ok(Heights { heading, body })
  }

  fn row(&self, row: &Row) -> Result<u32, Never> {
    Ok(match row.paragraph {
      0 => self.heading,
      _ => self.body,
    })
  }
}

fn text_height(room: Size<u32>) -> Result<u32, Never> {
  Ok(
    room
      .height
      .saturating_sub(READING_EDGE.saturating_mul(2))
      .saturating_sub(WORDS),
  )
}

struct Layout {
  pages: Vec<std::ops::Range<u32>>,
  heights: Vec<u32>,
  page: u32,
}

fn layout(
  area: &TextArea,
  rows: &[Row],
  appearance: Appearance,
  room: Size<u32>,
) -> Result<Layout, Never> {
  let Ok(sized) = Heights::of(appearance);

  let heights: Vec<u32> = rows
    .iter()
    .map(|row| {
      let Ok(tall) = sized.row(row);

      tall
    })
    .collect();

  let Ok(room_2) = text_height(room);
  let Ok(stacked) = pages::pages(&heights, room_2);
  let Ok(caret) = area.caret();
  let Ok(standing) = row_at(rows, caret);

  let Ok(page) = pages::holding(
    &stacked,
    match standing {
      Some((at, _)) => at,
      None => 0,
    },
  );

  Ok(Layout {
    pages: stacked,
    heights,
    page,
  })
}

fn a_page(
  area: &TextArea,
  rows: &[Row],
  appearance: Appearance,
  room: Size<u32>,
) -> Result<u32, Never> {
  let Ok(layout) = layout(area, rows, appearance, room);
  let Ok(at) = console_core_number_conversion::index(layout.page);

  Ok(match layout.pages.get(at) {
    Some(page) => page.end.saturating_sub(page.start).max(1),
    None => 1,
  })
}

struct Placement {
  row: Row,
  top: u32,
  tall: u32,
}

fn placed(layout: &Layout, rows: &[Row]) -> Result<Vec<Placement>, Never> {
  let Ok(at) = console_core_number_conversion::index(layout.page);

  let page = match layout.pages.get(at) {
    Some(page) => page.clone(),
    None => return Ok(Vec::new()),
  };

  let mut top = READING_EDGE;
  let mut found = Vec::new();

  for index in page {
    let Ok(at_2) = console_core_number_conversion::index(index);

    match (rows.get(at_2), layout.heights.get(at_2)) {
      (Some(row), Some(tall)) => {
        found.push(Placement {
          row: *row,
          top,
          tall: *tall,
        });
        top = top.saturating_add(*tall);
      }
      (Some(_) | None, Some(_) | None) => {}
    }
  }

  Ok(found)
}

fn width_of(said: &str, style: Style, appearance: Appearance) -> Result<u32, Never> {
  let Ok((font, weight)) = appearance.styled(style);
  let Ok(measured) = console_panel::ui::painting::measure_text(
    Run {
      said,
      weight,
      width: MEASURING,
    },
    &font,
  );

  Ok(measured.width)
}

fn small_width(said: &str) -> Result<u32, Never> {
  let font = Font {
    family: LETTERS.to_string(),
    height: SMALL,
  };
  let Ok(measured) = console_panel::ui::painting::measure_text(
    Run {
      said,
      weight: Weight::Plain,
      width: MEASURING,
    },
    &font,
  );

  Ok(measured.width)
}

fn caret_at(
  area: &TextArea,
  at: Point<i32>,
  appearance: Appearance,
  room: Size<u32>,
) -> Result<u32, Never> {
  let Ok(rows) = rows_of(area, appearance, room);
  let Ok(layout) = layout(area, &rows, appearance, room);
  let Ok(on_the_page) = placed(&layout, &rows);
  let Ok(down) = fitted::<i32, u32>(at.y.max(0));

  let nearest = on_the_page.iter().min_by_key(|placed| {
    let middle = placed.top.saturating_add(placed.tall.saturating_div(2));

    middle.abs_diff(down)
  });

  let placed = match nearest {
    Some(placed) => placed,
    None => return area.caret(),
  };

  let Ok(column) = appearance::column(room);
  let Ok(across) = fitted::<i32, u32>(at.x.max(0));
  let across_2 = across.saturating_sub(column.left);
  let Ok(text) = area.text();
  let Ok(style) = style_of(placed.row.paragraph);

  let fits = (placed.row.start..=placed.row.end).take_while(|end| {
    let Ok(before) = between(text, placed.row.start..*end);
    let Ok(wide) = width_of(before, style, appearance);

    wide <= across_2
  });

  Ok(match fits.last() {
    Some(end) => end,
    None => placed.row.start,
  })
}

fn draw(app: &App, surface: &mut Surface) -> Result<(), Never> {
  let Ok(room) = logical_size(surface);

  let Ok(shapes) = match &app.view {
    View::List(list) => list_shapes(list, &app.wearing, room),
    View::Page(writing) => page_shapes(writing, &app.wearing, app.appearance, room),
  };

  let Ok(painting) = console_panel::ui::painting::painter(room, &shapes, "notes");

  match surface.draw(painting) {
    Ok(()) => {}
    Err(fault) => eprintln!("notes: {fault}"),
  }

  Ok(())
}

fn background(room: Size<u32>, fill: Oklch) -> Result<Shape, Never> {
  Ok(Shape::Panel(Panel {
    at: Point { x: 0, y: 0 },
    size: room,
    round: Round(0),
    fill,
    edge: Edge::None,
  }))
}

fn words(
  said: &str,
  at: (u32, u32),
  (tall, weight): (u32, Weight),
  (width, ink): (u32, Oklch),
) -> Result<Shape, Never> {
  let Ok(x) = fitted::<u32, i32>(at.0);
  let Ok(y) = fitted::<u32, i32>(at.1);

  Ok(Shape::Text(Text {
    at: Point { x, y },
    width,
    said: said.to_string(),
    weight,
    font: Font {
      family: LETTERS.to_string(),
      height: tall,
    },
    ink,
  }))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ListControl {
  Close,
}

fn list_shapes(list: &List, wearing: &Wearing, room: Size<u32>) -> Result<Vec<Shape>, Never> {
  let Ok(fill) = theme::Surface::Window.color(wearing);
  let Ok(ground) = background(room, fill);
  let Ok(width) = fitted::<u32, i32>(room.width);
  let Ok(height) = fitted::<u32, i32>(room.height);
  let root = Area {
    origin: Point { x: 0, y: 0 },
    size: Size { width, height },
  };
  let Ok(mut canvas) = Canvas::new(
    root,
    CanvasStyle::from(wearing),
    Interaction {
      focus: None,
      hover: None,
    },
  );
  canvas.frame.shapes.push(ground);
  let Ok(header) = strip::window_area(width);
  let Ok(()) = strip::title_bar(&mut canvas, header, TITLE, ListControl::Close, wearing);
  let Ok(Visible(shown)) = shown(room);
  let Ok(shown_2) = console_core_number_conversion::index(shown);
  let Ok(last) = list.last();

  for (row, top) in (list.first..=last)
    .take(shown_2)
    .zip((0_u32..).map(|step| HEADER.saturating_add(step.saturating_mul(ROW))))
  {
    let Ok(y) = fitted::<u32, i32>(top);
    let area = Area {
      origin: Point { y, ..header.origin },
      size: Size { height: ROW.saturating_sub(strip::GAP.unsigned_abs()).cast_signed(), ..header.size },
    };
    let Ok(()) = note_row(&mut canvas, list, row, area, wearing);
  }

  Ok(canvas.frame.shapes)
}

fn note_row(
  canvas: &mut Canvas<ListControl>,
  list: &List,
  row: u32,
  area: Area,
  wearing: &Wearing,
) -> Result<(), Never> {
  let Ok(note) = list.note(row);
  let (title, opening) = match note {
    Some(record) => (record.title.as_str(), record.opening.as_str()),
    None => (NEW_NOTE, ""),
  };
  let tone = match ((row == list.selected).then_some(()), list.prompt) {
    (Some(()), Prompt::None) => RowTone::Focused,
    (Some(()), Prompt::Delete) => RowTone::Destructive,
    (None, _) => RowTone::Normal,
  };

  let (heading, detail) = match tone {
    RowTone::Destructive => (format!("Delete “{title}”?"), TRASHED),
    RowTone::Focused | RowTone::Normal => (title.to_owned(), opening),
  };

  let Ok(colors) = tone.colors(wearing);
  let Ok(()) = canvas.panel(area, colors.fill, ROW_RADIUS, None);
  let Ok(inset) = fitted::<u32, i32>(INSIDE);
  let text_area = Area {
    origin: Point {
      x: area.origin.x.saturating_add(inset),
      ..area.origin
    },
    size: Size {
      width: area.size.width.saturating_sub(inset.saturating_mul(2)),
      ..area.size
    },
  };
  list::label(
    canvas,
    text_area,
    list::Label {
      title: &heading,
      detail,
    },
    &colors,
  )
}

fn page_shapes(
  writing: &OpenNote,
  wearing: &Wearing,
  appearance: Appearance,
  room: Size<u32>,
) -> Result<Vec<Shape>, Never> {
  match &writing.mode {
    PageMode::Diagrams(preview) => return diagram_shapes(preview, wearing, appearance, room),
    PageMode::Writing => {}
  }

  let Ok(paper) = appearance.ground(wearing);
  let Ok(ink) = appearance.ink(wearing);
  let Ok(ground) = background(room, paper);
  let Ok(back) = words(
    BACK,
    (WORDS, WORDS),
    (SMALL, Weight::Plain),
    (MEASURING, ink),
  );
  let mut shapes = vec![ground, back];
  let Ok(button) = diagram_button("Diagrams · F5 / Y", ink, room);

  shapes.push(button);

  let Ok(text) = writing.area.text();
  let Ok(caret) = writing.area.caret();
  let Ok(rows) = rows_of(&writing.area, appearance, room);
  let Ok(layout) = layout(&writing.area, &rows, appearance, room);
  let Ok(on_the_page) = placed(&layout, &rows);
  let Ok(column) = appearance::column(room);
  let Ok(left) = fitted::<u32, i32>(column.left);
  let Ok(standing) = row_at(&rows, caret);

  for placed in &on_the_page {
    let Ok(style) = style_of(placed.row.paragraph);
    let Ok((font, weight)) = appearance.styled(style);
    let Ok(said) = between(text, placed.row.start..placed.row.end);
    let Ok(down) = fitted::<u32, i32>(placed.top);

    let (said_2, color) = match (text.is_empty(), placed.row.paragraph) {
      (true, 0) => (PLACEHOLDER, wearing.soft),
      (true, _) | (false, _) => (said, ink),
    };

    shapes.push(Shape::Text(Text {
      at: Point { x: left, y: down },
      width: column.width.saturating_add(READING_EDGE),
      said: said_2.to_string(),
      weight,
      font,
      ink: color,
    }));

    match ::core::primitive::bool::then_some(standing.map(|(_, row)| row) == Some(placed.row), ())
    {
      ::core::option::Option::Some(()) => {
        let Ok(before) = between(text, placed.row.start..caret);
        let Ok(across) = width_of(before, style, appearance);
        let Ok(across_2) = fitted::<u32, i32>(column.left.saturating_add(across));

        shapes.push(Shape::Panel(Panel {
          at: Point {
            x: across_2,
            y: down,
          },
          size: Size {
            width: CARET,
            height: placed.tall,
          },
          round: Round(0),
          fill: wearing.pink,
          edge: Edge::None,
        }));
      }
      ::core::option::Option::None => {}
    }
  }

  let Ok(count) = fitted::<_, u32>(layout.pages.len());

  match ::core::primitive::bool::then_some(count > 1, ()) {
    ::core::option::Option::Some(()) => {
      let folio = format!("{} of {count}", layout.page.saturating_add(1));
      let Ok(wide) = small_width(&folio);
      let across = room
        .width
        .saturating_sub(wide.min(room.width))
        .saturating_div(2);
      let foot = room
        .height
        .saturating_sub(READING_EDGE.saturating_div(2))
        .saturating_sub(SMALL);
      let Ok(number) = words(
        &folio,
        (across, foot),
        (SMALL, Weight::Plain),
        (MEASURING, ink),
      );

      shapes.push(number);
    }
    ::core::option::Option::None => {}
  }

  Ok(shapes)
}

fn diagram_button(label: &str, ink: Oklch, room: Size<u32>) -> Result<Shape, Never> {
  let Ok(wide) = small_width(label);
  let across = room
    .width
    .saturating_sub(wide)
    .saturating_sub(WORDS)
    .max(room.width.saturating_div(2));

  words(
    label,
    (across, WORDS),
    (SMALL, Weight::Plain),
    (room.width.saturating_sub(across), ink),
  )
}

fn diagram_shapes(
  preview: &Preview,
  wearing: &Wearing,
  appearance: Appearance,
  room: Size<u32>,
) -> Result<Vec<Shape>, Never> {
  let Ok(paper) = appearance.ground(wearing);
  let Ok(ink) = appearance.ink(wearing);
  let Ok(ground) = background(room, paper);
  let Ok(back) = words(
    BACK,
    (WORDS, WORDS),
    (SMALL, Weight::Plain),
    (MEASURING, ink),
  );
  let Ok(edit) = diagram_button("Edit · F5 / Y", ink, room);
  let mut shapes = vec![ground, back, edit];
  let Ok(column) = appearance::column(room);
  let available = room
    .height
    .saturating_sub(READING_EDGE.saturating_mul(2))
    .max(1);

  match &preview.image {
    Ok(pixels) => {
      let scale = (f64::from(column.width) / f64::from(pixels.width))
        .min(f64::from(available) / f64::from(pixels.height))
        .min(1.0);
      let Ok(width) = whole_u32(f64::from(pixels.width) * scale);
      let Ok(height) = whole_u32(f64::from(pixels.height) * scale);
      let Ok(x) = fitted::<u32, i32>(
        column
          .left
          .saturating_add(column.width.saturating_sub(width).saturating_div(2)),
      );
      let Ok(y) = fitted::<u32, i32>(
        READING_EDGE.saturating_add(available.saturating_sub(height).saturating_div(2)),
      );
      let at = Point { x, y };
      let size = Size {
        width: width.max(1),
        height: height.max(1),
      };

      shapes.push(Shape::Panel(Panel {
        at,
        size,
        round: Round(0),
        fill: Oklch::WHITE,
        edge: Edge::None,
      }));
      shapes.push(Shape::Picture(Picture {
        at,
        size,
        pixels: pixels.clone(),
      }));
    }
    Err(fault) => {
      let said = match ::core::primitive::bool::then_some(preview.sources.is_empty(), ()) {
                ::core::option::Option::Some(()) => "Add a Mermaid block to your note, then open Diagrams:\n\n```mermaid\nflowchart LR\n  Idea --> Note\n```".to_string(),
                ::core::option::Option::None => fault.to_string(),
            };
      let Ok(message) = words(
        &said,
        (column.left, READING_EDGE),
        (WORDS, Weight::Plain),
        (column.width, ink),
      );

      shapes.push(message);
    }
  }

  let Ok(count) = fitted::<_, u32>(preview.sources.len());

  match ::core::primitive::bool::then_some(count > 0, ()) {
    ::core::option::Option::Some(()) => {
      let folio = format!("{} of {count}", preview.selected.saturating_add(1));
      let Ok(wide) = small_width(&folio);
      let across = room.width.saturating_sub(wide).saturating_div(2);
      let foot = room
        .height
        .saturating_sub(READING_EDGE)
        .saturating_add(WORDS);
      let Ok(number) = words(&folio, (across, foot), (SMALL, Weight::Plain), (wide, ink));

      shapes.push(number);

      match ::core::primitive::bool::then_some(preview.selected > 0, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(previous) = words(
            "‹ Previous",
            (column.left, foot),
            (SMALL, Weight::Plain),
            (column.width.saturating_div(3), ink),
          );

          shapes.push(previous);
        }
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(preview.selected.saturating_add(1) < count, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(wide_2) = small_width("Next ›");
          let across_2 = column
            .left
            .saturating_add(column.width)
            .saturating_sub(wide_2);
          let Ok(next) = words(
            "Next ›",
            (across_2, foot),
            (SMALL, Weight::Plain),
            (wide_2, ink),
          );

          shapes.push(next);
        }
        ::core::option::Option::None => {}
      }
    }
    ::core::option::Option::None => {}
  }

  Ok(shapes)
}
