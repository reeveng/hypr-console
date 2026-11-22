//! A single-core round-robin selector. The scheduler owns one stable slot per
//! admitted context and invokes the selector once per scheduling quantum.

use console_core_never::Never;

pub const RUN_SLOTS: u32 = u64::BITS;
const SLOT_MASK: u32 = RUN_SLOTS.wrapping_sub(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunSlot(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidRunSlot;

impl RunSlot {
  pub fn within(index: u32) -> Result<Self, InvalidRunSlot> {
    match ::core::primitive::bool::then_some(index < RUN_SLOTS, ()) {
      ::core::option::Option::Some(()) => Ok(Self(index)),
      ::core::option::Option::None => Err(InvalidRunSlot),
    }
  }

  pub fn index(self) -> Result<u32, Never> {
    Ok(self.0)
  }
}

#[derive(Clone, Copy)]
pub struct RunnableSlots(u64);

impl RunnableSlots {
  pub const EMPTY: Self = Self(0);

  pub fn wake(self, slot: RunSlot) -> Result<Self, Never> {
    Ok(Self(self.0 | 1_u64.wrapping_shl(slot.0)))
  }

  pub fn park(self, slot: RunSlot) -> Result<Self, Never> {
    Ok(Self(self.0 & !1_u64.wrapping_shl(slot.0)))
  }
}

pub struct RoundRobin {
  cursor: RunSlot,
}

impl RoundRobin {
  pub fn starting_at(cursor: RunSlot) -> Result<Self, Never> {
    Ok(Self { cursor })
  }

  pub fn dispatch(&mut self, runnable: RunnableSlots) -> Result<Option<RunSlot>, Never> {
    let rotated = runnable.0.rotate_right(self.cursor.0);
    let offset = match rotated {
      0 => return Ok(None),
      1.. => rotated.trailing_zeros(),
    };
    let chosen = self.cursor.0.wrapping_add(offset) & SLOT_MASK;
    self.cursor = RunSlot(chosen.wrapping_add(1) & SLOT_MASK);

    Ok(Some(RunSlot(chosen)))
  }
}

#[cfg(kani)]
#[path = "../proofs/scheduling.rs"]
mod proofs;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_wakeup_across_the_boundary_runs_after_the_current_slot() -> Result<(), InvalidRunSlot> {
    let last = RunSlot::within(63)?;
    let first = RunSlot::within(0)?;
    let Ok(mut selector) = RoundRobin::starting_at(last);
    let Ok(last_ready) = RunnableSlots::EMPTY.wake(last);
    let Ok(both_ready) = last_ready.wake(first);
    let Ok(first_chosen) = selector.dispatch(both_ready);
    assert_eq!(first_chosen, Some(last));

    let Ok(second_chosen) = selector.dispatch(both_ready);
    assert_eq!(second_chosen, Some(first));

    let Ok(last_only) = both_ready.park(first);
    let Ok(third_chosen) = selector.dispatch(last_only);
    assert_eq!(third_chosen, Some(last));

    Ok(())
  }
}
