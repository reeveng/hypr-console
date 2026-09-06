//! A photograph and a film, drawn.
//!
//! ```text
//!     viewer-panel ~/Pictures/beach.jpg
//!     viewer-panel ~/Pictures
//!     viewer-panel
//! ```
//!
//! Opened by pressing A on a picture in the files panel, which is xdg-open,
//! which is `console-viewer.desktop`, which is this. Handed a folder instead
//! it opens on the first thing in it that can be shown, and handed nothing at
//! all it opens the pictures folder: the same entry is on the home screen, and
//! a card on the home screen that starts a program which prints a usage line to
//! a stderr nobody can see is a card that does nothing.
//!
//! What is here is the reading of a disk, GStreamer, and the drawing of a
//! card. Everything that is a decision -- which things in a folder can be
//! shown, which one is next, what the row under the picture says, and what a
//! press forgets about the last thing -- is `console_viewer`, where it is
//! tested without either. `watching` is the composition of the rest of it and
//! is a `console_program_contract::Program`.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use console_external_programs::Program;
use console_never::Never;
use console_number_conversion::fitted;
use console_panel::icons::Icon;
use console_panel::card::{Card, Door};
use console_panel::page::{Bar, Does, InEffect, Page, Picture, Press, Row, Rows, Stirred, Watch};
use console_program_contract::{Doing, Program as _, Turn, Word};
use crate::kinds::Kind;
use crate::{playing, saying};
use crate::playing::Running;
use crate::reel::Reel;
use crate::waking::Awake;
use crate::playing::Captions;
use crate::watching::{
    Alone, Its, Since, Watch as Watched, Watching, alone, awake, stirred,
};
use gstreamer::prelude::*;

fn kind_of(path: &Path) -> Result<String, Never> {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Ok(String::new());
    };

    let (kind, _) = gtk4::gio::functions::content_type_guess(Some(name), None::<&[u8]>);

    Ok(gtk4::gio::functions::content_type_get_mime_type(&kind)
        .map(|said| said.to_string())
        .unwrap_or_default())
}

fn listing(folder: &Path) -> Result<Vec<(String, String)>, Never> {
    let mut found: Vec<(String, String)> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| !entry.path().is_dir())
        .filter_map(|entry| {
            let Ok(name) = entry.file_name().into_string() else {
                eprintln!("viewer-panel: {:?}: this name is not text", entry.file_name());

                return None;
            };

            match name.starts_with('.') {
                true => None,
                false => {
                    let Ok(kind) = kind_of(&entry.path());

                    Some((name, kind))
                },
            }
        })
        .collect();

    found.sort_by(|(one, _), (two, _)| one.cmp(two));

    Ok(found)
}

struct Looking {
    folder: PathBuf,
    watching: Watching,
    began: Instant,
}

impl Looking {
    fn of(asked: &Path) -> Result<Option<Looking>, Never> {
        let (folder, opened) = match asked.is_dir() {
            true => (asked.to_path_buf(), String::new()),
            false => (
                asked.parent().unwrap_or(Path::new(".")).to_path_buf(),
                asked.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_string(),
            ),
        };

        let Ok(said) = listing(&folder);
        let Ok(found) = Reel::of(&said, &opened);

        let Some(reel) = found else {
            return Ok(None);
        };

        let Ok(watching) = Watching::of(reel, Since::ZERO);

        Ok(Some(Looking { folder, watching, began: Instant::now() }))
    }

    fn now(&self) -> Result<Since, Never> {
        Ok(self.began.elapsed())
    }

    fn at(&self) -> Result<PathBuf, Never> {
        let Ok(showing) = self.watching.showing();

        Ok(self.folder.join(&showing.name))
    }

    fn bytes(&self) -> Result<u64, Never> {
        let Ok(at) = self.at();

        Ok(match std::fs::metadata(&at) {
            Ok(held) => held.len(),
            Err(fault) => {
                eprintln!("viewer-panel: {}: {fault}", at.display());

                0
            }
        })
    }

    fn shape(&self) -> Result<(u32, u32), Never> {
        let Ok(showing) = self.watching.showing();
        let Ok(at) = self.at();

        Ok(match showing.kind {
            Kind::Film => (0, 0),
            Kind::Picture => match gtk4::gdk_pixbuf::Pixbuf::file_info(at) {
                Some((_, wide, tall)) => (wide.unsigned_abs(), tall.unsigned_abs()),
                None => (0, 0),
            },
        })
    }

