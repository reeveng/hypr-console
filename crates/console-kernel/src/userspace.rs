//! The first isolated renderer and browser tasks. Both execute instructions
//! from a user code page and exit through a software interrupt. The trap
//! entry runs on the stack from TSS.RSP0 and restores the kernel continuation
//! from the two words reserved above that stack. Only the boot CPU runs these
//! fixed, non-preemptible programs.

pub(super) mod document;
mod paging;

use core::arch::global_asm;
use core::fmt;

use console_kernel::interrupts::{Address, KERNEL_DATA, USER_CODE, USER_DATA, USER_SYSCALL};
use console_kernel_handles::{
  Boot, Bytes, CreatedProcess, HandleError, HandleValue, Kernel, MappingAccess, ProcessId, Right,
  Rights,
};

pub use paging::Reserved;
use paging::{CODE_ADDRESS, FRAME_ADDRESS, FRAME_ALIAS, PagingError, STACK_TOP, Task, UserCall};

pub const RETURN_CONTEXT_BYTES: u64 = 16;

const USER_CR4_MASK: i32 = !((1 << 7) | (1 << 18));
const RENDERED: u64 = 42;
const READ: u64 = 43;
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the frame width is a Rust array length used by the display and the built-in assembly test"
  )
)]
pub const FRAME_WIDTH: usize = 32;
const PIXELS_PER_COLOR: u32 = 256;
const COLORS: [u32; 4] = [0x0020_2020, 0x0060_6060, 0x00a0_a0a0, 0x00e0_e0e0];
const USER_READ_PROTECTION: u64 = 0b101;
const USER_WRITE_PROTECTION: u64 = 0b111;
const USER_PAGE_ABSENT: u64 = 0b100;
const USER_EXECUTE_PROTECTION: u64 = 0b1_0101;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Trap {
  reason: u64,
  value: u64,
}

pub(super) fn calculate(
  memory: &mut Reserved,
  stack: Address,
  program: &[u8],
  request: [u8; 8],
) -> Result<Option<f64>, PagingError> {
  let call = UserCall {
    stack: stack.0,
    argument: u64::from_le_bytes(request),
  };
  let trap = memory.browser.run(program, call)?;
  let answer = f64::from_bits(trap.value);

  Ok(match (trap.reason, answer.is_finite()) {
    (0, true) => Some(answer),
    _ => None,
  })
}

const _: () = {
  assert!(core::mem::size_of::<Trap>() == 16);
  assert!(core::mem::align_of::<Trap>() == 8);
  assert!(core::mem::offset_of!(Trap, reason) == 0);
  assert!(core::mem::offset_of!(Trap, value) == 8);
};

enum Program {
  Renderer,
  Browser,
  ReadSupervisor,
  WriteByte,
  ExecuteStack,
  ReadZero,
}

#[derive(Debug)]
pub enum Error {
  Paging(PagingError),
  Handle(HandleError),
  UnexpectedMapping,
  InvalidProgram,
  Renderer(u64, u64),
  Browser(u64, u64),
  SupervisorAccess(u64, u64),
  FrameWrite(u64, u64),
  CodeWrite(u64, u64),
  StackExecute(u64, u64),
  RevokedMapping {
    expected: u64,
    reason: u64,
    value: u64,
  },
  FrameOwner(u64, u64),
  FrameContent,
  FrameReuse(u64, u64),
}

impl fmt::Display for Error {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Paging(error) => write!(to, "{error}"),
      Self::Handle(error) => write!(to, "{error}"),
      Self::UnexpectedMapping => {
        to.write_str("the frame mapping authorization returned an unexpected result")
      }
      Self::InvalidProgram => to.write_str("the user program has invalid code boundaries"),
      Self::Renderer(reason, value) => write!(to, "renderer exit: {reason:#x}, {value:#x}"),
      Self::Browser(reason, value) => write!(to, "browser exit: {reason:#x}, {value:#x}"),
      Self::SupervisorAccess(reason, value) => {
        write!(to, "supervisor read probe: {reason:#x}, {value:#x}")
      }
      Self::FrameWrite(reason, value) => {
        write!(to, "read-only frame probe: {reason:#x}, {value:#x}")
      }
      Self::CodeWrite(reason, value) => {
        write!(to, "read-only code probe: {reason:#x}, {value:#x}")
      }
      Self::StackExecute(reason, value) => {
        write!(to, "non-executable stack probe: {reason:#x}, {value:#x}")
      }
      Self::RevokedMapping {
        expected,
        reason,
        value,
      } => {
        write!(
          to,
          "revoked frame probe at {expected:#x}: {reason:#x}, {value:#x}"
        )
      }
      Self::FrameOwner(reason, value) => {
        write!(to, "frame owner after revocation: {reason:#x}, {value:#x}")
      }
      Self::FrameContent => to.write_str("the shared frame contains unexpected pixels"),
      Self::FrameReuse(reason, value) => write!(to, "frame reuse probe: {reason:#x}, {value:#x}"),
    }
  }
}

