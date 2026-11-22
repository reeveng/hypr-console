//! Run with `cargo kani -p console-kernel-handles --lib --harness accounting::proofs`.

use super::*;
use crate::HANDLE_COST;

#[kani::proof]
fn charges_never_wrap_or_exceed_the_jobs_share() {
  let held: u64 = kani::any();
  let share: u64 = kani::any();
  let amount: u64 = kani::any();
  let budget = Budget {
    share: Bytes(share),
    charged: Bytes(held),
  };
  let exact = u128::from(held) + u128::from(amount);

  match charge_within(budget, amount) {
    Ok(charged) => {
      assert_eq!(u128::from(charged), exact);
      assert!(charged <= share);
    }
    Err(error) => {
      assert_eq!(error, HandleError::OverBudget);
      assert!(exact > u128::from(share));
    }
  }
}

#[kani::proof]
fn receipt_releases_message_bytes_and_transfers_only_the_handle_charge() {
  let sender: u64 = kani::any();
  let sender_share: u64 = kani::any();
  let receiver: u64 = kani::any();
  let receiver_share: u64 = kani::any();
  let message: u64 = kani::any();
  let handles: u64 = kani::any();
  let same_job: bool = kani::any();
  kani::assume(sender <= sender_share && receiver <= receiver_share);

  let sending = Budget {
    share: Bytes(sender_share),
    charged: Bytes(sender),
  };
  let receiving = Budget {
    share: Bytes(receiver_share),
    charged: Bytes(receiver),
  };
  let jobs = if same_job {
    DeliveryJobs::SameJob
  } else {
    DeliveryJobs::DifferentJobs(receiving)
  };
  let queued = u128::from(message) + u128::from(handles);
  let destination = u128::from(receiver) + u128::from(handles);

  match delivery_charges(sending, jobs, DeliveryCost { message, handles }) {
    Ok(DeliveryCharges::SameJob(after)) => {
      assert!(same_job && queued <= u128::from(sender));
      assert!(after <= sender_share);
      assert_eq!(u128::from(after) + u128::from(message), u128::from(sender));
    }
    Ok(DeliveryCharges::DifferentJobs {
      sending: left,
      receiving: right,
    }) => {
      assert!(!same_job && queued <= u128::from(sender));
      assert!(left <= sender_share && right <= receiver_share);
      assert_eq!(u128::from(left) + queued, u128::from(sender));
      assert_eq!(u128::from(right), destination);
      assert_eq!(
        u128::from(left) + u128::from(right) + u128::from(message),
        u128::from(sender) + u128::from(receiver)
      );
    }
    Err(HandleError::BadState) => assert!(queued > u128::from(sender)),
    Err(HandleError::OverBudget) => {
      assert!(!same_job && destination > u128::from(receiver_share))
    }
    Err(_) => panic!("unexpected receipt error"),
  }
}

#[kani::proof]
fn the_peer_window_caps_the_charge_of_queued_messages_and_handles() {
  let waiting: u64 = kani::any();
  let length: u64 = kani::any();
  let handles: u64 = kani::any();
  let cost = message_cost(MessageSize {
    bytes: length,
    handles,
  });
  let window = waiting_room(waiting);

  assert_eq!(window.is_ok(), waiting < 64);
  assert_eq!(cost.is_ok(), length <= 65_536 && handles <= 64);

  if let (Ok(()), Ok(payload)) = (window, cost) {
    let queued_cost = payload + handles * HANDLE_COST.0;
    let maximum = 65_536 + MESSAGE_COST.0 + 64 * HANDLE_COST.0;
    assert_eq!(payload, MESSAGE_COST.0 + length);
    assert!(queued_cost <= maximum);
    assert!(waiting * maximum + queued_cost <= 4_329_472);
  }
}
