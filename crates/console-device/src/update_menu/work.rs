use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_state_machine::{
  EffectDependencies, Service, SingleEffectMachine, SingleTransition,
};
use console_operating_system_linux::program_lifetime::{ProcessIdentity, forked};
use console_operating_system_linux::program_runtime::commands::finished;
use console_panel::frames::{self, Notice};
use console_program_contract::ExitStatus;
use console_program_lifetime::{Insistence, threads};

use super::{AVAILABLE_UPDATE, PreviousRelease, UpdateError, previous, rollback_command};

#[derive(Clone)]
pub(super) enum CheckResult {
  NotChecked,
  Current,
  Available(String),
  Failed(String),
}

#[derive(Clone)]
pub(super) enum Check {
  Ready { attempt: u64, result: CheckResult },
  Checking(u64),
}

#[derive(Clone)]
pub(super) enum Rollback {
  Checking,
  Available(PreviousRelease),
  Absent,
  Pending,
  Unavailable,
}

#[derive(Clone)]
pub(super) struct View {
  pub check: Check,
  pub rollback: Rollback,
}

enum Request {
  Check(u64),
  Checked(u64, CheckResult),
  Previous(Rollback),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Effect {
  ReadPrevious,
  Check(u64),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::ReadPrevious => &[Service::Process],
      Self::Check(_) => &[Service::Process, Service::Network, Service::FileSystem],
    })
  }
}

struct Menu;

impl SingleEffectMachine for Menu {
  type Input = ();
  type State = View;
  type Request = Request;
  type Effect = Effect;

  fn initialize(
    _input: &(),
    _previous: Option<View>,
  ) -> Result<SingleTransition<View, Effect>, Never> {
    Ok(SingleTransition {
      state: View {
        check: Check::Ready {
          attempt: 0,
          result: CheckResult::NotChecked,
        },
        rollback: Rollback::Checking,
      },
      effect: Some(Effect::ReadPrevious),
    })
  }

  fn handle(state: View, request: Request) -> Result<SingleTransition<View, Effect>, Never> {
    let mut next = state;
    let effect = match request {
      Request::Check(seen) => match next.check {
        Check::Ready { attempt, .. } => match (attempt == seen).then_some(()) {
          Some(()) => {
            let started = attempt.wrapping_add(1);
            next.check = Check::Checking(started);

            Some(Effect::Check(started))
          }
          None => None,
        },
        Check::Checking(_) => None,
      },
      Request::Checked(finished, result) => {
        match next.check {
          Check::Checking(attempt) => match (attempt == finished).then_some(()) {
            Some(()) => next.check = Check::Ready { attempt, result },
            None => {}
          },
          Check::Ready { .. } => {}
        }

        None
      }
      Request::Previous(status) => {
        next.rollback = status;

        None
      }
    };

    Ok(SingleTransition {
      state: next,
      effect,
    })
  }
}

struct Driver {
  state: View,
  initial: Option<Effect>,
  presses: Receiver<u64>,
  children: Vec<ProcessIdentity>,
}

impl Driver {
  fn request(&mut self, request: Request) -> Result<Option<Effect>, Never> {
    let Ok(next) = Menu::handle(self.state.clone(), request);
    self.state = next.state;

    Ok(next.effect)
  }

  fn check_requested(&mut self) -> Result<Option<Effect>, Never> {
    match self.presses.try_recv() {
      Ok(attempt) => self.request(Request::Check(attempt)),
      Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(None),
    }
  }
}

#[derive(Clone)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "successive panel row readers and the two independent effects exchange small snapshots under this lock; the drawing thread only sends presses through the bounded channel"
  )
)]
pub(super) struct Work {
  driver: Arc<Mutex<Option<Driver>>>,
}