impl From<PagingError> for Error {
  fn from(error: PagingError) -> Self {
    Self::Paging(error)
  }
}

impl From<HandleError> for Error {
  fn from(error: HandleError) -> Self {
    Self::Handle(error)
  }
}

unsafe extern "sysv64" {
  fn console_enter_user(
    root: u64,
    program: u64,
    stack: u64,
    trap_stack: u64,
    argument: u64,
  ) -> Trap;

  static renderer_start: u8;
  static renderer_end: u8;
  static browser_start: u8;
  static browser_end: u8;
  static supervisor_read_start: u8;
  static supervisor_read_end: u8;
  static write_byte_start: u8;
  static write_byte_end: u8;
  static execute_stack_start: u8;
  static execute_stack_end: u8;
  static read_zero_start: u8;
  static read_zero_end: u8;
}

global_asm!(
  ".global console_enter_user",
  "console_enter_user:",
  "push rbp",
  "push rbx",
  "push r12",
  "push r13",
  "push r14",
  "push r15",
  "pushfq",
  "cli",
  "mov rax, cr4",
  "push rax",
  "and rax, {user_cr4_mask}",
  "mov cr4, rax",
  "sub rsp, 520",
  "fxsave64 [rsp]",
  "fxrstor64 [rip + clean_fx_state]",
  "mov [rcx], rsp",
  "mov rax, cr3",
  "mov [rcx + 8], rax",
  "mov cr3, rdi",
  "push {user_data}",
  "push rdx",
  "push 2",
  "push {user_code}",
  "push rsi",
  "mov rdi, r8",
  "xor eax, eax",
  "xor ebx, ebx",
  "xor ecx, ecx",
  "xor edx, edx",
  "xor esi, esi",
  "xor ebp, ebp",
  "xor r8d, r8d",
  "xor r9d, r9d",
  "xor r10d, r10d",
  "xor r11d, r11d",
  "xor r12d, r12d",
  "xor r13d, r13d",
  "xor r14d, r14d",
  "xor r15d, r15d",
  "iretq",
  ".global CONSOLE_USER_SYSCALL",
  "CONSOLE_USER_SYSCALL:",
  "mov rdx, rdi",
  "test rax, rax",
  "jz restore_kernel",
  "mov rdx, rax",
  "mov rax, -1",
  "jmp restore_kernel",
  ".global CONSOLE_USER_PAGE_FAULT",
  "CONSOLE_USER_PAGE_FAULT:",
  "mov rax, [rsp + 8]",
  "mov rdx, cr2",
  "restore_kernel:",
  "cld",
  "mov rcx, [rsp + 64]",
  "mov cr3, rcx",
  "mov rsp, [rsp + 56]",
  "fxrstor64 [rsp]",
  "add rsp, 520",
  "pop rcx",
  "mov cr4, rcx",
  "mov cx, {kernel_data}",
  "mov ss, cx",
  "mov ds, cx",
  "mov es, cx",
  "popfq",
  "pop r15",
  "pop r14",
  "pop r13",
  "pop r12",
  "pop rbx",
  "pop rbp",
  "ret",
  ".balign 16",
  "clean_fx_state:",
  ".short 0x037f",
  ".zero 22",
  ".long 0x1f80",
  ".zero 484",
  ".global renderer_start, renderer_end",
  "renderer_start:",
  "push {rendered}",
  "mov ecx, {band}",
  "mov eax, {first}",
  "rep stosd",
  "mov ecx, {band}",
  "mov eax, {second}",
  "rep stosd",
  "mov ecx, {band}",
  "mov eax, {third}",
  "rep stosd",
  "mov ecx, {band}",
  "mov eax, {fourth}",
  "rep stosd",
  "pop rdi",
  "xor eax, eax",
  "int {syscall}",
  "renderer_end:",
  ".global browser_start, browser_end",
  "browser_start:",
  "xor eax, eax",
  "cmp dword ptr [rdi], {first}",
  "jne 2f",
  "cmp dword ptr [rdi + {band_bytes}], {second}",
  "jne 2f",
  "cmp dword ptr [rdi + 2 * {band_bytes}], {third}",
  "jne 2f",
  "cmp dword ptr [rdi + 3 * {band_bytes}], {fourth}",
  "jne 2f",
  "mov edi, {read}",
  "int {syscall}",
  "2:",
  "xor edi, edi",
  "int {syscall}",
  "browser_end:",
  ".global supervisor_read_start, supervisor_read_end",
  "supervisor_read_start:",
  "mov edi, [rdi]",
  "xor eax, eax",
  "int {syscall}",
  "supervisor_read_end:",
  ".global write_byte_start, write_byte_end",
  "write_byte_start:",
  "mov byte ptr [rdi], 0",
  "xor eax, eax",
  "int {syscall}",
  "write_byte_end:",
  ".global execute_stack_start, execute_stack_end",
  "execute_stack_start:",
  "push 0xc3",
  "jmp rsp",
  "execute_stack_end:",
  ".global read_zero_start, read_zero_end",
  "read_zero_start:",
  "xor eax, eax",
  "mov ecx, 1024",
  "repe scasd",
  "jne 3f",
  "mov edi, {read}",
  "int {syscall}",
  "3:",
  "xor edi, edi",
  "int {syscall}",
  "read_zero_end:",
  user_cr4_mask = const USER_CR4_MASK,
  kernel_data = const KERNEL_DATA.0,
  user_code = const USER_CODE.0,
  user_data = const USER_DATA.0,
  syscall = const USER_SYSCALL.0,
  rendered = const RENDERED,
  read = const READ,
  band = const PIXELS_PER_COLOR,
  band_bytes = const PIXELS_PER_COLOR.saturating_mul(4),
  first = const COLORS[0],
  second = const COLORS[1],
  third = const COLORS[2],
  fourth = const COLORS[3],
);

