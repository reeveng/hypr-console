//! A size-sealed file shared with the compositor. Painters write pixels before
//! attaching a buffer, and capture clients read pixels after capture completes.
//! A Wayland shared-memory pool length must fit in a signed 32-bit value.

use std::fs::File;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::FileExt;

use console_core_never::Never;
use console_core_number_conversion::{checked_offset, index};
use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};

pub struct Shared {
  held: File,
  length: u64,
}

impl Shared {
  pub fn of(long: u64) -> io::Result<Self> {
    match long {
      0 => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidInput,
          "a shared frame is empty",
        ));
      }
      1.. => {}
    }

    checked_offset(long).map_err(|fault| io::Error::new(io::ErrorKind::InvalidInput, fault))?;

    let flags = MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING;
    let held = memfd_create("console-draw-surface", flags)?;
    ftruncate(&held, long)?;
    fcntl_add_seals(&held, SealFlags::SHRINK | SealFlags::GROW)?;
    Ok(Self {
      held: File::from(held),
      length: long,
    })
  }

  pub fn descriptor(&self) -> Result<BorrowedFd<'_>, Never> {
    Ok(self.held.as_fd())
  }

  pub(crate) fn allocate_pixels(&self) -> io::Result<Vec<u8>> {
    match self.length {
      0..=2_147_483_647 => {}
      _ => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidInput,
          "pixel buffer exceeds the Wayland pool limit",
        ));
      }
    }

    let Ok(bytes) = index(self.length);
    let mut pixels = Vec::new();
    pixels
      .try_reserve_exact(bytes.min(2_147_483_647))
      .map_err(io::Error::other)?;
    pixels.resize(pixels.capacity().min(bytes), 0);

    Ok(pixels)
  }

  pub fn read_pixels_at(&self, pixels: &mut [u8], offset: u64) -> io::Result<()> {
    self.held.read_exact_at(pixels, offset)
  }

  pub(crate) fn write_pixels(&self, pixels: &[u8]) -> io::Result<()> {
    let Ok(length) = index(self.length);

    match (pixels.len() == length).then_some(()) {
      Some(()) => self.held.write_all_at(pixels, 0),
      None => Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "the pixels must fill the shared frame",
      )),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn what_is_written_is_what_is_read_back() -> io::Result<()> {
    let shared = Shared::of(64)?;
    let mut pixels = [0; 64];
    shared.write_pixels(&[0xab; 64])?;
    shared.read_pixels_at(&mut pixels, 0)?;

    assert_eq!(pixels, [0xab; 64]);

    shared.write_pixels(&[0xcd; 64])?;
    shared.read_pixels_at(&mut pixels, 0)?;

    assert_eq!(pixels, [0xcd; 64]);

    Ok(())
  }

  #[test]
  fn the_buffer_is_as_long_as_it_was_asked_for() -> io::Result<()> {
    let shared = Shared::of(4096)?;
    let Ok(held) = shared.descriptor();

    let fstat = rustix::fs::fstat(held)?;
    assert_eq!(fstat.st_size, 4096);

    Ok(())
  }

  #[test]
  fn a_buffer_must_fit_in_a_nonempty_rust_slice() {
    for length in [0, u64::MAX] {
      assert!(
        matches!(Shared::of(length), Err(fault) if fault.kind() == io::ErrorKind::InvalidInput)
      );
    }
  }
}
