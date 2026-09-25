//! Music, video and books off the net, drawn.
//!
//! ```text
//!     downloads
//!     downloads Video
//!     downloads Books
//! ```
//!
//! Two tabs over one search. What is typed is the same question either way, and
//! the tab decides what is asked for out of the answer: the Audio tab puts the
//! sound of a thing into the folder the music player reads, the Video tab puts
//! the whole of it into Videos. That is also why the two lists are drawn
//! differently -- a song is chosen by whose it is and a video by whether it is
//! the one everyone means -- and why they are two tabs of one app rather
//! than two programs.
//!
//! Nothing slow happens here. Looking is `downloads-find` and fetching is
//! `downloads-get`, both handed to `later`, and this draws whatever the first of
//! them wrote down.
//!
//! What is here is the machine: the folders, the pictures on the disk, and
//! whether a thing has already arrived. Where each tab is standing and what a
//! press does about it is `crate::standing`, which is a
//! `console_program_contract::Program`. The actor is what makes that state
//! reachable from a closure on GTK's thread; it steps by asking `update`, and
//! the effects are carried out in the callback holding the surface.

use std::path::Path;

use crate::getting;
use crate::looking::{self, Found, Looked};
use crate::rows;
use crate::standing::{
    Closes, Downloads, DownloadsEvent, DownloadsEffect, Destination, Standing, Tab, closes,
};
use crate::store::{self, Kind};
use console_core_never::Never;
use console_panel::actor::{self, Address, Answer};
use console_panel::page::{Aside, Handler, Page, Picture, Row, Rows, Showing};
use console_panel::card::Card;
use console_program_contract::{Arguments, Effect, Executable, Program, Update, Event};

enum Message {
    Event(DownloadsEvent, Answer<Vec<Effect<DownloadsEffect>>>),
    At { tab: u32, answer: Answer<Tab> },
}

struct Actor(Standing);

impl actor::Machine for Actor {
    type Message = Message;

    fn step(self, message: Message) -> Self {
        match message {
            Message::Event(heard, answer) => {
                let Update { state, effects } = Downloads::update(&self.0, &Event::Custom(heard));
                let _ = answer.say(effects);

                Actor(state)
            }
            Message::At { tab, answer } => {
                let Ok(at) = self.0.at(tab);
                let _ = answer.say(at);

                self
            },
        }
    }
}

type Panel = Address<Message>;

fn at(held: &Panel, tab: u32) -> Result<Tab, Never> {
    Ok(match held.ask(|answer| Message::At { tab, answer }) {
        Ok(tab) => tab,
        Err(_the_actor_has_gone) => {
            eprintln!("downloads: the panel's own state is missing, so it drew as it opened");

            Tab::default()
        }
    })
}

fn decided(held: &Panel, heard: DownloadsEvent) -> Result<Vec<Effect<DownloadsEffect>>, Never> {
    Ok(match held.ask(|answer| Message::Event(heard, answer)) {
        Ok(effects) => effects,
        Err(_the_actor_has_gone) => {
            eprintln!("downloads: the panel's own state is missing, so the press did nothing");

            Vec::new()
        }
    })
}

fn press(held: &Panel, heard: DownloadsEvent, showing: &dyn Showing) -> Result<(), Never> {
    let Ok(effects) = decided(held, heard);

    for effect in &effects {
        let Ok(()) = carry(effect, showing);
    }

    Ok(())
}

fn carry(effect: &Effect<DownloadsEffect>, showing: &dyn Showing) -> Result<(), Never> {
    match effect {
        Effect::Custom(DownloadsEffect::Replace(row)) => {
            let Ok(row) = console_core_number_conversion::fitted(*row);

            showing.replace(row)
        }
        Effect::Custom(DownloadsEffect::Refresh) => showing.refresh(),
        Effect::Custom(DownloadsEffect::Note(said)) => showing.note(said),
        Effect::Custom(DownloadsEffect::ForgetTyping) => showing.forget_typing(),

        Effect::Run(runs) => match runs.program {
            Executable::Internal(name) => {
                let mut whole = vec![name.to_string()];

                whole.extend(runs.arguments.clone());
                showing.later(whole);
            }
            Executable::External(_) => {},
        },

        Effect::Stream(_)
        | Effect::Prompt(_)
        | Effect::Spawn(_)
        | Effect::Subscribe(_)
        | Effect::Unsubscribe(_)
        | Effect::Write(_)
        | Effect::Notify(_)
        | Effect::Print(_)
        | Effect::Stop(_) => {},
    }

    Ok(())
}

