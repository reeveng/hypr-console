//! Music and video off the net, drawn.
//!
//! ```text
//!     download-panel
//!     download-panel Video
//! ```
//!
//! Two tabs over one search. What is typed is the same question either way, and
//! the tab decides what is asked for out of the answer: the Audio tab puts the
//! sound of a thing into the folder the music player reads, the Video tab puts
//! the whole of it into Videos. That is also why the two lists are drawn
//! differently -- a song is chosen by whose it is and a video by whether it is
//! the one everybody means -- and why they are two tabs of one panel rather
//! than two programs.
//!
//! Nothing slow happens here. Looking is `download-find` and fetching is
//! `download-get`, both handed to `later`, and this draws whatever the first of
//! them wrote down.
//!
//! What is here is the machine: the folders, the pictures on the disk, and
//! whether a thing has already arrived. Where each tab is standing and what a
//! press does about it is `crate::standing`, which is a
//! `console_program_contract::Program`. The actor is what makes that state
//! reachable from a closure on GTK's thread; it steps by asking `heard`, and
//! the doings are carried out in the callback holding the surface.

use std::path::Path;
use std::sync::Arc;

use gtk4::glib;
use crate::getting;
use crate::looking::{self, Found, Looked};
use crate::rows::{self, ABOUT};
use crate::standing::{
    Closes, Downloads, Heard, Its, Onto, Standing, Tab, closes,
};
use crate::store::{self, Kind};
use console_core_never::Never;
use console_panel::actor::{self, Addr, Answer};
use console_panel::page::{Does, Page, Picture, Row, Rows, Showing};
use console_panel::card::{Card, Door};
use console_program_contract::{Argv, Doing, Named, Program, Turn, Word};

enum Msg {
    Heard(Heard, Answer<Vec<Doing<Its>>>),
    At { tab: usize, answer: Answer<Tab> },
}

struct Held(Standing);

impl actor::Machine for Held {
    type Msg = Msg;

    fn step(self, message: Msg) -> Self {
        match message {
            Msg::Heard(heard, answer) => {
                let Turn { now, doings } = Downloads::heard(&self.0, &Word::Its(heard));
                let _ = answer.say(doings);

                Held(now)
            }
            Msg::At { tab, answer } => {
                let Ok(at) = self.0.at(tab);
                let _ = answer.say(at);

                self
            },
        }
    }
}

type Panel = Addr<Msg>;

fn at(held: &Panel, tab: usize) -> Result<Tab, Never> {
    Ok(match held.ask(|answer| Msg::At { tab, answer }) {
        Ok(tab) => tab,
        Err(_) => {
            eprintln!("download-panel: the panel's own state has gone, so it drew as it opened");

            Tab::default()
        }
    })
}

fn decided(held: &Panel, heard: Heard) -> Result<Vec<Doing<Its>>, Never> {
    Ok(match held.ask(|answer| Msg::Heard(heard, answer)) {
        Ok(doings) => doings,
        Err(_) => {
            eprintln!("download-panel: the panel's own state has gone, so the press did nothing");

            Vec::new()
        }
    })
}

fn press(held: &Panel, heard: Heard, showing: &dyn Showing) -> Result<(), Never> {
    let Ok(doings) = decided(held, heard);

    for doing in &doings {
        let Ok(()) = carry(doing, showing);
    }

    Ok(())
}