    fn heard(&mut self, heard: crate::watching::Heard) -> Result<Vec<Doing<Its>>, Never> {
        let Turn { now, doings } = Watched::heard(&self.watching, &Word::Its(heard));

        self.watching = now;

        Ok(doings)
    }
}

type Held = Arc<Mutex<Looking>>;

use crate::watching::Heard;

fn press(
    held: &Held,
    heard: Heard,
    showing: &dyn console_panel::page::Showing,
) -> Result<(), Never> {
    let doings = match held.lock() {
        Ok(mut looking) => {
            let Ok(doings) = looking.heard(heard);

            doings
        },
        Err(_the_lock_is_poisoned) => Vec::new(),
    };

    for doing in doings {
        match doing {
            Doing::Its(Its::Refresh) => showing.refresh(),
            Doing::Its(Its::TurnToTheCard) => showing.turn_to(CARD),

            Doing::Ask(_)
            | Doing::Watch(_)
            | Doing::AskWhoever(_)
            | Doing::Start(_)
            | Doing::Listen(_)
            | Doing::Deafen(_)
            | Doing::Write(_)
            | Doing::Say(_)
            | Doing::Print(_)
            | Doing::Stop(_) => {},
        }
    }

    Ok(())
}

fn quietly(held: &Held, heard: Heard) -> Result<(), Never> {
    match held.lock() {
        Ok(mut looking) => {
            let _ = looking.heard(heard);
        }
        Err(_the_lock_is_poisoned) => {},
    }

    Ok(())
}

thread_local! {
    static REEL: RefCell<Option<Reeling>> = const { RefCell::new(None) };
}

struct Reeling {
    at: PathBuf,
    play: gstreamer::Element,
    surface: gtk4::gdk::Paintable,
    rate: f64,
    told: Option<Captions>,
}

impl Drop for Reeling {
    fn drop(&mut self) {
        match self.play.set_state(gstreamer::State::Null) {
            Ok(_) => {},
            Err(fault) => {
                eprintln!("viewer-panel: {}: will not stop: {fault}", self.at.display());
            }
        }
    }
}

fn words_beside(at: &Path) -> Result<Option<PathBuf>, Never> {
    let Some(name) = at.file_name().and_then(|name| name.to_str()) else { return Ok(None) };

    let Some(folder) = at.parent() else { return Ok(None) };

    let Ok(beside) = playing::beside(name);

    Ok(beside.into_iter().map(|name| folder.join(name)).find(|beside| beside.is_file()))
}

fn open(at: &Path) -> Result<Option<Reeling>, Never> {
    let uri = match gtk4::glib::filename_to_uri(at, None) {
        Ok(uri) => uri,
        Err(fault) => {
            eprintln!("viewer-panel: {}: {fault}", at.display());

            return Ok(None);
        }
    };

    let sink = match gstreamer::ElementFactory::make("gtk4paintablesink").build() {
        Ok(sink) => sink,
        Err(fault) => {
            eprintln!("viewer-panel: no gtk4paintablesink, so no film can be drawn: {fault}");
            eprintln!("viewer-panel: it is gst-plugin-gtk4, which desktop.conf names");

            return Ok(None);
        }
    };

    let surface: gtk4::gdk::Paintable = sink.property("paintable");

    let mut building = gstreamer::ElementFactory::make("playbin")
        .property("uri", &uri)
        .property("video-sink", &sink);

    let Ok(beside) = words_beside(at);

    match beside {
        Some(beside) => match gtk4::glib::filename_to_uri(&beside, None) {
            Ok(uri) => building = building.property("suburi", uri),
            Err(fault) => eprintln!("viewer-panel: {}: {fault}", beside.display()),
        },
        None => {},
    }

    let play = match building.build() {
        Ok(play) => play,
        Err(fault) => {
            eprintln!("viewer-panel: {}: {fault}", at.display());

            return Ok(None);
        }
    };

    match play.set_state(gstreamer::State::Paused) {
        Ok(_) => {},
        Err(fault) => {
            eprintln!("viewer-panel: {}: will not open: {fault}", at.display());

            return Ok(None);
        }
    }

    Ok(Some(Reeling {
        at: at.to_path_buf(),
        play,
        surface,
        rate: 1.0,
        told: None,
    }))
}

