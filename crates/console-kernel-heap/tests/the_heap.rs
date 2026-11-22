//! A heap pressed through the bytes its callers keep, rather than its links.

use std::alloc::Layout;
use std::error::Error;
use std::mem::MaybeUninit;
use std::ptr::NonNull;

use console_core_never::Never;
use console_kernel_heap::{Bytes, Heap, HeapError, Ownership};

#[repr(align(4096))]
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  allow(
    explicit051_no_machine_width,
    reason = "a const array length has the machine width required by Rust's array type"
  )
)]
struct Arena<const N: usize>([MaybeUninit<u8>; N]);

struct Live {
  at: NonNull<u8>,
  layout: Layout,
  contents: u8,
}

impl Live {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the test initializes exactly the bytes of the exclusive block the heap just allocated"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit096_no_unchecked_memory",
    expect(
      explicit096_no_unchecked_memory,
      reason = "the allocator returned a live block of the recorded layout and no aliases have been created"
    )
  )]
  fn allocate(
    heap: &mut Heap<'_>,
    layout: Layout,
    contents: u8,
  ) -> Result<Option<Self>, HeapError> {
    let at = match heap.allocate(layout) {
      Ok(at) => at,
      Err(HeapError::Exhausted) => return Ok(None),
      Err(
        error @ (HeapError::Overflow
        | HeapError::TooSmall
        | HeapError::Foreign
        | HeapError::Overlap),
      ) => return Err(error),
    };

    // SAFETY: this allocation is live and disjoint, and the write covers only the requested bytes.
    unsafe { std::slice::from_raw_parts_mut(at.as_ptr(), layout.size()) }.fill(contents);

    Ok(Some(Self {
      at,
      layout,
      contents,
    }))
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "each Live entry holds a block initialized by allocate and still owned by the test"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit096_no_unchecked_memory",
    expect(
      explicit096_no_unchecked_memory,
      reason = "the test owns a live initialized allocation of this Layout for the duration of the check"
    )
  )]
  fn check(&self) -> Result<(), Never> {
    // SAFETY: every Live entry holds an allocation still owned by this test, and reads stop at its requested size.
    let bytes = unsafe { std::slice::from_raw_parts(self.at.as_ptr(), self.layout.size()) };

    assert!(
      bytes.iter().all(|byte| *byte == self.contents),
      "a live allocation was overwritten"
    );
    assert_eq!(self.at.as_ptr().addr().rem_euclid(self.layout.align()), 0);

    Ok(())
  }
}

#[test]
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  allow(
    explicit051_no_machine_width,
    reason = "the workload is counted in u64; only Layout sizes, alignments and slice indices are held at the native width required by those APIs"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "deallocation consumes live test allocations with their original layouts after their last reads"
  )
)]
fn mixed_sizes_alignments_and_free_orders_preserve_every_live_byte() -> Result<(), Box<dyn Error>>
{
  let mut arena = Arena([MaybeUninit::uninit(); 65536]);
  let Ok(mut heap) = Heap::new();

  heap.add(&mut arena.0)?;

  let mut live: [Option<Live>; 64] = std::array::from_fn(|_slot| None);
  let mut random = 0x3482_9031_1245_abed_u64;

  for step in 0_u64..4096 {
    random = random.wrapping_mul(6364136223846793005).wrapping_add(1);

    let slot = usize::try_from(random.rem_euclid(64))?;
    let entry = live.get_mut(slot).ok_or("no allocation slot")?;

    match entry.take() {
      Some(held) => {
        let Ok(()) = held.check();

        // SAFETY: the slot owned this live allocation, its original layout is retained, and check's borrowed slice has ended.
        unsafe { heap.deallocate(held.at, held.layout) }?;
      }
      None => {
        let size = usize::try_from(random.rotate_right(9).rem_euclid(2048).saturating_add(1))?;
        let shift = u32::try_from(random.rotate_right(23).rem_euclid(10))?;
        let alignment = 1_usize.checked_shl(shift).ok_or("alignment overflowed")?;
        let layout = Layout::from_size_align(size, alignment)?;
        let contents = u8::try_from(step.rem_euclid(251))?;
        let allocated = Live::allocate(&mut heap, layout, contents)?;
        *entry = allocated;
      }
    }

    for allocation in live.iter().flatten() {
      let Ok(()) = allocation.check();
    }
  }

  for allocation in live.into_iter().flatten() {
    let Ok(()) = allocation.check();

    // SAFETY: this consumes each remaining live allocation once with its original layout and no outstanding references.
    unsafe { heap.deallocate(allocation.at, allocation.layout) }?;
  }

  let Ok(statistics) = heap.statistics();

  assert_eq!(statistics.free, statistics.capacity);
  assert_eq!(statistics.allocations, 0);
  let entire = Layout::from_size_align(65520, 16)?;

  let _whole_region = heap.allocate(entire)?;

  Ok(())
}

