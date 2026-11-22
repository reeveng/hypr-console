//! A film, played: its sound through the speaker and its pictures onto the card.
//!
//! The sound and the pictures are read apart, from the same place in the film,
//! and kept together by one clock. The sound is `console_music_player`'s
//! `Sounding`, which is already the one place on this device that turns a file
//! into something heard, and it already knows how far through the file the
//! speaker has got. So the sound is the clock: each picture comes out of
//! `console_films` knowing which moment of the film it is, and is
//! handed to the card when the sound reaches that moment. A picture whose
//! moment has passed by the time it is decoded is not coloured or drawn, so a
//! slow machine shows fewer pictures of a film that stays in step with its
//! sound instead of every picture of one that falls behind it. A film with no
//! sound has no speaker to follow, and follows the wall instead.
//!
//! **Nothing is paused.** A pause lets the pictures go and keeps where the
//! film had got to, which is `Sounding`'s own argument for the music player
//! and it holds here for the same reasons: a decoder held open keeps the
//! machine awake for nobody. A seek, a speed and a choice of subtitles are the
//! same: the film is started again from where it is, with the new thing asked
//! for. [`next`] is that decision, and it is arithmetic, so it is what the
//! tests press.
//!
//! **The card is told, not asked.** Each picture goes to `console_panel::frames`,
//! which repaints the picture's own rectangle and nothing else, and once a
//! second of film the rows are asked for again so that the clock and the bar
//! under the picture move on. The words, the bar and the buttons are painted
//! once a second and the film as often as it has pictures.
//!
//! **Subtitles go with the picture they are said over.** libass drew them into
//! the frames when ffmpeg made the frames. Now each picture is handed to the
//! card with the lines due at its moment, and the card draws them over the
//! foot of the picture in its own letters -- so a line cannot fall out of step
//! with the picture, and it is as legible as every other word on the card. The
//! lines come from a track in the film or a file beside it, read by
//! `console_core_subtitle_files`.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{
  Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use console_concurrency::Cores;
use console_core_geometry::Size;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_u64};
use console_core_shapes::Pixels;
use console_core_subtitle_files::{Cue, Seconds, Written};
use console_films::{FramesPerSecond, Picture};
use console_music_player::sounding::{PositionReader, Sounding, Tempo, Wanted};

pub use console_films::Sound;

use crate::paths::{self, Name, Status};
use crate::playing::{self, Captions};

#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
  pub rate: FramesPerSecond,
  pub seconds: Seconds,
  pub sound: Sound,
  pub words: u32,
}

const ORDINARY_RATE: f64 = 30.0;

const FASTEST_RATE: f64 = 60.0;

