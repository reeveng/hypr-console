//! The kernel's own tables, loaded, and where the processor arrives through them.
//!
//! `console_kernel::interrupts` says what the tables hold; this is the edge
//! that puts them in memory that is never freed, tells the processor where
//! they are, and is entered when it uses them. The tables come off the
//! kernel's heap and are leaked rather than kept in a `static`: they are
//! built once from addresses only known when the machine is up, and nothing
//! reads them again but the processor.
//!
//! Entry is assembly, because nothing the pinned compiler offers on stable is
//! an interrupt handler: `extern "x86-interrupt"` is still behind a feature
//! gate, and the tree does not move to nightly for one calling convention.
//! Every vector has a stub of the same size -- so the gates are filled by
//! multiplying rather than from a list of symbols -- and each stub evens out
//! the one difference between vectors, pushing the error code the processor
//! did not, then its own number, and joins the one path that saves every
//! register, the floating-point state included, and calls `dispatch`. A
//! naked function per vector would be the same instructions written through
//! a macro; one assembler loop writes them without one.
//!
//! Kernel breakpoints and timer ticks resume the interrupted code. User-mode
//! exits and page faults return to the caller that entered the task. An
//! unhandled fault says one line through the
//! serial port, without the heap, because the fault may have interrupted the
//! allocator, and answers failure at the exit port instead of leaving the
//! machine to reset on a third fault.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::arch::{asm, global_asm};
use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use console_core_never::Never;
use console_kernel::interrupts::{
  Address, Arrival, ExceptionReport, Gate, InterruptAction, KERNEL_CODE, KERNEL_DATA, Segments,
  TASK_STATE, TableError, TaskState, USER_SYSCALL, VECTORS, Vector, stack_top, table_pointer,
};
use console_kernel_heap::Bytes;

use crate::boot;
use crate::display;
use crate::serial::Serial;
use crate::timer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Breakpoint {
  TakenOnce,
  Miscounted,
}

#[repr(C)]
pub struct Frame {
  vector: u64,
  error_code: u64,
  instruction: u64,
}

const _: () = {
  assert!(core::mem::size_of::<Frame>() == 24);
  assert!(core::mem::align_of::<Frame>() == 8);
  assert!(core::mem::offset_of!(Frame, vector) == 0);
  assert!(core::mem::offset_of!(Frame, error_code) == 8);
  assert!(core::mem::offset_of!(Frame, instruction) == 16);
};

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
static BREAKPOINTS: AtomicU64 = AtomicU64::new(0);

global_asm!(
    ".balign 16",
    ".global CONSOLE_KERNEL_INTERRUPT_STUBS",
    "CONSOLE_KERNEL_INTERRUPT_STUBS:",
    ".set CONSOLE_KERNEL_VECTOR, 0",
    ".rept 256",
    ".balign 16",
    ".if (CONSOLE_KERNEL_VECTOR == 8) || ((CONSOLE_KERNEL_VECTOR >= 10) && (CONSOLE_KERNEL_VECTOR <= 14)) || (CONSOLE_KERNEL_VECTOR == 17) || (CONSOLE_KERNEL_VECTOR == 21) || (CONSOLE_KERNEL_VECTOR == 29) || (CONSOLE_KERNEL_VECTOR == 30)",
    ".else",
    "pushq $0",
    ".endif",
    "pushq $CONSOLE_KERNEL_VECTOR",
    "jmp CONSOLE_KERNEL_INTERRUPT_ENTRY",
    ".set CONSOLE_KERNEL_VECTOR, CONSOLE_KERNEL_VECTOR + 1",
    ".endr",
    ".balign 16",
    "CONSOLE_KERNEL_INTERRUPT_ENTRY:",
    "testb $3, 24(%rsp)",
    "jz 1f",
    "cmpq ${syscall}, (%rsp)",
    "je CONSOLE_USER_SYSCALL",
    "cmpq $14, (%rsp)",
    "je CONSOLE_USER_PAGE_FAULT",
    "1:",
    "pushq %r15",
    "pushq %r14",
    "pushq %r13",
    "pushq %r12",
    "pushq %r11",
    "pushq %r10",
    "pushq %r9",
    "pushq %r8",
    "pushq %rbp",
    "pushq %rdi",
    "pushq %rsi",
    "pushq %rdx",
    "pushq %rcx",
    "pushq %rbx",
    "pushq %rax",
    "leaq 120(%rsp), %rdi",
    "movq %rsp, %rbp",
    "subq $512, %rsp",
    "andq $-16, %rsp",
    "fxsave64 (%rsp)",
    "cld",
    "call {dispatch}",
    "fxrstor64 (%rsp)",
    "movq %rbp, %rsp",
    "popq %rax",
    "popq %rbx",
    "popq %rcx",
    "popq %rdx",
    "popq %rsi",
    "popq %rdi",
    "popq %rbp",
    "popq %r8",
    "popq %r9",
    "popq %r10",
    "popq %r11",
    "popq %r12",
    "popq %r13",
    "popq %r14",
    "popq %r15",
    "addq $16, %rsp",
    "iretq",
    dispatch = sym dispatch,
    syscall = const USER_SYSCALL.0,
    options(att_syntax),
);

