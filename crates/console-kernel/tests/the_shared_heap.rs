//! Allocations live outside the lock and can be freed by a different thread.

use std::alloc::Layout;
use std::error::Error;
use std::mem::MaybeUninit;
use std::sync::Barrier;
use std::thread;

use console_kernel::locking::SpinLock;
use console_kernel_heap::Heap;

#[repr(align(4096))]
struct Arena([MaybeUninit<u8>; 65536]);

#[test]
fn only_one_borrower_can_hold_the_heap() -> Result<(), Box<dyn Error>> {
  let Ok(lock) = SpinLock::new(0_u64);
  let Ok(mut first) = lock.lock();

  *first = 9;

  assert!(matches!(lock.try_lock(), Ok(None)));

  drop(first);

  let Ok(second) = lock.lock();

  assert_eq!(*second, 9);

  Ok(())
}

#[test]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "each test worker exclusively owns its live allocation and only reads or writes its original layout before freeing it"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit103_no_thread_per_item",
  expect(
    explicit103_no_thread_per_item,
    reason = "the barrier intentionally synchronizes exactly eight workers to check simultaneous ownership of distinct heap blocks"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "each scoped worker owns its live allocation and returns it under the heap lock only after its local references end"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the scoped worker is joined and a worker panic is reported to the test as an error"
  )
)]
fn threads_keep_distinct_live_allocations_and_recover_all_the_memory()
-> Result<(), Box<dyn Error>> {
  let mut arena = Arena([MaybeUninit::uninit(); 65536]);
  let Ok(mut heap) = Heap::new();

  heap.add(&mut arena.0)?;

  let Ok(lock) = SpinLock::new(heap);
  let barrier = Barrier::new(8);

  thread::scope(|scope| -> Result<(), Box<dyn Error>> {
    let mut threads = Vec::new();

    for worker in 0_u8..8 {
      let lock_2 = &lock;
      let barrier_2 = &barrier;

      threads.push(
        scope.spawn(move || -> Result<(), console_kernel_heap::HeapError> {
          let layout = match Layout::from_size_align(257, 64) {
            Ok(layout) => layout,
            Err(_invalid_constant) => return Err(console_kernel_heap::HeapError::Overflow),
          };

          for _ in 0..256 {
            let at = {
              let Ok(mut held) = lock_2.lock();

              held.allocate(layout)?
            };

            // SAFETY: this thread owns this live allocation exclusively; the allocator's lock protects metadata while the allocation itself remains disjoint from every other live block.
            unsafe { std::slice::from_raw_parts_mut(at.as_ptr(), layout.size()) }.fill(worker);

            barrier_2.wait();

            // SAFETY: the allocation is still live and exclusively owned by this thread, and the read covers only its requested bytes.
            let bytes = unsafe { std::slice::from_raw_parts(at.as_ptr(), layout.size()) };
            assert!(bytes.iter().all(|byte| *byte == worker));

            let Ok(mut held) = lock_2.lock();

            // SAFETY: this thread returns its still-live allocation with its original layout and has no outstanding references into it.
            unsafe { held.deallocate(at, layout) }?;
          }

          Ok(())
        }),
      );
    }

    for thread in threads {
      let outcome = thread
        .join()
        .map_err(|_panicked| "an allocation worker panicked")?;

      outcome?;
    }

    Ok(())
  })?;

  let Ok(mut held) = lock.lock();
  let Ok(statistics) = held.statistics();

  assert_eq!(statistics.free, statistics.capacity);
  assert_eq!(statistics.allocations, 0);
  let entire = Layout::from_size_align(65520, 16)?;

  let _whole_region = held.allocate(entire)?;

  Ok(())
}

#[test]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "the test owns the allocation exclusively and moves the single slice borrow into the scoped worker before freeing it"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "ownership of the exclusive initialized allocation moves into the scoped worker and is returned with the original layout"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the scoped worker is joined and a worker panic is reported to the test as an error"
  )
)]
fn an_allocation_can_be_returned_by_a_different_thread() -> Result<(), Box<dyn Error>> {
  let mut arena = Arena([MaybeUninit::uninit(); 65536]);
  let Ok(mut heap) = Heap::new();

  heap.add(&mut arena.0)?;

  let Ok(lock) = SpinLock::new(heap);
  let layout = Layout::from_size_align(512, 32)?;
  let at = {
    let Ok(mut held) = lock.lock();

    held.allocate(layout)?
  };

  // SAFETY: the live allocation is exclusively borrowed for its requested size and the heap outlives the scoped worker.
  let contents = unsafe { std::slice::from_raw_parts_mut(at.as_ptr(), layout.size()) };

  contents.fill(0x4d);

  thread::scope(|scope| -> Result<(), Box<dyn Error>> {
    let lock_2 = &lock;
    let worker = scope.spawn(move || -> Result<(), console_kernel_heap::HeapError> {
      assert!(contents.iter().all(|byte| *byte == 0x4d));

      let at_2 = std::ptr::NonNull::from(contents).cast::<u8>();
      let Ok(mut held) = lock_2.lock();

      // SAFETY: allocation ownership moved to this thread, contents is consumed before freeing it, and the shared heap is exclusively locked.
      unsafe { held.deallocate(at_2, layout) }?;

      let Ok(statistics) = held.statistics();

      assert_eq!(statistics.free, statistics.capacity);
      assert_eq!(statistics.allocations, 0);

      Ok(())
    });

    let outcome = worker
      .join()
      .map_err(|_panicked| "the freeing thread panicked")?;

    outcome?;

    Ok(())
  })?;

  let Ok(mut held) = lock.lock();

  let reused = held.allocate(layout)?;

  assert_eq!(reused, at);

  Ok(())
}
