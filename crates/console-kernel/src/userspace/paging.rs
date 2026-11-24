//! A four-level address space for the built-in user programs. Each root
//! keeps the firmware's identity map at supervisor privilege and adds one
//! branch for user pages. The two roots can point at the same frame with
//! different permissions. The boot frame cursor reserves this storage before
//! the heap takes ownership of the remaining frames.

use core::arch::{asm, x86_64::__cpuid};
use core::fmt;
use core::mem::{offset_of, size_of};

use console_core_never::Never;
use console_kernel::frame_mappings::{
  FrameAccess, InvalidFrame, NewBranch, UserBranchError, UserPage, clear_frame, supervisor_entry,
  user_branch, user_leaf, with_frame_mapping,
};
use console_kernel::interrupts::Address;
use console_kernel::memory::{Frames, MemoryError, PAGE};
use console_kernel_handles::{HandleValue, Kernel, MappingAccess, ProcessId};

use super::document::wire::{DOCUMENT_ADDRESS, Document};

pub(super) const CODE_ADDRESS: u64 = 0x4000_0000_0000;
pub(super) const STACK_TOP: u64 = CODE_ADDRESS + 3 * PAGE;
pub const FRAME_ADDRESS: u64 = CODE_ADDRESS + 4 * PAGE;
pub const FRAME_ALIAS: u64 = FRAME_ADDRESS + PAGE;
const TRAP_STACK_BYTES: u64 = 16_384;

const ADDRESS: u64 = 0x000f_ffff_ffff_f000;
const FIVE_LEVELS: u64 = 1 << 12;
const PCID: u64 = 1 << 17;
const EFER: u32 = 0xc000_0080;
const NXE: u32 = 1 << 11;

#[repr(C, align(4096))]
struct Page<T>(T);

type Table = Page<[u64; 512]>;

#[repr(C)]
struct Leaves {
  code: u64,
  guard: u64,
  stack: u64,
  gap: u64,
  frame: [u64; 2],
  document: u64,
  unused: [u64; 505],
}

const _: [(); 4096] = [(); size_of::<Leaves>()];
const _: [(); 32] = [(); offset_of!(Leaves, frame)];
const _: () =
  assert!(DOCUMENT_ADDRESS == CODE_ADDRESS + 6 * PAGE && offset_of!(Leaves, document) == 48);

pub struct Frame(Page<[u32; 1024]>);

#[repr(C)]
pub struct AddressSpace {
  root: Table,
  directory_pointer: Table,
  directory: Table,
  leaves: Page<Leaves>,
  code: Page<[u8; 4096]>,
  stack: Page<[u8; 4096]>,
}

#[repr(C)]
pub struct Reserved {
  pub renderer: AddressSpace,
  pub browser: AddressSpace,
  pub frame: Frame,
  document: Page<Document>,
  trap_stack: Page<[u8; 16_384]>,
}

pub(super) struct Task<'space> {
  pub(super) id: ProcessId,
  pub(super) space: &'space mut AddressSpace,
  pub(super) stack: u64,
}

#[derive(Clone, Copy)]
pub(super) struct UserCall {
  pub(super) stack: u64,
  pub(super) argument: u64,
}

impl Task<'_> {
  pub(super) fn run_mapped(
    &mut self,
    kernel: &mut Kernel,
    frame: HandleValue,
    access: MappingAccess,
    program: &[u8],
    argument: u64,
  ) -> Result<super::Trap, super::Error> {
    let authority = kernel.authorize_mapping(self.id, frame, access)?;
    let AddressSpace {
      root,
      leaves,
      code,
      stack,
      ..
    } = &mut *self.space;
    let call = UserCall {
      stack: self.stack,
      argument,
    };
    let copy_result = with_frame_mapping(authority, &mut leaves.0.frame, move || {
      run_program(root, code, stack, program, call)
    })
    .map_err(PagingError::from)?;
    let trap = copy_result?;

    Ok(trap)
  }
}

const RESERVED_PAGES: u64 = 18;
const _: () = assert!(
  u64::from_le_bytes(size_of::<Reserved>().to_le_bytes()) == RESERVED_PAGES.saturating_mul(PAGE)
);

