//! Program preparation returns artifacts before the device stages files. The
//! same staging path accepts local builds and artifacts supplied by a release.

use std::marker::PhantomData;
use std::path::PathBuf;

use console_core_state_machine::{EffectDependencies, Machine, Never, Queue, Service};

use crate::{Unapplied, build::Programs, manifest::BuildName};

pub(crate) struct Input<'a> {
  pub(crate) programs: Programs,
  pub(crate) names: &'a [BuildName<'a>],
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Artifact {
  pub(crate) built: PathBuf,
  pub(crate) live: String,
}

pub(crate) enum State {
  Preparing,
  Staging,
  Finished(Result<Vec<String>, Unapplied>),
}

pub(crate) enum Effect<'a> {
  Build(&'a [BuildName<'a>]),
  UsePrebuilt(&'a [BuildName<'a>]),
  Stage(Vec<Artifact>),
}

impl EffectDependencies for Effect<'_> {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Build(_) => &[Service::Process, Service::FileSystem],
      Self::UsePrebuilt(_) | Self::Stage(_) => &[Service::FileSystem],
    })
  }
}

pub(crate) enum Report {
  Prepared(Result<Vec<Artifact>, Unapplied>),
  Staged(Result<Vec<String>, Unapplied>),
}

pub(crate) struct ProgramStaging<'a>(PhantomData<&'a ()>);

impl<'a> Machine for ProgramStaging<'a> {
  type Input = Input<'a>;
  type State = State;
  type Request = Report;
  type Effect = Effect<'a>;

  fn initialize(
    input: &Input<'a>,
    _previous: Option<State>,
    effects: &mut Queue<Effect<'a>>,
  ) -> Result<State, Never> {
    match input.names {
      [] => Ok(State::Finished(Ok(Vec::new()))),
      names => {
        let prepare = match input.programs {
          Programs::Compile => Effect::Build(names),
          Programs::Prebuilt => Effect::UsePrebuilt(names),
        };
        let Ok(()) = effects.offer(prepare);

        Ok(State::Preparing)
      }
    }
  }

  fn handle(
    state: State,
    reply: Report,
    effects: &mut Queue<Effect<'a>>,
  ) -> Result<State, Never> {
    Ok(match (state, reply) {
      (State::Preparing, Report::Prepared(Ok(artifacts))) => {
        let Ok(()) = effects.offer(Effect::Stage(artifacts));

        State::Staging
      }
      (State::Preparing, Report::Prepared(Err(fault))) => State::Finished(Err(fault)),
      (State::Staging, Report::Staged(result)) => State::Finished(result),
      (unchanged, _) => unchanged,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn compiled_and_prebuilt_programs_use_the_same_staging_effect() -> Result<(), Unapplied> {
    let name = BuildName::parse("console")?;
    let names = [name];

    for programs in [Programs::Compile, Programs::Prebuilt] {
      let Ok(first) = ProgramStaging::initial_transition(
        &Input {
          programs,
          names: &names,
        },
        None,
      );
      assert!(matches!(
        (programs, first.effects.as_slice()),
        (Programs::Compile, [Effect::Build(_)]) | (Programs::Prebuilt, [Effect::UsePrebuilt(_)])
      ));

      let artifacts = vec![Artifact {
        built: PathBuf::from("target/release/console"),
        live: "/usr/local/bin/console".to_string(),
      }];
      let Ok(prepared) = ProgramStaging::transition(first.state, Report::Prepared(Ok(artifacts)));
      assert!(
        matches!(prepared.effects.as_slice(), [Effect::Stage(staged)] if staged.len() == 1)
      );

      let Ok(complete) = ProgramStaging::transition(
        prepared.state,
        Report::Staged(Ok(vec!["/usr/local/bin/console".to_string()])),
      );
      assert!(
        matches!(complete.state, State::Finished(Ok(written)) if written == ["/usr/local/bin/console"])
      );
    }

    Ok(())
  }

  #[test]
  fn a_failed_build_cannot_stage_artifacts() -> Result<(), Unapplied> {
    let name = BuildName::parse("console")?;
    let names = [name];
    let Ok(first) = ProgramStaging::initial_transition(
      &Input {
        programs: Programs::Compile,
        names: &names,
      },
      None,
    );
    let Ok(failed) =
      ProgramStaging::transition(first.state, Report::Prepared(Err(Unapplied::CargoRefused)));

    assert!(failed.effects.is_empty());
    assert!(matches!(
      failed.state,
      State::Finished(Err(Unapplied::CargoRefused))
    ));

    Ok(())
  }
}
