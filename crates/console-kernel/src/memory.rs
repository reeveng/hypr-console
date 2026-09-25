//! Memory as the firmware leaves it, and the kernel's own bookkeeping over it.
//!
//! When boot services are exited the firmware hands over a memory map and
//! nothing else: a run of descriptors, each a kind, a physical start and a
//! count of pages. What is conventional memory is the kernel's from then on,
//! and everything else -- the firmware's runtime code, the loader's own image
//! and pool, the tables ACPI reads -- is left where it lies. Boot services
//! memory is free too once they are gone, but the stack the kernel is running
//! on is in it, so it stays unclaimed until the kernel has a stack of its own.
//!
//! The frames are handed out by a cursor that only climbs, and the heap is a
//! bump over chunks of them that never gives anything back. Neither frees,
//! because nothing the kernel does yet ends: a linked list of free blocks is
//! the next step when something does, and it is a step inside `Heap` rather
//! than a change to anybody who allocates. The frame at address zero is never
//! handed out, because a pointer to it is the null the allocator answers
//! failure with.
//!
//! Nothing here touches a machine. The map is bytes and the answers are
//! numbers, so every step is tested on this machine against a map written by
//! hand, and the firmware side only reads the map and hands the bytes over.

use core::alloc::Layout;
use core::fmt;
use core::slice::ChunksExact;

use console_core_never::Never;

pub const PAGE: u64 = 4096;

pub const MEBIBYTE: u64 = 1_048_576;

const CHUNK_PAGES: u64 = 1024;

const CONVENTIONAL: u32 = 7;

const DESCRIPTOR_SIZE: u64 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryError {
    DescriptorTooSmall(u64),
    Overflow,
    OutOfSpace,
    OutOfFrames,
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

#[derive(Debug, Clone)]
pub struct Frames<'a> {
    map: MemoryMap<'a>,
    next: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bump {
    next: u64,
    end: u64,
}

#[derive(Debug, Clone)]
pub struct Heap<'a> {
    frames: Frames<'a>,
    bump: Bump,
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
        allow(explicit051_no_machine_width, reason = "a slice is walked in strides of `usize`, and `console-core-number-conversion`, where the width is otherwise met, links `std` for `f64::round` and so cannot be linked into a kernel")
    )]
    pub fn new(bytes: &'a [u8], descriptor_size: u64) -> Result<Self, MemoryError> {
        let stride = match descriptor_size >= DESCRIPTOR_SIZE {
            true => usize::try_from(descriptor_size).map_err(|_too_wide| MemoryError::Overflow),
            false => Err(MemoryError::DescriptorTooSmall(descriptor_size)),
        };
        let stride = stride?;

        Ok(MemoryMap { descriptors: bytes.chunks_exact(stride) })
    }

    pub fn usable(&self) -> Result<impl Iterator<Item = Region> + use<'a>, Never> {
        Ok(self.descriptors.clone().filter_map(|descriptor| {
            let (kind, rest) = descriptor.split_first_chunk::<4>()?;
            let (_padding, rest) = rest.split_first_chunk::<4>()?;
            let (start, rest) = rest.split_first_chunk::<8>()?;
            let (_virtual_start, rest) = rest.split_first_chunk::<8>()?;
            let (pages, _attributes) = rest.split_first_chunk::<8>()?;

            match u32::from_le_bytes(*kind) == CONVENTIONAL {
                true => Some(Region { start: u64::from_le_bytes(*start), pages: u64::from_le_bytes(*pages) }),
                false => None,
            }
        }))
    }

    pub fn usable_bytes(&self) -> Result<u64, MemoryError> {
        let Ok(mut usable) = self.usable();

        usable.try_fold(0_u64, |sum, region| {
            let bytes = region.pages.checked_mul(PAGE).ok_or(MemoryError::Overflow)?;

            sum.checked_add(bytes).ok_or(MemoryError::Overflow)
        })
    }
}

impl<'a> Frames<'a> {
    #[must_use]
    pub fn new(map: MemoryMap<'a>) -> Result<Self, Never> {
        Ok(Frames { map, next: PAGE })
    }

    pub fn take(&mut self, pages: u64) -> Result<u64, MemoryError> {
        let bytes = pages.checked_mul(PAGE).ok_or(MemoryError::Overflow)?;
        let Ok(usable) = self.map.usable();

        for region in usable {
            let from = region.start.max(self.next);
            let until = from.checked_add(bytes).ok_or(MemoryError::Overflow)?;
            let end = region.end()?;

            match until <= end {
                true => {
                    self.next = until;

                    return Ok(from);
                },
                false => {},
            }
        }

        Err(MemoryError::OutOfFrames)
    }
}

impl Bump {
    fn allocate(&mut self, layout: Layout) -> Result<u64, MemoryError> {
        let size = u64::try_from(layout.size()).map_err(|_too_wide| MemoryError::Overflow)?;
        let align = u64::try_from(layout.align()).map_err(|_too_wide| MemoryError::Overflow)?;
        let start = self.next.checked_next_multiple_of(align).ok_or(MemoryError::Overflow)?;
        let end = start.checked_add(size).ok_or(MemoryError::Overflow)?;

        match end <= self.end {
            true => {
                self.next = end;

                Ok(start)
            },
            false => Err(MemoryError::OutOfSpace),
        }
    }
}

impl<'a> Heap<'a> {
    #[must_use]
    pub fn new(frames: Frames<'a>) -> Result<Self, Never> {
        Ok(Heap { frames, bump: Bump { next: 0, end: 0 } })
    }

