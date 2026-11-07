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
//! after its title. What that means on the disk is `console_notes::folder`'s.
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

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_books::appearance::{self, Appearance, READING_EDGE};
use console_books::pages::Style;
use console_core_arguments::{Command, Operands};
use console_core_color::Oklch;
use console_core_color::palette::{Wearing, WearingError};
use console_core_external_programs::Program;
use console_core_fonts::{EM, LETTERS};
use console_core_geometry::{Point, Size};
use console_core_internal_programs::InternalProgram;
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_i32, whole_u32};
use console_core_shapes::{Edge, Font, Panel, Picture, Pixels, Round, Shape, Text, Weight};
use console_draw_painting::Run;
use console_draw_surface::standing::{Closed, PointerEvent, SurfaceError, Window};
use console_draw_surface::{Keysym, Surface};
use console_input_text::area::row_at;
use console_input_text::{Row, TextArea, Typed, Way, between};
use console_notes::folder::{self, Destination, Leave, Note, OnDisk, Record};
use console_notes::pages;
use console_notes::diagrams::{self, DiagramError};
use console_panel::keys::{Driving, Meaning, meaning};
use console_panel::window::{self, Responder, Update, handle_input, merge};

const TITLE: &str = "Notes";

const NEW_NOTE: &str = "New Note";

const PLACEHOLDER: &str = "Title";

const BACK: &str = "\u{2039}  Notes";

const TRASHED: &str = "It goes to the trash";

const ROW: u32 = (EM * 4).unsigned_abs();

const HEADER: u32 = (EM * 5).unsigned_abs();

const HEADING: u32 = (EM * 4 / 3).unsigned_abs();

const WORDS: u32 = EM.unsigned_abs();

const SMALL: u32 = (EM * 5 / 6).unsigned_abs();

const INSIDE: u32 = (EM * 2 / 3).unsigned_abs();

const ROUND: u32 = (EM * 2 / 3).unsigned_abs();

const CARET: u32 = 2;

const MEASURING: u32 = 4096;

