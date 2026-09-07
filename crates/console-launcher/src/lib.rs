//! The menu.
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
//! The home screen is the handful of applications somebody put where they want
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
//! there, on the square that asked, and there is no Y and no browser. Which
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
use console_home_screen::{Home, Spot};
use console_home_screen::shape::Shape;
use console_core_never::Never;
use console_panel::actor::{self, Addr, Answer};
use console_panel::card::{Card, Door};
use console_panel::chooser::Again;
use console_panel::page::{Does, Page, Picture, Row, Rows};














pub const WHO: &str = "launcher";

const DOOR: &str = "menu";

const KEEP: &str = "--keep";

type Kept = Arc<Held>;

#[derive(Default)]
struct Held {
    all: OnceLock<Everything>,
    before: OnceLock<Everything>,
}

pub fn door(argv: &[String]) -> Result<Door, Never> {
    let again = match argv.iter().any(|word| word == KEEP) {
        true => Again::Keeps,
        false => Again::Closes,
    };

    Door::new(DOOR, again)
}

pub fn card(argv: &[String]) -> Result<Card, Never> {
    let Ok(word) = actor::supervise(|| Word { said: String::new() });
    let typed = word.addr.clone();
    let Ok(going) = asked_for(argv);
    let kept: Kept = Arc::default();

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
    Changed,
}

enum Msg {
    Said(Answer<String>),
    Type { word: String, answer: Answer<Narrowed> },
}

impl actor::Machine for Word {
    type Msg = Msg;

    fn step(self, message: Msg) -> Self {
        match message {
            Msg::Said(answer) => {
                let _ = answer.say(self.said.clone());
                self
            },
            Msg::Type { word, answer } => {
                let narrowed = match self.said == word {
                    true => Narrowed::Same,
                    false => Narrowed::Changed,
                };
                let _ = answer.say(narrowed);
                Word { said: word }
            },
        }
    }
}

type Typed = Addr<Msg>;

const ABOUT: &str = "Type to narrow the list";

const ON: &str = "on the home screen";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum For {
    Opening,
    Placing(Spot),
}

fn file() -> Result<PathBuf, Never> {
    let home = found::home()?;

    console_home_screen::file(&home)
}

fn shape() -> Result<Shape, Never> {
    let home = found::home()?;

    let at = console_home_screen::shape::at(&home)?;

    match std::fs::read_to_string(at) {
        Ok(said) => Shape::read(&said),
        Err(_) => Ok(Shape::USUAL),
    }
}

fn home() -> Result<Home, Never> {
    let Ok(at) = file();

    match std::fs::read_to_string(&at) {
        Ok(said) => Home::read(&said),
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Ok(Home::default()),
        Err(fault) => {
            eprintln!("launcher: {}: {fault}", at.display());

            Ok(Home::default())
        },
    }
}

