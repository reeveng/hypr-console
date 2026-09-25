//! A photograph drawn, and a film named.
//!
//! ```text
//!     viewer ~/Pictures/beach.jpg
//!     viewer ~/Pictures
//!     viewer
//! ```
//!  Opened by pressing A on a picture in the files panel, which is xdg-open,
//! which is `console-media-viewer.desktop`, which is this. Handed a folder
//! instead it opens on the first thing in it that can be shown, and handed
//! nothing at all it opens the pictures folder: the same entry is on the home
//! screen, and a card on the home screen that starts a program which prints a
//! usage line to a stderr no one can see is a card that does nothing.
//!
//! That argument was made here and then only half kept. A folder with no
//! picture and no film in it -- which is what a pictures folder is on a device
//! no one has taken a photograph on yet -- took the other road: the panel
//! decided it was not worth opening, said so on the same stderr no one can
//! see, and exited zero. Pressing Viewer on the home screen did nothing at
//! all, and a press that does nothing at all reads as a program that fell over
//! rather than as an empty folder. So a folder always opens now, and one with
//! nothing to show opens on the row the panel already draws for a list with
//! nothing in it. What is still refused is a *file* that is neither: there the
//! files panel is on the screen already, and leaving it there is better than
//! taking the screen to say no.
//!
//! What is here is the reading of a disk and the drawing of a card. Everything
//! that is a decision -- which things in a folder can be shown,
//! which one is next, what the row under the picture says, and what a press
//! forgets about the last thing -- is `console_media_viewer`, where it is
//! tested without either. `watching` is the composition of the rest of it and
//! is a `console_program_contract::Program`.
//!
//! Two pages: the thing on the screen, and everything on the device it could
//! be. The second is called Media rather than Folder because what a page is
//! named after is what is on it, the way the music panel's second page is
//! Music: a folder is where these came from, not what they are. It was the
//! folder for a while and the name was the argument against it -- `index` is
//! why it is a shelf of everything now, and what a heading over it stands
//! for.
//!
//! ## Subject is why an empty folder cannot be the end of it
//!
//! The shelf arrived after the paragraph above and the two were never held
//! against each other. A pictures folder with nothing in it took the short road
//! -- one row saying nothing here, and no second page at all -- so a device
//! with two films in Videos and no photographs said it had nothing on it, from
//! the panel whose second page exists to answer exactly that. The check that
//! would have caught it is the one no one had written: there was no press of
//! this panel at any stage.
//!
//! So the folder the panel opens on is where someone came in and never the
//! whole of what there is. When it holds nothing to show, the shelf is walked
//! and the panel stands on the first thing it found, wherever that was filed;
//! only a shelf that is empty too is a device with nothing on it, and only then
//! is there one row and nothing else. Handed nothing at all, the folder it
//! falls back on is the first of Pictures, Videos and Downloads that exists,
//! rather than Pictures or nothing -- a machine that has films and has never
//! been photographed on is the ordinary state of this one.
//!
//! ## A film plays on the card it was already shown on
//!
//! The frames came out of GStreamer into a GTK paintable once, and went with
//! the toolkit. They come back through `film` now: the row that shows a film
//! is the row that shows a photograph, and what is drawn in it is whatever
//! frame the player last handed `console_panel::frames`. Every read of the
//! rows keeps the player in step with what was pressed -- play, pause, a seek,
//! a speed, a subtitle track -- and the clock under the picture comes back out
//! of the player on the same read.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use console_core_localization::positional;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_panel::icons::Icon;
use console_panel::card::Card;
use console_panel::page::{Aside, Bar, Handler, Ends, Active, Page, Picture, ButtonPress, Row, Rows, WakeOutcome, Subject};
use console_program_contract::{Effect, Program as _, Update, Event};
use crate::editing::{self, Edit, Filter};
use crate::index;
use crate::kinds::Kind;
use crate::{playing, saying};
use crate::reel::Reel;
use crate::waking::Woken;
use crate::watching::{
    Alone, ViewerEffect, Since, Watch as Watched, Watching, alone, awake, stirred,
};
use console_content_types::Table;
use console_core_geometry::Size;
use console_core_number_conversion::{Float, toward_zero_u64};
use console_core_places::Folder;
use console_music_player::sounding::{Progress, Sounding};
use crate::film::{self, PlaybackRequest, Facts, Film};
use crate::playing::Running;

fn kinds() -> Result<Table, Never> {
    Ok(match Table::here() {
        Ok(table) => table,
        Err(why) => {
            eprintln!("viewer: nothing says what a file is: {why}");

            Table::default()
        }
    })
}