#[derive(Debug)]
pub enum PagingError {
  Unsupported,
  UserSlotOccupied,
  ProgramTooLarge,
  InvalidFrame,
}

impl fmt::Display for PagingError {
  fn fmt(&self, into: &mut fmt::Formatter<'_>) -> fmt::Result {
    into.write_str(match self {
      Self::Unsupported => "user tasks require four-level paging, NX and PCID disabled",
      Self::UserSlotOccupied => "the user virtual address range overlaps the firmware map",
      Self::ProgramTooLarge => "the user program does not fit into a code page",
      Self::InvalidFrame => "the frame has an invalid physical address",
    })
  }
}

impl From<UserBranchError> for PagingError {
  fn from(error: UserBranchError) -> Self {
    match error {
      UserBranchError::Occupied => Self::UserSlotOccupied,
      UserBranchError::InvalidFrame => Self::InvalidFrame,
    }
  }
}

impl From<InvalidFrame> for PagingError {
  fn from(_invalid: InvalidFrame) -> Self {
    Self::InvalidFrame
  }
}

impl<T> Page<T> {
  fn physical(&self) -> Result<u64, Never> {
    Ok(u64::from_le_bytes(
      core::ptr::from_ref(self).expose_provenance().to_le_bytes(),
    ))
  }
}

impl Frame {
  pub fn identity_address(&self) -> Result<u64, Never> {
    self.0.physical()
  }

  pub fn pixels(&self) -> Result<&[u32; 1024], Never> {
    Ok(&self.0.0)
  }

  pub(super) fn scrub(&mut self) -> Result<(), Never> {
    clear_frame(&mut self.0.0)
  }
}

impl Reserved {
  pub(super) fn load_document(&mut self, document: Document) -> Result<(), PagingError> {
    let Ok(address) = self.document.physical();
    self.document.0 = document;
    let checked_value = user_leaf(address, UserPage::Data(FrameAccess::Read))?;
    self.renderer.leaves.0.document = checked_value;

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the reserved physical address becomes a Rust pointer only when its ownership is borrowed"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the exclusive identity-mapped frames are initialized before creating a typed Reserved borrow"
    )
  )]
  pub unsafe fn take(frames: &mut Frames<'_>) -> Result<&'static mut Self, MemoryError> {
    let address = frames.take(RESERVED_PAGES)?;
    let at = usize::try_from(address).map_err(|_too_wide| MemoryError::Overflow)?;
    let memory = core::ptr::with_exposed_provenance_mut::<Self>(at);

    // SAFETY: the caller owns identity-mapped conventional RAM. The cursor has removed these page-aligned frames from the range available to the heap. Zero is valid for every member of Reserved.
    unsafe {
      memory.cast::<u8>().write_bytes(0, size_of::<Self>());
      Ok(&mut *memory)
    }
  }

  pub fn trap_stack(&self) -> Result<Address, MemoryError> {
    let offset = TRAP_STACK_BYTES
      .checked_sub(super::RETURN_CONTEXT_BYTES)
      .ok_or(MemoryError::Overflow)?;
    let Ok(base) = self.trap_stack.physical();
    let stack = base.checked_add(offset).ok_or(MemoryError::Overflow)?;

    Ok(Address(stack))
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the address in CR3 becomes a Rust pointer at the firmware page-table adapter"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "this reads the active CR3 and copies entries from its firmware-owned identity-mapped page"
    )
  )]
  pub fn prepare(&mut self) -> Result<(), PagingError> {
    enable_nx()?;

    let kernel: u64;

    // SAFETY: the boot code is running in kernel mode on the BSP.
    unsafe { asm!("mov {}, cr3", out(reg) kernel, options(nomem, nostack, preserves_flags)) };

    let root = usize::from_le_bytes((kernel & ADDRESS).to_le_bytes());
    let source = core::ptr::with_exposed_provenance::<u64>(root);

    for (index, (renderer, browser)) in self
      .renderer
      .root
      .0
      .iter_mut()
      .zip(&mut self.browser.root.0)
      .enumerate()
    {
      // SAFETY: CR3 names the identity-mapped firmware root, a page of 512 entries. The CPU may update accessed bits while it is active.
      let inherited = unsafe { source.add(index).read_volatile() };
      let Ok(supervisor) = supervisor_entry(inherited);

      *renderer = supervisor;
      *browser = supervisor;
    }

    self.renderer.map_user_pages()?;
    self.browser.map_user_pages()
  }
}

