//! Resolve a file revision, then its encoded audio, before decoding for analysis.

mod identity;
mod owners;

use std::fmt::Write;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_external_programs::Program;
use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};

use super::{FileId, Measurement, Request, Revision, cached_file, revision, scan};
use identity::AudioKey;

pub(super) use owners::{prune_old_revisions, prune_removed};

#[derive(Clone, Copy)]
pub(super) enum Work {
  Cached,
  Reuse,
  Measure,
}

enum State {
  CheckingRevision(Work),
  Identifying(Work, Option<Measurement>),
  CheckingContent(Work, Option<Measurement>),
  Measuring(Option<AudioKey>),
  Saving,
  Done(io::Result<Option<Measurement>>),
}

enum Effect {
  ReadRevision,
  Identify,
  ReadContent(AudioKey),
  Measure,
  Save(Option<AudioKey>, Measurement),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::ReadRevision | Self::ReadContent(..) | Self::Save(..) => &[Service::FileSystem],
      Self::Identify | Self::Measure => &[Service::FileSystem, Service::Process],
    })
  }
}

enum Reply {
  Revision(RevisionHit),
  Identified(io::Result<Option<AudioKey>>),
  Content(AudioKey, Option<Measurement>),
  Measured(io::Result<Measurement>),
  Saved(io::Result<Option<Measurement>>),
}

enum RevisionHit {
  Current(Measurement),
  Missing(Option<Measurement>),
  Uncached,
}

struct Index;

type Turn = SingleTransition<State, Effect>;

impl SingleEffectMachine for Index {
  type Input = Work;
  type State = State;
  type Request = Reply;
  type Effect = Effect;

  fn initialize(work: &Work, _previous: Option<State>) -> Result<Turn, Never> {
    Ok(Turn {
      state: State::CheckingRevision(*work),
      effect: Some(Effect::ReadRevision),
    })
  }

  fn handle(state: State, reply: Reply) -> Result<Turn, Never> {
    let (next, effect) = match (state, reply) {
      (State::CheckingRevision(_), Reply::Revision(RevisionHit::Current(saved))) => {
        (State::Done(Ok(Some(saved))), None)
      }
      (State::CheckingRevision(Work::Cached), Reply::Revision(RevisionHit::Missing(legacy))) => {
        (State::Done(Ok(legacy)), None)
      }
      (State::CheckingRevision(work), Reply::Revision(RevisionHit::Missing(legacy))) => {
        (State::Identifying(work, legacy), Some(Effect::Identify))
      }
      (State::CheckingRevision(Work::Measure), Reply::Revision(RevisionHit::Uncached)) => {
        (State::Measuring(None), Some(Effect::Measure))
      }
      (
        State::CheckingRevision(Work::Cached | Work::Reuse),
        Reply::Revision(RevisionHit::Uncached),
      ) => (State::Done(Ok(None)), None),
      (State::Identifying(work, legacy), Reply::Identified(Ok(Some(key)))) => (
        State::CheckingContent(work, legacy),
        Some(Effect::ReadContent(key)),
      ),
      (State::Identifying(work, legacy), Reply::Identified(Ok(None))) => {
        return on_miss(work, None, legacy);
      }
      (State::Identifying(..), Reply::Identified(Err(fault))) => (State::Done(Err(fault)), None),
      (State::CheckingContent(_, _), Reply::Content(key, Some(saved))) => {
        (State::Saving, Some(Effect::Save(Some(key), saved)))
      }
      (State::CheckingContent(work, legacy), Reply::Content(key, None)) => {
        return on_miss(work, Some(key), legacy);
      }
      (State::Measuring(key), Reply::Measured(Ok(measured))) => {
        (State::Saving, Some(Effect::Save(key, measured)))
      }
      (State::Measuring(_), Reply::Measured(Err(fault))) => (State::Done(Err(fault)), None),
      (State::Saving, Reply::Saved(saved)) => (State::Done(saved), None),
      (unchanged, _) => (unchanged, None),
    };

    Ok(Turn {
      state: next,
      effect,
    })
  }
}