fn listing(folder: &Path) -> Result<Vec<(String, String)>, Never> {
    let Ok(kinds) = kinds();

    let mut found: Vec<(String, String)> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| !entry.path().is_dir())
        .filter_map(|entry| {
            let name = match entry.file_name().into_string() {
                Ok(name) => name,
                Err(_not_text) => {
                    eprintln!("viewer: {:?}: this name is not text", entry.file_name());

                    return None;
                }
            };

            match name.starts_with('.') {
                true => None,
                false => {
                    let Ok(kind) = console_content_types::of(&kinds, &entry.path());

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
    film: Option<Film>,
    facts: Option<(PathBuf, Option<Facts>)>,
    sound: Option<Arc<Sounding>>,
    measured: Option<(PathBuf, (u32, u32))>,
}

impl Looking {
    fn of(asked: &Path) -> Result<Option<Looking>, Never> {
        let (folder, opened) = match asked.is_dir() {
            true => (asked.to_path_buf(), String::new()),
            false => {
                let holding = match asked.parent() {
                    Some(holding) => holding.to_path_buf(),
                    None => PathBuf::from(HERE),
                };

                let opened = match asked.file_name().and_then(|name| name.to_str()) {
                    Some(opened) => opened.to_string(),
                    None => String::new(),
                };

                (holding, opened)
            }
        };

        let Ok(said) = listing(&folder);
        let Ok(found) = Reel::of(&said, &opened);

        let reel = match found {
            Some(reel) => reel,
            None => return Ok(None),
        };

        let Ok(watching) = Watching::of(reel, Since::ZERO);

        #[cfg_attr(
            dylint_lib = "explicit039_no_reading_the_clock",
            allow(
                explicit039_no_reading_the_clock,
                reason = "`watching` takes every decision from a `Since` handed in and this is the one reading those are counted from, made where the panel goes up rather than anywhere a decision is taken"
            )
        )]
        Ok(Some(Looking {
            folder,
            watching,
            began: Instant::now(),
            film: None,
            facts: None,
            sound: None,
            measured: None,
        }))
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
                eprintln!("viewer: {}: {fault}", at.display());

                0
            }
        })
    }

    fn shape(&mut self) -> Result<(u32, u32), Never> {
        let Ok(showing) = self.watching.showing();
        let kind = showing.kind;
        let Ok(at) = self.at();

        match &self.measured {
            Some((was, shape)) => match *was == at {
                true => return Ok(*shape),
                false => {},
            },
            None => {},
        }

        let shape = match kind {
            Kind::Film => (0, 0),
            Kind::Picture => match console_pictures::measured(&at) {
                Ok(Some(Size { width: wide, height: tall })) => (wide, tall),
                Ok(None) => (0, 0),
                Err(why) => {
                    eprintln!("viewer: {}: {why}", at.display());

                    (0, 0)
                }
            },
        };

        self.measured = Some((at, shape));

        Ok(shape)
    }

    fn facts(&mut self, at: &Path) -> Result<Option<Facts>, Never> {
        match &self.facts {
            Some((was, facts)) => match was.as_path() == at {
                true => return Ok(facts.clone()),
                false => {},
            },
            None => {},
        }

        let Ok(facts) = film::facts(at);

        self.facts = Some((at.to_path_buf(), facts.clone()));

        match &facts {
            Some(facts) => {
                let Ok(whole) = toward_zero_u64(facts.seconds.ceil());
                let Ok(tracks) = film::sidecars(at);
                let at = self.watching.along.at;
                let Ok(beside) = fitted::<_, u32>(tracks.len());
                let _ = self.heard(ViewerEvent::Where { at, whole });
                let _ = self.heard(ViewerEvent::Tracks(facts.words.saturating_add(beside)));
            },
            None => {},
        }

        Ok(facts)
    }

    fn sound(&mut self) -> Result<Arc<Sounding>, Never> {
        match &self.sound {
            Some(sound) => return Ok(Arc::clone(sound)),
            None => {},
        }

        let Ok(sound) = Sounding::new(|progress| match progress {
            Progress::Ended => console_panel::frames::tell(console_panel::frames::Notice::Rows),
            Progress::ASecondPlayed => Ok(()),
        });
        let sound = Arc::new(sound);

        self.sound = Some(Arc::clone(&sound));

        Ok(sound)
    }

    fn in_step(&mut self) -> Result<(), Never> {
        let Ok(showing) = self.watching.showing();
        let kind = showing.kind;
        let Ok(at) = self.at();

        let facts = match kind {
            Kind::Film => {
                let Ok(facts) = self.facts(&at);

                facts
            },
            Kind::Picture => None,
        };

        let whole = self.watching.along.whole;
        let ended = match &self.film {
            Some(film) => {
                let Ok(ended) = film.ended();

                ended
            },
            None => film::Ended::No,
        };

        match ended {
            film::Ended::Yes => {
                self.film = None;

                let Ok(now) = self.now();
                let _ = self.heard(ViewerEvent::Where { at: whole, whole });
                let _ = self.heard(ViewerEvent::Running(now));
            },
            film::Ended::No => {},
        }

        let playing = self.film.as_ref().map(|film| film.asked.clone());
        let watching = &self.watching;
        let wanted = match (kind, watching.running, &facts) {
            (Kind::Film, Running::Yes, Some(_)) => Some(PlaybackRequest {
                film: at.clone(),
                from: 0.0,
                speed: watching.speed,
                captions: watching.captions,
                sought: watching.sought,
            }),
            (Kind::Film, Running::Yes, None)
            | (Kind::Film, Running::Paused, _)
            | (Kind::Picture, _, _) => None,
        };

        let Ok(next) = film::next(playing.as_ref(), wanted.as_ref());
        let heard_at = match &self.film {
            Some(film) => {
                let Ok(at) = film.at();

                Some(at)
            },
            None => None,
        };

        match (next, wanted, facts) {
            (film::Next::None, _, _) => match heard_at {
                Some(heard_at) => {
                    let Ok(at) = toward_zero_u64(heard_at);
                    let _ = self.heard(ViewerEvent::Where { at, whole });
                },
                None => {},
            },
            (film::Next::Stop, _, _) => {
                self.film = None;

                match heard_at {
                    Some(heard_at) => {
                        let Ok(at) = toward_zero_u64(heard_at);
                        let _ = self.heard(ViewerEvent::Where { at, whole });
                    },
                    None => {},
                }
            },
            (film::Next::Start, Some(wanted), Some(facts)) => {
                let same_film = playing.as_ref().is_some_and(|playing| playing.film == wanted.film);
                let moved = playing.as_ref().is_some_and(|playing| playing.sought != wanted.sought);
                let along = self.watching.along;
                let Ok(finished) = along.ended_now();

                let from = match (same_film, moved, heard_at, finished) {
                    (true, false, Some(heard_at), _) => heard_at,
                    (_, _, _, playing::Ended::Yes) => 0.0,
                    (true, true, _, playing::Ended::No)
                    | (true, false, None, playing::Ended::No)
                    | (false, _, _, playing::Ended::No) => {
                        let Ok(from) = along.at.float();

                        from
                    },
                };

                self.film = None;

                let Ok(sound) = self.sound();
                let room = match console_panel::frames::room(&at) {
                    Ok(Some(room)) => room,
                    Ok(None) | Err(_) => Size { width: 1280, height: 720 },
                };
                let Ok(started) = Film::start(PlaybackRequest { from, ..wanted }, &facts, room, sound);

                self.film = Some(started);
            },
            (film::Next::Start, None, _) | (film::Next::Start, Some(_), None) => {},
        }

        Ok(())
    }

    fn heard(&mut self, heard: crate::watching::ViewerEvent) -> Result<Vec<Effect<ViewerEffect>>, Never> {
        let Update { state, effects } = Watched::update(&self.watching, &Event::Custom(heard));

        self.watching = state;

        Ok(effects)
    }
}

