//! Locks that remain usable after a panic. Callers are responsible for
//! deciding whether protected data can be reused after an interrupted update.

use core::ops::ControlFlow;
use std::sync::{self, Condvar, LockResult, PoisonError, TryLockError, TryLockResult};

pub use std::sync::{MutexGuard, RwLockReadGuard, RwLockWriteGuard};

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the recovery adapter gives callers the contained value for either lock outcome"
  )
)]
pub fn unpoison<T>(result: LockResult<T>) -> T {
  match result {
    Ok(value) => value,
    Err(poisoned) => PoisonError::into_inner(poisoned),
  }
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the nonblocking lock interface answers an optional guard"
  )
)]
pub fn try_guard<T>(result: TryLockResult<T>) -> Option<T> {
  match result {
    Ok(guard) => Some(guard),
    Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
    Err(TryLockError::WouldBlock) => None,
  }
}

#[doc = "Recheck the predicate after each wake, including when the mutex is poisoned."]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "waiting hands the same mutex guard back to its caller"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the caller owns the wait predicate and its notification protocol"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit055_no_unbounded_loop",
  expect(
    explicit055_no_unbounded_loop,
    reason = "the caller's predicate determines completion and pending iterations wait for notification"
  )
)]
pub fn wait_while<'a, T>(
  cond: &Condvar,
  guard: MutexGuard<'a, T>,
  waiting: impl FnMut(&mut T) -> bool,
) -> MutexGuard<'a, T> {
  let outcome =
    core::iter::repeat(()).try_fold((guard, waiting), |(mut current, mut predicate), ()| {
      match predicate(&mut current).then_some(()) {
        Some(()) => ControlFlow::Continue((unpoison(cond.wait(current)), predicate)),
        None => ControlFlow::Break((current, predicate)),
      }
    });

  match outcome {
    ControlFlow::Break((ready, _predicate)) | ControlFlow::Continue((ready, _predicate)) => ready,
  }
}

#[derive(Debug, Default)]
pub struct Mutex<T>(sync::Mutex<T>);

impl<T> Mutex<T> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the constructor preserves the standard lock interface used by the decoder"
    )
  )]
  pub const fn new(value: T) -> Self {
    Self(sync::Mutex::new(value))
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "callers receive the mutex guard directly and manage recovery in their own state"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "the mutex guard protects caller-controlled critical sections using the standard blocking contract"
    )
  )]
  pub fn lock(&self) -> MutexGuard<'_, T> {
    unpoison(self.0.lock())
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the standard nonblocking mutex interface answers an optional guard"
    )
  )]
  pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
    try_guard(self.0.try_lock())
  }
}

#[derive(Debug, Default)]
pub struct RwLock<T>(sync::RwLock<T>);

impl<T> RwLock<T> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the constructor preserves the standard lock interface used by the decoder"
    )
  )]
  pub const fn new(value: T) -> Self {
    Self(sync::RwLock::new(value))
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the read-lock interface hands its recovered guard directly to the caller"
    )
  )]
  pub fn read(&self) -> RwLockReadGuard<'_, T> {
    unpoison(self.0.read())
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the write-lock interface hands its recovered guard directly to the caller"
    )
  )]
  pub fn write(&self) -> RwLockWriteGuard<'_, T> {
    unpoison(self.0.write())
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the standard nonblocking read-lock interface answers an optional guard"
    )
  )]
  pub fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
    try_guard(self.0.try_read())
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the standard nonblocking write-lock interface answers an optional guard"
    )
  )]
  pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
    try_guard(self.0.try_write())
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "exclusive access exposes the protected value as a direct mutable borrow"
    )
  )]
  pub fn get_mut(&mut self) -> &mut T {
    unpoison(self.0.get_mut())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use core::cell::Cell;
  use std::panic::catch_unwind;

  #[test]
  #[cfg_attr(
    dylint_lib = "explicit004_no_panic",
    expect(
      explicit004_no_panic,
      reason = "the caught panic prepares a poisoned mutex for the recovery test"
    )
  )]
  fn waiting_rechecks_the_predicate_after_reacquiring_a_poisoned_mutex() {
    let ready = Mutex::new(false);
    let changed = Condvar::new();
    let panicked = catch_unwind(|| {
      let _guard = ready.lock();
      panic!("poison the mutex");
    });
    assert!(matches!(panicked, Err(_panic_payload)));

    let guard = ready.lock();
    std::thread::scope(|scope| {
      scope.spawn(|| {
        *ready.lock() = true;
        changed.notify_one();
      });
      let checks = Cell::new(0_u32);
      let ready_guard = wait_while(&changed, guard, |state| {
        checks.set(checks.get().saturating_add(1));
        !*state
      });

      assert!(*ready_guard);
      assert!(checks.get() >= 2);
    });
  }
}
