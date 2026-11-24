use std::time::Duration;

use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};

use super::{Admission, Backend, Completion, ComputeId, Device, History, RoutingPolicy};

pub(super) struct Route;

#[derive(Default)]
pub(super) struct Profile {
  pub(super) history: History,
  active: Option<Active>,
}

#[derive(Clone, Copy)]
pub(super) enum Intent {
  Automatic(Device),
  Verify,
}

pub(super) struct Start {
  pub(super) id: ComputeId,
  pub(super) policy: RoutingPolicy,
  pub(super) intent: Intent,
}

struct Active {
  id: ComputeId,
  policy: RoutingPolicy,
  completion: Completion,
  mode: Mode,
}

impl Active {
  fn advance_verification(mut self, next: Backend) -> Result<Self, Never> {
    self.mode = Mode::Verify(next);
    Ok(self)
  }

  fn fallback(mut self, cpu: Backend) -> Result<Self, Never> {
    let Ok(()) = self.completion.fallback(cpu);
    self.mode = Mode::Fallback;
    Ok(self)
  }
}

#[derive(Clone, Copy)]
enum Mode {
  Automatic { preferred: Backend, device: Device },
  Fallback,
  Verify(Backend),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Attempt {
  pub(super) id: ComputeId,
  pub(super) backend: Backend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Effect {
  Attempt(Attempt),
  Publish(Attempt),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Attempt(_) => &[Service::Compute, Service::Clock],
      Self::Publish(_) => &[],
    })
  }
}

pub(super) enum Request {
  Start(Start),
  Replied(Attempt, AttemptResult),
}

pub(super) enum AttemptResult {
  Measured(Duration),
  Failed,
  Disagreed,
}

type Turn = SingleTransition<Profile, Effect>;

impl SingleEffectMachine for Route {
  type Input = ();
  type State = Profile;
  type Request = Request;
  type Effect = Effect;

  fn initialize(_input: &(), previous: Option<Profile>) -> Result<Turn, Never> {
    let profile = match previous {
      Some(profile) => profile,
      None => Profile::default(),
    };
    profile.emit(None)
  }

  fn handle(state: Profile, request: Request) -> Result<Turn, Never> {
    match request {
      Request::Start(start) => state.begin(start),
      Request::Replied(attempt, result) => state.replied(attempt, result),
    }
  }
}

impl Profile {
  fn emit(self, effect: Option<Effect>) -> Result<Turn, Never> {
    Ok(SingleTransition {
      state: self,
      effect,
    })
  }

  pub(super) fn abort(&mut self) -> Result<(), Never> {
    self.active = None;
    Ok(())
  }

  fn begin(mut self, start: Start) -> Result<Turn, Never> {
    match self.active {
      Some(_) => return self.emit(None),
      None => {}
    }

    self.history.turns = self.history.turns.saturating_add(1);
    let (backend, mode) = match start.intent {
      Intent::Verify => (Backend::SerialCpu, Mode::Verify(Backend::SerialCpu)),
      Intent::Automatic(device) => {
        let Ok(available) = self.history.available(start.policy, device);
        let Ok(preferred) = self.history.preferred(available);
        let Ok(selected) = self.history.choose(available, preferred);
        (selected, Mode::Automatic { preferred, device })
      }
    };
    let Ok(completion) = Completion::new(start.id, backend);
    self.attempt(
      Active {
        id: start.id,
        policy: start.policy,
        completion,
        mode,
      },
      backend,
    )
  }

  fn attempt(mut self, active: Active, backend: Backend) -> Result<Turn, Never> {
    let id = active.id;
    self.active = Some(active);
    self.emit(Some(Effect::Attempt(Attempt { id, backend })))
  }

  fn replied(mut self, attempt: Attempt, result: AttemptResult) -> Result<Turn, Never> {
    let active = match self.active.take() {
      Some(active) => active,
      None => return self.emit(None),
    };
    let expected = match active.mode {
      Mode::Verify(backend) => Some(backend),
      Mode::Automatic { .. } | Mode::Fallback => active.completion.pending,
    };
    let current = attempt.id == active.id && Some(attempt.backend) == expected;

    match bool::then_some(current, ()) {
      Some(()) => {}
      None => {
        self.active = Some(active);
        return self.emit(None);
      }
    }

    match result {
      AttemptResult::Measured(elapsed) => self.measured(active, attempt.backend, elapsed),
      AttemptResult::Failed => self.failed(active, attempt.backend),
      AttemptResult::Disagreed => self.disagreed(active, attempt.backend),
    }
  }

