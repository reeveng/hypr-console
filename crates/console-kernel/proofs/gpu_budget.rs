//! One nonpreemptible device lane and one job per class. Two reservations
//! per job include queued, running, and completed batches whose buffers are
//! still retained. The driver reports release after the buffer protocol.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
  Interactive,
  Background,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct BatchId(u64);

#[derive(Clone, Copy)]
struct Cost {
  bytes: u64,
  work: u16,
}

#[derive(Clone, Copy)]
struct Limits {
  bytes: u64,
  batch_work: u16,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
  Queued,
  Running,
  Retained,
}

#[derive(Clone, Copy)]
struct Reservation {
  id: BatchId,
  cost: Cost,
  state: State,
}

struct Job {
  limits: Limits,
  slots: [Option<Reservation>; 2],
  charged: u64,
  next: BatchId,
}

impl Job {
  fn new(limits: Limits) -> Self {
    Self {
      limits,
      slots: [None, None],
      charged: 0,
      next: BatchId(0),
    }
  }

  fn admit(&mut self, cost: Cost) -> Option<BatchId> {
    let charged = self.charged.checked_add(cost.bytes)?;
    if charged > self.limits.bytes || cost.work == 0 || cost.work > self.limits.batch_work {
      return None;
    }
    let next = BatchId(self.next.0.checked_add(1)?);
    let vacant = self.slots.iter_mut().find(|slot| slot.is_none())?;
    *vacant = Some(Reservation {
      id: next,
      cost,
      state: State::Queued,
    });
    self.charged = charged;
    self.next = next;
    Some(next)
  }

  fn start(&mut self) -> Option<BatchId> {
    let first = self
      .slots
      .iter_mut()
      .flatten()
      .filter(|r| r.state == State::Queued)
      .min_by_key(|r| r.id)?;
    first.state = State::Running;
    Some(first.id)
  }

  fn complete(&mut self, id: BatchId) -> Option<()> {
    let batch = self
      .slots
      .iter_mut()
      .flatten()
      .find(|r| r.id == id && r.state == State::Running)?;
    batch.state = State::Retained;
    Some(())
  }

  fn release(&mut self, id: BatchId) -> Option<u64> {
    let slot = self
      .slots
      .iter_mut()
      .find(|slot| slot.is_some_and(|r| r.id == id && r.state == State::Retained))?;
    let reservation = (*slot)?;
    let remaining = self.charged.checked_sub(reservation.cost.bytes)?;
    *slot = None;
    self.charged = remaining;
    Some(reservation.cost.bytes)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Running {
  class: Class,
  id: BatchId,
}

struct Device {
  interactive: Job,
  background: Job,
  active: Option<Running>,
}

impl Device {
  fn new(limits: Limits) -> Self {
    Self {
      interactive: Job::new(limits),
      background: Job::new(limits),
      active: None,
    }
  }

  fn dispatch(&mut self) -> Option<Running> {
    if self.active.is_some() {
      return None;
    }
    let work = match self.interactive.start() {
      Some(id) => Running {
        class: Class::Interactive,
        id,
      },
      None => Running {
        class: Class::Background,
        id: self.background.start()?,
      },
    };
    self.active = Some(work);
    Some(work)
  }

  fn complete(&mut self, completed: Running) -> Option<()> {
    if self.active != Some(completed) {
      return None;
    }
    match completed.class {
      Class::Interactive => self.interactive.complete(completed.id)?,
      Class::Background => self.background.complete(completed.id)?,
    }
    self.active = None;
    Some(())
  }
}

fn cost() -> Cost {
  Cost {
    bytes: kani::any(),
    work: kani::any(),
  }
}

fn supported_cost(bytes: u64) -> Cost {
  Cost { bytes, work: 1 }
}

#[kani::proof]
#[kani::unwind(5)]
fn gpu_reservations_bound_queue_space_bytes_and_batch_work() {
  let limits = Limits {
    bytes: kani::any(),
    batch_work: kani::any(),
  };
  let offered = [cost(), cost(), cost()];
  let mut job = Job::new(limits);
  let mut admitted = 0;
  let mut charged = 0_u128;
  for offer in offered {
    let total = charged + u128::from(offer.bytes);
    let fits = admitted < 2
      && total <= u128::from(limits.bytes)
      && (1..=limits.batch_work).contains(&offer.work);
    assert_eq!(job.admit(offer).is_some(), fits);
    if fits {
      admitted += 1;
      charged = total;
    }
    assert_eq!(job.slots.iter().flatten().count(), admitted);
    assert_eq!(u128::from(job.charged), charged);
    assert!(job.charged <= limits.bytes);
  }
  kani::cover!(admitted == 2);
}

#[kani::proof]
#[kani::unwind(5)]
fn completed_batches_hold_their_budget_until_the_matching_release() {
  let bytes = kani::any();
  let mut job = Job::new(Limits {
    bytes,
    batch_work: 1,
  });
  let batch = job.admit(supported_cost(bytes)).unwrap();
  assert_eq!(job.release(batch), None);
  assert_eq!(job.start(), Some(batch));
  assert_eq!(job.release(batch), None);
  assert_eq!(job.complete(batch), Some(()));
  assert_eq!(job.charged, bytes);
  assert_eq!(job.release(BatchId(0)), None);
  assert_eq!(job.release(batch), Some(bytes));
  assert_eq!(job.release(batch), None);
  let fresh = job.admit(supported_cost(bytes)).unwrap();
  assert!(fresh > batch);
  assert_eq!(job.start(), Some(fresh));
  assert_eq!(job.complete(fresh), Some(()));
  assert_eq!(job.release(batch), None);
  assert_eq!(job.charged, bytes);
}

#[kani::proof]
#[kani::unwind(5)]
fn interactive_fifo_work_waits_at_most_for_the_running_batch_and_one_older_batch() {
  let mut device = Device::new(Limits {
    bytes: 2,
    batch_work: 1,
  });
  let light = supported_cost(1);
  let background = device.background.admit(light).unwrap();
  let running = Running {
    class: Class::Background,
    id: background,
  };
  assert_eq!(device.dispatch(), Some(running));
  let _later_background = device.background.admit(light).unwrap();
  let older = if kani::any() {
    device.interactive.admit(light)
  } else {
    None
  };
  let target = device.interactive.admit(light).unwrap();
  assert_eq!(device.dispatch(), None);
  assert_eq!(device.complete(running), Some(()));

  if let Some(older) = older {
    let predecessor = Running {
      class: Class::Interactive,
      id: older,
    };
    assert_eq!(device.dispatch(), Some(predecessor));
    assert_eq!(device.complete(predecessor), Some(()));
    assert_eq!(device.interactive.release(older), Some(1));
  }

  let younger = device.interactive.admit(light).unwrap();
  assert!(younger > target);
  assert_eq!(
    device.dispatch(),
    Some(Running {
      class: Class::Interactive,
      id: target
    })
  );
}
