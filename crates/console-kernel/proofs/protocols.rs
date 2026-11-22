//! Run with `kani crates/console-kernel/proofs/protocols.rs`.
//!
//! These are state-machine models for multicore frame retirement and blocked
//! send cancellation. The hardware edge must unmap a frame before starting
//! retirement and must acknowledge only after the named core has invalidated
//! its translation. It must retain the previous epoch across frame reuse and
//! include every CPU that could still use the mapping. The DMA model follows
//! one outstanding device grant. The scrub operation must erase the complete
//! frame. The send model takes one serialized event at a time.

#[path = "dma.rs"]
mod dma;
#[path = "parallel.rs"]
mod parallel;

use dma::{GrantId, Released};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameEffect {
  Wait,
  ReturnFrame(u64),
  None,
}

struct Retirement {
  frame: u64,
  epoch: u64,
  pending: u64,
  dma: Option<GrantId>,
  scrubbed: bool,
  returned: bool,
}

impl Retirement {
  fn after_unmap(
    frame: u64,
    previous: u64,
    cached_on: u64,
    dma: Option<GrantId>,
  ) -> Option<Self> {
    let epoch = previous.checked_add(1)?;
    if dma.is_some_and(|grant| grant.frame != frame) {
      return None;
    }

    Some(Self {
      frame,
      epoch,
      pending: cached_on,
      dma,
      scrubbed: false,
      returned: false,
    })
  }

  fn acknowledge(&mut self, frame: u64, epoch: u64, core: u32) {
    if frame == self.frame && epoch == self.epoch {
      if let Some(bit) = 1_u64.checked_shl(core) {
        self.pending &= !bit;
      }
    }
  }

  fn device_released(&mut self, released: Released) {
    if self.dma == Some(released.grant()) {
      self.dma = None;
    }
  }

  fn scrub(&mut self, zero_frame: impl FnOnce(u64)) {
    if self.pending == 0 && self.dma.is_none() && !self.scrubbed {
      zero_frame(self.frame);
      self.scrubbed = true;
    }
  }

  fn reclaim(&mut self) -> FrameEffect {
    match (self.returned, self.scrubbed) {
      (true, _) => FrameEffect::None,
      (false, false) => FrameEffect::Wait,
      (false, true) => {
        self.returned = true;

        FrameEffect::ReturnFrame(self.frame)
      }
    }
  }
}

#[kani::proof]
fn retired_frames_get_a_new_epoch_or_refuse_retirement_at_overflow() {
  let frame: u64 = kani::any();
  let previous: u64 = kani::any();
  let cached: u64 = kani::any();

  match Retirement::after_unmap(frame, previous, cached, None) {
    Some(retirement) => {
      assert_eq!(retirement.frame, frame);
      assert!(retirement.epoch > previous);
      assert_eq!(retirement.pending, cached);
      assert!(!retirement.scrubbed);
      assert!(!retirement.returned);
    }
    None => assert_eq!(previous, u64::MAX),
  }
}

#[kani::proof]
fn only_a_matching_acknowledgement_can_clear_its_own_core() {
  let frame: u64 = kani::any();
  let previous: u64 = kani::any();
  let cached: u64 = kani::any();
  let acknowledgement_frame: u64 = kani::any();
  let acknowledgement_epoch: u64 = kani::any();
  let core: u32 = kani::any();
  let other: u32 = kani::any();
  kani::assume(other < 64);
  let Some(mut retirement) = Retirement::after_unmap(frame, previous, cached, None) else {
    return;
  };

  retirement.acknowledge(acknowledgement_frame, acknowledgement_epoch, core);
  let other_bit = 1_u64 << other;
  let received = acknowledgement_frame == frame && acknowledgement_epoch == retirement.epoch;
  let cleared_this_bit = received && other == core;

  assert_eq!(
    retirement.pending & other_bit != 0,
    cached & other_bit != 0 && !cleared_this_bit
  );
  assert!(!retirement.returned);

  let once = retirement.pending;
  retirement.acknowledge(acknowledgement_frame, acknowledgement_epoch, core);
  assert_eq!(retirement.pending, once);
}

