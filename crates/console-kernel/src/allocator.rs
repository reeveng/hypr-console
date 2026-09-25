//! The one global allocator, which starts as the firmware's and becomes the kernel's.
//!
//! `alloc` has a single allocator for the whole program and cannot be handed a
//! second one halfway through, so the change is a phase the allocator is in:
//! the firmware's pool until boot services are exited, and the kernel's heap
//! over its own frames from then on. The change only goes one way. A block the
//! pool handed out before it is simply never given back, because the pool is
//! gone and the heap does not free, and a block the heap handed out can never
//! reach the pool, because the pool is never asked again.

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr;

use console_core_never::Never;
use console_kernel::memory::Heap;

use crate::firmware;

enum Phase {
    Firmware,
    Kernel(Heap<'static>),
}

struct Current(UnsafeCell<Phase>);

// SAFETY: the kernel runs on one core with interrupts off, and the allocator is never entered from inside itself, so the phase is never reached from two places at once.
unsafe impl Sync for Current {}

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the global allocator is the language's, one per program and called by `alloc` with a layout and nothing else, so the phase it allocates in can only be where the whole program can reach it; it is changed once, when boot services are exited"
    )
)]
static CURRENT: Current = Current(UnsafeCell::new(Phase::Firmware));

pub fn take_over(heap: Heap<'static>) -> Result<(), Never> {
    // SAFETY: one core, interrupts off, and no allocation is in flight while `efi_main` hands the heap over.
    unsafe { *CURRENT.0.get() = Phase::Kernel(heap) };

    Ok(())
}

struct Kernel;

// SAFETY: a block comes either from the firmware's pool at an alignment the pool guarantees or from the heap at the alignment asked for, and is never handed to the other.
unsafe impl GlobalAlloc for Kernel {
    #[cfg_attr(
        dylint_lib = "explicit051_no_machine_width",
        allow(explicit051_no_machine_width, reason = "a pointer is an address at the machine's width, and the heap's address is met there and nowhere else")
    )]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: one core, interrupts off, and `alloc` is not entered from inside itself.
        let phase = unsafe { &mut *CURRENT.0.get() };

        match phase {
            Phase::Firmware => {
                let Ok(at) = firmware::pool_allocate(layout);

                at
            },
            Phase::Kernel(heap) => match heap.allocate(layout).map(usize::try_from) {
                Ok(Ok(at)) => ptr::with_exposed_provenance_mut(at),
                Ok(Err(_too_wide)) => ptr::null_mut(),
                Err(_out_of_memory) => ptr::null_mut(),
            },
        }
    }

    unsafe fn dealloc(&self, at: *mut u8, _layout: Layout) {
        // SAFETY: one core, interrupts off, and `dealloc` is not entered from inside itself.
        let phase = unsafe { &*CURRENT.0.get() };

        match phase {
            Phase::Firmware => {
                let Ok(()) = firmware::pool_free(at);
            },
            Phase::Kernel(_heap) => {},
        }
    }
}

#[global_allocator]
static KERNEL: Kernel = Kernel;
