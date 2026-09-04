//! Music and video off the net, drawn.
//!
//!     download-panel
//!     download-panel Video
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
//! them wrote down. What is held between one drawing and the next is where each
//! tab is standing and the word a search is out for, which is why there is a
//! lock: the rows are read on a thread of the panel's own.

use std::path::Path;
use std::sync::Arc;

use gtk4::glib;
use console_download::getting;
use console_download::looking::{self, Found, Looked};
use console_download::rows::{self, ABOUT, WAYS_START};
use console_download::store::{self, Kind};
use console_panel::actor::{self, Addr, Answer};
use console_panel::page::{Does, Page, Picture, Row, Rows, Showing};
use console_panel::{chooser, panel};

/// What a tab is looking at.
///
/// Y's list is a place you are rather than something drawn over the list, which
/// is what makes B mean one thing here: out of what else can be done with a
/// thing, then out of the word that found it, then out of the panel.
#[derive(Clone)]
enum Onto {
    List,
    Ways { found: Found, from: usize },
}

/// Where each tab is standing, and what it has been told.
///
/// One of each per tab, side by side, so that turning the shoulders leaves a
/// tab as it was. Looking for a song, going to the Video tab to fetch something
/// and coming back is a thing a person does, and a panel that had forgotten the
/// search is one that has to be typed into again.
struct Standing {
    typed: Vec<String>,
    /// The word a search is out for, while one is.
    asking: Vec<Option<String>>,
    onto: Vec<Onto>,
}

impl Standing {
    fn new() -> Self {
        Standing {
            typed: Kind::BOTH.iter().map(|_| String::new()).collect(),
            asking: Kind::BOTH.iter().map(|_| None).collect(),
            onto: Kind::BOTH.iter().map(|_| Onto::List).collect(),
        }
    }
}

/// What one tab is standing in, as one answer.
///
/// Three fields that are always wanted together, so they are asked for
/// together: three questions would be three crossings to the owner and three
/// chances for the tab to move between them.
struct Reading {
    typed: String,
    asking: Option<String>,
    onto: Onto,
}

/// Whether typing into a tab changed what was there.
///
/// Named rather than a bare `bool`, so the signature says which way round it
/// reads without a comment under it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Word {
    Same,
    Changed,
}

/// Everything that can happen to the standing, and nothing else.
///
/// One variant per thing a thumb can do to it. Reading this enum is reading
/// the whole of what this panel remembers, which is what a dozen scattered
/// `standing(...)` blocks could not be read as.
enum Msg {
    /// Look at something else on a tab.
    Look { tab: usize, onto: Onto },
    /// A search is out for this word.
    Asking { tab: usize, word: String },
    /// Forget what was typed into a tab.
    Forget { tab: usize },
    /// What a tab is standing in, with a search that has landed cleared on the
    /// way past.
    Standing { tab: usize, landed: String, answer: Answer<Reading> },
    /// What a tab is looking at.
    Looking { tab: usize, answer: Answer<Onto> },
    /// What a tab is looking at, and what has been typed into it.
    Both { tab: usize, answer: Answer<(Onto, String)> },
    /// Type into a tab, and say whether that changed it.
    Typed { tab: usize, word: String, answer: Answer<Word> },
}

impl actor::Machine for Standing {
    type Msg = Msg;

    fn step(mut self, message: Msg) -> Self {
        match message {
            Msg::Look { tab, onto } => {
                self.onto[tab] = onto;
                self
            },
            Msg::Asking { tab, word } => {
                self.asking[tab] = Some(word);
                self
            },
            Msg::Forget { tab } => {
                self.typed[tab] = String::new();
                self
            },
            Msg::Standing { tab, landed, answer } => {
                // A search is out until what is written down is the search it
                // was out for. Nothing else says it has ended: the looking is
                // done by a program off this one, and all it leaves behind is
                // the file.
                if self.asking[tab].as_deref() == Some(landed.as_str()) {
                    self.asking[tab] = None;
                }

                let _ = answer.say(Reading {
                    typed: self.typed[tab].clone(),
                    asking: self.asking[tab].clone(),
                    onto: self.onto[tab].clone(),
                });
                self
            },
            Msg::Looking { tab, answer } => {
                let _ = answer.say(self.onto[tab].clone());
                self
            },
            Msg::Both { tab, answer } => {
                let _ = answer.say((self.onto[tab].clone(), self.typed[tab].clone()));
                self
            },
            Msg::Typed { tab, word, answer } => {
                let changed = match self.typed[tab] == word {
                    true => Word::Same,
                    false => Word::Changed,
                };
                self.typed[tab] = word;
                let _ = answer.say(changed);
                self
            },
        }
    }
}