pub fn facts(film: &Path) -> Result<Option<Facts>, Never> {
  let opened = console_films::open(film);

  let opened_3 = match opened {
    Ok(Some(opened_2)) => opened_2,
    Ok(None) => return Ok(None),
    Err(fault) => {
      console_core_logging::warn!("{fault}");

      return Ok(None);
    }
  };

  let Ok(facts) = opened_3.facts();

  let facts_3 = match facts {
    Some(facts_2) => facts_2,
    None => return Ok(None),
  };

  let Ok(words) = fitted::<_, u32>(facts_3.subtitles.len());

  Ok(Some(Facts {
    rate: FramesPerSecond(match (facts_3.rate.0 > 0.0).then_some(()) {
      ::core::option::Option::Some(()) => facts_3.rate.0.min(FASTEST_RATE),
      None => ORDINARY_RATE,
    }),
    seconds: Seconds(facts_3.seconds.0.max(0.0)),
    sound: facts_3.sound,
    words,
  }))
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackRequest {
  pub film: PathBuf,
  pub from: Seconds,
  pub speed: u32,
  pub captions: Captions,
  pub sought: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
  None,
  Start,
  Stop,
}

pub fn next(
  playing: Option<&PlaybackRequest>,
  wanted: Option<&PlaybackRequest>,
) -> Result<Next, Never> {
  Ok(match (playing, wanted) {
    (None, None) => Next::None,
    (Some(_), None) => Next::Stop,
    (None, Some(_)) => Next::Start,
    (Some(playing_2), Some(wanted_2)) => {
      let same = playing_2.film == wanted_2.film
        && playing_2.speed == wanted_2.speed
        && playing_2.captions == wanted_2.captions
        && playing_2.sought == wanted_2.sought;

      match ::core::primitive::bool::then_some(same, ()) {
        ::core::option::Option::Some(()) => Next::None,
        ::core::option::Option::None => Next::Start,
      }
    }
  })
}

pub fn sidecars(film: &Path) -> Result<Vec<PathBuf>, Never> {
  let name = match film.file_name() {
    Some(name) => name,
    None => return Ok(Vec::new()),
  };

  let folder = match film.parent() {
    Some(folder) => folder,
    None => return Ok(Vec::new()),
  };

  let Ok(said) = playing::beside(name);

  Ok(
    said
      .into_iter()
      .filter_map(|filename| {
        let Ok(child) = Name::new(&filename);

        child.map(|child_2| folder.join(child_2))
      })
      .filter(|at| paths::status(at) == Ok(Status::File))
      .collect(),
  )
}

fn cues(at: &Path, film: &console_films::Film, captions: Captions) -> Result<Vec<Cue>, Never> {
  let Ok(track) = captions.track();

  let track_3 = match track {
    Some(track_2) => track_2,
    None => return Ok(Vec::new()),
  };

  let Ok(facts) = film.facts();

  let inside = match facts {
    Some(facts_2) => facts_2.subtitles,
    None => Vec::new(),
  };

  let Ok(wanted) = index(track_3);

  match inside.get(wanted) {
    Some(subtitles) => {
      return Ok(match film.cues(subtitles) {
        Ok(cues) => cues,
        Err(fault) => {
          console_core_logging::warn!("{fault}");

          Vec::new()
        }
      });
    }
    None => {}
  }

  let Ok(inside_2) = fitted::<_, u32>(inside.len());
  let Ok(beyond) = index(track_3.saturating_sub(inside_2));
  let Ok(beside) = sidecars(at);

  let file = match beside.get(beyond) {
    Some(file) => file,
    None => return Ok(Vec::new()),
  };

  let Ok(written) = Written::of(file);

  let (written_3, bytes) = match (written, std::fs::read(file)) {
    (Some(written_2), Ok(bytes)) => (written_2, bytes),
    (None, _) => return Ok(Vec::new()),
    (Some(_), Err(fault)) => {
      console_core_logging::warn!("{}: {fault}", file.display());

      return Ok(Vec::new());
    }
  };

  console_core_subtitle_files::read(&bytes, written_3)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Going {
  On,
  Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
  Yes,
  No,
}

struct Control {
  cancel: Receiver<()>,
  ended: SyncSender<()>,
}

#[derive(Clone)]
enum Clock {
  Audio {
    position: PositionReader,
    from: Seconds,
  },
  Wall {
    from: Seconds,
    tempo: Tempo,
    began: Instant,
  },
}

impl Clock {
  fn now(&self) -> Result<Seconds, Never> {
    match self {
      Clock::Audio { position, from } => {
        let Ok(position_2) = position.position();

        Ok(match position_2 {
          Some(seconds) => Seconds(seconds),
          None => *from,
        })
      }
      Clock::Wall { from, tempo, began } => {
        Ok(Seconds(from.0 + began.elapsed().as_secs_f64() * tempo.0))
      }
    }
  }
}

pub struct Film {
  pub asked: PlaybackRequest,
  clock: Clock,
  cancel: Option<SyncSender<()>>,
  completed: Receiver<()>,
  ended_seen: Cell<Ended>,
  pictures: Option<JoinHandle<()>>,
  sound: Option<Sounding>,
}

impl Film {
  pub fn start(
    asked: PlaybackRequest,
    facts: &Facts,
    room: Size<u32>,
    sound: Sounding,
  ) -> Result<Film, Never> {
    let Ok((_, speed)) = playing::speed(asked.speed);
    let tempo = Tempo(speed);
    let clock = match facts.sound {
      Sound::Audible => {
        let Ok(()) = sound.play_at(&asked.film, asked.from.0, tempo);
        let Ok(position) = sound.position_reader();

        Clock::Audio {
          position,
          from: asked.from,
        }
      }
      Sound::Silent => {
        let Ok(()) = sound.wanting(Wanted::Stopped);

        #[cfg_attr(
          dylint_lib = "explicit039_no_reading_the_clock",
          allow(
            explicit039_no_reading_the_clock,
            reason = "a silent film uses this wall-clock anchor for presentation timestamps"
          )
        )]
        let began = Instant::now();

        Clock::Wall {
          from: asked.from,
          tempo,
          began,
        }
      }
    };
    let picture_clock = clock.clone();
    let (cancel, stopped) = sync_channel(0);
    let (ended, completed) = sync_channel(1);
    let control = Control {
      cancel: stopped,
      ended,
    };

    let Ok(cores) = Cores::counted();
    let showing = Showing {
      film: asked.film.clone(),
      from: asked.from,
      tempo,
      rate: facts.rate,
      size: room,
      captions: asked.captions,
      cores,
    };
    let started = std::thread::Builder::new()
      .name("video frames".to_string())
      .spawn(move || {
        let Ok(()) = shown(&showing, &picture_clock, &control);
      });

    let pictures = match started {
      Ok(worker) => Some(worker),
      Err(fault) => {
        console_core_logging::warn!("could not start the video worker: {fault}");
        let Ok(()) = sound.wanting(Wanted::Stopped);

        None
      }
    };

    Ok(Film {
      asked,
      clock,
      cancel: Some(cancel),
      completed,
      ended_seen: Cell::new(Ended::No),
      pictures,
      sound: Some(sound),
    })
  }

  pub fn at(&self) -> Result<Seconds, Never> {
    self.clock.now()
  }

  pub fn ended(&self) -> Result<Ended, Never> {
    match self.completed.try_recv() {
      Ok(()) | Err(TryRecvError::Disconnected) => self.ended_seen.set(Ended::Yes),
      Err(TryRecvError::Empty) => return Ok(self.ended_seen.get()),
    }

    Ok(Ended::Yes)
  }

  pub fn stop(mut self) -> Result<Option<Sounding>, Never> {
    let Ok(()) = self.stop_worker();

    Ok(self.sound.take())
  }

  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "closing the stop channel wakes frame waits; joining finishes the in-progress decode before another film may publish frames"
    )
  )]
  fn stop_worker(&mut self) -> Result<(), Never> {
    drop(self.cancel.take());

    match &self.sound {
      Some(sound) => {
        let Ok(()) = sound.wanting(Wanted::Stopped);
      }
      None => {}
    }

    match self.pictures.take() {
      Some(worker) => match worker.join() {
        Ok(()) => {}
        Err(_panicked) => console_core_logging::warn!("the video worker stopped before closing"),
      },
      None => {}
    }

    Ok(())
  }
}

