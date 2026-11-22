//! Where the firmware hands over, and where the kernel hands back.

use alloc::vec::Vec;
use core::arch::x86_64::__cpuid;
use core::cell::Cell;
use core::ffi::c_void;
use core::fmt::{self, Write};
use core::panic::PanicInfo;

use console_core_never::Never;
use console_kernel::interrupts::Address;
use console_kernel::memory::{Frames, Heap, MEBIBYTE, MemoryMap};
use console_kernel::screen::Presentation;
use console_kernel::{Bytes, Exit};

use crate::acpi;
use crate::allocator;
use crate::display::{self, Opened};
use crate::exceptions::{self, Breakpoint};
use crate::firmware::{self, Status, SystemTable};
use crate::input;
use crate::serial::{self, Serial};
use crate::timer;
use crate::userspace::{self, Reserved};
use crate::xhci::Machine;

const EXIT_PORT: u16 = 0xf4;

const HYPERVISOR_PRESENT: u32 = 1 << 31;

const TICKS: u64 = 3;

const SUCCESS: u32 = 0;

pub const FAILURE: u32 = 1;

#[unsafe(no_mangle)]
#[cfg_attr(
  dylint_lib = "explicit066_no_heap_after_init",
  expect(
    explicit066_no_heap_after_init,
    reason = "the boot test allocates a firmware-owned block to verify that handover keeps it intact"
  )
)]
pub extern "efiapi" fn efi_main(image: *const c_void, system: *const SystemTable) -> Status {
  let Ok(serial) = serial::open();
  let Ok(()) = firmware::borrow(system);
  let Ok(graphics) = firmware::graphics(system);
  let Ok(opened) = display::open(graphics);
  let Ok(()) = say(&serial, format_args!("console-kernel: booted"));

  let Ok(()) = match opened {
    Opened::Drawn(panel) => say(
      &serial,
      format_args!(
        "console-kernel: the panel is {} by {}, and boot progress is drawn on it",
        panel.width.0, panel.height.0
      ),
    ),
    Opened::NothingLent => say(
      &serial,
      format_args!(
        "console-kernel: the firmware lent no panel, and the lines are on the serial port alone"
      ),
    ),
    Opened::Refused(error) => say(
      &serial,
      format_args!("console-kernel: the panel would not be drawn on: {error}"),
    ),
  };

  let before_handover = alloc::vec![0x5a_u8; 64];
  let Ok(acpi_root) = firmware::acpi_root(system);

  match firmware::exit_boot_services(image) {
    Ok(map) => own(&serial, map, before_handover, acpi_root),
    Err(error) => stop(format_args!("boot services handoff failed: {error}")),
  }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "boot disables firmware interrupt delivery and transfers disjoint identity-mapped pages to the stack and heap owners"
  )
)]
fn own(
  serial: &Serial,
  map: MemoryMap<'static>,
  before_handover: Vec<u8>,
  acpi_root: Option<acpi::Root>,
) -> ! {
  // SAFETY: nothing the firmware installed may interrupt the kernel once boot services are gone, and `cli` touches no memory.
  unsafe { core::arch::asm!("cli", options(nomem, nostack)) };

  let usable = map.usable_bytes();
  let Ok(mut frames) = Frames::new(map);
  // SAFETY: boot services have exited and the kernel owns the identity-mapped conventional frames. The reservation is removed from the cursor before the heap receives it.
  let reserved = match unsafe { Reserved::take(&mut frames) } {
    Ok(reserved) => reserved,
    Err(error) => stop(format_args!("could not reserve user pages: {error}")),
  };
  let trap_stack = match reserved.trap_stack() {
    Ok(stack) => stack,
    Err(error) => stop(format_args!(
      "could not reserve the user trap stack: {error}"
    )),
  };
  // SAFETY: boot services have exited, conventional pages are identity-mapped writable RAM, and the frame cursor is transferred to the kernel's only physical heap.
  let Ok(heap) = unsafe { Heap::new(frames) };
  let Ok(()) = allocator::take_over(heap);
  let Ok(()) = own_exceptions(serial, trap_stack);

  match ::core::primitive::bool::then_some(before_handover.iter().all(|byte| *byte == 0x5a), ()) {
    ::core::option::Option::Some(()) => drop(before_handover),
    ::core::option::Option::None => exit(FAILURE),
  }

  let Ok(()) = heap_check(serial);

  let owned = match usable {
    Ok(bytes) => {
      let Ok(()) = say(
        serial,
        format_args!(
          "console-kernel: boot services exited, the kernel owns {} MiB",
          bytes.div_euclid(MEBIBYTE)
        ),
      );

      bytes
    }
    Err(error) => {
      let Ok(()) = say(
        serial,
        format_args!("console-kernel: the memory map does not add up: {error}"),
      );

      exit(FAILURE)
    }
  };

  let Ok(()) = count_ticks(serial);
  let pixels = match userspace::check(reserved, trap_stack) {
    Ok(pixels) => pixels,
    Err(error) => stop(format_args!("user tasks failed: {error}")),
  };
  let Ok(()) = say(
    serial,
    format_args!(
      "console-kernel: the ring 3 renderer wrote a frame, the ring 3 browser read it, and both exited through syscalls"
    ),
  );
  let Ok(()) = say(
    serial,
    format_args!(
      "console-kernel: user mode could neither read the supervisor alias nor write the browser's frame"
    ),
  );
  let Ok(()) = say(
    serial,
    format_args!(
      "console-kernel: the CPU refused a user code write and a user stack instruction fetch"
    ),
  );
  let Ok(()) = say(
    serial,
    format_args!(
      "console-kernel: both browser aliases faulted after revocation and the owner could still read"
    ),
  );
  let Ok(()) = say(
    serial,
    format_args!(
      "console-kernel: closed and exited handles lost both aliases, and the next owner read a zeroed frame"
    ),
  );

  run(
    serial,
    Bytes(owned),
    pixels.as_chunks::<{ userspace::FRAME_WIDTH }>().0,
    acpi_root,
  )
}