fn keep(home: &Home) -> Result<(), Never> {
    let Ok(at) = file();

    match at.parent() {
        Some(above) => {
            let _ = std::fs::create_dir_all(above);
        }
        None => {},
    }

    let said = home.written()?;

    match std::fs::write(&at, said) {
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

fn turning(mut home: Home, shape: Shape, name: &str) -> Result<Home, Never> {
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

fn placing(mut home: Home, spot: Spot, name: &str) -> Result<Home, Never> {
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


fn all(kept: &Kept) -> Result<&Everything, Never> {
    Ok(kept.all.get_or_init(|| {
        let Ok(everything) = everything();

        everything
    }))
}

fn app_row(all: &Everything, name: &str, going: For, on: &Home) -> Result<Row, Never> {
    let picture =
        all.icon.get(name).map_or(Picture::Space, |at| Picture::At(PathBuf::from(at)));
    let named = name.to_string();

    match going {
        For::Placing(spot) => {
            let Ok(places) = Does::call(move |_| {
                let Ok(()) = placed(spot, &named);
                true
            });
            let Ok(row) = Row::new(name, "", places);

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

            let Ok(starts) = Does::call(move |_| {
                let Ok(()) = start(app.as_ref(), &named);
                true
            });
            let Ok(row) = Row::new(name, aside, starts);
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
    let Ok(looks) = Does::call(move |_| {
        let Ok(()) = looked_up(&word);
        true
    });
    let Ok(row) = Row::new(&format!("Look up {said:?}"), "", looks);

    row.picturing(Picture::Space)
}

fn rows(typed: &Typed, all: &Everything, going: For) -> Result<Vec<Row>, Never> {
    let mut word = String::new();

    match typed.ask(Msg::Said) {
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

fn before(typed: &Typed, kept: &Kept, going: For) -> Result<Vec<Row>, Never> {
    let Ok(listed) = kept_list(kept);

    rows(typed, listed, going)
}

fn kept_list(kept: &Kept) -> Result<&Everything, Never> {
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

fn pages(typed: &Typed, kept: &Kept, going: For) -> Result<Vec<Page>, Never> {
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
        let narrowed = typing.ask(|answer| Msg::Type { word: word.to_string(), answer });

        match matches!(narrowed, Ok(Narrowed::Changed)) {
            true => showing.replace(0),
            false => {},
        }
    });

    Ok(vec![page])
}


fn asked_for(asked: &[String]) -> Result<For, Never> {
    let Some(at) = asked.iter().position(|word| word == "--place") else {
        return Ok(For::Opening);
    };

    let said = asked.get(at.saturating_add(1)).map(String::as_str).unwrap_or_default();

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
        Some(app) => found::run(app)?,
        None => {
            let Ok(()) = looked_up(chosen);
        },
    }

    Ok(())
}

fn looked_up(said: &str) -> Result<(), Never> {
    let chosen = engines::chosen()?;
    let known = engines::one(&chosen)?;

    let Some(engine) = known else { return Ok(()) };

    let asked = engines::address(said, engine)?;

    let Some(address) = asked else { return Ok(()) };

    eprintln!("the menu was asked {said:?}: {address}");
    let Ok(opening) = opening(&address);
    let Ok(()) = console_panel::running::left_running(&opening);

    Ok(())
}

fn opening(address: &str) -> Result<Vec<String>, Never> {
    Program::XdgOpen.argv(&[address])
}


#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(answer) = answer;

        answer
    }

    const GRID: Shape = Shape::USUAL;

    fn taken() -> Home {
        let mut home = Home::default();
        ok(home.place(ok(Spot::new(0, 0, 0)), "Files"));
        ok(home.place(ok(Spot::new(0, 0, 1)), "Music"));
        home
    }

    #[test]
    fn y_is_a_switch() {
        let Ok(on) = turning(taken(), GRID, "Download");
        assert_eq!(ok(on.where_("Download")), Some(ok(Spot::new(0, 0, 2))), "the first free square");
        assert_eq!(ok(on.at(ok(Spot::new(0, 0, 0)))), Some("Files"), "and nothing else moved");

        let Ok(off) = turning(on, GRID, "Download");
        assert_eq!(ok(off.where_("Download")), None, "pressed again, it is off");
        assert_eq!(off, taken(), "and the home screen is where it was");
    }

    #[test]
    fn a_square_that_was_asked_for_is_the_square_it_lands_on() {
        let asked = ok(Spot::new(2, 1, 3));
        let Ok(placed) = placing(taken(), asked, "Download");

        assert_eq!(ok(placed.where_("Download")), Some(asked));
        assert_ne!(asked, ok(taken().first_free(GRID)), "the point of asking");
    }

    #[test]
    fn one_that_is_already_on_it_is_moved() {
        let asked = ok(Spot::new(1, 0, 0));
        let Ok(placed) = placing(taken(), asked, "Files");

        assert_eq!(ok(placed.where_("Files")), Some(asked));
        assert_eq!(ok(placed.at(ok(Spot::new(0, 0, 0)))), None, "and it left the square it was on");
    }

    #[test]
    fn what_the_words_after_the_program_name_ask_for() {
        let asked = |words: &[&str]| {
            asked_for(&words.iter().map(|word| (*word).to_string()).collect::<Vec<String>>())
        };

        assert_eq!(asked(&["--place", "2.1.3"]), Ok(For::Placing(ok(Spot::new(2, 1, 3)))));
        assert_eq!(asked(&[]), Ok(For::Opening), "the menu, opened the way it usually is");
        assert_eq!(asked(&["--keep"]), Ok(For::Opening), "the paddles, which only open it");
        assert_eq!(asked(&["--place"]), Ok(For::Opening), "a square that was not said");
        assert_eq!(asked(&["--place", "sideways"]), Ok(For::Opening), "a square that is not one");
    }
}
