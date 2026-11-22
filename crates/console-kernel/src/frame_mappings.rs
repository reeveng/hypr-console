//! Page-table entries for the prototype. The caller owns the reserved pages
//! and edits the table while its address space is inactive. Switching to that
//! address space reloads CR3.

use console_core_never::Never;
use console_kernel_handles::{FrameMapping, MappingAccess};

#[cfg(kani)]
#[path = "../proofs/frame_mappings.rs"]
mod proofs;

const PRESENT: u64 = 1;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const NO_EXECUTE: u64 = 1 << 63;
const PHYSICAL_ADDRESS: u64 = 0x000f_ffff_ffff_f000;

#[derive(Clone, Copy)]
pub enum FrameAccess {
  Read,
  Write,
}

pub enum UserPage {
  Code,
  Data(FrameAccess),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidFrame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserBranchError {
  Occupied,
  InvalidFrame,
}

pub struct FrameGrant(u64);

struct InstalledFrame<'table> {
  grant: FrameGrant,
  entries: &'table mut [u64; 2],
}

pub struct NewBranch {
  pub previous: u64,
  pub table: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Access {
  Allowed,
  Denied,
}

pub fn clear_frame(words: &mut [u32; 1024]) -> Result<(), Never> {
  *words = [0; 1024];

  Ok(())
}

pub(crate) fn user_access(entry: u64, access: FrameAccess) -> Result<Access, Never> {
  let required = match access {
    FrameAccess::Read => PRESENT | USER,
    FrameAccess::Write => PRESENT | USER | WRITABLE,
  };

  Ok(
    match ::core::primitive::bool::then_some(entry & required == required, ()) {
      ::core::option::Option::Some(()) => Access::Allowed,
      ::core::option::Option::None => Access::Denied,
    },
  )
}

fn physical_frame(frame: u64) -> Result<u64, InvalidFrame> {
  match ::core::primitive::bool::then_some(frame != 0 && frame & PHYSICAL_ADDRESS == frame, ()) {
    ::core::option::Option::Some(()) => Ok(frame),
    ::core::option::Option::None => Err(InvalidFrame),
  }
}

pub fn supervisor_entry(entry: u64) -> Result<u64, Never> {
  Ok(entry & !USER)
}

pub fn user_branch(branch: NewBranch) -> Result<u64, UserBranchError> {
  match branch.previous & PRESENT {
    0 => physical_frame(branch.table)
      .map(|address| address | PRESENT | WRITABLE | USER)
      .map_err(|_invalid| UserBranchError::InvalidFrame),
    _ => Err(UserBranchError::Occupied),
  }
}

pub fn user_leaf(frame: u64, page: UserPage) -> Result<u64, InvalidFrame> {
  let address = physical_frame(frame)?;
  let permissions = match page {
    UserPage::Code => 0,
    UserPage::Data(FrameAccess::Read) => NO_EXECUTE,
    UserPage::Data(FrameAccess::Write) => NO_EXECUTE | WRITABLE,
  };

  Ok(address | PRESENT | USER | permissions)
}

/// The two dedicated alias slots carry the authorized address and permissions during `run`.
/// They are both zero after `run` returns, including on a callback error.
/// If `run` activates the address space, it must deactivate it before returning.
pub fn with_frame_mapping<R>(
  authority: FrameMapping<'_>,
  entries: &mut [u64; 2],
  run: impl FnOnce() -> R,
) -> Result<R, InvalidFrame> {
  let access = match authority.access() {
    MappingAccess::Read => FrameAccess::Read,
    MappingAccess::Write => FrameAccess::Write,
  };
  let grant = FrameGrant::new(authority.address(), access)?;
  let Ok(result) = grant.with_installed(entries, run);

  Ok(result)
}

impl FrameGrant {
  pub fn new(frame: u64, access: FrameAccess) -> Result<Self, InvalidFrame> {
    user_leaf(frame, UserPage::Data(access)).map(Self)
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "exclusive aligned page-table entries must be written where the processor can observe them"
    )
  )]
  pub fn install(&self, entries: &mut [u64; 2]) -> Result<(), Never> {
    let [first, second] = entries;

    // SAFETY: the references lend distinct aligned u64 slots for both writes.
    // The page-table walker reads these slots outside Rust, so both stores are volatile.
    unsafe {
      core::ptr::from_mut(first).write_volatile(self.0);
      core::ptr::from_mut(second).write_volatile(self.0);
    }

    Ok(())
  }

  pub fn revoke(&mut self, entries: &mut [u64; 2]) -> Result<(), Never> {
    self.0 = 0;

    self.install(entries)
  }

  fn with_installed<R>(
    self,
    entries: &mut [u64; 2],
    run: impl FnOnce() -> R,
  ) -> Result<R, Never> {
    let Ok(_installed) = InstalledFrame::new(self, entries);

    Ok(run())
  }
}

impl<'table> InstalledFrame<'table> {
  fn new(grant: FrameGrant, entries: &'table mut [u64; 2]) -> Result<Self, Never> {
    let Ok(()) = grant.install(entries);

    Ok(Self { grant, entries })
  }
}

impl Drop for InstalledFrame<'_> {
  fn drop(&mut self) {
    let Ok(()) = self.grant.revoke(self.entries);
  }
}
