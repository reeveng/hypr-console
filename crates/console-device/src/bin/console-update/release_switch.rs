//! Activating a release can fail before or after the live checkout changes.
//! Once restoration is recorded, another invocation starts with restoration.

use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};

use super::UpdateError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Direction {
  Apply,
  Restore,
}

pub(super) enum State {
  Activating,
  Verifying(Verification),
  CheckingFailure(UpdateError),
  RecordingRestore(UpdateError),
  Restoring(Option<UpdateError>),
  Finalizing(Option<UpdateError>),
  Finished(Result<(), UpdateError>),
}

pub(super) enum Verification {
  Activation,
  Restoration(Option<UpdateError>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Finalize {
  Activated,
  Restored,
  FailedActivation,
}

pub(super) enum LiveRevision {
  Requested,
  FirstRelease,
  Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SwitchEffect {
  Activate,
  Verify,
  ReadLiveRevision,
  RecordRestore,
  Restore,
  Finalize(Finalize),
}

impl EffectDependencies for SwitchEffect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Activate | Self::Restore => &[Service::Process, Service::FileSystem],
      Self::Verify | Self::ReadLiveRevision => &[Service::Process],
      Self::RecordRestore => &[Service::Process, Service::Console, Service::FileSystem],
      Self::Finalize(_) => &[Service::Process, Service::FileSystem],
    })
  }
}

pub(super) enum Report {
  Activated(Result<(), UpdateError>),
  Verified(Result<(), UpdateError>),
  Observed(LiveRevision),
  RestoreRecorded,
  Restored,
  Finalized,
  Failed(UpdateError),
}

pub(super) struct ReleaseSwitch;

impl SingleEffectMachine for ReleaseSwitch {
  type Input = Direction;
  type State = State;
  type Request = Report;
  type Effect = SwitchEffect;

  fn initialize(
    direction: &Direction,
    _previous: Option<State>,
  ) -> Result<SingleTransition<State, SwitchEffect>, Never> {
    let (next, effect) = match direction {
      Direction::Apply => (State::Activating, SwitchEffect::Activate),
      Direction::Restore => (State::Restoring(None), SwitchEffect::Restore),
    };
    Ok(SingleTransition {
      state: next,
      effect: Some(effect),
    })
  }

