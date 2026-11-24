//! An extraction is published by a successful rename. Collisions choose
//! another name, and every terminal path through a staging directory cleans it.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_application_files::paths::ChildName;
use console_application_files::places::Is;
use console_application_files::unzipping::{self, Candidates, Lift};
use console_core_state_machine::{EffectDependencies, Machine, Never, Queue, Service};

pub(super) struct Destination<'a> {
  pub(super) folder: &'a Path,
  pub(super) stem: &'a str,
}

pub(super) struct Extraction<'a>(std::marker::PhantomData<&'a ()>);

pub(super) struct Staging<'a> {
  folder: &'a Path,
  remaining: Candidates<'a>,
  staging: PathBuf,
}

pub(super) struct Publication<'a> {
  staging: Staging<'a>,
  source: PathBuf,
}

pub(super) enum State<'a> {
  Reserving(&'a Path, Candidates<'a>),
  Extracting(Staging<'a>),
  Inspecting(Staging<'a>),
  Publishing(Publication<'a>),
  Cleaning(ExitCode),
  Finished(ExitCode),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Failure {
  Detail(String),
  Full,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Effect {
  Reserve,
  Extract(PathBuf),
  Inspect(PathBuf),
  Publish {
    source: PathBuf,
    destination: PathBuf,
  },
  Notify(Failure),
  Clean(PathBuf),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Reserve
      | Self::Extract(_)
      | Self::Inspect(_)
      | Self::Publish { .. }
      | Self::Clean(_) => &[Service::FileSystem],
      Self::Notify(_) => &[Service::Notifications],
    })
  }
}

pub(super) enum Renamed {
  Published,
  Occupied,
  Failed(String),
}

pub(super) enum Report {
  Reserved(io::Result<PathBuf>),
  Extracted(Result<(), String>),
  Inspected(Vec<(OsString, Is)>),
  Renamed(Renamed),
  Cleaned,
}

impl<'a> Machine for Extraction<'a> {
  type Input = Destination<'a>;
  type State = State<'a>;
  type Request = Report;
  type Effect = Effect;

  fn initialize(
    input: &Destination<'a>,
    _previous: Option<State<'a>>,
    effects: &mut Queue<Effect>,
  ) -> Result<State<'a>, Never> {
    let Ok(remaining) = unzipping::candidates(input.stem);
    let Ok(()) = effects.offer(Effect::Reserve);

    Ok(State::Reserving(input.folder, remaining))
  }

  fn handle(
    state: State<'a>,
    report: Report,
    effects: &mut Queue<Effect>,
  ) -> Result<State<'a>, Never> {
    match (state, report) {
      (State::Reserving(folder, remaining), Report::Reserved(Ok(staging))) => {
        let Ok(()) = effects.offer(Effect::Extract(staging.clone()));

        Ok(State::Extracting(Staging {
          folder,
          remaining,
          staging,
        }))
      }
      (State::Reserving(folder, _), Report::Reserved(Err(fault))) => {
        let Ok(()) = effects.offer(Effect::Notify(Failure::Detail(format!(
          "{}: {fault}",
          folder.display()
        ))));

        Ok(State::Finished(ExitCode::FAILURE))
      }
      (State::Extracting(staging), Report::Extracted(Ok(()))) => {
        let Ok(()) = effects.offer(Effect::Inspect(staging.staging.clone()));

        Ok(State::Inspecting(staging))
      }
      (State::Extracting(staging), Report::Extracted(Err(fault))) => {
        failed(staging.staging, Failure::Detail(fault), effects)
      }
      (State::Inspecting(staging), Report::Inspected(entries)) => {
        publication(staging, &entries, effects)
      }
      (State::Publishing(job), Report::Renamed(Renamed::Occupied)) => publish(job, effects),
      (State::Publishing(job), Report::Renamed(Renamed::Published)) => {
        clean(job.staging.staging, ExitCode::SUCCESS, effects)
      }
      (State::Publishing(job), Report::Renamed(Renamed::Failed(fault))) => {
        failed(job.staging.staging, Failure::Detail(fault), effects)
      }
      (State::Cleaning(code), Report::Cleaned) => Ok(State::Finished(code)),
      (unchanged, _) => Ok(unchanged),
    }
  }
}

fn publication<'a>(
  staging: Staging<'a>,
  entries: &[(OsString, Is)],
  effects: &mut Queue<Effect>,
) -> Result<State<'a>, Never> {
  let Ok(lift) = unzipping::lifting(entries);
  let source = match lift {
    Lift::TheFolderInside(name) => match ChildName::new(&name) {
      Ok(child) => staging.staging.join(child),
      Err(fault) => return failed(staging.staging, Failure::Detail(fault.to_string()), effects),
    },
    Lift::None => staging.staging.clone(),
  };

  publish(Publication { staging, source }, effects)
}

fn publish<'a>(
  mut job: Publication<'a>,
  effects: &mut Queue<Effect>,
) -> Result<State<'a>, Never> {
  let next = job
    .staging
    .remaining
    .find_map(|name| match ChildName::new(&name) {
      Ok(child) => Some(job.staging.folder.join(child)),
      Err(_invalid_name) => None,
    });

  match next {
    Some(destination) => {
      let Ok(()) = effects.offer(Effect::Publish {
        source: job.source.clone(),
        destination,
      });

      Ok(State::Publishing(job))
    }
    None => failed(job.staging.staging, Failure::Full, effects),
  }
}

fn failed<'a>(
  staging: PathBuf,
  failure: Failure,
  effects: &mut Queue<Effect>,
) -> Result<State<'a>, Never> {
  let Ok(()) = effects.offer(Effect::Notify(failure));

  clean(staging, ExitCode::FAILURE, effects)
}

fn clean<'a>(
  staging: PathBuf,
  code: ExitCode,
  effects: &mut Queue<Effect>,
) -> Result<State<'a>, Never> {
  let Ok(()) = effects.offer(Effect::Clean(staging));

  Ok(State::Cleaning(code))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_failed_extraction_cleans_up_without_publishing() {
    let Ok(started) = Extraction::initial_transition(
      &Destination {
        folder: Path::new("/downloads"),
        stem: "example",
      },
      None,
    );
    assert_eq!(started.effects, [Effect::Reserve]);
    let staging = PathBuf::from("/downloads/.unzip-1");
    let Ok(extracting) =
      Extraction::transition(started.state, Report::Reserved(Ok(staging.clone())));
    assert_eq!(extracting.effects, [Effect::Extract(staging.clone())]);
    let Ok(cleaning) = Extraction::transition(
      extracting.state,
      Report::Extracted(Err("bad archive".to_string())),
    );
    assert_eq!(
      cleaning.effects,
      [
        Effect::Notify(Failure::Detail("bad archive".to_string())),
        Effect::Clean(staging)
      ]
    );
    let Ok(done) = Extraction::transition(cleaning.state, Report::Cleaned);
    assert!(matches!(done.state, State::Finished(code) if code == ExitCode::FAILURE));
  }
}
