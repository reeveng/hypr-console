//! A continuation has one owner. The queue and core handoffs model the
//! scheduler's atomic actions, including checking for a wake before parking.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Core {
  First,
  Second,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Lease {
  core: Core,
  generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
  Ready,
  Running { lease: Lease, wake_pending: bool },
  Parked,
}

#[derive(Debug, Clone, Copy)]
enum Event {
  Claim(Core),
  Wake,
  Park(Lease),
  Yield(Lease),
}

#[derive(Debug, PartialEq, Eq)]
enum Effect {
  None,
  Exhausted,
  Start(Lease),
  Enqueue,
  Leave(Lease),
  Requeue(Lease),
}

struct Task {
  state: State,
  generation: u64,
}

impl Task {
  fn step(&mut self, event: Event) -> Effect {
    let (state, effect) = match (self.state, event) {
      (State::Ready, Event::Claim(core)) => {
        let Some(generation) = self.generation.checked_add(1) else {
          return Effect::Exhausted;
        };
        self.generation = generation;
        let lease = Lease { core, generation };
        (
          State::Running {
            lease,
            wake_pending: false,
          },
          Effect::Start(lease),
        )
      }
      (State::Running { lease, .. }, Event::Wake) => (
        State::Running {
          lease,
          wake_pending: true,
        },
        Effect::None,
      ),
      (State::Parked, Event::Wake) => (State::Ready, Effect::Enqueue),
      (
        State::Running {
          lease,
          wake_pending,
        },
        Event::Park(reported),
      ) if lease == reported => {
        if wake_pending {
          (State::Ready, Effect::Requeue(lease))
        } else {
          (State::Parked, Effect::Leave(lease))
        }
      }
      (State::Running { lease, .. }, Event::Yield(reported)) if lease == reported => {
        (State::Ready, Effect::Requeue(lease))
      }
      (state, _) => (state, Effect::None),
    };
    self.state = state;
    effect
  }
}

fn core() -> Core {
  if kani::any() {
    Core::First
  } else {
    Core::Second
  }
}

fn event(current: Lease) -> Event {
  let lease = if kani::any() {
    current
  } else {
    Lease {
      core: core(),
      generation: kani::any(),
    }
  };
  match kani::any::<u8>() % 4 {
    0 => Event::Claim(core()),
    1 => Event::Wake,
    2 => Event::Park(lease),
    _ => Event::Yield(lease),
  }
}

#[kani::proof]
fn a_scheduler_step_preserves_unique_queue_or_core_ownership() {
  let generation = kani::any();
  let lease = Lease {
    core: core(),
    generation,
  };
  let state = match kani::any::<u8>() % 3 {
    0 => State::Ready,
    1 => State::Parked,
    _ => State::Running {
      lease,
      wake_pending: kani::any(),
    },
  };
  let mut task = Task { state, generation };
  let mut queued = state == State::Ready;
  let mut running = if matches!(state, State::Running { .. }) {
    Some(lease)
  } else {
    None
  };
  match task.step(event(lease)) {
    Effect::None => {}
    Effect::Exhausted => {
      assert!(queued && running.is_none());
      assert_eq!(generation, u64::MAX);
    }
    Effect::Start(owner) => {
      assert!(queued && running.is_none());
      assert!(owner.generation > generation);
      queued = false;
      running = Some(owner);
    }
    Effect::Enqueue => {
      assert!(!queued && running.is_none());
      queued = true;
    }
    Effect::Leave(owner) => {
      assert!(!queued && running == Some(owner));
      running = None;
    }
    Effect::Requeue(owner) => {
      assert!(!queued && running == Some(owner));
      running = None;
      queued = true;
    }
  }
  assert_eq!(queued, task.state == State::Ready);
  assert_eq!(
    running,
    match task.state {
      State::Running { lease, .. } => Some(lease),
      State::Ready | State::Parked => None,
    }
  );
}

#[kani::proof]
fn wake_and_park_in_either_order_enqueue_exactly_once() {
  let generation = kani::any();
  let lease = Lease {
    core: core(),
    generation,
  };
  let mut task = Task {
    state: State::Running {
      lease,
      wake_pending: false,
    },
    generation,
  };
  if kani::any() {
    assert_eq!(task.step(Event::Wake), Effect::None);
    assert_eq!(task.step(Event::Park(lease)), Effect::Requeue(lease));
  } else {
    assert_eq!(task.step(Event::Park(lease)), Effect::Leave(lease));
    assert_eq!(task.step(Event::Wake), Effect::Enqueue);
  }
  assert_eq!(task.step(Event::Wake), Effect::None);
  assert_eq!(task.state, State::Ready);
}

#[kani::proof]
fn a_migrating_continuation_rejects_old_leases_and_exhausted_generations() {
  let generation = kani::any();
  let lease = Lease {
    core: core(),
    generation,
  };
  let mut task = Task {
    state: State::Running {
      lease,
      wake_pending: false,
    },
    generation,
  };
  assert_eq!(task.step(Event::Yield(lease)), Effect::Requeue(lease));
  let destination = core();
  let effect = task.step(Event::Claim(destination));
  if generation == u64::MAX {
    assert_eq!(effect, Effect::Exhausted);
    assert_eq!(task.state, State::Ready);
  } else {
    let current = Lease {
      core: destination,
      generation: generation + 1,
    };
    assert_eq!(effect, Effect::Start(current));
    assert_eq!(task.step(Event::Claim(lease.core)), Effect::None);
    assert_eq!(task.step(Event::Park(lease)), Effect::None);
    assert_eq!(task.step(Event::Yield(lease)), Effect::None);
    assert_eq!(
      task.state,
      State::Running {
        lease: current,
        wake_pending: false
      }
    );
  }
}
