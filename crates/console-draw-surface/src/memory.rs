//! The bytes the compositor reads while we are writing them.
//!
//! A shm buffer is one mapping shared by two processes and there is no handing
//! it over: what is drawn is on the screen the moment the surface is committed,
//! and the compositor may be reading the same page in the middle of it. That is
//! what the sealing is for. A descriptor that can still be shrunk is one the
//! compositor has to defend itself against by checking every access, and a
//! compositor that declines to defend itself against it maps a hole instead --
//! so the seals go on before anyone is shown the descriptor, and after that the
//! size is a fact rather than a promise.

use console_core_never::Never;
use console_core_number_conversion::{checked_offset, index};
use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};
use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
use std::ffi::c_void;
use std::io;
use std::os::fd::OwnedFd;

pub struct Shared {
  held: OwnedFd,
  at: *mut c_void,
  long: u64,
}

impl Shared {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "this constructor owns the sealed memfd and the mapping returned by the kernel"
    )
  )]
  pub fn of(long: u64) -> Result<Shared, io::Error> {
    let fits = match checked_offset(long) {
      Ok(addressable) => addressable > 0,
      Err(_outside_slice_range) => false,
    };

    match ::core::primitive::bool::then_some(fits, ()) {
      ::core::option::Option::None => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidInput,
          "a shared frame needs a nonempty addressable byte span",
        ));
      }
      ::core::option::Option::Some(()) => {}
    }

    let flags = MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING;
    let held = memfd_create("console-draw-surface", flags)?;

    ftruncate(&held, long)?;
    fcntl_add_seals(&held, SealFlags::SHRINK | SealFlags::GROW)?;

    let Ok(span) = index(long);

    // SAFETY: a memfd of exactly `long` bytes, cut and sealed above, at an
    // address the kernel picks and `Drop` gives back exactly once.
    let mapped = unsafe {
      mmap(
        std::ptr::null_mut(),
        span,
        ProtFlags::READ | ProtFlags::WRITE,
        MapFlags::SHARED,
        &held,
        0,
      )
    };

    let at = mapped?;
    let shared = Shared { held, at, long };

    match ::core::primitive::bool::then_some(at.is_null(), ()) {
      ::core::option::Option::Some(()) => Err(io::Error::other(
        "the kernel chose the null address for a shared frame",
      )),
      ::core::option::Option::None => Ok(shared),
    }
  }

  pub fn descriptor(&self) -> Result<&OwnedFd, Never> {
    Ok(&self.held)
  }

  pub fn long(&self) -> Result<u64, Never> {
    Ok(self.long)
  }

  #[cfg_attr(
    dylint_lib = "explicit096_no_unchecked_memory",
    expect(
      explicit096_no_unchecked_memory,
      reason = "mmap supplied a live writable mapping of the stored length and the memfd is sealed against resizing"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the exclusive borrow ties the pixel slice to the live writable mapping"
    )
  )]
  pub fn pixels(&mut self) -> Result<&mut [u8], Never> {
    let Ok(span) = index(self.long);

    // SAFETY: `long` bytes mapped writable by `of`, and never unmapped
    // while this borrow is alive because `Drop` takes `self` by value.
    Ok(unsafe { std::slice::from_raw_parts_mut(self.at.cast::<u8>(), span) })
  }
}

impl Drop for Shared {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "drop releases the mapping using the address and length supplied when it was made"
    )
  )]
  fn drop(&mut self) {
    let Ok(span) = index(self.long);

    // SAFETY: the mapping this struct made, unmapped exactly once.
    let _unmap_failure_cannot_be_retried = unsafe { munmap(self.at, span) };
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;

  #[test]
  fn what_is_written_is_what_is_read_back() -> Result<(), Box<dyn Error>> {
    let mut shared =
      Shared::of(64).map_err(|why| format!("no shared memory for a test: {why}"))?;
    let Ok(pixels) = shared.pixels();

    pixels.fill(0xab);

    let Ok(again) = shared.pixels();

    assert!(again.iter().all(|byte| *byte == 0xab));

    Ok(())
  }

  #[test]
  fn the_mapping_is_as_long_as_it_was_asked_for() -> Result<(), Box<dyn Error>> {
    let mut shared =
      Shared::of(4096).map_err(|why| format!("no shared memory for a test: {why}"))?;
    let Ok(pixels) = shared.pixels();

    assert_eq!(pixels.len(), 4096);

    Ok(())
  }

  #[test]
  fn a_mapping_must_fit_in_a_nonempty_rust_slice() {
    for length in [0, u64::MAX] {
      assert!(
        matches!(Shared::of(length), Err(fault) if fault.kind() == io::ErrorKind::InvalidInput)
      );
    }
  }
}
