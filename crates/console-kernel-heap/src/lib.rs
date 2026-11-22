//! Memory lent to a heap, divided, and put back together when it is freed.
//!
//! The caller lends disjoint writable regions for the lifetime of the heap.
//! A region keeps its own header and every free block keeps its own next link
//! and size, so neither allocation nor freeing reaches for another allocator.
//! First fit splits a block at the requested alignment, keeps both remainders,
//! and freeing joins both neighbors when they meet. Region headers stay put:
//! even adjacent borrowed regions are never joined across their provenance.
//!
//! `allocate` hands out a raw pointer into the lent memory. The caller may
//! access exactly the bytes its layout requests, until `deallocate`, and must
//! finish every such access before the lent memory is returned. Deallocation
//! is unsafe: its pointer must be a live allocation from this heap, its layout
//! must be the original layout, and no reference into the allocation may be
//! used again. Moving a heap transfers its exclusive ownership of the regions;
//! sharing one between threads requires a lock around its mutable operations.
//!
//! This is deliberately the bookkeeping, not the kernel's policy. Frames,
//! page mappings, interrupts and synchronization belong to the caller. The
//! same allocator therefore runs over an ordinary borrowed buffer in tests.

#![no_std]

mod blocks;

use core::alloc::Layout;
use core::cmp::Ordering;
use core::fmt;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ptr::NonNull;

use console_core_never::Never;

use blocks::{Free, Region, Request, UNIT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Bytes(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeapError {
  Overflow,
  TooSmall,
  Exhausted,
  Foreign,
  Overlap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
  Here,
  Elsewhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Statistics {
  pub capacity: Bytes,
  pub free: Bytes,
  pub allocations: u64,
}

pub struct Heap<'memory> {
  regions: Option<NonNull<Region>>,
  free: Option<NonNull<Free>>,
  statistics: Statistics,
  borrowed: PhantomData<&'memory mut [MaybeUninit<u8>]>,
}

// SAFETY: moving the heap transfers exclusive access to its borrowed regions; no pointers are shared through a safe reference, and all changes require `&mut self`.
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the heap transfers its exclusive borrow on move, which the raw metadata pointers cannot express to Send"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit069_lock_discipline",
  expect(
    explicit069_lock_discipline,
    reason = "moving the heap transfers its exclusive borrow of the regions; allocator metadata is accessible only through the new owner"
  )
)]
unsafe impl Send for Heap<'_> {}

impl<'memory> Heap<'memory> {
  #[must_use]
  pub const fn new() -> Result<Self, Never> {
    Ok(Heap {
      regions: None,
      free: None,
      statistics: Statistics {
        capacity: Bytes(0),
        free: Bytes(0),
        allocations: 0,
      },
      borrowed: PhantomData,
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the exclusive slice and checked alignment reserve space for the region header within the provided buffer"
    )
  )]
  pub fn add(&mut self, memory: &'memory mut [MaybeUninit<u8>]) -> Result<(), HeapError> {
    let length = u64::try_from(memory.len()).map_err(|_too_wide| HeapError::Overflow)?;
    let address =
      u64::try_from(memory.as_mut_ptr().addr()).map_err(|_too_wide| HeapError::Overflow)?;
    let start = address
      .checked_next_multiple_of(UNIT)
      .ok_or(HeapError::Overflow)?;
    let limit = address.checked_add(length).ok_or(HeapError::Overflow)?;
    let end = limit
      .checked_sub(limit.rem_euclid(UNIT))
      .ok_or(HeapError::Overflow)?;
    let bytes = end
      .checked_sub(start)
      .and_then(|bytes| bytes.checked_sub(UNIT))
      .ok_or(HeapError::TooSmall)?;

    match bytes.cmp(&UNIT) {
      Ordering::Equal | Ordering::Greater => {}
      Ordering::Less => return Err(HeapError::TooSmall),
    }

    let capacity = self
      .statistics
      .capacity
      .0
      .checked_add(bytes)
      .ok_or(HeapError::Overflow)?;
    let free = self
      .statistics
      .free
      .0
      .checked_add(bytes)
      .ok_or(HeapError::Overflow)?;
    let offset = start.checked_sub(address).ok_or(HeapError::Overflow)?;
    let base = NonNull::from(memory).cast::<u8>();
    let header = blocks::advance(base, Bytes(offset))?;
    let region = header.cast::<Region>();

    // SAFETY: the reserved header is aligned, writable, and not part of any allocation.
    unsafe {
      region.as_ptr().write(Region {
        end,
        next: self.regions,
      })
    };

    let block = blocks::advance(region.cast(), Bytes(UNIT))?;

    self.insert(block.cast(), Bytes(bytes))?;
    self.regions = Some(region);
    self.statistics.capacity = Bytes(capacity);
    self.statistics.free = Bytes(free);

    Ok(())
  }

