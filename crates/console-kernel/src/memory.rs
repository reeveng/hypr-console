//! Memory as the firmware leaves it, and the kernel's own bookkeeping over it.
//!
//! When boot services are exited the firmware hands over a memory map and
//! nothing else: a run of descriptors, each a kind, a physical start and a
//! count of pages. What is conventional memory is the kernel's from then on,
//! and everything else -- the firmware's runtime code, the loader's own image
//! and pool, the tables ACPI reads -- is left where it lies. Boot services
//! memory is free too once they are gone, but the stack the kernel is running
//! on is in it, so it stays unclaimed until the kernel has a stack of its own.
//! Every descriptor is checked before the map can supply frames. Overlapping
//! descriptors are rejected even when only one of them is conventional memory.
//!
//! The frames are handed out by a cursor that only climbs. The heap lends
//! chunks of those frames to `console-kernel-heap`, which splits and rejoins
//! free blocks without allocating bookkeeping of its own. Freed bytes are
//! reused before another chunk is taken; chunks stay owned by the heap rather
//! than being returned to the frame cursor. The frame at address zero is never
//! handed out, because a pointer to it is the null the allocator answers
//! failure with. Firmware pool allocations are outside the heap's regions and
//! are retained after the firmware has gone rather than freed into this heap.
//! Constructing a physical heap is unsafe: every conventional region in the
//! map must be mapped writable RAM exclusively owned for the heap's lifetime,
//! and nobody may construct another allocator over the same frames.
//!
//! The map and frame cursor are arithmetic over bytes and addresses. The
//! physical heap is the edge that turns those addresses into writable memory;
//! its host test lends a real buffer rather than dereferencing a synthetic
//! firmware map.

use core::alloc::Layout;
use core::fmt;
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use core::slice::ChunksExact;

use console_core_never::Never;
use console_kernel_heap::{Bytes, HeapError, Ownership};

#[cfg(kani)]
#[path = "../proofs/memory.rs"]
mod proofs;

pub const PAGE: u64 = 4096;

pub const MEBIBYTE: u64 = 1_048_576;

const CHUNK_PAGES: u64 = 1024;

const CONVENTIONAL: u32 = 7;

const DESCRIPTOR_SIZE: u64 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryError {
  DescriptorTooSmall(u64),
  IncompleteDescriptor,
  EmptyRegion(u64),
  EmptyReservation,
  UnalignedRegion(u64),
  OverlappingRegions,
  Overflow,
  OutOfFrames,
  Heap(HeapError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
  pub start: u64,
  pub pages: u64,
}

#[derive(Debug, Clone)]
pub struct MemoryMap<'a> {
  descriptors: ChunksExact<'a, u8>,
}

#[derive(Debug)]
pub struct Frames<'a> {
  map: MemoryMap<'a>,
  next: u64,
  region_end: u64,
}

pub struct Heap<'a> {
  frames: Frames<'a>,
  blocks: console_kernel_heap::Heap<'a>,
}

impl Region {
  pub fn end(&self) -> Result<u64, MemoryError> {
    let bytes = self.pages.checked_mul(PAGE).ok_or(MemoryError::Overflow)?;

    self.start.checked_add(bytes).ok_or(MemoryError::Overflow)
  }
}

