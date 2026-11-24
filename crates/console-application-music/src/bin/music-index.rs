//! Catalog refresh and background audio analysis, run by the Music Library task driver.

use std::collections::VecDeque;
use std::path::Path;
use std::process::ExitCode;

use console_application_music::cleanup::Reconciliation;
use console_application_music::looking::{FileRevision, Song};
use console_application_music::{album_art, albums, library, looking, review, suggestions};
use console_core_never::Never;
use console_core_state_machine::{EffectDependencies, Machine, Queue, Service, Transition};
use console_music_player::{loudness, tags};

#[derive(Clone, Copy)]
enum Mode {
  Tags,
  Audio,
}

struct Pending {
  remaining: VecDeque<Song>,
  songs: Vec<Song>,
  audio: Vec<Option<review::Audio>>,
  mode: Mode,
}

enum State {
  Pending(Pending),
  Finished,
}

enum Request {
  Measured(Song, Option<review::Audio>),
}

enum Effect {
  RemoveTracks,
  Read(Song, Mode),
  Checkpoint(Vec<Song>),
  Publish(Pending),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Read(..) => &[Service::FileSystem, Service::Process],
      Self::RemoveTracks | Self::Checkpoint(_) | Self::Publish(_) => &[Service::FileSystem],
    })
  }
}

struct Index;

impl Machine for Index {
  type Input = (Vec<Song>, Mode);
  type State = State;
  type Request = Request;
  type Effect = Effect;

  fn initialize(
    input: &Self::Input,
    _previous: Option<State>,
    effects: &mut Queue<Effect>,
  ) -> Result<State, Never> {
    let (songs, mode) = input;
    let Ok(()) = effects.offer(Effect::RemoveTracks);

    step(
      Pending {
        remaining: VecDeque::from(songs.clone()),
        songs: Vec::new(),
        audio: Vec::new(),
        mode: *mode,
      },
      effects,
    )
  }

  fn handle(state: State, request: Request, effects: &mut Queue<Effect>) -> Result<State, Never> {
    match (state, request) {
      (State::Finished, _) => Ok(State::Finished),
      (State::Pending(mut pending), Request::Measured(song, audio)) => {
        pending.songs.push(song);
        pending.audio.push(audio);
        let Ok(count) = console_core_number_conversion::fitted::<_, u32>(pending.songs.len());

        match count.wrapping_rem(50) {
          0 => {
            let all = pending
              .songs
              .iter()
              .chain(&pending.remaining)
              .cloned()
              .collect();
            let Ok(()) = effects.offer(Effect::Checkpoint(all));
          }
          _ => {}
        }

        step(pending, effects)
      }
    }
  }
}

fn step(mut pending: Pending, effects: &mut Queue<Effect>) -> Result<State, Never> {
  Ok(match pending.remaining.pop_front() {
    Some(song) => {
      let Ok(()) = effects.offer(Effect::Read(song, pending.mode));

      State::Pending(pending)
    }
    None => {
      let Ok(()) = effects.offer(Effect::Publish(pending));

      State::Finished
    }
  })
}

fn measured(mut song: Song, cache: &Path, mode: Mode) -> Result<Request, Never> {
  match song.read.then_some(()) {
    Some(()) => {}
    None => {
      let Ok(metadata) = tags::of(&song.path);
      let Ok(after) = FileRevision::of(&song.path);
      song.tags = metadata;
      song.read = after.is_some() && after == song.revision;
    }
  }

  let audio = match mode {
    Mode::Tags => None,
    Mode::Audio => {
      match loudness::index_in_background(&song.path) {
        Ok(()) => {}
        Err(fault) => {
          console_core_logging::debug!("could not measure {}: {fault}", song.path.display());
        }
      }

      review::audio_for(cache, &song)?
    }
  };

  Ok(Request::Measured(song, audio))
}

fn wrote<'a>(at: &Path, songs: impl IntoIterator<Item = &'a Song>) -> std::io::Result<()> {
  let Ok(written) = looking::serialize_records(songs);

  console_core_atomic_writes::whole_with_folders(at, written.as_bytes())
    .map_err(std::io::Error::other)
}