const TEXT: &str = "current-text";
const TRACKS: &str = "n-text";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Has {
    It,
    Not,
}

fn has(play: &gstreamer::Element, named: &str) -> Result<Has, Never> {
    Ok(match play.find_property(named).is_some() {
        true => Has::It,
        false => {
            eprintln!("viewer-panel: this player has no {named}, so subtitles cannot be chosen");

            Has::Not
        }
    })
}

fn reeling(held: &Held, at: &Path) -> Result<Option<gtk4::gdk::Paintable>, Never> {
    Ok(REEL.with_borrow_mut(|reel| {
        match reel.as_ref().is_none_or(|open| open.at != at) {
            true => {
                let Ok(opened) = open(at);

                *reel = opened;
            }
            false => {},
        }

        let open = reel.as_mut()?;

        let Ok(mut looking) = held.lock() else { return Some(open.surface.clone()) };

        match looking.watching.sought.take() {
            Some(to) => {
                let flags = gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::KEY_UNIT;
                let sought =
                    open.play.seek_simple(flags, gstreamer::ClockTime::from_seconds(to));

                match sought {
                    Ok(()) => {},
                    Err(fault) => {
                        eprintln!("viewer-panel: {}: will not seek: {fault}", at.display());
                    }
                }
            }
            None => {},
        }

        let Ok((_, rate)) = playing::speed(looking.watching.speed);

        match rate == open.rate {
            true => {},
            false => {
                let at_now = open
                    .play
                    .query_position::<gstreamer::ClockTime>()
                    .unwrap_or(gstreamer::ClockTime::ZERO);
                let flags = gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::ACCURATE;

                match open.play.seek(
                    rate,
                    flags,
                    gstreamer::SeekType::Set,
                    at_now,
                    gstreamer::SeekType::End,
                    gstreamer::ClockTime::ZERO,
                ) {
                    Ok(()) => open.rate = rate,
                    Err(fault) => {
                        eprintln!(
                            "viewer-panel: {}: will not run at {rate}: {fault}",
                            at.display()
                        );
                    }
                }
            }
        }

        let Ok(text) = has(&open.play, TEXT);
        let Ok(tracks) = has(&open.play, TRACKS);

        match text == Has::It && tracks == Has::It {
            true => {
                match open.told == Some(looking.watching.captions) {
                    true => {},
                    false => {
                        let Ok(track) = looking.watching.captions.track();

                        let told = match track {
                            Some(track) => {
                                let Ok(told) = fitted(track);

                                told
                            }
                            None => -1,
                        };

                        open.play.set_property(TEXT, told);
                        open.told = Some(looking.watching.captions);
                    }
                }

                let Ok(tracks) = fitted(open.play.property::<i32>(TRACKS).max(0));

                looking.watching.tracks = tracks;
            }
            false => {},
        }

        let wanted = match looking.watching.running {
            Running::Yes => gstreamer::State::Playing,
            Running::Paused => gstreamer::State::Paused,
        };

        match open.play.set_state(wanted) {
            Ok(_) => {},
            Err(fault) => {
                eprintln!("viewer-panel: {}: will not {wanted:?}: {fault}", at.display());
            }
        }

        let at = open
            .play
            .query_position::<gstreamer::ClockTime>()
            .map_or(looking.watching.along.at, |now| now.seconds());
        let whole = open
            .play
            .query_duration::<gstreamer::ClockTime>()
            .map_or(looking.watching.along.whole, |whole| whole.seconds());

        let _ = looking.heard(Heard::Where { at, whole });

        Some(open.surface.clone())
    }))
}


