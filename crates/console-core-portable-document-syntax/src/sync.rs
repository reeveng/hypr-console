#[cfg(feature = "std")]
pub(crate) use std::collections::HashMap;

#[cfg(not(feature = "std"))]
pub(crate) use alloc::collections::BTreeMap as HashMap;

#[cfg(feature = "std")]
pub(crate) use console_core_perfect_hash::collections::FixedMap;

#[cfg(not(feature = "std"))]
pub(crate) use alloc::collections::BTreeMap as FixedMap;

#[cfg(feature = "std")]
pub(crate) use std::sync::Arc;

#[cfg(not(feature = "std"))]
pub(crate) use alloc::rc::Rc as Arc;

#[cfg(feature = "std")]
pub(crate) use std::sync::OnceLock;

#[cfg(not(feature = "std"))]
pub(crate) use core::cell::OnceCell as OnceLock;

#[cfg(feature = "std")]
pub(crate) use std::sync::Mutex;

#[cfg(not(feature = "std"))]
pub(crate) use core::cell::RefCell as Mutex;

#[cfg(feature = "std")]
pub(crate) use std::sync::RwLock;

#[cfg(not(feature = "std"))]
pub(crate) use core::cell::RefCell as RwLock;

#[cfg(feature = "std")]
pub(crate) type MutexGuard<'a, T> = std::sync::MutexGuard<'a, T>;

#[cfg(not(feature = "std"))]
pub(crate) type MutexGuard<'a, T> = core::cell::RefMut<'a, T>;

#[cfg(feature = "std")]
pub(crate) type RwLockReadGuard<'a, T> = std::sync::RwLockReadGuard<'a, T>;

#[cfg(not(feature = "std"))]
pub(crate) type RwLockReadGuard<'a, T> = core::cell::Ref<'a, T>;

#[cfg(feature = "std")]
pub(crate) type RwLockWriteGuard<'a, T> = std::sync::RwLockWriteGuard<'a, T>;

#[cfg(not(feature = "std"))]
pub(crate) type RwLockWriteGuard<'a, T> = core::cell::RefMut<'a, T>;

pub(crate) trait MutexExt<T> {
  fn get(&self) -> Result<Option<MutexGuard<'_, T>>, console_core_never::Never>;
}

#[cfg(feature = "std")]
impl<T> MutexExt<T> for Mutex<T> {
  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "The shared cache mutex protects only a short index update and releases before decoding or other callbacks"
    )
  )]
  fn get(&self) -> Result<Option<MutexGuard<'_, T>>, console_core_never::Never> {
    Ok(Some(match self.lock() {
      Ok(guard) => guard,
      Err(poisoned) => poisoned.into_inner(),
    }))
  }
}

#[cfg(not(feature = "std"))]
impl<T> MutexExt<T> for Mutex<T> {
  fn get(&self) -> Result<Option<MutexGuard<'_, T>>, console_core_never::Never> {
    Ok(match self.try_borrow_mut() {
      Ok(guard) => Some(guard),
      Err(_active_cache_borrow) => None,
    })
  }
}

pub(crate) trait RwLockExt<T> {
  fn get(&self) -> Option<RwLockReadGuard<'_, T>>;

  fn try_get(&self) -> Option<RwLockReadGuard<'_, T>>;

  fn try_put(&self) -> Option<RwLockWriteGuard<'_, T>>;
}

#[cfg(feature = "std")]
impl<T> RwLockExt<T> for RwLock<T> {
  fn get(&self) -> Option<RwLockReadGuard<'_, T>> {
    match self.read() {
      Ok(reader) => Some(reader),
      Err(poisoned) => Some(poisoned.into_inner()),
    }
  }

  fn try_get(&self) -> Option<RwLockReadGuard<'_, T>> {
    match self.try_read() {
      Ok(guard) => Some(guard),
      Err(std::sync::TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
      Err(std::sync::TryLockError::WouldBlock) => None,
    }
  }

  fn try_put(&self) -> Option<RwLockWriteGuard<'_, T>> {
    match self.try_write() {
      Ok(guard) => Some(guard),
      Err(std::sync::TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
      Err(std::sync::TryLockError::WouldBlock) => None,
    }
  }
}

#[cfg(not(feature = "std"))]
impl<T> RwLockExt<T> for RwLock<T> {
  fn get(&self) -> Option<RwLockReadGuard<'_, T>> {
    self.try_get()
  }

  fn try_get(&self) -> Option<RwLockReadGuard<'_, T>> {
    match self.try_borrow() {
      Ok(guard) => Some(guard),
      Err(_active_write) => None,
    }
  }

  fn try_put(&self) -> Option<RwLockWriteGuard<'_, T>> {
    match self.try_borrow_mut() {
      Ok(guard) => Some(guard),
      Err(_active_borrows) => None,
    }
  }
}