type Shared = Arc<Mutex<Looking>>;

use crate::watching::ViewerEvent;

const HERE: &str = ".";


fn press(
    held: &Shared,
    heard: ViewerEvent,
    showing: &dyn console_panel::page::Showing,
) -> Result<(), Never> {
    let effects = match held.lock() {
        Ok(mut looking) => {
            let Ok(effects) = looking.heard(heard);

            effects
        },
        Err(_the_lock_is_poisoned) => Vec::new(),
    };

    for effect in effects {
        match effect {
            Effect::Custom(ViewerEffect::Refresh) => showing.refresh(),
            Effect::Custom(ViewerEffect::TurnToTheCard) => showing.turn_to(CARD),

            Effect::Run(_)
            | Effect::Stream(_)
            | Effect::Prompt(_)
            | Effect::Spawn(_)
            | Effect::Subscribe(_)
            | Effect::Unsubscribe(_)
            | Effect::Write(_)
            | Effect::Notify(_)
            | Effect::Print(_)
            | Effect::Stop(_) => {},
        }
    }

    Ok(())
}

fn shown_in_the_files(
    held: &Shared,
    at: &Path,
) -> Result<impl Fn(&dyn console_panel::page::Showing) -> bool + Send + Sync + 'static, Never> {
    let asking = Arc::clone(held);
    let at = at.to_path_buf();

    Ok(move |_: &dyn console_panel::page::Showing| {
        let effects = match asking.lock() {
            Ok(mut looking) => {
                let Ok(effects) = looking.heard(ViewerEvent::Shown(at.clone()));

                effects
            },
            Err(_the_lock_is_poisoned) => Vec::new(),
        };

        for effect in &effects {
            match effect {
                Effect::Spawn(runs) => {
                    let Ok(whole) = whole(runs);
                    let Ok(()) = console_panel::running::left_running(&whole);
                }

                Effect::Custom(_)
                | Effect::Run(_)
                | Effect::Stream(_)
                | Effect::Prompt(_)
                | Effect::Subscribe(_)
                | Effect::Unsubscribe(_)
                | Effect::Write(_)
                | Effect::Notify(_)
                | Effect::Print(_)
                | Effect::Stop(_) => {},
            }
        }

        true
    })
}