fn on_miss(work: Work, key: Option<AudioKey>, saved: Option<Measurement>) -> Result<Turn, Never> {
  let (state, effect) = match (work, saved) {
    (_, Some(measured)) => (State::Saving, Some(Effect::Save(key, measured))),
    (Work::Measure, None) => (State::Measuring(key), Some(Effect::Measure)),
    (Work::Cached | Work::Reuse, None) => (State::Done(Ok(None)), None),
  };

  Ok(Turn { state, effect })
}

struct Source {
  path: PathBuf,
  file: FileId,
  stamp: Revision,
}

enum Stability {
  Current,
  Changed,
}

impl Source {
  fn open(path: PathBuf) -> io::Result<Option<Self>> {
    let metadata = path.metadata()?;
    let Ok(stamp) = revision(&metadata);

    Ok(metadata.is_file().then(|| Self {
      path,
      stamp,
      file: FileId::from(&metadata),
    }))
  }

  fn check(&self) -> io::Result<Stability> {
    let after = self.path.metadata()?;
    let Ok(stamp) = revision(&after);
    let file = FileId::from(&after);

    Ok(
      match (self.file == file && self.stamp == stamp).then_some(()) {
        Some(()) => Stability::Current,
        None => Stability::Changed,
      },
    )
  }
}

struct Cache<'a> {
  root: &'a Path,
  analyser: Revision,
  revision: Revision,
}

impl<'a> Cache<'a> {
  fn open(root: &'a Path, song: &Source, executable: &Path) -> io::Result<Self> {
    let metadata = executable.metadata()?;
    let Ok(analyser) = revision(&metadata);
    let revision = Revision(format!("{} {}", song.stamp.0, analyser.0));

    Ok(Self {
      root,
      analyser,
      revision,
    })
  }

  fn audio(&self, key: &AudioKey) -> Result<PathBuf, Never> {
    Ok(self.root.join("encoded-v1").join(key))
  }

  fn save_content(&self, key: &AudioKey, measured: &Measurement) -> Result<(), Never> {
    let Ok(audio) = self.audio(key);
    let Ok(retained) = cached_file(&audio, &self.analyser);

    match retained {
      Some(_) => Ok(()),
      None => {
        let Ok(text) = encode(&self.analyser, measured);
        save(&audio, &text)
      }
    }
  }

  fn revision_hit(&self, song: &Source) -> Result<RevisionHit, Never> {
    let Ok(found) = owners::cached_at(self.root, song, &self.revision);

    Ok(match found {
      Some(saved) => RevisionHit::Current(saved),
      None => {
        let Ok(older) = self.legacy(song);
        RevisionHit::Missing(older)
      }
    })
  }

  fn legacy(&self, song: &Source) -> Result<Option<Measurement>, Never> {
    let Ok(name) = song.file.name();
    let inode_record = self.root.join("revisions").join(&name);
    let Ok(measured) = cached_file(&inode_record, &self.revision);

    Ok(match measured {
      Some(saved) => Some(saved),
      None => match inode_record.metadata() {
        Ok(_) => None,
        Err(fault) => match (fault.kind() == ErrorKind::NotFound).then_some(()) {
          Some(()) => {
            let Ok(saved) = cached_file(&self.root.join(name), &song.stamp);
            saved
          }
          None => None,
        },
      },
    })
  }
}

struct Driver<'a, Wanted> {
  song: Source,
  cache: Option<Cache<'a>>,
  analyser: Command,
  wanted: Wanted,
}

