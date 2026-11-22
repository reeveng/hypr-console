//! The global allocator, transferred from firmware to an owned kernel heap.
//!
//! Every entry holds the same lock, including the one-way handover, so neither
//! the free list nor the frame cursor assumes a single core. Local interrupts
//! are disabled before taking it and their former state is restored after it
//! is released. NMI and fault handlers must not allocate. The lock itself and
//! its guards allocate nothing.
//!
//! Blocks the firmware allocated before handover stay reserved: boot services
//! are gone, and those blocks are not part of any region the kernel heap was
//! given. Only the kernel's own allocations can return to its free list.

use core::alloc::{GlobalAlloc, Layout};
use core::marker::PhantomData;
use core::ptr;

use console_core_never::Never;
use console_kernel::locking::SpinLock;
use console_kernel::memory::Heap;

use crate::firmware;

enum Phase {
  Firmware,
  Kernel(Heap<'static>),
}

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  allow(
    explicit044_no_ambient_value,
    reason = "GlobalAlloc is one per program and is called without a state argument; the lock serializes that required global state and the one-way firmware handover"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "the Rust global allocator receives only layout and pointer; its ownership and phase are serialized by the kernel lock"
  )
)]
static CURRENT: SpinLock<Phase> = match SpinLock::new(Phase::Firmware) {
  Ok(lock) => lock,
};

enum Restore {
  Enabled,
  Disabled,
}

struct Interrupts {
  restore: Restore,
  local: PhantomData<*mut ()>,
}

impl Interrupts {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the allocator snapshots and masks local CPU interrupts before taking its lock"
    )
  )]
  fn disable() -> Result<Self, Never> {
    let flags: u64;

    // SAFETY: the allocator runs in kernel privilege; saving flags and disabling interrupts precedes the lock, and the assembly's memory clobber keeps heap operations inside the critical section.
    unsafe { core::arch::asm!("pushfq", "pop {}", "cli", out(reg) flags) };

    let restore = match ::core::primitive::bool::then_some(flags & 0x200 == 0, ()) {
      ::core::option::Option::Some(()) => Restore::Disabled,
      ::core::option::Option::None => Restore::Enabled,
    };

    Ok(Interrupts {
      restore,
      local: PhantomData,
    })
  }
}

impl Drop for Interrupts {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "dropping the CPU-local guard restores the saved interrupt flag after the allocator unlocks"
    )
  )]
  fn drop(&mut self) {
    match self.restore {
      Restore::Enabled => {
        // SAFETY: this guard remains on the core where it disabled interrupts, and every heap guard is dropped before the prior enabled state is restored.
        unsafe { core::arch::asm!("sti", options(nostack)) };
      }
      Restore::Disabled => {}
    }
  }
}

pub fn take_over(heap: Heap<'static>) -> Result<(), Never> {
  let Ok(_interrupts) = Interrupts::disable();
  let Ok(mut phase) = CURRENT.lock();

  *phase = Phase::Kernel(heap);

  Ok(())
}

struct Kernel;

// SAFETY: every access is serialized with interrupts disabled, only the owning allocator frees each block, and kernel allocation honors the caller's size and alignment.
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the Rust GlobalAlloc contract is implemented at the single firmware-to-kernel allocator boundary"
  )
)]
unsafe impl GlobalAlloc for Kernel {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    let Ok(_interrupts) = Interrupts::disable();
    let Ok(mut phase) = CURRENT.lock();

    match &mut *phase {
      Phase::Firmware => {
        let Ok(at) = firmware::pool_allocate(layout);

        at
      }
      Phase::Kernel(heap) => match heap.allocate(layout) {
        Ok(at) => at.as_ptr(),
        Err(_out_of_memory) => ptr::null_mut(),
      },
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the GlobalAlloc caller provides the pointer and original layout required by the selected allocator"
    )
  )]
  unsafe fn dealloc(&self, at: *mut u8, layout: Layout) {
    let Ok(_interrupts) = Interrupts::disable();
    let Ok(mut phase) = CURRENT.lock();

    match &mut *phase {
      Phase::Firmware => {
        let Ok(()) = firmware::pool_free(at);
      }
      Phase::Kernel(heap) => match ptr::NonNull::new(at) {
        Some(at_2) => {
          // SAFETY: GlobalAlloc's caller supplies a live allocation and its original layout; the physical heap retains firmware allocations and frees only its own.
          let freed = unsafe { heap.deallocate(at_2, layout) };

          match freed {
            Ok(()) => {}
            Err(_corrupt_heap) => stop_corrupt_heap(),
          }
        }
        None => {}
      },
    }
  }
}

#[global_allocator]
static KERNEL: Kernel = Kernel;

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "UD2 reports heap corruption without a panic allocation or reacquiring the held heap lock"
  )
)]
fn stop_corrupt_heap() -> ! {
  // SAFETY: the installed invalid-opcode trap terminates boot without accessing the allocator.
  unsafe { core::arch::asm!("ud2", options(noreturn, nostack)) }
}
