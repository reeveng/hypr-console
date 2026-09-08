//! The buttons, and where they are on this device.
//!
//! ```text
//!     layout-panel            open it
//!     layout-panel --first    open it because nobody has answered yet
//! ```
//!
//! One row per thing the desktop does, what plays it now beside it, and A on a
//! row asks for the button by putting a card up and waiting for a press. The
//! card is a program of its own, because what tells the controller daemon to
//! make the front of the machine inert while the question is on screen is the
//! card's own layer being there.
//!
//! A tab per input, and it opens on the one last pressed. Both are always
//! here -- a keyboard's page is drawn on a machine with no keyboard attached,
//! because a job with nothing on it there is exactly what somebody about to
//! plug one in wants to see -- so the last press decides only which is in
//! front. It is read from the file the daemon writes rather than heard live,
//! and `console_input_bindings::active` is the argument: a page that moved
//! under somebody halfway through moving a job would be answering a question
//! nobody asked.
//!
//! What is here is the machine: where the table is, what this device can send,
//! and a surface. `crate::rows` is the screen and
//! `crate::pressing` is what a press decides, and neither has
//! ever seen one.

use std::sync::Arc;

use crate::pressing::{FIRST, Heard, Its, Setting, Setup, TABLE, WRITTEN};
use crate::rows::{PUT_BACK_SURE, PUT_BACK_YES, Part, parts, rows};
use crate::table;
use console_core_never::Never;
use console_panel::page::{Does, Page, Row, Rows, Showing};
use console_panel::card::{Card, Door};
use console_input_bindings::bound::{EVERY, Input};
use console_program_contract::{Argv, Doing, Named, Program, Turn, Word, Writing};

const DOOR: &str = "buttons";

fn press(setting: &Setting, heard: Heard, showing: &dyn Showing) -> Result<(), Never> {
    let Turn { doings, .. } = Setup::heard(setting, &Word::Its(heard));

    for doing in &doings {
        let Ok(()) = carry(doing, showing);
    }

    Ok(())
}

fn carry(doing: &Doing<Its>, showing: &dyn Showing) -> Result<(), Never> {
    match doing {
        Doing::Its(Its::Note(said)) => showing.note(said),

        Doing::Its(Its::Sure) => {
            let Ok(putting) = putting_back();

            showing.sure(PUT_BACK_SURE, "", &[PUT_BACK_YES], Arc::new(move |showing, _| {
                match &putting {
                    Some(putting) => {
                        let Ok(()) = press(putting, Heard::Sure, showing);
                    },
                    None => {},
                }
            }));
        }

        Doing::Ask(runs) => match runs.program {
            Named::Ours(name) => {
                let mut whole = vec![name.to_string()];

                whole.extend(runs.argv.clone());
                showing.later(whole);
            }
            Named::Theirs(_) => {},
        },

        Doing::Write(writing) => match wrote(writing) {
            Ok(()) => {},
            Err(fault) => showing.note(&fault),
        },

        Doing::Watch(_)
        | Doing::AskWhoever(_)
        | Doing::Start(_)
        | Doing::Listen(_)
        | Doing::Deafen(_)
        | Doing::Say(_)
        | Doing::Print(_)
        | Doing::Stop(_) => {},
    }

    Ok(())
}

fn writing(doing: &Doing<Its>) -> Result<Option<&Writing>, Never> {
    Ok(match doing {
        Doing::Write(writing) => Some(writing),

        Doing::Its(_)
        | Doing::Ask(_)
        | Doing::Watch(_)
        | Doing::AskWhoever(_)
        | Doing::Start(_)
        | Doing::Listen(_)
        | Doing::Deafen(_)
        | Doing::Say(_)
        | Doing::Print(_)
        | Doing::Stop(_) => None,
    })
}

fn wrote(writing: &Writing) -> Result<(), String> {
    match writing.at.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{}: {fault}", holding.display()))?,
        None => {},
    }

    std::fs::write(&writing.at, &writing.what)
        .map_err(|fault| format!("{}: {fault}", writing.at.display()))
}

fn putting_back() -> Result<Option<Setting>, Never> {
    let Ok(at) = table::at();

    Ok(at.map(|at| Setting::Set { at }))
}

fn asks_for(part: &Part) -> Result<Does, Never> {
    let part = part.clone();

    Does::and_stay(move |showing| {
        let Ok(putting) = putting_back();

        match putting {
            Some(putting) => {
                let Ok(()) = press(&putting, Heard::Asked(part.clone()), showing);
            },
            None => {},
        }
    })
}

fn puts_it_all_back() -> Result<Does, Never> {
    Does::and_stay(|showing| {
        let Ok(putting) = putting_back();

        match putting {
            Some(putting) => {
                let Ok(()) = press(&putting, Heard::PutBack, showing);
            },
            None => {},
        }
    })
}

fn one_input(on: Input) -> Result<Vec<Row>, Never> {
    let Ok(table) = table::table();
    let Ok(front) = table::front();
    let Ok(parts) = parts(&table, &front, on);
    let Ok(back) = puts_it_all_back();

    rows(
        &parts,
        |part| {
            let Ok(asks) = asks_for(part);

            asks
        },
        back,
    )
}

fn pages() -> Result<Vec<Page>, Never> {
    let mut pages = Vec::new();

    for on in EVERY {
        let Ok(asked) = Rows::asked(move || {
            let Ok(rows) = one_input(on);

            rows
        });
        let Ok(says) = on.says();
        let Ok(page) = Page::new(says, asked);

        pages.push(page);
    }

    Ok(pages)
}

fn opening() -> Result<Argv, Never> {
    let Ok(at) = table::at();

    let mut words: Vec<String> = match &at {
        Some(at) => vec![TABLE.to_string(), at.display().to_string()],
        None => Vec::new(),
    };

    match std::env::args().any(|word| word == FIRST) {
        true => words.push(FIRST.to_string()),
        false => {},
    }

    match at.is_some_and(|at| at.exists()) {
        true => words.push(WRITTEN.to_string()),
        false => {},
    }

    Argv::of(&words.iter().map(String::as_str).collect::<Vec<&str>>())
}


pub const WHO: &str = "layout-panel";

pub fn door(_argv: &[String]) -> Result<Door, Never> {
    Door::closing(DOOR)
}

pub fn card(_argv: &[String]) -> Result<Card, Never> {
    let Ok(argv) = opening();

    let opened = Setup::opening(&argv);
    let Turn { doings, .. } = Setup::heard(&opened.state, &Word::Opened);

    let writings = doings.iter().filter_map(|doing| {
        let Ok(writing) = writing(doing);

        writing
    });

    for writing in writings {
        match wrote(writing) {
            Ok(()) => {},
            Err(fault) => eprintln!("layout-panel: {fault}"),
        }
    }

    let Ok(card) = Card::new(Arc::new(|| {
        let Ok(pages) = pages();

        pages
    }));
    let Ok(first) = opens_on();

    card.opening_at(Some(&first))
}

fn opens_on() -> Result<String, Never> {
    let Ok(home) = console_core_places::home();

    let Ok(on) = match home {
        Some(home) => console_input_bindings::active::read(&home),
        None => Ok(console_input_bindings::active::FIRST),
    };

    let Ok(says) = on.says();

    Ok(says.to_string())
}