impl Drop for Film {
  fn drop(&mut self) {
    let Ok(()) = self.stop_worker();
  }
}

struct Showing {
  film: PathBuf,
  from: Seconds,
  tempo: Tempo,
  rate: FramesPerSecond,
  size: Size<u32>,
  captions: Captions,
  cores: Cores,
}

fn state(control: &Control) -> Result<Going, Never> {
  Ok(match control.cancel.try_recv() {
    Ok(()) | Err(TryRecvError::Disconnected) => Going::Off,
    Err(TryRecvError::Empty) => Going::On,
  })
}

fn wait(control: &Control, seconds: Seconds) -> Result<Going, Never> {
  const LONGEST: Seconds = Seconds(0.25);
  let long = match Duration::try_from_secs_f64(seconds.0.clamp(0.0, LONGEST.0)) {
    Ok(long) => long,
    Err(fault) => {
      console_core_logging::warn!("invalid wait for a video frame: {fault}");

      return Ok(Going::Off);
    }
  };

  Ok(match control.cancel.recv_timeout(long) {
    Ok(()) | Err(RecvTimeoutError::Disconnected) => Going::Off,
    Err(RecvTimeoutError::Timeout) => Going::On,
  })
}

fn drawn(picture: &Picture, room: Size<u32>, cores: Cores) -> Result<Pixels, Never> {
  let Ok(shown) = picture.size();
  let Ok(made) = console_pictures::fit_within(shown, room);
  let Ok(drawn) = picture.color_pixels_at(made, cores);

  drawn.into_strided()
}