impl Program {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the linker-delimited program extent is a Rust slice length"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the assembly publishes two labels bounding initialized bytes in the resident image"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit096_no_unchecked_memory",
    expect(
      explicit096_no_unchecked_memory,
      reason = "each pair of linker labels bounds one built-in program and the start-to-end subtraction is checked"
    )
  )]
  fn bytes(self) -> Result<&'static [u8], Error> {
    let (start, end) = match self {
      Self::Renderer => (&raw const renderer_start, &raw const renderer_end),
      Self::Browser => (&raw const browser_start, &raw const browser_end),
      Self::ReadSupervisor => (
        &raw const supervisor_read_start,
        &raw const supervisor_read_end,
      ),
      Self::WriteByte => (&raw const write_byte_start, &raw const write_byte_end),
      Self::ExecuteStack => (&raw const execute_stack_start, &raw const execute_stack_end),
      Self::ReadZero => (&raw const read_zero_start, &raw const read_zero_end),
    };
    let length = end
      .addr()
      .checked_sub(start.addr())
      .ok_or(Error::InvalidProgram)?;

    // SAFETY: each start and end label delimits the bytes of its position-independent program in the resident kernel image.
    let code = unsafe {
      core::slice::from_raw_parts(
        core::ptr::with_exposed_provenance(start.expose_provenance()),
        length,
      )
    };

    Ok(code)
  }
}

fn lend_frame(
  kernel: &mut Kernel,
  owner: ProcessId,
  source: HandleValue,
  receiver: &CreatedProcess,
) -> Result<HandleValue, HandleError> {
  let Ok(readable) = Rights::from_rights(&[Right::Read, Right::Transfer]);
  lend(kernel, owner, source, receiver, readable)
}

fn lend(
  kernel: &mut Kernel,
  owner: ProcessId,
  source: HandleValue,
  receiver: &CreatedProcess,
  rights: Rights,
) -> Result<HandleValue, HandleError> {
  let outgoing = kernel.duplicate(owner, source, rights)?;
  kernel.write(owner, receiver.channel, &[], &[outgoing])?;

  let held = kernel.handles(receiver.id)?;
  let bootstrap =
    ::core::result::Result::<&_, _>::copied(held.first().ok_or(HandleError::BadState))?;
  let received = kernel.read(receiver.id, bootstrap)?;

  match received.handles.as_slice() {
    [frame] => Ok(*frame),
    _ => Err(HandleError::BadState),
  }
}