fn say(_serial: &Serial, line: fmt::Arguments<'_>) -> Result<(), Never> {
  let Ok(()) = display::print(line);
  let _written = write!(Serial, "{line}\r\n");

  Ok(())
}

fn stop(why: core::fmt::Arguments<'_>) -> ! {
  let Ok(()) = display::write_without_waiting(format_args!("console-kernel: {why}\n"));
  let _written = write!(Serial, "console-kernel: {why}\r\n");

  exit(FAILURE)
}

fn own_exceptions(serial: &Serial, trap_stack: Address) -> Result<(), Never> {
  match exceptions::install(trap_stack) {
    Ok(()) => {}
    Err(error) => {
      let Ok(()) = say(
        serial,
        format_args!("console-kernel: the kernel's own tables would not load: {error}"),
      );

      exit(FAILURE)
    }
  }

  let Ok(()) = say(
    serial,
    format_args!(
      "console-kernel: the kernel's own segments, task state and interrupt gates are loaded"
    ),
  );
  let Ok(taken) = exceptions::breakpoint();

  match taken {
    Breakpoint::TakenOnce => say(
      serial,
      format_args!("console-kernel: a breakpoint was taken and returned from"),
    ),
    Breakpoint::Miscounted => {
      let Ok(()) = say(
        serial,
        format_args!(
          "console-kernel: a breakpoint returned without its handler having counted it once"
        ),
      );

      exit(FAILURE)
    }
  }
}

