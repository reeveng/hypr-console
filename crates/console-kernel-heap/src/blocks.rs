//! Headers kept in free memory, and the arithmetic that splits and joins them.

use core::alloc::Layout;
use core::ptr::NonNull;

use crate::{Bytes, HeapError};

pub(crate) const UNIT: u64 = 16;

#[repr(C, align(16))]
pub(crate) struct Region {
  pub(crate) end: u64,
  pub(crate) next: Option<NonNull<Region>>,
}

#[derive(Clone, Copy)]
#[repr(C, align(16))]
pub(crate) struct Free {
  pub(crate) bytes: Bytes,
  pub(crate) next: Option<NonNull<Free>>,
}

const _: [(); 16] = [(); core::mem::size_of::<Free>()];
const _: [(); 16] = [(); core::mem::size_of::<Region>()];

#[derive(Clone, Copy)]
pub(crate) struct Request {
  pub(crate) bytes: Bytes,
  pub(crate) alignment: Bytes,
}

#[derive(Clone, Copy)]
pub(crate) struct Fit {
  pub(crate) at: NonNull<u8>,
  pub(crate) bytes: Bytes,
  pub(crate) before: Bytes,
  pub(crate) after: Bytes,
}

impl Request {
  pub(crate) fn new(layout: Layout) -> Result<Self, HeapError> {
    let size = u64::try_from(layout.size()).map_err(|_too_wide| HeapError::Overflow)?;
    let alignment = u64::try_from(layout.align()).map_err(|_too_wide| HeapError::Overflow)?;
    let bytes = size
      .max(UNIT)
      .checked_next_multiple_of(UNIT)
      .ok_or(HeapError::Overflow)?;

    Ok(Request {
      bytes: Bytes(bytes),
      alignment: Bytes(alignment.max(UNIT)),
    })
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  allow(
    explicit051_no_machine_width,
    reason = "a checked byte distance becomes the machine width only where a pointer is advanced within lent memory"
  )
)]
pub(crate) fn advance(at: NonNull<u8>, bytes: Bytes) -> Result<NonNull<u8>, HeapError> {
  let distance = usize::try_from(bytes.0).map_err(|_too_wide| HeapError::Overflow)?;
  let address = at.addr().checked_add(distance).ok_or(HeapError::Overflow)?;
  Ok(at.with_addr(address))
}

pub(crate) fn fit(
  at: NonNull<Free>,
  block: Free,
  request: Request,
) -> Result<Option<Fit>, HeapError> {
  let address = u64::try_from(at.as_ptr().addr()).map_err(|_too_wide| HeapError::Overflow)?;
  let start = address
    .checked_next_multiple_of(request.alignment.0)
    .ok_or(HeapError::Overflow)?;
  let before = start.checked_sub(address).ok_or(HeapError::Overflow)?;
  let used = before
    .checked_add(request.bytes.0)
    .ok_or(HeapError::Overflow)?;

  match block.bytes.0.checked_sub(used) {
    Some(after) => {
      let pointer = advance(at.cast(), Bytes(before))?;

      Ok(Some(Fit {
        at: pointer,
        bytes: request.bytes,
        before: Bytes(before),
        after: Bytes(after),
      }))
    }
    None => Ok(None),
  }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the heap owns both neighboring free-list headers in place, with no other references to them"
  )
)]
pub(crate) fn merge(
  mut at: NonNull<Free>,
  mut block: Free,
  previous: Option<NonNull<Free>>,
) -> Result<(NonNull<Free>, Free), HeapError> {
  match block.next {
    Some(next) => {
      let address = u64::try_from(at.as_ptr().addr()).map_err(|_too_wide| HeapError::Overflow)?;
      let end = address
        .checked_add(block.bytes.0)
        .ok_or(HeapError::Overflow)?;
      let next_address =
        u64::try_from(next.as_ptr().addr()).map_err(|_too_wide| HeapError::Overflow)?;

      match end.cmp(&next_address) {
        core::cmp::Ordering::Equal => {
          // SAFETY: next is initialized metadata for the adjacent free block, exclusively held by the heap.
          let following = unsafe { *next.as_ref() };

          let joined = block
            .bytes
            .0
            .checked_add(following.bytes.0)
            .ok_or(HeapError::Overflow)?;

          block.bytes = Bytes(joined);
          block.next = following.next;
        }
        core::cmp::Ordering::Less => {}
        core::cmp::Ordering::Greater => return Err(HeapError::Overlap),
      }
    }
    None => {}
  }

  match previous {
    Some(node) => {
      // SAFETY: previous is initialized metadata in the heap's exclusive free list.
      let preceding = unsafe { *node.as_ref() };
      let address =
        u64::try_from(node.as_ptr().addr()).map_err(|_too_wide| HeapError::Overflow)?;
      let end = address
        .checked_add(preceding.bytes.0)
        .ok_or(HeapError::Overflow)?;
      let at_address =
        u64::try_from(at.as_ptr().addr()).map_err(|_too_wide| HeapError::Overflow)?;

      match end.cmp(&at_address) {
        core::cmp::Ordering::Equal => {
          at = node;
          let joined = block
            .bytes
            .0
            .checked_add(preceding.bytes.0)
            .ok_or(HeapError::Overflow)?;

          block.bytes = Bytes(joined);
        }
        core::cmp::Ordering::Less => {}
        core::cmp::Ordering::Greater => return Err(HeapError::Overlap),
      }
    }
    None => {}
  }

  Ok((at, block))
}