fn expect_unmapped(
  kernel: &mut Kernel,
  task: &mut Task<'_>,
  frame: HandleValue,
  reason: HandleError,
) -> Result<(), Error> {
  match ::core::primitive::bool::then_some(
    !matches!(kernel.authorize_mapping(task.id, frame, MappingAccess::Read), Err(actual) if actual == reason),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Err(Error::UnexpectedMapping);
    }
    ::core::option::Option::None => {}
  }

  let read = Program::Browser.bytes()?;

  for address in [FRAME_ADDRESS, FRAME_ALIAS] {
    let trap = task.space.run(
      read,
      UserCall {
        stack: task.stack,
        argument: address,
      },
    )?;

    match ::core::primitive::bool::then_some(
      trap
        == (Trap {
          reason: USER_PAGE_ABSENT,
          value: address,
        }),
      (),
    ) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        return Err(Error::RevokedMapping {
          expected: address,
          reason: trap.reason,
          value: trap.value,
        });
      }
    }
  }

  Ok(())
}

fn expect_frame_read(
  kernel: &mut Kernel,
  task: &mut Task<'_>,
  frame: HandleValue,
  program: &[u8],
  address: u64,
) -> Result<(), Error> {
  let trap = task.run_mapped(kernel, frame, MappingAccess::Read, program, address)?;

  match trap {
    Trap {
      reason: 0,
      value: READ,
    } => Ok(()),
    Trap { reason, value } => Err(Error::Browser(reason, value)),
  }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the check registers its boot-reserved, exclusively owned frame, and re-registers only after revoking and scrubbing it"
  )
)]
pub fn check(memory: &mut Reserved, stack: Address) -> Result<(), Error> {
  memory.prepare()?;
  let Ok(physical) = memory.frame.identity_address();
  let Boot {
    mut kernel,
    root,
    root_job,
    executable_resource: _,
  } = Kernel::boot_with_memory_backing(Bytes(262_144), Bytes(0), 0)?;
  let browser_process = kernel.create_process(root, root_job)?;
  let mut renderer = Task {
    id: root,
    space: &mut memory.renderer,
    stack: stack.0,
  };
  let mut browser = Task {
    id: browser_process.id,
    space: &mut memory.browser,
    stack: stack.0,
  };
  let Ok(()) = memory.frame.scrub();

  // SAFETY: Reserved holds this initialized page for the entire check, and no other registration or user mapping exists yet.
  let source = unsafe { kernel.register_frame(root, physical) }?;
  let borrowed = lend_frame(&mut kernel, root, source, &browser_process)?;
  render_frame(&mut kernel, &mut renderer, source)?;
  check_permissions(
    &mut kernel,
    &mut renderer,
    &mut browser,
    borrowed,
    Address(physical),
  )?;
  check_revocation(
    &mut kernel,
    &mut renderer,
    &mut browser,
    &browser_process,
    FrameHandles { source, borrowed },
  )?;

  let next = kernel.create_process(root, root_job)?;
  renderer.id = next.id;
  let Ok(()) = memory.frame.scrub();

  // SAFETY: the prior registration has no remaining handles or mappings, each loan was removed after switching away, and the frame is scrubbed.
  let reused = unsafe { kernel.register_frame(next.id, physical) }?;
  let zero = Program::ReadZero.bytes()?;
  let trap = renderer.run_mapped(
    &mut kernel,
    reused,
    MappingAccess::Read,
    zero,
    FRAME_ADDRESS,
  )?;

  match trap {
    Trap {
      reason: 0,
      value: READ,
    } => {}
    Trap { reason, value } => return Err(Error::FrameReuse(reason, value)),
  }

  render_frame(&mut kernel, &mut renderer, reused)?;
  kernel.close(next.id, reused)?;
  kernel.exit(next.id)?;

  let Ok(pixels) = memory.frame.pixels();

  match ::core::primitive::bool::then_some(
    pixels
      .as_chunks::<256>()
      .0
      .iter()
      .zip(COLORS)
      .all(|(row, color)| row.iter().all(|pixel| *pixel == color)),
    (),
  ) {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(Error::FrameContent),
  }
}