fn rows(held: &Held) -> Result<Vec<Row>, Never> {
    let Ok(mut looking) = held.lock() else { return Ok(Vec::new()) };

    let Ok(standing) = looking.at();

    match standing.exists() {
        true => {},
        false => {
            let Ok(listing) = listing(&looking.folder);
            let Ok(at) = looking.now();
            let _ = looking.heard(Heard::Listed { listing, at });
        }
    }

    let Ok(now) = looking.now();
    let Ok(showing) = looking.watching.showing();
    let shot = showing.clone();
    let Ok(at) = looking.at();
    let Ok((wide, tall)) = looking.shape();
    let shown = at.exists().then(|| at.clone());
    let stepping = Arc::clone(held);

    let Ok(card) = match shot.kind {
        Kind::Picture => Row::showing(Picture::Showing(shown.clone())),
        Kind::Film => Row::showing(Picture::Playing(shown.clone())),
    };
    let Ok(opens) = Does::and_stay(|showing| showing.open_out());
    let Ok(card) = card.choosing(opens);
    let Ok(card) = card.levelled(Arc::new(move |by| {
        let Ok(by) = fitted(by);
        let Ok(()) = walk(&stepping, by);
    }));
    let Ok(card) = card.ended("", "");
    let Ok(card) = card.chief();
    let mut every = vec![card];

    let Ok(awake) = awake(&looking.watching, now);

    match awake {
        Awake::No => return Ok(every),
        Awake::Yes => {},
    }

    let Ok(where_in) = where_in(&looking.watching);
    let Ok(naming) = Row::naming(&shot.name, &where_in);
    let Ok(naming) = naming.in_the_middle();

    every.push(naming);

    match shot.kind {
        Kind::Film => {
            let Ok(bar) = bar_row(held, looking.watching.along);

            every.push(bar);
        }
        Kind::Picture => {},
    }

    let Ok(alone) = alone(&looking.watching);

    match shot.kind == Kind::Film || alone == Alone::No {
        true => {
            let Ok(transport) = transport(held, &looking.watching);

            every.push(transport);
        }
        false => {},
    }

    let facts = match shown.is_some() {
        true => {
            let Ok(size) = crate::fitting::Size::new(wide, tall);
            let Ok(bytes) = looking.bytes();
            let Ok(said) = saying::under(shot.kind, size, bytes, looking.watching.along);

            said
        }
        false => {
            let Ok(said) = saying::wont_open(&shot.name);

            said
        },
    };

    what_is_this(every, held, shot.kind, &facts, looking.watching.tracks)
}

fn where_in(watching: &Watching) -> Result<String, Never> {
    let Ok(alone) = alone(watching);

    Ok(match alone {
        Alone::No => {
            let Ok(which) = watching.reel.which();
            let Ok(many) = watching.reel.many();

            format!("{which} of {many}")
        },
        Alone::Yes => String::new(),
    })
}

fn what_is_this(
    rows: Vec<Row>,
    held: &Held,
    kind: Kind,
    facts: &str,
    tracks: usize,
) -> Result<Vec<Row>, Never> {
    Ok(rows.into_iter()
        .map(|row| {
            let Ok(heading) = row.heading();

            match (heading, row.naming) {
                (console_panel::page::Heading::Yes, false) => row,
                (console_panel::page::Heading::Yes, true)
                | (console_panel::page::Heading::No, _) => {
                    let asking = Arc::clone(held);
                    let facts = facts.to_string();

                    let Ok(offering) = row.offering(move |showing| {
                        let Ok(()) = what_else(&asking, showing, kind, &facts, tracks);

                        false
                    });

                    offering
                }
            }
        })
        .collect())
}

fn what_else(
    held: &Held,
    showing: &dyn console_panel::page::Showing,
    kind: Kind,
    facts: &str,
    tracks: usize,
) -> Result<(), Never> {
    let speeding = Arc::clone(held);
    let wording = Arc::clone(held);
    let about = facts.to_string();

    match kind {
        Kind::Picture => showing.sure(
            "About this picture",
            facts,
            &[FULL_SCREEN],
            Arc::new(move |showing, _| showing.open_out()),
        ),
        Kind::Film => showing.sure(
            "About this film",
            facts,
            &["Speed", "Subtitles", FULL_SCREEN],
            Arc::new(move |showing, which| match which {
                0 => {
                    let Ok(()) = how_fast(&speeding, showing, &about);
                }
                1 => {
                    let Ok(()) = which_words(&wording, showing, &about, tracks);
                }
                _ => showing.open_out(),
            }),
        ),
    }

    Ok(())
}

const FULL_SCREEN: &str = "Full screen";

fn how_fast(
    held: &Held,
    showing: &dyn console_panel::page::Showing,
    name: &str,
) -> Result<(), Never> {
    let setting = Arc::clone(held);
    let says: Vec<&str> = playing::SPEEDS.iter().map(|(says, _)| *says).collect();

    showing.sure(
        "How fast",
        name,
        &says,
        Arc::new(move |_, which| {
            let Ok(at) = now(&setting);
            let Ok(()) = quietly(&setting, Heard::Speed { which, at });
        }),
    );

    Ok(())
}

