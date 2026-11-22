//! What the processor is told about where an interrupt goes, as numbers.
//!
//! Until the kernel loads tables of its own it runs on the firmware's, and a
//! fault after boot services have gone is answered by whatever the firmware
//! left behind or by nothing: a triple fault, and a machine that resets
//! without a word. The tables are three. The segments say what code and data
//! are, which in long mode is one bit that matters and a shape the processor
//! still insists on; the task state gives the processor a kernel stack for
//! user-mode traps and a separate stack for double faults; and the gates
//! say, for every vector, where the processor goes.
//!
//! All of it is here rather than beside the firmware because none of it
//! touches a machine. A descriptor is bytes at positions the manual gives,
//! and they are laid out with `from_le_bytes` over the bytes in the manual's
//! order rather than with shifts, so the encoding reads as the figure it was
//! copied from. Nothing is packed: the task state's fields sit at odd
//! multiples of four, so it is spelled in halves of four bytes and needs no
//! packing to land where the manual says, and the ten bytes `lgdt` and
//! `lidt` read are ten bytes. The tests hold each encoding against the
//! manual's layout, on this machine, before anything is booted.
//!
//! What a vector means is decided here too. The stubs the gates point at are
//! all one size, so a stub's address is the first one's plus a multiple, and
//! `Arrival` is the one place that says which vectors come back and which are
//! the end of the run.

use core::fmt;
use core::mem::{offset_of, size_of};

use console_core_never::Never;
use console_kernel_heap::Bytes;

#[cfg(kani)]
#[path = "../proofs/interrupts.rs"]
mod proofs;

pub const KERNEL_CODE: Selector = Selector(0x08);

pub const KERNEL_DATA: Selector = Selector(0x10);

pub const TASK_STATE: Selector = Selector(0x18);

pub const USER_CODE: Selector = Selector(0x2b);

pub const USER_DATA: Selector = Selector(0x33);

pub const USER_SYSCALL: Vector = Vector(0x80);

pub const VECTORS: u64 = 256;

pub const STUB: Bytes = Bytes(16);

pub const TIMER: Vector = Vector(32);

pub const SPURIOUS: Vector = Vector(255);

const DOUBLE_FAULT: Vector = Vector(8);

const STACK_ALIGNMENT: u64 = 16;

const PRESENT: u8 = 0x80;

const CODE_OR_DATA: u8 = 0x10;

const USER_PRIVILEGE: u8 = 0x60;

const EXECUTABLE: u8 = 0x08;

const READABLE_OR_WRITABLE: u8 = 0x02;

const AVAILABLE_TASK_STATE: u8 = 0x09;

const INTERRUPT_GATE: u8 = 0x0e;

const GRANULAR: u8 = 0x80;

const DEFAULT_32: u8 = 0x40;

const LONG: u8 = 0x20;

const LIMIT_HIGH: u8 = 0x0f;

const TASK_STATE_LIMIT: [u8; 2] = [0x67, 0x00];

const NO_IO_MAP: u16 = 104;

const CONTROLLER_ENABLED: u64 = 0x800;

const CONTROLLER_REGISTER_MAPPED: u64 = 0x400;

const CONTROLLER_BASE: u64 = 0x000f_ffff_ffff_f000;

const _: [(); 56] = [(); size_of::<Segments>()];

const _: [(); 104] = [(); size_of::<TaskState>()];

const _: [(); 36] = [(); offset_of!(TaskState, interrupt_stacks)];

const _: [(); 4] = [(); offset_of!(TaskState, privilege_stacks)];

const _: [(); 102] = [(); offset_of!(TaskState, io_map_base)];

