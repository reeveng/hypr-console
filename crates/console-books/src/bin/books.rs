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
//! A book open is a page and nothing else: left and right turn it, and so do
//! a tap on either side of it. Where it was left is written at every turn,
//! because the moment a reader stops reading is the moment nobody is going to
//! press anything to say so. Which book is open is written when it opens and
//! taken away when it is closed, so `books` with no book named comes back to the
//! page it was on when the machine went down rather than to the library.
//!
//! It is an app, and holds the lock an app holds: opened again on the same
//! book it is already up, opened on another the one up stands down for it, and
//! the paddle puts it away when it is the app on top.
//!
//! This is a layer surface of its own rather than a panel. A panel is a card of
//! rows and a library is a grid of pictures, which is the one arrangement the
//! panel surface does not draw; the home screen is the other grid on this
//! machine and is drawn the same way, as shapes placed by arithmetic that
//! `console_books::grid` does with no screen in the room.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime};

use console_books::appearance::{Appearance, Paint, Typeface};
use console_books::flow::{self, Relative};
use console_books::library::{self, Book, CatalogEntry, Format};
use console_books::open::{self, BookError, Comic, OpenPublication};
use console_books::pages::{self, Layout, Page, Style};
use console_books::progress::{self, Fraction, Location, Position};
use console_books::reading::{self, End, PagePosition, Turn, Destination};
use console_books::grid::{self, Direction, ScrollOffset, Selection, Grid};
use console_core_color::Oklch;
use console_core_color::palette::{Wearing, WearingError};
use console_core_external_programs::Program;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_events::subscription::Received;
use console_program_contract::{Change, Topic};
use std::sync::mpsc::Receiver;
use console_core_number_conversion::{fitted, toward_zero_i32};
use console_core_internal_programs::InternalProgram;
use console_core_shapes::{Edge, Font, Panel, Picture, Pixels, Round, Shape, Text, Weight};
use console_draw_painting::{Frame, Run};
use console_draw_surface::standing::{Anchor, Closed, Keyboard, KeyboardEvent, Margin, PointerEvent, Room, Under, Wanted};
use console_draw_surface::{Keysym, Surface};
use console_panel::picker::{self, Alone};
use console_panel::pictures::{self as store, Side};

const NAMESPACE: &str = "console-books";

const LIBRARY: &str = "Library";

const SEARCH: &str = "Search";

const BOOK_STORE: &str = "Book Store";

const STORE_TAB: &str = "Books";

const CLOSE: &str = "\u{2715}";

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
    search: String,
    selected: u32,
    scroll: ScrollOffset,
}

enum Content {
    Publication { book: OpenPublication, pages: Vec<Page> },
    PortableDocument { pages: u32 },
    Comic(Comic),
}

