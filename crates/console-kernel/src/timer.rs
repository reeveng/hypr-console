//! A timer that interrupts, counted for a moment and then put away.
//!
//! The timer is the local APIC's, not the PIT behind the legacy 8259. Every
//! machine this kernel can boot on has one per core, it needs no line routed
//! through a second controller to reach the processor, and OVMF leaves it
//! enabled and mapped where its base register says, so there was nothing to
//! work around. The 8259 pair is masked whole before interrupts are let in,
//! because its lines are numbered from wherever the firmware left them and a
//! stray one would arrive at a vector that means something else.
//!
//! The bootstrap processor uses x2APIC when available and otherwise keeps
//! the firmware's memory-mapped xAPIC. Both modes use the same timer registers.
//!
//! The period is a count of the controller's own clock and not a length of
//! time. Nothing has measured that clock yet, so this proves interrupts
//! arrive and are returned from, which is what a scheduler will stand on,
//! and says nothing of how long a tick is. The boot check polls with a TSC
//! limit so it can report a timer that delivers no interrupts.

use core::arch::asm;
use core::arch::x86_64::{__cpuid, _rdtsc};
use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

use console_core_never::Never;
use console_kernel::interrupts::LocalController;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerError {
  Disabled,
  Stalled(u64),
}

pub struct Counted {
  pub ticks: u64,
  pub mode: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Register(u32);

const BASE_REGISTER: u32 = 0x1b;

const X2APIC_ENABLE: u32 = 1 << 10;

const X2APIC_FEATURE: u32 = 1 << 21;

const X2APIC_MSR_BASE: u32 = 0x800;

const TASK_PRIORITY: Register = Register(0x80);

const END_OF_INTERRUPT: Register = Register(0xb0);

const SPURIOUS_VECTOR: Register = Register(0xf0);

const TIMER_ENTRY: Register = Register(0x320);

const INITIAL_COUNT: Register = Register(0x380);

const DIVIDE: Register = Register(0x3e0);

const ACKNOWLEDGED: u32 = 0;

const ENABLED_WITH_SPURIOUS_VECTOR_255: u32 = 0x1ff;

const PERIODIC_ON_VECTOR_32: u32 = 0x2_0020;

const MASKED: u32 = 0x1_0000;

const BY_SIXTEEN: u32 = 0x3;

const PERIOD: u32 = 100_000;

const STOPPED: u32 = 0;

const FIRST_LEGACY_MASK: u16 = 0x21;

const SECOND_LEGACY_MASK: u16 = 0xa1;

const EVERY_LINE: u8 = 0xff;

const TIMEOUT_CYCLES: u64 = 4_000_000_000;

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  allow(
    explicit044_no_ambient_value,
    reason = "an interrupt handler is entered by the processor, which hands it a stack frame and nothing else, so what it counts can only be where both it and the code it interrupted can reach"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  allow(
    explicit068_no_global_mutable_state,
    reason = "an interrupt handler is entered by the processor, which hands it a stack frame and nothing else, so what it counts can only be where both it and the code it interrupted can reach"
  )
)]
static TICKS: AtomicU64 = AtomicU64::new(0);

pub fn tick() -> Result<(), Never> {
  let _before = TICKS.fetch_add(1, Ordering::SeqCst);
  let Ok(base) = base_register();
  let Ok(controller) = LocalController::read(base);

  write(controller, END_OF_INTERRUPT, ACKNOWLEDGED)
}

