//! Model of one device grant. Its epoch distinguishes it from prior grants.
//! Drain confirms that old device requests cannot touch the frame again.
//! Invalidation covers the IOMMU and any cached device translations. Both
//! completion events must be authenticated by the hardware-facing service.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GrantId {
  pub(super) device: u16,
  pub(super) domain: u32,
  pub(super) frame: u64,
  pub(super) epoch: u64,
}

pub(super) struct Released(GrantId);

impl Released {
  pub(super) fn grant(&self) -> GrantId {
    self.0
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
  Active,
  Stopped,
  Drained,
  Invalidated,
  Released,
}

pub(super) struct Grant {
  id: GrantId,
  write: bool,
  state: State,
}

impl Grant {
  pub(super) fn new(id: GrantId, write: bool) -> Self {
    Self {
      id,
      write,
      state: State::Active,
    }
  }

  fn allows(&self, request: GrantId, write: bool) -> bool {
    self.state == State::Active && self.id == request && (!write || self.write)
  }

  pub(super) fn stop(&mut self) {
    if self.state == State::Active {
      self.state = State::Stopped;
    }
  }

  pub(super) fn drained(&mut self, grant: GrantId) {
    if self.state == State::Stopped && grant == self.id {
      self.state = State::Drained;
    }
  }

  pub(super) fn invalidated(&mut self, grant: GrantId) {
    if self.state == State::Drained && grant == self.id {
      self.state = State::Invalidated;
    }
  }

  pub(super) fn release(&mut self) -> Option<Released> {
    if self.state == State::Invalidated {
      self.state = State::Released;
      Some(Released(self.id))
    } else {
      None
    }
  }
}

fn grant_id() -> GrantId {
  GrantId {
    device: kani::any(),
    domain: kani::any(),
    frame: kani::any(),
    epoch: kani::any(),
  }
}

#[kani::proof]
fn dma_admission_is_limited_to_the_grant_and_stops_at_revocation() {
  let id = grant_id();
  let request = grant_id();
  let write = kani::any();
  let wants_write = kani::any();
  let mut grant = Grant::new(id, write);
  let same_target = id.device == request.device
    && id.domain == request.domain
    && id.frame == request.frame
    && id.epoch == request.epoch;

  assert_eq!(
    grant.allows(request, wants_write),
    same_target && (write || !wants_write)
  );

  grant.stop();
  assert!(!grant.allows(request, wants_write));
}

#[kani::proof]
#[kani::unwind(7)]
fn dma_release_needs_matching_drain_and_invalidation_in_order_and_happens_once() {
  let id = grant_id();
  let mut grant = Grant::new(id, kani::any());
  let mut stopped = false;
  let mut drained = false;
  let mut invalidated = false;
  let mut released = false;

  for _ in 0..6 {
    let acknowledgement = if kani::any() { id } else { grant_id() };
    match kani::any::<u8>() % 4 {
      0 => {
        grant.stop();
        stopped = true;
      }
      1 => {
        grant.drained(acknowledgement);
        drained |= stopped && acknowledgement == id;
      }
      2 => {
        grant.invalidated(acknowledgement);
        invalidated |= drained && acknowledgement == id;
      }
      _ => {
        let result = grant.release();
        assert_eq!(
          result.is_some(),
          stopped && drained && invalidated && !released
        );
        if let Some(receipt) = result {
          assert_eq!(receipt.grant(), id);
          released = true;
        }
      }
    }
    assert_eq!(grant.allows(id, false), !stopped);
  }
  kani::cover!(released);
}