fn whole(runs: &console_program_contract::Command) -> Result<Vec<String>, Never> {
    let Ok(name) = runs.program.name();

    let mut arguments = vec![name.to_string()];

    arguments.extend(runs.arguments.clone());

    Ok(arguments)
}

fn quietly(held: &Shared, heard: ViewerEvent) -> Result<(), Never> {
    match held.lock() {
        Ok(mut looking) => {
            let _ = looking.heard(heard);
        }
        Err(_the_lock_is_poisoned) => {},
    }

    Ok(())
}

fn rows(held: &Shared) -> Result<Vec<Row>, Never> {
    let mut looking = match held.lock() {
        Ok(looking) => looking,
        Err(_the_lock_is_poisoned) => return Ok(Vec::new()),
    };

    let Ok(standing) = looking.at();

    match standing.exists() {
        true => {},
        false => {
            let Ok(listing) = listing(&looking.folder);
            let Ok(at) = looking.now();
            let _ = looking.heard(ViewerEvent::Listed { listing, at });
        }
    }

    let Ok(()) = looking.in_step();

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
    let Ok(opens) = Handler::and_stay(|showing| showing.open_out());
    let Ok(card) = card.choosing(opens);
    let Ok(card) = card.leveled(Arc::new(move |by| {
        let Ok(by) = fitted(by);
        let Ok(()) = walk(&stepping, by);
    }));
    let Ok(card) = card.ended(Ends { less: "", more: "" });
    let Ok(card) = card.chief();
    let mut every = vec![card];

    let Ok(awake) = awake(&looking.watching, now);

    match awake {
        Woken::No => return Ok(every),
        Woken::Yes => {},
    }

    let Ok(where_in) = where_in(&looking.watching);
    let Ok(naming) = Row::naming(&shot.name, Aside(&where_in));
    let Ok(naming) = naming.in_the_middle();

    every.push(naming);

    match shot.kind {
        Kind::Film => {
            let Ok(bar) = bar_row(held, looking.watching.along);

            every.push(bar);
        }
        Kind::Picture => {},
    }

    let facts = match shown.is_some() {
        true => {
            let size = console_core_geometry::Size { width: wide, height: tall };
            let Ok(bytes) = looking.bytes();
            let Ok(said) = saying::under(shot.kind, size, bytes, looking.watching.along);

            said
        }
        false => {
            let Ok(said) = saying::wont_open(&shot.name);

            said
        },
    };

    let Ok(mut every) = what_is_this(every, held, shot.kind, &facts, looking.watching.tracks);
    let Ok(transport) = transport(held, &looking.watching);

    every.push(transport);

    Ok(every)
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
    held: &Shared,
    kind: Kind,
    facts: &str,
    tracks: u32,
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
    held: &Shared,
    showing: &dyn console_panel::page::Showing,
    kind: Kind,
    facts: &str,
    tracks: u32,
) -> Result<(), Never> {
    let speeding = Arc::clone(held);
    let wording = Arc::clone(held);
    let about = facts.to_string();

    match kind {
        Kind::Picture => {
            let editing = Arc::clone(held);
            let mut does: Vec<&str> = editing::EDITS
                .iter()
                .map(|edit| {
                    let Ok(says) = edit.says();

                    says
                })
                .collect();

            does.push(FULL_SCREEN);

            showing.sure(
                "About This Picture",
                Subject(facts),
                &does,
                Arc::new(move |showing, which| {
                    let Ok(which) = console_core_number_conversion::index(which);

                    match editing::EDITS.get(which) {
                        Some(edit) => {
                            let Ok(()) = edited(&editing, showing, *edit);
                        },
                        None => showing.open_out(),
                    }
                }),
            )
        },
        Kind::Film => showing.sure(
            "About This Video",
            Subject(facts),
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

const FULL_SCREEN: &str = "Full Screen";

const ZOOM_IN_FIRST: &str = "Zoom in on what to keep, then choose Crop.";

const NOT_SAVED: &str = "The edited picture couldn't be saved.";

fn edited(held: &Shared, showing: &dyn console_panel::page::Showing, edit: Edit) -> Result<(), Never> {
    let mut looking = match held.lock() {
        Ok(looking) => looking,
        Err(_the_lock_is_poisoned) => return Ok(()),
    };

    let Ok(at) = looking.at();
    let Ok((wide, tall)) = looking.shape();
    let Ok(framed) = showing.framed();

    let region = match (edit, framed) {
        (Edit::Crop, Some(framed)) => match framed.of == at {
            true => {
                let Ok(region) = editing::kept(&framed, Size { width: wide, height: tall });

                region
            },
            false => None,
        },
        (Edit::Crop, None) | (Edit::RotateLeft | Edit::RotateRight | Edit::Flip, _) => None,
    };

    let Ok(filter) = editing::filter(edit, region);

    let filter = match filter {
        Filter::Is(filter) => filter,
        Filter::ZoomInFirst => {
            showing.note(ZOOM_IN_FIRST);

            return Ok(());
        },
    };

    let into = match editing::saved(&at, &filter) {
        Ok(into) => into,
        Err(why) => {
            eprintln!("viewer: {why}");
            showing.note(NOT_SAVED);

            return Ok(());
        },
    };

    let Ok(found) = Looking::of(&into);

    match found {
        Some(found) => {
            looking.folder = found.folder;
            looking.watching = found.watching;
        },
        None => {},
    }

    showing.refresh();

    Ok(())
}

fn how_fast(
    held: &Shared,
    showing: &dyn console_panel::page::Showing,
    name: &str,
) -> Result<(), Never> {
    let setting = Arc::clone(held);
    let says: Vec<&str> = playing::SPEEDS.iter().map(|(says, _)| *says).collect();

    showing.sure(
        "Playback Speed",
        Subject(name),
        &says,
        Arc::new(move |_, which| {
            let Ok(which) = console_core_number_conversion::fitted(which);
            let Ok(at) = now(&setting);
            let Ok(()) = quietly(&setting, ViewerEvent::Speed { which, at });
        }),
    );

    Ok(())
}

fn which_words(
    held: &Shared,
    showing: &dyn console_panel::page::Showing,
    name: &str,
    tracks: u32,
) -> Result<(), Never> {
    let setting = Arc::clone(held);
    let Ok(said) = playing::captions(tracks);
    let says: Vec<&str> = said.iter().map(String::as_str).collect();

    showing.sure(
        "Subtitles",
        Subject(name),
        &says,
        Arc::new(move |_, which| {
            let Ok(which) = console_core_number_conversion::fitted(which);
            let Ok(at) = now(&setting);
            let Ok(()) = quietly(&setting, ViewerEvent::Text { which, at });
        }),
    );

    Ok(())
}

fn transport(held: &Shared, watching: &Watching) -> Result<Row, Never> {
    let running = watching.running;
    let Ok(only) = alone(watching);
    let Ok(showing) = watching.showing();
    let kind = showing.kind;
    let name = showing.name.clone();
    let tracks = watching.tracks;

    let mut presses = Vec::new();

    let Ok(()) = walking(held, only, Icon::Previous, -1, &mut presses);

    let at = match kind {
        Kind::Film => {
            let Ok(skip) = fitted::<u64, i32>(playing::SKIP.saturating_div(playing::STEP));
            let Ok(back) = skipping(held, Icon::Rewind, skip.saturating_neg());
            let (on, wording) = (Arc::clone(held), Arc::clone(held));
            let Ok(icon) = running.icon();
            let Ok(running) = ButtonPress::new(icon, Active::No, move |showing| {
                let Ok(at) = now(&on);
                let Ok(()) = press(&on, ViewerEvent::Running(at), showing);
            });
            let Ok(running) = running.chief();
            let Ok(forward) = skipping(held, Icon::FastForward, skip);
            let Ok(words) = ButtonPress::new(Icon::Subtitles, Active::No, move |showing| {
                let Ok(()) = which_words(&wording, showing, &name, tracks);
            });

            presses.push(back);

            let Ok(at) = fitted::<_, u32>(presses.len());

            presses.push(running);
            presses.push(forward);

            let Ok(()) = walking(held, only, Icon::Next, 1, &mut presses);

            presses.push(words);

            at
        }
        Kind::Picture => {
            let Ok(further) = zooming(Icon::ZoomOut, Closer::No);
            let Ok(closer) = zooming(Icon::ZoomIn, Closer::Yes);

            presses.push(further);

            let Ok(at) = fitted::<_, u32>(presses.len());

            presses.push(closer);

            let Ok(()) = walking(held, only, Icon::Next, 1, &mut presses);

            at
        }
    };

    let Ok(whole) = ButtonPress::new(Icon::FullScreen, Active::No, |showing| showing.toggle_full_screen());
    let Ok(whole) = whole.cornered();

    presses.push(whole);

    Row::pressing(presses, at)
}

fn walking(held: &Shared, only: Alone, icon: Icon, by: i32, presses: &mut Vec<ButtonPress>) -> Result<(), Never> {
    match only {
        Alone::Yes => {},
        Alone::No => {
            let stepping = Arc::clone(held);
            let Ok(walk) = ButtonPress::new(icon, Active::No, move |showing| {
                let Ok(()) = walk(&stepping, by);

                showing.refresh();
            });

            presses.push(walk);
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Closer {
    Yes,
    No,
}

fn zooming(icon: Icon, closer: Closer) -> Result<ButtonPress, Never> {
    let factor = match closer {
        Closer::Yes => console_panel::zoom::STEP,
        Closer::No => 1.0 / console_panel::zoom::STEP,
    };

    ButtonPress::new(icon, Active::No, move |showing| showing.zoom_by(factor))
}

fn skipping(held: &Shared, icon: Icon, by: i32) -> Result<ButtonPress, Never> {
    let skipping = Arc::clone(held);

    ButtonPress::new(icon, Active::No, move |showing| {
        let Ok(()) = scrub(&skipping, by);

        showing.refresh();
    })
}

fn bar_row(held: &Shared, along: playing::Along) -> Result<Row, Never> {
    let stepping = Arc::clone(held);
    let tapped = Arc::clone(held);

    let whole = match along.whole > 0 {
        true => {
            let Ok(clock) = positional(Duration::from_secs(along.whole));

            clock
        },
        false => String::new(),
    };

    let Ok(at) = positional(Duration::from_secs(along.at));
    let Ok(nothing) = Handler::and_stay(|_| {});
    let Ok(row) = Row::new(&at, Aside(&whole), nothing);
    let Ok(row) = row.picturing(Picture::Bar(Bar { at: along.at, of: along.whole }));
    let Ok(row) = row.ended(Ends { less: "", more: "" });
    let Ok(row) = row.leveled(Arc::new(move |by| {
        let Ok(()) = scrub(&stepping, by);
    }));

    row.seeking(move |showing, fraction| {
        let Ok(()) = seek_to(&tapped, fraction);

        showing.refresh();
    })
}

fn now(held: &Shared) -> Result<Since, Never> {
    Ok(match held.lock() {
        Ok(looking) => {
            let Ok(now) = looking.now();

            now
        },
        Err(_the_lock_is_poisoned) => Since::ZERO,
    })
}

fn seek_to(held: &Shared, fraction: f64) -> Result<(), Never> {
    let Ok(at) = now(held);

    quietly(held, ViewerEvent::SoughtTo { fraction, at })
}

fn scrub(held: &Shared, by: i32) -> Result<(), Never> {
    let Ok(at) = now(held);

    quietly(held, ViewerEvent::Scrubbed { by, at })
}

fn walk(held: &Shared, by: i32) -> Result<(), Never> {
    let Ok(at) = now(held);

    quietly(held, ViewerEvent::Stepped { by, at })
}

const MEDIA: [Folder; 3] = [Folder::Pictures, Folder::Videos, Folder::Downloads];

const CARD: u32 = 0;

fn media_folders(here: &Path) -> Result<Vec<PathBuf>, Never> {
    let mut folders: Vec<PathBuf> = MEDIA
        .into_iter()
        .filter_map(|folder| {
            let Ok(hers) = folder.hers();

            hers
        })
        .filter(|at| at.is_dir())
        .collect();

    folders.push(here.to_path_buf());

    index::kept(&folders)
}

fn read_for_the_index(at: &Path) -> Result<Vec<index::Read>, Never> {
    let Ok(kinds) = kinds();

    Ok(std::fs::read_dir(at)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = match entry.file_name().into_string() {
                Ok(name) => name,
                Err(_not_text) => return None,
            };
            let path = entry.path();
            let folder = path.is_dir();

            let mime = match folder {
                true => String::new(),
                false => {
                    let Ok(mime) = console_content_types::of(&kinds, &path);

                    mime
                },
            };

            Some(index::Read { name, path, folder, mime })
        })
        .collect())
}

fn indexed(held: &Shared) -> Result<Vec<index::Found>, Never> {
    let here = match held.lock() {
        Ok(looking) => looking.folder.clone(),
        Err(_the_lock_is_poisoned) => return Ok(Vec::new()),
    };

    let Ok(folders) = media_folders(&here);

    index::under(&folders, &read_for_the_index)
}

fn media_rows(held: &Shared) -> Result<Vec<Row>, Never> {
    let standing = match held.lock() {
        Ok(looking) => looking.folder.join(&{
            let Ok(showing) = looking.watching.reel.showing();

            showing.name.clone()
        }),
        Err(_the_lock_is_poisoned) => PathBuf::new(),
    };

    let Ok(found) = indexed(held);

    let mut rows: Vec<Row> = Vec::new();

    for thing in found {
        let going = Arc::clone(held);
        let at = thing.path.clone();
        let picture = match thing.kind {
            Kind::Picture => Picture::At(thing.path.clone()),
            Kind::Film => Picture::Named(Icon::Film),
        };

        let aside = match thing.path == standing {
            true => console_panel::page::NOW,
            false => "",
        };

        let Ok(stands) = Handler::and_stay(move |showing| {
            let Ok(()) = looked_at(&going, &at, showing);
        });
        let Ok(row) = Row::new(&thing.name, Aside(aside), stands);
        let Ok(row) = row.picturing(picture);
        let Ok(row) = row.selectable(&thing.path.to_string_lossy());
        let Ok(shows) = shown_in_the_files(held, &thing.path);
        let Ok(offering) = console_panel::page::shown_or_selected(&thing.name, &thing.path, move |showing| {
            let _ = shows(showing);
        });
        let Ok(row) = row.offering(offering);

        rows.push(row);
    }

    console_panel::page::lettered(rows)
}

fn looked_at(held: &Shared, at: &Path, showing: &dyn console_panel::page::Showing) -> Result<(), Never> {
    let here = match held.lock() {
        Ok(looking) => looking.folder.clone(),
        Err(_the_lock_is_poisoned) => return Ok(()),
    };

    let name = match at.file_name().and_then(|name| name.to_str()) {
        Some(name) => name.to_string(),
        None => String::new(),
    };

    match at.parent() == Some(here.as_path()) {
        true => {
            let Ok(now) = now(held);

            return press(held, ViewerEvent::StoodOn { name, at: now }, showing);
        },
        false => {},
    }

    let found = match Looking::of(at) {
        Ok(Some(found)) => found,
        Ok(None) | Err(_) => return Ok(()),
    };

    match held.lock() {
        Ok(mut looking) => {
            looking.folder = found.folder;
            looking.watching = found.watching;
        },
        Err(_the_lock_is_poisoned) => return Ok(()),
    }

    showing.turn_to(CARD);
    showing.refresh();

    Ok(())
}

fn stir(held: &Shared) -> Result<WakeOutcome, Never> {
    let mut looking = match held.lock() {
        Ok(looking) => looking,
        Err(_the_lock_is_poisoned) => return Ok(WakeOutcome::AlreadyAwake),
    };

    let Ok(at) = looking.now();
    let Ok(was) = stirred(&looking.watching, at);
    let _ = looking.heard(ViewerEvent::WakeOutcome(at));

    Ok(match was {
        crate::watching::WakeOutcome::AlreadyAwake => WakeOutcome::AlreadyAwake,
        crate::watching::WakeOutcome::Woke => WakeOutcome::Woke,
    })
}

fn pages(held: &Shared) -> Result<Vec<Page>, Never> {
    let drawing = Arc::clone(held);
    let stirring = Arc::clone(held);
    let listing = Arc::clone(held);
    let Ok(asked) = Rows::asked(move || {
        let Ok(rows) = rows(&drawing);

        rows
    });
    let Ok(page) = Page::new("Viewing", asked);
    let Ok(looking) = page.stirring(move || {
        let Ok(stirred) = stir(&stirring);

        stirred
    });
    let Ok(asked) = Rows::asked(move || {
        let Ok(rows) = media_rows(&listing);

        rows
    });
    let Ok(media) = Page::new("Media", asked);
    let Ok(media) = media.trashing_selected();

    Ok(vec![looking, media])
}

fn by_default() -> Result<Option<PathBuf>, Never> {
    Ok(MEDIA
        .into_iter()
        .filter_map(|folder| {
            let Ok(hers) = folder.hers();

            hers
        })
        .find(|folder| folder.is_dir()))
}

fn standing_on(asked: &Path) -> Result<Option<Looking>, Never> {
    let Ok(here) = Looking::of(asked);

    match here {
        Some(looking) => return Ok(Some(looking)),
        None => {},
    }

    let Ok(folders) = media_folders(asked);
    let Ok(found) = index::under(&folders, &read_for_the_index);

    let first = match found.first() {
        Some(first) => first.path.clone(),
        None => return Ok(None),
    };

    Looking::of(&first)
}


pub const WHO: &str = "viewer";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worth {
    Opening,
    None,
}

pub fn worth_opening(arguments: &[String]) -> Result<Worth, Never> {
    let Ok(asked) = asked_for(arguments);

    let asked = match asked {
        Some(asked) => asked,
        None => return Ok(Worth::Opening),
    };

    match asked.is_dir() {
        true => return Ok(Worth::Opening),
        false => {},
    }

    let Ok(found) = Looking::of(&asked);

    match found {
        Some(_) => Ok(Worth::Opening),
        None => {
            eprintln!("viewer: {}: this is neither a picture nor a film", asked.display());

            Ok(Worth::None)
        }
    }
}

fn asked_for(arguments: &[String]) -> Result<Option<PathBuf>, Never> {
    let said = arguments.first().filter(|said| !said.is_empty()).map(PathBuf::from);

    match said {
        Some(said) => Ok(Some(said)),
        None => {
            let Ok(folder) = by_default();

            let folder = match folder {
                Some(folder) => folder,
                None => return Ok(None),
            };

            Ok(Some(folder))
        }
    }
}

const NOTHING_HERE: &str = "No Pictures or Videos";

const NO_FOLDER: &str = "No Pictures Folder";

fn saying_only(says: &'static str) -> Result<Card, Never> {
    Card::new(Arc::new(move || {
        let Ok(asked) = Rows::asked(move || {
            let Ok(row) = Row::nothing(says);

            vec![row]
        });
        let Ok(page) = Page::new("Media", asked);

        vec![page]
    }))
}

pub fn card(arguments: &[String]) -> Result<Card, Never> {
    let Ok(asked) = asked_for(arguments);

    let asked = match asked {
        Some(asked) => asked,
        None => return saying_only(NO_FOLDER),
    };

    let Ok(found) = standing_on(&asked);

    let looking = match found {
        Some(looking) => looking,
        None => return saying_only(NOTHING_HERE),
    };

    let held: Shared = Arc::new(Mutex::new(looking));
    let drawing = Arc::clone(&held);

    let Ok(card) = Card::new(Arc::new(move || {
        let Ok(pages) = pages(&drawing);

        pages
    }));

    card.shutting(Box::new(|| Ok(())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_folder_with_a_film() -> PathBuf {
        let folder = std::env::temp_dir().join(format!("console-viewer-card-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&folder);
        let Ok(mut making) = console_core_external_programs::Program::Ffmpeg.command();
        let made = making
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", "testsrc=s=64x48:r=25:d=4"])
            .arg(folder.join("silent.mkv"))
            .status();

        assert!(made.is_ok_and(|how| how.success()), "ffmpeg made no film to play");

        folder
    }

    #[test]
    fn play_starts_the_film_its_clock_moves_on_and_pause_stops_it_where_it_was() {
        let folder = a_folder_with_a_film();
        let film = folder.join("silent.mkv");
        let mut looking = match Looking::of(&film) {
            Ok(Some(looking)) => looking,
            Ok(None) | Err(_) => panic!("a folder with a film in it opens on the film"),
        };

        let Ok(()) = looking.in_step();

        assert!(looking.film.is_none(), "a film opens stopped");
        assert_eq!(looking.watching.along.whole, 4, "the length of the film is known before it plays");

        let _ = looking.heard(ViewerEvent::Running(Since::ZERO));
        let Ok(()) = looking.in_step();

        assert!(looking.film.is_some(), "play started nothing");

        let patience = console_waiting::Schedule::of(std::time::Duration::from_secs(20)).expect("a patience");
        let Ok(moved) = console_waiting::until(patience, || {
            let Ok(()) = looking.in_step();

            Ok(match looking.watching.along.at >= 1 {
                true => console_waiting::Ready::Yes,
                false => console_waiting::Ready::NotYet,
            })
        });

        assert_eq!(moved, console_waiting::Outcome::Happened, "the clock under the film never moved");

        let _ = looking.heard(ViewerEvent::Running(Since::ZERO));
        let Ok(()) = looking.in_step();
        let stopped_at = looking.watching.along.at;

        assert!(looking.film.is_none(), "pause left the film playing");
        assert!(stopped_at >= 1, "pause lost the place: {stopped_at}");

        let _ = looking.heard(ViewerEvent::Running(Since::ZERO));
        let Ok(()) = looking.in_step();
        let from = looking.film.as_ref().map(|film| film.at());

        let _ = std::fs::remove_dir_all(&folder);

        match from {
            Some(Ok(from)) => assert!(from >= 1.0, "play again started from the beginning: {from}"),
            Some(Err(_)) | None => panic!("play again started nothing"),
        }
    }
}