pub fn count(wanted: u64) -> Result<Counted, TimerError> {
  let Ok(controller) = enable_controller();
  let mode = match controller {
    LocalController::MemoryMapped(_) => "xAPIC",
    LocalController::RegisterMapped => "x2APIC",
    LocalController::Disabled => return Err(TimerError::Disabled),
  };

  let before = TICKS.load(Ordering::SeqCst);
  let Ok(()) = out(FIRST_LEGACY_MASK, EVERY_LINE);
  let Ok(()) = out(SECOND_LEGACY_MASK, EVERY_LINE);
  let Ok(()) = write(
    controller,
    SPURIOUS_VECTOR,
    ENABLED_WITH_SPURIOUS_VECTOR_255,
  );
  let Ok(()) = write(controller, TASK_PRIORITY, 0);
  let Ok(()) = write(controller, DIVIDE, BY_SIXTEEN);
  let Ok(()) = write(controller, TIMER_ENTRY, PERIODIC_ON_VECTOR_32);
  let Ok(()) = write(controller, INITIAL_COUNT, PERIOD);
  let Ok(()) = wait(before.saturating_add(wanted));
  let Ok(()) = write(controller, TIMER_ENTRY, MASKED);
  let Ok(()) = write(controller, INITIAL_COUNT, STOPPED);
  let counted = TICKS.load(Ordering::SeqCst).saturating_sub(before);

  match ::core::primitive::bool::then_some(counted >= wanted, ()) {
    ::core::option::Option::Some(()) => Ok(Counted {
      ticks: counted,
      mode,
    }),
    ::core::option::Option::None => Err(TimerError::Stalled(counted)),
  }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the boot timer poll samples this CPU TSC and enables interrupts only while the checked timer and exception handlers are installed"
  )
)]
fn wait(until: u64) -> Result<(), Never> {
  // SAFETY: the bootstrap processor supports RDTSC and stays on this core for the check.
  let started = unsafe { _rdtsc() };

  // SAFETY: every vector has a gate and a handler that returns or ends the run, the legacy controller is masked, and the only source left unmasked is the timer just programmed.
  unsafe { asm!("sti", options(nomem, nostack)) };

  let _finished = (0..TIMEOUT_CYCLES).any(|_poll| {
    core::hint::spin_loop();

    // SAFETY: RDTSC reads the same counter as above and does not access memory.
    let elapsed = unsafe { _rdtsc() }.wrapping_sub(started);

    TICKS.load(Ordering::SeqCst) >= until || elapsed >= TIMEOUT_CYCLES
  });

  // SAFETY: masking interrupts touches no memory, and the rest of the boot runs as it did before the timer was tried.
  unsafe { asm!("cli", options(nomem, nostack)) };

  Ok(())
}

#[must_use]
fn enable_controller() -> Result<LocalController, Never> {
  let Ok(base) = base_register();
  let Ok(controller) = LocalController::read(base);

  match controller {
    LocalController::MemoryMapped(_) => match __cpuid(1).ecx & X2APIC_FEATURE {
      0 => Ok(controller),
      _ => {
        let Ok(()) = write_msr(BASE_REGISTER, base | u64::from(X2APIC_ENABLE));

        Ok(LocalController::RegisterMapped)
      }
    },
    LocalController::RegisterMapped | LocalController::Disabled => Ok(controller),
  }
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the APIC base is read from its privileged model-specific register"
  )
)]
fn base_register() -> Result<u64, Never> {
  let low: u32;
  let high: u32;

  // SAFETY: the APIC base register exists on every processor that reaches long mode, and reading it changes nothing.
  unsafe {
    asm!("rdmsr", in("ecx") BASE_REGISTER, out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags))
  };

  Ok(::core::primitive::u64::wrapping_shl(u64::from(high), 32_u32) | u64::from(low))
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the xAPIC register write uses the firmware identity map and a single DWORD store"
  )
)]
fn write(controller: LocalController, register: Register, value: u32) -> Result<(), Never> {
  match controller {
    LocalController::MemoryMapped(base) => {
      // SAFETY: the firmware's identity map covers the xAPIC page, and these registers take a single doubleword write.
      unsafe {
        asm!("mov dword ptr [{base} + {register}], {value:e}", base = in(reg) base.0, register = in(reg) u64::from(register.0), value = in(reg) value, options(nostack, preserves_flags))
      };

      Ok(())
    }
    LocalController::RegisterMapped => write_msr(
      X2APIC_MSR_BASE.saturating_add(register.0.wrapping_shr(4)),
      u64::from(value),
    ),
    LocalController::Disabled => Ok(()),
  }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "x2APIC is programmed through privileged WRMSR after checking controller support"
  )
)]
fn write_msr(register: u32, value: u64) -> Result<(), Never> {
  let [a, b, c, d, e, f, g, h] = value.to_le_bytes();
  let low = u32::from_le_bytes([a, b, c, d]);
  let high = u32::from_le_bytes([e, f, g, h]);

  // SAFETY: the caller writes either the supported xAPIC-to-x2APIC transition or a writable register in active x2APIC mode.
  unsafe {
    asm!("wrmsr", in("ecx") register, in("eax") low, in("edx") high, options(nostack, preserves_flags))
  };

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "masking the legacy interrupt controllers requires writing their I/O ports"
  )
)]
fn out(port: u16, value: u8) -> Result<(), Never> {
  // SAFETY: the two ports written are the legacy interrupt controllers' mask registers, which nothing else in the kernel drives; an `out` touches no memory.
  unsafe {
    asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags))
  };

  Ok(())
}

impl fmt::Display for TimerError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      TimerError::Disabled => to.write_str("the local interrupt controller is disabled"),
      TimerError::Stalled(counted) => write!(
        to,
        "the timer interrupted {counted} times before the deadline"
      ),
    }
  }
}