const COMMAND: Command = Command {
    name: "notes",
    about: "the notes, and the note open on them; a note's first line is its title",
    flags: &[],
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum Notice {
    None,
    Copy(String),
    Error(String),
}

struct List {
    folder: PathBuf,
    notes: Vec<Note>,
    selected: u32,
    first: u32,
    prompt: Prompt,
    pressed: Option<u32>,
    notice: Notice,
}

struct OpenNote {
    at: Option<PathBuf>,
    known: String,
    area: TextArea,
    notice: Notice,
    mode: PageMode,
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
        let mut preview = Preview { sources, selected: 0, image: Err(DiagramError::NoDiagram) };
        let Ok(()) = preview.render();

        Ok(preview)
    }

    fn render(&mut self) -> Result<(), Never> {
        let Ok(at) = console_core_number_conversion::index(self.selected);

        self.image = match self.sources.get(at) {
            Some(source) => diagrams::drawn(source, Size { width: 2048, height: 1536 }),
            None => Err(DiagramError::NoDiagram),
        };

        Ok(())
    }

    fn step(&mut self, by: i32) -> Result<Update, Never> {
        let Ok(count) = fitted::<_, u32>(self.sources.len());
        let selected = self.selected.saturating_add_signed(by).min(count.saturating_sub(1));

        match selected == self.selected {
            true => Ok(Update::None),
            false => {
                self.selected = selected;

                let Ok(()) = self.render();

                Ok(Update::Redraw)
            },
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

    code
}

fn run(arguments: &[String]) -> Result<(), NotesError> {
    let Ok(home) = console_core_places::home();
    let home = home.ok_or(NotesError::NoHome)?;
    let wearing = Wearing::worn().map_err(NotesError::Palette)?;
    let Ok(appearance) = Appearance::current();
    let folder = home.join(folder::FOLDER);
    let Ok(list) = List::of(&folder, None);
    let mut app = App { folder, wearing, appearance, view: View::List(list) };

    match arguments.first() {
        Some(asked) => {
            let Ok(_opened) = app.open(Some(PathBuf::from(asked)));
        },
        None => {},
    }

    let mut surface = Surface::connect().map_err(NotesError::Surface)?;
    let Ok(named) = InternalProgram::Notes.name();
    let window = Window { app_id: named.to_string(), title: TITLE.to_string() };

    surface.show_window(&window).map_err(NotesError::Surface)?;

    let ended = event_loop(&mut app, &mut surface);
    let Ok(_left) = app.leave_page();

    ended
}

fn event_loop(app: &mut App, surface: &mut Surface) -> Result<(), NotesError> {
    let looked = iterate((app, surface, Update::Redraw, None::<Size<u32>>), |(app, surface, update, drawn_at)| {
        let drawn_at = match update {
            Update::Redraw => {
                let Ok(()) = draw(app, surface);
                let Ok(room) = logical_size(surface);

                Some(room)
            },
            Update::None => drawn_at,
            Update::Exit => return Ok(Step::Halt(Ok(()))),
        };

        match surface.wait(&[], None) {
            Ok(()) => {},
            Err(fault) => return Ok(Step::Halt(Err(NotesError::Surface(fault)))),
        }

        let Ok(closed) = surface.closed();

        match closed {
            Closed::Yes => return Ok(Step::Halt(Ok(()))),
            Closed::No => {},
        }

        let Ok(room) = logical_size(surface);
        let Ok(input) = handle_input(app, surface, room);

        let resized = match drawn_at == Some(room) {
            true => Update::None,
            false => Update::Redraw,
        };

        let Ok(update) = merge(input, resized);

        Ok(Step::Again((app, surface, update, drawn_at)))
    });

    match looked {
        Ok(looked) => looked,
        Err(Endless) => Ok(()),
    }
}

fn logical_size(surface: &Surface) -> Result<Size<u32>, Never> {
    let Ok(logical) = surface.logical();

    Ok(match logical {
        Some(logical) => logical,
        None => Size { width: 1280, height: 800 },
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
        Ok(_let_go) => {},
        Err(fault) => eprintln!("notes: the keyboard would not come up: {fault}"),
    }

    Ok(())
}

fn hide_keyboard() -> Result<(), Never> {
    use console_input_keyboard::remote::{self, Answer, Command};

    let Ok(answer) = remote::send(Command::Hide);

    match answer {
        Answer::Received | Answer::Sessionless => {},
        Answer::Failed { at, why } => eprintln!("notes: the keyboard could not be hidden at {}: {why}", at.display()),
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
        self.on_list(room, |list, shown| list.pointed(event, shown))
    }

    fn on_list(&mut self, room: Size<u32>, answer: impl FnOnce(&mut List, Visible) -> Result<Choice, Never>) -> Result<Update, Never> {
        let Ok(shown) = shown(room);

        let choice = match &mut self.view {
            View::List(list) => answer(list, shown),
            View::Page(_) => return Ok(Update::None),
        };

        let Ok(choice) = choice;

        self.choose(choice)
    }

    fn choose(&mut self, choice: Choice) -> Result<Update, Never> {
        match choice {
            Choice::Stay(update) => Ok(update),
            Choice::Open(at) => self.open(at),
            Choice::Exit => Ok(Update::Exit),
        }
    }

    fn open(&mut self, at: Option<PathBuf>) -> Result<Update, Never> {
        let read = match at.as_deref() {
            Some(at) => match console_core_atomic_writes::text_or_empty(at) {
                Ok(file) => {
                    let Ok(text) = folder::text(&file, at);

                    Ok((file, text))
                },
                Err(fault) => Err(fault.to_string()),
            },
            None => Ok((String::new(), String::new())),
        };

        match read {
            Ok((known, text)) => {
                let Ok(area) = TextArea::holding(&text);

                self.view = View::Page(Box::new(OpenNote { at, known, area, notice: Notice::None, mode: PageMode::Writing }));

                let Ok(()) = show_keyboard();
            },
            Err(fault) => match &mut self.view {
                View::List(list) => list.notice = Notice::Error(fault),
                View::Page(_) => {},
            },
        }

        Ok(Update::Redraw)
    }

    fn page_key(&mut self, key: Keysym, room: Size<u32>) -> Result<Update, Never> {
        match key {
            Keysym::F5 | Keysym::F18 | Keysym::XF86_Launch9 => return self.toggle_diagrams(),
            _ => {},
        }

        match (&self.view, key) {
            (View::Page(writing), Keysym::Escape) => match writing.mode {
                PageMode::Writing => return self.back(),
                PageMode::Diagrams(_) => return self.toggle_diagrams(),
            },
            _ => {},
        }

        let appearance = self.appearance;

        let writing = match &mut self.view {
            View::Page(writing) => writing,
            View::List(_) => return Ok(Update::None),
        };

        match &mut writing.mode {
            PageMode::Diagrams(preview) => return match key {
                Keysym::Left | Keysym::Up | Keysym::Page_Up => preview.step(-1),
                Keysym::Right | Keysym::Down | Keysym::Page_Down => preview.step(1),
                Keysym::Return | Keysym::KP_Enter => self.toggle_diagrams(),
                _ => Ok(Update::None),
            },
            PageMode::Writing => {},
        }

        let Ok(rows) = rows_of(&writing.area, appearance, room);

        let Ok(typed) = match key {
            Keysym::Page_Up | Keysym::Page_Down => {
                let Ok(by) = a_page(&writing.area, &rows, appearance, room);

                let way = match key == Keysym::Page_Up {
                    true => Way::Up,
                    false => Way::Down,
                };

                writing.area.stepped(&rows, way, by)
            },
            key => writing.area.typed(key, &rows),
        };

        match typed {
            Typed::Edited => {
                let Ok(()) = writing.kept(&self.folder);

                Ok(Update::Redraw)
            },
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
                },
                PageMode::Writing => {},
            },
            _ => {},
        }

        let at = match event {
            PointerEvent::Down { at } => at,
            PointerEvent::Moved { .. }
            | PointerEvent::Scrolled { .. }
            | PointerEvent::Pinched { .. }
            | PointerEvent::Up
            | PointerEvent::Left => return Ok(Update::None),
        };

        let Ok(at) = point_of(at);
        let Ok(edge) = fitted::<u32, i32>(READING_EDGE);

        match at.y < edge {
            true => {
                let Ok(across) = fitted::<i32, u32>(at.x.max(0));

                return match across >= room.width.saturating_div(2) {
                    true => self.toggle_diagrams(),
                    false => self.back(),
                };
            },
            false => {},
        }

        let appearance = self.appearance;

        let writing = match &mut self.view {
            View::Page(writing) => writing,
            View::List(_) => return Ok(Update::None),
        };

        match &mut writing.mode {
            PageMode::Diagrams(preview) => {
                let Ok(down) = fitted::<i32, u32>(at.y.max(0));
                let Ok(across) = fitted::<i32, u32>(at.x.max(0));

                return match down >= room.height.saturating_sub(READING_EDGE) {
                    true => preview.step(match across < room.width.saturating_div(2) {
                        true => -1,
                        false => 1,
                    }),
                    false => Ok(Update::None),
                };
            },
            PageMode::Writing => {},
        }

        let Ok(caret) = caret_at(&writing.area, at, appearance, room);
        let Ok(_moved) = writing.area.placed(caret);
        let Ok(()) = show_keyboard();

        Ok(Update::Redraw)
    }

    fn toggle_diagrams(&mut self) -> Result<Update, Never> {
        let writing = match &mut self.view {
            View::Page(writing) => writing,
            View::List(_) => return Ok(Update::None),
        };

        writing.mode = match writing.mode {
            PageMode::Writing => {
                let Ok(text) = writing.area.text();
                let Ok(preview) = Preview::of(text);
                let Ok(()) = hide_keyboard();

                PageMode::Diagrams(preview)
            },
            PageMode::Diagrams(_) => {
                let Ok(()) = show_keyboard();

                PageMode::Writing
            },
        };

        Ok(Update::Redraw)
    }

    fn back(&mut self) -> Result<Update, Never> {
        let Ok(left) = self.leave_page();
        let Ok(list) = List::of(&self.folder, left.as_deref());

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

        let written = OnDisk { at: Some(&at), known: &writing.known };

        Ok(match folder::leave(&self.folder, written, text) {
            Ok(Leave::Retain) => Some(at),
            Ok(Leave::Rename(to)) => Some(to),
            Ok(Leave::Remove) => None,
            Err(fault) => {
                eprintln!("notes: {fault}");

                Some(at)
            },
        })
    }
}

impl Responder for App {
    fn key_pressed(&mut self, key: Keysym, room: Size<u32>) -> Result<Update, Never> {
        let Ok(screen) = self.screen();

        match screen {
            Screen::List => self.list_key(key, room),
            Screen::Page => self.page_key(key, room),
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
    fn kept(&mut self, folder: &Path) -> Result<(), Never> {
        let Ok(text) = self.area.text();

        match (self.at.as_ref(), text.trim().is_empty()) {
            (None, true) => return Ok(()),
            (None, false) | (Some(_), _) => {},
        }

        let written = OnDisk { at: self.at.as_deref(), known: &self.known };

        match folder::save(folder, written, text) {
            Ok(Record { at, known, destination }) => {
                self.notice = match (destination, &self.notice) {
                    (Destination::Copy, _) => Notice::Copy(format!(
                        "This note changed somewhere else, so this is kept as {}",
                        at.display()
                    )),
                    (Destination::Original, Notice::Copy(said)) => Notice::Copy(said.clone()),
                    (Destination::Original, Notice::None | Notice::Error(_)) => Notice::None,
                };
                self.at = Some(at);
                self.known = known;
            },
            Err(fault) => self.notice = Notice::Error(fault.to_string()),
        }

        Ok(())
    }
}

impl List {
    fn of(folder: &Path, standing: Option<&Path>) -> Result<List, Never> {
        let (notes, notice) = match folder::listed(folder) {
            Ok(notes) => (notes, Notice::None),
            Err(fault) => (Vec::new(), Notice::Error(fault.to_string())),
        };

        let found = (1_u32..).zip(&notes).find(|(_, note)| Some(note.path.as_path()) == standing);

        let selected = match found {
            Some((at, _)) => at,
            None => 0,
        };

        Ok(List { folder: folder.to_path_buf(), notes, selected, first: 0, prompt: Prompt::None, pressed: None, notice })
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
            },
            (Meaning::Choose, Prompt::None) => {
                let Ok(choice) = self.choice();

                choice
            },
            (Meaning::More, Prompt::None) => {
                let Ok(note) = self.note(self.selected);

                match note {
                    Some(_) => {
                        self.prompt = Prompt::Delete;

                        Choice::Stay(Update::Redraw)
                    },
                    None => Choice::Stay(Update::None),
                }
            },
            (Meaning::Close | Meaning::Abandon, Prompt::None) => Choice::Exit,
            (Meaning::Choose, Prompt::Delete) => {
                let Ok(()) = self.trash();

                Choice::Stay(Update::Redraw)
            },
            (Meaning::Close | Meaning::Abandon, Prompt::Delete) => {
                self.prompt = Prompt::None;

                Choice::Stay(Update::Redraw)
            },
            (Meaning::None | Meaning::Nudge(_) | Meaning::Tab(_), Prompt::None)
            | (Meaning::Step(_) | Meaning::More | Meaning::None | Meaning::Nudge(_) | Meaning::Tab(_), Prompt::Delete) => {
                Choice::Stay(Update::None)
            },
        })
    }

    fn pointed(&mut self, event: PointerEvent, shown: Visible) -> Result<Choice, Never> {
        Ok(match event {
            PointerEvent::Down { at } => {
                let Ok(at) = point_of(at);
                let Ok(row) = self.row_under(at);

                self.pressed = row;

                match row {
                    Some(row) => {
                        self.prompt = Prompt::None;

                        let Ok(()) = self.stand_on(row, shown);

                        Choice::Stay(Update::Redraw)
                    },
                    None => Choice::Stay(Update::None),
                }
            },
            PointerEvent::Up => match self.pressed.take() {
                Some(row) => match row == self.selected {
                    true => {
                        let Ok(choice) = self.choice();

                        choice
                    },
                    false => Choice::Stay(Update::None),
                },
                None => Choice::Stay(Update::None),
            },
            PointerEvent::Scrolled { by } => {
                let Ok(last) = self.last();
                let Ok(step) = toward_zero_i32(by.signum());
                let Ok(()) = self.stand_on(self.selected.saturating_add_signed(step).min(last), shown);

                Choice::Stay(Update::Redraw)
            },
            PointerEvent::Left => {
                self.pressed = None;

                Choice::Stay(Update::None)
            },
            PointerEvent::Moved { .. } | PointerEvent::Pinched { .. } => Choice::Stay(Update::None),
        })
    }

    fn row_under(&self, at: Point<i32>) -> Result<Option<u32>, Never> {
        let Ok(down) = fitted::<i32, u32>(at.y.max(0));
        let Ok(last) = self.last();

        Ok(match down.checked_sub(HEADER) {
            Some(into) => {
                let row = self.first.saturating_add(into.saturating_div(ROW));

                match row > last {
                    true => None,
                    false => Some(row),
                }
            },
            None => None,
        })
    }

    fn stand_on(&mut self, row: u32, shown: Visible) -> Result<(), Never> {
        let Visible(shown) = shown;

        self.selected = row;

        self.first = match (row < self.first, row >= self.first.saturating_add(shown)) {
            (true, _) => row,
            (false, true) => row.saturating_add(1).saturating_sub(shown),
            (false, false) => self.first,
        };

        Ok(())
    }

    fn choice(&self) -> Result<Choice, Never> {
        let Ok(note) = self.note(self.selected);

        Ok(Choice::Open(note.map(|note| note.path.clone())))
    }

    fn trash(&mut self) -> Result<(), Never> {
        let Ok(note) = self.note(self.selected);

        let at = match note {
            Some(note) => note.path.display().to_string(),
            None => return Ok(()),
        };

        let Ok(trashing) = Program::Gio.arguments(&["trash", "--", &at]);

        let notice = match console_core_external_programs::capture_output(&trashing) {
            Ok(_said) => Notice::None,
            Err(fault) => Notice::Error(fault.to_string()),
        };

        let Ok(listed) = List::of(&self.folder, None);
        let Ok(last) = listed.last();

        *self = List { selected: self.selected.min(last), first: self.first, notice, ..listed };

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Visible(u32);

fn shown(room: Size<u32>) -> Result<Visible, Never> {
    Ok(Visible(room.height.saturating_sub(HEADER).saturating_div(ROW).max(1)))
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
        let Ok(lines) = console_draw_painting::wrapped(Run { said, weight, width: column.width }, &font);

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
    Ok(room.height.saturating_sub(READING_EDGE.saturating_mul(2)).saturating_sub(WORDS))
}

struct Layout {
    pages: Vec<std::ops::Range<u32>>,
    heights: Vec<u32>,
    page: u32,
}

fn layout(area: &TextArea, rows: &[Row], appearance: Appearance, room: Size<u32>) -> Result<Layout, Never> {
    let Ok(sized) = Heights::of(appearance);

    let heights: Vec<u32> = rows
        .iter()
        .map(|row| {
            let Ok(tall) = sized.row(row);

            tall
        })
        .collect();

    let Ok(room) = text_height(room);
    let Ok(stacked) = pages::pages(&heights, room);
    let Ok(caret) = area.caret();
    let Ok(standing) = row_at(rows, caret);

    let Ok(page) = pages::holding(&stacked, match standing {
        Some((at, _)) => at,
        None => 0,
    });

    Ok(Layout { pages: stacked, heights, page })
}

fn a_page(area: &TextArea, rows: &[Row], appearance: Appearance, room: Size<u32>) -> Result<u32, Never> {
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
        let Ok(at) = console_core_number_conversion::index(index);

        match (rows.get(at), layout.heights.get(at)) {
            (Some(row), Some(tall)) => {
                found.push(Placement { row: *row, top, tall: *tall });
                top = top.saturating_add(*tall);
            },
            (Some(_) | None, Some(_) | None) => {},
        }
    }

    Ok(found)
}

fn width_of(said: &str, style: Style, appearance: Appearance) -> Result<u32, Never> {
    let Ok((font, weight)) = appearance.styled(style);
    let Ok(measured) = console_draw_painting::measure_text(Run { said, weight, width: MEASURING }, &font);

    Ok(measured.width)
}

fn small_width(said: &str) -> Result<u32, Never> {
    let font = Font { family: LETTERS.to_string(), height: SMALL };
    let Ok(measured) = console_draw_painting::measure_text(Run { said, weight: Weight::Plain, width: MEASURING }, &font);

    Ok(measured.width)
}

fn caret_at(area: &TextArea, at: Point<i32>, appearance: Appearance, room: Size<u32>) -> Result<u32, Never> {
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
    let across = across.saturating_sub(column.left);
    let Ok(text) = area.text();
    let Ok(style) = style_of(placed.row.paragraph);

    let fits = (placed.row.start..=placed.row.end).take_while(|end| {
        let Ok(before) = between(text, placed.row.start..*end);
        let Ok(wide) = width_of(before, style, appearance);

        wide <= across
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

    let Ok(painting) = console_draw_painting::painter(room, &shapes, "notes");

    match surface.draw(painting) {
        Ok(()) => {},
        Err(fault) => eprintln!("notes: {fault}"),
    }

    Ok(())
}

fn background(room: Size<u32>, fill: Oklch) -> Result<Shape, Never> {
    Ok(Shape::Panel(Panel { at: Point { x: 0, y: 0 }, size: room, round: Round(0), fill, edge: Edge::None }))
}

fn words(said: &str, at: (u32, u32), (tall, weight): (u32, Weight), (width, ink): (u32, Oklch)) -> Result<Shape, Never> {
    let Ok(x) = fitted::<u32, i32>(at.0);
    let Ok(y) = fitted::<u32, i32>(at.1);

    Ok(Shape::Text(Text {
        at: Point { x, y },
        width,
        said: said.to_string(),
        weight,
        font: Font { family: LETTERS.to_string(), height: tall },
        ink,
    }))
}

fn list_shapes(list: &List, wearing: &Wearing, room: Size<u32>) -> Result<Vec<Shape>, Never> {
    let Ok(ground) = background(room, wearing.panel);
    let Ok(column) = appearance::column(room);
    let Ok(header) = words(TITLE, (column.left, WORDS), (HEADING, Weight::Bold), (column.width, wearing.text));
    let mut shapes = vec![ground, header];
    let Ok(Visible(shown)) = shown(room);
    let Ok(shown) = console_core_number_conversion::index(shown);
    let Ok(last) = list.last();

    for (row, top) in (list.first..=last).take(shown).zip((0_u32..).map(|step| HEADER.saturating_add(step.saturating_mul(ROW)))) {
        let Ok(note) = list.note(row);

        let (title, opening) = match note {
            Some(note) => (note.title.as_str(), note.opening.as_str()),
            None => (NEW_NOTE, ""),
        };

        let lit = (row == list.selected, list.prompt);

        let (fill, ink, quiet) = match lit {
            (true, Prompt::None) => (Some(wearing.pink), wearing.night, wearing.night),
            (true, Prompt::Delete) => (Some(wearing.coral), wearing.night, wearing.night),
            (false, _) => (None, wearing.text, wearing.soft),
        };

        let (title, opening) = match lit {
            (true, Prompt::Delete) => (format!("Delete \u{201c}{title}\u{201d}?"), TRASHED.to_string()),
            (true, Prompt::None) | (false, _) => (title.to_string(), opening.to_string()),
        };

        let Ok(y) = fitted::<u32, i32>(top);
        let Ok(x) = fitted::<u32, i32>(column.left);

        match fill {
            Some(fill) => shapes.push(Shape::Panel(Panel {
                at: Point { x, y },
                size: Size { width: column.width, height: ROW.saturating_sub(INSIDE.saturating_div(2)) },
                round: Round(ROUND),
                fill,
                edge: Edge::None,
            })),
            None => {},
        }

        let inner = column.width.saturating_sub(INSIDE.saturating_mul(2));
        let Ok(said) = words(&title, (column.left.saturating_add(INSIDE), top.saturating_add(INSIDE)), (WORDS, Weight::Bold), (inner, ink));

        shapes.push(said);

        match opening.is_empty() {
            true => {},
            false => {
                let under = top.saturating_add(INSIDE).saturating_add(WORDS).saturating_add(INSIDE.saturating_div(2));
                let Ok(aside) = words(&opening, (column.left.saturating_add(INSIDE), under), (SMALL, Weight::Plain), (inner, quiet));

                shapes.push(aside);
            },
        }
    }

    let Ok(()) = say(&mut shapes, &list.notice, wearing, room);

    Ok(shapes)
}

fn say(shapes: &mut Vec<Shape>, said: &Notice, wearing: &Wearing, room: Size<u32>) -> Result<(), Never> {
    let words_said = match said {
        Notice::None => return Ok(()),
        Notice::Copy(said) | Notice::Error(said) => said,
    };

    let Ok(column) = appearance::column(room);
    let foot = room.height.saturating_sub(READING_EDGE.saturating_div(2)).saturating_sub(SMALL);
    let Ok(told) = words(words_said, (column.left, foot), (SMALL, Weight::Plain), (column.width, wearing.coral));

    shapes.push(told);

    Ok(())
}

fn page_shapes(writing: &OpenNote, wearing: &Wearing, appearance: Appearance, room: Size<u32>) -> Result<Vec<Shape>, Never> {
    match &writing.mode {
        PageMode::Diagrams(preview) => return diagram_shapes(preview, wearing, appearance, room),
        PageMode::Writing => {},
    }

    let Ok(paper) = appearance.ground(wearing);
    let Ok(ink) = appearance.ink(wearing);
    let Ok(ground) = background(room, paper);
    let Ok(back) = words(BACK, (WORDS, WORDS), (SMALL, Weight::Plain), (MEASURING, ink));
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

        let (said, color) = match (text.is_empty(), placed.row.paragraph) {
            (true, 0) => (PLACEHOLDER, wearing.soft),
            (true, _) | (false, _) => (said, ink),
        };

        shapes.push(Shape::Text(Text {
            at: Point { x: left, y: down },
            width: column.width.saturating_add(READING_EDGE),
            said: said.to_string(),
            weight,
            font,
            ink: color,
        }));

        match standing.map(|(_, row)| row) == Some(placed.row) {
            true => {
                let Ok(before) = between(text, placed.row.start..caret);
                let Ok(across) = width_of(before, style, appearance);
                let Ok(across) = fitted::<u32, i32>(column.left.saturating_add(across));

                shapes.push(Shape::Panel(Panel {
                    at: Point { x: across, y: down },
                    size: Size { width: CARET, height: placed.tall },
                    round: Round(0),
                    fill: wearing.pink,
                    edge: Edge::None,
                }));
            },
            false => {},
        }
    }

    let Ok(count) = fitted::<_, u32>(layout.pages.len());

    match count > 1 {
        true => {
            let folio = format!("{} of {count}", layout.page.saturating_add(1));
            let Ok(wide) = small_width(&folio);
            let across = room.width.saturating_sub(wide.min(room.width)).saturating_div(2);
            let foot = room.height.saturating_sub(READING_EDGE.saturating_div(2)).saturating_sub(SMALL);
            let Ok(number) = words(&folio, (across, foot), (SMALL, Weight::Plain), (MEASURING, ink));

            shapes.push(number);
        },
        false => {},
    }

    let Ok(()) = say(&mut shapes, &writing.notice, wearing, room);

    Ok(shapes)
}

fn diagram_button(label: &str, ink: Oklch, room: Size<u32>) -> Result<Shape, Never> {
    let Ok(wide) = small_width(label);
    let across = room.width.saturating_sub(wide).saturating_sub(WORDS).max(room.width.saturating_div(2));

    words(label, (across, WORDS), (SMALL, Weight::Plain), (room.width.saturating_sub(across), ink))
}

fn diagram_shapes(preview: &Preview, wearing: &Wearing, appearance: Appearance, room: Size<u32>) -> Result<Vec<Shape>, Never> {
    let Ok(paper) = appearance.ground(wearing);
    let Ok(ink) = appearance.ink(wearing);
    let Ok(ground) = background(room, paper);
    let Ok(back) = words(BACK, (WORDS, WORDS), (SMALL, Weight::Plain), (MEASURING, ink));
    let Ok(edit) = diagram_button("Edit · F5 / Y", ink, room);
    let mut shapes = vec![ground, back, edit];
    let Ok(column) = appearance::column(room);
    let available = room.height.saturating_sub(READING_EDGE.saturating_mul(2)).max(1);

    match &preview.image {
        Ok(pixels) => {
            let scale = (f64::from(column.width) / f64::from(pixels.width))
                .min(f64::from(available) / f64::from(pixels.height)).min(1.0);
            let Ok(width) = whole_u32(f64::from(pixels.width) * scale);
            let Ok(height) = whole_u32(f64::from(pixels.height) * scale);
            let Ok(x) = fitted::<u32, i32>(column.left.saturating_add(column.width.saturating_sub(width).saturating_div(2)));
            let Ok(y) = fitted::<u32, i32>(READING_EDGE.saturating_add(available.saturating_sub(height).saturating_div(2)));
            let at = Point { x, y };
            let size = Size { width: width.max(1), height: height.max(1) };

            shapes.push(Shape::Panel(Panel {
                at, size, round: Round(0), fill: Oklch { lightness: 1.0, chroma: 0.0, hue: 0.0 }, edge: Edge::None,
            }));
            shapes.push(Shape::Picture(Picture { at, size, pixels: pixels.clone() }));
        },
        Err(fault) => {
            let said = match preview.sources.is_empty() {
                true => "Add a Mermaid block to your note, then open Diagrams:\n\n```mermaid\nflowchart LR\n  Idea --> Note\n```".to_string(),
                false => fault.to_string(),
            };
            let Ok(message) = words(&said, (column.left, READING_EDGE), (WORDS, Weight::Plain), (column.width, ink));

            shapes.push(message);
        },
    }

    let Ok(count) = fitted::<_, u32>(preview.sources.len());

    match count > 0 {
        true => {
            let folio = format!("{} of {count}", preview.selected.saturating_add(1));
            let Ok(wide) = small_width(&folio);
            let across = room.width.saturating_sub(wide).saturating_div(2);
            let foot = room.height.saturating_sub(READING_EDGE).saturating_add(WORDS);
            let Ok(number) = words(&folio, (across, foot), (SMALL, Weight::Plain), (wide, ink));

            shapes.push(number);

            match preview.selected > 0 {
                true => {
                    let Ok(previous) = words("‹ Previous", (column.left, foot), (SMALL, Weight::Plain), (column.width.saturating_div(3), ink));

                    shapes.push(previous);
                },
                false => {},
            }

            match preview.selected.saturating_add(1) < count {
                true => {
                    let Ok(wide) = small_width("Next ›");
                    let across = column.left.saturating_add(column.width).saturating_sub(wide);
                    let Ok(next) = words("Next ›", (across, foot), (SMALL, Weight::Plain), (wide, ink));

                    shapes.push(next);
                },
                false => {},
            }
        },
        false => {},
    }

    Ok(shapes)
}