#[test]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the test writes two distinct live allocations, frees only the first, and reads the second within its original layout"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "the test holds distinct initialized regions of the requested Layout while writing and inspecting them"
  )
)]
fn a_freed_allocation_is_available_again() -> Result<(), Box<dyn Error>> {
  let mut arena = Arena([MaybeUninit::uninit(); 4096]);
  let Ok(mut heap) = Heap::new();

  heap.add(&mut arena.0)?;

  let layout = Layout::from_size_align(512, 16)?;
  let first = heap.allocate(layout)?;
  let second = heap.allocate(layout)?;

  // SAFETY: both pointers name distinct live allocations of this layout; the slices are dropped before either allocation is freed.
  unsafe {
    std::slice::from_raw_parts_mut(first.as_ptr(), layout.size()).fill(0x31);
    std::slice::from_raw_parts_mut(second.as_ptr(), layout.size()).fill(0x72);
  }

  // SAFETY: first is live in this heap with the same layout and no outstanding references.
  unsafe { heap.deallocate(first, layout) }?;

  let reused = heap.allocate(layout)?;

  assert_eq!(reused, first);

  // SAFETY: second remains allocated and untouched, and the read stays inside its requested size.
  let bytes = unsafe { std::slice::from_raw_parts(second.as_ptr(), layout.size()) };
  assert!(bytes.iter().all(|byte| *byte == 0x72));

  Ok(())
}

#[test]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "each pointer is an independently allocated block freed exactly once with its original layout"
  )
)]
fn freeing_the_middle_joins_both_neighbors() -> Result<(), Box<dyn Error>> {
  let mut arena = Arena([MaybeUninit::uninit(); 400]);
  let Ok(mut heap) = Heap::new();

  heap.add(&mut arena.0)?;

  let layout = Layout::from_size_align(128, 16)?;
  let first = heap.allocate(layout)?;
  let middle = heap.allocate(layout)?;
  let last = heap.allocate(layout)?;

  assert_eq!(heap.allocate(layout), Err(HeapError::Exhausted));

  for at in [first, last, middle] {
    // SAFETY: these are distinct live allocations from this heap with their original layout and no outstanding references.
    unsafe { heap.deallocate(at, layout) }?;
  }

  let combined = Layout::from_size_align(384, 16)?;

  let joined = heap.allocate(combined)?;

  assert_eq!(joined, first);

  Ok(())
}

#[test]
fn alignment_keeps_the_prefix_available() -> Result<(), Box<dyn Error>> {
  let mut arena = Arena([MaybeUninit::uninit(); 8192]);
  let Ok(mut heap) = Heap::new();

  heap.add(&mut arena.0)?;

  let Ok(before) = heap.statistics();
  let page_aligned = Layout::from_size_align(1, 4096)?;
  let tiny = Layout::from_size_align(16, 16)?;
  let aligned = heap.allocate(page_aligned)?;
  let small = heap.allocate(tiny)?;
  let Ok(after) = heap.statistics();

  assert_eq!(aligned.as_ptr().addr().rem_euclid(4096), 0);
  assert!(small.as_ptr().addr() < aligned.as_ptr().addr());
  assert_eq!(before.free.0.checked_sub(after.free.0), Some(32));

  Ok(())
}

#[test]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "each tested free order is a permutation of the three live allocations of the same layout"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  allow(
    explicit051_no_machine_width,
    reason = "the free-order index becomes the native width only when indexing the test's fixed array through get"
  )
)]
fn all_free_orders_restore_the_whole_region() -> Result<(), Box<dyn Error>> {
  for order in [
    [0_u8, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
  ] {
    let mut arena = Arena([MaybeUninit::uninit(); 400]);
    let Ok(mut heap) = Heap::new();

    heap.add(&mut arena.0)?;

    let layout = Layout::from_size_align(128, 16)?;
    let first = heap.allocate(layout)?;
    let middle = heap.allocate(layout)?;
    let last = heap.allocate(layout)?;
    let pointers = [first, middle, last];

    for which in order {
      let pointer = pointers
        .get(usize::from(which))
        .ok_or("the free order names no allocation")?;
      let at = *pointer;

      // SAFETY: each permutation frees every allocation exactly once with its original layout.
      unsafe { heap.deallocate(at, layout) }?;
    }

    let Ok(statistics) = heap.statistics();

    assert_eq!(statistics.free, statistics.capacity);
    assert_eq!(statistics.allocations, 0);
    let whole = Layout::from_size_align(384, 16)?;
    let joined = heap.allocate(whole)?;

    assert_eq!(joined, first);
  }

  Ok(())
}

#[test]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the test frees only the distinct outer allocations of three adjacent blocks using the layout that created them"
  )
)]
fn fragmented_space_does_not_answer_a_contiguous_request() -> Result<(), Box<dyn Error>> {
  let mut arena = Arena([MaybeUninit::uninit(); 400]);
  let Ok(mut heap) = Heap::new();

  heap.add(&mut arena.0)?;

  let layout = Layout::from_size_align(128, 16)?;
  let first = heap.allocate(layout)?;
  let _middle = heap.allocate(layout)?;
  let last = heap.allocate(layout)?;

  for at in [first, last] {
    // SAFETY: both allocations are live and distinct, have their original layout, and have no outstanding references.
    unsafe { heap.deallocate(at, layout) }?;
  }

  let Ok(before) = heap.statistics();

  let bigger = Layout::from_size_align(256, 16)?;

  assert_eq!(heap.allocate(bigger), Err(HeapError::Exhausted));
  assert_eq!(heap.statistics(), Ok(before));

  let reused_first = heap.allocate(layout)?;
  let reused_last = heap.allocate(layout)?;

  assert_eq!(reused_first, first);
  assert_eq!(reused_last, last);

  Ok(())
}

