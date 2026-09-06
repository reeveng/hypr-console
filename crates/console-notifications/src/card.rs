//! What the desktop has said, drawn.
//!
//! ```text
//!     notices-panel
//!     notices-panel Earlier
//! ```
//!
//! The bell on the right of the bar opens it, and tapping the bell again puts
//! it away, which is how every other icon along there works.
//!
//! What is here is the asking of mako and the drawing. What each tab holds
//! once it has been asked is `crate::rows`; where a press
//! leaves you is `crate::notices`, which is a
//! `console_program_contract::Program` and holds the whole of what this panel
//! decides. The actor is what makes that state reachable from a closure on
//! GTK's thread; it steps by asking `heard`, and the doings are carried out in
//! the callback that has the surface in its hand.

use std::sync::Arc;

use console_external_programs::Program;
use crate::notices::{Closes, Heard, Its, Notices, Onto, closes};
use crate::reading::{self, Notice};
use crate::rows::{Chosen, earlier_rows, gone_rows, one_rows, tab, waiting_rows};
use console_panel::actor::{self, Addr, Answer};
use console_panel::card::{Card, Door};
use console_panel::page::{Does, Page, Row, Rows, Showing, Watch};
use console_panel::running::said;
use console_never::Never;
use console_program_contract::{Argv, Doing, Named, Program as _, Turn, Word};

fn makoctl(argv: &[&str]) -> Result<String, Never> {
    said(Program::Makoctl, argv)
}

fn waiting() -> Result<Vec<Notice>, Never> {
    let Ok(listed) = makoctl(&["list", "-j"]);

    reading::read(&listed)
}

fn earlier() -> Result<Vec<Notice>, Never> {
    let Ok(listed) = makoctl(&["history", "-j"]);

    reading::read(&listed)
}

struct Looking {
    onto: Onto,
}

enum Msg {
    Heard(Heard, Answer<Vec<Doing<Its>>>),
    At(Answer<Onto>),
}

impl actor::Machine for Looking {
    type Msg = Msg;

    fn step(self, message: Msg) -> Self {
        match message {
            Msg::Heard(heard, answer) => {
                let Turn { now, doings } = Notices::heard(&self.onto, &Word::Its(heard));
                let _ = answer.say(doings);

                Looking { onto: now }
            }
            Msg::At(answer) => {
                let _ = answer.say(self.onto);

                self
            },
        }
    }
}

type Held = Addr<Msg>;

fn looking_at(held: &Held) -> Result<Onto, Never> {
    Ok(match held.ask(Msg::At) {
        Ok(onto) => onto,
        Err(_) => Onto::List,
    })
}

fn press(held: &Held, heard: Heard, showing: &dyn Showing) -> Result<(), Never> {
    let doings = match held.ask(|answer| Msg::Heard(heard, answer)) {
        Ok(doings) => doings,
        Err(_) => {
            eprintln!("notices-panel: the panel's own state has gone, so the press did nothing");

            Vec::new()
        }
    };

    for doing in doings {
        let Ok(()) = carry(&doing, showing);
    }

    Ok(())
}

fn carry(doing: &Doing<Its>, showing: &dyn Showing) -> Result<(), Never> {
    match doing {
        Doing::Ask(runs) => {
            let argv: Vec<&str> = runs.argv.iter().map(String::as_str).collect();

            match runs.program {
                Named::Theirs(program) => {
                    let _ = said(program, &argv);
                }
                Named::Ours(name) => {
                    let mut whole = vec![name.to_string()];

                    whole.extend(runs.argv.clone());
                    showing.later(whole);
                }
            }
        }
        Doing::Its(Its::Replace(row)) => showing.replace(*row),
        Doing::Its(Its::Refresh) => showing.refresh(),
        Doing::Watch(_)
        | Doing::AskWhoever(_)
        | Doing::Start(_)
        | Doing::Listen(_)
        | Doing::Deafen(_)
        | Doing::Write(_)
        | Doing::Say(_)
        | Doing::Print(_)
        | Doing::Stop(_) => {},
    }

    Ok(())
}

