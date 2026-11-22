//! Run with `cargo kani -p console-kernel --lib --harness scheduling`.

use super::*;

fn slot() -> RunSlot {
  let index: u32 = kani::any();
  kani::assume(index < RUN_SLOTS);
  RunSlot::within(index).unwrap()
}

fn distance(from: RunSlot, to: RunSlot) -> u32 {
  (to.0 + RUN_SLOTS - from.0) % RUN_SLOTS
}

#[kani::proof]
fn each_dispatch_selects_the_first_runnable_slot_or_idles() {
  let start = slot();
  let candidate = slot();
  let ready: u64 = kani::any();
  let Ok(mut scheduler) = RoundRobin::starting_at(start);
  let Ok(selected) = scheduler.dispatch(RunnableSlots(ready));

  match selected {
    None => {
      assert_eq!(ready, 0);
      assert_eq!(scheduler.cursor, start);
    }
    Some(chosen) => {
      assert!(chosen.0 < RUN_SLOTS);
      assert_ne!(ready & (1_u64 << chosen.0), 0);
      assert_eq!(scheduler.cursor.0, (chosen.0 + 1) % RUN_SLOTS);
      if ready & (1_u64 << candidate.0) != 0 {
        assert!(distance(start, chosen) <= distance(start, candidate));
      }
    }
  }
}

// This one-step decrease applies to every mask containing the target. Therefore
// wakeups and parks in all other slots cannot postpone it past one revolution.
#[kani::proof]
fn a_continuously_runnable_slot_gets_a_quantum_within_one_revolution() {
  let start = slot();
  let target = slot();
  let others: u64 = kani::any();
  let Ok(mut scheduler) = RoundRobin::starting_at(start);
  let Ok(ready) = RunnableSlots(others).wake(target);
  let Ok(chosen) = scheduler.dispatch(ready);
  let chosen = chosen.unwrap();
  let before = distance(start, target) + 1;
  assert!(before <= RUN_SLOTS);

  if chosen != target {
    let after = distance(scheduler.cursor, target) + 1;
    assert!(after < before);
  }

  kani::cover!(start.0 == 0 && target.0 == 63 && chosen.0 == 0);
}

// Each decision removes its chosen slot from the arc up to the target, and
// keeps previously passed slots out of that arc. With a stable set of N
// runnable slots, at most N distinct choices can precede or be the target.
#[kani::proof]
fn no_slot_runs_twice_before_a_waiting_slot_in_the_same_round() {
  let start = slot();
  let target = slot();
  let previously_passed = slot();
  let others: u64 = kani::any();
  let Ok(mut scheduler) = RoundRobin::starting_at(start);
  let Ok(ready) = RunnableSlots(others).wake(target);
  let Ok(chosen) = scheduler.dispatch(ready);
  let chosen = chosen.unwrap();

  if chosen != target {
    let before = distance(start, target);
    let after = distance(scheduler.cursor, target);
    assert!(distance(scheduler.cursor, chosen) > after);

    if distance(start, previously_passed) > before {
      assert!(distance(scheduler.cursor, previously_passed) > after);
    }
  }
}
