//! Admission and budget transitions calculated before changing a queue. A
//! send moves already-paid handles into the message and charges its header
//! and bytes to the sender. Receipt releases the message charge and moves
//! only the handle charge to the receiver.

use super::{
  Budget, Bytes, HandleError, MESSAGE_BYTES_LIMIT, MESSAGE_COST, MESSAGE_HANDLES_LIMIT,
  WAITING_MESSAGES_LIMIT,
};

#[cfg(kani)]
#[path = "../proofs/accounting.rs"]
mod proofs;

pub(super) enum DeliveryJobs {
  SameJob,
  DifferentJobs(Budget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DeliveryCharges {
  SameJob(u64),
  DifferentJobs { sending: u64, receiving: u64 },
}

pub(super) fn charge_within(budget: Budget, amount: u64) -> Result<u64, HandleError> {
  budget
    .charged
    .0
    .checked_add(amount)
    .filter(|charged| *charged <= budget.share.0)
    .ok_or(HandleError::OverBudget)
}

pub(super) struct DeliveryCost {
  pub(super) message: u64,
  pub(super) handles: u64,
}

pub(super) fn delivery_charges(
  sending: Budget,
  jobs: DeliveryJobs,
  DeliveryCost { message, handles }: DeliveryCost,
) -> Result<DeliveryCharges, HandleError> {
  let queued = message.checked_add(handles).ok_or(HandleError::BadState)?;
  let left = sending
    .charged
    .0
    .checked_sub(queued)
    .ok_or(HandleError::BadState)?;

  match jobs {
    DeliveryJobs::SameJob => {
      let after_release = Budget {
        share: sending.share,
        charged: Bytes(left),
      };

      charge_within(after_release, handles).map(DeliveryCharges::SameJob)
    }
    DeliveryJobs::DifferentJobs(receiving) => {
      let received = charge_within(receiving, handles)?;

      Ok(DeliveryCharges::DifferentJobs {
        sending: left,
        receiving: received,
      })
    }
  }
}

pub(super) struct MessageSize {
  pub(super) bytes: u64,
  pub(super) handles: u64,
}

pub(super) fn message_cost(size: MessageSize) -> Result<u64, HandleError> {
  match bool::then_some(
    size.bytes <= u64::from(MESSAGE_BYTES_LIMIT) && size.handles <= u64::from(MESSAGE_HANDLES_LIMIT),
    (),
  ) {
    Some(()) => MESSAGE_COST
      .0
      .checked_add(size.bytes)
      .ok_or(HandleError::OverBudget),
    None => Err(HandleError::MessageTooLarge),
  }
}

pub(super) fn waiting_room(waiting: u64) -> Result<(), HandleError> {
  match bool::then_some(waiting < u64::from(WAITING_MESSAGES_LIMIT), ()) {
    Some(()) => Ok(()),
    None => Err(HandleError::ChannelFull),
  }
}