  pub fn required(layout: Layout) -> Result<Bytes, HeapError> {
    let request = Request::new(layout)?;
    let bytes = request
      .bytes
      .0
      .checked_add(request.alignment.0)
      .and_then(|bytes| bytes.checked_add(UNIT.saturating_mul(2)))
      .ok_or(HeapError::Overflow)?;

    Ok(Bytes(bytes))
  }

  #[must_use]
  pub fn statistics(&self) -> Result<Statistics, Never> {
    Ok(self.statistics)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the region list contains only initialized headers reserved by the heap, whose immutable borrow holds them live"
    )
  )]
  pub fn owns(&self, at: NonNull<u8>) -> Result<Ownership, Never> {
    let address = match u64::try_from(at.as_ptr().addr()) {
      Ok(address) => address,
      Err(_too_wide) => return Ok(Ownership::Elsewhere),
    };
    let mut region = self.regions;

    for _ in 0..self.statistics.capacity.0.div_euclid(UNIT) {
      let pointer = match region {
        Some(pointer) => pointer,
        None => break,
      };

      // SAFETY: region headers remain initialized and reserved for the entire lifetime of the heap.
      let held = unsafe { pointer.as_ref() };
      let origin = match u64::try_from(pointer.as_ptr().addr()) {
        Ok(origin) => origin,
        Err(_too_wide) => return Ok(Ownership::Elsewhere),
      };
      let start = origin.saturating_add(UNIT);

      match (address.cmp(&start), address.cmp(&held.end)) {
        (Ordering::Equal | Ordering::Greater, Ordering::Less) => return Ok(Ownership::Here),
        (Ordering::Less, _) | (_, Ordering::Equal | Ordering::Greater) => region = held.next,
      }
    }

    Ok(Ownership::Elsewhere)
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "free-list links refer to initialized headers kept in the unavailable portion of the caller's borrowed storage"
    )
  )]
  pub fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>, HeapError> {
    let request = Request::new(layout)?;
    let mut current = self.free;
    let mut previous = None;

    for _ in 0..self.statistics.capacity.0.div_euclid(UNIT) {
      let pointer = match current {
        Some(pointer) => pointer,
        None => break,
      };

      // SAFETY: every free-list link names an initialized header in exclusively borrowed, unallocated memory.
      let block = unsafe { *pointer.as_ref() };
      let found = blocks::fit(pointer, block, request)?;

      match found {
        Some(fit) => {
          self.take(previous, pointer, block, fit)?;

          return Ok(fit.at);
        }
        None => {
          previous = current;
          current = block.next;
        }
      }
    }

    Err(HeapError::Exhausted)
  }

  #[allow(
    clippy::missing_safety_doc,
    reason = "the module head states the allocation, layout, lifetime and exclusive-access requirements of deallocate"
  )]
  pub unsafe fn deallocate(&mut self, at: NonNull<u8>, layout: Layout) -> Result<(), HeapError> {
    let request = Request::new(layout)?;
    let Ok(ownership) = self.owns(at);

    match ownership {
      Ownership::Here => {}
      Ownership::Elsewhere => return Err(HeapError::Foreign),
    }

    let free = self
      .statistics
      .free
      .0
      .checked_add(request.bytes.0)
      .ok_or(HeapError::Overflow)?;
    let allocations = self
      .statistics
      .allocations
      .checked_sub(1)
      .ok_or(HeapError::Overlap)?;

    self.insert(at.cast(), request.bytes)?;
    self.statistics.free = Bytes(free);
    self.statistics.allocations = allocations;

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "fit verifies the split and the heap owns the aligned prefix and suffix where it leaves free-list headers"
    )
  )]
  fn take(
    &mut self,
    previous: Option<NonNull<Free>>,
    pointer: NonNull<Free>,
    block: Free,
    fit: blocks::Fit,
  ) -> Result<(), HeapError> {
    let free = self
      .statistics
      .free
      .0
      .checked_sub(fit.bytes.0)
      .ok_or(HeapError::Overflow)?;
    let allocations = self
      .statistics
      .allocations
      .checked_add(1)
      .ok_or(HeapError::Overflow)?;
    let mut next = block.next;

    match fit.after.0 {
      0 => {}
      _remaining => {
        let tail = blocks::advance(fit.at, fit.bytes)?;
        let suffix = tail.cast::<Free>();

        // SAFETY: the aligned suffix is at least a header long, lies within this free block, and does not overlap the allocation.
        unsafe {
          suffix.as_ptr().write(Free {
            bytes: fit.after,
            next,
          })
        };

        next = Some(suffix);
      }
    }

    match fit.before.0 {
      0 => {}
      _remaining => {
        // SAFETY: the prefix is an aligned free block at least a header long, and the allocation starts after it.
        unsafe {
          pointer.as_ptr().write(Free {
            bytes: fit.before,
            next,
          })
        };

        next = Some(pointer);
      }
    }

    let Ok(()) = self.link(previous, next);
    self.statistics.free = Bytes(free);
    self.statistics.allocations = allocations;

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the predecessor's header lives in free memory and this mutable heap borrow exclusively owns the list"
    )
  )]
  fn link(
    &mut self,
    previous: Option<NonNull<Free>>,
    next: Option<NonNull<Free>>,
  ) -> Result<(), Never> {
    match previous {
      Some(mut node) => {
        // SAFETY: the predecessor is a live free-list node exclusively held through this mutable heap borrow.
        unsafe { node.as_mut().next = next };
      }
      None => self.free = next,
    }

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the heap owns initialized free-list links and merge checks the bounds of the free block that receives the new header"
    )
  )]
  fn insert(&mut self, at: NonNull<Free>, bytes: Bytes) -> Result<(), HeapError> {
    let mut next = self.free;
    let mut previous = None;

    for _ in 0..self.statistics.capacity.0.div_euclid(UNIT) {
      let pointer = match next {
        Some(pointer) => pointer,
        None => break,
      };

      match pointer.as_ptr().addr().cmp(&at.as_ptr().addr()) {
        Ordering::Less => {
          previous = Some(pointer);
          // SAFETY: the node is initialized free-list metadata and the heap is exclusively borrowed.
          next = unsafe { pointer.as_ref().next };
        }
        Ordering::Equal | Ordering::Greater => break,
      }
    }

    let (merged, block) = blocks::merge(at, Free { bytes, next }, previous)?;

    // SAFETY: add supplies exclusive borrowed memory and deallocate requires a live allocation; merge retains only unallocated neighboring blocks.
    unsafe { merged.as_ptr().write(block) };

    match previous.filter(|predecessor| *predecessor == merged) {
      Some(_) => {}
      None => {
        let Ok(()) = self.link(previous, Some(merged));
      }
    }

    Ok(())
  }
}

impl fmt::Display for HeapError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      HeapError::Overflow => to.write_str("the heap's size or address overflowed"),
      HeapError::TooSmall => to.write_str("the region cannot hold the heap's headers"),
      HeapError::Exhausted => to.write_str("no free block fits the allocation"),
      HeapError::Foreign => to.write_str("the allocation belongs to another allocator"),
      HeapError::Overlap => to.write_str("the block is already free"),
    }
  }
}

impl core::error::Error for HeapError {}
