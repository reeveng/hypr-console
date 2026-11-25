//! Collect local identifiers, then scan the exported tree for their use.

use std::path::{Path, PathBuf};

use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};
use console_manifest_publish::names::{self, Watched};

use crate::{Unpublished, files_naming};

pub(super) struct Report {
  pub missing: Vec<String>,
  pub findings: Findings,
}

type Findings = Vec<(PathBuf, &'static str)>;
type Turn = SingleTransition<State, Effect>;

pub(super) fn scan(
  repository: &Path,
  export: &Path,
  additional: Vec<Watched>,
) -> Result<Report, Unpublished> {
  let Ok(first) = Privacy::initialize(&(), None);
  let mut inputs = Inputs {
    repository,
    export,
    additional,
  };

  match Privacy::run_effects(first, 2, &mut inputs, |context, effect| {
    let Ok(reply) = perform(context, effect);

    reply
  }) {
    Ok(State::Complete(report)) => report,
    Ok(State::Collecting | State::Scanning(_)) => Err(Unpublished::PrivacyCheckIncomplete),
    Err(_unperformed_effect) => Err(Unpublished::PrivacyCheckIncomplete),
  }
}

struct Privacy;

enum State {
  Collecting,
  Scanning(Vec<String>),
  Complete(Result<Report, Unpublished>),
}

enum Request {
  Collected {
    identifiers: Vec<Watched>,
    missing: Vec<String>,
  },
  Scanned(Result<Findings, Unpublished>),
}

enum Effect {
  Collect,
  Scan(Vec<Watched>),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Collect => &[Service::Process],
      Self::Scan(_) => &[Service::FileSystem],
    })
  }
}

impl SingleEffectMachine for Privacy {
  type Input = ();
  type State = State;
  type Request = Request;
  type Effect = Effect;

  fn initialize(_input: &(), _previous: Option<State>) -> Result<Turn, Never> {
    Ok(Turn {
      state: State::Collecting,
      effect: Some(Effect::Collect),
    })
  }

  fn handle(current: State, request: Request) -> Result<Turn, Never> {
    Ok(match (current, request) {
      (
        State::Collecting,
        Request::Collected {
          identifiers,
          missing,
        },
      ) => Turn {
        state: State::Scanning(missing),
        effect: Some(Effect::Scan(identifiers)),
      },
      (State::Scanning(missing), Request::Scanned(result)) => Turn {
        state: State::Complete(result.map(|findings| Report { missing, findings })),
        effect: None,
      },
      (state, _) => Turn {
        state,
        effect: None,
      },
    })
  }
}

struct Inputs<'a> {
  repository: &'a Path,
  export: &'a Path,
  additional: Vec<Watched>,
}

fn perform(inputs: &mut Inputs<'_>, effect: Effect) -> Result<Request, Never> {
  Ok(match effect {
    Effect::Collect => {
      let Ok((mut identifiers, missing)) = names::forbidden_names(inputs.repository);

      identifiers.append(&mut inputs.additional);
      Request::Collected {
        identifiers,
        missing,
      }
    }
    Effect::Scan(names) => Request::Scanned(files_naming(inputs.export, &names)),
  })
}
