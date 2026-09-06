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
//! What is here is the machine: where the table is, what this device can send,
//! and a surface. `crate::rows` is the screen and
//! `crate::pressing` is what a press decides, and neither has
//! ever seen one.

use std::sync::Arc;

use crate::pressing::{FIRST, Heard, Its, Setting, Setup, TABLE, WRITTEN};
use crate::rows::{PUT_BACK_SURE, PUT_BACK_YES, Part, TABS, parts, rows};
use crate::table;
use console_never::Never;
use console_panel::page::{Does, Page, Row, Rows, Showing};
use console_panel::card::{Card, Door};
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
                let Ok(()) = press(&putting, Heard::Sure, showing);
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

fn putting_back() -> Result<Setting, Never> {
    let Ok(at) = table::at();

    Ok(Setting::Set { at })
}

fn asks_for(part: &Part) -> Result<Does, Never> {
    let part = part.clone();

    Does::and_stay(move |showing| {
        let Ok(putting) = putting_back();
        let Ok(()) = press(&putting, Heard::Asked(part.clone()), showing);
    })
}

fn puts_it_all_back() -> Result<Does, Never> {
    Does::and_stay(|showing| {
        let Ok(putting) = putting_back();
        let Ok(()) = press(&putting, Heard::PutBack, showing);
    })
}

fn buttons_tab() -> Result<Vec<Row>, Never> {
    let Ok(table) = table::table();
    let Ok(front) = table::front();
    let Ok(parts) = parts(&table, &front);
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
    let Ok(asked) = Rows::asked(|| {
        let Ok(rows) = buttons_tab();

        rows
    });

    let Ok(page) = Page::new(TABS.first().copied().unwrap_or(""), asked);

    Ok(vec![page])
}

fn opening() -> Result<Argv, Never> {
    let Ok(at) = table::at();
    let mut words: Vec<String> = vec![TABLE.to_string(), at.display().to_string()];

    match std::env::args().any(|word| word == FIRST) {
        true => words.push(FIRST.to_string()),
        false => {},
    }

    match at.exists() {
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

    Card::new(Arc::new(|| {
        let Ok(pages) = pages();

        pages
    }))
}