/// Where the panel reaches it. Cloned into every closure that used to be
/// handed the lock.
type Held = Addr<Msg>;

/// What a tab is looking at, asked of the owner.
///
/// The list, if the owner has gone: the panel is on its way out by then, and
/// the list is the tab as it opens.
fn looking_at(held: &Held, tab: usize) -> Onto {
    match held.ask(|answer| Msg::Looking { tab, answer }) {
        Ok(onto) => onto,
        Err(_) => Onto::List,
    }
}

/// Look at something else, and stand on a given row of it.
fn look(held: &Held, tab: usize, onto: Onto, showing: &dyn Showing, row: usize) {
    let _ = held.tell(Msg::Look { tab, onto });
    showing.replace(row);
}

// ------------------------------------------------------------------ the rows

/// What the last search on this tab came to.
fn looked(kind: Kind) -> Looked {
    let at = store::found_at(&glib::user_cache_dir(), kind);

    // No search on this tab yet, which is what a fresh cache looks like, or a
    // cache this cannot read. Both are a tab with nothing found in it, which
    // is where every session starts anyway.
    let Ok(said) = std::fs::read_to_string(at) else {
        return Looked::default();
    };

    looking::kept(&said)
}

fn rows_of(held: &Held, tab: usize) -> Vec<Row> {
    let kind = Kind::BOTH[tab];
    let looked = looked(kind);
    let landed = looked.asked.clone();

    let Ok(Reading { typed, asking, onto }) =
        held.ask(|answer| Msg::Standing { tab, landed, answer })
    else {
        return Vec::new();
    };

    match onto {
        Onto::Ways { found, from } => ways(held, tab, &found, from),
        Onto::List => {
            let cache = glib::user_cache_dir();
            let folder = getting::into(kind);
            rows::rows(&typed, asking.as_deref(), &looked, looking_for(held, tab, &typed), &|at,
             found| {
                thing(held, tab, at, found, &cache, &folder)
            })
        }
    }
}

/// One thing that was found. A fetches it, and Y is what else can be done with
/// it.
fn thing(held: &Held, tab: usize, at: usize, found: &Found, cache: &Path, folder: &Path) -> Row {
    let kind = Kind::BOTH[tab];
    // Asked of the folder rather than of a list this panel keeps, so a song
    // fetched last week says so as surely as one fetched a minute ago.
    let aside = looking::aside(kind, found, getting::holds(folder, &found.id));
    let fetching = found.clone();
    let opening = found.clone();
    let held = held.clone();
    Row::new(
        &found.title,
        &aside,
        Does::and_stay(move |showing| get(showing, kind, &fetching)),
    )
    .picturing(picture(cache, found))
    .offering(move |showing| {
        look(&held, tab, Onto::Ways { found: opening.clone(), from: at }, showing, WAYS_START);
        false
    })
}

/// The picture of a thing, where one has been fetched.
///
/// Room is kept either way, so a list whose pictures have not all arrived still
/// has its titles starting in one place.
fn picture(cache: &Path, found: &Found) -> Picture {
    match store::picture_of(cache, &found.id) {
        Some(at) if at.exists() => Picture::At(at),
        _ => Picture::Space,
    }
}

/// What else can be done with the thing Y was pressed on.
fn ways(held: &Held, tab: usize, found: &Found, from: usize) -> Vec<Row> {
    let other = Kind::BOTH[tab].other();
    let leaving = held.clone();
    let backing = held.clone();
    let one = found.clone();
    let get_the_other = Does::and_stay(move |showing| {
        get(showing, other, &one);
        // Back to the list it was opened from: what Y offers is done with, and
        // the thing it was about is where the highlight belongs.
        look(&leaving, tab, Onto::List, showing, from);
    });
    rows::ways(
        found,
        other,
        move |showing| look(&backing, tab, Onto::List, showing, from),
        get_the_other,
    )
}