extern "sysv64" fn dispatch(frame: &Frame) {
  let Ok(arrival) = Arrival::of(Vector(frame.vector));
  let Ok(action) = arrival.action();

  match action {
    InterruptAction::CountBreakpoint => {
      let _before = BREAKPOINTS.fetch_add(1, Ordering::SeqCst);
    }
    InterruptAction::CountTick => {
      let Ok(()) = timer::tick();
    }
    InterruptAction::Resume => {}
    InterruptAction::Stop => fatal(frame),
  }
}

fn fatal(frame: &Frame) -> ! {
  let Ok(accessed) = last_page_fault();
  let fault = ExceptionReport {
    vector: Vector(frame.vector),
    instruction: Address(frame.instruction),
    error_code: frame.error_code,
    accessed,
  };
  let mut serial = Serial;

  let Ok(()) = display::write_without_waiting(format_args!("console-kernel: {fault}\n"));
  let Ok(()) = match write!(serial, "console-kernel: {fault}\r\n") {
    Ok(()) | Err(core::fmt::Error) => Ok::<(), Never>(()),
  };

  boot::exit(boot::FAILURE)
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the exception handler reads the fault address from privileged register CR2"
  )
)]
fn last_page_fault() -> Result<Address, Never> {
  let accessed: u64;

  // SAFETY: reading CR2 is allowed at the privilege the kernel runs at and changes nothing; it holds the address of the last page fault, or zero before the first.
  unsafe { asm!("mov {}, cr2", out(reg) accessed, options(nomem, nostack, preserves_flags)) };

  Ok(Address(accessed))
}

#[cfg_attr(
  dylint_lib = "explicit059_no_unchecked_api",
  allow(
    explicit059_no_unchecked_api,
    reason = "the processor reads the tables and the double-fault stack by address for as long as the machine is up, so there is no owner whose end could be said: freeing any of them would leave it pointing at memory the heap hands to somebody else"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit066_no_heap_after_init",
  expect(
    explicit066_no_heap_after_init,
    reason = "the boot CPU reserves the exception tables and double-fault stack before enabling interrupts; the CPU retains them for the kernel lifetime"
  )
)]
pub fn install(trap_top: Address) -> Result<(), TableError> {
  let double_fault = vec![0_u8; 16384].leak();
  let stack_base = address(double_fault.as_ptr())?;
  let stack_bytes = length(double_fault)?;
  let double_fault_top = stack_top(stack_base, stack_bytes)?;
  let Ok(state) = TaskState::new(trap_top, double_fault_top);
  let kept_state = Box::leak(Box::new(state));
  let state_at = address(core::ptr::from_ref(kept_state))?;
  let Ok(segments) = Segments::new(state_at);
  let kept_segments = Box::leak(Box::new(segments));
  let segments_at = address(core::ptr::from_ref(kept_segments))?;
  let segments_length = length(kept_segments)?;
  let segments_pointer = table_pointer(segments_at, segments_length)?;
  let Ok(()) = load_segments(&segments_pointer);
  let Ok(first_stub) = first_stub();
  let built = (0..VECTORS)
    .map(|vector| Gate::of(first_stub, Vector(vector)))
    .collect::<Result<Vec<Gate>, TableError>>();
  let every = built?;
  let gates = every.leak();
  let gates_at = address(gates.as_ptr())?;
  let gates_length = length(gates)?;
  let gates_pointer = table_pointer(gates_at, gates_length)?;

  let Ok(()) = load_gates(&gates_pointer);

  Ok(())
}

