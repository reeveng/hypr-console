//! The menu: the surface every application on this machine is opened from.
//!
//! Applications come out in the order you actually use them: the ones you open
//! most, most often, and everything else alphabetically after them.
//!
//! It is drawn as a panel, like the settings, the guide and the files. It was
//! wofi for a long time, and wofi cost four separate things: it listed itself
//! under its own name, so the bar could not say whether the menu was up; it
//! could not be told to shrink when the on-screen keyboard took the bottom of
//! the screen; it wanted one press to highlight a row and another to take it;
//! and the icon that opened it could not close it. None of the four is a fault
//! in wofi. They are one fact: the menu was the only surface on this machine
//! that was not ours.
//!
//! What is typed is a name to the machine and a question to the browser, and
//! it does not stop being the second because it was the first. The last row of
//! the list offers to ask it, under everything the machine answered with.
//!
//! ## And it is where the home screen is filled from
//!
//! The home screen is the handful of applications someone put where they want
//! them, and this is every application there is, in the order they are used,
//! found by typing. They are the same list read two ways, which is why they
//! read it out of the same place -- and it is why what goes on the home screen
//! is decided here rather than on a card of its own. That card existed, it was
//! this list with a word beside some of the rows, and the word is all that is
//! left of it: Y puts the row you are standing on on the home screen, or takes
//! it off, and the row says which it is.
//!
//! A is untouched by that. A row does one thing on A -- open the application --
//! because one thing is what a thumb should have to know, and everything else
//! it could be done to is behind Y. That is what Y is on the files, on the
//! music, on the downloads and on the home screen itself.
//!
//! ## Except when the home screen asked
//!
//! `--place` is the home screen opening this on one of its empty squares, and
//! then the whole card is that one question: A puts what it is standing on
//! there, on the square that asked, and there is no Y and no browser. Subject
//! square it was travels as `console_home_screen::Spot::said` and comes back through
//! `Spot::read`, so only the home screen's own model says how a square is
//! spelled.
//!
//! It lives above `console_applications` rather than in it because of that reading:
//! the menu crate is what applications this machine has, the home screen is
//! built on top of it, and a surface that has to know about both belongs over
//! the two of them.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use console_applications::{counts, entry, found, narrow};
use console_default_applications::engines;
use console_core_external_programs::Program;
use console_home_screen::{HomeScreen, Spot};
use console_home_screen::shape::Shape;
use console_core_never::Never;
use console_panel::actor::{self, Address, Answer};
use console_panel::card::{Card, Door};
use console_panel::picker::Again;
use console_panel::page::{Aside, Handler, Page, Picture, Row, Rows};














pub const WHO: &str = "launcher";

const DOOR: &str = "menu";

const KEEP: &str = "--keep";

type Shared = Arc<Cache>;

#[derive(Default)]
struct Cache {
    all: OnceLock<Everything>,
    before: OnceLock<Everything>,
}

pub fn door(arguments: &[String]) -> Result<Door, Never> {
    let again = match arguments.iter().any(|word| word == KEEP) {
        true => Again::Keeps,
        false => Again::Closes,
    };

    Door::new(DOOR, again)
}

pub fn card(arguments: &[String]) -> Result<Card, Never> {
    let Ok(word) = actor::supervise(|| Word { said: String::new() });
    let typed = word.addr.clone();
    let Ok(going) = asked_for(arguments);
    let kept: Shared = Arc::default();

    let Ok(card) = Card::new(Arc::new(move || {
        let Ok(pages) = pages(&typed, &kept, going);

        pages
    }));

    card.shutting(Box::new(move || word.shutdown()))
}

struct Everything {
    apps: BTreeMap<String, entry::Application>,
    icon: BTreeMap<String, String>,
    order: Vec<String>,
}

struct Word {
    said: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Narrowed {
    Same,
    Different,
}

enum Message {
    Reply(Answer<String>),
    Type { word: String, answer: Answer<Narrowed> },
}

impl actor::Machine for Word {
    type Message = Message;