fn which_words(
    held: &Held,
    showing: &dyn console_panel::page::Showing,
    name: &str,
    tracks: usize,
) -> Result<(), Never> {
    let setting = Arc::clone(held);
    let Ok(said) = playing::captions(tracks);
    let says: Vec<&str> = said.iter().map(String::as_str).collect();

    showing.sure(
        "Subtitles",
        name,
        &says,
        Arc::new(move |_, which| {
            let Ok(at) = now(&setting);
            let Ok(()) = quietly(&setting, Heard::Words { which, at });
        }),
    );

    Ok(())
}

fn transport(held: &Held, watching: &Watching) -> Result<Row, Never> {
    let running = watching.running;
    let Ok(only) = alone(watching);
    let Ok(showing) = watching.showing();
    let plays = showing.kind == Kind::Film;
    let (back, on, forward) = (Arc::clone(held), Arc::clone(held), Arc::clone(held));

    let mut presses = Vec::new();

    match only {
        Alone::Yes => {},
        Alone::No => {
            let Ok(previous) = Press::new(Icon::Previous, InEffect::No, move |showing| {
                let Ok(()) = walk(&back, -1);

                showing.refresh();
            });

            presses.push(previous);
        }
    }

    match plays {
        true => {
            let Ok(icon) = running.icon();
            let Ok(running) = Press::new(icon, InEffect::No, move |showing| {
                let Ok(at) = now(&on);
                let Ok(()) = press(&on, Heard::Running(at), showing);
            });
            let Ok(running) = running.chief();

            presses.push(running);
        }
        false => {},
    }

    match only {
        Alone::Yes => {},
        Alone::No => {
            let Ok(next) = Press::new(Icon::Next, InEffect::No, move |showing| {
                let Ok(()) = walk(&forward, 1);

                showing.refresh();
            });

            presses.push(next);
        }
    }

    let at = match (only, plays) {
        (Alone::Yes, _) => 0,
        (Alone::No, true) => 1,
        (Alone::No, false) => 0,
    };

    Row::pressing(presses, at)
}

fn bar_row(held: &Held, along: playing::Along) -> Result<Row, Never> {
    let stepping = Arc::clone(held);
    let tapped = Arc::clone(held);

    let whole = match along.whole > 0 {
        true => {
            let Ok(clock) = playing::clock(along.whole);

            clock
        },
        false => String::new(),
    };

    let Ok(at) = playing::clock(along.at);
    let Ok(nothing) = Does::and_stay(|_| {});
    let Ok(row) = Row::new(&at, &whole, nothing);
    let Ok(row) = row.picturing(Picture::Bar(Bar { at: along.at, of: along.whole }));
    let Ok(row) = row.ended("", "");
    let Ok(row) = row.levelled(Arc::new(move |by| {
        let Ok(()) = scrub(&stepping, by);
    }));

    row.seeking(move |showing, fraction| {
        let Ok(()) = seek_to(&tapped, fraction);

        showing.refresh();
    })
}

fn now(held: &Held) -> Result<Since, Never> {
    Ok(match held.lock() {
        Ok(looking) => {
            let Ok(now) = looking.now();

            now
        },
        Err(_the_lock_is_poisoned) => Since::ZERO,
    })
}

fn seek_to(held: &Held, fraction: f64) -> Result<(), Never> {
    let Ok(at) = now(held);

    quietly(held, Heard::SoughtTo { fraction, at })
}

fn scrub(held: &Held, by: i32) -> Result<(), Never> {
    let Ok(at) = now(held);

    quietly(held, Heard::Scrubbed { by, at })
}

fn walk(held: &Held, by: isize) -> Result<(), Never> {
    let Ok(at) = now(held);

    quietly(held, Heard::Stepped { by, at })
}

const CARD: usize = 0;

fn folder_rows(held: &Held) -> Result<Vec<Row>, Never> {
    let Ok(looking) = held.lock() else { return Ok(Vec::new()) };

    let here = looking.watching.reel.which();

    let Ok(every) = looking.watching.reel.every();

    Ok(every
        .iter()
        .enumerate()
        .map(|(at, shot)| {
            let going = Arc::clone(held);
            let name = shot.name.clone();
            let picture = match shot.kind {
                Kind::Picture => Picture::At(looking.folder.join(&shot.name)),
                Kind::Film => Picture::Named(Icon::Film),
            };

            let Ok(here) = here;

            let aside = match at.saturating_add(1) == here {
                true => console_panel::page::NOW,
                false => "",
            };

            let Ok(stands) = Does::and_stay(move |showing| {
                let Ok(at) = now(&going);
                let Ok(()) = press(&going, Heard::StoodOn { name: name.clone(), at }, showing);
            });
            let Ok(row) = Row::new(&shot.name, aside, stands);
            let Ok(row) = row.picturing(picture);

            row
        })
        .collect())
}