const _: [(); 16] = [(); size_of::<Gate>()];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selector(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Address(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vector(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableError {
  Empty,
  InvalidVector(Vector),
  TooLong(Bytes),
  Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stack {
  Interrupted,
  DoubleFault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Segments {
  null: u64,
  code: u64,
  data: u64,
  task_state: [u64; 2],
  user_code: u64,
  user_data: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct Halves {
  low: u32,
  high: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct TaskState {
  reserved_before_privilege_stacks: u32,
  privilege_stacks: [Halves; 3],
  reserved_before_interrupt_stacks: Halves,
  interrupt_stacks: [Halves; 7],
  reserved_after_interrupt_stacks: Halves,
  reserved_before_io_map_base: u16,
  io_map_base: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Gate {
  low: u64,
  high: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exception {
  DivideError,
  Debug,
  NonMaskableInterrupt,
  Overflow,
  BoundRangeExceeded,
  InvalidOpcode,
  DeviceNotAvailable,
  DoubleFault,
  InvalidTaskState,
  SegmentNotPresent,
  StackSegmentFault,
  GeneralProtection,
  PageFault,
  FloatingPoint,
  AlignmentCheck,
  MachineCheck,
  SimdFloatingPoint,
  Virtualization,
  ControlProtection,
  Reserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrival {
  Breakpoint,
  Tick,
  Spurious,
  Exception(Exception),
  Unexpected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptAction {
  CountBreakpoint,
  CountTick,
  Resume,
  Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExceptionReport {
  pub vector: Vector,
  pub instruction: Address,
  pub error_code: u64,
  pub accessed: Address,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalController {
  Disabled,
  MemoryMapped(Address),
  RegisterMapped,
}

impl Segments {
  #[must_use]
  pub fn new(task_state: Address) -> Result<Self, Never> {
    let [first, second, third, fourth, fifth, sixth, seventh, eighth] =
      task_state.0.to_le_bytes();
    let [limit_low, limit_high] = TASK_STATE_LIMIT;

    Ok(Segments {
      null: 0,
      code: u64::from_le_bytes([
        0xff,
        0xff,
        0,
        0,
        0,
        PRESENT | CODE_OR_DATA | EXECUTABLE | READABLE_OR_WRITABLE,
        GRANULAR | LONG | LIMIT_HIGH,
        0,
      ]),
      data: u64::from_le_bytes([
        0xff,
        0xff,
        0,
        0,
        0,
        PRESENT | CODE_OR_DATA | READABLE_OR_WRITABLE,
        GRANULAR | DEFAULT_32 | LIMIT_HIGH,
        0,
      ]),
      task_state: [
        u64::from_le_bytes([
          limit_low,
          limit_high,
          first,
          second,
          third,
          PRESENT | AVAILABLE_TASK_STATE,
          0,
          fourth,
        ]),
        u64::from_le_bytes([fifth, sixth, seventh, eighth, 0, 0, 0, 0]),
      ],
      user_code: u64::from_le_bytes([
        0xff,
        0xff,
        0,
        0,
        0,
        PRESENT | USER_PRIVILEGE | CODE_OR_DATA | EXECUTABLE | READABLE_OR_WRITABLE,
        GRANULAR | LONG | LIMIT_HIGH,
        0,
      ]),
      user_data: u64::from_le_bytes([
        0xff,
        0xff,
        0,
        0,
        0,
        PRESENT | USER_PRIVILEGE | CODE_OR_DATA | READABLE_OR_WRITABLE,
        GRANULAR | DEFAULT_32 | LIMIT_HIGH,
        0,
      ]),
    })
  }
}

impl Halves {
  const ZERO: Halves = Halves { low: 0, high: 0 };

  fn of(whole: u64) -> Result<Self, Never> {
    let [first, second, third, fourth, fifth, sixth, seventh, eighth] = whole.to_le_bytes();

    Ok(Halves {
      low: u32::from_le_bytes([first, second, third, fourth]),
      high: u32::from_le_bytes([fifth, sixth, seventh, eighth]),
    })
  }
}

impl TaskState {
  #[must_use]
  pub fn new(kernel_stack: Address, double_fault_stack: Address) -> Result<Self, Never> {
    let Ok(privilege_zero) = Halves::of(kernel_stack.0);
    let Ok(double_fault) = Halves::of(double_fault_stack.0);

    Ok(TaskState {
      reserved_before_privilege_stacks: 0,
      privilege_stacks: [privilege_zero, Halves::ZERO, Halves::ZERO],
      reserved_before_interrupt_stacks: Halves::ZERO,
      interrupt_stacks: [
        double_fault,
        Halves::ZERO,
        Halves::ZERO,
        Halves::ZERO,
        Halves::ZERO,
        Halves::ZERO,
        Halves::ZERO,
      ],
      reserved_after_interrupt_stacks: Halves::ZERO,
      reserved_before_io_map_base: 0,
      io_map_base: NO_IO_MAP,
    })
  }
}

impl Stack {
  fn index(self) -> Result<u8, Never> {
    Ok(match self {
      Stack::Interrupted => 0,
      Stack::DoubleFault => 1,
    })
  }
}

impl Gate {
  #[must_use]
  pub fn new(handler: Address, stack: Stack) -> Result<Self, Never> {
    Self::with_privilege(handler, stack, 0)
  }

  fn with_privilege(handler: Address, stack: Stack, privilege: u8) -> Result<Self, Never> {
    let [first, second, third, fourth, fifth, sixth, seventh, eighth] = handler.0.to_le_bytes();
    let [selector_low, selector_high] = KERNEL_CODE.0.to_le_bytes();
    let Ok(stack_index) = stack.index();

    Ok(Gate {
      low: u64::from_le_bytes([
        first,
        second,
        selector_low,
        selector_high,
        stack_index,
        PRESENT | privilege | INTERRUPT_GATE,
        third,
        fourth,
      ]),
      high: u64::from_le_bytes([fifth, sixth, seventh, eighth, 0, 0, 0, 0]),
    })
  }

  pub fn of(first_stub: Address, vector: Vector) -> Result<Self, TableError> {
    match ::core::primitive::bool::then_some(vector.0 < VECTORS, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(TableError::InvalidVector(vector)),
    }

    let offset = vector.0.checked_mul(STUB.0).ok_or(TableError::Overflow)?;
    let handler = first_stub
      .0
      .checked_add(offset)
      .ok_or(TableError::Overflow)?;

    let stack = match ::core::primitive::bool::then_some(vector == DOUBLE_FAULT, ()) {
      ::core::option::Option::Some(()) => Stack::DoubleFault,
      ::core::option::Option::None => Stack::Interrupted,
    };

    let privilege = match ::core::primitive::bool::then_some(vector == USER_SYSCALL, ()) {
      ::core::option::Option::Some(()) => USER_PRIVILEGE,
      ::core::option::Option::None => 0,
    };
    let Ok(gate) = Gate::with_privilege(Address(handler), stack, privilege);

    Ok(gate)
  }
}

pub fn table_pointer(base: Address, length: Bytes) -> Result<[u8; 10], TableError> {
  let last = length.0.checked_sub(1).ok_or(TableError::Empty)?;
  let limit = u16::try_from(last).map_err(|_too_long| TableError::TooLong(length))?;
  let [limit_low, limit_high] = limit.to_le_bytes();
  let [first, second, third, fourth, fifth, sixth, seventh, eighth] = base.0.to_le_bytes();

  Ok([
    limit_low, limit_high, first, second, third, fourth, fifth, sixth, seventh, eighth,
  ])
}

pub fn stack_top(start: Address, length: Bytes) -> Result<Address, TableError> {
  let end = start.0.checked_add(length.0).ok_or(TableError::Overflow)?;
  let top = end
    .checked_sub(end.rem_euclid(STACK_ALIGNMENT))
    .ok_or(TableError::Overflow)?;

  match ::core::primitive::bool::then_some(top > start.0, ()) {
    ::core::option::Option::Some(()) => Ok(Address(top)),
    ::core::option::Option::None => Err(TableError::Empty),
  }
}

impl Arrival {
  #[must_use]
  pub fn of(vector: Vector) -> Result<Self, Never> {
    Ok(match vector.0 {
      0 => Arrival::Exception(Exception::DivideError),
      1 => Arrival::Exception(Exception::Debug),
      2 => Arrival::Exception(Exception::NonMaskableInterrupt),
      3 => Arrival::Breakpoint,
      4 => Arrival::Exception(Exception::Overflow),
      5 => Arrival::Exception(Exception::BoundRangeExceeded),
      6 => Arrival::Exception(Exception::InvalidOpcode),
      7 => Arrival::Exception(Exception::DeviceNotAvailable),
      8 => Arrival::Exception(Exception::DoubleFault),
      10 => Arrival::Exception(Exception::InvalidTaskState),
      11 => Arrival::Exception(Exception::SegmentNotPresent),
      12 => Arrival::Exception(Exception::StackSegmentFault),
      13 => Arrival::Exception(Exception::GeneralProtection),
      14 => Arrival::Exception(Exception::PageFault),
      16 => Arrival::Exception(Exception::FloatingPoint),
      17 => Arrival::Exception(Exception::AlignmentCheck),
      18 => Arrival::Exception(Exception::MachineCheck),
      19 => Arrival::Exception(Exception::SimdFloatingPoint),
      20 => Arrival::Exception(Exception::Virtualization),
      21 => Arrival::Exception(Exception::ControlProtection),
      9 | 15 | 22..=31 => Arrival::Exception(Exception::Reserved),
      32 => Arrival::Tick,
      255 => Arrival::Spurious,
      _nobody_asked_for => Arrival::Unexpected,
    })
  }

  pub fn action(self) -> Result<InterruptAction, Never> {
    Ok(match self {
      Self::Breakpoint => InterruptAction::CountBreakpoint,
      Self::Tick => InterruptAction::CountTick,
      Self::Spurious => InterruptAction::Resume,
      Self::Exception(_) | Self::Unexpected => InterruptAction::Stop,
    })
  }
}

impl LocalController {
  #[must_use]
  pub fn read(base_register: u64) -> Result<Self, Never> {
    Ok(
      match (
        base_register & CONTROLLER_ENABLED == 0,
        base_register & CONTROLLER_REGISTER_MAPPED == 0,
      ) {
        (true, _) => LocalController::Disabled,
        (false, true) => LocalController::MemoryMapped(Address(base_register & CONTROLLER_BASE)),
        (false, false) => LocalController::RegisterMapped,
      },
    )
  }
}

impl fmt::Display for Exception {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    to.write_str(match self {
      Exception::DivideError => "divide error",
      Exception::Debug => "debug exception",
      Exception::NonMaskableInterrupt => "non-maskable interrupt",
      Exception::Overflow => "overflow",
      Exception::BoundRangeExceeded => "bound range exceeded",
      Exception::InvalidOpcode => "invalid opcode",
      Exception::DeviceNotAvailable => "device not available",
      Exception::DoubleFault => "double fault",
      Exception::InvalidTaskState => "invalid task state segment",
      Exception::SegmentNotPresent => "segment not present",
      Exception::StackSegmentFault => "stack-segment fault",
      Exception::GeneralProtection => "general protection fault",
      Exception::PageFault => "page fault",
      Exception::FloatingPoint => "x87 floating-point error",
      Exception::AlignmentCheck => "alignment check",
      Exception::MachineCheck => "machine check",
      Exception::SimdFloatingPoint => "SIMD floating-point exception",
      Exception::Virtualization => "virtualization exception",
      Exception::ControlProtection => "control protection exception",
      Exception::Reserved => "an exception the processor reserves",
    })
  }
}

impl fmt::Display for ExceptionReport {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    let ExceptionReport {
      vector,
      instruction,
      error_code,
      accessed,
    } = self;
    let Ok(arrival) = Arrival::of(*vector);

    match arrival {
      Arrival::Exception(Exception::PageFault) => {
        write!(
          to,
          "page fault at instruction {:#018x}, error code {error_code:#x}, touching {:#018x}",
          instruction.0, accessed.0
        )
      }
      Arrival::Exception(exception) => write!(
        to,
        "{exception} at instruction {:#018x}, error code {error_code:#x}",
        instruction.0
      ),
      Arrival::Breakpoint | Arrival::Tick | Arrival::Spurious | Arrival::Unexpected => {
        write!(
          to,
          "an interrupt nothing handles, vector {}, at instruction {:#018x}, error code {error_code:#x}",
          vector.0, instruction.0
        )
      }
    }
  }
}

impl fmt::Display for TableError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      TableError::Empty => to.write_str("a table or a stack with nothing in it cannot be loaded"),
      TableError::InvalidVector(Vector(vector)) => {
        write!(to, "interrupt vector {vector} is outside the table")
      }
      TableError::TooLong(Bytes(length)) => write!(
        to,
        "a table of {length} bytes is longer than the processor can be told"
      ),
      TableError::Overflow => to.write_str("an address went past the end of the address space"),
    }
  }
}

#[cfg(test)]
mod tests {
  use alloc::format;
  use alloc::vec::Vec;

  use super::*;

  const HANDLER: Address = Address(0x1234_5678_9abc_def0);

  fn bytes(words: &[u64]) -> Result<Vec<u8>, Never> {
    Ok(words.iter().flat_map(|word| word.to_le_bytes()).collect())
  }

  fn halves(state: &TaskState) -> Result<Vec<u8>, Never> {
    let stacks = state
      .privilege_stacks
      .iter()
      .chain([&state.reserved_before_interrupt_stacks])
      .chain(&state.interrupt_stacks)
      .chain([&state.reserved_after_interrupt_stacks]);

    Ok(
      state
        .reserved_before_privilege_stacks
        .to_le_bytes()
        .into_iter()
        .chain(stacks.flat_map(|half| {
          half
            .low
            .to_le_bytes()
            .into_iter()
            .chain(half.high.to_le_bytes())
        }))
        .chain(state.reserved_before_io_map_base.to_le_bytes())
        .chain(state.io_map_base.to_le_bytes())
        .collect(),
    )
  }

  #[test]
  fn the_segments_are_the_flat_long_mode_ones_the_manual_draws() -> Result<(), Never> {
    let Ok(segments) = Segments::new(HANDLER);

    assert_eq!(segments.null, 0);
    assert_eq!(segments.code, 0x00af_9a00_0000_ffff);
    assert_eq!(segments.data, 0x00cf_9200_0000_ffff);
    assert_eq!(segments.user_code, 0x00af_fa00_0000_ffff);
    assert_eq!(segments.user_data, 0x00cf_f200_0000_ffff);

    Ok(())
  }

  #[test]
  fn a_task_state_descriptor_spreads_its_address_over_sixteen_bytes() -> Result<(), Never> {
    let Ok(segments) = Segments::new(HANDLER);
    let Ok(written) = bytes(&segments.task_state);

    assert_eq!(
      written,
      [
        0x67, 0x00, 0xf0, 0xde, 0xbc, 0x89, 0x00, 0x9a, 0x78, 0x56, 0x34, 0x12, 0x00, 0x00, 0x00,
        0x00
      ]
    );
    assert_eq!(TASK_STATE, Selector(0x18));

    Ok(())
  }

  #[test]
  fn the_task_state_holds_both_kernel_stacks_and_no_io_map() -> Result<(), Never> {
    let kernel_stack = Address(0x1122_3344_5566_7788);
    let Ok(state) = TaskState::new(kernel_stack, HANDLER);
    let Ok(written) = halves(&state);

    assert_eq!(written.len(), 104);
    assert_eq!(
      written.get(4..12),
      Some(kernel_stack.0.to_le_bytes().as_slice())
    );
    assert_eq!(
      written.get(36..44),
      Some(HANDLER.0.to_le_bytes().as_slice())
    );
    assert_eq!(written.get(102..104), Some([0x68, 0x00].as_slice()));
    assert_eq!(written.iter().filter(|byte| **byte != 0).count(), 17);

    Ok(())
  }

  #[test]
  fn a_gate_is_a_present_interrupt_gate_into_kernel_code() -> Result<(), Never> {
    let Ok(gate) = Gate::new(HANDLER, Stack::DoubleFault);
    let Ok(written) = bytes(&[gate.low, gate.high]);

    assert_eq!(
      written,
      [
        0xf0, 0xde, 0x08, 0x00, 0x01, 0x8e, 0xbc, 0x9a, 0x78, 0x56, 0x34, 0x12, 0x00, 0x00, 0x00,
        0x00
      ]
    );

    Ok(())
  }

  #[test]
  fn every_vector_has_a_stub_and_only_a_double_fault_changes_stack() -> Result<(), TableError> {
    let first = Address(0x40_0000);
    let breakpoint = Gate::of(first, Vector(3))?;
    let double_fault = Gate::of(first, Vector(8))?;
    let last = Gate::of(first, Vector(255))?;
    let syscall = Gate::of(first, USER_SYSCALL)?;
    let Ok(expected_breakpoint) = Gate::new(Address(0x40_0030), Stack::Interrupted);
    let Ok(expected_double_fault) = Gate::new(Address(0x40_0080), Stack::DoubleFault);
    let Ok(expected_last) = Gate::new(Address(0x40_0ff0), Stack::Interrupted);

    assert_eq!(breakpoint, expected_breakpoint);
    assert_eq!(double_fault, expected_double_fault);
    assert_eq!(last, expected_last);
    assert_eq!(syscall.low, 0x0040_ee00_0008_0800);
    assert_eq!(
      Gate::of(Address(u64::MAX), Vector(1)),
      Err(TableError::Overflow)
    );

    Ok(())
  }

  #[test]
  fn a_table_pointer_is_the_last_byte_then_the_address() -> Result<(), TableError> {
    let gates = table_pointer(HANDLER, Bytes(4096))?;

    assert_eq!(
      gates,
      [0xff, 0x0f, 0xf0, 0xde, 0xbc, 0x9a, 0x78, 0x56, 0x34, 0x12]
    );
    assert_eq!(table_pointer(HANDLER, Bytes(0)), Err(TableError::Empty));
    assert_eq!(
      table_pointer(HANDLER, Bytes(65537)),
      Err(TableError::TooLong(Bytes(65537)))
    );

    Ok(())
  }

  #[test]
  fn a_stack_starts_at_its_aligned_end() -> Result<(), TableError> {
    let top = stack_top(Address(0x1003), Bytes(0x4000))?;

    assert_eq!(top, Address(0x5000));
    assert_eq!(stack_top(Address(0x1001), Bytes(8)), Err(TableError::Empty));
    assert_eq!(
      stack_top(Address(u64::MAX), Bytes(16)),
      Err(TableError::Overflow)
    );

    Ok(())
  }

  #[test]
  fn the_vectors_mean_what_the_manual_says() -> Result<(), Never> {
    assert_eq!(
      Arrival::of(Vector(0)),
      Ok(Arrival::Exception(Exception::DivideError))
    );
    assert_eq!(Arrival::of(Vector(3)), Ok(Arrival::Breakpoint));
    assert_eq!(
      Arrival::of(Vector(6)),
      Ok(Arrival::Exception(Exception::InvalidOpcode))
    );
    assert_eq!(
      Arrival::of(Vector(8)),
      Ok(Arrival::Exception(Exception::DoubleFault))
    );
    assert_eq!(
      Arrival::of(Vector(13)),
      Ok(Arrival::Exception(Exception::GeneralProtection))
    );
    assert_eq!(
      Arrival::of(Vector(14)),
      Ok(Arrival::Exception(Exception::PageFault))
    );
    assert_eq!(
      Arrival::of(Vector(15)),
      Ok(Arrival::Exception(Exception::Reserved))
    );
    assert_eq!(Arrival::of(TIMER), Ok(Arrival::Tick));
    assert_eq!(Arrival::of(Vector(33)), Ok(Arrival::Unexpected));
    assert_eq!(Arrival::of(SPURIOUS), Ok(Arrival::Spurious));

    Ok(())
  }

  #[test]
  #[cfg_attr(
    dylint_lib = "explicit066_no_heap_after_init",
    expect(
      explicit066_no_heap_after_init,
      reason = "this hosted formatting test collects strings for assertions; the interrupt handler has no live allocation"
    )
  )]
  fn a_fault_says_what_where_and_why_on_one_line() -> Result<(), Never> {
    let invalid_opcode = ExceptionReport {
      vector: Vector(6),
      instruction: Address(0xe55_1000),
      error_code: 0,
      accessed: Address(0),
    };
    let page_fault = ExceptionReport {
      vector: Vector(14),
      instruction: Address(0xe55_1000),
      error_code: 2,
      accessed: Address(0xdead_b000),
    };
    let stray = ExceptionReport {
      vector: Vector(77),
      instruction: Address(0xe55_1000),
      error_code: 0,
      accessed: Address(0),
    };

    assert_eq!(
      format!("{invalid_opcode}"),
      "invalid opcode at instruction 0x000000000e551000, error code 0x0"
    );
    assert_eq!(
      format!("{page_fault}"),
      "page fault at instruction 0x000000000e551000, error code 0x2, touching 0x00000000deadb000"
    );
    assert_eq!(
      format!("{stray}"),
      "an interrupt nothing handles, vector 77, at instruction 0x000000000e551000, error code 0x0"
    );

    Ok(())
  }

  #[test]
  fn the_local_controller_is_read_off_its_base_register() -> Result<(), Never> {
    assert_eq!(
      LocalController::read(0xfee0_0900),
      Ok(LocalController::MemoryMapped(Address(0xfee0_0000)))
    );
    assert_eq!(
      LocalController::read(0xfee0_0d00),
      Ok(LocalController::RegisterMapped)
    );
    assert_eq!(
      LocalController::read(0xfee0_0100),
      Ok(LocalController::Disabled)
    );

    Ok(())
  }
}