#[test]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "each pointer belongs to a distinct live block in one of the two regions and is freed with its original layout"
  )
)]
fn separately_lent_regions_keep_their_boundaries() -> Result<(), Box<dyn Error>> {
  let mut arena = Arena([MaybeUninit::uninit(); 1024]);
  let (left, right) = arena.0.split_at_mut(512);
  let Ok(mut heap) = Heap::new();

  heap.add(right)?;
  heap.add(left)?;

  let layout = Layout::from_size_align(496, 16)?;
  let first = heap.allocate(layout)?;
  let second = heap.allocate(layout)?;

  assert!(first.as_ptr().addr() < second.as_ptr().addr());

  for at in [first, second] {
    // SAFETY: each pointer is a live allocation of its original layout, with no outstanding references.
    unsafe { heap.deallocate(at, layout) }?;
  }

  let crossing = Layout::from_size_align(512, 16)?;

  assert_eq!(heap.allocate(crossing), Err(HeapError::Exhausted));

  let reused_first = heap.allocate(layout)?;
  let reused_second = heap.allocate(layout)?;

  assert_eq!(reused_first, first);
  assert_eq!(reused_second, second);

  Ok(())
}

#[test]
fn a_region_too_small_for_bookkeeping_is_refused_without_changing_the_heap()
-> Result<(), Box<dyn Error>> {
  let mut tiny = Arena([MaybeUninit::uninit(); 31]);
  let mut arena = Arena([MaybeUninit::uninit(); 4096]);
  let Ok(mut heap) = Heap::new();

  assert_eq!(heap.add(&mut tiny.0), Err(HeapError::TooSmall));

  heap.add(&mut arena.0)?;

  let Ok(statistics) = heap.statistics();

  assert_eq!(statistics.capacity, Bytes(4080));
  let entire = Layout::from_size_align(4080, 16)?;

  let _whole_region = heap.allocate(entire)?;

  Ok(())
}

#[test]
fn foreign_memory_is_distinguished_from_owned_allocations() -> Result<(), Box<dyn Error>> {
  let mut arena = Arena([MaybeUninit::uninit(); 4096]);
  let mut foreign = 7_u8;
  let Ok(mut heap) = Heap::new();

  heap.add(&mut arena.0)?;

  let tiny = Layout::from_size_align(1, 1)?;
  let at = heap.allocate(tiny)?;

  assert_eq!(heap.owns(at), Ok(Ownership::Here));
  assert_eq!(
    heap.owns(NonNull::from(&mut foreign)),
    Ok(Ownership::Elsewhere)
  );

  Ok(())
}

#[test]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the test writes only the live allocation and inspects initialized edge bytes after the heap releases the borrow"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit059_no_unchecked_api",
  allow(
    explicit059_no_unchecked_api,
    reason = "every byte begins initialized to 0x5a; assume_init reads only the prefix and suffix that were never lent to the heap"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "the allocator returned an exclusive allocation for the requested layout and the test writes that exact extent"
  )
)]
fn an_unaligned_region_lends_only_its_aligned_interior() -> Result<(), Box<dyn Error>> {
  let mut arena = Arena([MaybeUninit::new(0x5a); 4096]);

  {
    let memory = arena.0.get_mut(1..4095).ok_or("no interior")?;
    let Ok(mut heap) = Heap::new();

    heap.add(memory)?;

    let entire = Layout::from_size_align(4048, 16)?;
    let at = heap.allocate(entire)?;

    // SAFETY: the allocation is live and the write covers exactly its requested bytes.
    unsafe { std::slice::from_raw_parts_mut(at.as_ptr(), 4048) }.fill(0x99);
  }

  let prefix = arena.0.get(..16).ok_or("no prefix")?;
  let suffix = arena.0.get(4080..).ok_or("no suffix")?;

  for byte in prefix.iter().chain(suffix) {
    // SAFETY: the entire buffer began initialized, and the allocator never owns the prefix or suffix being inspected.
    assert_eq!(unsafe { byte.assume_init() }, 0x5a);
  }

  Ok(())
}