  fn measured(
    mut self,
    active: Active,
    backend: Backend,
    elapsed: Duration,
  ) -> Result<Turn, Never> {
    let Ok(()) = self.history.record(backend, elapsed);
    let policy = active.policy;

    match active.mode {
      Mode::Automatic { preferred, device } => {
        let Ok(available) = self.history.available(policy, device);
        let Ok(next) = self.history.preferred(available);

        match (
          bool::then_some(preferred == next, ()),
          bool::then_some(backend == preferred, ()),
        ) {
          (None, _) => {
            let Ok(()) = self.history.reset_probes(policy);
          }
          (Some(()), None) => {
            let Ok(samples) = self.history.samples.get_mut(backend);
            let Ok(()) = samples.probe.advance(self.history.turns, policy);
          }
          (Some(()), Some(())) => {}
        }

        self.publish(active, backend)
      }
      Mode::Fallback => self.publish(active, backend),
      Mode::Verify(checked) => {
        let Ok(samples) = self.history.samples.get_mut(checked);
        samples.mismatch = false;
        let next = match checked {
          Backend::SerialCpu => Backend::Cpu,
          Backend::Cpu => Backend::Vulkan,
          Backend::Vulkan => {
            let Ok(()) = self.history.reset_probes(policy);
            return self.publish(active, Backend::SerialCpu);
          }
        };
        let Ok(verification) = active.advance_verification(next);
        self.attempt(verification, next)
      }
    }
  }

  fn failed(mut self, mut active: Active, backend: Backend) -> Result<Turn, Never> {
    let policy = active.policy;

    match (backend, active.mode) {
      (Backend::Vulkan, _) | (_, Mode::Verify(_)) => {
        let Ok(samples) = self.history.samples.get_mut(backend);
        let Ok(()) = samples.retry.advance(self.history.turns, policy);
      }
      (Backend::SerialCpu | Backend::Cpu, Mode::Automatic { .. } | Mode::Fallback) => {}
    }

    match (backend, active.mode) {
      (Backend::Vulkan, Mode::Automatic { .. }) => {
        let Ok(available) = self.history.available(policy, Device::CpuOnly);
        let Ok(cpu) = self.history.preferred(available);
        let Ok(fallback) = active.fallback(cpu);
        self.attempt(fallback, cpu)
      }
      (Backend::SerialCpu | Backend::Cpu, _)
      | (Backend::Vulkan, Mode::Fallback | Mode::Verify(_)) => {
        let Ok(()) = active.completion.cancel();
        self.emit(None)
      }
    }
  }

  fn disagreed(mut self, mut active: Active, backend: Backend) -> Result<Turn, Never> {
    let Ok(()) = active.completion.cancel();
    let Ok(samples) = self.history.samples.get_mut(backend);
    samples.mismatch = true;
    samples.elapsed.clear();
    self.emit(None)
  }

  fn publish(self, mut active: Active, backend: Backend) -> Result<Turn, Never> {
    let id = active.id;
    let Ok(admission) = active.completion.accept(id, backend);
    let effect = match admission {
      Admission::Accepted => Some(Effect::Publish(Attempt { id, backend })),
      Admission::Stale => None,
    };
    self.emit(effect)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_stale_gpu_reply_and_a_duplicate_cpu_reply_emit_no_effect() {
    let id = ComputeId(16);
    let mut profile = Profile::default();

    for (backend, millis) in [
      (Backend::SerialCpu, 30),
      (Backend::Cpu, 20),
      (Backend::Vulkan, 4),
    ] {
      let Ok(()) = profile
        .history
        .record(backend, Duration::from_millis(millis));
    }

    let policy = RoutingPolicy::default();
    let Ok(()) = profile.history.reset_probes(policy);
    let Ok(started) = Route::handle(
      profile,
      Request::Start(Start {
        id,
        policy,
        intent: Intent::Automatic(Device::Vulkan),
      }),
    );
    let gpu = Attempt {
      id,
      backend: Backend::Vulkan,
    };
    assert_eq!(started.effect, Some(Effect::Attempt(gpu)));
    let Ok(fallback) = Route::handle(started.state, Request::Replied(gpu, AttemptResult::Failed));
    let cpu = Attempt {
      id,
      backend: Backend::Cpu,
    };
    assert_eq!(fallback.effect, Some(Effect::Attempt(cpu)));
    let measured = || AttemptResult::Measured(Duration::from_millis(4));
    let Ok(late) = Route::handle(fallback.state, Request::Replied(gpu, measured()));
    assert_eq!(late.effect, None);
    let Ok(ordinary) = Route::handle(late.state, Request::Replied(cpu, measured()));
    assert_eq!(ordinary.effect, Some(Effect::Publish(cpu)));
    let Ok(duplicate) = Route::handle(ordinary.state, Request::Replied(cpu, measured()));
    assert_eq!(duplicate.effect, None);
  }
}