  fn handle(
    state: State,
    report: Report,
  ) -> Result<SingleTransition<State, SwitchEffect>, Never> {
    let (next, effect) = match (state, report) {
      (State::Finished(result), _) => (State::Finished(result), None),
      (_, Report::Failed(fault)) => (State::Finished(Err(fault)), None),
      (State::Activating, Report::Activated(Ok(()))) => (
        State::Verifying(Verification::Activation),
        Some(SwitchEffect::Verify),
      ),
      (State::Verifying(Verification::Activation), Report::Verified(Ok(()))) => (
        State::Finalizing(None),
        Some(SwitchEffect::Finalize(Finalize::Activated)),
      ),
      (State::Activating, Report::Activated(Err(fault)))
      | (State::Verifying(Verification::Activation), Report::Verified(Err(fault))) => (
        State::CheckingFailure(fault),
        Some(SwitchEffect::ReadLiveRevision),
      ),
      (State::CheckingFailure(fault), Report::Observed(LiveRevision::Requested)) => (
        State::RecordingRestore(fault),
        Some(SwitchEffect::RecordRestore),
      ),
      (State::CheckingFailure(fault), Report::Observed(LiveRevision::Other)) => (
        State::Finalizing(Some(fault)),
        Some(SwitchEffect::Finalize(Finalize::FailedActivation)),
      ),
      (State::CheckingFailure(fault), Report::Observed(LiveRevision::FirstRelease)) => {
        let detail = format!("{fault}. Run sudo console-update to resume the first release.");
        (State::Finished(Err(UpdateError::from(detail))), None)
      }
      (State::RecordingRestore(fault), Report::RestoreRecorded) => {
        (State::Restoring(Some(fault)), Some(SwitchEffect::Restore))
      }
      (State::Restoring(original), Report::Restored) => (
        State::Verifying(Verification::Restoration(original)),
        Some(SwitchEffect::Verify),
      ),
      (State::Verifying(Verification::Restoration(_)), Report::Verified(Err(fault))) => {
        (State::Finished(Err(fault)), None)
      }
      (State::Verifying(Verification::Restoration(original)), Report::Verified(Ok(()))) => {
        let finalize = match &original {
          Some(_) => Finalize::FailedActivation,
          None => Finalize::Restored,
        };

        (
          State::Finalizing(original),
          Some(SwitchEffect::Finalize(finalize)),
        )
      }
      (State::Finalizing(original), Report::Finalized) => {
        let result = match original {
          Some(fault) => Err(fault),
          None => Ok(()),
        };
        (State::Finished(result), None)
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
  use crate::{PENDING, Switch, slot_path, switch_effect};

  #[test]
  fn a_failed_activation_records_restoration_before_restoring() {
    let Ok(first) = ReleaseSwitch::initialize(&Direction::Apply, None);
    assert_eq!(first.effect, Some(SwitchEffect::Activate));

    let Ok(verify) = ReleaseSwitch::handle(first.state, Report::Activated(Ok(())));
    assert_eq!(verify.effect, Some(SwitchEffect::Verify));
    let failure = UpdateError::from("health failed");
    let Ok(check) = ReleaseSwitch::handle(verify.state, Report::Verified(Err(failure)));
    assert_eq!(check.effect, Some(SwitchEffect::ReadLiveRevision));

    let Ok(record) =
      ReleaseSwitch::handle(check.state, Report::Observed(LiveRevision::Requested));
    assert_eq!(record.effect, Some(SwitchEffect::RecordRestore));

    let Ok(restore) = ReleaseSwitch::handle(record.state, Report::RestoreRecorded);
    assert_eq!(restore.effect, Some(SwitchEffect::Restore));

    let Ok(verify_restore) = ReleaseSwitch::handle(restore.state, Report::Restored);
    assert_eq!(verify_restore.effect, Some(SwitchEffect::Verify));
    let Ok(cleanup) = ReleaseSwitch::handle(verify_restore.state, Report::Verified(Ok(())));
    assert_eq!(
      cleanup.effect,
      Some(SwitchEffect::Finalize(Finalize::FailedActivation))
    );

    let Ok(done) = ReleaseSwitch::handle(cleanup.state, Report::Finalized);
    assert!(
      matches!(done.state, State::Finished(Err(fault)) if fault.to_string() == "health failed")
    );
  }

  #[test]
  fn resuming_a_recorded_restore_does_not_try_activation_again() -> Result<(), UpdateError> {
    let releases = console_core_temporary_directories::private("console-update-restore")?;
    let mut change = Switch {
      slot: "release-1-2".to_string(),
      before: Some("a".repeat(40)),
      after: "b".repeat(40),
      direction: Direction::Apply,
    };
    change.record(&releases)?;
    let other = slot_path(&releases, &change.slot)?;
    let report = switch_effect(
      &releases.join("live"),
      &releases,
      &other,
      &mut change,
      SwitchEffect::RecordRestore,
    )?;
    assert!(matches!(report, Report::RestoreRecorded));

    let raw = std::fs::read_to_string(releases.join(PENDING))?;
    let pending = Switch::read(&raw)?;
    let Ok(first) = ReleaseSwitch::initialize(&pending.direction, None);
    assert_eq!(first.effect, Some(SwitchEffect::Restore));

    let Ok(verify) = ReleaseSwitch::handle(first.state, Report::Restored);
    assert_eq!(verify.effect, Some(SwitchEffect::Verify));
    let Ok(cleanup) = ReleaseSwitch::handle(verify.state, Report::Verified(Ok(())));
    assert_eq!(
      cleanup.effect,
      Some(SwitchEffect::Finalize(Finalize::Restored))
    );

    let Ok(done) = ReleaseSwitch::handle(cleanup.state, Report::Finalized);
    assert!(matches!(done.state, State::Finished(Ok(()))));

    std::fs::remove_dir_all(releases)?;

    Ok(())
  }

  #[test]
  fn a_failure_before_activation_only_cleans_up() {
    let Ok(first) = ReleaseSwitch::initialize(&Direction::Apply, None);
    let failure = UpdateError::from("engine failed");
    let Ok(check) = ReleaseSwitch::handle(first.state, Report::Activated(Err(failure)));
    let Ok(cleanup) = ReleaseSwitch::handle(check.state, Report::Observed(LiveRevision::Other));

    assert_eq!(
      cleanup.effect,
      Some(SwitchEffect::Finalize(Finalize::FailedActivation))
    );
  }

  #[test]
  fn a_failed_first_release_after_activation_stays_pending_for_a_retry() {
    let Ok(first) = ReleaseSwitch::initialize(&Direction::Apply, None);
    let Ok(verify) = ReleaseSwitch::handle(first.state, Report::Activated(Ok(())));
    let Ok(check) = ReleaseSwitch::handle(
      verify.state,
      Report::Verified(Err(UpdateError::from("health failed"))),
    );
    assert_eq!(check.effect, Some(SwitchEffect::ReadLiveRevision));

    let Ok(retry) =
      ReleaseSwitch::handle(check.state, Report::Observed(LiveRevision::FirstRelease));
    assert!(retry.effect.is_none());
    assert!(
      matches!(retry.state, State::Finished(Err(fault)) if fault.to_string().contains("resume the first release"))
    );
  }

  #[test]
  fn an_unhealthy_restore_leaves_the_pending_record_in_place() {
    let Ok(first) = ReleaseSwitch::initialize(&Direction::Restore, None);
    let Ok(verify) = ReleaseSwitch::handle(first.state, Report::Restored);
    let failure = UpdateError::from("restore failed");
    let Ok(failed) = ReleaseSwitch::handle(verify.state, Report::Verified(Err(failure)));

    assert!(failed.effect.is_none());
    assert!(
      matches!(failed.state, State::Finished(Err(fault)) if fault.to_string() == "restore failed")
    );
  }
}