impl AddressSpace {
  fn map_user_pages(&mut self) -> Result<(), PagingError> {
    let Ok(pointer_at) = self.directory_pointer.physical();
    let Ok(directory_at) = self.directory.physical();
    let Ok(leaves_at) = self.leaves.physical();
    let root = self
      .root
      .0
      .get_mut(128)
      .ok_or(PagingError::UserSlotOccupied)?;

    let [pointer, ..] = &mut self.directory_pointer.0;
    let [directory_entry, ..] = &mut self.directory.0;
    let planned_root = user_branch(NewBranch {
      previous: *root,
      table: pointer_at,
    })?;
    let planned_pointer = user_branch(NewBranch {
      previous: *pointer,
      table: directory_at,
    })?;
    let planned_directory = user_branch(NewBranch {
      previous: *directory_entry,
      table: leaves_at,
    })?;
    let Ok(code) = self.code.physical();
    let Ok(stack) = self.stack.physical();
    let planned_code = user_leaf(code, UserPage::Code)?;
    let planned_stack = user_leaf(stack, UserPage::Data(FrameAccess::Write))?;

    *root = planned_root;
    *pointer = planned_pointer;
    *directory_entry = planned_directory;
    self.leaves.0.code = planned_code;
    self.leaves.0.stack = planned_stack;

    Ok(())
  }

  pub(super) fn run(
    &mut self,
    program: &[u8],
    call: UserCall,
  ) -> Result<super::Trap, PagingError> {
    run_program(
      &mut self.root,
      &mut self.code,
      &mut self.stack,
      program,
      call,
    )
  }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the assembly enters the selected bounded user program after installing its prepared address space"
  )
)]
fn run_program(
  root: &mut Table,
  code: &mut Page<[u8; 4096]>,
  stack: &mut Page<[u8; 4096]>,
  program: &[u8],
  call: UserCall,
) -> Result<super::Trap, PagingError> {
  code.0.fill(0xcc);
  let head = code
    .0
    .get_mut(..program.len())
    .ok_or(PagingError::ProgramTooLarge)?;

  for (to, from) in head.iter_mut().zip(program) {
    *to = *from;
  }

  stack.0.fill(0);
  let Ok(root_2) = root.physical();

  // SAFETY: the physical pages and trap stack belong to the fixed boot reservation. No Rust access to the frame overlaps these fixed user programs. The assembly switches CR3 with global translations and PCID disabled, and restores it before returning.
  let trap = unsafe {
    super::console_enter_user(root_2, CODE_ADDRESS, STACK_TOP, call.stack, call.argument)
  };

  Ok(trap)
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the paging adapter reads CR4 and sets EFER.NXE after checking CPU support"
  )
)]
fn enable_nx() -> Result<(), PagingError> {
  let controls: u64;

  // SAFETY: the BSP is already in long mode with a readable CR4.
  unsafe { asm!("mov {}, cr4", out(reg) controls, options(nomem, nostack, preserves_flags)) };

  let extended = __cpuid(0x8000_0000).eax >= 0x8000_0001;

  match ::core::primitive::bool::then_some(
    extended
      && __cpuid(0x8000_0001).edx & ::core::primitive::u32::wrapping_shl(1, 20_u32) != 0
      && controls & (FIVE_LEVELS | PCID) == 0,
    (),
  ) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(PagingError::Unsupported),
  }

  let low: u32;
  let high: u32;

  // SAFETY: long mode exposes EFER; CPUID confirmed that its NXE bit is supported.
  unsafe {
    asm!("rdmsr", in("ecx") EFER, out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags));
    asm!("wrmsr", in("ecx") EFER, in("eax") low | NXE, in("edx") high, options(nostack, preserves_flags));
  }

  Ok(())
}
