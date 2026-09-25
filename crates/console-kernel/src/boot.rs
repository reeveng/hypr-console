//! Where the firmware hands over, and where the kernel hands back.

use alloc::format;
use core::ffi::c_void;
use core::panic::PanicInfo;

use console_kernel::Exit;
use console_kernel::memory::{Frames, Heap, MEBIBYTE, MemoryMap};

use crate::firmware::{self, Status, SystemTable};
use crate::serial::{self, Serial};
use crate::allocator;

const EXIT_PORT: u16 = 0xf4;

const SUCCESS: u32 = 0;

const FAILURE: u32 = 1;

#[unsafe(no_mangle)]
pub extern "efiapi" fn efi_main(image: *const c_void, system: *const SystemTable) -> Status {
    let Ok(serial) = serial::open();
    let Ok(()) = firmware::borrow(system);
    let Ok(()) = serial.print("console-kernel: booted");

    match firmware::exit_boot_services(image) {
        Ok(map) => own(&serial, map),
        Err(error) => {
            let Ok(()) = serial.print(&format!("console-kernel: boot services would not exit: {error}"));

            exit(FAILURE)
        },
    }
}

fn own(serial: &Serial, map: MemoryMap<'static>) -> ! {
    // SAFETY: nothing the firmware installed may interrupt the kernel once boot services are gone, and `cli` touches no memory.
    unsafe { core::arch::asm!("cli", options(nomem, nostack)) };

    let usable = map.usable_bytes();
    let Ok(frames) = Frames::new(map);
    let Ok(heap) = Heap::new(frames);
    let Ok(()) = allocator::take_over(heap);

    match usable {
        Ok(bytes) => {
            let Ok(()) = serial.print(&format!("console-kernel: boot services exited, the kernel owns {} MiB", bytes.div_euclid(MEBIBYTE)));
        },
        Err(error) => {
            let Ok(()) = serial.print(&format!("console-kernel: the memory map does not add up: {error}"));

            exit(FAILURE)
        },
    }

    run(serial)
}

fn run(serial: &Serial) -> ! {
    let Ok(run) = console_kernel::run();

    for line in &run.log {
        let Ok(()) = serial.print(&format!("console-kernel: {line}"));
    }

    let answer = match run.exit {
        Exit::Success => {
            let Ok(()) = serial.print("console-kernel: every step held");

            SUCCESS
        },
        Exit::Failure(error) => {
            let Ok(()) = serial.print(&format!("console-kernel: stopped: {error}"));

            FAILURE
        },
    };

    exit(answer)
}

fn exit(answer: u32) -> ! {
    // SAFETY: port 0xf4 is QEMU's isa-debug-exit when `just kernel` starts it, and on a machine where nothing listens there the write goes nowhere.
    unsafe { core::arch::asm!("out dx, eax", in("dx") EXIT_PORT, in("eax") answer) };

    halt()
}

fn halt() -> ! {
    loop {
        // SAFETY: `hlt` waits for an interrupt and touches no memory.
        unsafe { core::arch::asm!("hlt") };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit(FAILURE)
}