fn shown(showing: &Showing, clock: &Clock, control: &Control) -> Result<(), Never> {
  let Ok(going) = state(control);

  match going {
    Going::Off => return Ok(()),
    Going::On => {}
  }

  let opened = console_films::open(&showing.film);

  let film = match opened {
    Ok(Some(film)) => film,
    Ok(None) => return ended(control),
    Err(fault) => {
      console_core_logging::warn!("opening the film: {fault}");

      return ended(control);
    }
  };

  let Ok(cues) = cues(&showing.film, &film, showing.captions);
  let pictures = film.pictures(showing.from);

  let pictures_3 = match pictures {
    Ok(Some(pictures_2)) => pictures_2,
    Ok(None) => return ended(control),
    Err(fault) => {
      console_core_logging::warn!("starting video playback: {fault}");

      return ended(control);
    }
  };

  let mut skipped = 0_u64;
  let played = console_core_iteration::iterate(
    (pictures_3, u64::MAX, &mut skipped),
    |(mut pictures_4, said_second, skipped_2)| {
      let Ok(going_2) = state(control);

      match going_2 {
        Going::Off => return Ok(Step::Halt(Stop::TurnedOff)),
        Going::On => {}
      }

      let next = pictures_4.next_picture();

      let picture = match next {
        Ok(Some(picture)) => picture,
        Ok(None) => return Ok(Step::Halt(Stop::FilmOver)),
        Err(fault) => return Ok(Step::Halt(Stop::DecodeFailed(fault))),
      };

      let Ok(put) = put(showing, (clock, control), &picture, &cues);

      Ok(match put {
        Put::Off => Step::Halt(Stop::TurnedOff),
        Put::Late => {
          *skipped_2 = skipped_2.saturating_add(1);

          Step::Again((pictures_4, said_second, skipped_2))
        }
        Put::Shown(due) => {
          let Ok(second) = toward_zero_u64(due.0);

          match ::core::primitive::bool::then_some(second == said_second, ()) {
            ::core::option::Option::Some(()) => {}
            ::core::option::Option::None => {
              let Ok(()) = console_panel::frames::tell(console_panel::frames::Notice::Rows);
            }
          }

          Step::Again((pictures_4, second, skipped_2))
        }
      })
    },
  );

  let terminal = match played {
    Ok(stop) => stop,
    Err(fault) => {
      console_core_logging::warn!("video playback stopped unexpectedly: {fault}");

      return ended(control);
    }
  };

  match &terminal {
    Stop::DecodeFailed(fault) => {
      console_core_logging::warn!(
        "video decoding stopped after skipping {skipped} late frames: {fault}"
      );
    }
    Stop::FilmOver | Stop::TurnedOff => match skipped {
      0 => {}
      _ => console_core_logging::warn!("skipped {skipped} late video frames during playback"),
    },
  }

  match terminal {
    Stop::TurnedOff => Ok(()),
    Stop::FilmOver | Stop::DecodeFailed(_) => ended(control),
  }
}

enum Put {
  Shown(Seconds),
  Late,
  Off,
}

fn put(
  showing: &Showing,
  keeping: (&Clock, &Control),
  picture: &Picture,
  cues: &[Cue],
) -> Result<Put, Never> {
  let (clock, control) = keeping;
  let a_frame = 1.0 / showing.rate.0;
  let Ok(due) = picture.at();
  let Ok(still) = until_due(
    clock,
    control,
    Due {
      at: Seconds(due),
      early: Seconds(a_frame / 2.0),
      tempo: showing.tempo,
    },
  );

  match still {
    Going::Off => return Ok(Put::Off),
    Going::On => {}
  }

  let Ok(still_2) = state(control);

  match still_2 {
    Going::Off => return Ok(Put::Off),
    Going::On => {}
  }

  let Ok(now) = clock.now();

  match ::core::primitive::bool::then_some(now.0 - due > a_frame * 2.0, ()) {
    ::core::option::Option::Some(()) => return Ok(Put::Late),
    ::core::option::Option::None => {}
  }

  let Ok(pixels) = drawn(picture, showing.size, showing.cores);
  let Ok(going) = state(control);

  match going {
    Going::Off => return Ok(Put::Off),
    Going::On => {}
  }

  let Ok(said) = console_core_subtitle_files::showing(cues, Seconds(due));
  let Ok(()) = console_panel::frames::insert(&showing.film, pixels, said);

  Ok(Put::Shown(Seconds(due)))
}