    fn step(self, message: Message) -> Self {
        match message {
            Message::Reply(answer) => {
                let _ = answer.say(self.said.clone());
                self
            },
            Message::Type { word, answer } => {
                let narrowed = match self.said == word {
                    true => Narrowed::Same,
                    false => Narrowed::Different,
                };
                let _ = answer.say(narrowed);
                Word { said: word }
            },
        }
    }
}

type Typed = Address<Message>;

const ABOUT: &str = "Type to narrow the list";

const ON: &str = "on the home screen";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum For {
    Opening,
    Placing(Spot),
}

fn file() -> Result<Option<PathBuf>, Never> {
    let hers = console_core_places::home()?;

    match hers {
        Some(hers) => {
            let at = console_home_screen::file(&hers)?;

            Ok(Some(at))
        }
        None => Ok(None),
    }
}

fn shape() -> Result<Shape, Never> {
    let hers = console_core_places::home()?;

    let at = match hers {
        Some(hers) => console_home_screen::shape::at(&hers)?,
        None => return Ok(Shape::USUAL),
    };

    match std::fs::read_to_string(at) {
        Ok(said) => Shape::read(&said),
        Err(_) => Ok(Shape::USUAL),
    }
}

fn home() -> Result<HomeScreen, Never> {
    let Ok(kept) = file();

    let at = match kept {
        Some(at) => at,
        None => return Ok(HomeScreen::default()),
    };

    let Ok(held) = console_core_atomic_writes::read(&at);

    match held {
        console_core_atomic_writes::Stored::Text(said) => HomeScreen::read(&said),
        console_core_atomic_writes::Stored::Absent => Ok(HomeScreen::default()),
        console_core_atomic_writes::Stored::Failed(fault) => {
            eprintln!("launcher: {}: {fault}", at.display());

            Ok(HomeScreen::default())
        },
    }
}

fn keep(home: &HomeScreen) -> Result<(), Never> {
    let Ok(kept) = file();

    let at = match kept {
        Some(at) => at,
        None => {
            eprintln!("launcher: no home to keep the home screen in; leaving it as it is");

            return Ok(());
        }
    };

    match at.parent() {
        Some(above) => {
            let _ = std::fs::create_dir_all(above);
        }
        None => {},
    }

    let said = home.written()?;

    match console_core_atomic_writes::whole(&at, said.as_bytes()) {
        Ok(()) => {},
        Err(fault) => eprintln!("launcher: {}: {fault}", at.display()),
    }

    Ok(())
}

fn turned(name: &str) -> Result<(), Never> {
    let Ok(home) = home();
    let Ok(shape) = shape();
    let Ok(turned) = turning(home, shape, name);
    let Ok(()) = keep(&turned);

    Ok(())
}

fn turning(mut home: HomeScreen, shape: Shape, name: &str) -> Result<HomeScreen, Never> {
    let placed = home.where_(name)?;

    match placed {
        Some(_) => home.forget(name)?,
        None => {
            let spot = home.first_free(shape)?;

            home.place(spot, name)?;
        },
    }

    Ok(home)
}

fn placed(spot: Spot, name: &str) -> Result<(), Never> {
    let Ok(home) = home();
    let Ok(placed) = placing(home, spot, name);
    let Ok(()) = keep(&placed);

    Ok(())
}

fn placing(mut home: HomeScreen, spot: Spot, name: &str) -> Result<HomeScreen, Never> {
    home.forget(name)?;

    home.place(spot, name)?;

    Ok(home)
}

fn everything() -> Result<Everything, Never> {
    let machine = found::machine()?;

    counting(machine)
}

fn everything_before() -> Result<Everything, Never> {
    let remembered = found::remembered()?;

    counting(remembered)
}

fn counting(found: found::Found) -> Result<Everything, Never> {
    let names: Vec<String> = found.apps.keys().cloned().collect();

    let counted = found::counted()?;

    let order = counts::order(&names, &counted)?;

    Ok(Everything { apps: found.apps, icon: found.icon, order })
}


fn all(kept: &Shared) -> Result<&Everything, Never> {
    Ok(kept.all.get_or_init(|| {
        let Ok(everything) = everything();

        everything
    }))
}

fn app_row(all: &Everything, name: &str, going: For, on: &HomeScreen) -> Result<Row, Never> {
    let picture =
        all.icon.get(name).map_or(Picture::Space, |at| Picture::At(PathBuf::from(at)));
    let named = name.to_string();

    match going {
        For::Placing(spot) => {
            let Ok(places) = Handler::call(move |_| {
                let Ok(()) = placed(spot, &named);
                true
            });
            let Ok(row) = Row::new(name, Aside(""), places);

            row.picturing(picture)
        }
        For::Opening => {
            let app = all.apps.get(name).cloned();
            let placed = on.where_(name)?;

            let aside = match placed {
                Some(_) => ON,
                None => "",
            };
            let switching = name.to_string();

            let Ok(starts) = Handler::call(move |_| {
                let Ok(()) = start(app.as_ref(), &named);
                true
            });
            let Ok(row) = Row::new(name, Aside(aside), starts);
            let Ok(pictured) = row.picturing(picture);

            pictured.offering(move |showing| {
                let Ok(()) = turned(&switching);
                showing.refresh();

                false
            })
        },
    }
}

fn looking_up_row(said: &str) -> Result<Row, Never> {
    let word = said.to_string();
    let Ok(looks) = Handler::call(move |_| {
        let Ok(()) = looked_up(&word);
        true
    });
    let Ok(row) = Row::new(&format!("Look up {said:?}"), Aside(""), looks);

    row.picturing(Picture::Space)
}

fn rows(typed: &Typed, all: &Everything, going: For) -> Result<Vec<Row>, Never> {
    let mut word = String::new();

    match typed.ask(Message::Reply) {
        Ok(said) => word = said,
        Err(_) => {},
    }

    let Ok(on) = home();
    let standing = narrow::matching(&all.order, &word)?;
    let mut rows: Vec<Row> = standing
        .iter()
        .map(|name| {
            let Ok(row) = app_row(all, name, going, &on);

            row
        })
        .collect();
    let said = word.trim();

    match !said.is_empty() && going == For::Opening {
        true => {
            let Ok(row) = looking_up_row(said);

            rows.push(row);
        },
        false => {},
    }

    Ok(rows)
}

fn before(typed: &Typed, kept: &Shared, going: For) -> Result<Vec<Row>, Never> {
    let Ok(listed) = kept_list(kept);

    rows(typed, listed, going)
}

fn kept_list(kept: &Shared) -> Result<&Everything, Never> {
    Ok(kept.before.get_or_init(|| {
        let Ok(everything) = everything_before();

        everything
    }))
}

fn heading(going: For) -> Result<&'static str, Never> {
    Ok(match going {
        For::Opening => "Menu",
        For::Placing(_) => "Put one on the home screen",
    })
}

