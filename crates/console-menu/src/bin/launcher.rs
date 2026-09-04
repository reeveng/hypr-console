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

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use console_menu::{counts, entry, found, narrow};
use console_defaults::engines;
use console_panel::actor::{self, Addr, Answer};
use console_panel::page::{Does, Page, Picture, Row, Rows};
use console_panel::{chooser, panel};














/// Everything the menu knows: the applications, their pictures, and the order
/// they are worth showing in.
///
/// There are two of these and they are the same shape. `all` is the machine as
/// it is, read once on the thread the panel reads its rows on; `kept_list` is
/// the machine as it was last time, read off one file on the loop that draws.
/// The rows are built from whichever is to hand, by one function, so the second
/// is a first drawing of the first and never a different list.
struct Everything {
    apps: BTreeMap<String, entry::Application>,
    icon: BTreeMap<String, String>,
    /// The names in the order they are used in.
    order: Vec<String>,
}

/// What has been typed so far, and the only thing that owns it.
///
/// The one thing this menu holds between one drawing and the next. It is a
/// machine rather than a value behind a lock because the thumb writes it on
/// the main thread and the list is read on another, and a word cannot be half
/// written when the reader arrives.
struct Word {
    said: String,
}

/// Whether typing changed what was there.
///
/// Named rather than a bare `bool`, so the signature says which way round it
/// reads without a comment under it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Narrowed {
    Same,
    Changed,
}

/// Everything that can happen to it, and nothing else.
enum Msg {
    /// What has been typed.
    Said(Answer<String>),
    /// Type this, and say whether it changed anything.
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

/// Where the menu reaches it.
type Typed = Addr<Msg>;

/// What the empty line says it is for.
const ABOUT: &str = "Type to narrow the list";

/// Read the machine: every application it has, and a picture for each.
///
/// Written down as it is found, so the next menu can open on it. What it costs
/// is one file, and only when the answer has changed since the last one.
fn everything() -> Everything {
    counting(found::machine())
}

/// What the menu found last time it was opened.
///
/// The list a menu opens on, so that the applications are on the screen in the
/// moment the card is, rather than a moment after it. What is on this machine
/// is very nearly what was on it, and the reading behind this replaces the
/// whole list either way -- so the ones that are wrong are wrong for as long as
/// it takes to read the desktop files, and until now that was how long the menu
/// was empty for.
///
/// The counts are not remembered, because they do not have to be: they are one
/// file already, read here as they are read there, so the order the rows come
/// out in is this time's order and not last time's.
fn everything_before() -> Everything {
    counting(found::remembered())
}

/// The applications and their pictures, in the order they are worth showing.
///
/// The one place the order is worked out, so a remembered list and a read one
/// come out the same way round.
fn counting(found: found::Found) -> Everything {
    let names: Vec<String> = found.apps.keys().cloned().collect();
    let order = counts::order(&names, &found::counted());
    Everything { apps: found.apps, icon: found.icon, order }
}


/// The machine, read once and then answered from.
///
/// A menu is up for as long as it takes to choose something, and reading the
/// desktop files again for every letter typed would be the whole of
/// `/usr/share/applications` read on every thumb press. Read where the panel
/// reads its rows, which is a thread of its own, so the card is on the screen
/// before this starts.
fn all() -> &'static Everything {
    static ALL: OnceLock<Everything> = OnceLock::new();
    ALL.get_or_init(everything)
}

/// One application, with its picture.
///
/// The panel keeps room at the front of every row whether or not there is a
/// picture to put in it, so an application the icon theme has nothing for
/// still has its name where the others have theirs.
fn app_row(all: &Everything, name: &str) -> Row {
    let picture =
        all.icon.get(name).map_or(Picture::Space, |at| Picture::At(PathBuf::from(at)));
    let app = all.apps.get(name).cloned();
    let named = name.to_string();
    Row::new(
        name,
        "",
        Does::call(move |_| {
            start(app.as_ref(), &named);
            true
        }),
    )
    .picturing(picture)
}

/// The row that asks the browser instead, under everything the machine has.
///
/// wofi handed back whatever was typed and the browser was asked it without
/// anybody being told that was about to happen. Said out loud it is a row like
/// any other: it can be read before it is taken, and stepped past.
///
/// It stood there only while the list had been narrowed to nothing, which made
/// the browser the answer to a word the machine did not know and no answer at
/// all to a word it half knew. "map", on a machine with a map editor installed,
/// is somebody who has to leave the menu and start again somewhere else. So the
/// row is the last one on the list rather than the only one: what the machine
/// answers with comes first, and under it, one press further down than the last
/// application, is the rest of the world.
fn looking_up_row(said: &str) -> Row {
    let word = said.to_string();
    Row::new(
        &format!("Look up {said:?}"),
        "",
        Does::call(move |_| {
            looked_up(&word);
            true
        }),
    )
    .picturing(Picture::Space)
}