enum Stop {
  FilmOver,
  TurnedOff,
  DecodeFailed(console_films::FilmError),
}

#[derive(Clone, Copy)]
struct Due {
  at: Seconds,
  early: Seconds,
  tempo: Tempo,
}

fn until_due(clock: &Clock, control: &Control, due: Due) -> Result<Going, Never> {
  let Due {
    at: due_2,
    early,
    tempo,
  } = due;
  let waited = console_core_iteration::iterate((), |()| {
    let Ok(now) = clock.now();

    match ::core::primitive::bool::then_some(now.0 + early.0 >= due_2.0, ()) {
      ::core::option::Option::Some(()) => return Ok(Step::Halt(Going::On)),
      ::core::option::Option::None => {}
    }

    let Ok(still) = wait(control, Seconds((due_2.0 - now.0) / tempo.0));

    Ok(match still {
      Going::Off => Step::Halt(Going::Off),
      Going::On => Step::Again(()),
    })
  });

  Ok(match waited {
    Ok(going) => going,
    Err(_endless) => Going::On,
  })
}

fn ended(control: &Control) -> Result<(), Never> {
  match control.ended.try_send(()) {
    Ok(()) | Err(TrySendError::Full(())) => {
      console_panel::frames::tell(console_panel::frames::Notice::Rows)
    }
    Err(TrySendError::Disconnected(())) => Ok(()),
  }
}

#[cfg(test)]
mod tests {
  use std::error::Error;

  use super::*;

  fn request(film: &str) -> Result<PlaybackRequest, Never> {
    Ok(PlaybackRequest {
      film: PathBuf::from(film),
      from: Seconds(0.0),
      speed: 1,
      captions: Captions::Off,
      sought: None,
    })
  }

  #[test]
  fn a_film_starts_when_it_is_wanted_and_stops_when_it_is_not() {
    let Ok(film) = request("/x/holiday.mp4");

    assert_eq!(next(None, None), Ok(Next::None));
    assert_eq!(next(None, Some(&film)), Ok(Next::Start));
    assert_eq!(next(Some(&film), None), Ok(Next::Stop));
    assert_eq!(next(Some(&film), Some(&film)), Ok(Next::None));
  }

  #[test]
  fn the_film_moving_on_by_itself_is_not_a_reason_to_start_it_again() {
    let Ok(playing) = request("/x/holiday.mp4");
    let later = PlaybackRequest {
      from: Seconds(42.0),
      ..playing.clone()
    };

    assert_eq!(next(Some(&playing), Some(&later)), Ok(Next::None));
  }

  #[test]
  fn a_seek_a_speed_subtitles_or_another_film_start_it_again() {
    let Ok(playing) = request("/x/holiday.mp4");

    for wanted in [
      PlaybackRequest {
        sought: Some(90),
        ..playing.clone()
      },
      PlaybackRequest {
        speed: 3,
        ..playing.clone()
      },
      PlaybackRequest {
        captions: Captions::Track(0),
        ..playing.clone()
      },
      PlaybackRequest {
        film: PathBuf::from("/x/other.mkv"),
        ..playing.clone()
      },
    ] {
      assert_eq!(
        next(Some(&playing), Some(&wanted)),
        Ok(Next::Start),
        "{wanted:?}"
      );
    }
  }

  #[test]
  fn what_a_film_says_about_itself_is_what_playing_it_needs() -> Result<(), Box<dyn Error>> {
    let Ok(at) = paths::fixture("red-then-blue.vp9.webm");
    let Ok(found) = facts(&at);
    let heard = found.ok_or("a film with pictures in it is a film")?;

    assert!((heard.rate.0 - 25.0).abs() < 0.5, "{heard:?}");
    assert!((heard.seconds.0 - 2.0).abs() < 0.1, "{heard:?}");
    assert_eq!((heard.sound, heard.words), (Sound::Audible, 0));

    let Ok(at_2) = paths::fixture("with-subtitles.av1.mkv");
    let Ok(worded) = facts(&at_2);
    let worded_2 = worded.ok_or("a film with words in it is a film")?;

    assert_eq!((worded_2.sound, worded_2.words), (Sound::Silent, 1));

    let not_a_film = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");

    assert_eq!(facts(&not_a_film), Ok(None));

    Ok(())
  }