impl<'a> MemoryMap<'a> {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    allow(
      explicit051_no_machine_width,
      reason = "a slice is walked in strides of `usize`, and `console-core-number-conversion`, where the width is otherwise met, links `std` for `f64::round` and so cannot be linked into a kernel"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit056_no_hidden_panic_api",
    expect(
      explicit056_no_hidden_panic_api,
      reason = "only a stride at least the firmware descriptor size reaches chunks_exact"
    )
  )]
  pub fn new(bytes: &'a [u8], descriptor_size: u64) -> Result<Self, MemoryError> {
    let stride = match ::core::primitive::bool::then_some(descriptor_size >= DESCRIPTOR_SIZE, ())
    {
      ::core::option::Option::Some(()) => {
        usize::try_from(descriptor_size).map_err(|_too_wide| MemoryError::Overflow)
      }
      ::core::option::Option::None => Err(MemoryError::DescriptorTooSmall(descriptor_size)),
    };
    let stride_2 = stride?;

    let descriptors = bytes.chunks_exact(stride_2);

    match ::core::primitive::bool::then_some(!descriptors.remainder().is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        return Err(MemoryError::IncompleteDescriptor);
      }
      ::core::option::Option::None => {}
    }

    let map = Self { descriptors };
    map.validate()?;

    Ok(map)
  }

  fn regions(&self) -> Result<impl Iterator<Item = (u32, Region)> + Clone + use<'a>, Never> {
    Ok(self.descriptors.clone().filter_map(|descriptor| {
      let (kind, rest) = descriptor.split_first_chunk::<4>()?;
      let (_padding, rest_2) = rest.split_first_chunk::<4>()?;
      let (start, rest_3) = rest_2.split_first_chunk::<8>()?;
      let (_virtual_start, rest_4) = rest_3.split_first_chunk::<8>()?;
      let (pages, _attributes) = rest_4.split_first_chunk::<8>()?;

      Some((
        u32::from_le_bytes(*kind),
        Region {
          start: u64::from_le_bytes(*start),
          pages: u64::from_le_bytes(*pages),
        },
      ))
    }))
  }

  fn validate(&self) -> Result<(), MemoryError> {
    let mut previous_start = 0;
    let mut previous_end = 0;
    let mut unordered = false;
    let Ok(regions) = self.regions();

    for (_, region) in regions {
      match (region.pages, region.start.is_multiple_of(PAGE)) {
        (0, _) => return Err(MemoryError::EmptyRegion(region.start)),
        (_, false) => return Err(MemoryError::UnalignedRegion(region.start)),
        (_, true) => {}
      }

      match (region.start < previous_start, region.start < previous_end) {
        (true, _) => unordered = true,
        (false, true) => return Err(MemoryError::OverlappingRegions),
        (false, false) => {}
      }

      previous_start = region.start;
      let checked_value = region.end()?;
      previous_end = checked_value;
    }

    match ::core::primitive::bool::then_some(unordered, ()) {
      ::core::option::Option::Some(()) => self.validate_unordered(),
      ::core::option::Option::None => Ok(()),
    }
  }

  fn validate_unordered(&self) -> Result<(), MemoryError> {
    let Ok(regions) = self.regions();
    let comparisons = self.descriptors.clone().scan(regions, |remaining, _| {
      let (_, region) = remaining.next()?;

      Some((region, remaining.clone()))
    });

    for (region, later_regions) in comparisons {
      let end = region.end()?;

      for (_, later) in later_regions {
        let checked_value = later.end()?;

        match ::core::primitive::bool::then_some(
          region.start < checked_value && later.start < end,
          (),
        ) {
          ::core::option::Option::Some(()) => return Err(MemoryError::OverlappingRegions),
          ::core::option::Option::None => {}
        }
      }
    }

    Ok(())
  }

  pub fn usable(&self) -> Result<impl Iterator<Item = Region> + use<'a>, Never> {
    let Ok(regions) = self.regions();

    Ok(regions.filter_map(|(kind, region)| (kind == CONVENTIONAL).then_some(region)))
  }

  pub fn usable_bytes(&self) -> Result<u64, MemoryError> {
    let Ok(mut usable) = self.usable();

    usable.try_fold(0_u64, |sum, region| {
      let bytes = region
        .pages
        .checked_mul(PAGE)
        .ok_or(MemoryError::Overflow)?;

      sum.checked_add(bytes).ok_or(MemoryError::Overflow)
    })
  }
}

impl<'a> Frames<'a> {
  #[must_use]
  pub fn new(map: MemoryMap<'a>) -> Result<Self, Never> {
    Ok(Frames {
      map,
      next: PAGE,
      region_end: PAGE,
    })
  }

  pub fn take(&mut self, pages: u64) -> Result<u64, MemoryError> {
    match pages {
      0 => return Err(MemoryError::EmptyReservation),
      _ => {}
    }

    let bytes = pages.checked_mul(PAGE).ok_or(MemoryError::Overflow)?;
    let cursor = self.next;

    match ::core::primitive::bool::then_some(self.region_end.saturating_sub(cursor) >= bytes, ())
    {
      ::core::option::Option::Some(()) => {
        let checked_value = cursor.checked_add(bytes).ok_or(MemoryError::Overflow)?;
        self.next = checked_value;
        return Ok(cursor);
      }
      ::core::option::Option::None => {}
    }

    let Ok(usable) = self.map.usable();
    let mut lowest = None;

    for region in usable {
      let from = region.start.max(cursor);
      let end = region.end()?;

      match ::core::primitive::bool::then_some(end.saturating_sub(from) >= bytes, ()) {
        ::core::option::Option::Some(()) => {
          lowest = Some(lowest.map_or((from, end), |previous: (u64, u64)| {
            previous.min((from, end))
          }));
        }
        ::core::option::Option::None => {}
      }
    }

    let (from, end) = lowest.ok_or(MemoryError::OutOfFrames)?;
    let checked_value_2 = from.checked_add(bytes).ok_or(MemoryError::Overflow)?;
    self.next = checked_value_2;
    self.region_end = end;

    Ok(from)
  }
}

