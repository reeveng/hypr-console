//! Bring up the user services after swapping in a generation, then verify them before keeping it.

use std::marker::PhantomData;

use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};

use crate::unapplied::Unapplied;

pub(crate) struct Checked<'a> {
  pub(crate) attempted: Vec<&'a String>,
  pub(crate) failed: Vec<String>,
}

pub(crate) enum State {
  Reloading,
  Starting,
  Checking,
  Restoring,
  Finished(Result<(), Unapplied>),
}

pub(crate) enum Effect<'a> {
  Reload,
  Start,
  Check(Vec<&'a String>),
  Restore(Checked<'a>),
}

impl EffectDependencies for Effect<'_> {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Reload => &[Service::Process],
      Self::Start => &[
        Service::Console,
        Service::FileSystem,
        Service::Input,
        Service::Process,
      ],
      Self::Check(_) => &[Service::Compute, Service::Console, Service::Process],
      Self::Restore(_) => &[Service::Console, Service::FileSystem, Service::Process],
    })
  }
}

pub(crate) enum Report<'a> {
  ReloadAttempted,
  Started(Vec<&'a String>),
  Checked(Checked<'a>),
  Restored(Vec<String>),
}

pub(crate) struct Startup<'a>(PhantomData<&'a ()>);

impl<'a> SingleEffectMachine for Startup<'a> {
  type Input = ();
  type State = State;
  type Request = Report<'a>;
  type Effect = Effect<'a>;

  fn initialize(
    _input: &(),
    _previous: Option<State>,
  ) -> Result<SingleTransition<State, Effect<'a>>, Never> {
    Ok(SingleTransition {
      state: State::Reloading,
      effect: Some(Effect::Reload),
    })
  }

  fn handle(
    state: State,
    report: Report<'a>,
  ) -> Result<SingleTransition<State, Effect<'a>>, Never> {
    let (next, effect) = match (state, report) {
      (State::Reloading, Report::ReloadAttempted) => (State::Starting, Some(Effect::Start)),
      (State::Starting, Report::Started(attempted)) => {
        (State::Checking, Some(Effect::Check(attempted)))
      }
      (State::Checking, Report::Checked(checked)) => match checked.failed.as_slice() {
        [] => (State::Finished(Ok(())), None),
        [_, ..] => (State::Restoring, Some(Effect::Restore(checked))),
      },
      (State::Restoring, Report::Restored(failed)) => {
        (State::Finished(Err(Unapplied::Restore(failed))), None)
      }
      (unchanged, _) => (unchanged, None),
    };

    Ok(SingleTransition {
      state: next,
      effect,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reload_and_start_precede_verification() {
    let Ok(first) = Startup::initialize(&(), None);
    assert!(matches!(first.effect, Some(Effect::Reload)));
    let Ok(start) = Startup::handle(first.state, Report::ReloadAttempted);
    assert!(matches!(start.effect, Some(Effect::Start)));
    let Ok(check) = Startup::handle(start.state, Report::Started(Vec::new()));
    assert!(matches!(check.effect, Some(Effect::Check(_))));
    let Ok(done) = Startup::handle(
      check.state,
      Report::Checked(Checked {
        attempted: Vec::new(),
        failed: Vec::new(),
      }),
    );
    assert!(matches!(done.state, State::Finished(Ok(()))));
    assert!(done.effect.is_none());
  }

  #[test]
  fn failed_verification_restores_every_attempted_unit() {
    let idle = "console-idle.service".to_owned();
    let light = "console-light.service".to_owned();
    let Ok(first) = Startup::initialize(&(), None);
    let mut restored = None;
    let done = Startup::run_effects(
      first,
      4,
      &mut restored,
      |restored_units, effect| match effect {
        Effect::Reload => Report::ReloadAttempted,
        Effect::Start => Report::Started(vec![&idle, &light]),
        Effect::Check(attempted) => Report::Checked(Checked {
          attempted,
          failed: vec![idle.clone()],
        }),
        Effect::Restore(checked) => {
          *restored_units = Some(checked.attempted);

          Report::Restored(checked.failed)
        }
      },
    );

    assert_eq!(restored, Some(vec![&idle, &light]));
    assert!(
      matches!(done, Ok(State::Finished(Err(Unapplied::Restore(failed))))
      if failed.as_slice() == [idle.as_str()])
    );
  }
}
