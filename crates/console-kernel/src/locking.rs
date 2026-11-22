//! Exclusive kernel state, with ownership transferred by acquire and release.
//!
//! The heap cannot assume one core forever. This lock serializes its owner,
//! including the frame cursor used to grow it, without allocating anything.
//! The firmware edge disables local interrupts before taking it and restores
//! their former state after releasing it, so an interrupt on the same core
//! cannot wait for the allocation it interrupted. NMI and fault paths must
//! not allocate. A guard is not transferable between threads.
//!
//! Waiting spins because the kernel does not yet switch execution contexts.
//! This is mutual exclusion, not a contention optimization: per-core caches
//! and scheduler-aware waiting can follow measurements once cores are up.

use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

use console_core_never::Never;

pub struct SpinLock<T> {
  held: AtomicBool,
  value: UnsafeCell<T>,
}

// SAFETY: the acquire/release flag permits exactly one mutable borrower at a time, and `Send` requires that the protected value may move between those borrowers.
#[cfg_attr(
  dylint_lib = "explicit069_lock_discipline",
  expect(
    explicit069_lock_discipline,
    reason = "the acquire and release of held make the UnsafeCell exclusive to a single Guard, and T can move between guards on different threads"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the guard transfers exclusive access to the cell with an acquire-release protocol and a Send bound"
  )
)]
unsafe impl<T: Send> Sync for SpinLock<T> {}

pub struct Guard<'lock, T> {
  lock: &'lock SpinLock<T>,
  local: PhantomData<*mut ()>,
}

impl<T> SpinLock<T> {
  #[must_use]
  pub const fn new(value: T) -> Result<Self, Never> {
    Ok(SpinLock {
      held: AtomicBool::new(false),
      value: UnsafeCell::new(value),
    })
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit069_lock_discipline",
    expect(
      explicit069_lock_discipline,
      reason = "successful Acquire pairs with Guard drop's Release; a failed acquisition never accesses the protected value"
    )
  )]
  pub fn try_lock(&self) -> Result<Option<Guard<'_, T>>, Never> {
    Ok(
      match self
        .held
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
      {
        Ok(_unheld) => Some(Guard {
          lock: self,
          local: PhantomData,
        }),
        Err(_held) => None,
      },
    )
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit054_no_loop",
    allow(
      explicit054_no_loop,
      reason = "the kernel's allocation lock retries until its owner releases it; no scheduler exists yet to park a contender"
    )
  )]
  pub fn lock(&self) -> Result<Guard<'_, T>, Never> {
    loop {
      let Ok(acquired) = self.try_lock();

      match acquired {
        Some(guard) => return Ok(guard),
        None => core::hint::spin_loop(),
      }
    }
  }
}

impl<T> Deref for Guard<'_, T> {
  type Target = T;

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the acquired guard makes a shared reference into the cell while preventing a concurrent mutable borrow"
    )
  )]
  fn deref(&self) -> &T {
    // SAFETY: this guard holds the acquired flag, and a shared guard borrow cannot overlap a mutable guard borrow.
    unsafe { &*self.lock.value.get() }
  }
}

impl<T> DerefMut for Guard<'_, T> {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the acquired guard and its exclusive borrow jointly protect the only mutable reference into the cell"
    )
  )]
  fn deref_mut(&mut self) -> &mut T {
    // SAFETY: only the guard holding the acquired flag can reach the cell, and this mutable guard borrow excludes all other access.
    unsafe { &mut *self.lock.value.get() }
  }
}

impl<T> Drop for Guard<'_, T> {
  fn drop(&mut self) {
    #[cfg_attr(
      dylint_lib = "explicit069_lock_discipline",
      expect(
        explicit069_lock_discipline,
        reason = "Release publishes the protected value to the next successful Acquire of held"
      )
    )]
    self.lock.held.store(false, Ordering::Release);
  }
}