fn carry(doing: &Doing<Its>, showing: &dyn Showing) -> Result<(), Never> {
    match doing {
        Doing::Its(Its::Replace(row)) => showing.replace(*row),
        Doing::Its(Its::Refresh) => showing.refresh(),
        Doing::Its(Its::Note(said)) => showing.note(said),
        Doing::Its(Its::ForgetTyping) => showing.forget_typing(),

        Doing::Ask(runs) => match runs.program {
            Named::Ours(name) => {
                let mut whole = vec![name.to_string()];

                whole.extend(runs.argv.clone());
                showing.later(whole);
            }
            Named::Theirs(_) => {},
        },

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

fn looked(kind: Kind) -> Result<Looked, Never> {
    let Ok(at) = store::found_at(&glib::user_cache_dir(), kind);

    let said = match std::fs::read_to_string(at) {
        Ok(said) => said,
        Err(_fault) => return Ok(Looked::default()),
    };

    looking::kept(&said)
}

fn folder(kind: Kind) -> Result<String, Never> {
    let Ok(into) = getting::into(kind);

    Ok(into.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| into.display().to_string()))
}

fn rows_of(held: &Panel, tab: usize, kind: Kind) -> Result<Vec<Row>, Never> {
    let Ok(looked) = looked(kind);
    let _ = decided(held, Heard::Landed { tab, asked: looked.asked.clone() });
    let Ok(Tab { typed, asking, onto }) = at(held, tab);

    match onto {
        Onto::Ways { found, .. } => ways(held, tab, kind, &found),
        Onto::List => {
            let cache = glib::user_cache_dir();
            let Ok(into) = getting::into(kind);
            let Ok(looking_for) = looking_for(held, tab, kind);

            rows::rows(&typed, asking.as_deref(), &looked, looking_for, &|at, found| {
                let Ok(thing) = thing(held, tab, kind, at, found, &cache, &into);

                thing
            })
        }
    }
}

fn thing(
    held: &Panel,
    tab: usize,
    kind: Kind,
    at: usize,
    found: &Found,
    cache: &Path,
    into: &Path,
) -> Result<Row, Never> {
    let Ok(have) = getting::holds(into, &found.id);
    let Ok(aside) = looking::aside(kind, found, have);
    let Ok(chose) = chose(held, tab, kind, found);
    let Ok(picture) = picture(cache, found);
    let Ok(offers) = offers(held, tab, found, at);

    let Ok(row) = Row::new(&found.title, &aside, chose);
    let Ok(pictured) = row.picturing(picture);

    pictured.offering(offers)
}

fn chose(held: &Panel, tab: usize, kind: Kind, found: &Found) -> Result<Does, Never> {
    let held = held.clone();
    let found = found.clone();

    Does::and_stay(move |showing| {
        let Ok(into) = getting::into(kind);
        let Ok(have) = getting::holds(&into, &found.id);
        let Ok(folder) = folder(kind);

        let Ok(()) = press(&held, Heard::Chose {
            tab,
            kind,
            found: found.clone(),
            have,
            into: folder,
        }, showing);
    })
}

fn offers(
    held: &Panel,
    tab: usize,
    found: &Found,
    from: usize,
) -> Result<impl Fn(&dyn Showing) -> bool + Send + Sync + 'static, Never> {
    let held = held.clone();
    let found = found.clone();

    Ok(move |showing: &dyn Showing| {
        let Ok(()) = press(&held, Heard::Offered { tab, found: found.clone(), from }, showing);

        false
    })
}

fn picture(cache: &Path, found: &Found) -> Result<Picture, Never> {
    let Ok(at) = store::picture_of(cache, &found.id);

    Ok(match at {
        Some(at) if at.exists() => Picture::At(at),
        Some(_) | None => Picture::Space,
    })
}

fn ways(held: &Panel, tab: usize, kind: Kind, found: &Found) -> Result<Vec<Row>, Never> {
    let Ok(other) = kind.other();
    let backing = held.clone();
    let Ok(chose) = chose(held, tab, other, found);

    rows::ways(
        found,
        other,
        move |showing| {
            let Ok(()) = press(&backing, Heard::Back { tab }, showing);
        },
        chose,
    )
}

fn looking_for(held: &Panel, tab: usize, kind: Kind) -> Result<Does, Never> {
    let held = held.clone();

    Does::and_stay(move |showing| {
        let Ok(()) = press(&held, Heard::LookFor { tab, kind }, showing);
    })
}

fn pages(held: &Panel) -> Result<Vec<Page>, Never> {
    Ok(Kind::BOTH
        .iter()
        .enumerate()
        .map(|(tab, kind)| {
            let Ok(page) = page(held, tab, *kind);

            page
        })
        .collect())
}

fn page(held: &Panel, tab: usize, kind: Kind) -> Result<Page, Never> {
    let reading = held.clone();
    let backing = held.clone();
    let Ok(word) = kind.tab();
    let Ok(asked) = Rows::asked(move || {
        let Ok(rows) = rows_of(&reading, tab, kind);

        rows
    });
    let Ok(page) = Page::new(word, asked);
    let Ok(page) = page.on_back(move |showing| {
        let Ok(at) = at(&backing, tab);
        let was = Standing { tabs: vec![at] };

        let Ok(()) = press(&backing, Heard::Back { tab }, showing);

        let Ok(closes) = closes(&was, 0);

        match closes {
            Closes::Yes => true,
            Closes::No => false,
        }
    });
    let Ok(held_at) = at(held, tab);

    match held_at.onto {
        Onto::List => {},
        Onto::Ways { .. } => return Ok(page),
    }

    let typing = held.clone();

    page.searching(ABOUT, move |showing, word| {
        let Ok(()) = press(&typing, Heard::Typed { tab, word: word.to_string() }, showing);
    })
}


pub const WHO: &str = "download-panel";

const DOOR: &str = "download";

pub fn door(_argv: &[String]) -> Result<Door, Never> {
    Door::closing(DOOR)
}

pub fn card(argv: &[String]) -> Result<Card, Never> {
    let tab = argv.first().cloned();
    let opening = Downloads::opening(&Argv::default());
    let Ok(standing) = actor::supervise(move || Held(opening.state.clone()));
    let held = standing.addr.clone();

    let Ok(card) = Card::new(Arc::new(move || {
        let Ok(pages) = pages(&held);

        pages
    }));
    let Ok(card) = card.opening_at(tab.as_deref());

    card.shutting(Box::new(move || standing.shutdown()))
}