impl<'a> Heap<'a> {
  #[must_use]
  #[allow(
    clippy::missing_safety_doc,
    reason = "the module head states the mapping, ownership and lifetime requirements of the physical heap constructor"
  )]
  pub unsafe fn new(frames: Frames<'a>) -> Result<Self, Never> {
    let Ok(blocks) = console_kernel_heap::Heap::new();

    Ok(Heap { frames, blocks })
  }

  pub fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>, MemoryError> {
    match self.blocks.allocate(layout) {
      Ok(at) => Ok(at),
      Err(HeapError::Exhausted) => {
        self.grow(layout)?;

        self.blocks.allocate(layout).map_err(MemoryError::Heap)
      }
      Err(
        error @ (HeapError::Overflow
        | HeapError::TooSmall
        | HeapError::Foreign
        | HeapError::Overlap),
      ) => Err(MemoryError::Heap(error)),
    }
  }

  #[allow(
    clippy::missing_safety_doc,
    reason = "deallocation requires a live kernel or pre-handover firmware allocation and its original layout, exactly as the global allocator's caller promises"
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the global allocator passes a live allocation with its original layout and the block owner is checked before returning it"
    )
  )]
  pub unsafe fn deallocate(
    &mut self,
    at: NonNull<u8>,
    layout: Layout,
  ) -> Result<(), MemoryError> {
    let Ok(ownership) = self.blocks.owns(at);

    match ownership {
      Ownership::Here => {
        // SAFETY: the caller supplies a live allocation and its original layout; ownership was checked before forwarding it to the heap.
        unsafe { self.blocks.deallocate(at, layout) }.map_err(MemoryError::Heap)
      }
      Ownership::Elsewhere => Ok(()),
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    allow(
      explicit051_no_machine_width,
      reason = "a mapped frame address and slice length are converted to the machine width only where physical memory is lent to the heap"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit096_no_unchecked_memory",
    expect(
      explicit096_no_unchecked_memory,
      reason = "length is checked from the page count; the frame allocator hands over disjoint identity-mapped pages owned by this kernel"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the physical heap takes disjoint writable frames from the cursor and lends them once as uninitialized storage"
    )
  )]
  fn grow(&mut self, layout: Layout) -> Result<(), MemoryError> {
    let Bytes(needed) = console_kernel_heap::Heap::required(layout).map_err(MemoryError::Heap)?;
    let pages = needed.div_ceil(PAGE).max(CHUNK_PAGES);
    let start = self.frames.take(pages)?;
    let bytes = pages.checked_mul(PAGE).ok_or(MemoryError::Overflow)?;
    let address = usize::try_from(start).map_err(|_too_wide| MemoryError::Overflow)?;
    let length = usize::try_from(bytes).map_err(|_too_wide| MemoryError::Overflow)?;
    let pointer = core::ptr::with_exposed_provenance_mut::<MaybeUninit<u8>>(address);

    // SAFETY: the frame cursor lends disjoint conventional-memory pages, identity-mapped by UEFI and owned by the kernel after boot services exit; they stay mapped for the heap's lifetime.
    let memory = unsafe { core::slice::from_raw_parts_mut(pointer, length) };

    self.blocks.add(memory).map_err(MemoryError::Heap)
  }
}