fn render_frame(
  kernel: &mut Kernel,
  renderer: &mut Task<'_>,
  frame: HandleValue,
) -> Result<(), Error> {
  let program = Program::Renderer.bytes()?;
  let trap = renderer.run_mapped(kernel, frame, MappingAccess::Write, program, FRAME_ADDRESS)?;

  match trap {
    Trap {
      reason: 0,
      value: RENDERED,
    } => Ok(()),
    Trap { reason, value } => Err(Error::Renderer(reason, value)),
  }
}

fn check_permissions(
  kernel: &mut Kernel,
  renderer: &mut Task<'_>,
  browser: &mut Task<'_>,
  borrowed: HandleValue,
  physical: Address,
) -> Result<(), Error> {
  let read = Program::Browser.bytes()?;

  for address in [FRAME_ADDRESS, FRAME_ALIAS] {
    expect_frame_read(kernel, browser, borrowed, read, address)?;
  }

  let write_denied = matches!(
    kernel.authorize_mapping(browser.id, borrowed, MappingAccess::Write),
    Err(HandleError::AccessDenied(Right::Write))
  );

  match write_denied.then_some(()) {
    Some(()) => {}
    None => return Err(Error::UnexpectedMapping),
  }

  let read_supervisor = Program::ReadSupervisor.bytes()?;
  let supervisor = renderer.space.run(
    read_supervisor,
    UserCall {
      stack: renderer.stack,
      argument: physical.0,
    },
  )?;

  match ::core::primitive::bool::then_some(
    supervisor
      == (Trap {
        reason: USER_READ_PROTECTION,
        value: physical.0,
      }),
    (),
  ) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      return Err(Error::SupervisorAccess(supervisor.reason, supervisor.value));
    }
  }

  let write_byte = Program::WriteByte.bytes()?;
  let frame_write = browser.run_mapped(
    kernel,
    borrowed,
    MappingAccess::Read,
    write_byte,
    FRAME_ADDRESS,
  )?;

  match frame_write {
    Trap {
      reason: USER_WRITE_PROTECTION,
      value: FRAME_ADDRESS,
    } => {}
    Trap { reason, value } => return Err(Error::FrameWrite(reason, value)),
  }

  let code_write = browser.space.run(
    write_byte,
    UserCall {
      stack: browser.stack,
      argument: CODE_ADDRESS,
    },
  )?;

  match code_write {
    Trap {
      reason: USER_WRITE_PROTECTION,
      value: CODE_ADDRESS,
    } => {}
    Trap { reason, value } => return Err(Error::CodeWrite(reason, value)),
  }

  let execute = Program::ExecuteStack.bytes()?;
  let stack = browser.space.run(
    execute,
    UserCall {
      stack: browser.stack,
      argument: 0,
    },
  )?;
  let denied = Trap {
    reason: USER_EXECUTE_PROTECTION,
    value: STACK_TOP.saturating_sub(8),
  };

  match ::core::primitive::bool::then_some(stack == denied, ()) {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(Error::StackExecute(stack.reason, stack.value)),
  }
}

struct FrameHandles {
  source: HandleValue,
  borrowed: HandleValue,
}

fn check_revocation(
  kernel: &mut Kernel,
  renderer: &mut Task<'_>,
  browser: &mut Task<'_>,
  browser_process: &CreatedProcess,
  handles: FrameHandles,
) -> Result<(), Error> {
  let FrameHandles { source, borrowed } = handles;
  kernel.revoke(renderer.id, source)?;
  expect_unmapped(kernel, browser, borrowed, HandleError::BadHandle)?;

  let read = Program::Browser.bytes()?;
  let owner = renderer.run_mapped(kernel, source, MappingAccess::Read, read, FRAME_ADDRESS)?;

  match owner {
    Trap {
      reason: 0,
      value: READ,
    } => {}
    Trap { reason, value } => return Err(Error::FrameOwner(reason, value)),
  }

  let closed = lend_frame(kernel, renderer.id, source, browser_process)?;
  expect_frame_read(kernel, browser, closed, read, FRAME_ADDRESS)?;
  kernel.close(browser.id, closed)?;
  expect_unmapped(kernel, browser, closed, HandleError::BadHandle)?;

  let exited = lend_frame(kernel, renderer.id, source, browser_process)?;
  expect_frame_read(kernel, browser, exited, read, FRAME_ALIAS)?;
  kernel.exit(browser.id)?;
  expect_unmapped(kernel, browser, exited, HandleError::ProcessNotFound)?;
  kernel.close(renderer.id, source)?;

  Ok(())
}