    pub fn allocate(&mut self, layout: Layout) -> Result<u64, MemoryError> {
        match self.bump.allocate(layout) {
            Ok(at) => Ok(at),
            Err(MemoryError::OutOfSpace) => {
                self.grow(layout)?;

                self.bump.allocate(layout)
            },
            Err(error @ (MemoryError::DescriptorTooSmall(_) | MemoryError::Overflow | MemoryError::OutOfFrames)) => Err(error),
        }
    }

    fn grow(&mut self, layout: Layout) -> Result<(), MemoryError> {
        let size = u64::try_from(layout.size()).map_err(|_too_wide| MemoryError::Overflow)?;
        let align = u64::try_from(layout.align()).map_err(|_too_wide| MemoryError::Overflow)?;
        let needed = size.checked_add(align).ok_or(MemoryError::Overflow)?;
        let pages = needed.div_ceil(PAGE).max(CHUNK_PAGES);
        let start = self.frames.take(pages)?;
        let bytes = pages.checked_mul(PAGE).ok_or(MemoryError::Overflow)?;
        let end = start.checked_add(bytes).ok_or(MemoryError::Overflow)?;

        self.bump = Bump { next: start, end };

        Ok(())
    }
}

impl fmt::Display for MemoryError {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryError::DescriptorTooSmall(size) => write!(to, "a memory map descriptor of {size} bytes is smaller than the specification's"),
            MemoryError::Overflow => to.write_str("an address went past the end of the address space"),
            MemoryError::OutOfSpace => to.write_str("the heap has no room left in the frames it holds"),
            MemoryError::OutOfFrames => to.write_str("no usable region has that many frames left"),
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
            (CONVENTIONAL, Region { start: 0, pages: 160 }),
            (LOADER_DATA, Region { start: 0x10_0000, pages: 16 }),
            (CONVENTIONAL, Region { start: 0x20_0000, pages: 2048 }),
            (BOOT_SERVICES_DATA, Region { start: 0xa0_0000, pages: 64 }),
            (CONVENTIONAL, Region { start: 0x100_0000, pages: 4 }),
        ])
    }

    #[cfg_attr(
        dylint_lib = "explicit051_no_machine_width",
        allow(explicit051_no_machine_width, reason = "a `Layout` is measured in `usize` by `alloc`, and the test asks for one at the width the kernel holds sizes in")
    )]
    fn layout(size: u64, align: u64) -> Result<Layout, Missed> {
        let size = usize::try_from(size).map_err(|_too_wide| Missed::Layout)?;
        let align = usize::try_from(align).map_err(|_too_wide| Missed::Layout)?;

        Layout::from_size_align(size, align).map_err(|_not_a_layout| Missed::Layout)
    }

    #[test]
    fn only_conventional_memory_is_usable_at_the_firmwares_stride() -> Result<(), Missed> {
        let bytes = firmware()?;
        let map = MemoryMap::new(&bytes, FIRMWARE_STRIDE)?;

        let Ok(usable) = map.usable();

        assert_eq!(
            usable.collect::<Vec<_>>(),
            [Region { start: 0, pages: 160 }, Region { start: 0x20_0000, pages: 2048 }, Region { start: 0x100_0000, pages: 4 }]
        );
        assert_eq!(map.usable_bytes()?, 2212 * PAGE);

        Ok(())
    }

    #[test]
    fn a_descriptor_smaller_than_the_specifications_is_refused() -> Result<(), Missed> {
        let bytes = firmware()?;

        assert_eq!(MemoryMap::new(&bytes, 32).err(), Some(MemoryError::DescriptorTooSmall(32)));

        Ok(())
    }

    #[test]
    fn frames_skip_zero_and_move_on_when_a_region_is_too_small() -> Result<(), Missed> {
        let bytes = firmware()?;
        let Ok(mut frames) = Frames::new(MemoryMap::new(&bytes, FIRMWARE_STRIDE)?);

        assert_eq!(frames.take(159)?, PAGE);
        assert_eq!(frames.take(1)?, 0x20_0000);
        assert_eq!(frames.take(2047)?, 0x20_1000);
        assert_eq!(frames.take(2)?, 0x100_0000);
        assert_eq!(frames.take(4), Err(MemoryError::OutOfFrames));
        assert_eq!(frames.take(2)?, 0x100_2000);
        assert_eq!(frames.take(1), Err(MemoryError::OutOfFrames));

        Ok(())
    }

    #[test]
    fn the_heap_aligns_grows_into_new_frames_and_says_when_they_run_out() -> Result<(), Missed> {
        let bytes = firmware()?;
        let Ok(frames) = Frames::new(MemoryMap::new(&bytes, FIRMWARE_STRIDE)?);
        let Ok(mut heap) = Heap::new(frames);

        assert_eq!(heap.allocate(layout(3, 1)?)?, 0x20_0000);
        assert_eq!(heap.allocate(layout(8, 8)?)?, 0x20_0008);
        assert_eq!(heap.allocate(layout(16, 4096)?)?, 0x20_1000);
        assert_eq!(heap.allocate(layout(0x3f_f000, 8)?)?, 0x60_0000);
        assert_eq!(heap.allocate(layout(0x10_0000, 8)?), Err(MemoryError::OutOfFrames));
        assert_eq!(heap.allocate(layout(64, 8)?)?, 0x9f_f000);

        Ok(())
    }
}