fn count_ticks(serial: &Serial) -> Result<(), Never> {
  match timer::count(TICKS) {
    Ok(counted) => say(
      serial,
      format_args!(
        "console-kernel: the {} timer interrupted {} times, and is masked again",
        counted.mode, counted.ticks
      ),
    ),
    Err(error) => {
      let Ok(()) = say(
        serial,
        format_args!("console-kernel: the timer would not count: {error}"),
      );

      exit(FAILURE)
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit066_no_heap_after_init",
  expect(
    explicit066_no_heap_after_init,
    reason = "the boot test allocates and frees initialized blocks to verify reuse after ExitBootServices"
  )
)]
fn heap_check(serial: &Serial) -> Result<(), Never> {
  let first = core::hint::black_box(alloc::vec![0xa5_u8; 1_048_576]);
  let address = u64::try_from(first.as_ptr().expose_provenance());

  drop(first);

  for _ in 0..16 {
    let again = core::hint::black_box(alloc::vec![0x3c_u8; 1_048_576]);

    match ::core::primitive::bool::then_some(u64::try_from(again.as_ptr().expose_provenance()) == address
      && again.iter().all(|byte| *byte == 0x3c), ())
    {
      ::core::option::Option::Some(()) => drop(again),
      ::core::option::Option::None => {
        let Ok(()) = say(
          serial,
          format_args!("console-kernel: freed heap memory was not reused"),
        );

        exit(FAILURE)
      }
    }
  }

  say(
    serial,
    format_args!("console-kernel: heap memory freed and reused after firmware handover"),
  )
}

fn run(
  serial: &Serial,
  memory: Bytes,
  frame: &[[u32; userspace::FRAME_WIDTH]],
  acpi_root: Option<acpi::Root>,
) -> ! {
  let page_shown = Cell::new(Presentation::Skipped);
  let Ok(outcome) = console_kernel::run_with_frame(
    memory,
    |line| {
      let _sent_to_serial = write!(Serial, "console-kernel: {line}\r\n");
    },
    |size, pixels| {
      let Ok(shown) = display::draw_monochrome_frame(size, pixels);
      page_shown.set(shown);
    },
  );

  let answer = match outcome {
    Exit::Success => {
      let Ok(()) = say(serial, format_args!("console-kernel: every step held"));

      let Ok(()) = report_presentation(serial, page_shown.get(), frame);
      let Ok(machine) = machine();

      match input::run(acpi_root, machine) {
        Ok(()) => {}
        Err(error) => stop(format_args!("USB keyboard stopped: {error}")),
      }

      SUCCESS
    }
    Exit::Failure(error) => {
      let Ok(()) = say(serial, format_args!("console-kernel: stopped: {error}"));

      FAILURE
    }
  };

  exit(answer)
}

fn report_presentation(
  serial: &Serial,
  shown: Presentation,
  frame: &[[u32; userspace::FRAME_WIDTH]],
) -> Result<(), Never> {
  match shown {
    Presentation::Presented => serial.print("console-kernel: the browser page is on the panel"),
    Presentation::Skipped => {
      let Ok(fallback) = display::draw_frame(frame);

      match fallback {
        Presentation::Presented => {
          serial.print("console-kernel: the shared test frame is on the panel")
        }
        Presentation::Skipped => Ok(()),
      }
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "QEMU's debug exit device is reached through a port write and only in a guest"
  )
)]
pub fn exit(answer: u32) -> ! {
  let Ok(platform) = machine();

  match platform {
    Machine::Virtual => {
      // SAFETY: the QEMU test guest connects this port to its debug exit device.
      unsafe { core::arch::asm!("out dx, eax", in("dx") EXIT_PORT, in("eax") answer) };
    }
    Machine::Physical => {}
  }

  halt()
}

fn machine() -> Result<Machine, Never> {
  Ok(match __cpuid(1).ecx & HYPERVISOR_PRESENT {
    0 => Machine::Physical,
    _ => Machine::Virtual,
  })
}

#[cfg_attr(
  dylint_lib = "explicit054_no_loop",
  allow(
    explicit054_no_loop,
    reason = "a machine that has halted has no next step to name: this returns `!`, and `hlt` wakes on every interrupt, so the only answer is to halt again"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the terminal halt loop issues the privileged CPU halt instruction"
  )
)]
fn halt() -> ! {
  loop {
    // SAFETY: `hlt` waits for an interrupt and touches no memory.
    unsafe { core::arch::asm!("hlt") };
  }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
  let mut serial = Serial;

  let Ok(()) = display::write_without_waiting(format_args!("console-kernel: panicked: {info}\n"));
  let Ok(()) = match write!(serial, "console-kernel: panicked: {info}\r\n") {
    Ok(()) | Err(core::fmt::Error) => Ok::<(), Never>(()),
  };

  exit(FAILURE)
}