fn pages(typed: &Typed, kept: &Shared, going: For) -> Result<Vec<Page>, Never> {
    let listing = typed.clone();
    let waiting = typed.clone();
    let typing = typed.clone();
    let reading = Arc::clone(kept);
    let remembered = Arc::clone(kept);

    let Ok(heading) = heading(going);

    let Ok(asked) = Rows::asked(move || {
        let Ok(all) = all(&reading);
        let Ok(rows) = rows(&listing, all, going);

        rows
    });
    let Ok(page) = Page::new(heading, asked);
    let Ok(page) = page.meanwhile(move || {
        let Ok(before) = before(&waiting, &remembered, going);

        before
    });
    let Ok(page) = page.searching(ABOUT, move |showing, word| {
        let narrowed = typing.ask(|answer| Message::Type { word: word.to_string(), answer });

        match matches!(narrowed, Ok(Narrowed::Different)) {
            true => showing.replace(0),
            false => {},
        }
    });

    Ok(vec![page])
}


fn asked_for(asked: &[String]) -> Result<For, Never> {
    let said = match asked.iter().skip_while(|word| *word != "--place").nth(1) {
        Some(said) => said.as_str(),
        None => return Ok(For::Opening),
    };

    let read = Spot::read(said)?;

    Ok(match read {
        Some(spot) => For::Placing(spot),
        None => {
            eprintln!("launcher: --place {said:?} is not a square, so this is the menu");

            For::Opening
        },
    })
}

