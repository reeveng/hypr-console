//! Byte storage reserved at boot and shared by memory objects.
//!
//! Live spans stay ordered by start. Claiming a gap clears its bytes and
//! inserts a span in that order without growing the reserved allocation.
//! Released spans no longer occupy bytes, so adjacent gaps are reusable.

use alloc::alloc::{Layout, alloc_zeroed};
use alloc::vec::Vec;

use console_core_number_conversion::{checked_index, fixed_index, index};

use crate::{Bytes, HandleError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Span {
  start: u64,
  end: u64,
}

impl Span {
  const EMPTY: Self = Self { start: 0, end: 0 };
}

pub(crate) struct BackingLimits {
  pub(crate) bytes: Bytes,
  pub(crate) objects: u64,
}

pub(crate) struct Backing {
  bytes: Vec<u8>,
  live: Vec<Span>,
  object_limit: u64,
}

impl Backing {
  pub(crate) fn reserve_at_boot(limits: BackingLimits) -> Result<Self, HandleError> {
    let bytes = zeroed_at_boot(limits.bytes)?;
    let objects = checked_index(limits.objects).map_err(|_too_many| HandleError::NoResources)?;
    let _layout = Layout::array::<Span>(objects).map_err(|_too_large| HandleError::NoResources)?;

    #[cfg_attr(
      dylint_lib = "explicit066_no_heap_after_init",
      expect(
        explicit066_no_heap_after_init,
        reason = "live-span bookkeeping is reserved during Kernel::boot_with_memory_backing"
      )
    )]
    let mut live = Vec::new();

    #[cfg_attr(
      dylint_lib = "explicit088_no_arriving_length",
      expect(
        explicit088_no_arriving_length,
        reason = "this is a trusted boot limit and Layout::array checks the byte capacity against the allocator's representable range"
      )
    )]
    let reservation = live.try_reserve_exact(objects);

    reservation.map_err(|_no_reserve| HandleError::NoResources)?;

    Ok(Self {
      bytes,
      live,
      object_limit: limits.objects,
    })
  }

  pub(crate) fn claim(&mut self, Bytes(length): Bytes) -> Result<Span, HandleError> {
    match length {
      0 => return Ok(Span::EMPTY),
      1.. => {}
    }

    let Ok(live) = fixed_index(self.live.len());

    match bool::then_some(live >= self.object_limit, ()) {
      Some(()) => return Err(HandleError::NoResources),
      None => {}
    }

    let mut start = 0_u64;
    let mut before = live;

    for (position, span) in (0..live).zip(&self.live) {
      match bool::then_some(span.start.saturating_sub(start) >= length, ()) {
        Some(()) => {
          before = position;

          break;
        }
        None => start = span.end,
      }
    }

    let Ok(capacity) = fixed_index(self.bytes.len());
    let end = start
      .checked_add(length)
      .filter(|end| *end <= capacity)
      .ok_or(HandleError::NoResources)?;
    let span = Span { start, end };
    let bytes = self.contents(span)?;

    bytes.fill(0);

    let Ok(position) = index(before);
    let mut carried = span;

    for later in self.live.iter_mut().skip(position) {
      core::mem::swap(later, &mut carried);
    }

    self.live.push(carried);

    Ok(span)
  }

  pub(crate) fn release(&mut self, span: Span) -> Result<(), HandleError> {
    match bool::then_some(span == Span::EMPTY, ()) {
      Some(()) => return Ok(()),
      None => {}
    }

    let position = match self.live.binary_search_by_key(&span.start, |live| live.start) {
      Ok(native) => {
        let Ok(fixed) = fixed_index(native);

        fixed
      }
      Err(_absent) => return Err(HandleError::BadState),
    };
    let Ok(native) = index(position);
    let rest = self.live.get_mut(native..).ok_or(HandleError::BadState)?;

    match bool::then_some(rest.first() == Some(&span), ()) {
      Some(()) => rest.rotate_left(1),
      None => return Err(HandleError::BadState),
    }

    let _released = self.live.pop().ok_or(HandleError::BadState)?;

    Ok(())
  }

  pub(crate) fn contents(&mut self, span: Span) -> Result<&mut [u8], HandleError> {
    let start = checked_index(span.start).map_err(|_too_far| HandleError::BadState)?;
    let end = checked_index(span.end).map_err(|_too_far| HandleError::BadState)?;

    self.bytes.get_mut(start..end).ok_or(HandleError::BadState)
  }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "this boot allocation alone owns the checked zeroed buffer until it transfers ownership to Vec"
  )
)]
fn zeroed_at_boot(Bytes(count): Bytes) -> Result<Vec<u8>, HandleError> {
  match count {
    0 => return Ok(Vec::default()),
    1.. => {}
  }

  let length = checked_index(count).map_err(|_too_long| HandleError::NoResources)?;
  let layout = Layout::array::<u8>(length).map_err(|_too_large| HandleError::NoResources)?;

  // SAFETY: Layout::array checked the size and alignment for exactly length
  // bytes, and the early return above excludes zero-sized allocations.
  let storage = unsafe { alloc_zeroed(layout) };

  match bool::then_some(storage.is_null(), ()) {
    Some(()) => return Err(HandleError::NoResources),
    None => {}
  }

  #[cfg_attr(
    dylint_lib = "explicit096_no_unchecked_memory",
    expect(
      explicit096_no_unchecked_memory,
      reason = "the allocator answered the checked array layout above with a non-null, zero-initialized buffer; this transfers sole ownership into Vec"
    )
  )]
  // SAFETY: the global allocator returned exactly this layout and initialized
  // all bytes to zero. The vector takes sole ownership with the original size.
  let owned = unsafe { Vec::from_raw_parts(storage, length, length) };

  Ok(owned)
}