// -------------------------------------------------------------- what it does

/// Hand the word to the program that looks, and say on the row that it is out.
///
/// Drawn again before it is handed over, because `later` only draws again when
/// what it started has ended: without this the press has no answer until the
/// network gives one, which is the shape of a button that looks broken.
fn looking_for(held: &Held, tab: usize, typed: &str) -> Does {
    let held = held.clone();
    let kind = Kind::BOTH[tab];
    let word = typed.trim().to_string();
    Does::and_stay(move |showing| {
        let _ = held.tell(Msg::Asking { tab, word: word.clone() });
        showing.refresh();
        showing.later(vec![
            "download-find".to_string(),
            kind.flag().to_string(),
            word.clone(),
        ]);
    })
}

/// Hand one thing to the program that fetches it, and say it is going.
///
/// The corner rather than a row, because the list is unchanged by the press and
/// a press that leaves everything exactly as it was is a press somebody makes
/// again. What it says is where the thing will be, since that is the one thing
/// the person pressing cannot see from here.
fn get(showing: &dyn Showing, kind: Kind, found: &Found) {
    let into = getting::into(kind);
    let where_ = into
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| into.display().to_string());

    // What is there is not fetched again. The row already says "have it", and
    // a press that started a minute of work whose only possible ending is a
    // fault is a row saying one thing and doing another.
    if getting::holds(&into, &found.id) == getting::Have::It {
        showing.note(&format!("{} is already in {where_}", found.title));
        return;
    }

    showing.note(&format!("{} is on its way into {where_}", found.title));
    // Its name goes with it, so what is said when it lands or when it fails is
    // the thing itself rather than a link nobody can read.
    showing.later(vec![
        "download-get".to_string(),
        kind.flag().to_string(),
        found.url.clone(),
        found.title.clone(),
    ]);
}

/// Stop looking, and leave the tab as it was before anything was typed.
///
/// The word is the panel's, so it is the panel that is asked to forget it. What
/// was found stays up: it is still true, and B is one step back rather than a
/// way of throwing the last minute away.
fn stopped(held: &Held, tab: usize, showing: &dyn Showing) {
    showing.forget_typing();
    let _ = held.tell(Msg::Forget { tab });
    showing.replace(rows::LINE);
}

// ------------------------------------------------------------------ the tabs

fn pages(held: &Held) -> Vec<Page> {
    Kind::BOTH.iter().enumerate().map(|(tab, kind)| page(held, tab, kind.tab())).collect()
}

fn page(held: &Held, tab: usize, title: &str) -> Page {
    let reading = held.clone();
    let backing = held.clone();
    let page = Page::new(title, Rows::asked(move || rows_of(&reading, tab))).on_back(
        move |showing| {
            let Ok((onto, typed)) = backing.ask(|answer| Msg::Both { tab, answer }) else {
                return true;
            };

            match onto {
                Onto::Ways { from, .. } => {
                    look(&backing, tab, Onto::List, showing, from);
                    false
                }
                Onto::List if !typed.trim().is_empty() => {
                    stopped(&backing, tab, showing);
                    false
                }
                Onto::List => true,
            }
        },
    );

    // Only over the list. Y's list is three things that can be done with one
    // thing, and a line to type into over them is an invitation to type at a
    // page that is not asking for letters.
    if !matches!(looking_at(held, tab), Onto::List) {
        return page;
    }

    let typing = held.clone();
    page.searching(ABOUT, move |showing, word| {
        let changed = typing.ask(|answer| Msg::Typed { tab, word: word.to_string(), answer });

        // Standing on the line, which is where the letters go. The row under it
        // has just become the row that looks for what is in it.
        if matches!(changed, Ok(Word::Changed)) {
            showing.replace(0);
        }
    })
}

fn main() {
    // A tab may be named, so something that means video opens on it.
    let tab = std::env::args().nth(1);

    if chooser::alone("download", chooser::Again::Closes) == chooser::Alone::No {
        return;
    }

    let standing = actor::supervise(Standing::new);
    let held = standing.addr.clone();
    panel::show(Arc::new(move || pages(&held)), 0, tab.as_deref());
    // The panel is down and nothing is going to ask again. Waited for rather
    // than dropped, so a message already in the mailbox is finished with.
    standing.shutdown();
}