fn back_up(held: &Held) -> Result<Chosen, Never> {
    let held = held.clone();

    Ok(Arc::new(move |showing: &dyn Showing| {
        let Ok(()) = press(&held, Heard::Back, showing);
    }))
}

fn open(held: &Held, id: u32) -> Result<Does, Never> {
    let held = held.clone();

    Does::and_stay(move |showing| {
        let Ok(()) = press(&held, Heard::Chose(id), showing);
    })
}

fn dismiss(held: &Held, id: u32) -> Result<Does, Never> {
    let held = held.clone();

    Does::and_stay(move |showing| {
        let Ok(()) = press(&held, Heard::Dismissed(id), showing);
    })
}

fn clear(held: &Held) -> Result<Does, Never> {
    let held = held.clone();

    Does::and_stay(move |showing| {
        let Ok(()) = press(&held, Heard::ClearAll, showing);
    })
}

fn waiting_tab(looking: &Held) -> Result<Vec<Row>, Never> {
    let Ok(held) = waiting();
    let Ok(onto) = looking_at(looking);

    match onto {
        Onto::List => {
            let Ok(clears) = clear(looking);

            waiting_rows(
                &held,
                |notice| {
                    let Ok(does) = open(looking, notice.id);

                    does
                },
                clears,
            )
        }
        Onto::One(id) => match held.iter().find(|notice| notice.id == id) {
            Some(notice) => {
                let Ok(back) = back_up(looking);
                let Ok(dismisses) = dismiss(looking, id);

                one_rows(notice, &back, dismisses)
            }
            None => {
                let Ok(back) = back_up(looking);

                gone_rows(&back)
            }
        },
    }
}

fn earlier_tab() -> Result<Vec<Row>, Never> {
    let Ok(held) = earlier();

    earlier_rows(&held)
}

fn arriving() -> Result<Watch, Never> {
    let Ok(stdbuf) = Program::Stdbuf.name();
    let Ok(busctl) = Program::Busctl.name();

    Watch::on(
        &[
            stdbuf,
            "-oL",
            busctl,
            "--user",
            "monitor",
            "org.freedesktop.Notifications",
        ],
        "Member=Notif",
    )
}

fn pages(looking: &Held) -> Result<Vec<Page>, Never> {
    let drawing = looking.clone();
    let backing = looking.clone();
    let Ok(asked) = Rows::asked(move || {
        let Ok(rows) = waiting_tab(&drawing);

        rows
    });
    let Ok(first) = tab(0);
    let Ok(waiting) = Page::new(first, asked);
    let Ok(watch) = arriving();
    let Ok(watching) = waiting.watching(watch);
    let Ok(waiting) = watching.on_back(move |showing| {
        let Ok(onto) = looking_at(&backing);
        let Ok(()) = press(&backing, Heard::Back, showing);

        let Ok(closes) = closes(&onto);

        match closes {
            Closes::Yes => true,
            Closes::No => false,
        }
    });
    let Ok(read) = Rows::asked(|| {
        let Ok(rows) = earlier_tab();

        rows
    });
    let Ok(second) = tab(1);
    let Ok(cleared) = Page::new(second, read);

    Ok(vec![waiting, cleared])
}


pub const WHO: &str = "notices-panel";

const UNDER: i32 = 250;

pub fn door(argv: &[String]) -> Result<Door, Never> {
    let tab = argv.first().map(String::as_str).unwrap_or_default();

    Door::closing(&format!("notices {tab}"))
}

pub fn card(argv: &[String]) -> Result<Card, Never> {
    let tab = argv.first().cloned();
    let Ok(opened) = Argv::of(&tab.as_deref().into_iter().collect::<Vec<&str>>());

    let opening = Notices::opening(&opened);
    let Ok(looking) = actor::supervise(move || Looking { onto: opening.state });
    let held = looking.addr.clone();

    let Ok(card) = Card::new(Arc::new(move || {
        let Ok(pages) = pages(&held);

        pages
    }));
    let Ok(card) = card.under(UNDER);
    let Ok(card) = card.opening_at(tab.as_deref());

    card.shutting(Box::new(move || looking.shutdown()))
}