impl fmt::Display for MemoryError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      MemoryError::DescriptorTooSmall(size) => write!(
        to,
        "a memory map descriptor of {size} bytes is smaller than the specification's"
      ),
      MemoryError::IncompleteDescriptor => {
        to.write_str("the memory map ends in the middle of a descriptor")
      }
      MemoryError::EmptyRegion(start) => write!(to, "the region at {start:#x} has no pages"),
      MemoryError::Overflow => to.write_str("an address went past the end of the address space"),
      MemoryError::EmptyReservation => {
        to.write_str("a frame reservation must contain at least one page")
      }
      MemoryError::UnalignedRegion(start) => write!(
        to,
        "the region at {start:#x} starts between page boundaries"
      ),
      MemoryError::OverlappingRegions => to.write_str("memory map descriptors overlap"),
      MemoryError::OutOfFrames => to.write_str("no usable region has that many frames left"),
      MemoryError::Heap(error) => write!(to, "{error}"),
    }
  }
}

#[cfg(test)]
mod tests {
  use alloc::vec::Vec;

  use super::*;

  const LOADER_DATA: u32 = 2;

  const BOOT_SERVICES_DATA: u32 = 4;

  const FIRMWARE_STRIDE: u64 = 48;

  #[derive(Debug)]
  enum Missed {
    Memory(MemoryError),
    Layout,
  }