impl<Wanted: Fn() -> Request> Driver<'_, Wanted> {
  fn perform(&mut self, effect: Effect) -> Result<Reply, Never> {
    Ok(match effect {
      Effect::ReadRevision => {
        let found = match &self.cache {
          Some(cache) => {
            let Ok(found) = cache.revision_hit(&self.song);
            found
          }
          None => RevisionHit::Uncached,
        };

        Reply::Revision(found)
      }
      Effect::Identify => Reply::Identified(self.identify()),
      Effect::ReadContent(key) => {
        let saved = self.cache.as_ref().and_then(|cache| {
          let Ok(path) = cache.audio(&key);
          let Ok(found) = cached_file(&path, &cache.analyser);
          found
        });

        Reply::Content(key, saved)
      }
      Effect::Measure => Reply::Measured(scan(&mut self.analyser, &self.song.path, &self.wanted)),
      Effect::Save(key, measured) => Reply::Saved(self.save(key, measured)),
    })
  }

  fn identify(&self) -> io::Result<Option<AudioKey>> {
    match identity::of(&self.song.path, &self.wanted) {
      Ok(key) => Ok(key),
      Err(fault) => match (fault.kind() == ErrorKind::Interrupted).then_some(()) {
        Some(()) => Err(fault),
        None => {
          console_core_logging::debug!(
            "could not identify audio in {}: {fault}",
            self.song.path.display()
          );
          Ok(None)
        }
      },
    }
  }

  fn save(
    &self,
    key: Option<AudioKey>,
    measured: Measurement,
  ) -> io::Result<Option<Measurement>> {
    let _guard = match &self.cache {
      Some(cache) => {
        let locked = owners::lock(cache.root)?;
        Some(locked)
      }
      None => None,
    };
    let stability = self.song.check()?;

    match stability {
      Stability::Current => {}
      Stability::Changed => return Ok(None),
    }

    match &self.cache {
      Some(cache) => {
        let metadata = Path::new(self.analyser.get_program()).metadata()?;
        let Ok(current) = revision(&metadata);

        match (current == cache.analyser).then_some(()) {
          Some(()) => {}
          None => return Ok(None),
        }

        match &key {
          Some(identified) => {
            let Ok(()) = cache.save_content(identified, &measured);
          }
          None => {}
        }

        let Ok(text) = encode(&cache.revision, &measured);
        let Ok(file) = owners::at(cache.root, &self.song.path);
        let Ok(record) = owners::encode(&self.song, key.as_ref(), &text);
        let Ok(()) = save(&file, &record);
      }
      None => {}
    }

    Ok(Some(measured))
  }
}

fn encode(stamp: &Revision, measured: &Measurement) -> Result<String, Never> {
  let mut record = format!(
    "{}\n{} {}\n",
    stamp.0, measured.lufs.0, measured.peak_dbtp.0
  );

  for (bin, count) in &measured.moments {
    let _written = writeln!(record, "{bin} {count}");
  }

  Ok(record)
}

fn save(path: &Path, record: &str) -> Result<(), Never> {
  match console_core_atomic_writes::whole_with_folders(path, record.as_bytes()) {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("could not save the audio loudness index: {fault}"),
  }

  Ok(())
}

pub(super) fn read(
  song: &Path,
  root: Option<&Path>,
  work: Work,
  wanted: impl Fn() -> Request,
) -> io::Result<Option<Measurement>> {
  let path = std::path::absolute(song)?;
  let opened = Source::open(path)?;
  let source = match opened {
    Some(opened_source) => opened_source,
    None => return Ok(None),
  };
  let analyser = Program::Ffmpeg.provide()?;
  let cache = match root {
    Some(at) => {
      let cache = Cache::open(at, &source, Path::new(analyser.get_program()))?;
      Some(cache)
    }
    None => None,
  };
  let mut driver = Driver {
    song: source,
    cache,
    analyser,
    wanted,
  };
  let Ok(first) = Index::initialize(&work, None);

  match Index::run_effects(first, 5, &mut driver, |context, effect| {
    let Ok(reply) = context.perform(effect);
    reply
  }) {
    Ok(State::Done(result)) => result,
    Ok(_) | Err(_) => Err(io::Error::other("audio index did not settle")),
  }
}