fn publish(
  at: &Path,
  cache: &Path,
  pending: Pending,
  prior: &[Song],
  cleanup: &Reconciliation<'_>,
) -> std::io::Result<()> {
  cleanup.remove_covers(prior, &pending.songs)?;

  match pending.mode {
    Mode::Tags => {}
    Mode::Audio => {
      loudness::prune_removed()?;
      let Ok(findings) = review::compare(&pending.songs, &pending.audio);
      let Ok(()) = review::save(cache, &findings);
      let Ok(()) = suggestions::local(cache, &pending.songs, &pending.audio);
      let Ok(albums) = albums::groups(&pending.songs);

      let Ok(()) = album_art::local(&albums);
    }
  }

  let Ok(catalog) = cleanup.catalog(&pending.songs);
  wrote(at, catalog)
}

fn index(mode: Mode) -> std::io::Result<()> {
  let _scan = match mode {
    Mode::Tags => None,
    Mode::Audio => {
      let scan = loudness::BackgroundScan::start()?;

      Some(scan)
    }
  };
  let Ok(configured) = console_core_places::Base::Cache.user();
  let root =
    configured.ok_or_else(|| std::io::Error::other("Music Library cache is unavailable"))?;
  let Ok(at) = looking::at(&root);
  let cache = at
    .parent()
    .ok_or_else(|| std::io::Error::other("Music Library cache has no parent"))?;
  let _gate = looking::lock_catalog(cache)?;
  let Ok(known) = looking::load(&at);
  let Ok(music) = library::folder();
  let Ok(songs) = looking::songs(&music, &library::things, &known);
  let Ok(cleanup) = Reconciliation::observe(&known, &songs);
  let initial = (songs, mode);
  let Ok(start) = Index::initial_transition(&initial, None);
  let done = console_core_iteration::iterate(start, |Transition { state, effects }| {
    use console_core_iteration::Step;

    let mut next = None;

    for effect in effects {
      match effect {
        Effect::RemoveTracks => match cleanup.remove_tracks(cache, &initial.0) {
          Ok(()) => {}
          Err(fault) => return Ok(Step::Halt(Err(fault))),
        },
        Effect::Read(song, how) => {
          let Ok(observed) = measured(song, cache, how);
          next = Some(observed);
        }
        Effect::Checkpoint(visited) => {
          let Ok(checkpoint) = cleanup.checkpoint(&visited);

          match wrote(&at, checkpoint) {
            Ok(()) => {}
            Err(fault) => return Ok(Step::Halt(Err(fault))),
          }
        }
        Effect::Publish(pending) => {
          return Ok(Step::Halt(publish(&at, cache, pending, &known, &cleanup)));
        }
      }
    }

    match next {
      Some(request) => {
        let Ok(transition) = Index::transition(state, request);

        Ok(Step::Again(transition))
      }
      None => Ok(Step::Halt(Ok(()))),
    }
  })
  .map_err(std::io::Error::other)?;

  done
}

fn run(words: &[String]) -> std::io::Result<ExitCode> {
  match words {
    [] => index(Mode::Tags).map(|()| ExitCode::SUCCESS),
    [flag] if flag == "--analyze" => index(Mode::Audio).map(|()| ExitCode::SUCCESS),
    [flag, wanted] if flag == "--suggest" => {
      let query = serde_json::from_str(wanted).map_err(std::io::Error::other)?;

      suggestions::search(&query).map(|()| ExitCode::SUCCESS)
    }
    [flag, wanted] if flag == "--cover" => {
      let album = serde_json::from_str(wanted).map_err(std::io::Error::other)?;
      let cover = album_art::fetch(&album)?;

      Ok(match cover {
        album_art::Cover::Saved => ExitCode::SUCCESS,
        album_art::Cover::Unavailable => ExitCode::from(2),
      })
    }
    _ => Err(std::io::Error::new(
      std::io::ErrorKind::InvalidInput,
      "expected no arguments, --analyze, --cover LOOKUP, or --suggest RECORDING",
    )),
  }
}

fn main() -> ExitCode {
  let words: Vec<_> = std::env::args().skip(1).collect();

  match run(&words) {
    Ok(code) => code,
    Err(fault) => {
      console_core_logging::warn!("music-index: {fault}");

      ExitCode::FAILURE
    }
  }
}
