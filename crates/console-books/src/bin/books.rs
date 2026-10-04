//! The library, and the book open on it.
//!
//!     books
//!     books ~/Books/Moby-Dick.epub
//!
//! The library is every book in `~/Books`, covers side by side with the title
//! across the foot of each and how far through it is in the corner, under a
//! line holding the name of the place and a search. The d-pad walks the
//! covers, A opens one where it was put down, B closes the book and then the
//! library, and the letters typed go into the search. The last cover is the
//! Book Store, which is the Books tab of the downloads panel.
//!
//! A book open is a page and little else: left and right turn it, and so do
//! the small bar at either side of it and a swipe across it. A tap on the page
//! itself turns nothing, because that is where a note goes. The two letters in
//! the top corner make the text smaller and larger, and so does a pinch.
//!
//! Down the left margin is what a finger on the page does. The hand with a line
//! through it is reading, where a touch on the words does nothing, so a book
//! held with a thumb on the glass marks nothing by accident. The marker
//! highlights: a drag runs over the words, a tap takes the word under it, and a
//! tap on a highlight takes it off again. The note types one: the words it is
//! dragged or tapped over are underlined, a card opens under them with the
//! keyboard, and what is typed is kept letter by letter. The cross on the card
//! closes it to an icon beside the line, and a tap on the icon opens it again
//! whatever is in the hand; a tap on the card writes in it. The pencil writes,
//! and the eraser rubs out the stroke, the highlight or the note under the
//! finger. Under them is whether handwriting is about the page or the
//! paragraph it is next to, and the two arrows take back the last change on
//! the page that is open and bring it back again -- never one on a page nobody
//! is looking at. Without a hand, Tab or Y walks the tools, and with the marker
//! or the note up and down pick a paragraph and A highlights the whole of it or
//! opens a note on it. B backs out of the writing, then out of the notes open
//! on the page, and only then out of the book. A mouse is a finger here. What
//! each of those means is `console_books::annotating`'s to decide; this
//! program measures where the finger is in the book's own terms and draws
//! what comes back. Every note of a book is in one Markdown file under
//! `~/Books/Notes`, which `console_books::notes` argues for.
//!
//! A PDF page and a comic page are pictures, and they are marked the same way:
//! their words come from the PDF's own text or, failing that, from tesseract
//! reading the picture, which `console_books::live_text` argues for. The
//! recognizing runs beside the reader, and the footer says so while it does,
//! so a turn to the next page never waits on it. A highlight on a picture is a
//! line under the words, because a picture drawn over cannot be seen through. Where it was left is written at every turn,
//! because the moment a reader stops reading is the moment nobody is going to
//! press anything to say so. Which book is open is written when it opens and
//! taken away when it is closed, so `books` with no book named comes back to the
//! page it was on when the machine went down rather than to the library.
//!
//! It is an app, and holds the lock an app holds: opened again on the same
//! book it is brought forward, and opened on another the one up stands down
//! for it. It is a window on a workspace of its own rather than a layer over
//! the whole screen, so a panel opens over it, a person can keep it on one
//! desktop and work on another, and the paddle closes it as it closes any
//! window in front.
//!
//! This is a surface of its own rather than a panel. A panel is a card of
//! rows and a library is a grid of pictures, which is the one arrangement the
//! panel surface does not draw; the home screen is the other grid on this
//! machine and is drawn the same way, as shapes placed by arithmetic that
//! `console_books::grid` does with no screen in the room.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime};

use console_books::annotating::{AnnotationEffect, AnnotationEvent, Annotating, Annotations, Gesture, Made, RAIL, Rail, Target, Tool, Touch, Typed, Visible, VisibleLine};
use console_books::appearance::{Appearance, Paint, TextSize};
use console_books::flow::{self, Block, Relative};
use console_books::notes::{self, Mark, Note, Popup, Scale, Scope, Spot, Stroke};
use console_books::library::{self, Book, CatalogEntry, Format};
use console_books::live_text::{self, Paragraph};
use console_books::open::{self, BookError, Comic, OpenPublication};
use console_books::pages::{self, Layout, Page, Style};
use console_books::progress::{self, Fraction, Location, Position};
use console_books::reading::{self, End, PagePosition, Turn, Destination};
use console_books::grid::{self, Direction, ScrollOffset, Selection, Grid};
use console_core_arguments::{Command, Operands};
use console_core_color::Oklch;
use console_core_color::palette::{Wearing, WearingError};
use console_core_external_programs::Program;
use console_core_iteration::{Endless, Step, iterate};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_events::subscription::Received;
use console_program_contract::{Change, EventGroup};
use console_program_lifetime::{BoundToParent, Still};
use std::sync::mpsc::Receiver;
use console_core_number_conversion::{fitted, toward_zero_i32};
use console_core_internal_programs::InternalProgram;
use console_core_shapes::{Edge, Font, Panel, Picture, Pixels, Round, Shape, Text, Weight};
use console_core_state_machine::{Machine, Transition};
use console_draw_painting::Run;
use console_draw_surface::standing::{Closed, KeyboardEvent, PointerEvent, Window};
use console_draw_surface::{Keysym, Surface};
use console_panel::card::refusal;
use console_panel::picker::{self, Alone};
use console_panel::pictures::{self as store, Side};

const TITLE: &str = "Books";

const LIBRARY: &str = "Library";

const SEARCH: &str = "Search";

const BOOK_STORE: &str = "Book Store";

const STORE_TAB: &str = "Books";

const CLOSE: &str = "\u{2715}";

const BACK: &str = "\u{2039}";

const FORWARD: &str = "\u{203a}";

const LETTER: &str = "A";

const TURN_BAR: Size<u32> = Size { width: 22, height: 64 };

const CORNER: u32 = 72;

const READING: &str = "\u{f1831}";

const MARKER: &str = "\u{ee5a}";

const PENCIL: &str = "\u{f03eb}";

const NOTE: &str = "\u{f11d7}";

const ON_THE_PAGE: &str = "\u{f09ee}";

const ON_THE_PARAGRAPH: &str = "\u{f027d}";

const UNDO: &str = "\u{f054c}";

const REDO: &str = "\u{f044e}";

const RAIL_BUTTON: u32 = 36;

const RAIL_GAP: u32 = 10;

const RAIL_TOP: u32 = 96;

const PEN: u32 = 3;

const ERASER: &str = "\u{f01fe}";

const ERASER_REACH: u32 = 16;

const NOTE_ICON: u32 = 24;

const NOTE_WIDE: u32 = 420;

const NOTE_PADDING: u32 = 12;

const NOTE_TEXT: u32 = 16;

const CARET: &str = "|";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    Close,
    Turn(Turn),
    Zoom(Zoom),
    Rail(Rail),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotePart {
    Icon,
    Close,
    Card,
}

#[derive(Debug, Clone, PartialEq)]
struct NoteFrame {
    at: Spot,
    shown: Shown,
}

#[derive(Debug, Clone, PartialEq)]
enum Shown {
    Icon(Panel),
    Card { card: Panel, close: Panel, lines: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Underline {
    Filled,
    Ruled,
}

const SANS: &str = "Noto Sans";

const WAITING: Duration = Duration::from_millis(500);

const READING_WIDE: u32 = 720;

const READING_EDGE: u32 = 48;

const TITLE_LINES: u32 = 2;

fn font(family: &str, tall: u32) -> Result<Font, Never> {
    Ok(Font { family: family.to_string(), height: tall })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Update {
    None,
    Redraw,
    Exit,
}

struct Library {
    home: PathBuf,
    books: Vec<Book>,
    locations: BTreeMap<String, Location>,
    catalog_changed: Option<SystemTime>,
    covers: Option<PathBuf>,
    covers_changed: Option<SystemTime>,
    search: String,
    selected: u32,
    scroll: ScrollOffset,
}

enum Content {
    Publication { book: OpenPublication, blocks: Vec<Block>, pages: Vec<Page> },
    PortableDocument { pages: u32 },
    Comic(Comic),
}

struct Reader {
    book: Book,
    content: Content,
    section: u32,
    page: u32,
    layout_size: Size<u32>,
    appearance: Appearance,
    cached_picture: Option<((u32, u32), Pixels)>,
    restore: Restore,
    cache: PathBuf,
    notes_at: Option<PathBuf>,
    live: LiveText,
    languages: Option<String>,
}

enum Stage {
    Script { picture: PathBuf, base: PathBuf },
    Transcript,
}

enum Recognition<'a> {
    Script { picture: &'a Path, base: &'a Path },
    Transcript { picture: &'a Path, base: &'a Path, languages: &'a str },
}

enum LiveText {
    Unasked,
    Recognizing { section: u32, running: BoundToParent, out: PathBuf, stage: Stage },
    Read { section: u32, paragraphs: Vec<Paragraph> },
}

enum View {
    Library,
    Reader(Box<Reader>),
}

struct App {
    library: Library,
    view: View,
    wearing: Wearing,
    appearance: Appearance,
    pointer_down: Option<Point<i32>>,
    pointer_at: Option<Point<i32>>,
    pinch: Option<Pinch>,
    annotations: Annotations,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Pinch {
    from: TextSize,
    by: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zoom {
    ZoomedOut,
    ZoomedIn,
}

const COMMAND: Command = Command {
    name: "books",
    about: "the library, and the book open on it; with nothing, the book that was open when it was left",
    flags: &[],
    operands: Operands::Optional("BOOK"),
};

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();

    let Ok(refusing) = refusal(&COMMAND, &arguments);

    match refusing {
        Some(code) => return code,
        None => {},
    }

    let Ok(named) = InternalProgram::Books.name();
    let Ok(alone) = picker::alone_as(named, &arguments);

    match alone {
        Alone::Yes => {},
        Alone::No => return ExitCode::SUCCESS,
    }

    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("books: {fault}");

            ExitCode::FAILURE
        },
    }
}

#[derive(Debug)]
enum BooksError {
    NoHome,
    Palette(WearingError),
    Surface(console_draw_surface::standing::SurfaceError),
}

impl std::fmt::Display for BooksError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BooksError::NoHome => write!(to, "HOME is not set, so there is no Books folder"),
            BooksError::Palette(fault) => write!(to, "{fault}"),
            BooksError::Surface(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for BooksError {}

fn window() -> Result<Window, Never> {
    let Ok(named) = InternalProgram::Books.name();

    Ok(Window { app_id: named.to_string(), title: TITLE.to_string() })
}

fn run(arguments: &[String]) -> Result<(), BooksError> {
    let Ok(home) = console_core_places::home();
    let home = home.ok_or(BooksError::NoHome)?;
    let wearing = Wearing::worn().map_err(BooksError::Palette)?;
    let Ok(appearance) = Appearance::current();
    let Ok(library) = Library::of(home);
    let Ok(annotations) = Annotations::empty();
    let mut app = App { library, view: View::Library, wearing, appearance, pointer_down: None, pointer_at: None, pinch: None, annotations };

    let Ok(open) = progress::open_path(&app.library.home);
    let Ok(left_open) = console_core_atomic_writes::read(&open);
    let Ok(left_open) = left_open.text();

    match (arguments.first(), left_open) {
        (Some(asked), _) => {
            let Ok(()) = app.open_path(Path::new(asked));
        },
        (None, Some(left_open)) => {
            let Ok(()) = app.open_path(Path::new(left_open.trim_end()));
        },
        (None, None) => {},
    }

    let mut surface = Surface::connect().map_err(BooksError::Surface)?;
    let Ok(window) = window();

    surface.show_window(&window).map_err(BooksError::Surface)?;

    event_loop(&mut app, &mut surface)
}

fn event_loop(app: &mut App, surface: &mut Surface) -> Result<(), BooksError> {
    let Ok(shelf) = library::books_folder(&app.library.home);
    let Ok(changes) = console_events::subscription::connect(&[EventGroup::Path(shelf)]);
    let Ok(arriving) = changes.received();
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

        match surface.wait(&[], Some(WAITING)) {
            Ok(()) => {},
            Err(fault) => return Ok(Step::Halt(Err(BooksError::Surface(fault)))),
        }

        let Ok(closed) = surface.closed();

        match closed {
            Closed::Yes => return Ok(Step::Halt(Ok(()))),
            Closed::No => {},
        }

        let Ok(room) = logical_size(surface);
        let Ok(input) = handle_input(app, surface, room);
        let Ok(restyled) = app.reload_appearance();
        let Ok(recataloged) = app.library.reload_if_changed();
        let Ok(covered) = app.library.redraw_if_covers_changed();
        let Ok(landed) = receive(&mut app.library, arriving);
        let Ok(changed) = merge(recataloged, landed);
        let Ok(changed) = merge(changed, covered);
        let Ok(changed) = merge(changed, restyled);
        let Ok(recognized) = app.poll_live_text();
        let Ok(changed) = merge(changed, recognized);
        let Ok(resized) = resized(drawn_at, room);

        let update = match (input, changed, resized) {
            (Update::Exit, _, _) => Update::Exit,
            (Update::Redraw, _, _) | (_, Update::Redraw, _) | (_, _, Update::Redraw) => Update::Redraw,
            (Update::None, Update::None | Update::Exit, Update::None | Update::Exit) => Update::None,
        };

        Ok(Step::Again((app, surface, update, drawn_at)))
    });

    match looked {
        Ok(looked) => looked,
        Err(Endless) => Ok(()),
    }
}

fn receive(library: &mut Library, arriving: &Receiver<Received>) -> Result<Update, Never> {
    let landed = arriving.try_iter().find_map(|received| match received {
        Received::Event(change) => Some(change),
        Received::Connected => None,
    });

    match landed {
        Some(change) => library.apply_change(&change),
        None => Ok(Update::None),
    }
}

fn resized(drawn_at: Option<Size<u32>>, room: Size<u32>) -> Result<Update, Never> {
    Ok(match drawn_at == Some(room) {
        true => Update::None,
        false => Update::Redraw,
    })
}

fn logical_size(surface: &Surface) -> Result<Size<u32>, Never> {
    let Ok(logical) = surface.logical();

    Ok(match logical {
        Some(logical) => logical,
        None => Size { width: 1280, height: 800 },
    })
}

fn handle_input(app: &mut App, surface: &mut Surface, room: Size<u32>) -> Result<Update, Never> {
    let Ok(keys) = surface.keyboard_events();
    let Ok(pointer) = surface.pointer_events();
    let mut outcome = Update::None;

    for KeyboardEvent::Down { key } in keys {
        let Ok(update) = app.key_pressed(key, room);

        let Ok(merged) = merge(outcome, update);

        outcome = merged;
    }

    for event in pointer {
        let Ok(update) = app.pointer_event(event, room);

        let Ok(merged) = merge(outcome, update);

        outcome = merged;
    }

    Ok(outcome)
}

#[must_use]
fn merge(was: Update, now: Update) -> Result<Update, Never> {
    Ok(match (was, now) {
        (Update::Exit, _) | (_, Update::Exit) => Update::Exit,
        (Update::Redraw, _) | (_, Update::Redraw) => Update::Redraw,
        (Update::None, Update::None) => Update::None,
    })
}

fn catalog_path(home: &Path) -> Result<PathBuf, Never> {
    let Ok(cache) = library::cache_folder(home);

    Ok(cache.join(library::CATALOG))
}

fn modified(at: &Path) -> Result<Option<SystemTime>, Never> {
    Ok(match std::fs::metadata(at).and_then(|found| found.modified()) {
        Ok(when) => Some(when),
        Err(_absent) => None,
    })
}

impl Library {
    fn of(home: PathBuf) -> Result<Library, Never> {
        let Ok(at) = progress::path(&home);
        let Ok(contents) = console_core_atomic_writes::read(&at);

        let locations = match contents.text() {
            Ok(Some(contents)) => {
                let Ok(read) = progress::parse(&contents);

                read
            },
            Ok(None) | Err(_) => BTreeMap::new(),
        };

        let Ok(covers) = store::store();

        let mut library = Library {
            home,
            books: Vec::new(),
            locations,
            catalog_changed: None,
            covers,
            covers_changed: None,
            search: String::new(),
            selected: 0,
            scroll: ScrollOffset::default(),
        };

        let Ok(()) = library.reload();

        Ok(library)
    }

    fn reload(&mut self) -> Result<(), Never> {
        let Ok(folder) = library::books_folder(&self.home);
        let Ok(at) = catalog_path(&self.home);
        let Ok(contents) = console_core_atomic_writes::read(&at);

        let catalog: BTreeMap<String, CatalogEntry> = match contents.text() {
            Ok(Some(contents)) => {
                let Ok(read) = library::read_catalog(&contents);

                read
            },
            Ok(None) | Err(_) => BTreeMap::new(),
        };

        let Ok(names) = library::names(&folder);
        let Ok(books) = library::books(&folder, &names, &catalog);
        let unseen = books.iter().any(|book| !catalog.contains_key(&book.name));

        self.books = books;
        let Ok(changed) = modified(&at);

        self.catalog_changed = changed;

        match unseen {
            true => {
                let Ok(()) = spawn_detached(InternalProgram::BooksCatalog, &[]);
            },
            false => {},
        }

        Ok(())
    }

    fn reload_if_changed(&mut self) -> Result<Update, Never> {
        let Ok(at) = catalog_path(&self.home);
        let Ok(changed) = modified(&at);

        Ok(match changed == self.catalog_changed {
            true => Update::None,
            false => {
                let Ok(()) = self.reload();

                Update::Redraw
            },
        })
    }

    fn redraw_if_covers_changed(&mut self) -> Result<Update, Never> {
        let changed = match &self.covers {
            Some(at) => {
                let Ok(changed) = modified(at);

                changed
            },
            None => None,
        };

        Ok(match changed == self.covers_changed {
            true => Update::None,
            false => {
                self.covers_changed = changed;

                Update::Redraw
            },
        })
    }

    fn apply_change(&mut self, change: &Change) -> Result<Update, Never> {
        let Ok(folder) = library::books_folder(&self.home);

        Ok(match (&change.event_group, Path::new(&change.text).starts_with(&folder)) {
            (EventGroup::Path(_), true) => {
                let Ok(()) = self.reload();

                Update::Redraw
            },
            (EventGroup::Path(_), false)
            | (
                EventGroup::Compositor
                | EventGroup::Sound
                | EventGroup::Network
                | EventGroup::Wifi
                | EventGroup::Bluetooth
                | EventGroup::Battery
                | EventGroup::Notifications
                | EventGroup::Units
                | EventGroup::Player,
                _,
            ) => Update::None,
        })
    }

    fn visible_books(&self) -> Result<Vec<&Book>, Never> {
        library::search(&self.books, &self.search)
    }