fn address<T>(of: *const T) -> Result<Address, TableError> {
  u64::try_from(of.expose_provenance())
    .map(Address)
    .map_err(|_too_wide| TableError::Overflow)
}

fn length<T: ?Sized>(of: &T) -> Result<Bytes, TableError> {
  u64::try_from(core::mem::size_of_val(of))
    .map(Bytes)
    .map_err(|_too_wide| TableError::Overflow)
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the interrupt stub base is a linker symbol read through RIP-relative assembly"
  )
)]
fn first_stub() -> Result<Address, Never> {
  let stub: u64;

  // SAFETY: the symbol is the first interrupt stub, assembled above into this program's own text, and `lea` only computes where it is.
  unsafe {
    asm!("lea {}, [rip + CONSOLE_KERNEL_INTERRUPT_STUBS]", out(reg) stub, options(nomem, nostack, preserves_flags))
  };

  Ok(Address(stub))
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "loading the validated GDT and task-state selector requires privileged x86 instructions"
  )
)]
fn load_segments(pointer: &[u8; 10]) -> Result<(), Never> {
  // SAFETY: `pointer` names a leaked table whose second and third entries are flat long-mode code and data and whose fourth is a leaked task state, interrupts are masked, and the far return lands on the next instruction in the same code under the new selector.
  unsafe {
    asm!(
        "lgdt [{pointer}]",
        "push {code}",
        "lea {scratch}, [rip + 2f]",
        "push {scratch}",
        "retfq",
        "2:",
        "mov ds, {data:x}",
        "mov es, {data:x}",
        "mov fs, {data:x}",
        "mov gs, {data:x}",
        "mov ss, {data:x}",
        "ltr {task:x}",
        pointer = in(reg) pointer.as_ptr(),
        code = in(reg) u64::from(KERNEL_CODE.0),
        data = in(reg) KERNEL_DATA.0,
        task = in(reg) TASK_STATE.0,
        scratch = out(reg) _,
    )
  };

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "LIDT installs the verified table of addressable handlers before interrupts are enabled"
  )
)]
fn load_gates(pointer: &[u8; 10]) -> Result<(), Never> {
  // SAFETY: `pointer` names a leaked table of one gate per vector, each into a stub assembled above, reached through the code selector that was just loaded.
  unsafe {
    asm!("lidt [{}]", in(reg) pointer.as_ptr(), options(readonly, nostack, preserves_flags))
  };

  Ok(())
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the boot trap self-test invokes the installed breakpoint gate with INT3"
  )
)]
pub fn breakpoint() -> Result<Breakpoint, Never> {
  let before = BREAKPOINTS.load(Ordering::SeqCst);

  // SAFETY: the breakpoint gate is loaded and its handler counts and returns to the next instruction with every register as it was.
  unsafe { asm!("int3") };

  let after = BREAKPOINTS.load(Ordering::SeqCst);

  Ok(match after.checked_sub(before) {
    Some(1) => Breakpoint::TakenOnce,
    Some(_) | None => Breakpoint::Miscounted,
  })
}