impl Work {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "row readers and effect workers need the same update menu state until their individual work ends"
    )
  )]
  pub(super) fn new() -> Result<(Self, SyncSender<u64>, View), Never> {
    let Ok(initial) = Menu::initialize(&(), None);
    let (checking, presses) = sync_channel(1);
    let first = initial.state.clone();
    let work = Self {
      driver: Arc::new(Mutex::new(Some(Driver {
        state: initial.state,
        initial: initial.effect,
        presses,
        children: Vec::new(),
      }))),
    };

    Ok((work, checking, first))
  }

  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "only short state transitions hold this lock; row reads and process workers do their external work after releasing it"
    )
  )]
  fn driver(&self) -> Result<MutexGuard<'_, Option<Driver>>, Never> {
    Ok(match self.driver.lock() {
      Ok(driver) => driver,
      Err(fault) => {
        console_core_logging::warn!("Console Updates: could not read update work: {fault}");
        let mut recovered = fault.into_inner();
        *recovered = None;

        recovered
      }
    })
  }

  pub(super) fn view(&self) -> Result<Option<View>, Never> {
    let (snapshot, initial, requested) = {
      let Ok(mut held) = self.driver();
      let driver = match held.as_mut() {
        Some(working) => working,
        None => return Ok(None),
      };
      let Ok(requested) = driver.check_requested();

      (driver.state.clone(), driver.initial.take(), requested)
    };
    let Ok(()) = self.start(initial);
    let Ok(()) = self.start(requested);

    Ok(Some(snapshot))
  }

  fn start(&self, pending: Option<Effect>) -> Result<(), Never> {
    let effect = match pending {
      Some(effect) => effect,
      None => return Ok(()),
    };
    let work = self.clone();
    let started = thread::Builder::new().spawn(move || {
      let Ok(reply) = work.perform(effect);
      let Ok(()) = work.report(reply);
    });

    match started {
      Ok(thread) => threads::let_go(thread),
      Err(fault) => {
        let message = format!("Could not start an update worker: {fault}");
        console_core_logging::warn!("Console Updates: {message}");

        self.report(match effect {
          Effect::ReadPrevious => Request::Previous(Rollback::Unavailable),
          Effect::Check(attempt) => Request::Checked(attempt, CheckResult::Failed(message)),
        })
      }
    }
  }

  fn report(&self, reply: Request) -> Result<(), Never> {
    {
      let Ok(mut held) = self.driver();

      match held.as_mut() {
        Some(driver) => {
          let Ok(_no_new_work) = driver.request(reply);
        }
        None => return Ok(()),
      }
    }

    frames::tell(Notice::Rows)
  }

  pub(super) fn close(&self) -> Result<(), Never> {
    let open = {
      let Ok(mut held) = self.driver();

      held.take()
    };

    match open {
      Some(driver) => {
        for child in driver.children {
          let Ok(_ended_or_signaled) = child.interrupt(Insistence::Insist);
        }
      }
      None => {}
    }

    Ok(())
  }

  fn perform(&self, effect: Effect) -> Result<Request, Never> {
    Ok(match effect {
      Effect::ReadPrevious => {
        let Ok(mut command) = rollback_command();
        let status = self.capture(&mut command).and_then(|said| previous(&said));
        Request::Previous(match status {
          Ok(found) => found,
          Err(fault) => {
            console_core_logging::warn!(
              "Console Updates: could not check the previous release: {fault}"
            );
            Rollback::Unavailable
          }
        })
      }
      Effect::Check(attempt) => {
        let Ok(mut command) = InternalProgram::Update.command();
        command.arg(AVAILABLE_UPDATE);
        Request::Checked(
          attempt,
          match self.capture(&mut command) {
            Ok(tag) => match tag.as_str() {
              "" => CheckResult::Current,
              _tag => CheckResult::Available(tag),
            },
            Err(fault) => CheckResult::Failed(fault.to_string()),
          },
        )
      }
    })
  }

  fn capture(&self, command: &mut Command) -> Result<String, UpdateError> {
    let process = forked(
      command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped()),
    )?;
    let Ok(identity) = process.identity();

    {
      let Ok(mut held) = self.driver();

      match held.as_mut() {
        Some(driver) => driver.children.push(identity),
        None => return Err(UpdateError::from("the updates panel closed")),
      }
    }

    let completed = finished(process, None);

    {
      let Ok(mut held) = self.driver();

      match held.as_mut() {
        Some(driver) => driver.children.retain(|child| *child != identity),
        None => return Err(UpdateError::from("the updates panel closed")),
      }
    }

    let reply = completed?;

    match reply.status {
      ExitStatus::Success => Ok(reply.output.trim().to_owned()),
      ExitStatus::Failure(_) | ExitStatus::Interrupt => match reply.error_output.trim() {
        "" => Err(UpdateError::from(format!(
          "{command:?} ended before returning its result"
        ))),
        detail => Err(UpdateError::from(detail)),
      },
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn checking_for_updates_can_finish_before_the_previous_release_lookup() {
    let Ok(opened) = Menu::initialize(&(), None);
    assert_eq!(opened.effect, Some(Effect::ReadPrevious));
    assert!(matches!(opened.state.rollback, Rollback::Checking));

    let Ok(checking) = Menu::handle(opened.state, Request::Check(0));
    assert_eq!(checking.effect, Some(Effect::Check(1)));
    assert!(matches!(checking.state.check, Check::Checking(1)));

    let Ok(checked) = Menu::handle(
      checking.state,
      Request::Checked(1, CheckResult::Available("console-v1.2.3".to_owned())),
    );
    assert!(matches!(checked.state.rollback, Rollback::Checking));

    let Ok(loaded) = Menu::handle(checked.state, Request::Previous(Rollback::Absent));
    assert!(matches!(loaded.state.rollback, Rollback::Absent));
    assert!(matches!(
      loaded.state.check,
      Check::Ready { attempt: 1, result: CheckResult::Available(tag) } if tag == "console-v1.2.3"
    ));
  }

  #[test]
  fn duplicate_presses_and_old_replies_cannot_replace_the_current_check() {
    let Ok(opened) = Menu::initialize(&(), None);
    let Ok(checking) = Menu::handle(opened.state, Request::Check(0));
    let Ok(duplicate) = Menu::handle(checking.state, Request::Check(0));
    assert_eq!(duplicate.effect, None);

    let reason = "Could not resolve api.github.com.";
    let Ok(failed) = Menu::handle(
      duplicate.state,
      Request::Checked(1, CheckResult::Failed(reason.to_owned())),
    );
    let Ok(stale_press) = Menu::handle(failed.state, Request::Check(0));
    assert_eq!(stale_press.effect, None);
    assert!(matches!(
      &stale_press.state.check,
      Check::Ready { attempt: 1, result: CheckResult::Failed(fault) } if fault == reason
    ));

    let Ok(retried) = Menu::handle(stale_press.state, Request::Check(1));
    assert_eq!(retried.effect, Some(Effect::Check(2)));
    let Ok(stale_reply) = Menu::handle(retried.state, Request::Checked(1, CheckResult::Current));
    assert!(matches!(stale_reply.state.check, Check::Checking(2)));

    let Ok(checked) = Menu::handle(stale_reply.state, Request::Checked(2, CheckResult::Current));
    assert!(matches!(
      checked.state.check,
      Check::Ready {
        attempt: 2,
        result: CheckResult::Current
      }
    ));
  }
}