  impl fmt::Display for Missed {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
      match self {
        Missed::Memory(error) => write!(to, "{error}"),
        Missed::Layout => to.write_str("the test asked for a layout that is not one"),
      }
    }
  }

  impl From<MemoryError> for Missed {
    fn from(error: MemoryError) -> Self {
      Missed::Memory(error)
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit066_no_heap_after_init",
    expect(
      explicit066_no_heap_after_init,
      reason = "this hosted fixture emulates a firmware memory map inside a buffer owned by the test harness"
    )
  )]
  fn map(regions: &[(u32, Region)]) -> Result<Vec<u8>, Missed> {
    let mut bytes = Vec::new();

    for (kind, region) in regions {
      bytes.extend_from_slice(&kind.to_le_bytes());
      bytes.extend_from_slice(&[0; 4]);
      bytes.extend_from_slice(&region.start.to_le_bytes());
      bytes.extend_from_slice(&region.start.to_le_bytes());
      bytes.extend_from_slice(&region.pages.to_le_bytes());
      bytes.extend_from_slice(&[0; 16]);
    }

    Ok(bytes)
  }

  fn firmware() -> Result<Vec<u8>, Missed> {
    map(&[
      (
        CONVENTIONAL,
        Region {
          start: 0,
          pages: 160,
        },
      ),
      (
        LOADER_DATA,
        Region {
          start: 0x10_0000,
          pages: 16,
        },
      ),
      (
        CONVENTIONAL,
        Region {
          start: 0x20_0000,
          pages: 2048,
        },
      ),
      (
        BOOT_SERVICES_DATA,
        Region {
          start: 0xa0_0000,
          pages: 64,
        },
      ),
      (
        CONVENTIONAL,
        Region {
          start: 0x100_0000,
          pages: 4,
        },
      ),
    ])
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    allow(
      explicit051_no_machine_width,
      reason = "a `Layout` is measured in `usize` by `alloc`, and the test asks for one at the width the kernel holds sizes in"
    )
  )]
  fn layout((size, align): (u64, u64)) -> Result<Layout, Missed> {
    let size_2 = usize::try_from(size).map_err(|_too_wide| Missed::Layout)?;
    let align_2 = usize::try_from(align).map_err(|_too_wide| Missed::Layout)?;

    Layout::from_size_align(size_2, align_2).map_err(|_not_a_layout| Missed::Layout)
  }

  #[test]
  fn only_conventional_memory_is_usable_at_the_firmwares_stride() -> Result<(), Missed> {
    let bytes = firmware()?;
    let map = MemoryMap::new(&bytes, FIRMWARE_STRIDE)?;

    let Ok(usable) = map.usable();

    assert_eq!(
      usable.collect::<Vec<_>>(),
      [
        Region {
          start: 0,
          pages: 160
        },
        Region {
          start: 0x20_0000,
          pages: 2048
        },
        Region {
          start: 0x100_0000,
          pages: 4
        }
      ]
    );
    let usable_bytes = map.usable_bytes()?;
    assert_eq!(usable_bytes, PAGE.saturating_mul(2212));

    Ok(())
  }

  #[test]
  fn a_descriptor_smaller_than_the_specifications_is_refused() -> Result<(), Missed> {
    let bytes = firmware()?;

    let refused = MemoryMap::new(&bytes, 32);

    assert!(
      matches!(refused, Err(MemoryError::DescriptorTooSmall(32))),
      "{refused:?}"
    );

    Ok(())
  }

  #[test]
  fn a_partial_final_descriptor_is_refused() -> Result<(), Missed> {
    let mut bytes = firmware()?;
    let _last = bytes.pop();

    assert!(matches!(
      MemoryMap::new(&bytes, FIRMWARE_STRIDE),
      Err(MemoryError::IncompleteDescriptor)
    ));

    Ok(())
  }

  #[test]
  fn frames_follow_physical_addresses_in_an_unordered_map() -> Result<(), Missed> {
    let bytes = map(&[
      (
        CONVENTIONAL,
        Region {
          start: PAGE.saturating_mul(16),
          pages: 1,
        },
      ),
      (
        CONVENTIONAL,
        Region {
          start: PAGE,
          pages: 2,
        },
      ),
    ])?;
    let map = MemoryMap::new(&bytes, FIRMWARE_STRIDE)?;
    let Ok(mut frames) = Frames::new(map);

    let first = frames.take(1)?;
    let second = frames.take(1)?;
    let third = frames.take(1)?;
    assert_eq!(first, PAGE);
    assert_eq!(second, PAGE.saturating_mul(2));
    assert_eq!(third, PAGE.saturating_mul(16));
    assert_eq!(frames.take(1), Err(MemoryError::OutOfFrames));

    Ok(())
  }

  #[test]
  fn frames_skip_zero_and_move_on_when_a_region_is_too_small() -> Result<(), Missed> {
    let bytes = firmware()?;
    let map = MemoryMap::new(&bytes, FIRMWARE_STRIDE)?;
    let Ok(mut frames) = Frames::new(map);

    let frame = frames.take(159)?;
    assert_eq!(frame, PAGE);
    let frame_2 = frames.take(1)?;
    assert_eq!(frame_2, 0x20_0000);
    let frame_3 = frames.take(2047)?;
    assert_eq!(frame_3, 0x20_1000);
    let frame_4 = frames.take(2)?;
    assert_eq!(frame_4, 0x100_0000);
    assert_eq!(frames.take(4), Err(MemoryError::OutOfFrames));
    let frame_5 = frames.take(2)?;
    assert_eq!(frame_5, 0x100_2000);
    assert_eq!(frames.take(1), Err(MemoryError::OutOfFrames));

    Ok(())
  }

  #[test]
  #[cfg_attr(
    dylint_lib = "explicit066_no_heap_after_init",
    expect(
      explicit066_no_heap_after_init,
      reason = "this hosted test supplies the simulated physical storage before exercising the allocator"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the test owns the backing buffer and returns only its live allocation with the exact original layout"
    )
  )]
  fn the_heap_reuses_freed_blocks_after_growing_and_exhausting_its_frames() -> Result<(), Missed>
  {
    let mut memory = alloc::vec![0_u8; 0x80_1000];
    let address = u64::try_from(memory.as_mut_ptr().expose_provenance())
      .map_err(|_too_wide| Missed::Layout)?;
    let start = address
      .checked_next_multiple_of(PAGE)
      .ok_or(Missed::Layout)?;
    let bytes = map(&[(CONVENTIONAL, Region { start, pages: 2048 })])?;
    let map = MemoryMap::new(&bytes, FIRMWARE_STRIDE)?;
    let Ok(frames) = Frames::new(map);

    // SAFETY: the map describes only the aligned interior of the live buffer, exclusively lent to this heap until it is dropped before the buffer.
    let Ok(mut heap) = unsafe { Heap::new(frames) };
    let wanted = layout((0x30_0000, 4096))?;
    let first = heap.allocate(wanted)?;
    let second = heap.allocate(wanted)?;

    assert_eq!(first.as_ptr().addr().rem_euclid(4096), 0);
    assert_eq!(second.as_ptr().addr().rem_euclid(4096), 0);
    assert!(second.as_ptr().addr() >= first.as_ptr().addr().saturating_add(0x30_0000));
    let too_large = layout((0x80_0000, 16))?;

    assert_eq!(heap.allocate(too_large), Err(MemoryError::OutOfFrames));

    // SAFETY: first is a live allocation from this heap, the layout is unchanged, and no references into it exist.
    unsafe { heap.deallocate(first, wanted) }?;

    let reused = heap.allocate(wanted)?;

    assert_eq!(reused, first);

    Ok(())
  }
}