  #[test]
  fn subtitles_come_from_the_film_first_and_then_from_a_file_beside_it()
  -> Result<(), Box<dyn Error>> {
    let folder = console_core_temporary_directories::fresh("viewer-words")?;
    let at = folder.join("holiday.mkv");
    let Ok(inside) = paths::fixture("with-subtitles.av1.mkv");

    std::fs::copy(&inside, &at)?;
    console_core_atomic_writes::whole(
      &folder.join("holiday.srt"),
      b"1\n00:00:00,100 --> 00:00:00,900\nBeside it\n",
    )?;

    let opened = console_films::open(&at)?;
    let film = opened.ok_or("the copy is a film")?;
    let Ok(off) = cues(&at, &film, Captions::Off);
    let Ok(inside_2) = cues(&at, &film, Captions::Track(0));
    let Ok(beside) = cues(&at, &film, Captions::Track(1));
    let Ok(past_the_end) = cues(&at, &film, Captions::Track(2));
    let Ok(said_inside) = console_core_subtitle_files::showing(&inside_2, Seconds(0.6));
    let Ok(said_beside) = console_core_subtitle_files::showing(&beside, Seconds(0.6));

    assert!(off.is_empty());
    assert_eq!(said_inside, vec![String::from("Red, at first")]);
    assert_eq!(said_beside, vec![String::from("Beside it")]);
    assert!(past_the_end.is_empty());

    Ok(())
  }

  #[test]
  fn a_film_plays_through_its_frames_onto_the_card_and_says_when_it_is_over()
  -> Result<(), Box<dyn Error>> {
    let Ok(at) = paths::fixture("with-subtitles.av1.mkv");
    let Ok(facts) = facts(&at);
    let facts_2 = facts.ok_or("the film it was handed is no film")?;

    assert_eq!(
      facts_2.sound,
      Sound::Silent,
      "this test must not make a noise on the machine running it"
    );

    let room = Size {
      width: 32,
      height: 32,
    };
    let _looking = console_panel::frames::current(&at, room);
    let Ok(sound) = Sounding::new(|_progress| Ok(()));
    let asked = PlaybackRequest {
      film: at.clone(),
      from: Seconds(0.0),
      speed: 1,
      captions: Captions::Track(0),
      sought: None,
    };
    let Ok(film) = Film::start(asked, &facts_2, room, sound);
    let Ok(patience) = console_waiting::Schedule::of(Duration::from_secs(20));
    let mut observations: (Vec<Vec<u8>>, Vec<Vec<String>>) = (Vec::new(), Vec::new());

    let Ok(over) = console_waiting::until_handed(patience, &mut observations, |(seen, heard)| {
      let Ok(current) = console_panel::frames::current(&at, room);

      match current {
        Some(frame) => {
          match ::core::primitive::bool::then_some(seen.contains(&frame.bytes), ()) {
            ::core::option::Option::Some(()) => {}
            ::core::option::Option::None => {
              assert_eq!(
                (frame.width, frame.height),
                (32, 21),
                "a frame is the film's shape inside the room"
              );

              seen.push(frame.bytes);
            }
          }
        }
        None => {}
      }

      let Ok(said) = console_panel::frames::said(&at);

      match ::core::primitive::bool::then_some(heard.contains(&said), ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => heard.push(said),
      }

      let Ok(ended) = film.ended();

      Ok(match ended {
        Ended::Yes => console_waiting::Ready::Yes,
        Ended::No => console_waiting::Ready::NotYet,
      })
    });
    let Ok(Seconds(heard_at)) = film.at();

    drop(film);

    let (seen, heard) = observations;

    assert_eq!(
      over,
      console_waiting::Outcome::Happened,
      "a two second film never ended"
    );
    assert!(
      seen.len() >= 2,
      "the picture on the card never moved on: {} different frames reached it",
      seen.len()
    );
    assert!(
      heard.contains(&vec![String::from("Red, at first")])
        && heard.contains(&vec![String::from("then blue")]),
      "the subtitles were not said over the film: {heard:?}"
    );
    assert!(heard_at > 0.5, "the film's clock stood still at {heard_at}");

    Ok(())
  }
}