/// The list as the typed word leaves it, with the offer to look that word up.
///
/// Nothing typed is not a question, so the menu opens on the applications and
/// nothing else: an empty line handed to an engine is that engine's front page,
/// which is not what anybody standing on the bottom row meant to ask for.
fn rows(typed: &Typed, all: &Everything) -> Vec<Row> {
    // Nothing typed, if the owner has gone: the menu is on its way out by
    // then, and the applications are what it opens on.
    let mut word = String::new();

    // The owner has gone, which the comment above is about: the menu is on its
    // way out and the applications are what it opens on. Not a fault, and not
    // worth a line in the journal on the way out of a program.
    if let Ok(said) = typed.ask(Msg::Said) {
        word = said;
    }

    let standing = narrow::matching(&all.order, &word);
    let mut rows: Vec<Row> = standing.iter().map(|name| app_row(all, name)).collect();
    let said = word.trim();

    if !said.is_empty() {
        rows.push(looking_up_row(said));
    }

    rows
}

/// The menu before the machine has been read.
///
/// Narrowed by whatever has been typed, like the list it stands in for. Nothing
/// has been typed at the moment this is first drawn, but the letters do not
/// wait for the desktop files either: somebody who opens the menu and types
/// straight into it should see the remembered list narrow rather than see the
/// whole of it until the reading lands.
fn before(typed: &Typed) -> Vec<Row> {
    rows(typed, kept_list())
}

/// What was written down last time, read once.
///
/// Read on the main loop, where the drawing happens, so it is one file and
/// nothing else: no directory walked, no desktop file opened, nothing asked of
/// another program. A card that had to work something out before it could be
/// drawn would be the wait this is here to end, moved rather than removed.
fn kept_list() -> &'static Everything {
    static KEPT: OnceLock<Everything> = OnceLock::new();
    KEPT.get_or_init(everything_before)
}

/// The one tab, and the line that narrows it.
fn pages(typed: &Typed) -> Vec<Page> {
    let listing = typed.clone();
    let waiting = typed.clone();
    let typing = typed.clone();
    vec![
        Page::new("Menu", Rows::asked(move || rows(&listing, all())))
            .meanwhile(move || before(&waiting))
            .searching(ABOUT, move |showing, word| {
                let narrowed = typing.ask(|answer| Msg::Type { word: word.to_string(), answer });

                // Back to the top, because the row that was being stood on is
                // not the row standing there now.
                if matches!(narrowed, Ok(Narrowed::Changed)) {
                    showing.replace(0);
                }
            }),
    ]
}

fn main() {
    // The menu is on a button, on a paddle, on a key and on the bar. Pressed
    // again while it is already up, each of those used to draw a second menu
    // over the first, in the same place, and backing out of one left the other
    // looking like a menu that ignores you. Now the second press closes it.
    let asked: Vec<String> = std::env::args().skip(1).collect();
    // The daemon says --keep because the paddles it reads only open. The bar
    // does not, because a finger has no other way to put the menu away.
    let again = match asked.iter().any(|word| word == "--keep") {
        true => chooser::Again::Keeps,
        false => chooser::Again::Closes,
    };

    if chooser::alone("menu", again) == chooser::Alone::No {
        return;
    }

    let word = actor::supervise(|| Word { said: String::new() });
    let typed = word.addr.clone();
    // The machine is not read here. It is read where the panel reads its rows,
    // which is a thread of its own, so the card is up and answering the buttons
    // while the desktop files are being opened -- and until they are, it is the
    // list this menu found last time.
    panel::show(Arc::new(move || pages(&typed)), 0, None);
    // The menu is down and nothing is going to ask again. Waited for rather
    // than dropped, so a message already in the mailbox is finished with.
    word.shutdown();
}

/// Run what was chosen, and say what that was.
///
/// A press that chose something and a press that chose nothing look the same
/// from the outside, which is how "it only works sometimes" gets reported
/// about a button that worked every time and a program that never started.
fn start(app: Option<&entry::Application>, chosen: &str) {
    match app {
        Some(app) => found::run(app),
        None => looked_up(chosen),
    }
}

/// A line that matched no application, handed to the browser.
///
/// Somebody who wanted something this machine does not have, or has under a
/// name they did not type. A menu that closed and did nothing was the old
/// answer to the first of those and had nothing at all to say to the second.
///
/// Which engine is asked, and which browser opens it, are both the settings
/// panel's to say. Neither is named here.
fn looked_up(said: &str) {
    let Some(engine) = engines::one(&engines::chosen()) else { return };

    let Some(address) = engines::address(said, engine) else { return };

    eprintln!("the menu was asked {said:?}: {address}");
    console_panel::running::left_running(&opening(&address));
}

/// What opens an address, whoever this desktop says its browser is.
fn opening(address: &str) -> Vec<String> {
    vec!["xdg-open".to_string(), address.to_string()]
}

