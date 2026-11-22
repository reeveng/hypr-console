#![forbid(unsafe_code)]
use console_core_number_conversion::{fixed_index, index};
use std::{alloc::Layout, cmp::Ordering};

use crate::src::error::Rav1dError::ENOMEM;
use crate::src::error::Rav1dResult;
use console_core_synchronization::nonpoison::Mutex;
use std::sync::Arc;

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "frame references, decoder workers and C callbacks can outlive the context that hands them each reference"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit088_no_arriving_length",
  expect(explicit088_no_arriving_length, reason = "the reservation comes from the type layout and the fixed reference count header")
)]
pub(crate) fn try_arc<T>(value: T) -> Rav1dResult<Arc<T>> {
  let layout = Layout::new::<([*const (); 2], T)>();
  let mut probe = Vec::<u8>::new();
  probe.try_reserve(layout.size()).map_err(|_allocation| ENOMEM)?;
  drop(probe);
  Ok(Arc::new(value))
}

pub struct MemPool<T> {
  bufs: Mutex<Vec<Vec<T>>>,
}

impl<T> MemPool<T> {
  pub const fn new() -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self {
        bufs: Mutex::new(Vec::new()),
      }
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(explicit088_no_arriving_length, reason = "the requested length must fit the target's native index and the allocation layout before reserving storage")
  )]
  pub fn pop_init(&self, size: u64, init_value: T) -> Rav1dResult<Vec<T>>
  where
    T: Copy,
  {
    let Ok(native_size) = index(size);
    let Ok(round_trip) = fixed_index(native_size);

    match round_trip.cmp(&size) {
      Ordering::Equal => {}
      Ordering::Less | Ordering::Greater => return Err(ENOMEM),
    }

    let _layout = Layout::array::<T>(native_size).map_err(|_invalid_layout| ENOMEM)?;

    match self.bufs.try_lock().and_then(|mut buffers| buffers.pop()) {
      Some(buf) => match native_size.cmp(&buf.len()) {
        Ordering::Less | Ordering::Equal => return Ok(buf),
        Ordering::Greater => {}
      },
      None => {}
    }

    let mut buf = Vec::new();
    buf.try_reserve(native_size).map_err(|_allocation| ENOMEM)?;
    buf.resize(native_size, init_value);
    Ok(buf)
  }

  pub fn push(&self, buf: Vec<T>) -> core::result::Result<(), console_core_never::Never> {
    match self.bufs.try_lock() {
      Some(mut buffers) => buffers.push(buf),
      None => drop(buf),
    }

    Ok(())
  }
}

impl<T> Default for MemPool<T> {
  fn default() -> Self {
    let Ok(mem_pool) = Self::new();

    mem_pool
  }
}
