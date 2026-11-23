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
//! is a `console_core_state_machine::Machine`.
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

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvError, SyncSender, TrySendError, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::editing::{self, Edit, EditError, Planned};
use crate::film::{self, Facts, Film, PlaybackRequest};
use crate::index;
use crate::kinds::Kind;
use crate::paths::{self, Name, Status};
use crate::playing::Running;
use crate::reel::Reel;
use crate::units::Fraction;
use crate::waking::Wake;
use crate::watching::{
  Alone, Since, ViewerEffect, Watch as Watched, Watching, alone, awake, stirred,
};
use crate::{playing, saying};
use console_content_types::Table;
use console_core_geometry::Size;
use console_core_localization::positional;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_number_conversion::{Float, toward_zero_u64};
use console_core_places::Folder;
use console_core_state_machine::{Machine, Transition};
use console_core_subtitle_files::Seconds;
use console_music_player::sounding::{Progress, Sounding};
use console_panel::card::Card;
use console_panel::icons::Icon;
use console_panel::page::{
  ActionOutcome, Active, Aside, Bar, ButtonPress, Ends, Handler, Page, Picture, Row, Rows, Set,
  Subject, WakeOutcome,
};
use console_program_contract::{Effect, Event};

fn kinds() -> Result<Table, Never> {
  Ok(match Table::here() {
    Ok(table) => table,
    Err(why) => {
      console_core_logging::warn!("nothing says what a file is: {why}");

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
    .filter(|entry| paths::status(&entry.path()) != Ok(Status::Directory))
    .filter_map(|entry| {
      let name = match entry.file_name().into_string() {
        Ok(name) => name,
        Err(_not_text) => {
          console_core_logging::warn!("{:?}: this name is not text", entry.file_name());

          return None;
        }
      };

      match ::core::primitive::bool::then_some(name.starts_with('.'), ()) {
        ::core::option::Option::Some(()) => None,
        ::core::option::Option::None => {
          let Ok(kind) = console_content_types::of(&kinds, &entry.path());

          Some((name, kind))
        }
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
  sound: Option<Sounding>,
  measured: Option<(PathBuf, (u32, u32))>,
}

impl Looking {
  fn of(asked: &Path) -> Result<Option<Looking>, Never> {
    let (folder, opened) =
      match ::core::primitive::bool::then_some(paths::status(asked) == Ok(Status::Directory), ())
      {
        ::core::option::Option::Some(()) => (asked.to_path_buf(), String::new()),
        ::core::option::Option::None => {
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
    let Ok(showing) = self.watching.current();

    let Ok(name) = Name::new(OsStr::new(&showing.name));

    Ok(match name {
      Some(name_2) => self.folder.join(name_2),
      None => self.folder.clone(),
    })
  }

  fn bytes(&self) -> Result<u64, Never> {
    let Ok(at) = self.at();

    Ok(match std::fs::metadata(&at) {
      Ok(held) => held.len(),
      Err(fault) => {
        console_core_logging::warn!("{}: {fault}", at.display());

        0
      }
    })
  }

  fn shape(&mut self) -> Result<(u32, u32), Never> {
    let Ok(showing) = self.watching.current();
    let kind = showing.kind;
    let Ok(at) = self.at();

    match &self.measured {
      Some((was, shape)) => match ::core::primitive::bool::then_some(*was == at, ()) {
        ::core::option::Option::Some(()) => return Ok(*shape),
        ::core::option::Option::None => {}
      },
      None => {}
    }

    let shape = match kind {
      Kind::Film => (0, 0),
      Kind::Picture => match console_pictures::measure(&at) {
        Ok(Some(Size {
          width: wide,
          height: tall,
        })) => (wide, tall),
        Ok(None) => (0, 0),
        Err(why) => {
          console_core_logging::warn!("{}: {why}", at.display());

          (0, 0)
        }
      },
    };

    self.measured = Some((at, shape));

    Ok(shape)
  }

  fn facts(&mut self, at: &Path) -> Result<Option<Facts>, Never> {
    match &self.facts {
      Some((was, facts)) => match ::core::primitive::bool::then_some(was.as_path() == at, ()) {
        ::core::option::Option::Some(()) => return Ok(facts.clone()),
        ::core::option::Option::None => {}
      },
      None => {}
    }

    let Ok(facts) = film::facts(at);

    self.facts = Some((at.to_path_buf(), facts.clone()));

    match &facts {
      Some(facts_2) => {
        let Ok(whole) = toward_zero_u64(facts_2.seconds.0.ceil());
        let Ok(tracks) = film::sidecars(at);
        let at_2 = self.watching.along.at;
        let Ok(beside) = fitted::<_, u32>(tracks.len());
        let ::core::result::Result::Ok(_) =
          self.handle_event(ViewerEvent::Where { at: at_2, whole });
        let ::core::result::Result::Ok(_) =
          self.handle_event(ViewerEvent::Tracks(facts_2.words.saturating_add(beside)));
      }
      None => {}
    }

    Ok(facts)
  }

  fn sound(&mut self) -> Result<Sounding, Never> {
    match self.sound.take() {
      Some(sound) => Ok(sound),
      None => Sounding::new(|progress| match progress {
        Progress::Ended => console_panel::frames::tell(console_panel::frames::Notice::Rows),
        Progress::ASecondPlayed => Ok(()),
      }),
    }
  }

  fn stop(&mut self) -> Result<(), Never> {
    match self.film.take() {
      Some(film) => {
        let Ok(sound) = film.stop();
        self.sound = sound;
      }
      None => {}
    }

    Ok(())
  }

  fn in_step(&mut self) -> Result<(), Never> {
    let Ok(showing) = self.watching.current();
    let kind = showing.kind;
    let Ok(at) = self.at();

    let facts = match kind {
      Kind::Film => {
        let Ok(facts) = self.facts(&at);

        facts
      }
      Kind::Picture => None,
    };

    let whole = self.watching.along.whole;
    let ended = match &self.film {
      Some(film) => {
        let Ok(ended) = film.ended();

        ended
      }
      None => film::Ended::No,
    };

    match ended {
      film::Ended::Yes => {
        let Ok(()) = self.stop();

        let Ok(now) = self.now();
        let ::core::result::Result::Ok(_) =
          self.handle_event(ViewerEvent::Where { at: whole, whole });
        let ::core::result::Result::Ok(_) = self.handle_event(ViewerEvent::Running(now));
      }
      film::Ended::No => {}
    }

    let playing = self.film.as_ref().map(|film| film.asked.clone());
    let watching = &self.watching;
    let wanted = match (kind, watching.running, &facts) {
      (Kind::Film, Running::Yes, Some(_)) => Some(PlaybackRequest {
        film: at.clone(),
        from: Seconds(0.0),
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
        let Ok(at_2) = film.at();

        Some(at_2)
      }
      None => None,
    };

    match (next, wanted, facts) {
      (film::Next::None, _, _) => match heard_at {
        Some(heard_at_2) => {
          let Ok(at_3) = toward_zero_u64(heard_at_2.0);
          let ::core::result::Result::Ok(_) =
            self.handle_event(ViewerEvent::Where { at: at_3, whole });
        }
        None => {}
      },
      (film::Next::Stop, _, _) => {
        let Ok(()) = self.stop();

        match heard_at {
          Some(heard_at_3) => {
            let Ok(at_4) = toward_zero_u64(heard_at_3.0);
            let ::core::result::Result::Ok(_) =
              self.handle_event(ViewerEvent::Where { at: at_4, whole });
          }
          None => {}
        }
      }
      (film::Next::Start, Some(wanted_2), Some(facts_2)) => {
        let same_film = playing
          .as_ref()
          .is_some_and(|playing_2| playing_2.film == wanted_2.film);
        let moved = playing
          .as_ref()
          .is_some_and(|playing_3| playing_3.sought != wanted_2.sought);
        let along = self.watching.along;
        let Ok(finished) = along.ended_now();

        let from = match (same_film, moved, heard_at, finished) {
          (true, false, Some(heard_at_4), _) => heard_at_4,
          (_, _, _, playing::Ended::Yes) => Seconds(0.0),
          (true, true, _, playing::Ended::No)
          | (true, false, None, playing::Ended::No)
          | (false, _, _, playing::Ended::No) => {
            let Ok(from) = along.at.float();

            Seconds(from)
          }
        };

        let Ok(()) = self.stop();

        let Ok(sound) = self.sound();
        let room = match console_panel::frames::room(&at) {
          Ok(Some(room)) => room,
          Ok(None) | Err(_) => Size {
            width: 1280,
            height: 720,
          },
        };
        let Ok(started) =
          Film::start(PlaybackRequest { from, ..wanted_2 }, &facts_2, room, sound);

        self.film = Some(started);
      }
      (film::Next::Start, None, _) | (film::Next::Start, Some(_), None) => {}
    }

    Ok(())
  }

  fn answer(&mut self, request: Request) -> Result<Reply, Never> {
    Ok(match request {
      Request::Event(heard) => {
        let Ok(effects) = self.handle_event(heard);
        Reply::Effects(effects)
      }
      Request::View => {
        let Ok(view) = self.view();
        Reply::View(Box::new(view))
      }
      Request::Location => {
        let Ok(at) = self.at();
        Reply::Location {
          folder: self.folder.clone(),
          at,
        }
      }
      Request::Time => {
        let Ok(time) = self.now();
        Reply::Time(time)
      }
      Request::Edit(edit, framed) => {
        let Ok(outcome) = self.edit(edit, framed);
        Reply::Edited(outcome)
      }
      Request::Open(at) => {
        let Ok(effects) = self.looked_at(&at);
        Reply::Effects(effects)
      }
      Request::Wake => {
        let Ok(at) = self.now();
        let Ok(was) = stirred(&self.watching, at);
        let Ok(_effects) = self.handle_event(ViewerEvent::WakeOutcome(at));

        Reply::WakeResult(match was {
          crate::watching::WakeOutcome::AlreadyAwake => WakeOutcome::AlreadyAwake,
          crate::watching::WakeOutcome::Woke => WakeOutcome::Woke,
        })
      }
    })
  }

  fn view(&mut self) -> Result<View, Never> {
    let Ok(standing) = self.at();

    match std::fs::metadata(&standing) {
      Ok(_metadata) => {}
      Err(_file_unavailable) => {
        let Ok(listing) = listing(&self.folder);
        let Ok(at) = self.now();
        let Ok(_effects) = self.handle_event(ViewerEvent::Listed { listing, at });
      }
    }

    let Ok(()) = self.in_step();
    let Ok(now) = self.now();
    let Ok(showing) = self.watching.current();
    let shot = showing.clone();
    let Ok(at) = self.at();
    let path = match std::fs::metadata(&at) {
      Ok(_metadata) => Some(at),
      Err(_file_unavailable) => None,
    };
    let Ok(awake) = awake(&self.watching, now);
    let Ok(only) = alone(&self.watching);
    let Ok(where_in) = where_in(&self.watching);
    let Ok(details) = match (awake, &path) {
      (Wake::No, _) => Ok(String::new()),
      (Wake::Yes, Some(_path)) => {
        let Ok((width, height)) = self.shape();
        let Ok(bytes) = self.bytes();

        saying::under(
          shot.kind,
          Size { width, height },
          bytes,
          self.watching.along,
        )
      }
      (Wake::Yes, None) => saying::wont_open(&shot.name),
    };

    Ok(View {
      shot,
      path,
      awake,
      where_in,
      details,
      only,
      running: self.watching.running,
      along: self.watching.along,
      tracks: self.watching.tracks,
    })
  }

  fn looked_at(&mut self, at: &Path) -> Result<Vec<Effect<ViewerEffect>>, Never> {
    let nearby = (at.parent() == Some(self.folder.as_path())).then_some(());

    match (nearby, at.file_name().and_then(|name| name.to_str())) {
      (Some(()), Some(name)) => {
        let Ok(now) = self.now();
        return self.handle_event(ViewerEvent::StoodOn {
          name: name.to_owned(),
          at: now,
        });
      }
      (Some(()), None) | (None, _) => {}
    }

    let Ok(found) = Looking::of(at);

    match found {
      Some(looking) => {
        self.folder = looking.folder;
        self.watching = looking.watching;

        Ok(vec![
          Effect::Custom(ViewerEffect::TurnToTheCard),
          Effect::Custom(ViewerEffect::Refresh),
        ])
      }
      None => Ok(Vec::new()),
    }
  }

  fn edit(
    &mut self,
    edit: Edit,
    framed: Option<console_panel::zoom::Framed>,
  ) -> Result<EditReply, Never> {
    let Ok(at) = self.at();
    let Ok((wide, tall)) = self.shape();

    let region = match (edit, framed) {
      (Edit::Crop, Some(framed_2)) => {
        match ::core::primitive::bool::then_some(framed_2.of == at, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(region) = editing::crop_region(
              &framed_2,
              Size {
                width: wide,
                height: tall,
              },
            );

            region
          }
          ::core::option::Option::None => None,
        }
      }
      (Edit::Crop, None) | (Edit::RotateLeft | Edit::RotateRight | Edit::Flip, _) => None,
    };

    let Ok(planned) = editing::planned(edit, region);

    let change = match planned {
      Planned::Change(change) => change,
      Planned::ZoomInFirst => {
        return Ok(EditReply::Note(ZOOM_IN_FIRST.to_owned()));
      }
    };

    let into = match editing::saved(&at, change) {
      Ok(into) => into,
      Err(why) => {
        console_core_logging::warn!("{why}");

        let Ok(told) = unsaved(&why);

        return Ok(EditReply::Note(told));
      }
    };

    let Ok(found) = Looking::of(&into);

    match found {
      Some(found_2) => {
        self.folder = found_2.folder;
        self.watching = found_2.watching;
      }
      None => {}
    }

    Ok(EditReply::Saved)
  }

  fn handle_event(
    &mut self,
    heard: crate::watching::ViewerEvent,
  ) -> Result<Vec<Effect<ViewerEffect>>, Never> {
    let Ok(Transition { state, effects }) =
      Watched::transition(self.watching.clone(), Event::Custom(heard));

    self.watching = state;

    Ok(effects)
  }
}

type Shared = SyncSender<Mail>;
use crate::watching::ViewerEvent;

const HERE: &str = ".";

enum Request {
  Event(ViewerEvent),
  View,
  Location,
  Time,
  Edit(Edit, Option<console_panel::zoom::Framed>),
  Open(PathBuf),
  Wake,
}

enum Reply {
  Effects(Vec<Effect<ViewerEffect>>),
  View(Box<View>),
  Location { folder: PathBuf, at: PathBuf },
  Time(Since),
  Edited(EditReply),
  WakeResult(WakeOutcome),
}

enum EditReply {
  Saved,
  Note(String),
}

struct View {
  shot: crate::reel::Shot,
  path: Option<PathBuf>,
  awake: Wake,
  where_in: String,
  details: String,
  only: Alone,
  running: Running,
  along: playing::Along,
  tracks: u32,
}

enum Mail {
  Request(Request, SyncSender<Reply>),
  Close,
}

struct Owner {
  address: Shared,
  thread: Option<JoinHandle<()>>,
}

impl Owner {
  fn start(looking: Looking) -> std::io::Result<Owner> {
    let (address, received) = sync_channel(16);
    let thread = std::thread::Builder::new()
      .name("viewer state".to_owned())
      .spawn(move || {
        let Ok(()) = requested(looking, received);
      })?;

    Ok(Owner {
      address,
      thread: Some(thread),
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "the owner delivers its terminal request and joins before the card closes so the film is stopped and its audio and frames are released"
    )
  )]
  fn close(&mut self) -> Result<(), Never> {
    let thread = match self.thread.take() {
      Some(thread) => thread,
      None => return Ok(()),
    };
    let _already_closed = self.address.send(Mail::Close);

    match thread.join() {
      Ok(()) => {}
      Err(_panicked) => {
        console_core_logging::warn!("the media viewer worker did not close normally")
      }
    }

    Ok(())
  }
}

impl Drop for Owner {
  fn drop(&mut self) {
    let Ok(()) = self.close();
  }
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the dedicated viewer worker opts out of polling and its owner can deliver Close or disconnect the mailbox"
  )
)]
fn requested(looking: Looking, received: Receiver<Mail>) -> Result<(), Never> {
  use console_core_iteration::{Step, iterate};

  let processed = iterate((looking, received), |(mut viewer, inbox)| {
    Ok(match inbox.recv() {
      Ok(Mail::Request(request, answering)) => {
        let Ok(answer) = viewer.answer(request);
        let _requester_closed = answering.try_send(answer);

        Step::Again((viewer, inbox))
      }
      Ok(Mail::Close) | Err(RecvError) => Step::Halt(()),
    })
  });

  match processed {
    Ok(()) => Ok(()),
    Err(_endless) => Ok(()),
  }
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "a queued viewer operation owns the single reply sender, which can only complete or disconnect, and an accepted edit must report its result"
  )
)]
fn ask(held: &Shared, request: Request) -> Result<Option<Reply>, Never> {
  let (answer, answered) = sync_channel(1);

  match held.try_send(Mail::Request(request, answer)) {
    Ok(()) => {}
    Err(TrySendError::Full(_request)) => {
      console_core_logging::warn!("the viewer is still handling an earlier request");

      return Ok(None);
    }
    Err(TrySendError::Disconnected(_request)) => return Ok(None),
  }

  Ok(match answered.recv() {
    Ok(reply) => Some(reply),
    Err(RecvError) => None,
  })
}

fn press(
  held: &Shared,
  heard: ViewerEvent,
  showing: &dyn console_panel::page::Showing,
) -> Result<(), Never> {
  let Ok(effects) = changed(held, Request::Event(heard));

  perform(effects, showing)
}

fn changed(held: &Shared, request: Request) -> Result<Vec<Effect<ViewerEffect>>, Never> {
  let Ok(reply) = ask(held, request);

  Ok(match reply {
    Some(Reply::Effects(effects)) => effects,
    Some(_) | None => Vec::new(),
  })
}

fn perform(
  effects: Vec<Effect<ViewerEffect>>,
  showing: &dyn console_panel::page::Showing,
) -> Result<(), Never> {
  for effect in effects {
    match effect {
      Effect::Custom(ViewerEffect::Refresh) => showing.refresh(),
      Effect::Custom(ViewerEffect::TurnToTheCard) => showing.turn_to(CARD),
      Effect::Spawn(runs) => {
        let Ok(whole) = whole(&runs);
        let Ok(()) = console_panel::running::left_running(&whole);
      }
      Effect::Run(_)
      | Effect::Stream(_)
      | Effect::Fork(_)
      | Effect::Interrupt(_)
      | Effect::Prompt(_)
      | Effect::Subscribe(_)
      | Effect::Unsubscribe(_)
      | Effect::Write(_)
      | Effect::Notify(_)
      | Effect::Print(_)
      | Effect::Stop(_) => {}
    }
  }

  Ok(())
}

fn shown_in_the_files(
  held: &Shared,
  at: &Path,
) -> Result<impl Fn(&dyn console_panel::page::Showing) + Send + Sync + 'static, Never> {
  let asking = held.clone();
  let path = at.to_path_buf();

  Ok(move |showing: &dyn console_panel::page::Showing| {
    let Ok(()) = press(&asking, ViewerEvent::Shown(path.clone()), showing);
  })
}

fn whole(runs: &console_program_contract::Command) -> Result<Vec<String>, Never> {
  let Ok(name) = runs.program.name();

  let mut arguments = vec![name.to_string()];
  arguments.extend(runs.arguments.clone());

  Ok(arguments)
}

fn quietly(held: &Shared, heard: ViewerEvent) -> Result<(), Never> {
  let Ok(_effects) = changed(held, Request::Event(heard));

  Ok(())
}

fn rows(held: &Shared) -> Result<Vec<Row>, Never> {
  let Ok(reply) = ask(held, Request::View);
  let view = match reply {
    Some(Reply::View(view)) => view,
    Some(_) | None => return Ok(Vec::new()),
  };
  let stepping = held.clone();
  let Ok(card) = match view.shot.kind {
    Kind::Picture => Row::picture(Picture::Showing(view.path.clone())),
    Kind::Film => Row::picture(Picture::Playing(view.path.clone())),
  };
  let Ok(opens) = Handler::and_stay(|showing| showing.open_out());
  let Ok(chooses) = card.choosing(opens);
  let Ok(adjusted) = chooses.adjusting(move |by| {
    let Ok(by_2) = fitted(by);
    let Ok(()) = walk(&stepping, by_2);
  });
  let Ok(ended) = adjusted.ended(Ends { less: "", more: "" });
  let Ok(chief) = ended.chief();
  let mut every = vec![chief];

  match view.awake {
    Wake::No => return Ok(every),
    Wake::Yes => {}
  }

  let Ok(naming) = Row::naming(&view.shot.name, Aside(&view.where_in));
  let Ok(centered) = naming.in_the_middle();
  every.push(centered);

  match view.shot.kind {
    Kind::Film => {
      let Ok(bar) = bar_row(held, view.along);
      every.push(bar);
    }
    Kind::Picture => {}
  }

  let Ok(mut with_options) =
    what_is_this(every, held, view.shot.kind, &view.details, view.tracks);
  let Ok(transport) = transport(held, &view);
  with_options.push(transport);

  Ok(with_options)
}

fn where_in(watching: &Watching) -> Result<String, Never> {
  let Ok(alone) = alone(watching);

  Ok(match alone {
    Alone::No => {
      let Ok(which) = watching.reel.which();
      let Ok(many) = watching.reel.many();

      format!("{which} of {many}")
    }
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
  Ok(
    rows
      .into_iter()
      .map(|row| {
        let Ok(heading) = row.heading();

        match (heading, row.naming) {
          (console_panel::page::Heading::Yes, false) => row,
          (console_panel::page::Heading::Yes, true) | (console_panel::page::Heading::No, _) => {
            let asking = held.clone();
            let facts_2 = facts.to_string();

            let Ok(offering) = row.offering(move |showing| {
              let Ok(()) = what_else(&asking, showing, kind, &facts_2, tracks);

              ActionOutcome::Stay
            });

            offering
          }
        }
      })
      .collect(),
  )
}

fn what_else(
  held: &Shared,
  showing: &dyn console_panel::page::Showing,
  kind: Kind,
  facts: &str,
  tracks: u32,
) -> Result<(), Never> {
  let speeding = held.clone();
  let wording = held.clone();
  let about = facts.to_string();

  match kind {
    Kind::Picture => {
      let editing = held.clone();
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
        Box::new(move |showing_2, which| {
          let Ok(which_2) = console_core_number_conversion::index(which);

          match editing::EDITS.get(which_2) {
            Some(edit) => {
              let Ok(()) = edited(&editing, showing_2, *edit);
            }
            None => showing_2.open_out(),
          }
        }),
      )
    }
    Kind::Film => showing.sure(
      "About This Video",
      Subject(facts),
      &["Speed", "Subtitles", FULL_SCREEN],
      Box::new(move |showing_3, which| match which {
        0 => {
          let Ok(()) = how_fast(&speeding, showing_3, &about);
        }
        1 => {
          let Ok(()) = which_words(&wording, showing_3, &about, tracks);
        }
        _ => showing_3.open_out(),
      }),
    ),
  }

  Ok(())
}

const FULL_SCREEN: &str = "Full Screen";

const ZOOM_IN_FIRST: &str = "Zoom in on what to keep, then choose Crop.";

const NOT_SAVED: &str = "The edited picture couldn't be saved.";

const NOT_EDITABLE: &str = "Editing this kind of picture isn't supported yet.";

fn unsaved(why: &EditError) -> Result<String, Never> {
  Ok(match why {
    EditError::Unsupported(at) => match at.extension().and_then(OsStr::to_str) {
      Some(ending) => format!(
        "Editing {} pictures isn't supported yet.",
        ending.to_uppercase()
      ),
      None => NOT_EDITABLE.to_string(),
    },
    EditError::Rejected(_, format) => {
      let Ok(says) = format.says();

      format!("Editing this kind of {says} isn't supported yet.")
    }
    EditError::Unnamed(_)
    | EditError::Jpeg(..)
    | EditError::Png(..)
    | EditError::Unwritten(_) => NOT_SAVED.to_string(),
  })
}

fn edited(
  held: &Shared,
  showing: &dyn console_panel::page::Showing,
  edit: Edit,
) -> Result<(), Never> {
  let Ok(framed) = showing.framed();
  let Ok(reply) = ask(held, Request::Edit(edit, framed));

  match reply {
    Some(Reply::Edited(EditReply::Saved)) => showing.refresh(),
    Some(Reply::Edited(EditReply::Note(note))) => showing.note(&note),
    Some(_) | None => {}
  }

  Ok(())
}

fn how_fast(
  held: &Shared,
  showing: &dyn console_panel::page::Showing,
  name: &str,
) -> Result<(), Never> {
  let setting = held.clone();
  let says: Vec<&str> = playing::SPEEDS.iter().map(|(says, _)| *says).collect();

  showing.sure(
    "Playback Speed",
    Subject(name),
    &says,
    Box::new(move |_, which| {
      let Ok(which_2) = console_core_number_conversion::fitted(which);
      let Ok(at) = now(&setting);
      let Ok(()) = quietly(&setting, ViewerEvent::Speed { which: which_2, at });
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
  let setting = held.clone();
  let Ok(said) = playing::captions(tracks);
  let says: Vec<&str> = said.iter().map(String::as_str).collect();

  showing.sure(
    "Subtitles",
    Subject(name),
    &says,
    Box::new(move |_, which| {
      let Ok(which_2) = console_core_number_conversion::fitted(which);
      let Ok(at) = now(&setting);
      let Ok(()) = quietly(&setting, ViewerEvent::Text { which: which_2, at });
    }),
  );

  Ok(())
}

fn transport(held: &Shared, view: &View) -> Result<Row, Never> {
  let running = view.running;
  let only = view.only;
  let kind = view.shot.kind;
  let name = view.shot.name.clone();
  let tracks = view.tracks;

  let mut presses = Vec::new();

  let Ok(()) = walking(held, only, Icon::Previous, -1, &mut presses);

  let at = match kind {
    Kind::Film => {
      let Ok(skip) = fitted::<u64, i32>(playing::SKIP.saturating_div(playing::STEP));
      let Ok(back) = skipping(held, Icon::Rewind, skip.saturating_neg());
      let (on, wording) = (held.clone(), held.clone());
      let Ok(icon) = running.icon();
      let Ok(running_2) = ButtonPress::new(icon, Active::No, move |showing_2| {
        let Ok(at) = now(&on);
        let Ok(()) = press(&on, ViewerEvent::Running(at), showing_2);
      });
      let Ok(running_3) = running_2.chief();
      let Ok(forward) = skipping(held, Icon::FastForward, skip);
      let Ok(words) = ButtonPress::new(Icon::Subtitles, Active::No, move |showing_3| {
        let Ok(()) = which_words(&wording, showing_3, &name, tracks);
      });

      presses.push(back);

      let Ok(at) = fitted::<_, u32>(presses.len());

      presses.push(running_3);
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

  let Ok(whole) = ButtonPress::new(Icon::FullScreen, Active::No, |showing_4| {
    showing_4.toggle_full_screen()
  });
  let Ok(whole_2) = whole.cornered();

  presses.push(whole_2);

  Row::pressing(presses, at)
}

fn walking(
  held: &Shared,
  only: Alone,
  icon: Icon,
  by: i32,
  presses: &mut Vec<ButtonPress>,
) -> Result<(), Never> {
  match only {
    Alone::Yes => {}
    Alone::No => {
      let stepping = held.clone();
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
  let skipping = held.clone();

  ButtonPress::new(icon, Active::No, move |showing| {
    let Ok(()) = scrub(&skipping, by);

    showing.refresh();
  })
}

fn bar_row(held: &Shared, along: playing::Along) -> Result<Row, Never> {
  let stepping = held.clone();
  let tapped = held.clone();

  let whole = match ::core::primitive::bool::then_some(along.whole > 0, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(clock) = positional(Duration::from_secs(along.whole));

      clock
    }
    ::core::option::Option::None => String::new(),
  };

  let Ok(at) = positional(Duration::from_secs(along.at));
  let Ok(nothing) = Handler::and_stay(|_| {});
  let Ok(row) = Row::new(&at, Aside(&whole), nothing);
  let Ok(row_2) = row.picturing(Picture::Bar(Bar {
    at: along.at,
    of: along.whole,
  }));
  let Ok(row_3) = row_2.ended(Ends { less: "", more: "" });
  let Ok(row_4) = row_3.adjusting(move |by| {
    let Ok(()) = scrub(&stepping, by);
  });

  row_4.seeking(move |showing, fraction| {
    let Ok(()) = seek_to(&tapped, Fraction(fraction));

    showing.refresh();
  })
}

fn now(held: &Shared) -> Result<Since, Never> {
  let Ok(reply) = ask(held, Request::Time);

  Ok(match reply {
    Some(Reply::Time(now)) => now,
    Some(_) | None => Since::ZERO,
  })
}

fn seek_to(held: &Shared, fraction: Fraction) -> Result<(), Never> {
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
      let Ok(hers) = folder.user();

      hers
    })
    .filter(|at| paths::status(at) == Ok(Status::Directory))
    .collect();

  folders.push(here.to_path_buf());

  index::outermost(&folders)
}

fn read_for_the_index(at: &Path) -> Result<Vec<index::Read>, Never> {
  let Ok(kinds) = kinds();

  Ok(
    std::fs::read_dir(at)
      .into_iter()
      .flatten()
      .flatten()
      .filter_map(|entry| {
        let name = match entry.file_name().into_string() {
          Ok(name) => name,
          Err(_not_text) => return None,
        };
        let path = entry.path();
        let folder = paths::status(&path) == Ok(Status::Directory);

        let mime = match ::core::primitive::bool::then_some(folder, ()) {
          ::core::option::Option::Some(()) => String::new(),
          ::core::option::Option::None => {
            let Ok(mime) = console_content_types::of(&kinds, &path);

            mime
          }
        };

        Some(index::Read {
          name,
          path,
          folder,
          mime,
        })
      })
      .collect(),
  )
}

fn media_rows(held: &Shared) -> Result<Vec<Row>, Never> {
  let Ok(reply) = ask(held, Request::Location);
  let (folder, standing) = match reply {
    Some(Reply::Location { folder, at }) => (folder, at),
    Some(_) | None => return Ok(Vec::new()),
  };
  let Ok(folders) = media_folders(&folder);
  let Ok(found) = index::under(&folders, &read_for_the_index);

  let mut rows: Vec<Row> = Vec::new();

  for thing in found {
    let going = held.clone();
    let at = thing.path.clone();
    let picture = match thing.kind {
      Kind::Picture => Picture::At(thing.path.clone()),
      Kind::Film => Picture::Named(Icon::Film),
    };

    let aside = match ::core::primitive::bool::then_some(thing.path == standing, ()) {
      ::core::option::Option::Some(()) => console_panel::page::NOW,
      ::core::option::Option::None => "",
    };

    let Ok(stands) = Handler::and_stay(move |showing| {
      let Ok(()) = looked_at(&going, &at, showing);
    });
    let Ok(row) = Row::new(&thing.name, Aside(aside), stands);
    let Ok(row_2) = row.picturing(picture);

    match thing.path.to_str() {
      Some(path) => {
        let Ok(selectable) = row_2.selectable(path);
        let Ok(shows) = shown_in_the_files(held, &thing.path);
        let Ok(offering) =
          console_panel::page::shown_or_selected(&thing.name, &thing.path, shows);
        let Ok(row_3) = selectable.offering(offering);

        rows.push(row_3);
      }
      None => rows.push(row_2),
    }
  }

  console_panel::page::lettered(rows)
}

fn looked_at(
  held: &Shared,
  at: &Path,
  showing: &dyn console_panel::page::Showing,
) -> Result<(), Never> {
  let Ok(effects) = changed(held, Request::Open(at.to_path_buf()));

  perform(effects, showing)
}

fn stir(held: &Shared) -> Result<WakeOutcome, Never> {
  let Ok(reply) = ask(held, Request::Wake);

  Ok(match reply {
    Some(Reply::WakeResult(wake)) => wake,
    Some(_) | None => WakeOutcome::AlreadyAwake,
  })
}

fn pages(held: &Shared) -> Result<Vec<Page>, Never> {
  let drawing = held.clone();
  let stirring = held.clone();
  let listing = held.clone();
  let compute_draw = move || {
    let Ok(rows) = rows(&drawing);
    rows
  };
  let Ok(asked) = Rows::computed(compute_draw.clone());
  let page = Page {
    title: "Viewing".to_string(),
    rows: asked,
    sought: None,
    back: None,
    entered: None,
    meanwhile: None,
    watch: None,
    listens: None,
    stirs: None,
    set: Set::FromTheTop,
    selecting: Vec::new(),
  };
  let Ok(page_with_draw) = page.meanwhile(compute_draw);
  let Ok(looking) = page_with_draw.stirring(move || {
    let Ok(stirred) = stir(&stirring);

    stirred
  });
  let compute_media = move || {
    let Ok(rows) = media_rows(&listing);
    rows
  };
  let Ok(asked_2) = Rows::computed(compute_media.clone());
  let page_2 = Page {
    title: "Media".to_string(),
    rows: asked_2,
    sought: None,
    back: None,
    entered: None,
    meanwhile: None,
    watch: None,
    listens: None,
    stirs: None,
    set: Set::FromTheTop,
    selecting: Vec::new(),
  };
  let Ok(page_with_media) = page_2.meanwhile(compute_media);
  let Ok(media) = page_with_media.trashing_selected();

  Ok(vec![looking, media])
}

fn by_default() -> Result<Option<PathBuf>, Never> {
  Ok(
    MEDIA
      .into_iter()
      .filter_map(|folder| {
        let Ok(hers) = folder.user();

        hers
      })
      .find(|folder| paths::status(folder) == Ok(Status::Directory)),
  )
}

fn standing_on(asked: &Path) -> Result<Option<Looking>, Never> {
  let Ok(here) = Looking::of(asked);

  match here {
    Some(looking) => return Ok(Some(looking)),
    None => {}
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

  let asked_3 = match asked {
    Some(asked_2) => asked_2,
    None => return Ok(Worth::Opening),
  };

  match ::core::primitive::bool::then_some(paths::status(&asked_3) == Ok(Status::Directory), ()) {
    ::core::option::Option::Some(()) => return Ok(Worth::Opening),
    ::core::option::Option::None => {}
  }

  let Ok(found) = Looking::of(&asked_3);

  match found {
    Some(_) => Ok(Worth::Opening),
    None => {
      eprintln!(
        "viewer: {}: this is neither a picture nor a film",
        asked_3.display()
      );

      Ok(Worth::None)
    }
  }
}

fn asked_for(arguments: &[String]) -> Result<Option<PathBuf>, Never> {
  let said = arguments
    .first()
    .filter(|said| !said.is_empty())
    .map(PathBuf::from);

  match said {
    Some(said_2) => Ok(Some(said_2)),
    None => {
      let Ok(folder) = by_default();

      let folder_3 = match folder {
        Some(folder_2) => folder_2,
        None => return Ok(None),
      };

      Ok(Some(folder_3))
    }
  }
}

const NOTHING_HERE: &str = "No Pictures or Videos";

const NO_FOLDER: &str = "No Pictures Folder";

fn saying_only(says: &'static str) -> Result<Card, Never> {
  Card::new(move || {
    let Ok(row) = Row::placeholder(says);
    let Ok(page) = Page::new("Media", Rows::Fixed(vec![row]));

    vec![page]
  })
}

pub fn card(arguments: &[String]) -> Result<Card, Never> {
  let Ok(asked) = asked_for(arguments);

  let asked_3 = match asked {
    Some(asked_2) => asked_2,
    None => return saying_only(NO_FOLDER),
  };

  let Ok(found) = standing_on(&asked_3);

  let looking = match found {
    Some(looking) => looking,
    None => return saying_only(NOTHING_HERE),
  };

  let mut worker = match Owner::start(looking) {
    Ok(worker) => worker,
    Err(fault) => {
      console_core_logging::warn!("could not start the media viewer: {fault}");

      return saying_only("The viewer could not start");
    }
  };
  let address = worker.address.clone();
  let Ok(card) = Card::new(move || {
    let Ok(pages) = pages(&address);

    pages
  });

  card.shutting(Box::new(move || worker.close()))
}

#[cfg(test)]
mod tests {
  use std::error::Error;

  use super::*;

  fn a_folder_with_a_film() -> Result<PathBuf, Box<dyn Error>> {
    let folder = console_core_temporary_directories::fresh("viewer-card")?;
    let Ok(silent) = paths::fixture("red-then-blue.av1.mkv");

    std::fs::copy(silent, folder.join("silent.mkv"))?;

    Ok(folder)
  }

  #[test]
  fn play_starts_the_film_its_clock_moves_on_and_pause_stops_it_where_it_was()
  -> Result<(), Box<dyn Error>> {
    let folder = a_folder_with_a_film()?;
    let source = folder.join("silent.mkv");
    let Ok(opened) = Looking::of(&source);
    let mut looking = opened.ok_or("a folder with a film in it opens on the film")?;

    let Ok(()) = looking.in_step();

    assert!(looking.film.is_none(), "a film opens stopped");
    assert_eq!(
      looking.watching.along.whole, 2,
      "the length of the film is known before it plays"
    );

    let ::core::result::Result::Ok(_) = looking.handle_event(ViewerEvent::Running(Since::ZERO));
    let Ok(()) = looking.in_step();

    assert!(looking.film.is_some(), "play started nothing");

    let Ok(patience) = console_waiting::Schedule::of(std::time::Duration::from_secs(20));
    let Ok(moved) = console_waiting::until_handed(patience, &mut looking, |viewer| {
      let Ok(()) = viewer.in_step();

      Ok(
        match ::core::primitive::bool::then_some(viewer.watching.along.at >= 1, ()) {
          ::core::option::Option::Some(()) => console_waiting::Ready::Yes,
          ::core::option::Option::None => console_waiting::Ready::NotYet,
        },
      )
    });

    assert_eq!(
      moved,
      console_waiting::Outcome::Happened,
      "the clock under the film never moved"
    );

    let ::core::result::Result::Ok(_) = looking.handle_event(ViewerEvent::Running(Since::ZERO));
    let Ok(()) = looking.in_step();
    let stopped_at = looking.watching.along.at;

    assert!(looking.film.is_none(), "pause left the film playing");
    assert!(stopped_at >= 1, "pause lost the place: {stopped_at}");
    let stopped = looking
      .sound
      .as_ref()
      .ok_or("pause did not return audio ownership")?;
    let Ok(clock_after_pause) = stopped.position_reader();

    let ::core::result::Result::Ok(_) = looking.handle_event(ViewerEvent::Running(Since::ZERO));
    let Ok(()) = looking.in_step();
    assert!(matches!(clock_after_pause.position(), Ok(Some(_position))));
    let resumed = looking.film.as_ref().map(|film_2| {
      let Ok(Seconds(at)) = film_2.at();

      at
    });

    drop(looking);
    std::fs::remove_dir_all(&folder)?;

    let from = resumed.ok_or("play again started nothing")?;

    assert!(from >= 1.0, "play again started from the beginning: {from}");

    Ok(())
  }

  #[test]
  fn a_picture_that_cannot_be_edited_says_which_kind_it_is() {
    let heic = EditError::Unsupported(PathBuf::from("/p/phone.heic"));
    let cmyk = EditError::Rejected(
      PathBuf::from("/p/print.jpg"),
      console_pictures::Format::Jpeg,
    );
    let unnamed = EditError::Unsupported(PathBuf::from("/p/scan"));

    assert_eq!(
      unsaved(&heic),
      Ok(String::from("Editing HEIC pictures isn't supported yet."))
    );
    assert_eq!(
      unsaved(&cmyk),
      Ok(String::from(
        "Editing this kind of JPEG isn't supported yet."
      ))
    );
    assert_eq!(unsaved(&unnamed), Ok(String::from(NOT_EDITABLE)));
  }
}