struct Reader {
    book: Book,
    content: Content,
    section: u32,
    page: u32,
    layout_size: Size<u32>,
    typeface: Typeface,
    cached_picture: Option<((u32, u32), Pixels)>,
    restore: Restore,
    cache: PathBuf,
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
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Ok(alone) = picker::alone_as(NAMESPACE, &arguments);

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

fn surface_request() -> Result<Wanted, Never> {
    Ok(Wanted {
        namespace: NAMESPACE.to_string(),
        anchor: Anchor::Whole,
        size: Size { width: 0, height: 0 },
        margin: Margin { top: 0, right: 0, bottom: 0, left: 0 },
        keyboard: Keyboard::OnDemand,
        room: Room::Around,
        under: Under::None,
    })
}

fn run(arguments: &[String]) -> Result<(), BooksError> {
    let Ok(home) = console_core_places::home();
    let home = home.ok_or(BooksError::NoHome)?;
    let wearing = Wearing::worn().map_err(BooksError::Palette)?;
    let Ok(appearance) = Appearance::chosen();
    let Ok(library) = Library::of(home);
    let mut app = App { library, view: View::Library, wearing, appearance, pointer_down: None };

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
    let Ok(request) = surface_request();

    surface.show(&request).map_err(BooksError::Surface)?;

    event_loop(&mut app, &mut surface)
}

fn event_loop(app: &mut App, surface: &mut Surface) -> Result<(), BooksError> {
    let Ok(shelf) = library::books_folder(&app.library.home);
    let Ok(changes) = console_events::subscription::connect(&[Topic::Path(shelf)]);
    let Ok(arriving) = changes.received();
    let mut update = Update::Redraw;
    let mut drawn_at: Option<Size<u32>> = None;

    loop {
        match update {
            Update::Redraw => {
                let Ok(()) = draw(app, surface);
                let Ok(room) = logical_size(surface);

                drawn_at = Some(room);
            },
            Update::None => {},
            Update::Exit => return Ok(()),
        }

        surface.wait(&[], Some(WAITING)).map_err(BooksError::Surface)?;

        let Ok(closed) = surface.closed();

        match closed {
            Closed::Yes => return Ok(()),
            Closed::No => {},
        }

        let Ok(room) = logical_size(surface);
        let Ok(input) = handle_input(app, surface, room);
        let Ok(restyled) = app.restyled();
        let Ok(recataloged) = app.library.reload_if_changed();
        let Ok(landed) = heard(&mut app.library, arriving);
        let Ok(changed) = merge(recataloged, landed);
        let Ok(changed) = merge(changed, restyled);
        let Ok(resized) = resized(drawn_at, room);

        update = match (input, changed, resized) {
            (Update::Exit, _, _) => Update::Exit,
            (Update::Redraw, _, _) | (_, Update::Redraw, _) | (_, _, Update::Redraw) => Update::Redraw,
            (Update::None, Update::None | Update::Exit, Update::None | Update::Exit) => Update::None,
        };
    }
}

fn heard(library: &mut Library, arriving: &Receiver<Received>) -> Result<Update, Never> {
    let landed = arriving.try_iter().find_map(|received| match received {
        Received::Event(change) => Some(change),
        Received::Connected => None,
    });

    match landed {
        Some(change) => library.landed(&change),
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

        let mut library = Library {
            home,
            books: Vec::new(),
            locations,
            catalog_changed: None,
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

    fn landed(&mut self, change: &Change) -> Result<Update, Never> {
        let Ok(folder) = library::books_folder(&self.home);

        Ok(match (&change.topic, Path::new(&change.text).starts_with(&folder)) {
            (Topic::Path(_), true) => {
                let Ok(()) = self.reload();

                Update::Redraw
            },
            (Topic::Path(_), false)
            | (
                Topic::Compositor
                | Topic::Sound
                | Topic::Network
                | Topic::Wifi
                | Topic::Bluetooth
                | Topic::Battery
                | Topic::Notifications
                | Topic::Units
                | Topic::Player,
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

        let Ok(title) = library::title_from_name(&name);

        self.open_book(Book { path: at.to_path_buf(), name, format, title, cover: None })
    }

    fn open_book(&mut self, book: Book) -> Result<(), Never> {
        let location = self.library.locations.get(&book.name).copied();
        let Ok(cache) = library::cache_folder(&self.library.home);

        let Ok(open) = progress::open_path(&self.library.home);
        let held = book.path.to_string_lossy().into_owned();

        match Reader::open(book, location, cache, self.appearance.typeface) {
            Ok(reader) => {
                self.view = View::Reader(Box::new(reader));

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
        let Ok(selected) = grid::moved(selection, grid.columns, direction);
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
        let Ok(turn) = turn_of(key);

        match (turn, key) {
            (Some(turn), _) => self.turn_page(turn, room),
            (None, Keysym::Escape | Keysym::BackSpace) => self.close_book(),
            (None, _) => Ok(Update::None),
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

        Ok(Update::Redraw)
    }

    fn pointer_event(&mut self, event: PointerEvent, room: Size<u32>) -> Result<Update, Never> {
        match event {
            PointerEvent::Down { at } => {
                let Ok(at) = point_of(at);

                self.pointer_down = Some(at);

                Ok(Update::None)
            },
            PointerEvent::Up => match self.pointer_down.take() {
                Some(at) => self.tap(at, room),
                None => Ok(Update::None),
            },
            PointerEvent::Moved { .. } | PointerEvent::Scrolled { .. } | PointerEvent::Pinched { .. } | PointerEvent::Left => Ok(Update::None),
        }
    }

    fn tap(&mut self, at: Point<i32>, room: Size<u32>) -> Result<Update, Never> {
        let Ok(screen) = self.screen();
        let Ok(corner) = close_button(room);
        let Ok(on_corner) = corner.covers(at);

        match (screen, on_corner) {
            (Screen::Library, console_core_shapes::Covers::Yes) => Ok(Update::Exit),
            (Screen::Reader, console_core_shapes::Covers::Yes) => self.close_book(),
            (Screen::Library, console_core_shapes::Covers::No) => self.library_tap(at, room),
            (Screen::Reader, console_core_shapes::Covers::No) => {
                let Ok(third) = fitted::<u32, i32>(room.width.saturating_div(3));

                let turn = match at.x < third {
                    true => Turn::Back,
                    false => Turn::Forward,
                };

                self.turn_page(turn, room)
            },
        }
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
    let Ok(across) = fitted::<u32, i32>(room.width.saturating_sub(72));

    Ok(Panel {
        at: Point { x: across, y: 0 },
        size: Size { width: 72, height: 72 },
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

fn body_font(typeface: Typeface) -> Result<(Font, Weight), Never> {
    let Ok(family) = typeface.family();
    let Ok(font) = font(family, 22);

    Ok((font, Weight::Plain))
}

fn heading_font(typeface: Typeface) -> Result<(Font, Weight), Never> {
    let Ok(family) = typeface.family();
    let Ok(font) = font(family, 30);

    Ok((font, Weight::Bold))
}

fn style_font(style: Style, typeface: Typeface) -> Result<(Font, Weight), Never> {
    match style {
        Style::Body => body_font(typeface),
        Style::Heading => heading_font(typeface),
    }
}

fn line_height(style: Style, typeface: Typeface) -> Result<u32, Never> {
    let Ok((font, weight)) = style_font(style, typeface);
    let Ok(measured) = console_draw_painting::measured(Run { said: "Ag", weight, width: 4096 }, &font);

    Ok(measured.height.saturating_mul(5).saturating_div(4))
}

fn text_height(room: Size<u32>) -> Result<u32, Never> {
    Ok(room.height.saturating_sub(READING_EDGE.saturating_mul(2)).saturating_sub(24))
}

fn paginate_chapter(book: &OpenPublication, section: u32, room: Size<u32>, typeface: Typeface) -> Result<Vec<Page>, Never> {
    let Ok(at) = console_core_number_conversion::index(section);

    let blocks = match book.publication.chapters.get(at).map(|chapter| book.archive.text(chapter)) {
        Some(Ok(text)) => {
            let Ok(blocks) = flow::blocks(&text);

            blocks
        },
        Some(Err(fault)) => vec![flow::Block::Paragraph(format!("This chapter could not be read: {fault}"))],
        None => Vec::new(),
    };

    let Ok(column) = column(room);
    let Ok(height) = text_height(room);
    let Ok(line) = line_height(Style::Body, typeface);
    let Ok(heading) = line_height(Style::Heading, typeface);
    let layout = Layout { height, line_height: line, heading_height: heading };

    let wrap = |text: &str, style: Style| -> Vec<String> {
        let Ok((font, weight)) = style_font(style, typeface);
        let Ok(lines) = console_draw_painting::wrapped(Run { said: text, weight, width: column.width }, &font);

        lines
    };

    pages::paginate(&blocks, layout, &wrap)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Restore {
    None,
    Edge(End),
    Fraction(Fraction),
}

impl Reader {
    fn open(book: Book, location: Option<Location>, cache: PathBuf, typeface: Typeface) -> Result<Reader, BookError> {
        let content = match book.format {
            Format::Publication => {
                let opened = open::publication(&book.path)?;

                Content::Publication { book: opened, pages: Vec::new() }
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
            typeface,
            cached_picture: None,
            restore,
            cache,
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
            Content::Publication { book, pages } => {
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

                let Ok(laid) = paginate_chapter(book, self.section, room, self.typeface);

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
        let Ok(measured) = console_draw_painting::measured(Run { said: &line_text, weight: style.weight, width: box_wide }, &font);
        let left = box_wide.saturating_sub(measured.width).saturating_div(2);
        let Ok(at) = offset(box_at, Point { x: left, y: row.saturating_mul(line) });

        placed.push((line_text, at));
    }

    Ok(placed)
}

fn line_height_of(font: &Font, weight: Weight) -> Result<u32, Never> {
    let Ok(measured) = console_draw_painting::measured(Run { said: "Ag", weight, width: 4096 }, font);

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
    let Ok(measured) = console_draw_painting::measured(Run { said: shown, weight: style.weight, width: style.width }, &font);
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
    let Ok(ending) = open::ending(named);
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
            Content::Publication { book, pages } => match (pages.get(page), book.publication.chapters.get(section)) {
                (Some(Page::Picture(link)), Some(chapter)) => {
                    let Ok(named) = flow::resolved(Relative { base: chapter, path: link });

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

fn centered_picture(pixels: Pixels, area: Size<u32>) -> Result<Shape, Never> {
    let Ok(size) = console_pictures::fitted(Size { width: pixels.width, height: pixels.height }, area);
    let Ok(across) = fitted::<u32, i32>(area.width.saturating_sub(size.width).saturating_div(2));
    let Ok(down) = fitted::<u32, i32>(area.height.saturating_sub(size.height).saturating_div(2));

    Ok(Shape::Picture(Picture { at: Point { x: across, y: down }, size, pixels }))
}

fn text_page_shapes(lines: &[pages::Line], room: Size<u32>, ink: Oklch, typeface: Typeface) -> Result<Vec<Shape>, Never> {
    let Ok(column) = column(room);
    let Ok(left) = fitted::<u32, i32>(column.left);
    let mut shapes = Vec::new();

    for line in lines {
        let Ok((font, weight)) = style_font(line.style, typeface);
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

fn reader_shapes(reader: &mut Reader, look: Look, room: Size<u32>, device: Size<u32>) -> Result<Vec<Shape>, Never> {
    let Ok(()) = reader.ensure_layout(room);
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
            let Ok(text) = text_page_shapes(&lines, room, look.ink, reader.typeface);

            shapes.extend(text);
        },
        None => {
            let Ok(pixels) = reader.current_picture(device);

            match pixels {
                Some(pixels) => {
            let Ok(shape) = centered_picture(pixels, room);

            shapes.push(shape)
        },
                None => {},
            }
        },
    }

    let Ok(footer) = footer(reader, room, look);

    shapes.extend(footer);

    Ok(shapes)
}

fn footer(reader: &Reader, room: Size<u32>, look: Look) -> Result<Vec<Shape>, Never> {
    let Ok(location) = reader.location();
    let Ok(column) = column(room);
    let Ok(left) = fitted::<u32, i32>(column.left);
    let Ok(down) = fitted::<u32, i32>(room.height.saturating_sub(34));
    let Ok(close_across) = fitted::<u32, i32>(room.width.saturating_sub(grid::MARGIN).saturating_sub(22));
    let text = format!("{}    {}%", reader.book.title, location.percent);
    let style = TextStyle { size: 13, weight: Weight::Plain, width: column.width };
    let close_style = TextStyle { size: 22, weight: Weight::Plain, width: 40 };
    let Ok(text) = text_shape(&text, Point { x: left, y: down }, style, look.ink);
    let Ok(close) = text_shape(CLOSE, Point { x: close_across, y: 22 }, close_style, look.ink);

    Ok(vec![text, close])
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Look {
    ground: Oklch,
    ink: Oklch,
}

fn look(wearing: &Wearing, appearance: Appearance) -> Result<Look, Never> {
    Ok(Look {
        ground: match appearance.background {
            Paint::Chosen(ground) => ground,
            Paint::Desktop => wearing.ground,
        },
        ink: match appearance.text {
            Paint::Chosen(ink) => ink,
            Paint::Desktop => wearing.text,
        },
    })
}

impl App {
    fn restyled(&mut self) -> Result<Update, Never> {
        let Ok(chosen) = Appearance::chosen();

        Ok(match chosen == self.appearance {
            true => Update::None,
            false => {
                self.appearance = chosen;

                match &mut self.view {
                    View::Reader(reader) => {
                        reader.typeface = chosen.typeface;
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

            reader_shapes(reader, look, room, device)
        },
    };

    let Ok(shapes) = shapes;

    let drawn = surface.draw(|pixels, device, _scale| {
        match console_draw_painting::onto(pixels, Frame { device, points: room }, &shapes) {
            Ok(()) => {},
            Err(fault) => eprintln!("books: {fault}"),
        }

        Ok(())
    });

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
    fn a_book_that_lands_in_books_while_the_library_is_open_is_on_the_shelf() {
        let home = std::env::temp_dir().join(format!("console-books-landing-{}", std::process::id()));
        let Ok(folder) = library::books_folder(&home);
        std::fs::create_dir_all(&folder).expect("a Books folder");
        let Ok(mut open) = Library::of(home.clone());

        assert_eq!(open.books.len(), 0);

        let book = folder.join("Meditations [2680].epub");
        std::fs::write(&book, b"").expect("a finished download");
        let song = home.join("Music").join("Africa [x].opus");

        let Ok(elsewhere) = open.landed(&Change { topic: Topic::Path(folder.clone()), text: song.display().to_string() });

        assert_eq!(elsewhere, Update::None, "a song is not a book");

        let Ok(landed) = open.landed(&Change { topic: Topic::Path(folder.clone()), text: book.display().to_string() });

        assert_eq!(landed, Update::Redraw, "the finished download went unnoticed");
        assert_eq!(open.books.len(), 1);

        std::fs::remove_dir_all(&home).expect("the made-up home taken away");
    }

    #[test]
    fn a_book_is_read_on_the_page_and_in_the_ink_that_were_chosen() {
        let palette = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../files/usr/local/lib/console/palette.sh"))
            .expect("the palette as the machine spends it");
        let Ok(spent) = console_core_color::palette::read(&palette);
        let wearing = Wearing::out_of(&spent).expect("the desktop's own colors");
        let sepia = Oklch { lightness: 0.93, chroma: 0.045, hue: 72.0 };
        let brown = Oklch { lightness: 0.42, chroma: 0.12, hue: 36.0 };
        let chosen = Appearance { background: Paint::Chosen(sepia), text: Paint::Chosen(brown), typeface: Typeface::Serif };

        assert_eq!(look(&wearing, chosen), Ok(Look { ground: sepia, ink: brown }));
        assert_eq!(
            look(&wearing, Appearance::default()),
            Ok(Look { ground: wearing.ground, ink: wearing.text }),
            "nothing chosen is the desktop's own"
        );
    }

    #[test]
    fn what_is_typed_sits_in_the_middle_of_the_search() {
        let Ok(search) = search_field(Size { width: 1024, height: 640 });
        let style = TextStyle { size: 16, weight: Weight::Plain, width: search.size.width.saturating_sub(40) };
        let shown = format!("\u{2315}  {SEARCH}");
        let Ok(font) = font(SANS, style.size);
        let Ok(measured) = console_draw_painting::measured(Run { said: &shown, weight: style.weight, width: style.width }, &font);
        let Ok(at) = typed_at(&search, &shown, style);
        let Ok(tall) = fitted::<u32, i32>(measured.height);
        let Ok(field) = fitted::<u32, i32>(search.size.height);
        let above = at.y.saturating_sub(search.at.y);
        let below = search.at.y.saturating_add(field).saturating_sub(at.y.saturating_add(tall));

        assert!(above.abs_diff(below) <= 1, "{above} above the words and {below} under them");
    }
}