#[kani::proof]
fn a_frame_can_be_reused_once_and_only_after_cpu_and_device_release_and_scrubbing() {
  let frame: u64 = kani::any();
  let pending: u64 = kani::any();
  let device_id = GrantId {
    device: kani::any(),
    domain: kani::any(),
    frame,
    epoch: kani::any(),
  };
  let device_mapped: bool = kani::any();
  let dma = device_mapped.then_some(device_id);
  let mut retirement = Retirement::after_unmap(frame, 0, pending, dma).unwrap();
  assert_eq!(retirement.reclaim(), FrameEffect::Wait);

  let want_release: bool = kani::any();
  let device_ack = GrantId {
    device: kani::any(),
    domain: kani::any(),
    frame: kani::any(),
    epoch: kani::any(),
  };
  if want_release {
    let mut grant = dma::Grant::new(device_ack, true);
    grant.stop();
    grant.drained(device_ack);
    grant.invalidated(device_ack);
    retirement.device_released(grant.release().unwrap());
  }

  let device_idle = !device_mapped || (want_release && device_ack == device_id);
  let mut scrubbed = false;
  retirement.scrub(|scrub| {
    assert_eq!(scrub, frame);
    scrubbed = true;
  });
  assert_eq!(scrubbed, pending == 0 && device_idle);

  match retirement.reclaim() {
    FrameEffect::Wait => {
      assert!(!scrubbed);
      assert!(!retirement.returned);
      assert_eq!(retirement.pending, pending);
    }
    FrameEffect::ReturnFrame(returned) => {
      assert!(scrubbed);
      assert_eq!(returned, frame);
      assert_eq!(retirement.reclaim(), FrameEffect::None);
      retirement.scrub(|_| unreachable!());
    }
    FrameEffect::None => panic!("the first reclaim has to wait or return the frame"),
  }
  kani::cover!(scrubbed && device_mapped);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendEvent {
  Receive { room: bool },
  Cancel,
  SenderExit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendEffect {
  None,
  Deliver(u64),
  Withdraw(u64),
}

#[derive(Debug, PartialEq, Eq)]
enum SendState {
  Pending(u64),
  Delivered,
  Withdrawn,
}

impl SendState {
  fn step(self, event: SendEvent) -> (Self, SendEffect) {
    match (self, event) {
      (Self::Pending(charge), SendEvent::Receive { room: true }) => {
        (Self::Delivered, SendEffect::Deliver(charge))
      }
      (Self::Pending(charge), SendEvent::Cancel | SendEvent::SenderExit) => {
        (Self::Withdrawn, SendEffect::Withdraw(charge))
      }
      (state, _) => (state, SendEffect::None),
    }
  }
}

fn event() -> SendEvent {
  match (kani::any::<bool>(), kani::any::<bool>()) {
    (true, room) => SendEvent::Receive { room },
    (false, true) => SendEvent::Cancel,
    (false, false) => SendEvent::SenderExit,
  }
}

#[kani::proof]
fn a_send_has_one_outcome_and_moves_its_charge_only_with_that_outcome() {
  let charge: u64 = kani::any();
  let first = event();
  let second = event();
  let (after_first, first_effect) = SendState::Pending(charge).step(first);

  match first {
    SendEvent::Receive { room: true } => {
      assert_eq!(after_first, SendState::Delivered);
      assert_eq!(first_effect, SendEffect::Deliver(charge));
    }
    SendEvent::Receive { room: false } => {
      assert_eq!(after_first, SendState::Pending(charge));
      assert_eq!(first_effect, SendEffect::None);
    }
    SendEvent::Cancel | SendEvent::SenderExit => {
      assert_eq!(after_first, SendState::Withdrawn);
      assert_eq!(first_effect, SendEffect::Withdraw(charge));
    }
  }

  match after_first {
    SendState::Pending(held) => {
      let expected = match second {
        SendEvent::Receive { room: true } => (SendState::Delivered, SendEffect::Deliver(held)),
        SendEvent::Receive { room: false } => (SendState::Pending(held), SendEffect::None),
        SendEvent::Cancel | SendEvent::SenderExit => {
          (SendState::Withdrawn, SendEffect::Withdraw(held))
        }
      };

      assert_eq!(after_first.step(second), expected);
    }
    SendState::Delivered | SendState::Withdrawn => {
      let delivered = after_first == SendState::Delivered;
      let (after_second, second_effect) = after_first.step(second);

      assert_eq!(
        after_second,
        if delivered {
          SendState::Delivered
        } else {
          SendState::Withdrawn
        }
      );
      assert_eq!(second_effect, SendEffect::None);
    }
  }
}
