//! Start the credential checks together. Launch the fingerprint worker on the
//! lock screen's main thread because Linux ties it to its spawning thread.
//! Wait for enrollment before locking when there is no other usable method.

use std::sync::mpsc::{TrySendError, sync_channel};
use std::time::Duration;

use console_concurrency::HostWorkers;
use console_core_parallel_execution::Workers;
use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};
use console_login_window::Availability;
use console_login_window::fingerprint::Verifier;
use console_login_window::password;
use rustix::event::{PollFd, PollFlags, Timespec, poll};

struct Preparation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
  Checking(Availability),
  WaitingForEnrollment,
  Checked(Availability),
}

enum Request {
  Inspected(Availability),
  EnrollmentChecked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
  Inspect(Availability),
  AwaitEnrollment,
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Inspect(_) | Self::AwaitEnrollment => &[Service::Process],
    })
  }
}

impl SingleEffectMachine for Preparation {
  type Input = Availability;
  type State = State;
  type Request = Request;
  type Effect = Effect;

  fn initialize(
    pattern: &Availability,
    _previous: Option<State>,
  ) -> Result<SingleTransition<State, Effect>, Never> {
    Ok(SingleTransition {
      state: State::Checking(*pattern),
      effect: Some(Effect::Inspect(*pattern)),
    })
  }

  fn handle(state: State, request: Request) -> Result<SingleTransition<State, Effect>, Never> {
    Ok(match (state, request) {
      (
        State::Checking(Availability::Unavailable),
        Request::Inspected(Availability::Unavailable),
      ) => SingleTransition {
        state: State::WaitingForEnrollment,
        effect: Some(Effect::AwaitEnrollment),
      },
      (State::Checking(_), Request::Inspected(password)) => SingleTransition {
        state: State::Checked(password),
        effect: None,
      },
      (State::WaitingForEnrollment, Request::EnrollmentChecked) => SingleTransition {
        state: State::Checked(Availability::Unavailable),
        effect: None,
      },
      (State::Checking(_) | State::Checked(_), Request::EnrollmentChecked)
      | (State::WaitingForEnrollment | State::Checked(_), Request::Inspected(_)) => {
        SingleTransition {
          state,
          effect: None,
        }
      }
    })
  }
}

pub fn check(pattern: Availability) -> Result<(Availability, Option<Verifier>), Never> {
  let Ok(first) = Preparation::initialize(&pattern, None);
  let mut fingerprint = None;
  let finished = Preparation::run_effects(first, 2, &mut fingerprint, |context, effect| {
    let Ok(reply) = perform(context, effect);
    reply
  });
  Ok(match finished {
    Ok(State::Checked(password)) => (password, fingerprint),
    Ok(State::Checking(_) | State::WaitingForEnrollment) => {
      console_core_logging::warn!("the unlock method checks did not finish");
      (Availability::Unavailable, None)
    }
    Err(pending) => {
      console_core_logging::warn!("the unlock method check did not run {pending:?}");
      (Availability::Unavailable, None)
    }
  })
}

fn perform(fingerprint: &mut Option<Verifier>, effect: Effect) -> Result<Request, Never> {
  Ok(match effect {
    Effect::Inspect(pattern) => {
      let Ok(verifier) = Verifier::for_session();
      *fingerprint = Some(verifier);
      let Ok(ready) = check_password(pattern);
      Request::Inspected(ready)
    }
    Effect::AwaitEnrollment => {
      match fingerprint {
        Some(verifier) => {
          let Ok(()) = await_enrollment(verifier);
        }
        None => {}
      }

      Request::EnrollmentChecked
    }
  })
}

fn check_password(pattern: Availability) -> Result<Availability, Never> {
  let Ok(enabled) = password::enable_for_lock();

  match (pattern, enabled) {
    (Availability::Unavailable, Availability::Available) => {}
    (_, availability) => return Ok(availability),
  }

  let (send, receive) = sync_channel(1);
  let started = HostWorkers.start("lock-password-status".to_owned(), move || {
    let Ok(availability) = password::configured_for_session();

    match send.try_send(availability) {
      Ok(()) => {}
      Err(TrySendError::Disconnected(_)) => {
        console_core_logging::debug!("the lock stopped waiting for the account password check")
      }
      Err(TrySendError::Full(_)) => {
        console_core_logging::warn!("the account password check already replied")
      }
    }
  });

  let worker = match started {
    Ok(worker) => worker,
    Err(fault) => {
      console_core_logging::warn!("cannot start the account password check: {fault}");
      return Ok(Availability::Unavailable);
    }
  };
  let checked = receive.recv_timeout(Duration::from_secs(2));
  drop(worker);

  Ok(match checked {
    Ok(availability) => availability,
    Err(fault) => {
      console_core_logging::warn!("cannot check the account password: {fault}");
      Availability::Unavailable
    }
  })
}

fn await_enrollment(verifier: &mut Verifier) -> Result<(), Never> {
  const WAIT: Timespec = Timespec {
    tv_sec: 4,
    tv_nsec: 0,
  };
  let Ok(pending) = verifier.fd();
  let descriptor = match pending {
    Some(present) => present,
    None => return Ok(()),
  };
  let mut descriptors = [PollFd::new(&descriptor, PollFlags::IN)];

  match poll(&mut descriptors, Some(&WAIT)) {
    Ok(0) => console_core_logging::debug!("fingerprint enrollment check timed out"),
    Ok(_) => {
      let Ok(_event) = verifier.response();
    }
    Err(fault) => console_core_logging::debug!("cannot wait for fingerprint enrollment: {fault}"),
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_only_unlock_method_is_checked_before_the_compositor_lock() {
    let Ok(first) = Preparation::initialize(&Availability::Unavailable, None);
    let Ok(checked) =
      Preparation::handle(first.state, Request::Inspected(Availability::Unavailable));
    assert_eq!(checked.effect, Some(Effect::AwaitEnrollment));
    let Ok(settled) = Preparation::handle(checked.state, Request::EnrollmentChecked);
    assert_eq!(settled.state, State::Checked(Availability::Unavailable));
    assert_eq!(settled.effect, None);
  }

  #[test]
  fn a_known_fallback_does_not_wait_for_fingerprint_enrollment() {
    let Ok(first) = Preparation::initialize(&Availability::Unavailable, None);
    let Ok(settled) =
      Preparation::handle(first.state, Request::Inspected(Availability::Available));
    assert_eq!(settled.state, State::Checked(Availability::Available));
    assert_eq!(settled.effect, None);
  }
}