fn looked(kind: Kind) -> Result<Looked, Never> {
    let Ok(cache) = store::cache();

    let cache = match cache {
        Some(cache) => cache,
        None => return Ok(Looked::default()),
    };

    let Ok(at) = store::found_at(&cache, kind);

    let said = match std::fs::read_to_string(at) {
        Ok(said) => said,
        Err(_unreadable) => return Ok(Looked::default()),
    };

    looking::kept(&said)
}

fn folder(kind: Kind) -> Result<String, Never> {
    let Ok(into) = getting::into(kind);

    Ok(match into.file_name() {
        Some(named) => named.to_string_lossy().to_string(),
        None => into.display().to_string(),
    })
}

fn rows_of(held: &Panel, tab: u32, kind: Kind) -> Result<Vec<Row>, Never> {
    let Ok(looked) = looked(kind);
    let _ = decided(held, DownloadsEvent::Landed { tab, asked: looked.asked.clone() });
    let Ok(Tab { typed, asking, onto }) = at(held, tab);

    match onto {
        Destination::Ways { found, .. } => ways(held, tab, kind, &found),
        Destination::List => {
            let Ok(cache) = store::cache();

            let cache = match cache {
                Some(cache) => cache,
                None => return Ok(Vec::new()),
            };

            let Ok(into) = getting::into(kind);
            let Ok(looking_for) = looking_for(held, tab, kind);

            let Ok(rows) = rows::rows(&typed, asking.as_deref(), &looked, looking_for, &|at, found| {
                let Ok(thing) = thing(held, tab, kind, Line(at), found, &cache, &into);

                thing
            });

            rows::noted(kind, rows)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Line(u32);

fn thing(
    held: &Panel,
    tab: u32,
    kind: Kind,
    at: Line,
    found: &Found,
    cache: &Path,
    into: &Path,
) -> Result<Row, Never> {
    let Ok(have) = getting::holds(into, &found.id);
    let Ok(aside) = looking::aside(kind, found, have);
    let Ok(chose) = chose(held, tab, kind, found);
    let Ok(picture) = picture(cache, found);
    let Ok(offers) = offers(held, tab, found, at);

    let Ok(row) = Row::new(&found.title, Aside(&aside), chose);
    let Ok(pictured) = row.picturing(picture);
    let Ok(pictured) = pictured.selectable(&found.id);

    pictured.offering(offers)
}

fn chose(held: &Panel, tab: u32, kind: Kind, found: &Found) -> Result<Handler, Never> {
    let held = held.clone();
    let found = found.clone();

    Handler::and_stay(move |showing| {
        let Ok(into) = getting::into(kind);
        let Ok(have) = getting::holds(&into, &found.id);
        let Ok(folder) = folder(kind);

        let Ok(()) = press(&held, DownloadsEvent::Chose {
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
    tab: u32,
    found: &Found,
    from: Line,
) -> Result<impl Fn(&dyn Showing) -> bool + Send + Sync + 'static, Never> {
    let held = held.clone();
    let found = found.clone();
    let from = from.0;

    Ok(move |showing: &dyn Showing| {
        let Ok(()) = press(&held, DownloadsEvent::Offered { tab, found: found.clone(), from }, showing);

        false
    })
}

fn picture(cache: &Path, found: &Found) -> Result<Picture, Never> {
    let Ok(at) = store::picture_of(cache, &found.id);

    Ok(match at {
        Some(at) => match at.exists() {
            true => Picture::At(at),
            false => Picture::Space,
        },
        None => Picture::Space,
    })
}

fn ways(held: &Panel, tab: u32, kind: Kind, found: &Found) -> Result<Vec<Row>, Never> {
    let Ok(other) = kind.other();
    let backing = held.clone();

    let other = match other {
        Some(other) => {
            let Ok(chose) = chose(held, tab, other, found);

            Some((other, chose))
        },
        None => None,
    };

    let selecting = held.clone();
    let id = found.id.clone();
    let Ok(ways) = rows::ways(
        found,
        move |showing| {
            let Ok(()) = press(&backing, DownloadsEvent::Back { tab }, showing);
        },
        other,
    );
    let Ok(selects) = Handler::and_stay(move |showing| {
        let Ok(()) = press(&selecting, DownloadsEvent::Back { tab }, showing);

        showing.select(vec![id.clone()]);
    });
    let Ok(select) = Row::new(console_panel::page::SELECT, Aside(""), selects);

    Ok([ways, vec![select]].concat())
}

fn fetched_together(held: &Panel, tab: u32, kind: Kind, showing: &dyn Showing, keys: &[String]) -> Result<(), Never> {
    let Ok(looked) = looked(kind);
    let wanted: std::collections::BTreeSet<&String> = keys.iter().collect();

    for found in looked.found.iter().filter(|found| wanted.contains(&found.id)) {
        let Ok(into) = getting::into(kind);
        let Ok(have) = getting::holds(&into, &found.id);
        let Ok(folder) = folder(kind);

        let Ok(()) = press(held, DownloadsEvent::Chose { tab, kind, found: found.clone(), have, into: folder }, showing);
    }

    Ok(())
}

fn looking_for(held: &Panel, tab: u32, kind: Kind) -> Result<Handler, Never> {
    let held = held.clone();

    Handler::and_stay(move |showing| {
        let Ok(()) = press(&held, DownloadsEvent::LookFor { tab, kind }, showing);
    })
}

fn pages(held: &Panel) -> Result<Vec<Page>, Never> {
    Ok(Kind::ALL
        .iter()
        .enumerate()
        .map(|(tab, kind)| {
            let Ok(tab) = console_core_number_conversion::fitted::<_, u32>(tab);
            let Ok(page) = page(held, tab, *kind);

            page
        })
        .collect())
}

const DOWNLOAD: &str = "Download";

fn page(held: &Panel, tab: u32, kind: Kind) -> Result<Page, Never> {
    let reading = held.clone();
    let backing = held.clone();
    let Ok(word) = kind.tab();
    let Ok(asked) = Rows::asked(move || {
        let Ok(rows) = rows_of(&reading, tab, kind);

        rows
    });
    let Ok(page) = Page::new(word, asked);
    let fetching = held.clone();
    let Ok(page) = page.selecting(DOWNLOAD, move |showing, keys| {
        let Ok(()) = fetched_together(&fetching, tab, kind, showing, keys);
    });
    let Ok(page) = page.on_back(move |showing| {
        let Ok(at) = at(&backing, tab);
        let was = Standing { tabs: vec![at] };

        let Ok(()) = press(&backing, DownloadsEvent::Back { tab }, showing);

        let Ok(closes) = closes(&was, 0);

        match closes {
            Closes::Yes => true,
            Closes::No => false,
        }
    });
    let Ok(held_at) = at(held, tab);

    match held_at.onto {
        Destination::List => {},
        Destination::Ways { .. } => return Ok(page),
    }

    let typing = held.clone();

    let Ok(about) = rows::about(kind);

    page.searching(about, move |showing, word| {
        let Ok(()) = press(&typing, DownloadsEvent::Typed { tab, word: word.to_string() }, showing);
    })
}


pub const WHO: &str = "downloads";

pub fn card(arguments: &[String]) -> Result<Card, Never> {
    let initial = Downloads::init(&Arguments::default());
    let Ok(card) = Card::supervised(move || Actor(initial.state.clone()), pages);

    card.opening_at(arguments.first().map(String::as_str))
}
