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
use std::sync::{Arc, Mutex};

use gtk4::glib;
use console_download::getting;
use console_download::looking::{self, Found, Looked};
use console_download::rows::{self, ABOUT, WAYS_START};
use console_download::store::{self, Kind};
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

type Held = Arc<Mutex<Standing>>;

/// The lock, taken and given back before anything is drawn or run.
fn standing<T>(held: &Held, then: impl FnOnce(&mut Standing) -> T) -> T {
    then(&mut held.lock().expect("the standing"))
}

/// Look at something else, and stand on a given row of it.
fn look(held: &Held, tab: usize, onto: Onto, showing: &dyn Showing, row: usize) {
    standing(held, |standing| standing.onto[tab] = onto);
    showing.replace(row);
}

// ------------------------------------------------------------------ the rows

/// What the last search on this tab came to.
fn looked(kind: Kind) -> Looked {
    let at = store::found_at(&glib::user_cache_dir(), kind);
    looking::kept(&std::fs::read_to_string(at).unwrap_or_default())
}

fn rows_of(held: &Held, tab: usize) -> Vec<Row> {
    let kind = Kind::BOTH[tab];
    let looked = looked(kind);
    let (typed, asking, onto) = standing(held, |standing| {
        // A search is out until what is written down is the search it was out
        // for. Nothing else says it has ended: the looking is done by a program
        // off this one, and all it leaves behind is the file.
        if standing.asking[tab].as_deref() == Some(looked.asked.as_str()) {
            standing.asking[tab] = None;
        }
        (standing.typed[tab].clone(), standing.asking[tab].clone(), standing.onto[tab].clone())
    });
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
    let held = Arc::clone(held);
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
    let leaving = Arc::clone(held);
    let backing = Arc::clone(held);
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
    let held = Arc::clone(held);
    let kind = Kind::BOTH[tab];
    let word = typed.trim().to_string();
    Does::and_stay(move |showing| {
        standing(&held, |standing| standing.asking[tab] = Some(word.clone()));
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
    if getting::holds(&into, &found.id) {
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
    standing(held, |standing| standing.typed[tab] = String::new());
    showing.replace(rows::LINE);
}

// ------------------------------------------------------------------ the tabs

fn pages(held: &Held) -> Vec<Page> {
    Kind::BOTH.iter().enumerate().map(|(tab, kind)| page(held, tab, kind.tab())).collect()
}

fn page(held: &Held, tab: usize, title: &str) -> Page {
    let reading = Arc::clone(held);
    let backing = Arc::clone(held);
    let page = Page::new(title, Rows::asked(move || rows_of(&reading, tab))).on_back(
        move |showing| {
            let (onto, typed) = standing(&backing, |standing| {
                (standing.onto[tab].clone(), standing.typed[tab].clone())
            });
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
    if !matches!(standing(held, |standing| standing.onto[tab].clone()), Onto::List) {
        return page;
    }
    let typing = Arc::clone(held);
    page.searching(ABOUT, move |showing, word| {
        let changed = standing(&typing, |standing| {
            let changed = standing.typed[tab] != word;
            standing.typed[tab] = word.to_string();
            changed
        });
        // Standing on the line, which is where the letters go. The row under it
        // has just become the row that looks for what is in it.
        if changed {
            showing.replace(0);
        }
    })
}

fn main() {
    // A tab may be named, so something that means video opens on it.
    let tab = std::env::args().nth(1);

    if !chooser::alone("download", chooser::Again::Closes) {
        return;
    }
    let held: Held = Arc::new(Mutex::new(Standing::new()));
    panel::show(Arc::new(move || pages(&held)), 0, tab.as_deref());
}