fn start(app: Option<&entry::Application>, chosen: &str) -> Result<(), Never> {
    match app {
        Some(app) => {
            let command = found::command(app)?;

            match command {
                Some(arguments) => {
                    let Ok(()) = console_panel::running::left_running(&arguments);
                }
                None => {}
            }
        }
        None => {
            let Ok(()) = looked_up(chosen);
        },
    }

    Ok(())
}

fn looked_up(said: &str) -> Result<(), Never> {
    let chosen = engines::chosen()?;
    let known = engines::one(&chosen)?;

    let engine = match known {
        Some(engine) => engine,
        None => return Ok(()),
    };

    let asked = engines::address(said, engine)?;

    let address = match asked {
        Some(address) => address,
        None => return Ok(()),
    };

    eprintln!("the menu was asked {said:?}: {address}");
    let Ok(opening) = opening(&address);
    let Ok(()) = console_panel::running::left_running(&opening);

    Ok(())
}

fn opening(address: &str) -> Result<Vec<String>, Never> {
    Program::XdgOpen.arguments(&[address])
}


#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(answer) = answer;

        answer
    }

    const GRID: Shape = Shape::USUAL;

    fn taken() -> HomeScreen {
        let mut home = HomeScreen::default();
        ok(home.place(Spot { pane: 0, row: 0, column: 0 }, "Files"));
        ok(home.place(Spot { pane: 0, row: 0, column: 1 }, "Music"));
        home
    }

    #[test]
    fn y_is_a_switch() {
        let Ok(on) = turning(taken(), GRID, "Download");
        let first = Spot { pane: 0, row: 0, column: 0 };

        let free = Spot { pane: 0, row: 0, column: 2 };

        assert_eq!(ok(on.where_("Download")), Some(free), "the first free square");
        assert_eq!(ok(on.at(first)), Some("Files"), "and nothing else moved");

        let Ok(off) = turning(on, GRID, "Download");
        assert_eq!(ok(off.where_("Download")), None, "pressed again, it is off");
        assert_eq!(off, taken(), "and the home screen is where it was");
    }

    #[test]
    fn a_square_that_was_asked_for_is_the_square_it_lands_on() {
        let asked = Spot { pane: 2, row: 1, column: 3 };
        let Ok(placed) = placing(taken(), asked, "Download");

        assert_eq!(ok(placed.where_("Download")), Some(asked));
        assert_ne!(asked, ok(taken().first_free(GRID)), "the point of asking");
    }

    #[test]
    fn one_that_is_already_on_it_is_moved() {
        let asked = Spot { pane: 1, row: 0, column: 0 };
        let Ok(placed) = placing(taken(), asked, "Files");

        assert_eq!(ok(placed.where_("Files")), Some(asked));
        let first = Spot { pane: 0, row: 0, column: 0 };

        assert_eq!(ok(placed.at(first)), None, "and it left the square it was on");
    }

    #[test]
    fn what_the_words_after_the_program_name_ask_for() {
        let asked = |words: &[&str]| {
            asked_for(&words.iter().map(|word| (*word).to_string()).collect::<Vec<String>>())
        };

        let spot = Spot { pane: 2, row: 1, column: 3 };

        assert_eq!(asked(&["--place", "2.1.3"]), Ok(For::Placing(spot)));
        assert_eq!(asked(&[]), Ok(For::Opening), "the menu, opened the way it usually is");
        assert_eq!(asked(&["--keep"]), Ok(For::Opening), "the paddles, which only open it");
        assert_eq!(asked(&["--place"]), Ok(For::Opening), "a square that was not said");
        assert_eq!(asked(&["--place", "sideways"]), Ok(For::Opening), "a square that is not one");
    }
}