    fn save_location(&mut self, name: &str, location: Location) -> Result<(), Never> {
        self.locations.insert(name.to_string(), location);

        let Ok(at) = progress::path(&self.home);
        let Ok(text) = progress::serialize(&self.locations);

        match at.parent().map(std::fs::create_dir_all) {
            Some(Err(fault)) => eprintln!("books: {}: {fault}", at.display()),
            Some(Ok(())) | None => {},
        }

        match console_core_atomic_writes::whole(&at, text.as_bytes()) {
            Ok(()) => {},
            Err(fault) => eprintln!("books: where this book was left was not kept: {fault}"),
        }

        Ok(())
    }
}

fn spawn_detached(program: InternalProgram, arguments: &[&str]) -> Result<(), Never> {
    let Ok(mut command) = program.command();

    command.args(arguments).stdout(std::process::Stdio::null());

    match console_program_lifetime::let_go(&mut command) {
        Ok(_let_go) => {},
        Err(fault) => {
            let Ok(name) = program.name();

            eprintln!("books: {name} would not start: {fault}");
        },
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Library,
    Reader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Book(u32),
    BookStore,
}

fn direction(key: Keysym) -> Result<Option<Direction>, Never> {
    Ok(match key {
        Keysym::Left => Some(Direction::Left),
        Keysym::Right => Some(Direction::Right),
        Keysym::Up => Some(Direction::Up),
        Keysym::Down => Some(Direction::Down),
        _ => None,
    })
}

fn turn_of(key: Keysym) -> Result<Option<Turn>, Never> {
    Ok(match key {
        Keysym::Right | Keysym::Down | Keysym::Page_Down | Keysym::Return | Keysym::KP_Enter | Keysym::space => {
            Some(Turn::Forward)
        },
        Keysym::Left | Keysym::Up | Keysym::Page_Up => Some(Turn::Back),
        _ => None,
    })
}

fn point_of(at: (f64, f64)) -> Result<Point<i32>, Never> {
    let Ok(across) = toward_zero_i32(at.0);
    let Ok(down) = toward_zero_i32(at.1);

    Ok(Point { x: across, y: down })
}

impl App {
    fn screen(&self) -> Result<Screen, Never> {
        Ok(match &self.view {
            View::Library => Screen::Library,
            View::Reader(_) => Screen::Reader,
        })
    }

    fn open_path(&mut self, at: &Path) -> Result<(), Never> {
        let name = match at.file_name().and_then(|name| name.to_str()) {
            Some(name) => name.to_string(),
            None => return Ok(()),
        };

        let Ok(format) = library::format_of(&name);

        let format = match format {
            Some(format) => format,
            None => {
                eprintln!("books: {}: this is not a book this reader opens", at.display());

                return Ok(());
            },
        };

        let Ok(title) = console_core_file_names::title(&name);

        self.open_book(Book { path: at.to_path_buf(), name, format, title, cover: None })
    }

    fn open_book(&mut self, book: Book) -> Result<(), Never> {
        let location = self.library.locations.get(&book.name).copied();
        let Ok(cache) = library::cache_folder(&self.library.home);
        let Ok(notes_at) = notes::path(&self.library.home, &book.title);

        let Ok(open) = progress::open_path(&self.library.home);
        let held = book.path.to_string_lossy().into_owned();

        match Reader::open(book, location, Stored { cache, notes_at }, self.appearance) {
            Ok(mut reader) => {
                let Ok(notes) = reader.read_notes();

                self.view = View::Reader(Box::new(reader));

                let Ok(_update) = self.annotate(AnnotationEvent::Loaded(notes));

                match open.parent().map(std::fs::create_dir_all) {
                    Some(Err(fault)) => eprintln!("books: {}: {fault}", open.display()),
                    Some(Ok(())) | None => {},
                }

                match console_core_atomic_writes::whole(&open, held.as_bytes()) {
                    Ok(()) => {},
                    Err(fault) => eprintln!("books: which book is open was not kept: {fault}"),
                }
            },
            Err(fault) => eprintln!("books: {fault}"),
        }

        Ok(())
    }

    fn close_book(&mut self) -> Result<Update, Never> {
        let Ok(_update) = self.annotate(AnnotationEvent::Left);

        self.put_book_away()
    }

    fn put_book_away(&mut self) -> Result<Update, Never> {
        let Ok(open) = progress::open_path(&self.library.home);

        self.view = View::Library;

        match std::fs::remove_file(&open) {
            Ok(()) => {},
            Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
                true => {},
                false => eprintln!("books: {}: {fault}", open.display()),
            },
        }

        Ok(Update::Redraw)
    }

    fn key_pressed(&mut self, key: Keysym, room: Size<u32>) -> Result<Update, Never> {
        let Ok(screen) = self.screen();

        match screen {
            Screen::Library => self.library_key_pressed(key, room),
            Screen::Reader => self.reader_key_pressed(key, room),
        }
    }

    fn library_key_pressed(&mut self, key: Keysym, room: Size<u32>) -> Result<Update, Never> {
        let Ok(direction) = direction(key);

        match direction {
            Some(direction) => return self.move_selection(direction, room),
            None => {},
        }

        Ok(match (key, self.library.search.is_empty()) {
            (Keysym::Return | Keysym::KP_Enter, _) => {
                let Ok(item) = self.selected_item();

                self.activate(item)?
            },
            (Keysym::Escape, true) | (Keysym::BackSpace, true) => Update::Exit,
            (Keysym::Escape, false) => {
                self.library.search.clear();

                self.reset_selection()?
            },
            (Keysym::BackSpace, false) => {
                let _ = self.library.search.pop();

                self.reset_selection()?
            },
            (key, _) => match key.key_char().filter(|letter| !letter.is_control()) {
                Some(letter) => {
                    self.library.search.push(letter);

                    self.reset_selection()?
                },
                None => Update::None,
            },
        })
    }

    fn reset_selection(&mut self) -> Result<Update, Never> {
        self.library.selected = 0;
        self.library.scroll = ScrollOffset::default();

        Ok(Update::Redraw)
    }

    fn move_selection(&mut self, direction: Direction, room: Size<u32>) -> Result<Update, Never> {
        let Ok(grid) = grid::layout(room);
        let Ok(visible) = self.library.visible_books();
        let Ok(count) = fitted::<_, u32>(visible.len());
        let selection = Selection { index: self.library.selected, count: count.saturating_add(1) };
        let Ok(selected) = grid::step_selection(selection, grid.columns, direction);
        let Ok(scroll) = grid::scroll_offset(grid, selected, self.library.scroll);

        self.library.selected = selected;
        self.library.scroll = scroll;

        Ok(Update::Redraw)
    }

    fn selected_item(&self) -> Result<Item, Never> {
        let Ok(visible) = self.library.visible_books();
        let Ok(count) = fitted::<_, u32>(visible.len());

        Ok(match self.library.selected < count {
            true => Item::Book(self.library.selected),
            false => Item::BookStore,
        })
    }

    fn activate(&mut self, item: Item) -> Result<Update, Never> {
        match item {
            Item::Book(at) => {
                let Ok(visible) = self.library.visible_books();
                let Ok(at) = console_core_number_conversion::index(at);
                let book = visible.get(at).map(|book| (*book).clone());

                match book {
                    Some(book) => {
                        let Ok(()) = self.open_book(book);
                    },
                    None => {},
                }
            },
            Item::BookStore => {
                let Ok(()) = spawn_detached(InternalProgram::Downloads, &[STORE_TAB]);
            },
        }

        Ok(Update::Redraw)
    }

    fn reader_key_pressed(&mut self, key: Keysym, room: Size<u32>) -> Result<Update, Never> {
        match self.annotations.writing {
            Some(_) => return self.write_key(key),
            None => {},
        }

        let Ok(turn) = turn_of(key);

        let Ok(meant) = console_panel::keys::meaning(key, console_panel::keys::Driving::Panel);
        let key = match meant {
            console_panel::keys::Meaning::More => Keysym::Tab,
            console_panel::keys::Meaning::Abandon
            | console_panel::keys::Meaning::Choose
            | console_panel::keys::Meaning::None
            | console_panel::keys::Meaning::Nudge(_)
            | console_panel::keys::Meaning::Close
            | console_panel::keys::Meaning::Step(_)
            | console_panel::keys::Meaning::Tab(_) => key,
        };

        match (self.annotations.tool, key, turn) {
            (_, Keysym::Tab, _) => self.annotate(AnnotationEvent::Walked),
            (Tool::Highlight | Tool::Text, Keysym::Up, _) => self.move_cursor(Turn::Back),
            (Tool::Highlight | Tool::Text, Keysym::Down, _) => self.move_cursor(Turn::Forward),
            (tool @ (Tool::Highlight | Tool::Text), Keysym::Return | Keysym::KP_Enter, _) => self.mark_cursor(tool),
            (_, _, Some(turn)) => self.turn_page(turn, room),
            (_, Keysym::Escape | Keysym::BackSpace, None) => {
                let Ok(page) = self.visible();

                match page {
                    Some(page) => self.annotate(AnnotationEvent::Backed(page)),
                    None => self.close_book(),
                }
            },
            (_, _, None) => Ok(Update::None),
        }
    }

    fn write_key(&mut self, key: Keysym) -> Result<Update, Never> {
        let typed = match key {
            Keysym::Escape => Some(Typed::Finished),
            Keysym::BackSpace => Some(Typed::Backspace),
            Keysym::Return | Keysym::KP_Enter => Some(Typed::Newline),
            key => key.key_char().filter(|letter| !letter.is_control()).map(Typed::Letter),
        };

        match typed {
            Some(typed) => self.annotate(AnnotationEvent::Typed(typed)),
            None => Ok(Update::None),
        }
    }

    fn visible(&self) -> Result<Option<Visible>, Never> {
        match &self.view {
            View::Reader(reader) => reader.visible().map(Some),
            View::Library => Ok(None),
        }
    }

    fn annotate(&mut self, event: AnnotationEvent) -> Result<Update, Never> {
        let Ok(empty) = Annotations::empty();
        let state = std::mem::replace(&mut self.annotations, empty);
        let Ok(Transition { state, effects }) = Annotating::transition(state, event);
        let mut update = Update::Redraw;

        self.annotations = state;

        for effect in effects {
            match effect {
                AnnotationEffect::Save(notes) => match &self.view {
                    View::Reader(reader) => {
                        let Ok(()) = reader.write_notes(&notes);
                    },
                    View::Library => {},
                },
                AnnotationEffect::ShowKeyboard => {
                    let Ok(()) = spawn_detached(InternalProgram::KeyboardShow, &[]);
                },
                AnnotationEffect::Back => {
                    let Ok(closed) = self.put_book_away();
                    let Ok(merged) = merge(update, closed);

                    update = merged;
                },
            }
        }

        Ok(update)
    }

    fn move_cursor(&mut self, way: Turn) -> Result<Update, Never> {
        let Ok(page) = self.visible();

        match page {
            Some(page) => self.annotate(AnnotationEvent::Picked { way, page }),
            None => Ok(Update::None),
        }
    }

    fn mark_cursor(&mut self, tool: Tool) -> Result<Update, Never> {
        let reader = match &self.view {
            View::Reader(reader) => reader,
            View::Library => return Ok(Update::None),
        };

        let Ok(text) = match self.annotations.cursor {
            Some(block) => reader.block_text(block),
            None => Ok(None),
        };

        let (block, end) = match (self.annotations.cursor, text.map(length)) {
            (Some(block), Some(Ok(end))) => (block, end),
            (None, _) | (_, None) => return Ok(Update::None),
        };

        let from = Spot { section: reader.section, block, start: 0 };
        let Ok(made) = reader.made(tool, from, Spot { start: end, ..from });

        match made {
            Some(made) => self.annotate(AnnotationEvent::Made(made)),
            None => Ok(Update::None),
        }
    }

    fn turn_page(&mut self, turn: Turn, room: Size<u32>) -> Result<Update, Never> {
        let reader = match &mut self.view {
            View::Reader(reader) => reader,
            View::Library => return Ok(Update::None),
        };

        let Ok(()) = reader.ensure_layout(room);
        let Ok(destination) = reader.turn(turn, room);

        match destination {
            Destination::AtEnd => return Ok(Update::None),
            Destination::Page(_) | Destination::Section { .. } => {},
        }

        let Ok(location) = reader.location();
        let name = reader.book.name.clone();
        let Ok(()) = self.library.save_location(&name, location);

        self.annotate(AnnotationEvent::Left)
    }

    fn pointer_event(&mut self, event: PointerEvent, room: Size<u32>) -> Result<Update, Never> {
        match event {
            PointerEvent::Down { at } => {
                let Ok(at) = point_of(at);

                self.pointer_down = Some(at);
                self.pointer_at = Some(at);

                self.begin_marking(at, room)
            },
            PointerEvent::Moved { at } => {
                let Ok(at) = point_of(at);

                self.pointer_at = Some(at);

                self.continue_marking(at, room)
            },
            PointerEvent::Pinched { by } => {
                self.pointer_down = None;

                let Ok(()) = self.stop_marking();

                self.pinched(by)
            },
            PointerEvent::Up => {
                self.pinch = None;

                let Ok(finished) = self.finish_marking(room);
                let lifted = (self.pointer_down.take(), self.pointer_at.take());

                match (finished, lifted) {
                    (Some(update), _) => Ok(update),
                    (None, (Some(from), Some(to))) => self.lifted(from, to, room),
                    (None, (Some(from), None)) => self.tap(from, room),
                    (None, (None, _)) => Ok(Update::None),
                }
            },
            PointerEvent::Left => {
                self.pointer_down = None;
                self.pointer_at = None;
                self.pinch = None;

                let Ok(()) = self.stop_marking();

                Ok(Update::Redraw)
            },
            PointerEvent::Scrolled { .. } => Ok(Update::None),
        }
    }

    fn lifted(&mut self, from: Point<i32>, to: Point<i32>, room: Size<u32>) -> Result<Update, Never> {
        let Ok(screen) = self.screen();
        let Ok(swiped) = reading::swipe(from, to);

        match (screen, swiped) {
            (Screen::Reader, Some(turn)) => self.turn_page(turn, room),
            (Screen::Reader, None) | (Screen::Library, _) => self.tap(from, room),
        }
    }

    fn pinched(&mut self, by: f64) -> Result<Update, Never> {
        let pinch = match self.pinch {
            Some(pinch) => Pinch { by: pinch.by * by, ..pinch },
            None => Pinch { from: self.appearance.text_size, by },
        };

        self.pinch = Some(pinch);

        let Ok(size) = pinch.from.scaled(pinch.by);

        self.resize_text(size)
    }

    fn zoom(&mut self, zoom: Zoom) -> Result<Update, Never> {
        let Ok(size) = match zoom {
            Zoom::ZoomedOut => self.appearance.text_size.smaller(),
            Zoom::ZoomedIn => self.appearance.text_size.larger(),
        };

        self.resize_text(size)
    }

    fn resize_text(&mut self, size: TextSize) -> Result<Update, Never> {
        let reader = match &mut self.view {
            View::Reader(reader) => reader,
            View::Library => return Ok(Update::None),
        };

        let reflows = match reader.content {
            Content::Publication { .. } => size != self.appearance.text_size,
            Content::PortableDocument { .. } | Content::Comic(_) => false,
        };

        match reflows {
            true => {},
            false => return Ok(Update::None),
        }

        self.appearance.text_size = size;
        reader.appearance.text_size = size;
        reader.layout_size = Size { width: 0, height: 0 };

        let Ok(()) = size.choose();

        Ok(Update::Redraw)
    }

    fn tap(&mut self, at: Point<i32>, room: Size<u32>) -> Result<Update, Never> {
        let Ok(screen) = self.screen();
        let Ok(corner) = close_button(room);
        let Ok(on_corner) = corner.covers(at);

        match (screen, on_corner) {
            (Screen::Library, console_core_shapes::Covers::Yes) => Ok(Update::Exit),
            (Screen::Reader, console_core_shapes::Covers::Yes) => self.close_book(),
            (Screen::Library, console_core_shapes::Covers::No) => self.library_tap(at, room),
            (Screen::Reader, console_core_shapes::Covers::No) => self.reader_tap(at, room),
        }
    }

    fn reader_tap(&mut self, at: Point<i32>, room: Size<u32>) -> Result<Update, Never> {
        let Ok(on_a_note) = self.note_part_at(at, room);

        match on_a_note {
            Some((spot, NotePart::Icon)) => return self.annotate(AnnotationEvent::Opened(spot)),
            Some((spot, NotePart::Close)) => return self.annotate(AnnotationEvent::Closed(spot)),
            Some((spot, NotePart::Card)) => return self.annotate(AnnotationEvent::Tapped(spot)),
            None => {},
        }

        let Ok(control) = control_at(at, room);

        match (control, &self.annotations.writing) {
            (Some(Control::Close), _) => self.close_book(),
            (Some(Control::Turn(turn)), _) => self.turn_page(turn, room),
            (Some(Control::Zoom(zoom)), _) => self.zoom(zoom),
            (Some(Control::Rail(rail)), _) => self.press(rail, room),
            (None, Some(_)) => self.annotate(AnnotationEvent::Dropped),
            (None, None) => Ok(Update::None),
        }
    }

    fn note_part_at(&self, at: Point<i32>, room: Size<u32>) -> Result<Option<(Spot, NotePart)>, Never> {
        let reader = match &self.view {
            View::Reader(reader) => reader,
            View::Library => return Ok(None),
        };

        let Ok(frames) = note_frames(reader, &self.annotations, room);

        note_part(&frames, at)
    }

    fn press(&mut self, rail: Rail, room: Size<u32>) -> Result<Update, Never> {
        let reader = match &mut self.view {
            View::Reader(reader) => reader,
            View::Library => return Ok(Update::None),
        };

        let Ok(()) = reader.ensure_layout(room);
        let Ok(page) = reader.visible();

        self.annotate(AnnotationEvent::Pressed { rail, page })
    }

    fn begin_marking(&mut self, at: Point<i32>, room: Size<u32>) -> Result<Update, Never> {
        let Ok(control) = control_at(at, room);
        let Ok(on_a_note) = self.note_part_at(at, room);
        let tool = self.annotations.tool;

        let reader = match &self.view {
            View::Reader(reader) => reader,
            View::Library => return Ok(Update::None),
        };

        let touch = match (on_a_note, control, tool) {
            (Some((spot, _)), _, Tool::Eraser) => Touch::Erasing(Some(Target::Note(spot))),
            (Some(_), _, _) | (None, Some(_), _) | (None, None, Tool::Read) => return Ok(Update::None),
            (None, None, Tool::Highlight | Tool::Text) => {
                let Ok(spot) = reader.spot_at(at, room);

                match spot {
                    Some(spot) => Touch::Selecting(spot),
                    None => return Ok(Update::None),
                }
            },
            (None, None, Tool::Pen) => Touch::Drawing(at),
            (None, None, Tool::Eraser) => {
                let Ok(target) = reader.target_under(&self.annotations.notes, at, room);

                Touch::Erasing(target)
            },
        };

        self.annotate(AnnotationEvent::Touched(touch))
    }

    fn continue_marking(&mut self, at: Point<i32>, room: Size<u32>) -> Result<Update, Never> {
        let reader = match &self.view {
            View::Reader(reader) => reader,
            View::Library => return Ok(Update::None),
        };

        let touch = match &self.annotations.gesture {
            Gesture::None => return Ok(Update::None),
            Gesture::Selecting { .. } => {
                let Ok(spot) = reader.spot_at(at, room);

                match spot {
                    Some(spot) => Touch::Selecting(spot),
                    None => return Ok(Update::None),
                }
            },
            Gesture::Drawing(_) => Touch::Drawing(at),
            Gesture::Erasing => {
                let Ok(target) = reader.target_under(&self.annotations.notes, at, room);

                Touch::Erasing(target)
            },
        };

        self.annotate(AnnotationEvent::Moved(touch))
    }

    fn stop_marking(&mut self) -> Result<(), Never> {
        match self.annotations.gesture {
            Gesture::None => {},
            Gesture::Selecting { .. } | Gesture::Drawing(_) | Gesture::Erasing => {
                let Ok(_update) = self.annotate(AnnotationEvent::Dropped);
            },
        }

        Ok(())
    }

    fn finish_marking(&mut self, room: Size<u32>) -> Result<Option<Update>, Never> {
        let reader = match &self.view {
            View::Reader(reader) => reader,
            View::Library => return Ok(None),
        };

        let Ok(made) = match (&self.annotations.gesture, self.annotations.tool) {
            (Gesture::None, _) => return Ok(None),
            (Gesture::Selecting { from, to }, tool) => reader.made(tool, *from, *to),
            (Gesture::Drawing(stroke), _) => reader.handwriting(&self.annotations.notes, stroke, self.annotations.scope, room),
            (Gesture::Erasing, _) => Ok(None),
        };

        let event = match made {
            Some(made) => AnnotationEvent::Made(made),
            None => AnnotationEvent::Dropped,
        };

        self.annotate(event).map(Some)
    }

    fn library_tap(&mut self, at: Point<i32>, room: Size<u32>) -> Result<Update, Never> {
        let Ok(search) = search_field(room);
        let Ok(on_search) = search.covers(at);

        match on_search {
            console_core_shapes::Covers::Yes => {
                let Ok(()) = spawn_detached(InternalProgram::KeyboardToggle, &[]);

                return Ok(Update::None);
            },
            console_core_shapes::Covers::No => {},
        }

        let Ok(grid) = grid::layout(room);
        let Ok(visible) = self.library.visible_books();
        let Ok(count) = fitted::<_, u32>(visible.len());
        let Ok(hit) = cover_at(grid, at, Viewport { count: count.saturating_add(1), scroll: self.library.scroll });

        match hit {
            Some(index) => {
                self.library.selected = index;

                let Ok(item) = self.selected_item();

                self.activate(item)
            },
            None => Ok(Update::None),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Viewport {
    count: u32,
    scroll: ScrollOffset,
}

fn visible_range(grid: Grid, viewport: Viewport) -> Result<std::ops::Range<u32>, Never> {
    let ScrollOffset(first_row) = viewport.scroll;
    let from = first_row.saturating_mul(grid.columns.get());
    let rows = grid.rows_seen.get().saturating_add(1);
    let past = from.saturating_add(rows.saturating_mul(grid.columns.get())).min(viewport.count);

    Ok(from..past)
}

fn cover_panel(grid: Grid, at: Point<i32>) -> Result<Panel, Never> {
    Ok(Panel { at, size: grid.cover, round: Round(6), fill: Oklch { lightness: 0.0, chroma: 0.0, hue: 0.0 }, edge: Edge::None })
}

fn cover_at(grid: Grid, at: Point<i32>, viewport: Viewport) -> Result<Option<u32>, Never> {
    let Ok(range) = visible_range(grid, viewport);

    for index in range {
        let Ok(placed) = grid::position(grid, index, viewport.scroll);
        let Ok(panel) = cover_panel(grid, placed);
        let Ok(covers) = panel.covers(at);

        match covers {
            console_core_shapes::Covers::Yes => return Ok(Some(index)),
            console_core_shapes::Covers::No => {},
        }
    }

    Ok(None)
}

fn close_button(room: Size<u32>) -> Result<Panel, Never> {
    let Ok(across) = fitted::<u32, i32>(room.width.saturating_sub(CORNER));

    Ok(Panel {
        at: Point { x: across, y: 0 },
        size: Size { width: CORNER, height: CORNER },
        round: Round(0),
        fill: Oklch { lightness: 0.0, chroma: 0.0, hue: 0.0 },
        edge: Edge::None,
    })
}

fn search_field(room: Size<u32>) -> Result<Panel, Never> {
    let wide = room.width.saturating_div(3).clamp(240, 460);
    let Ok(across) = fitted::<u32, i32>(room.width.saturating_sub(wide).saturating_div(2));

    Ok(Panel {
        at: Point { x: across, y: 18 },
        size: Size { width: wide, height: 40 },
        round: Round(20),
        fill: Oklch { lightness: 0.0, chroma: 0.0, hue: 0.0 },
        edge: Edge::None,
    })
}

fn control_at(at: Point<i32>, room: Size<u32>) -> Result<Option<Control>, Never> {
    let Ok(close) = close_button(room);
    let mut controls = vec![(close, Control::Close)];

    for (index, rail) in (0_u32..).zip(RAIL) {
        let Ok(button) = rail_button(room, index);

        controls.push((button, Control::Rail(rail)));
    }

    for turn in [Turn::Back, Turn::Forward] {
        let Ok(target) = turn_target(room, turn);

        controls.push((target, Control::Turn(turn)));
    }

    for zoom in [Zoom::ZoomedOut, Zoom::ZoomedIn] {
        let Ok(button) = zoom_button(zoom);

        controls.push((button, Control::Zoom(zoom)));
    }

    Ok(controls.into_iter().find_map(|(panel, control)| match panel.covers(at) {
        Ok(console_core_shapes::Covers::Yes) => Some(control),
        Ok(console_core_shapes::Covers::No) => None,
    }))
}

fn rail_button(room: Size<u32>, index: u32) -> Result<Panel, Never> {
    let Ok(column) = column(room);
    let Ok(across) = fitted::<u32, i32>(column.left.saturating_sub(RAIL_BUTTON).saturating_div(2));
    let Ok(down) = fitted::<u32, i32>(RAIL_TOP.saturating_add(index.saturating_mul(RAIL_BUTTON.saturating_add(RAIL_GAP))));

    Ok(Panel {
        at: Point { x: across, y: down },
        size: Size { width: RAIL_BUTTON, height: RAIL_BUTTON },
        round: Round(RAIL_BUTTON.saturating_div(2)),
        fill: Oklch { lightness: 0.0, chroma: 0.0, hue: 0.0 },
        edge: Edge::None,
    })
}

fn zoom_button(zoom: Zoom) -> Result<Panel, Never> {
    let across = match zoom {
        Zoom::ZoomedOut => 0,
        Zoom::ZoomedIn => CORNER,
    };
    let Ok(across) = fitted::<u32, i32>(across);

    Ok(Panel {
        at: Point { x: across, y: 0 },
        size: Size { width: CORNER, height: CORNER },
        round: Round(0),
        fill: Oklch { lightness: 0.0, chroma: 0.0, hue: 0.0 },
        edge: Edge::None,
    })
}

fn turn_bar(room: Size<u32>, turn: Turn) -> Result<Panel, Never> {
    let Ok(column) = column(room);
    let inset = column.left.saturating_sub(TURN_BAR.width).saturating_div(2);
    let across = match turn {
        Turn::Back => inset,
        Turn::Forward => room.width.saturating_sub(inset).saturating_sub(TURN_BAR.width),
    };
    let Ok(across) = fitted::<u32, i32>(across);
    let centered = room.height.saturating_sub(TURN_BAR.height).saturating_div(2);
    let Ok(rail) = fitted::<_, u32>(RAIL.len());
    let under_the_rail = RAIL_TOP.saturating_add(rail.saturating_mul(RAIL_BUTTON.saturating_add(RAIL_GAP))).saturating_add(TURN_BAR.height.saturating_div(2));
    let Ok(down) = fitted::<u32, i32>(centered.max(under_the_rail));

    Ok(Panel { at: Point { x: across, y: down }, size: TURN_BAR, round: Round(11), fill: Oklch { lightness: 0.0, chroma: 0.0, hue: 0.0 }, edge: Edge::None })
}

fn turn_target(room: Size<u32>, turn: Turn) -> Result<Panel, Never> {
    let Ok(column) = column(room);
    let Ok(bar) = turn_bar(room, turn);
    let tall = TURN_BAR.height.saturating_mul(2);
    let Ok(lift) = fitted::<u32, i32>(TURN_BAR.height.saturating_div(2));
    let across = match turn {
        Turn::Back => 0,
        Turn::Forward => column.left.saturating_add(column.width),
    };
    let Ok(across) = fitted::<u32, i32>(across);

    Ok(Panel {
        at: Point { x: across, y: bar.at.y.saturating_sub(lift) },
        size: Size { width: room.width.saturating_sub(column.width).saturating_div(2), height: tall },
        ..bar
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Column {
    left: u32,
    width: u32,
}

fn column(room: Size<u32>) -> Result<Column, Never> {
    let wide = room.width.saturating_sub(READING_EDGE.saturating_mul(2)).min(READING_WIDE);
    let left = room.width.saturating_sub(wide).saturating_div(2);

    Ok(Column { left, width: wide })
}

fn body_font(appearance: Appearance) -> Result<(Font, Weight), Never> {
    let Ok(family) = appearance.typeface.family();
    let Ok(font) = font(family, appearance.text_size.0);

    Ok((font, Weight::Plain))
}

fn heading_font(appearance: Appearance) -> Result<(Font, Weight), Never> {
    let Ok(family) = appearance.typeface.family();
    let Ok(tall) = appearance.text_size.heading();
    let Ok(font) = font(family, tall);

    Ok((font, Weight::Bold))
}

fn style_font(style: Style, appearance: Appearance) -> Result<(Font, Weight), Never> {
    match style {
        Style::Body => body_font(appearance),
        Style::Heading => heading_font(appearance),
    }
}

fn line_height(style: Style, appearance: Appearance) -> Result<u32, Never> {
    let Ok((font, weight)) = style_font(style, appearance);
    let Ok(measured) = console_draw_painting::measure_text(Run { said: "Ag", weight, width: 4096 }, &font);

    Ok(measured.height.saturating_mul(5).saturating_div(4))
}

fn text_height(room: Size<u32>) -> Result<u32, Never> {
    Ok(room.height.saturating_sub(READING_EDGE.saturating_mul(2)).saturating_sub(24))
}

fn chapter_blocks(book: &OpenPublication, section: u32) -> Result<Vec<Block>, Never> {
    let Ok(at) = console_core_number_conversion::index(section);

    Ok(match book.publication.chapters.get(at).map(|chapter| book.archive.text(chapter)) {
        Some(Ok(text)) => {
            let Ok(blocks) = flow::blocks(&text);

            blocks
        },
        Some(Err(fault)) => vec![Block::Paragraph(format!("This chapter could not be read: {fault}"))],
        None => Vec::new(),
    })
}

fn paginate_chapter(blocks: &[Block], room: Size<u32>, appearance: Appearance) -> Result<Vec<Page>, Never> {
    let Ok(column) = column(room);
    let Ok(height) = text_height(room);
    let Ok(line) = line_height(Style::Body, appearance);
    let Ok(heading) = line_height(Style::Heading, appearance);
    let layout = Layout { height, line_height: line, heading_height: heading };

    let wrap = |text: &str, style: Style| -> Vec<String> {
        let Ok((font, weight)) = style_font(style, appearance);
        let Ok(lines) = console_draw_painting::wrapped(Run { said: text, weight, width: column.width }, &font);

        lines
    };

    pages::paginate(blocks, layout, &wrap)
}

fn document_page_count(at: &Path) -> Result<u32, BookError> {
    let Ok(mut asking) = Program::Pdfinfo.command();
    let text = asking.arg(at).output().map_err(|fault| BookError::Reading(at.to_path_buf(), fault))?;
    let text = String::from_utf8_lossy(&text.stdout).into_owned();

    let pages = text
        .lines()
        .find_map(|line| line.strip_prefix("Pages:"))
        .map(|count| count.trim().parse::<u32>());

    match pages {
        Some(Ok(pages)) => Ok(pages.max(1)),
        Some(Err(_)) | None => Err(BookError::NotFound("page count".to_string())),
    }
}

fn through_page(pages: &[Page], page: u32) -> Result<u32, Never> {
    let Ok(page) = console_core_number_conversion::index(page);
    let (before, total) = pages.iter().enumerate().fold((0_u64, 0_u64), |(before, total), (at, each)| {
        let Ok(letters) = pages::letters(each);

        match at < page {
            true => (before.saturating_add(letters), total.saturating_add(letters)),
            false => (before, total.saturating_add(letters)),
        }
    });

    progress::through_text(progress::Reached { before, total })
}

struct Stored {
    cache: PathBuf,
    notes_at: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Restore {
    None,
    Edge(End),
    Fraction(Fraction),
}

impl Reader {
    fn open(book: Book, location: Option<Location>, kept: Stored, appearance: Appearance) -> Result<Reader, BookError> {
        let content = match book.format {
            Format::Publication => {
                let opened = open::publication(&book.path)?;

                Content::Publication { book: opened, blocks: Vec::new(), pages: Vec::new() }
            },
            Format::PortableDocument => {
                let pages = document_page_count(&book.path)?;

                Content::PortableDocument { pages }
            },
            Format::Comic => {
                let comic = open::comic(&book.path)?;

                Content::Comic(comic)
            },
        };

        let (section, restore) = match location {
            Some(location) => (location.section, Restore::Fraction(Fraction(location.fraction))),
            None => (0, Restore::None),
        };

        let mut reader = Reader {
            book,
            content,
            section: 0,
            page: 0,
            layout_size: Size { width: 0, height: 0 },
            appearance,
            cached_picture: None,
            restore,
            cache: kept.cache,
            notes_at: Some(kept.notes_at),
            live: LiveText::Unasked,
            languages: None,
        };

        let Ok(sections) = reader.sections();

        reader.section = section.min(sections.saturating_sub(1));

        Ok(reader)
    }

    fn sections(&self) -> Result<u32, Never> {
        let Ok(count) = match &self.content {
            Content::Publication { book, .. } => fitted::<_, u32>(book.publication.chapters.len()),
            Content::PortableDocument { pages } => Ok(*pages),
            Content::Comic(comic) => fitted::<_, u32>(comic.pages.len()),
        };

        Ok(count)
    }

    fn pages(&self) -> Result<u32, Never> {
        match &self.content {
            Content::Publication { pages, .. } => fitted::<_, u32>(pages.len()),
            Content::PortableDocument { .. } | Content::Comic(_) => Ok(1),
        }
    }

    fn letters(&self) -> Result<Vec<u64>, Never> {
        Ok(match &self.content {
            Content::Publication { pages, .. } => pages.iter().map(|page| {
                let Ok(letters) = pages::letters(page);

                letters
            }).collect(),
            Content::PortableDocument { .. } | Content::Comic(_) => vec![1],
        })
    }

    fn ensure_layout(&mut self, room: Size<u32>) -> Result<(), Never> {
        match &mut self.content {
            Content::Publication { book, blocks, pages } => {
                match (self.layout_size == room, pages.is_empty()) {
                    (true, false) => return Ok(()),
                    (true, true) | (false, _) => {},
                }

                let kept = match self.restore {
                    Restore::None => {
                        let Ok(kept) = through_page(pages, self.page);

                        Restore::Fraction(Fraction(kept))
                    },
                    Restore::Edge(end) => Restore::Edge(end),
                    Restore::Fraction(fraction) => Restore::Fraction(fraction),
                };

                let Ok(read) = chapter_blocks(book, self.section);
                let Ok(laid) = paginate_chapter(&read, room, self.appearance);

                *blocks = read;
                *pages = laid;
                self.restore = kept;
            },
            Content::PortableDocument { .. } | Content::Comic(_) => {},
        }

        self.layout_size = room;

        self.restore_position()
    }

    fn restore_position(&mut self) -> Result<(), Never> {
        let Ok(pages) = self.pages();

        self.page = match self.restore {
            Restore::None | Restore::Edge(End::First) => 0,
            Restore::Edge(End::Last) => pages.saturating_sub(1),
            Restore::Fraction(fraction) => {
                let Ok(letters) = self.letters();
                let Ok(page) = progress::page_holding(fraction, &letters);

                page.min(pages.saturating_sub(1))
            },
        };

        self.restore = Restore::None;

        Ok(())
    }

    fn turn(&mut self, turn: Turn, room: Size<u32>) -> Result<Destination, Never> {
        let Ok(sections) = self.sections();
        let Ok(pages) = self.pages();
        let position = PagePosition {
            section: Position { index: self.section, count: sections },
            page: Position { index: self.page, count: pages },
        };
        let Ok(destination) = reading::turn(position, turn);

        match destination {
            Destination::Page(page) => self.page = page,
            Destination::Section { section, end } => {
                self.section = section;
                self.restore = Restore::Edge(end);
                self.layout_size = Size { width: 0, height: 0 };

                let Ok(()) = self.ensure_layout(room);
            },
            Destination::AtEnd => {},
        }

        Ok(destination)
    }

    fn location(&self) -> Result<Location, Never> {
        let Ok(sections) = self.sections();
        let Ok(pages) = self.pages();
        let Ok(fraction) = match &self.content {
            Content::Publication { pages, .. } => through_page(pages, self.page),
            Content::PortableDocument { .. } | Content::Comic(_) => Ok(0),
        };
        let Ok(read) = progress::fraction(Position { index: self.page.saturating_add(1), count: pages });
        let Ok(percent) = progress::percent(Position { index: self.section, count: sections }, Fraction(read));

        Ok(Location { section: self.section, fraction, percent })
    }
}

fn background(room: Size<u32>, fill: Oklch) -> Result<Shape, Never> {
    Ok(Shape::Panel(Panel { at: Point { x: 0, y: 0 }, size: room, round: Round(0), fill, edge: Edge::None }))
}

fn text_shape(text: &str, at: Point<i32>, style: TextStyle, ink: Oklch) -> Result<Shape, Never> {
    let Ok(font) = font(SANS, style.size);

    Ok(Shape::Text(Text { at, width: style.width, said: text.to_string(), weight: style.weight, font, ink }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TextStyle {
    size: u32,
    weight: Weight,
    width: u32,
}

fn offset(at: Point<i32>, by: Point<u32>) -> Result<Point<i32>, Never> {
    let Ok(across) = fitted::<u32, i32>(by.x);
    let Ok(down) = fitted::<u32, i32>(by.y);

    Ok(Point { x: at.x.saturating_add(across), y: at.y.saturating_add(down) })
}

fn centered_lines(text: &str, box_at: Point<i32>, box_wide: u32, style: TextStyle) -> Result<Vec<(String, Point<i32>)>, Never> {
    let Ok(font) = font(SANS, style.size);
    let Ok(lines) = console_draw_painting::wrapped(Run { said: text, weight: style.weight, width: box_wide }, &font);
    let Ok(line) = line_height_of(&font, style.weight);
    let mut placed = Vec::new();

    for (row, line_text) in (0u32..).zip(lines.into_iter().take(2)) {
        let Ok(measured) = console_draw_painting::measure_text(Run { said: &line_text, weight: style.weight, width: box_wide }, &font);
        let left = box_wide.saturating_sub(measured.width).saturating_div(2);
        let Ok(at) = offset(box_at, Point { x: left, y: row.saturating_mul(line) });

        placed.push((line_text, at));
    }

    Ok(placed)
}

fn line_height_of(font: &Font, weight: Weight) -> Result<u32, Never> {
    let Ok(measured) = console_draw_painting::measure_text(Run { said: "Ag", weight, width: 4096 }, font);

    Ok(measured.height)
}

struct CoverItem<'a> {
    book: &'a Book,
    at: Point<i32>,
    highlight: Highlight,
    percent: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Highlight {
    On,
    Off,
}

fn highlight_shape(grid: Grid, at: Point<i32>, fill: Oklch) -> Result<Shape, Never> {
    Ok(Shape::Panel(Panel {
        at: Point { x: at.x.saturating_sub(5), y: at.y.saturating_sub(5) },
        size: Size { width: grid.cover.width.saturating_add(10), height: grid.cover.height.saturating_add(10) },
        round: Round(10),
        fill,
        edge: Edge::None,
    }))
}

fn cover_shapes(drawing: &CoverItem<'_>, grid: Grid, side: Side, colors: &Wearing) -> Result<Vec<Shape>, Never> {
    let mut shapes = Vec::new();

    match drawing.highlight {
        Highlight::On => {
            let Ok(shape) = highlight_shape(grid, drawing.at, colors.pink);

            shapes.push(shape)
        },
        Highlight::Off => {},
    }

    let pixels = match &drawing.book.cover {
        Some(cover) => {
            let Ok(pixels) = store::pixels_at(cover, side);

            pixels
        },
        None => None,
    };

    match pixels {
        Some(pixels) => shapes.push(Shape::Picture(Picture { at: drawing.at, size: grid.cover, pixels })),
        None => {
            let Ok(panel) = cover_panel(grid, drawing.at);

            shapes.push(Shape::Panel(Panel { fill: colors.panel, ..panel }));
        },
    }

    let Ok(title) = title_band(drawing, grid, colors);

    shapes.extend(title);

    let Ok(badge) = percent_badge(drawing, colors);

    shapes.extend(badge);

    Ok(shapes)
}

fn title_band(drawing: &CoverItem<'_>, grid: Grid, colors: &Wearing) -> Result<Vec<Shape>, Never> {
    let style = TextStyle { size: 13, weight: Weight::Bold, width: grid.cover.width.saturating_sub(12) };
    let Ok(font) = font(SANS, style.size);
    let Ok(line) = line_height_of(&font, style.weight);
    let band = line.saturating_mul(TITLE_LINES).saturating_add(12);
    let Ok(band_at) = offset(drawing.at, Point { x: 0, y: grid.cover.height.saturating_sub(band) });
    let mut shapes = vec![Shape::Panel(Panel {
        at: band_at,
        size: Size { width: grid.cover.width, height: band },
        round: Round(0),
        fill: colors.night,
        edge: Edge::None,
    })];
    let Ok(text_at) = offset(band_at, Point { x: 6, y: 6 });
    let Ok(lines) = centered_lines(&drawing.book.title, text_at, style.width, style);

    for (text, at) in lines {
        let Ok(shape) = text_shape(&text, at, TextStyle { width: style.width.saturating_add(8), ..style }, colors.text);

        shapes.push(shape);
    }

    Ok(shapes)
}

fn percent_badge(drawing: &CoverItem<'_>, colors: &Wearing) -> Result<Vec<Shape>, Never> {
    let percent = match drawing.percent {
        Some(percent) => percent,
        None => return Ok(Vec::new()),
    };

    let Ok(badge_at) = offset(drawing.at, Point { x: 6, y: 0 });
    let Ok(text_at) = offset(badge_at, Point { x: 5, y: 3 });
    let style = TextStyle { size: 11, weight: Weight::Bold, width: 60 };
    let Ok(text) = text_shape(&format!("{percent}%"), text_at, style, colors.text);

    Ok(vec![
        Shape::Panel(Panel { at: badge_at, size: Size { width: 40, height: 20 }, round: Round(0), fill: colors.night, edge: Edge::None }),
        text,
    ])
}

fn store_tile(grid: Grid, at: Point<i32>, highlight: Highlight, colors: &Wearing) -> Result<Vec<Shape>, Never> {
    let mut shapes = Vec::new();

    match highlight {
        Highlight::On => {
            let Ok(shape) = highlight_shape(grid, at, colors.pink);

            shapes.push(shape)
        },
        Highlight::Off => {},
    }

    let Ok(panel) = cover_panel(grid, at);

    shapes.push(Shape::Panel(Panel { fill: colors.panel, edge: Edge::Of { wide: 2, color: colors.soft }, ..panel }));

    let middle = grid.cover.height.saturating_div(2);
    let Ok(plus_at) = offset(at, Point { x: 0, y: middle.saturating_sub(60) });
    let Ok(plus) = centered_lines("+", plus_at, grid.cover.width, TextStyle { size: 48, weight: Weight::Plain, width: grid.cover.width });
    let Ok(named_at) = offset(at, Point { x: 0, y: middle.saturating_add(8) });
    let named_style = TextStyle { size: 15, weight: Weight::Bold, width: grid.cover.width };
    let Ok(named) = centered_lines(BOOK_STORE, named_at, grid.cover.width, named_style);

    for (text, at) in plus {
        {
            let Ok(shape) = text_shape(&text, at, TextStyle { size: 48, weight: Weight::Plain, width: grid.cover.width }, colors.text);

            shapes.push(shape)
        };
    }

    for (text, at) in named {
        {
            let Ok(shape) = text_shape(&text, at, named_style, colors.text);

            shapes.push(shape)
        };
    }

    Ok(shapes)
}

fn header(library: &Library, room: Size<u32>, colors: &Wearing) -> Result<Vec<Shape>, Never> {
    let Ok(search) = search_field(room);
    let search_style = TextStyle { size: 16, weight: Weight::Plain, width: search.size.width.saturating_sub(40) };
    let (typed, ink) = match library.search.is_empty() {
        true => (SEARCH.to_string(), colors.soft),
        false => (library.search.clone(), colors.text),
    };
    let shown = format!("\u{2315}  {typed}");
    let Ok(typed_at) = typed_at(&search, &shown, search_style);
    let Ok(close_across) = fitted::<u32, i32>(room.width.saturating_sub(grid::MARGIN).saturating_sub(22));
    let title_style = TextStyle { size: 26, weight: Weight::Bold, width: 400 };
    let close_style = TextStyle { size: 22, weight: Weight::Plain, width: 40 };

    let Ok(title) = text_shape(LIBRARY, Point { x: 32, y: 20 }, title_style, colors.text);
    let Ok(typed) = text_shape(&shown, typed_at, search_style, ink);
    let Ok(close) = text_shape(CLOSE, Point { x: close_across, y: 22 }, close_style, colors.text);

    Ok(vec![title, Shape::Panel(Panel { fill: colors.panel, ..search }), typed, close])
}

fn typed_at(search: &Panel, shown: &str, style: TextStyle) -> Result<Point<i32>, Never> {
    let Ok(font) = font(SANS, style.size);
    let Ok(measured) = console_draw_painting::measure_text(Run { said: shown, weight: style.weight, width: style.width }, &font);
    let down = search.size.height.saturating_sub(measured.height).saturating_div(2);

    offset(search.at, Point { x: 20, y: down })
}

fn request_missing_covers(visible: &[&Book], side: Side) -> Result<(), Never> {
    let wanted: Vec<String> = visible
        .iter()
        .filter_map(|book| book.cover.as_ref())
        .map(|cover| cover.display().to_string())
        .collect();
    let Ok(missing) = store::missing_at(&wanted, side);

    match missing.is_empty() {
        true => Ok(()),
        false => store::make_at(&missing, side),
    }
}

fn library_shapes(app: &App, room: Size<u32>, device: Size<u32>) -> Result<Vec<Shape>, Never> {
    let colors = &app.wearing;
    let library = &app.library;
    let Ok(grid) = grid::layout(room);
    let Ok(visible) = library.visible_books();
    let Ok(count) = fitted::<_, u32>(visible.len());
    let Ok(side) = cover_pixel_size(grid, room, device);
    let Ok(()) = request_missing_covers(&visible, side);
    let Ok(mut shapes) = header(library, room, colors);
    let viewport = Viewport { count: count.saturating_add(1), scroll: library.scroll };
    let Ok(range) = visible_range(grid, viewport);

    let Ok(ground) = background(room, colors.ground);

    shapes.insert(0, ground);

    for index in range {
        let Ok(at) = grid::position(grid, index, library.scroll);
        let highlight = match index == library.selected {
            true => Highlight::On,
            false => Highlight::Off,
        };
        let Ok(position) = console_core_number_conversion::index(index);

        let drawn = match visible.get(position) {
            Some(book) => {
                let percent = library.locations.get(&book.name).map(|location| location.percent);
                let drawing = CoverItem { book, at, highlight, percent };

                cover_shapes(&drawing, grid, side, colors)
            },
            None => store_tile(grid, at, highlight, colors),
        };

        let Ok(drawn) = drawn;

        shapes.extend(drawn);
    }

    Ok(shapes)
}

fn cover_pixel_size(grid: Grid, room: Size<u32>, device: Size<u32>) -> Result<Side, Never> {
    let tall = u64::from(grid.cover.height).saturating_mul(u64::from(device.height));
    let scaled = tall.checked_div(u64::from(room.height)).map(u32::try_from);

    Ok(Side(match scaled {
        Some(Ok(tall)) => tall,
        None => grid.cover.height,
        Some(Err(_too_tall_to_count)) => grid.cover.height,
    }))
}

fn decode_image(cache: &Path, named: &str, bytes: &[u8], within: Size<u32>) -> Result<Option<Pixels>, Never> {
    let Ok(ending) = open::extension(named);
    let at = cache.join(format!("page.{ending}"));

    match std::fs::create_dir_all(cache).map(|()| console_core_atomic_writes::whole(&at, bytes)) {
        Ok(Ok(())) => {},
        Ok(Err(fault)) => {
            eprintln!("books: {fault}");

            return Ok(None);
        },
        Err(fault) => {
            eprintln!("books: {}: {fault}", cache.display());

            return Ok(None);
        },
    }

    Ok(match console_pictures::decoded(&at, within) {
        Ok(pixels) => pixels,
        Err(fault) => {
            eprintln!("books: {fault}");

            None
        },
    })
}

fn render_document_page(at: &Path, page: u32, within: Size<u32>) -> Result<Option<Pixels>, Never> {
    let Ok(mut drawing) = Program::Pdftoppm.command();
    let number = page.saturating_add(1).to_string();
    let tall = within.height.to_string();

    let drawn = drawing
        .args(["-f", &number, "-l", &number, "-scale-to-y", &tall, "-scale-to-x", "-1"])
        .arg(at)
        .output();

    Ok(match drawn {
        Ok(drawn) => {
            let Ok(pixels) = console_pictures::from_portable_pixmap(&drawn.stdout);

            pixels
        },
        Err(fault) => {
            eprintln!("books: poppler would not draw {}: {fault}", at.display());

            None
        },
    })
}

impl Reader {
    fn current_picture(&mut self, within: Size<u32>) -> Result<Option<Pixels>, Never> {
        let key = (self.section, self.page);

        match &self.cached_picture {
            Some((cached, pixels)) => match *cached == key {
                true => return Ok(Some(pixels.clone())),
                false => {},
            },
            None => {},
        }

        let Ok(pixels) = self.load_picture(within);

        self.cached_picture = pixels.clone().map(|pixels| (key, pixels));

        Ok(pixels)
    }

    fn load_picture(&self, within: Size<u32>) -> Result<Option<Pixels>, Never> {
        let Ok(section) = console_core_number_conversion::index(self.section);
        let Ok(page) = console_core_number_conversion::index(self.page);

        let (named, archive) = match &self.content {
            Content::PortableDocument { .. } => return render_document_page(&self.book.path, self.section, within),
            Content::Comic(comic) => match comic.pages.get(section) {
                Some(named) => (named.clone(), &comic.archive),
                None => return Ok(None),
            },
            Content::Publication { book, pages, .. } => match (pages.get(page), book.publication.chapters.get(section)) {
                (Some(Page::Picture(link)), Some(chapter)) => {
                    let Ok(named) = flow::resolve(Relative { base: chapter, path: link });

                    (named, &book.archive)
                },
                (Some(Page::Text(_)) | None, _) | (_, None) => return Ok(None),
            },
        };

        match archive.extract(&named) {
            Ok(bytes) => decode_image(&self.cache, &named, &bytes, within),
            Err(fault) => {
                eprintln!("books: {fault}");

                Ok(None)
            },
        }
    }
}

impl Reader {
    fn read_notes(&mut self) -> Result<Vec<Note>, Never> {
        let at = match &self.notes_at {
            Some(at) => at.clone(),
            None => return Ok(Vec::new()),
        };

        let Ok(contents) = console_core_atomic_writes::read(&at);

        Ok(match contents.text() {
            Ok(Some(text)) => {
                let Ok(read) = notes::parse(&text);

                read
            },
            Ok(None) => Vec::new(),
            Err(fault) => {
                eprintln!("books: {}: {fault}; nothing will be written over it", at.display());

                self.notes_at = None;

                Vec::new()
            },
        })
    }

    fn write_notes(&self, notes: &[Note]) -> Result<(), Never> {
        let at = match &self.notes_at {
            Some(at) => at,
            None => {
                eprintln!("books: the notes were not kept, because the file already there could not be read");

                return Ok(());
            },
        };

        let Ok(text) = notes::serialize(&self.book.title, notes);

        match at.parent().map(std::fs::create_dir_all) {
            Some(Err(fault)) => eprintln!("books: {}: {fault}", at.display()),
            Some(Ok(())) | None => {},
        }

        match console_core_atomic_writes::whole(at, text.as_bytes()) {
            Ok(()) => {},
            Err(fault) => eprintln!("books: the notes were not kept: {fault}"),
        }

        Ok(())
    }

    fn page_lines(&self) -> Result<Vec<pages::Line>, Never> {
        let Ok(page) = console_core_number_conversion::index(self.page);

        Ok(match &self.content {
            Content::Publication { pages, .. } => match pages.get(page) {
                Some(Page::Text(lines)) => lines.clone(),
                Some(Page::Picture(_)) | None => Vec::new(),
            },
            Content::PortableDocument { .. } | Content::Comic(_) => Vec::new(),
        })
    }

    fn block_text(&self, block: u32) -> Result<Option<&str>, Never> {
        let Ok(at) = console_core_number_conversion::index(block);

        Ok(match &self.content {
            Content::Publication { blocks, .. } => match blocks.get(at) {
                Some(Block::Heading(text) | Block::Paragraph(text)) => Some(text.as_str()),
                Some(Block::Picture(_)) | None => None,
            },
            Content::PortableDocument { .. } | Content::Comic(_) => {
                let Ok(paragraphs) = self.paragraphs();

                paragraphs.and_then(|paragraphs| paragraphs.get(at)).map(|paragraph| paragraph.text.as_str())
            },
        })
    }

    fn paragraphs(&self) -> Result<Option<&[Paragraph]>, Never> {
        Ok(match &self.live {
            LiveText::Read { section, paragraphs } => match *section == self.section {
                true => Some(paragraphs.as_slice()),
                false => None,
            },
            LiveText::Unasked | LiveText::Recognizing { .. } => None,
        })
    }

    fn picture_frame(&self, room: Size<u32>) -> Result<Option<Panel>, Never> {
        let key = (self.section, self.page);

        let pixels = match &self.cached_picture {
            Some((cached, pixels)) => match *cached == key {
                true => pixels,
                false => return Ok(None),
            },
            None => return Ok(None),
        };

        let Ok(size) = console_pictures::fit_within(Size { width: pixels.width, height: pixels.height }, room);
        let Ok(across) = fitted::<u32, i32>(room.width.saturating_sub(size.width).saturating_div(2));
        let Ok(down) = fitted::<u32, i32>(room.height.saturating_sub(size.height).saturating_div(2));

        Ok(Some(Panel { at: Point { x: across, y: down }, size, round: Round(0), fill: Oklch { lightness: 0.0, chroma: 0.0, hue: 0.0 }, edge: Edge::None }))
    }

    fn on_the_picture(&self, at: Point<i32>, room: Size<u32>) -> Result<Option<Point<f64>>, Never> {
        let Ok(frame) = self.picture_frame(room);

        Ok(frame.map(|frame| Point {
            x: f64::from(at.x.saturating_sub(frame.at.x)) / f64::from(frame.size.width.max(1)),
            y: f64::from(at.y.saturating_sub(frame.at.y)) / f64::from(frame.size.height.max(1)),
        }))
    }

    fn heading(&self) -> Result<String, Never> {
        let number = self.section.saturating_add(1);

        Ok(match &self.content {
            Content::Publication { blocks, .. } => match blocks.iter().find_map(|block| match block {
                Block::Heading(text) => Some(text.replace('\n', " ")),
                Block::Paragraph(_) | Block::Picture(_) => None,
            }) {
                Some(heading) => heading,
                None => format!("Chapter {number}"),
            },
            Content::PortableDocument { .. } | Content::Comic(_) => format!("Page {number}"),
        })
    }

    fn spot_at(&self, at: Point<i32>, room: Size<u32>) -> Result<Option<Spot>, Never> {
        match &self.content {
            Content::PortableDocument { .. } | Content::Comic(_) => return self.spot_on_the_picture(at, room),
            Content::Publication { .. } => {},
        }

        let Ok(lines) = self.page_lines();
        let Ok(line) = nearest_line(&lines, at, room, self.appearance);

        let line = match line {
            Some(line) => line,
            None => return Ok(None),
        };

        let Ok(origin) = line_origin(line, room);
        let Ok(letter) = letter_at(line, at.x.saturating_sub(origin.x), self.appearance);

        Ok(Some(Spot { section: self.section, block: line.block, start: line.start.saturating_add(letter) }))
    }

    fn spot_on_the_picture(&self, at: Point<i32>, room: Size<u32>) -> Result<Option<Spot>, Never> {
        let Ok(paragraphs) = self.paragraphs();
        let Ok(share) = self.on_the_picture(at, room);

        let Ok(found) = match (paragraphs, share) {
            (Some(paragraphs), Some(share)) => live_text::word_at(paragraphs, share),
            (None, _) | (_, None) => Ok(None),
        };

        Ok(found.map(|found| Spot { section: self.section, block: found.paragraph, start: found.start }))
    }

    fn visible(&self) -> Result<Visible, Never> {
        Ok(match &self.content {
            Content::Publication { .. } => {
                let Ok(lines) = self.page_lines();
                let lines = lines
                    .iter()
                    .map(|line| {
                        let Ok(wide) = length(&line.text);

                        VisibleLine { block: line.block, start: line.start, end: line.start.saturating_add(wide) }
                    })
                    .collect();

                Visible::Text { section: self.section, lines }
            },
            Content::PortableDocument { .. } | Content::Comic(_) => {
                let Ok(paragraphs) = self.paragraphs();
                let Ok(paragraphs) = match paragraphs {
                    Some(paragraphs) => fitted::<_, u32>(paragraphs.len()),
                    None => Ok(0),
                };

                Visible::Picture { section: self.section, paragraphs }
            },
        })
    }

    fn made(&self, tool: Tool, from: Spot, to: Spot) -> Result<Option<Made>, Never> {
        let Ok(text) = self.block_text(from.block);

        let text = match text {
            Some(text) => text.to_string(),
            None => return Ok(None),
        };

        let Ok(end) = match to.block.cmp(&from.block) {
            std::cmp::Ordering::Equal => Ok(to.start),
            std::cmp::Ordering::Greater => length(&text),
            std::cmp::Ordering::Less => Ok(0),
        };

        let tapped = match end == from.start {
            true => Some(from),
            false => None,
        };
        let Ok(run) = self.widen(from.block, from.start..end);
        let Ok(context) = notes::marked(&text, run.clone());
        let Ok(heading) = self.heading();
        let spot = Spot { start: run.start, ..from };

        Ok(match tool {
            Tool::Highlight => Some(Made::Highlight { tapped, note: Note { spot, mark: Mark::Highlight { end: run.end }, heading, context } }),
            Tool::Text => Some(Made::Text(Note { spot, mark: Mark::Text { end: run.end, text: String::new(), popup: Popup::Open }, heading, context })),
            Tool::Read | Tool::Pen | Tool::Eraser => None,
        })
    }

    fn widen(&self, block: u32, run: std::ops::Range<u32>) -> Result<std::ops::Range<u32>, Never> {
        let Ok(at) = console_core_number_conversion::index(block);
        let Ok(paragraphs) = self.paragraphs();
        let Ok(text) = self.block_text(block);

        match (&self.content, paragraphs.and_then(|paragraphs| paragraphs.get(at)), text) {
            (Content::PortableDocument { .. } | Content::Comic(_), Some(paragraph), _) => live_text::widen(paragraph, run),
            (Content::Publication { .. }, _, Some(text)) | (Content::PortableDocument { .. } | Content::Comic(_), None, Some(text)) => notes::widen(text, run),
            (_, _, None) => Ok(run),
        }
    }

    fn handwriting(&self, notes: &[Note], stroke: &Stroke, scope: Scope, room: Size<u32>) -> Result<Option<Made>, Never> {
        let Ok(placed) = self.anchor(stroke, scope, room);
        let Ok(measured) = self.measure(room);

        let (Placement { spot, origin, context, scope }, now) = match (placed, measured) {
            (Some(placed), Some(now)) => (placed, now),
            (None, _) | (_, None) => return Ok(None),
        };

        let Ok(joining) = self.joining(notes, stroke, Joining { scope, now, room });
        let Ok(joining) = match joining {
            Some((origin, written)) => relative(stroke, origin, Scale { written: now, now: written }).map(Some),
            None => Ok(None),
        };
        let Ok(kept) = relative(stroke, origin, Scale { written: now, now });
        let Ok(heading) = self.heading();
        let note = Note { spot, mark: Mark::Handwriting { scope, size: now, strokes: vec![kept] }, heading, context };

        Ok(Some(Made::Handwriting { joining, note }))
    }

    fn joining(&self, notes: &[Note], stroke: &Stroke, asked: Joining) -> Result<Option<(Point<i32>, u32)>, Never> {
        let Joining { scope, now, room } = asked;
        let (spot, was, written) = match notes.last() {
            Some(Note { spot, mark: Mark::Handwriting { scope, size, .. }, .. }) => (*spot, *scope, *size),
            Some(Note { mark: Mark::Highlight { .. } | Mark::Text { .. }, .. }) | None => return Ok(None),
        };

        let Ok(origin) = self.origin_on_page(spot, room);
        let Ok(far) = fitted::<u32, i32>(now.saturating_mul(8));

        Ok(match (origin, stroke.0.first(), spot.section == self.section && was == scope) {
            (Some(origin), Some(first), true) => match first.x.abs_diff(origin.x).max(first.y.abs_diff(origin.y)) <= far.unsigned_abs() {
                true => Some((origin, written)),
                false => None,
            },
            (None, _, _) | (_, None, _) | (_, _, false) => None,
        })
    }

    fn measure(&self, room: Size<u32>) -> Result<Option<u32>, Never> {
        let Ok(frame) = self.picture_frame(room);

        Ok(match &self.content {
            Content::Publication { .. } => Some(self.appearance.text_size.0),
            Content::PortableDocument { .. } | Content::Comic(_) => frame.map(|frame| frame.size.height),
        })
    }

    fn anchor(&self, stroke: &Stroke, scope: Scope, room: Size<u32>) -> Result<Option<Placement>, Never> {
        let first = match stroke.0.first() {
            Some(first) => *first,
            None => return Ok(None),
        };

        match &self.content {
            Content::PortableDocument { .. } | Content::Comic(_) => return self.anchor_on_the_picture(first, scope, room),
            Content::Publication { .. } => {},
        }

        let Ok(lines) = self.page_lines();
        let Ok(near) = self.spot_at(first, room);

        let near = match near {
            Some(near) => near,
            None => return Ok(None),
        };

        let Ok(paragraph) = self.block_text(near.block);
        let Ok(word) = match paragraph {
            Some(text) => notes::widen(text, near.start..near.start),
            None => Ok(near.start..near.start),
        };
        let spot = Spot { start: word.start, ..near };
        let Ok(origin) = self.origin_on_page(spot, room);

        let origin = match origin {
            Some(origin) => origin,
            None => return Ok(None),
        };

        let context = match (scope, paragraph) {
            (Scope::Paragraph, Some(text)) => text.to_string(),
            (Scope::Paragraph, None) | (Scope::Page, _) => {
                let Ok(page) = page_text(&lines);

                page
            },
        };

        Ok(Some(Placement { spot, origin, context, scope }))
    }

    fn origin_on_page(&self, spot: Spot, room: Size<u32>) -> Result<Option<Point<i32>>, Never> {
        let Ok(frame) = self.picture_frame(room);

        match (&self.content, spot.section == self.section) {
            (_, false) => return Ok(None),
            (Content::PortableDocument { .. } | Content::Comic(_), true) => return Ok(frame.map(|frame| frame.at)),
            (Content::Publication { .. }, true) => {},
        }

        let Ok(lines) = self.page_lines();
        let found = lines.iter().find(|line| {
            let Ok(wide) = length(&line.text);

            line.block == spot.block && line.start <= spot.start && spot.start < line.start.saturating_add(wide).max(line.start.saturating_add(1))
        });

        let line = match found {
            Some(line) => line,
            None => return Ok(None),
        };

        let Ok(into) = console_core_number_conversion::index(spot.start.saturating_sub(line.start));
        let before = match line.text.get(..into) {
            Some(before) => before,
            None => "",
        };
        let Ok(across) = width_of(before, line.style, self.appearance);
        let Ok(origin) = line_origin(line, room);
        let Ok(at) = offset(origin, Point { x: across, y: 0 });

        Ok(Some(at))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Joining {
    scope: Scope,
    now: u32,
    room: Size<u32>,
}

impl Reader {
    fn target_under(&self, notes: &[Note], at: Point<i32>, room: Size<u32>) -> Result<Option<Target>, Never> {
        let Ok(stroke) = self.stroke_under(notes, at, room);

        match stroke {
            Some(Rubbed { note, stroke }) => return Ok(Some(Target::Stroke { note, stroke })),
            None => {},
        }

        let Ok(under) = self.letter_under(at, room);

        Ok(under.map(Target::Letter))
    }

    fn letter_under(&self, at: Point<i32>, room: Size<u32>) -> Result<Option<Spot>, Never> {
        match &self.content {
            Content::PortableDocument { .. } | Content::Comic(_) => {
                let Ok(paragraphs) = self.paragraphs();
                let Ok(share) = self.on_the_picture(at, room);
                let Ok(found) = match (paragraphs, share) {
                    (Some(paragraphs), Some(share)) => live_text::word_under(paragraphs, share),
                    (None, _) | (_, None) => Ok(None),
                };

                Ok(found.map(|found| Spot { section: self.section, block: found.paragraph, start: found.start }))
            },
            Content::Publication { .. } => {
                let Ok(lines) = self.page_lines();
                let Ok(line) = nearest_line(&lines, at, room, self.appearance);
                let Ok(on_it) = match line {
                    Some(line) => {
                        let Ok(origin) = line_origin(line, room);
                        let Ok(tall) = line_height(line.style, self.appearance);
                        let Ok(tall) = fitted::<u32, i32>(tall);

                        Ok::<bool, Never>(origin.y <= at.y && at.y <= origin.y.saturating_add(tall))
                    },
                    None => Ok(false),
                };

                match on_it {
                    true => self.spot_at(at, room),
                    false => Ok(None),
                }
            },
        }
    }

    fn stroke_under(&self, notes: &[Note], at: Point<i32>, room: Size<u32>) -> Result<Option<Rubbed>, Never> {
        let Ok(measured) = self.measure(room);
        let Ok(reach) = fitted::<u32, i32>(ERASER_REACH);

        let now = match measured {
            Some(now) => now,
            None => return Ok(None),
        };

        for (index, note) in (0_u32..).zip(notes.iter()).filter(|(_, note)| note.spot.section == self.section) {
            let Ok(origin) = self.origin_on_page(note.spot, room);

            match (&note.mark, origin) {
                (Mark::Handwriting { strokes, size, .. }, Some(origin)) => {
                    let found = (0_u32..).zip(strokes.iter()).find(|(_, stroke)| {
                        let placed: Vec<Point<f64>> = stroke
                            .0
                            .iter()
                            .map(|point| {
                                let Ok(grown) = notes::scaled(*point, Scale { written: *size, now });

                                Point { x: f64::from(grown.x.saturating_add(origin.x)), y: f64::from(grown.y.saturating_add(origin.y)) }
                            })
                            .collect();
                        let lone = placed.first().map(|point| [*point, *point]);

                        placed
                            .windows(2)
                            .filter_map(|pair| match pair {
                                [from, to] => Some([*from, *to]),
                                _ => None,
                            })
                            .chain(lone)
                            .any(|[from, to]| {
                                let Ok(away) = distance_to_segment(Point { x: f64::from(at.x), y: f64::from(at.y) }, Segment { from, to });

                                away <= f64::from(reach)
                            })
                    });

                    match found.map(|(stroke, _)| stroke) {
                        Some(stroke) => return Ok(Some(Rubbed { note: index, stroke })),
                        None => {},
                    }
                },
                (Mark::Handwriting { .. }, None) | (Mark::Highlight { .. } | Mark::Text { .. }, _) => {},
            }
        }

        Ok(None)
    }
}

impl Reader {
    fn run_place(&self, spot: Spot, end: u32, room: Size<u32>) -> Result<Option<RunPlace>, Never> {
        let Ok(aside) = fitted::<u32, i32>(NOTE_ICON.saturating_add(8));
        let Ok(icon) = fitted::<u32, i32>(NOTE_ICON);

        match &self.content {
            Content::Publication { .. } => {
                let Ok(lines) = self.page_lines();
                let covering: Vec<&pages::Line> = lines
                    .iter()
                    .filter(|line| {
                        let Ok(wide) = length(&line.text);

                        line.block == spot.block && spot.start < line.start.saturating_add(wide) && line.start < end
                    })
                    .collect();

                let (first, last) = match (covering.first(), covering.last()) {
                    (Some(first), Some(last)) => (*first, *last),
                    (None, _) | (_, None) => return Ok(None),
                };

                let Ok(top) = line_origin(first, room);
                let Ok(bottom) = line_origin(last, room);
                let Ok(tall) = line_height(last.style, self.appearance);
                let Ok(tall) = fitted::<u32, i32>(tall);

                Ok(Some(RunPlace { top: top.y, bottom: bottom.y.saturating_add(tall), left: top.x, icon: Point { x: top.x.saturating_sub(aside), y: top.y } }))
            },
            Content::PortableDocument { .. } | Content::Comic(_) => {
                let Ok(paragraphs) = self.paragraphs();
                let Ok(frame) = self.picture_frame(room);
                let Ok(at) = console_core_number_conversion::index(spot.block);

                let (paragraph, frame) = match (paragraphs.and_then(|paragraphs| paragraphs.get(at)), frame) {
                    (Some(paragraph), Some(frame)) => (paragraph, frame),
                    (None, _) | (_, None) => return Ok(None),
                };

                let Ok(areas) = live_text::areas(paragraph, &(spot.start..end));

                let (first, last) = match (areas.first(), areas.last()) {
                    (Some(first), Some(last)) => (*first, *last),
                    (None, _) | (_, None) => return Ok(None),
                };

                let Ok((from, _)) = on_the_frame(first, frame);
                let Ok((_, to)) = on_the_frame(last, frame);
                let Ok(top) = console_core_number_conversion::whole_i32(first.top * f64::from(frame.size.height));

                Ok(Some(RunPlace {
                    top: frame.at.y.saturating_add(top),
                    bottom: to.y,
                    left: from.x,
                    icon: Point { x: to.x.saturating_add(4), y: to.y.saturating_sub(icon) },
                }))
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    from: Point<f64>,
    to: Point<f64>,
}

#[derive(Debug, Clone, Copy)]
struct Rubbed {
    note: u32,
    stroke: u32,
}

fn distance_to_segment(at: Point<f64>, segment: Segment) -> Result<f64, Never> {
    let Segment { from, to } = segment;
    let along = Point { x: to.x - from.x, y: to.y - from.y };
    let length = along.x.mul_add(along.x, along.y * along.y);
    let share = match length > 0.0 {
        true => (((at.x - from.x) * along.x + (at.y - from.y) * along.y) / length).clamp(0.0, 1.0),
        false => 0.0,
    };
    let nearest = Point { x: share.mul_add(along.x, from.x), y: share.mul_add(along.y, from.y) };

    Ok((at.x - nearest.x).hypot(at.y - nearest.y))
}

fn relative(stroke: &Stroke, origin: Point<i32>, scale: Scale) -> Result<Stroke, Never> {
    Ok(Stroke(
        stroke
            .0
            .iter()
            .map(|at| {
                let Ok(kept) = notes::scaled(Point { x: at.x.saturating_sub(origin.x), y: at.y.saturating_sub(origin.y) }, scale);

                kept
            })
            .collect(),
    ))
}

impl Reader {
    fn anchor_on_the_picture(&self, first: Point<i32>, scope: Scope, room: Size<u32>) -> Result<Option<Placement>, Never> {
        let Ok(frame) = self.picture_frame(room);
        let Ok(paragraphs) = self.paragraphs();
        let Ok(near) = self.spot_on_the_picture(first, room);

        let origin = match frame {
            Some(frame) => frame.at,
            None => return Ok(None),
        };

        let (block, context) = match (scope, near, paragraphs) {
            (Scope::Paragraph, Some(near), Some(paragraphs)) => {
                let Ok(at) = console_core_number_conversion::index(near.block);

                (near.block, paragraphs.get(at).map(|paragraph| paragraph.text.clone()))
            },
            (Scope::Page, _, Some(paragraphs)) => {
                (0, Some(paragraphs.iter().map(|paragraph| paragraph.text.as_str()).collect::<Vec<&str>>().join("\n")))
            },
            (Scope::Paragraph, None, Some(_)) | (_, _, None) => (0, None),
        };

        let (scope, context) = match (scope, context) {
            (scope, Some(context)) => (scope, context),
            (Scope::Page | Scope::Paragraph, None) => (Scope::Page, String::new()),
        };

        Ok(Some(Placement { spot: Spot { section: self.section, block, start: 0 }, origin, context, scope }))
    }

    fn ask_live_text(&mut self, tool: Tool, notes: &[Note]) -> Result<(), Never> {
        let marked_here = notes.iter().any(|note| note.spot.section == self.section && matches!(note.mark, Mark::Highlight { .. } | Mark::Text { .. }));
        let asked_here = match &self.live {
            LiveText::Read { section, .. } | LiveText::Recognizing { section, .. } => *section == self.section,
            LiveText::Unasked => false,
        };

        let wanted = match (&self.content, tool, marked_here, asked_here) {
            (Content::Publication { .. }, _, _, _) | (_, _, _, true) => return Ok(()),
            (Content::PortableDocument { .. } | Content::Comic(_), Tool::Highlight | Tool::Text, _, false) | (Content::PortableDocument { .. } | Content::Comic(_), _, true, false) => self.section,
            (Content::PortableDocument { .. } | Content::Comic(_), Tool::Read | Tool::Pen | Tool::Eraser, false, false) => return Ok(()),
        };

        let Ok(folder) = self.words_folder();
        let base = folder.join(format!("{}-{wanted}", self.book.name));
        let Ok(out) = beside(&base, "tsv");
        let Ok(kept) = console_core_atomic_writes::read(&out);

        match kept.text() {
            Ok(Some(table)) => {
                let Ok(paragraphs) = live_text::from_recognition(&table);

                self.live = LiveText::Read { section: wanted, paragraphs };

                return Ok(());
            },
            Ok(None) => {},
            Err(fault) => eprintln!("books: {}: {fault}", out.display()),
        }

        let Ok(layered) = match &self.content {
            Content::PortableDocument { .. } => text_layer(&self.book.path, wanted),
            Content::Publication { .. } | Content::Comic(_) => Ok(Vec::new()),
        };

        match layered.is_empty() {
            false => {
                self.live = LiveText::Read { section: wanted, paragraphs: layered };

                return Ok(());
            },
            true => {},
        }

        let Ok(picture) = self.picture_for_recognition(&base);

        let picture = match picture {
            Some(picture) => picture,
            None => {
                self.live = LiveText::Read { section: wanted, paragraphs: Vec::new() };

                return Ok(());
            },
        };

        let Ok(started) = match &self.languages {
            Some(languages) => recognize(Recognition::Transcript { picture: &picture, base: &base, languages }),
            None => recognize(Recognition::Script { picture: &picture, base: &base }),
        };
        let Ok(asked) = beside(&base, "osd");

        self.live = match (started, self.languages.is_some()) {
            (Some(running), true) => LiveText::Recognizing { section: wanted, running, out, stage: Stage::Transcript },
            (Some(running), false) => LiveText::Recognizing { section: wanted, running, out: asked, stage: Stage::Script { picture, base } },
            (None, _) => LiveText::Read { section: wanted, paragraphs: Vec::new() },
        };

        Ok(())
    }

    fn words_folder(&self) -> Result<PathBuf, Never> {
        let folder = self.cache.join("words");

        match std::fs::create_dir_all(&folder) {
            Ok(()) => {},
            Err(fault) => eprintln!("books: {}: {fault}", folder.display()),
        }

        Ok(folder)
    }

    fn picture_for_recognition(&self, base: &Path) -> Result<Option<PathBuf>, Never> {
        let Ok(section) = console_core_number_conversion::index(self.section);

        match &self.content {
            Content::PortableDocument { .. } => {
                let Ok(mut drawing) = Program::Pdftoppm.command();
                let number = self.section.saturating_add(1).to_string();
                let drawn = drawing.args(["-png", "-r", "150", "-singlefile", "-f", &number, "-l", &number]).arg(&self.book.path).arg(base).output();

                Ok(match drawn {
                    Ok(_drawn) => {
                        let Ok(drawn) = beside(base, "png");

                        Some(drawn)
                    },
                    Err(fault) => {
                        eprintln!("books: poppler would not draw {} for reading: {fault}", self.book.path.display());

                        None
                    },
                })
            },
            Content::Comic(comic) => {
                let named = match comic.pages.get(section) {
                    Some(named) => named,
                    None => return Ok(None),
                };
                let Ok(ending) = open::extension(named);
                let Ok(at) = beside(base, &ending);

                Ok(match comic.archive.extract(named).map(|bytes| console_core_atomic_writes::whole(&at, &bytes)) {
                    Ok(Ok(())) => Some(at),
                    Ok(Err(fault)) => {
                        eprintln!("books: {fault}");

                        None
                    },
                    Err(fault) => {
                        eprintln!("books: {fault}");

                        None
                    },
                })
            },
            Content::Publication { .. } => Ok(None),
        }
    }

    fn poll_live_text(&mut self) -> Result<Update, Never> {
        let (section, out) = match &mut self.live {
            LiveText::Recognizing { section, running, out, stage } => {
                let Ok(still) = running.still();

                match (still, stage) {
                    (Still::Running, _) => return Ok(Update::None),
                    (Still::Ended, Stage::Script { picture, base }) => {
                        let (section, picture, base, asked) = (*section, picture.clone(), base.clone(), out.clone());

                        return self.read_in_its_script(section, Recognized { picture, base, asked });
                    },
                    (Still::Ended, Stage::Transcript) => (*section, out.clone()),
                }
            },
            LiveText::Unasked | LiveText::Read { .. } => return Ok(Update::None),
        };

        let Ok(kept) = console_core_atomic_writes::read(&out);

        let paragraphs = match kept.text() {
            Ok(Some(table)) => {
                let Ok(paragraphs) = live_text::from_recognition(&table);

                paragraphs
            },
            Ok(None) => Vec::new(),
            Err(fault) => {
                eprintln!("books: {}: {fault}", out.display());

                Vec::new()
            },
        };

        self.live = LiveText::Read { section, paragraphs };

        Ok(Update::Redraw)
    }
}

struct Recognized {
    picture: PathBuf,
    base: PathBuf,
    asked: PathBuf,
}

impl Reader {
    fn read_in_its_script(&mut self, section: u32, recognized: Recognized) -> Result<Update, Never> {
        let Recognized { picture, base, asked } = recognized;
        let Ok(kept) = console_core_atomic_writes::read(&asked);

        let Ok(script) = match kept.text() {
            Ok(Some(answered)) => live_text::script(&answered),
            Ok(None) => Ok(None),
            Err(fault) => {
                eprintln!("books: {}: {fault}", asked.display());

                Ok(None)
            },
        };

        let Ok(installed) = installed_languages();
        let Ok(languages) = live_text::languages(script.as_deref(), &installed);
        let Ok(started) = recognize(Recognition::Transcript { picture: &picture, base: &base, languages: &languages });
        let Ok(out) = beside(&base, "tsv");

        self.languages = Some(languages);
        self.live = match started {
            Some(running) => LiveText::Recognizing { section, running, out, stage: Stage::Transcript },
            None => LiveText::Read { section, paragraphs: Vec::new() },
        };

        Ok(Update::None)
    }
}

fn installed_languages() -> Result<Vec<String>, Never> {
    let Ok(mut asking) = Program::Tesseract.command();

    Ok(match asking.arg("--list-langs").output() {
        Ok(answered) => {
            let Ok(listed) = live_text::installed(&String::from_utf8_lossy(&answered.stdout));

            listed
        },
        Err(fault) => {
            eprintln!("books: tesseract would not say which languages it has: {fault}");

            Vec::new()
        },
    })
}

fn beside(base: &Path, ending: &str) -> Result<PathBuf, Never> {
    let mut named = base.as_os_str().to_owned();

    named.push(".");
    named.push(ending);

    Ok(PathBuf::from(named))
}

fn text_layer(at: &Path, section: u32) -> Result<Vec<Paragraph>, Never> {
    let Ok(mut asking) = Program::Pdftotext.command();
    let number = section.saturating_add(1).to_string();
    let answered = asking.args(["-bbox-layout", "-f", &number, "-l", &number]).arg(at).arg("-").output();

    Ok(match answered {
        Ok(answered) => {
            let Ok(paragraphs) = live_text::from_layout(&String::from_utf8_lossy(&answered.stdout));

            paragraphs
        },
        Err(fault) => {
            eprintln!("books: poppler would not say what {} says: {fault}", at.display());

            Vec::new()
        },
    })
}

fn recognize(asked: Recognition<'_>) -> Result<Option<BoundToParent>, Never> {
    let Ok(mut asking) = Program::Tesseract.command();

    match asked {
        Recognition::Script { picture, base } => asking.arg(picture).arg(base).args(["--psm", "0", "-l", "osd"]),
        Recognition::Transcript { picture, base, languages } => asking.arg(picture).arg(base).args(["-l", languages, "tsv"]),
    };

    asking.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());

    Ok(match console_program_lifetime::alongside(&mut asking) {
        Ok(running) => Some(running),
        Err(fault) => {
            eprintln!("books: tesseract would not start: {fault}");

            None
        },
    })
}

struct Placement {
    spot: Spot,
    origin: Point<i32>,
    context: String,
    scope: Scope,
}

fn page_text(lines: &[pages::Line]) -> Result<String, Never> {
    let mut text = String::new();
    let mut last: Option<u32> = None;

    for line in lines {
        let between = match (last, text.is_empty()) {
            (_, true) => "",
            (Some(block), false) => match block == line.block {
                true => " ",
                false => "\n",
            },
            (None, false) => " ",
        };

        text.push_str(between);
        text.push_str(&line.text);
        last = Some(line.block);
    }

    Ok(text)
}

fn line_origin(line: &pages::Line, room: Size<u32>) -> Result<Point<i32>, Never> {
    let Ok(column) = column(room);
    let Ok(left) = fitted::<u32, i32>(column.left);
    let Ok(down) = fitted::<u32, i32>(READING_EDGE.saturating_add(line.top));

    Ok(Point { x: left, y: down })
}

fn nearest_line(lines: &[pages::Line], at: Point<i32>, room: Size<u32>, appearance: Appearance) -> Result<Option<&pages::Line>, Never> {
    Ok(lines.iter().min_by_key(|line| {
        let Ok(origin) = line_origin(line, room);
        let Ok(tall) = line_height(line.style, appearance);
        let Ok(half) = fitted::<u32, i32>(tall.saturating_div(2));

        origin.y.saturating_add(half).abs_diff(at.y)
    }))
}

fn width_of(text: &str, style: Style, appearance: Appearance) -> Result<u32, Never> {
    let Ok((font, weight)) = style_font(style, appearance);
    let Ok(measured) = console_draw_painting::measure_text(Run { said: text, weight, width: 4096 }, &font);

    Ok(measured.width)
}

fn length(text: &str) -> Result<u32, Never> {
    fitted::<_, u32>(text.len())
}

fn letter_at(line: &pages::Line, across: i32, appearance: Appearance) -> Result<u32, Never> {
    let Ok(across) = fitted::<i32, u32>(across.max(0));
    let last = line
        .text
        .char_indices()
        .map(|(at, _)| at)
        .skip(1)
        .chain(std::iter::once(line.text.len()))
        .take_while(|end| {
            let before = match line.text.get(..*end) {
                Some(before) => before,
                None => "",
            };
            let Ok(wide) = width_of(before, line.style, appearance);

            wide <= across
        })
        .last()
        .map(fitted::<_, u32>);

    Ok(match last {
        Some(Ok(end)) => end,
        None => 0,
    })
}

fn centered_picture(pixels: Pixels, area: Size<u32>) -> Result<Shape, Never> {
    let Ok(size) = console_pictures::fit_within(Size { width: pixels.width, height: pixels.height }, area);
    let Ok(across) = fitted::<u32, i32>(area.width.saturating_sub(size.width).saturating_div(2));
    let Ok(down) = fitted::<u32, i32>(area.height.saturating_sub(size.height).saturating_div(2));

    Ok(Shape::Picture(Picture { at: Point { x: across, y: down }, size, pixels }))
}

fn text_page_shapes(lines: &[pages::Line], room: Size<u32>, ink: Oklch, appearance: Appearance) -> Result<Vec<Shape>, Never> {
    let Ok(column) = column(room);
    let Ok(left) = fitted::<u32, i32>(column.left);
    let mut shapes = Vec::new();

    for line in lines {
        let Ok((font, weight)) = style_font(line.style, appearance);
        let Ok(down) = fitted::<u32, i32>(READING_EDGE.saturating_add(line.top));

        shapes.push(Shape::Text(Text {
            at: Point { x: left, y: down },
            width: column.width.saturating_add(READING_EDGE),
            said: line.text.clone(),
            weight,
            font,
            ink,
        }));
    }

    Ok(shapes)
}

fn reader_shapes(reader: &mut Reader, annotations: &Annotations, look: Look, room: Size<u32>, device: Size<u32>) -> Result<Vec<Shape>, Never> {
    let Ok(()) = reader.ensure_layout(room);
    let Ok(()) = reader.ask_live_text(annotations.tool, &annotations.notes);
    let Ok(ground) = background(room, look.ground);
    let mut shapes = vec![ground];
    let Ok(page) = console_core_number_conversion::index(reader.page);
    let lines = match &reader.content {
        Content::Publication { pages, .. } => match pages.get(page) {
            Some(Page::Text(lines)) => Some(lines.clone()),
            Some(Page::Picture(_)) | None => None,
        },
        Content::PortableDocument { .. } | Content::Comic(_) => None,
    };

    match lines {
        Some(lines) => {
            let Ok(marked) = highlight_shapes(reader, annotations, &lines, room, look);
            let Ok(cursor) = cursor_shapes(reader, annotations.cursor, &lines, room, look);
            let Ok(text) = text_page_shapes(&lines, room, look.ink, reader.appearance);

            shapes.extend(marked);
            shapes.extend(cursor);
            shapes.extend(text);
        },
        None => {
            let Ok(pixels) = reader.current_picture(device);

            match pixels {
                Some(pixels) => {
                    let Ok(shape) = centered_picture(pixels, room);
                    let Ok(marked) = picture_highlight_shapes(reader, annotations, room, look);

                    shapes.push(shape);
                    shapes.extend(marked);
                },
                None => {},
            }
        },
    }

    let Ok(written) = stroke_shapes(reader, annotations, room, look);
    let Ok(footer) = footer(reader, room, look);
    let Ok(frames) = note_frames(reader, annotations, room);
    let Ok(noted) = note_shapes(&frames, look);
    let Ok(rail) = rail_shapes(Tools { tool: annotations.tool, scope: annotations.scope }, room, look);

    shapes.extend(written);
    shapes.extend(footer);
    shapes.extend(noted);
    shapes.extend(rail);

    Ok(shapes)
}

fn highlight_runs(reader: &Reader, annotations: &Annotations) -> Result<Vec<(u32, std::ops::Range<u32>, Underline)>, Never> {
    let mut runs: Vec<(u32, std::ops::Range<u32>, Underline)> = annotations
        .notes
        .iter()
        .filter(|note| note.spot.section == reader.section)
        .filter_map(|note| match note.mark {
            Mark::Highlight { end } => Some((note.spot.block, note.spot.start..end, Underline::Filled)),
            Mark::Text { end, .. } => Some((note.spot.block, note.spot.start..end, Underline::Ruled)),
            Mark::Handwriting { .. } => None,
        })
        .collect();

    match annotations.gesture {
        Gesture::Selecting { from, to } => {
            let Ok(text) = reader.block_text(from.block);
            let Ok(end) = match (to.block.cmp(&from.block), text) {
                (std::cmp::Ordering::Equal, _) => Ok(to.start),
                (std::cmp::Ordering::Greater, Some(text)) => length(text),
                (std::cmp::Ordering::Greater, None) | (std::cmp::Ordering::Less, _) => Ok(0),
            };
            let Ok(run) = match text {
                Some(_) => reader.widen(from.block, from.start..end),
                None => Ok(from.start..end),
            };

            runs.push((from.block, run, Underline::Filled));
        },
        Gesture::None | Gesture::Drawing(_) | Gesture::Erasing => {},
    }

    Ok(runs)
}

fn highlight_shapes(reader: &Reader, annotations: &Annotations, lines: &[pages::Line], room: Size<u32>, look: Look) -> Result<Vec<Shape>, Never> {
    let Ok(runs) = highlight_runs(reader, annotations);
    let mut shapes = Vec::new();

    for line in lines {
        let Ok(wide) = length(&line.text);
        let ends = line.start..line.start.saturating_add(wide);

        for (_, run, underline) in runs.iter().filter(|(block, _, _)| *block == line.block) {
            let Ok(from) = console_core_number_conversion::index(run.start.max(ends.start).saturating_sub(line.start));
            let Ok(to) = console_core_number_conversion::index(run.end.min(ends.end).saturating_sub(line.start));

            match (from < to, line.text.get(..from), line.text.get(..to)) {
                (true, Some(before), Some(through)) => {
                    let Ok(left) = width_of(before, line.style, reader.appearance);
                    let Ok(right) = width_of(through, line.style, reader.appearance);
                    let Ok(tall) = line_height(line.style, reader.appearance);
                    let Ok(origin) = line_origin(line, room);
                    let Ok(at) = offset(origin, Point { x: left, y: 0 });

                    match underline {
                        Underline::Filled => shapes.push(Shape::Panel(Panel {
                            at,
                            size: Size { width: right.saturating_sub(left), height: tall },
                            round: Round(3),
                            fill: look.highlight,
                            edge: Edge::None,
                        })),
                        Underline::Ruled => {
                            let Ok(under) = offset(at, Point { x: 0, y: tall.saturating_sub(2) });
                            let Ok(end) = offset(under, Point { x: right.saturating_sub(left), y: 0 });

                            shapes.push(Shape::Line(console_core_shapes::Line { from: under, to: end, width: 2, color: look.pen }));
                        },
                    }
                },
                (false, _, _) | (true, None, _) | (true, _, None) => {},
            }
        }
    }

    Ok(shapes)
}

fn picture_highlight_shapes(reader: &Reader, annotations: &Annotations, room: Size<u32>, look: Look) -> Result<Vec<Shape>, Never> {
    let Ok(runs) = highlight_runs(reader, annotations);
    let Ok(paragraphs) = reader.paragraphs();
    let Ok(frame) = reader.picture_frame(room);

    let (paragraphs, frame) = match (paragraphs, frame) {
        (Some(paragraphs), Some(frame)) => (paragraphs, frame),
        (None, _) | (_, None) => return Ok(Vec::new()),
    };

    let mut shapes = Vec::new();
    let cursor = annotations.cursor.map(|block| (block, 0..u32::MAX));
    let marked = runs
        .iter()
        .map(|(block, run, underline)| {
            let color = match underline {
                Underline::Filled => look.highlight,
                Underline::Ruled => look.pen,
            };

            (*block, run.clone(), color)
        })
        .chain(cursor.map(|(block, run)| (block, run, look.pen)));

    for (block, run, color) in marked {
        let Ok(at) = console_core_number_conversion::index(block);
        let Ok(covered) = match paragraphs.get(at) {
            Some(paragraph) => live_text::areas(paragraph, &run),
            None => Ok(Vec::new()),
        };

        for area in covered {
            let Ok(placed) = on_the_frame(area, frame);

            shapes.push(Shape::Line(console_core_shapes::Line { from: placed.0, to: placed.1, width: 4, color }));
        }
    }

    Ok(shapes)
}

fn on_the_frame(area: live_text::Area, frame: Panel) -> Result<(Point<i32>, Point<i32>), Never> {
    let wide = f64::from(frame.size.width);
    let tall = f64::from(frame.size.height);
    let Ok(left) = console_core_number_conversion::whole_i32(area.left * wide);
    let Ok(right) = console_core_number_conversion::whole_i32((area.left + area.width) * wide);
    let Ok(under) = console_core_number_conversion::whole_i32((area.top + area.height) * tall);
    let down = frame.at.y.saturating_add(under).saturating_add(2);

    Ok((Point { x: frame.at.x.saturating_add(left), y: down }, Point { x: frame.at.x.saturating_add(right), y: down }))
}

fn cursor_shapes(reader: &Reader, cursor: Option<u32>, lines: &[pages::Line], room: Size<u32>, look: Look) -> Result<Vec<Shape>, Never> {
    let mut shapes = Vec::new();

    for line in lines.iter().filter(|line| Some(line.block) == cursor) {
        let Ok(origin) = line_origin(line, room);
        let Ok(tall) = line_height(line.style, reader.appearance);

        shapes.push(Shape::Panel(Panel {
            at: Point { x: origin.x.saturating_sub(12), y: origin.y },
            size: Size { width: 4, height: tall },
            round: Round(2),
            fill: look.pen,
            edge: Edge::None,
        }));
    }

    Ok(shapes)
}

fn segments(stroke: &Stroke, origin: Point<i32>, color: Oklch) -> Result<Vec<Shape>, Never> {
    let points: Vec<Point<i32>> = stroke.0.iter().map(|at| Point { x: at.x.saturating_add(origin.x), y: at.y.saturating_add(origin.y) }).collect();

    Ok(match points.as_slice() {
        [only] => vec![Shape::Line(console_core_shapes::Line { from: *only, to: *only, width: PEN, color })],
        points => points
            .windows(2)
            .filter_map(|pair| match pair {
                [from, to] => Some(Shape::Line(console_core_shapes::Line { from: *from, to: *to, width: PEN, color })),
                _ => None,
            })
            .collect(),
    })
}

fn stroke_shapes(reader: &Reader, annotations: &Annotations, room: Size<u32>, look: Look) -> Result<Vec<Shape>, Never> {
    let Ok(measured) = reader.measure(room);
    let mut shapes = Vec::new();

    for note in annotations.notes.iter().filter(|note| note.spot.section == reader.section) {
        let Ok(origin) = match note.mark {
            Mark::Handwriting { .. } => reader.origin_on_page(note.spot, room),
            Mark::Highlight { .. } | Mark::Text { .. } => Ok(None),
        };

        match (&note.mark, origin, measured) {
            (Mark::Handwriting { strokes, size, .. }, Some(origin), Some(now)) => {
                for stroke in strokes {
                    let Ok(grown) = relative(stroke, Point { x: 0, y: 0 }, Scale { written: *size, now });
                    let Ok(drawn) = segments(&grown, origin, look.pen);

                    shapes.extend(drawn);
                }
            },
            (Mark::Handwriting { .. } | Mark::Highlight { .. } | Mark::Text { .. }, _, _) => {},
        }
    }

    match &annotations.gesture {
        Gesture::Drawing(stroke) => {
            let Ok(drawn) = segments(stroke, Point { x: 0, y: 0 }, look.pen);

            shapes.extend(drawn);
        },
        Gesture::None | Gesture::Selecting { .. } | Gesture::Erasing => {},
    }

    Ok(shapes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RunPlace {
    top: i32,
    bottom: i32,
    left: i32,
    icon: Point<i32>,
}

fn note_font() -> Result<(Font, u32), Never> {
    let Ok(font) = font(SANS, NOTE_TEXT);
    let Ok(tall) = line_height_of(&font, Weight::Plain);

    Ok((font, tall.saturating_mul(5).saturating_div(4)))
}

fn card_lines(text: &str, writing: Typing, wide: u32) -> Result<Vec<String>, Never> {
    let Ok((font, _)) = note_font();
    let shown = match writing {
        Typing::Yes => format!("{text}{CARET}"),
        Typing::No => text.to_string(),
    };
    let mut lines = Vec::new();

    for paragraph in shown.split('\n') {
        let Ok(wrapped) = console_draw_painting::wrapped(Run { said: paragraph, weight: Weight::Plain, width: wide }, &font);

        match wrapped.is_empty() {
            true => lines.push(String::new()),
            false => lines.extend(wrapped),
        }
    }

    Ok(lines)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Typing {
    Yes,
    No,
}

fn note_frames(reader: &Reader, annotations: &Annotations, room: Size<u32>) -> Result<Vec<NoteFrame>, Never> {
    let Ok(column) = column(room);
    let Ok(left) = fitted::<u32, i32>(column.left);
    let Ok(edge) = fitted::<u32, i32>(READING_EDGE);
    let written = annotations.writing.as_ref().map(|writing| writing.at);
    let mut frames = Vec::new();

    for note in annotations.notes.iter().filter(|note| note.spot.section == reader.section) {
        let (end, text, popup) = match &note.mark {
            Mark::Text { end, text, popup } => (*end, text, *popup),
            Mark::Highlight { .. } | Mark::Handwriting { .. } => continue,
        };
        let writing = match written == Some(note.spot) {
            true => Typing::Yes,
            false => Typing::No,
        };
        let Ok(placed) = reader.run_place(note.spot, end, room);

        let place = match (placed, writing) {
            (Some(place), _) => place,
            (None, Typing::Yes) => RunPlace { top: edge, bottom: edge, left, icon: Point { x: left, y: edge } },
            (None, Typing::No) => continue,
        };

        let shown = match (popup, writing) {
            (Popup::Closed, Typing::No) => Shown::Icon(Panel {
                at: place.icon,
                size: Size { width: NOTE_ICON, height: NOTE_ICON },
                round: Round(NOTE_ICON.saturating_div(2)),
                fill: Oklch { lightness: 0.0, chroma: 0.0, hue: 0.0 },
                edge: Edge::None,
            }),
            (Popup::Open, _) | (Popup::Closed, Typing::Yes) => {
                let Ok(card) = card(text, writing, place, room);

                card
            },
        };

        frames.push(NoteFrame { at: note.spot, shown });
    }

    Ok(frames)
}

fn card(text: &str, writing: Typing, place: RunPlace, room: Size<u32>) -> Result<Shown, Never> {
    let wide = NOTE_WIDE.min(room.width.saturating_sub(READING_EDGE.saturating_mul(2)));
    let close = NOTE_ICON.saturating_add(4);
    let Ok(lines) = card_lines(text, writing, wide.saturating_sub(NOTE_PADDING.saturating_mul(2)).saturating_sub(close));
    let Ok((_, line)) = note_font();
    let Ok(count) = fitted::<_, u32>(lines.len());
    let tall = NOTE_PADDING.saturating_mul(2).saturating_add(line.saturating_mul(count.max(1)));
    let Ok(across) = fitted::<u32, i32>(room.width.saturating_sub(wide));
    let Ok(high) = fitted::<u32, i32>(tall);
    let Ok(floor) = fitted::<u32, i32>(room.height.saturating_sub(8));
    let below = place.bottom.saturating_add(6);
    let down = match below.saturating_add(high) <= floor {
        true => below,
        false => place.top.saturating_sub(6).saturating_sub(high).max(0),
    };
    let at = Point { x: place.left.min(across).max(0), y: down };
    let Ok(close_at) = offset(at, Point { x: wide.saturating_sub(close).saturating_sub(4), y: 4 });

    Ok(Shown::Card {
        card: Panel { at, size: Size { width: wide, height: tall }, round: Round(8), fill: Oklch { lightness: 0.0, chroma: 0.0, hue: 0.0 }, edge: Edge::None },
        close: Panel { at: close_at, size: Size { width: close, height: close }, round: Round(close.saturating_div(2)), fill: Oklch { lightness: 0.0, chroma: 0.0, hue: 0.0 }, edge: Edge::None },
        lines,
    })
}

fn note_part(frames: &[NoteFrame], at: Point<i32>) -> Result<Option<(Spot, NotePart)>, Never> {
    Ok(frames.iter().rev().find_map(|frame| {
        let part = match &frame.shown {
            Shown::Icon(icon) => match icon.covers(at) {
                Ok(console_core_shapes::Covers::Yes) => Some(NotePart::Icon),
                Ok(console_core_shapes::Covers::No) => None,
            },
            Shown::Card { card, close, .. } => match (close.covers(at), card.covers(at)) {
                (Ok(console_core_shapes::Covers::Yes), _) => Some(NotePart::Close),
                (Ok(console_core_shapes::Covers::No), Ok(console_core_shapes::Covers::Yes)) => Some(NotePart::Card),
                (Ok(console_core_shapes::Covers::No), Ok(console_core_shapes::Covers::No)) => None,
            },
        };

        part.map(|part| (frame.at, part))
    }))
}

fn note_shapes(frames: &[NoteFrame], look: Look) -> Result<Vec<Shape>, Never> {
    let mut shapes = Vec::new();
    let Ok(icons) = font(console_core_fonts::ICONS, 20);
    let Ok((words, line)) = note_font();

    for frame in frames {
        match &frame.shown {
            Shown::Icon(icon) => {
                let Ok(measured) = console_draw_painting::measure_text(Run { said: NOTE, weight: Weight::Plain, width: NOTE_ICON }, &icons);
                let Ok(at) = offset(icon.at, Point {
                    x: NOTE_ICON.saturating_sub(measured.width).saturating_div(2),
                    y: NOTE_ICON.saturating_sub(measured.height).saturating_div(2),
                });

                shapes.push(Shape::Panel(Panel { fill: look.highlight, ..*icon }));
                shapes.push(Shape::Text(Text { at, width: NOTE_ICON, said: NOTE.to_string(), weight: Weight::Plain, font: icons.clone(), ink: look.ink }));
            },
            Shown::Card { card, close, lines } => {
                let Ok(cross) = font(SANS, 14);
                let Ok(measured) = console_draw_painting::measure_text(Run { said: CLOSE, weight: Weight::Plain, width: close.size.width }, &cross);
                let Ok(cross_at) = offset(close.at, Point {
                    x: close.size.width.saturating_sub(measured.width).saturating_div(2),
                    y: close.size.height.saturating_sub(measured.height).saturating_div(2),
                });

                shapes.push(Shape::Panel(Panel { fill: look.highlight, edge: Edge::Of { wide: 1, color: look.ink }, ..*card }));
                shapes.push(Shape::Text(Text { at: cross_at, width: close.size.width, said: CLOSE.to_string(), weight: Weight::Plain, font: cross, ink: look.ink }));

                for (row, said) in (0_u32..).zip(lines) {
                    let Ok(at) = offset(card.at, Point { x: NOTE_PADDING, y: NOTE_PADDING.saturating_add(row.saturating_mul(line)) });

                    shapes.push(Shape::Text(Text {
                        at,
                        width: card.size.width.saturating_sub(NOTE_PADDING.saturating_mul(2)),
                        said: said.clone(),
                        weight: Weight::Plain,
                        font: words.clone(),
                        ink: look.ink,
                    }));
                }
            },
        }
    }

    Ok(shapes)
}

fn glyph(rail: Rail, scope: Scope) -> Result<&'static str, Never> {
    Ok(match (rail, scope) {
        (Rail::Tool(Tool::Read), _) => READING,
        (Rail::Tool(Tool::Highlight), _) => MARKER,
        (Rail::Tool(Tool::Text), _) => NOTE,
        (Rail::Tool(Tool::Pen), _) => PENCIL,
        (Rail::Tool(Tool::Eraser), _) => ERASER,
        (Rail::Scope, Scope::Page) => ON_THE_PAGE,
        (Rail::Scope, Scope::Paragraph) => ON_THE_PARAGRAPH,
        (Rail::Undo, _) => UNDO,
        (Rail::Redo, _) => REDO,
    })
}

fn rail_shapes(tools: Tools, room: Size<u32>, look: Look) -> Result<Vec<Shape>, Never> {
    let mut shapes = Vec::new();

    for (index, rail) in (0_u32..).zip(RAIL) {
        let Ok(button) = rail_button(room, index);
        let (fill, ink) = match rail == Rail::Tool(tools.tool) {
            true => (look.ink, look.ground),
            false => (look.ground, look.ink),
        };
        let Ok(said) = glyph(rail, tools.scope);
        let Ok(font) = font(console_core_fonts::ICONS, 20);
        let Ok(measured) = console_draw_painting::measure_text(Run { said, weight: Weight::Plain, width: RAIL_BUTTON }, &font);
        let Ok(at) = offset(button.at, Point {
            x: RAIL_BUTTON.saturating_sub(measured.width).saturating_div(2),
            y: RAIL_BUTTON.saturating_sub(measured.height).saturating_div(2),
        });

        shapes.push(Shape::Panel(Panel { fill, edge: Edge::Of { wide: 1, color: look.ink }, ..button }));
        shapes.push(Shape::Text(Text { at, width: RAIL_BUTTON, said: said.to_string(), weight: Weight::Plain, font, ink }));
    }

    Ok(shapes)
}

fn footer(reader: &Reader, room: Size<u32>, look: Look) -> Result<Vec<Shape>, Never> {
    let Ok(location) = reader.location();
    let Ok(column) = column(room);
    let Ok(left) = fitted::<u32, i32>(column.left);
    let Ok(down) = fitted::<u32, i32>(room.height.saturating_sub(34));
    let Ok(close_across) = fitted::<u32, i32>(room.width.saturating_sub(grid::MARGIN).saturating_sub(22));
    let reading = match &reader.live {
        LiveText::Recognizing { section, .. } => match *section == reader.section {
            true => "    Reading the page\u{2026}",
            false => "",
        },
        LiveText::Unasked | LiveText::Read { .. } => "",
    };
    let text = format!("{}    {}%{reading}", reader.book.title, location.percent);
    let style = TextStyle { size: 13, weight: Weight::Plain, width: column.width };
    let close_style = TextStyle { size: 22, weight: Weight::Plain, width: 40 };
    let Ok(text) = text_shape(&text, Point { x: left, y: down }, style, look.ink);
    let Ok(close) = text_shape(CLOSE, Point { x: close_across, y: 22 }, close_style, look.ink);
    let mut shapes = vec![text, close];

    for turn in [Turn::Back, Turn::Forward] {
        let Ok(bar) = turn_bar(room, turn);
        let Ok(drawn) = turn_bar_shapes(bar, turn, look);

        shapes.extend(drawn);
    }

    match reader.content {
        Content::Publication { .. } => {
            let Ok(letters) = zoom_shapes(look);

            shapes.extend(letters);
        },
        Content::PortableDocument { .. } | Content::Comic(_) => {},
    }

    Ok(shapes)
}

fn turn_bar_shapes(bar: Panel, turn: Turn, look: Look) -> Result<Vec<Shape>, Never> {
    let arrow = match turn {
        Turn::Back => BACK,
        Turn::Forward => FORWARD,
    };
    let style = TextStyle { size: 20, weight: Weight::Plain, width: bar.size.width };
    let Ok(font) = font(SANS, style.size);
    let Ok(measured) = console_draw_painting::measure_text(Run { said: arrow, weight: style.weight, width: style.width }, &font);
    let Ok(at) = offset(bar.at, Point {
        x: bar.size.width.saturating_sub(measured.width).saturating_div(2),
        y: bar.size.height.saturating_sub(measured.height).saturating_div(2),
    });
    let Ok(arrow) = text_shape(arrow, at, style, look.ink);

    Ok(vec![Shape::Panel(Panel { fill: look.ground, edge: Edge::Of { wide: 1, color: look.ink }, ..bar }), arrow])
}

fn zoom_shapes(look: Look) -> Result<Vec<Shape>, Never> {
    let mut shapes = Vec::new();

    for (zoom, size) in [(Zoom::ZoomedOut, 15), (Zoom::ZoomedIn, 24)] {
        let Ok(button) = zoom_button(zoom);
        let style = TextStyle { size, weight: Weight::Plain, width: CORNER };
        let Ok(font) = font(SANS, style.size);
        let Ok(measured) = console_draw_painting::measure_text(Run { said: LETTER, weight: style.weight, width: style.width }, &font);
        let Ok(at) = offset(button.at, Point {
            x: CORNER.saturating_sub(measured.width).saturating_div(2),
            y: CORNER.saturating_sub(measured.height).saturating_div(2),
        });
        let Ok(letter) = text_shape(LETTER, at, style, look.ink);

        shapes.push(letter);
    }

    Ok(shapes)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Look {
    ground: Oklch,
    ink: Oklch,
    pen: Oklch,
    highlight: Oklch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tools {
    tool: Tool,
    scope: Scope,
}

fn look(wearing: &Wearing, appearance: Appearance) -> Result<Look, Never> {
    let ground = match appearance.background {
        Paint::Chosen(ground) => ground,
        Paint::Desktop => wearing.ground,
    };

    Ok(Look {
        ground,
        ink: match appearance.text {
            Paint::Chosen(ink) => ink,
            Paint::Desktop => wearing.text,
        },
        pen: wearing.pink,
        highlight: Oklch { lightness: ground.lightness * 0.8 + 0.12, chroma: 0.09, hue: 95.0 },
    })
}

impl App {
    fn poll_live_text(&mut self) -> Result<Update, Never> {
        match &mut self.view {
            View::Reader(reader) => reader.poll_live_text(),
            View::Library => Ok(Update::None),
        }
    }

    fn reload_appearance(&mut self) -> Result<Update, Never> {
        let Ok(chosen) = Appearance::current();

        Ok(match chosen == self.appearance {
            true => Update::None,
            false => {
                self.appearance = chosen;

                match &mut self.view {
                    View::Reader(reader) => {
                        reader.appearance = chosen;
                        reader.layout_size = Size { width: 0, height: 0 };
                    },
                    View::Library => {},
                }

                Update::Redraw
            },
        })
    }
}

fn draw(app: &mut App, surface: &mut Surface) -> Result<(), Never> {
    let Ok(room) = logical_size(surface);
    let Ok(scale) = surface.scale();
    let Ok(device) = scale.device(room);

    let shapes = match &mut app.view {
        View::Library => library_shapes(app, room, device),
        View::Reader(reader) => {
            let Ok(look) = look(&app.wearing, app.appearance);

            reader_shapes(reader, &app.annotations, look, room, device)
        },
    };

    let Ok(shapes) = shapes;

    let Ok(painting) = console_draw_painting::painter(room, &shapes, "books");
    let drawn = surface.draw(painting);

    match drawn {
        Ok(()) => {},
        Err(fault) => eprintln!("books: {fault}"),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_keyboard_taking_the_bottom_of_the_screen_redraws_the_library_into_what_is_left() {
        let screen = Size { width: 1024, height: 640 };
        let above_the_keyboard = Size { width: 1024, height: 340 };

        assert_eq!(resized(Some(screen), screen), Ok(Update::None));
        assert_eq!(resized(Some(screen), above_the_keyboard), Ok(Update::Redraw), "the old picture is squeezed into the new room");
        assert_eq!(resized(None, screen), Ok(Update::Redraw));
    }

    #[test]
    fn a_book_that_lands_in_books_while_the_library_is_open_is_on_the_shelf() -> Result<(), Box<dyn std::error::Error>> {
        let home = console_core_temporary_directories::fresh("books-landing")?;
        let Ok(folder) = library::books_folder(&home);
        std::fs::create_dir_all(&folder)?;
        let Ok(mut open) = Library::of(home.clone());

        assert_eq!(open.books.len(), 0);

        let book = folder.join("Meditations [2680].epub");
        console_core_atomic_writes::whole(&book, b"")?;
        let song = home.join("Music").join("Africa [x].opus");

        let Ok(elsewhere) = open.apply_change(&Change { event_group: EventGroup::Path(folder.clone()), text: song.display().to_string() });

        assert_eq!(elsewhere, Update::None, "a song is not a book");

        let Ok(landed) = open.apply_change(&Change { event_group: EventGroup::Path(folder.clone()), text: book.display().to_string() });

        assert_eq!(landed, Update::Redraw, "the finished download went unnoticed");
        assert_eq!(open.books.len(), 1);

        std::fs::remove_dir_all(&home)?;

        Ok(())
    }

    #[test]
    fn covers_made_while_the_library_is_open_are_drawn_without_a_press() -> Result<(), Box<dyn std::error::Error>> {
        let home = console_core_temporary_directories::fresh("books-covers")?;
        let Ok(mut open) = Library::of(home.clone());
        let store = home.join("pictures");
        open.covers = Some(store.clone());

        assert_eq!(open.redraw_if_covers_changed(), Ok(Update::None), "nothing has been made yet");

        console_core_atomic_writes::whole(&store, b"covers")?;

        assert_eq!(open.redraw_if_covers_changed(), Ok(Update::Redraw), "the covers landed and the library went on showing blanks");
        assert_eq!(open.redraw_if_covers_changed(), Ok(Update::None), "the same covers drawn twice");

        std::fs::remove_dir_all(&home)?;

        Ok(())
    }

    #[test]
    fn a_book_is_read_on_the_page_and_in_the_ink_that_were_chosen() -> Result<(), Box<dyn std::error::Error>> {
        let palette = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../files/usr/local/lib/console/palette.sh"))?;
        let Ok(spent) = console_core_color::palette::read(&palette);
        let wearing = Wearing::out_of(&spent)?;
        let sepia = Oklch { lightness: 0.93, chroma: 0.045, hue: 72.0 };
        let brown = Oklch { lightness: 0.42, chroma: 0.12, hue: 36.0 };
        let chosen = Appearance { background: Paint::Chosen(sepia), text: Paint::Chosen(brown), ..Appearance::default() };

        let Ok(on_sepia) = look(&wearing, chosen);
        let Ok(on_the_desktop) = look(&wearing, Appearance::default());

        assert_eq!((on_sepia.ground, on_sepia.ink), (sepia, brown));
        assert_eq!((on_the_desktop.ground, on_the_desktop.ink), (wearing.ground, wearing.text), "nothing chosen is the desktop's own");

        Ok(())
    }

    #[test]
    fn the_bars_beside_the_page_turn_it_and_the_page_itself_is_left_for_notes() -> Result<(), &'static str> {
        let room = Size { width: 1280, height: 800 };
        let Ok(column) = column(room);
        let Ok(back) = turn_target(room, Turn::Back);
        let Ok(forward) = turn_target(room, Turn::Forward);
        let Ok(left) = fitted::<u32, i32>(column.left);
        let Ok(middle) = fitted::<u32, i32>(room.height.saturating_div(2));
        let Ok(page_middle) = fitted::<u32, i32>(column.left.saturating_add(column.width.saturating_div(2)));
        let Ok(right_edge) = fitted::<u32, i32>(room.width.saturating_sub(4));
        let Ok(back_bar) = turn_bar(room, Turn::Back);
        let Ok(half) = fitted::<u32, i32>(TURN_BAR.height.saturating_div(2));
        let beside_the_rail = back_bar.at.y.saturating_add(half);

        assert_eq!(back.covers(Point { x: 4, y: beside_the_rail }), Ok(console_core_shapes::Covers::Yes));
        assert_eq!(forward.covers(Point { x: right_edge, y: beside_the_rail }), Ok(console_core_shapes::Covers::Yes));
        assert_eq!(back.covers(Point { x: left.saturating_add(10), y: middle }), Ok(console_core_shapes::Covers::No), "the text is not the bar");
        assert_eq!(back.covers(Point { x: 4, y: 40 }), Ok(console_core_shapes::Covers::No), "the bar is not the whole height");
        assert!(back.size.height < room.height.saturating_div(4), "the bar is small");

        for turn in [Turn::Back, Turn::Forward] {
            let Ok(target) = turn_target(room, turn);

            assert_eq!(target.covers(Point { x: page_middle, y: middle }), Ok(console_core_shapes::Covers::No));
        }

        for short in [room, Size { width: 1280, height: 560 }] {
            let Ok(bar) = turn_bar(short, Turn::Back);

            for index in (0_u32..).zip(RAIL).map(|(index, _)| index) {
                let Ok(button) = rail_button(short, index);

                assert!(button.at.y.saturating_add(36) < bar.at.y, "the rail stands clear of the bar that turns back");
            }
        }

        Ok(())
    }

    const ROOM: Size<u32> = Size { width: 1280, height: 800 };

    #[derive(Debug)]
    enum Error {
        NotFound(&'static str),
        Disk(std::io::Error),
        TemporaryDirectory(console_core_temporary_directories::Unmade),
        Palette(console_core_color::palette::PaletteError),
    }

    impl std::fmt::Display for Error {
        fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Error::NotFound(what) => write!(to, "{what}"),
                Error::Disk(fault) => write!(to, "{fault}"),
                Error::TemporaryDirectory(fault) => write!(to, "{fault}"),
                Error::Palette(fault) => write!(to, "{fault}"),
            }
        }
    }

    impl From<&'static str> for Error {
        fn from(what: &'static str) -> Self {
            Error::NotFound(what)
        }
    }

    impl From<std::io::Error> for Error {
        fn from(fault: std::io::Error) -> Self {
            Error::Disk(fault)
        }
    }

    impl From<console_core_temporary_directories::Unmade> for Error {
        fn from(fault: console_core_temporary_directories::Unmade) -> Self {
            Error::TemporaryDirectory(fault)
        }
    }

    impl From<console_core_color::palette::PaletteError> for Error {
        fn from(fault: console_core_color::palette::PaletteError) -> Self {
            Error::Palette(fault)
        }
    }

    fn reading(named: &str) -> Result<(App, PathBuf), Error> {
        let home = console_core_temporary_directories::fresh(named)?;
        let palette = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../files/usr/local/lib/console/palette.sh"))?;
        let Ok(spent) = console_core_color::palette::read(&palette);
        let wearing = Wearing::out_of(&spent)?;
        let Ok(library) = Library::of(home.clone());
        let Ok(annotations) = Annotations::empty();
        let mut app = App {
            library,
            view: View::Library,
            wearing,
            appearance: Appearance::default(),
            pointer_down: None,
            pointer_at: None,
            pinch: None,
            annotations,
        };
        let Ok(()) = app.open_path(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/a-short-book.epub")));

        match &mut app.view {
            View::Reader(reader) => {
                let Ok(()) = reader.ensure_layout(ROOM);
            },
            View::Library => return Err(Error::NotFound("the book did not open")),
        }

        Ok((app, home))
    }

    fn reader(app: &App) -> Result<&Reader, Error> {
        match &app.view {
            View::Reader(reader) => Ok(reader),
            View::Library => Err(Error::NotFound("the book closed")),
        }
    }

    fn letter_on_the_page(app: &App, word: &str) -> Result<Point<i32>, Error> {
        let reader = reader(app)?;
        let Ok(lines) = reader.page_lines();
        let line = lines.iter().find(|line| line.text.contains(word)).ok_or("the word is on the page")?;
        let before = line.text.split_once(word).map(|(before, _)| before).ok_or("the word is on the line")?;
        let Ok(across) = width_of(before, line.style, reader.appearance);
        let Ok(origin) = line_origin(line, ROOM);
        let Ok(at) = offset(origin, Point { x: across.saturating_add(3), y: 10 });

        Ok(at)
    }

    fn drag(app: &mut App, from: Point<i32>, to: Point<i32>) -> Result<(), Never> {
        let Ok(from) = from.map(f64::from);
        let Ok(to) = to.map(f64::from);

        for event in [PointerEvent::Down { at: (from.x, from.y) }, PointerEvent::Moved { at: (to.x, to.y) }, PointerEvent::Up] {
            let Ok(_update) = app.pointer_event(event, ROOM);
        }

        Ok(())
    }

    fn kept(app: &App) -> Result<String, Error> {
        let reader = reader(app)?;
        let at = reader.notes_at.as_ref().ok_or("the notes have a file")?;
        let text = std::fs::read_to_string(at)?;

        Ok(text)
    }

    #[test]
    fn a_hand_on_the_page_while_reading_marks_nothing() -> Result<(), Error> {
        let (mut app, home) = reading("books-reading-marks-nothing")?;
        let from = letter_on_the_page(&app, "years")?;
        let to = letter_on_the_page(&app, "precisely")?;
        let Ok(()) = drag(&mut app, from, to);
        let reader = reader(&app)?;

        assert_eq!(app.annotations.notes, Vec::new());
        assert_eq!(reader.page, 0, "a drag down the words is not a swipe");
        assert!(reader.notes_at.as_ref().is_some_and(|at| !at.exists()), "nothing was written");

        std::fs::remove_dir_all(&home)?;

        Ok(())
    }

    #[test]
    fn the_marker_highlights_whole_words_into_the_file_and_a_tap_takes_it_off() -> Result<(), Error> {
        let (mut app, home) = reading("books-marker")?;
        let Ok(_update) = app.press(Rail::Tool(Tool::Highlight), ROOM);
        let from = letter_on_the_page(&app, "ars ago")?;
        let to = letter_on_the_page(&app, "mind")?;
        let Ok(()) = drag(&mut app, from, to);
        let highlighted = kept(&app)?;

        assert!(highlighted.contains("> Some ==years ago, never mind== how long"), "{highlighted}");
        assert!(highlighted.contains("## Loomings"), "the note says which chapter it is in");

        let Ok(()) = drag(&mut app, from, from);
        let taken_off = kept(&app)?;

        assert_eq!(app.annotations.notes, Vec::new(), "a tap on a highlight takes it off");
        assert!(!taken_off.contains("=="), "{taken_off}");

        std::fs::remove_dir_all(&home)?;

        Ok(())
    }

    #[test]
    fn handwriting_is_kept_beside_its_paragraph_and_drawn_there_again() -> Result<(), Error> {
        let (mut app, home) = reading("books-pencil")?;
        let Ok(_update) = app.press(Rail::Tool(Tool::Pen), ROOM);
        let from = letter_on_the_page(&app, "money")?;
        let to = Point { x: from.x.saturating_add(80), y: from.y.saturating_add(4) };
        let Ok(()) = drag(&mut app, from, to);
        let written = kept(&app)?;

        assert!(written.contains("> Some years ago, never mind how long precisely, having little or no money in my purse."), "{written}");
        assert!(written.contains("<svg"), "{written}");

        let Ok(read) = notes::parse(&written);
        let reader = reader(&app)?;

        assert_eq!(read, app.annotations.notes, "the file says everything the reader holds");

        let look = Look { ground: app.wearing.ground, ink: app.wearing.text, pen: app.wearing.pink, highlight: app.wearing.ground };
        let Ok(drawn) = stroke_shapes(reader, &app.annotations, ROOM, look);

        assert_eq!(
            drawn.first(),
            Some(&Shape::Line(console_core_shapes::Line { from, to, width: PEN, color: look.pen })),
            "the stroke is drawn where the hand was"
        );

        std::fs::remove_dir_all(&home)?;

        Ok(())
    }

    fn resize(app: &mut App, size: TextSize) -> Result<(), Never> {
        app.appearance.text_size = size;

        match &mut app.view {
            View::Reader(reader) => {
                reader.appearance.text_size = size;
                reader.layout_size = Size { width: 0, height: 0 };

                let Ok(()) = reader.ensure_layout(ROOM);
            },
            View::Library => {},
        }

        Ok(())
    }

    #[test]
    fn handwriting_stays_beside_its_word_when_the_text_is_resized() -> Result<(), Error> {
        let (mut app, home) = reading("books-pencil-resized")?;
        let Ok(_update) = app.press(Rail::Tool(Tool::Pen), ROOM);
        let from = letter_on_the_page(&app, "money")?;
        let Ok(()) = drag(&mut app, from, Point { x: from.x.saturating_add(40), y: from.y });
        let before = reader(&app)?;
        let spot = app.annotations.notes.first().map(|note| note.spot).ok_or("a note was written")?;
        let Ok(was) = before.origin_on_page(spot, ROOM);
        let was = was.ok_or("the word it was written beside is on the page")?;
        let Ok(()) = resize(&mut app, TextSize(33));
        let after = reader(&app)?;
        let Ok(now) = after.origin_on_page(spot, ROOM);
        let now = now.ok_or("the word is still on the page at the new size")?;
        let look = Look { ground: app.wearing.ground, ink: app.wearing.text, pen: app.wearing.pink, highlight: app.wearing.ground };
        let Ok(drawn) = stroke_shapes(after, &app.annotations, ROOM, look);
        let Ok(offset) = notes::scaled(Point { x: from.x.saturating_sub(was.x), y: from.y.saturating_sub(was.y) }, Scale { written: 22, now: 33 });

        assert_ne!(was, now, "the word moved when the text grew");
        assert_eq!(
            drawn.first().map(|shape| match shape {
                Shape::Line(line) => Some(line.from),
                Shape::Panel(_) | Shape::Text(_) | Shape::Picture(_) | Shape::Cropped(_) | Shape::Clip(_) => None,
            }),
            Some(Some(Point { x: now.x.saturating_add(offset.x), y: now.y.saturating_add(offset.y) })),
            "the stroke starts where it did beside the word, grown with the text"
        );

        std::fs::remove_dir_all(&home)?;

        Ok(())
    }

    #[test]
    fn undo_and_redo_reach_only_the_page_that_is_open() -> Result<(), Error> {
        let (mut app, home) = reading("books-undo")?;
        let Ok(_update) = app.press(Rail::Tool(Tool::Highlight), ROOM);
        let at = letter_on_the_page(&app, "Ishmael")?;
        let Ok(()) = drag(&mut app, at, at);

        assert_eq!(app.annotations.notes.len(), 1);

        let Ok(_update) = app.turn_page(Turn::Forward, ROOM);
        let Ok(_update) = app.press(Rail::Undo, ROOM);

        assert_eq!(app.annotations.notes.len(), 1, "undo on the next chapter left the note on the first alone");

        let Ok(_update) = app.turn_page(Turn::Back, ROOM);
        let Ok(_update) = app.press(Rail::Undo, ROOM);

        assert_eq!(app.annotations.notes.len(), 0, "undo on its own page took it back");

        let Ok(_update) = app.turn_page(Turn::Forward, ROOM);
        let Ok(_update) = app.press(Rail::Redo, ROOM);

        assert_eq!(app.annotations.notes.len(), 0, "redo on another page brought nothing back");

        let Ok(_update) = app.turn_page(Turn::Back, ROOM);
        let Ok(_update) = app.press(Rail::Redo, ROOM);
        let restored = kept(&app)?;

        assert!(restored.contains("==Ishmael.=="), "{restored}");

        std::fs::remove_dir_all(&home)?;

        Ok(())
    }

    #[test]
    fn a_highlight_taken_off_by_a_tap_comes_back_with_undo() -> Result<(), Error> {
        let (mut app, home) = reading("books-undo-tap")?;
        let Ok(_update) = app.press(Rail::Tool(Tool::Highlight), ROOM);
        let at = letter_on_the_page(&app, "Ishmael")?;
        let Ok(()) = drag(&mut app, at, at);
        let Ok(()) = drag(&mut app, at, at);

        assert_eq!(app.annotations.notes.len(), 0, "the second tap took it off");

        let Ok(_update) = app.press(Rail::Undo, ROOM);
        let restored = kept(&app)?;

        assert!(restored.contains("==Ishmael.=="), "undo put back what the tap took off: {restored}");

        let Ok(_update) = app.press(Rail::Redo, ROOM);
        assert_eq!(app.annotations.notes.len(), 0, "redo took it off again");

        std::fs::remove_dir_all(&home)?;

        Ok(())
    }

    #[test]
    fn the_eraser_takes_off_the_stroke_and_the_highlight_under_it() -> Result<(), Error> {
        let (mut app, home) = reading("books-eraser")?;
        let Ok(_update) = app.press(Rail::Tool(Tool::Pen), ROOM);
        let from = letter_on_the_page(&app, "money")?;
        let to = Point { x: from.x.saturating_add(80), y: from.y };
        let Ok(()) = drag(&mut app, from, to);
        let Ok(_update) = app.press(Rail::Tool(Tool::Highlight), ROOM);
        let word = letter_on_the_page(&app, "Ishmael")?;
        let Ok(()) = drag(&mut app, word, word);

        assert_eq!(app.annotations.notes.len(), 2);

        let Ok(_update) = app.press(Rail::Tool(Tool::Eraser), ROOM);
        let away = Point { x: from.x.saturating_add(200), y: from.y.saturating_sub(60) };
        let Ok(()) = drag(&mut app, away, away);

        assert_eq!(app.annotations.notes.len(), 2, "rubbing where nothing is written takes nothing");

        let Ok(()) = drag(&mut app, Point { x: from.x.saturating_add(40), y: from.y.saturating_add(3) }, word);
        let rubbed = kept(&app)?;
        assert_eq!(app.annotations.notes, Vec::new(), "{rubbed}");

        let Ok(_update) = app.press(Rail::Undo, ROOM);
        assert_eq!(app.annotations.notes.len(), 1, "undo put back the last thing rubbed out");

        std::fs::remove_dir_all(&home)?;

        Ok(())
    }

    #[test]
    fn y_on_the_controller_walks_the_tools_the_way_tab_does() -> Result<(), Error> {
        let (mut app, home) = reading("books-y")?;
        let mut walked = Vec::new();

        for _ in 0..5 {
            let Ok(_update) = app.key_pressed(Keysym::F18, ROOM);

            walked.push(app.annotations.tool);
        }

        assert_eq!(walked, vec![Tool::Highlight, Tool::Text, Tool::Pen, Tool::Eraser, Tool::Read]);

        std::fs::remove_dir_all(&home)?;

        Ok(())
    }

    #[test]
    fn without_a_hand_a_paragraph_is_picked_and_highlighted_whole() -> Result<(), Error> {
        let (mut app, home) = reading("books-keys")?;

        for key in [Keysym::Tab, Keysym::Down, Keysym::Down, Keysym::Return] {
            let Ok(_update) = app.key_pressed(key, ROOM);
        }

        let highlighted = kept(&app)?;

        assert!(highlighted.contains("> ==Call me Ishmael.=="), "{highlighted}");
        let after = reader(&app)?;

        assert_eq!(after.page, 0, "down picked a paragraph rather than turning the page");

        std::fs::remove_dir_all(&home)?;

        Ok(())
    }

    fn note_frames_now(app: &App) -> Result<Vec<NoteFrame>, Error> {
        let reader = reader(app)?;
        let Ok(frames) = note_frames(reader, &app.annotations, ROOM);

        Ok(frames)
    }

    fn middle(panel: Panel) -> Result<Point<i32>, Never> {
        offset(panel.at, Point { x: panel.size.width.saturating_div(2), y: panel.size.height.saturating_div(2) })
    }

    #[test]
    fn a_typed_note_is_written_under_its_words_and_closed_to_its_icon() -> Result<(), Error> {
        let (mut app, home) = reading("books-typed")?;
        let Ok(_update) = app.press(Rail::Tool(Tool::Text), ROOM);
        let at = letter_on_the_page(&app, "Ishmael")?;
        let Ok(()) = drag(&mut app, at, at);

        for key in [Keysym::W, Keysym::h, Keysym::o, Keysym::x, Keysym::BackSpace, Keysym::question, Keysym::Escape] {
            let Ok(_update) = app.key_pressed(key, ROOM);
        }

        let written = kept(&app)?;

        assert!(written.contains("> Call me ==Ishmael.==\n\nWho?\n"), "{written}");
        assert!(written.contains("<!-- books text open "), "{written}");

        let frames = note_frames_now(&app)?;
        let (card, close) = match frames.as_slice() {
            [NoteFrame { shown: Shown::Card { card, close, lines }, .. }] => {
                assert_eq!(lines, &vec!["Who?".to_string()], "the card shows what was typed, and backspace typed rather than closing the book");

                (*card, *close)
            },
            _ => return Err(Error::NotFound("one card is up")),
        };

        assert!(card.at.y > at.y, "the card is under the words it is about");

        let Ok(cross) = middle(close);
        let Ok(()) = drag(&mut app, cross, cross);
        let closed = kept(&app)?;

        assert!(closed.contains("<!-- books text closed "), "{closed}");

        let frames = note_frames_now(&app)?;
        let icon = match frames.as_slice() {
            [NoteFrame { shown: Shown::Icon(icon), .. }] => *icon,
            _ => return Err(Error::NotFound("the note closed to its icon")),
        };
        let Ok(_update) = app.press(Rail::Tool(Tool::Read), ROOM);
        let Ok(tapped) = middle(icon);
        let Ok(()) = drag(&mut app, tapped, tapped);
        let frames = note_frames_now(&app)?;

        assert!(matches!(frames.as_slice(), [NoteFrame { shown: Shown::Card { .. }, .. }]), "a tap on the icon opens it, whatever is in the hand");

        let Ok(_update) = app.key_pressed(Keysym::Escape, ROOM);
        let frames = note_frames_now(&app)?;

        assert!(matches!(frames.as_slice(), [NoteFrame { shown: Shown::Icon(_), .. }]), "B put the open note away before the book");

        let Ok(_update) = app.key_pressed(Keysym::Escape, ROOM);

        assert!(matches!(app.view, View::Library), "and then the book");

        std::fs::remove_dir_all(&home)?;

        Ok(())
    }

    fn opening(named: &str, book: &Path) -> Result<(App, PathBuf), Error> {
        let (mut app, home) = reading(named)?;
        let Ok(()) = app.open_path(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join(book));
        let Ok(_update) = app.press(Rail::Tool(Tool::Highlight), ROOM);
        let Ok(()) = shown(&mut app);

        Ok((app, home))
    }

    fn shown(app: &mut App) -> Result<(), Never> {
        let Ok(look) = look(&app.wearing, app.appearance);

        match &mut app.view {
            View::Reader(reader) => {
                let Ok(_shapes) = reader_shapes(reader, &app.annotations, look, ROOM, ROOM);
            },
            View::Library => {},
        }

        Ok(())
    }

    fn word_on_the_picture(app: &App, word: &str) -> Result<Point<i32>, Error> {
        let reader = reader(app)?;
        let Ok(paragraphs) = reader.paragraphs();
        let Ok(frame) = reader.picture_frame(ROOM);
        let frame = frame.ok_or("the page is drawn")?;
        let paragraphs = paragraphs.ok_or("the page's words are read")?;
        let area = paragraphs
            .iter()
            .flat_map(|paragraph| paragraph.words.iter().map(move |each| (paragraph, each)))
            .find(|(paragraph, each)| {
                let Ok(start) = console_core_number_conversion::index(each.run.start);
                let Ok(end) = console_core_number_conversion::index(each.run.end);

                paragraph.text.get(start..end) == Some(word)
            })
            .map(|(_, each)| each.area)
            .ok_or("the word is on the page")?;
        let Ok((left, _)) = on_the_frame(area, frame);
        let Ok(tall) = fitted::<u32, i32>(frame.size.height);
        let Ok(top) = console_core_number_conversion::whole_i32(area.top * f64::from(tall));

        Ok(Point { x: left.x.saturating_add(4), y: frame.at.y.saturating_add(top).saturating_add(6) })
    }

    #[test]
    fn a_pdf_is_highlighted_by_the_words_it_carries() -> Result<(), Error> {
        let (mut app, home) = opening("books-pdf", Path::new("a-page.pdf"))?;
        let from = word_on_the_picture(&app, "years")?;
        let to = word_on_the_picture(&app, "mind")?;
        let Ok(()) = drag(&mut app, from, to);
        let highlighted = kept(&app)?;

        assert!(highlighted.contains("## Page 1"), "{highlighted}");
        assert!(highlighted.contains("> Some ==years ago, never mind== how long precisely, having little."), "{highlighted}");

        std::fs::remove_dir_all(&home)?;

        Ok(())
    }

    #[test]
    fn a_comic_is_read_off_the_picture_and_highlighted_the_same_way() -> Result<(), Error> {
        let (mut app, home) = opening("books-comic", Path::new("a-comic.cbz"))?;
        let Ok(patience) = console_waiting::Schedule::asking_every(Duration::from_secs(30), Duration::from_millis(100));
        let Ok(recognized) = console_waiting::until_handed(patience, &mut app, |app| {
            let Ok(update) = app.poll_live_text();

            Ok(match update {
                Update::Redraw => console_waiting::Ready::Yes,
                Update::None | Update::Exit => console_waiting::Ready::NotYet,
            })
        });

        assert_eq!(recognized, console_waiting::Outcome::Happened, "tesseract read the page");

        let at = word_on_the_picture(&app, "Ishmael.")?;
        let Ok(()) = drag(&mut app, at, at);
        let highlighted = kept(&app)?;

        assert!(highlighted.contains("> Call me ==Ishmael.=="), "{highlighted}");

        std::fs::remove_dir_all(&home)?;

        Ok(())
    }

    #[test]
    fn what_is_typed_sits_in_the_middle_of_the_search() {
        let Ok(search) = search_field(Size { width: 1024, height: 640 });
        let style = TextStyle { size: 16, weight: Weight::Plain, width: search.size.width.saturating_sub(40) };
        let shown = format!("\u{2315}  {SEARCH}");
        let Ok(font) = font(SANS, style.size);
        let Ok(measured) = console_draw_painting::measure_text(Run { said: &shown, weight: style.weight, width: style.width }, &font);
        let Ok(at) = typed_at(&search, &shown, style);
        let Ok(tall) = fitted::<u32, i32>(measured.height);
        let Ok(field) = fitted::<u32, i32>(search.size.height);
        let above = at.y.saturating_sub(search.at.y);
        let below = search.at.y.saturating_add(field).saturating_sub(at.y.saturating_add(tall));

        assert!(above.abs_diff(below) <= 1, "{above} above the words and {below} under them");
    }
}