fn stir(held: &Held) -> Result<Stirred, Never> {
    let Ok(mut looking) = held.lock() else { return Ok(Stirred::Awake) };

    let Ok(at) = looking.now();
    let Ok(was) = stirred(&looking.watching, at);
    let _ = looking.heard(Heard::Stirred(at));

    Ok(match was {
        crate::watching::Stirred::Awake => Stirred::Awake,
        crate::watching::Stirred::Woke => Stirred::Woke,
    })
}

fn pages(held: &Held) -> Result<Vec<Page>, Never> {
    let drawing = Arc::clone(held);
    let stirring = Arc::clone(held);
    let listing = Arc::clone(held);
    let Ok(shell) = Program::Sh.name();

    let Ok(asked) = Rows::asked(move || {
        let Ok(rows) = rows(&drawing);

        rows
    });
    let Ok(page) = Page::new("Looking", asked);
    let Ok(watch) = Watch::on(
        &[shell, "-c", "while true; do echo tick; sleep 1; done"],
        "tick",
    );
    let Ok(page) = page.watching(watch);
    let Ok(looking) = page.stirring(move || {
        let Ok(stirred) = stir(&stirring);

        stirred
    });
    let Ok(asked) = Rows::asked(move || {
        let Ok(rows) = folder_rows(&listing);

        rows
    });
    let Ok(folder) = Page::new("Folder", asked);

    Ok(vec![looking, folder])
}

fn by_default() -> Result<Option<PathBuf>, Never> {
    Ok(gtk4::glib::user_special_dir(gtk4::glib::UserDirectory::Pictures)
        .filter(|folder| folder.is_dir()))
}


pub const WHO: &str = "viewer-panel";

const DOOR: &str = "viewer";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worth {
    Opening,
    Nothing,
}

pub fn door(_argv: &[String]) -> Result<Door, Never> {
    Door::closing(DOOR)
}

pub fn worth_opening(argv: &[String]) -> Result<Worth, Never> {
    let Ok(asked) = asked_for(argv);

    let Some(asked) = asked else { return Ok(Worth::Nothing) };

    let Ok(found) = Looking::of(&asked);

    match found {
        Some(_) => Ok(Worth::Opening),
        None => {
            eprintln!("viewer-panel: {}: nothing here is a picture or a film", asked.display());

            Ok(Worth::Nothing)
        }
    }
}

fn asked_for(argv: &[String]) -> Result<Option<PathBuf>, Never> {
    let said = argv.first().filter(|said| !said.is_empty()).map(PathBuf::from);

    match said {
        Some(said) => Ok(Some(said)),
        None => {
            let Ok(folder) = by_default();

            let Some(folder) = folder else {
                eprintln!("usage: viewer-panel FILE-OR-FOLDER");

                return Ok(None);
            };

            Ok(Some(folder))
        }
    }
}

pub fn card(argv: &[String]) -> Result<Card, Never> {
    let Ok(asked) = asked_for(argv);

    let Some(asked) = asked else { return Card::new(Arc::new(Vec::new)) };

    let Ok(found) = Looking::of(&asked);

    let Some(looking) = found else { return Card::new(Arc::new(Vec::new)) };

    match gstreamer::init() {
        Ok(()) => {},
        Err(fault) => eprintln!("viewer-panel: no film can be read: {fault}"),
    }

    let held: Held = Arc::new(Mutex::new(looking));
    let drawing = Arc::clone(&held);
    let reading = Arc::clone(&held);

    let Ok(()) = console_panel::panel::films(move |at| {
        let Ok(drawn) = reeling(&reading, at);

        drawn
    });
    let Ok(card) = Card::new(Arc::new(move || {
        let Ok(pages) = pages(&drawing);

        pages
    }));

    card.shutting(Box::new(|| {
        REEL.with(|held| *held.borrow_mut() = None);

        Ok(())
    }))
}
