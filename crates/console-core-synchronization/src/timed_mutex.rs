use std::sync::Condvar;
use std::time::Instant;

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;

use crate::nonpoison::{Mutex, unpoison};

#[doc = "Exclusive access with a deadline for acquiring it."]
#[doc = "The caller controls how long the work runs after acquiring access."]
pub struct TimedMutex<T> {
  value: Mutex<Option<T>>,
  available: Condvar,
}

#[doc = "Returning the value wakes all waiters because each waiter has its own deadline."]
struct Guard<'a, T> {
  lock: &'a TimedMutex<T>,
  value: Option<T>,
}

impl<T> Drop for Guard<'_, T> {
  fn drop(&mut self) {
    *self.lock.value.lock() = self.value.take();
    self.lock.available.notify_all();
  }
}

impl<T> TimedMutex<T> {
  pub fn new(value: T) -> Result<Self, Never> {
    Ok(Self {
      value: Mutex::new(Some(value)),
      available: Condvar::new(),
    })
  }

  #[doc = "Return None if the deadline passes before the work can acquire the value."]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the timed acquisition interface preserves the optional callback answer used by existing callers"
    )
  )]
  pub fn with_until<R>(
    &self,
    deadline: Instant,
    now: impl Fn() -> Instant,
    work: impl FnOnce(&mut T) -> R,
  ) -> Option<R> {
    let Ok(acquired) = self.take_until(deadline, now);
    let value = acquired?;
    let mut held = Guard {
      lock: self,
      value: Some(value),
    };
    held.value.as_mut().map(work)
  }

  fn take_until(&self, deadline: Instant, now: impl Fn() -> Instant) -> Result<Option<T>, Never> {
    let initial_slot = self.value.lock();

    let outcome = iterate(initial_slot, |mut slot| {
      let remaining = deadline.saturating_duration_since(now());

      match remaining.is_zero().then_some(()) {
        Some(()) => return Ok(Step::Halt(None)),
        None => {}
      }

      match slot.take() {
        Some(value) => Ok(Step::Halt(Some(value))),
        None => {
          let (returned_slot, timeout) = unpoison(self.available.wait_timeout(slot, remaining));
          Ok(match timeout.timed_out().then_some(()) {
            Some(()) => Step::Halt(None),
            None => Step::Again(returned_slot),
          })
        }
      }
    });

    Ok(match outcome {
      Ok(acquired) => acquired,
      Err(Endless) => None,
    })
  }
}
