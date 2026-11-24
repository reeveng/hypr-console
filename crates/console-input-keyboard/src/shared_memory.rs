//! A sealed keymap descriptor is passed to `zwp_virtual_keyboard_v1_keymap`.
//! The compositor maps the file and expects a NUL after the xkb source.

use std::fs::File;
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;

use console_core_number_conversion::fitted;
use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};

pub fn keymap_file(text: &str) -> io::Result<(OwnedFd, u64)> {
  let Ok(written) = fitted::<_, u64>(text.len());
  let long = written.saturating_add(1);
  let held = memfd_create(
    "console-keyboard-keymap",
    MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
  )?;

  ftruncate(&held, long)?;

  let file = File::from(held);

  file.write_all_at(text.as_bytes(), 0)?;
  file.write_all_at(&[0], written)?;

  let seals = SealFlags::SEAL | SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE;

  fcntl_add_seals(&file, seals)?;

  Ok((OwnedFd::from(file), long))
}

#[cfg(test)]
mod tests {
  use console_wayland::memory::Shared;
  use rustix::io::Errno;

  use super::*;

  type Failure = Box<dyn std::error::Error>;

  #[test]
  fn a_keymap_is_written_and_reads_back_with_its_terminator() -> Result<(), Failure> {
    let text = "xkb_keymap { }";
    let (held, long) = keymap_file(text)?;
    let file = File::from(held);
    let mut bytes = [0; 15];

    file.read_exact_at(&mut bytes, 0)?;

    assert_eq!(long, 15);
    assert_eq!(
      bytes.strip_suffix(&[0]),
      Some(text.as_bytes()),
      "the compositor reads to the length and wants a nul"
    );

    Ok(())
  }

  #[test]
  fn a_keymap_that_has_been_handed_over_cannot_be_rewritten() -> Result<(), Failure> {
    let (held, _long) = keymap_file("xkb_keymap { }")?;
    let file = File::from(held);
    let refused = file.write_all_at(b"bad", 0);

    assert!(
      matches!(refused, Err(fault) if fault.kind() == io::ErrorKind::PermissionDenied),
      "a sealed keymap was writable again"
    );

    Ok(())
  }

  #[test]
  fn a_frame_cannot_be_grown_or_shrunk() -> Result<(), Failure> {
    let shared = Shared::of(64)?;
    let Ok(held) = shared.descriptor();
    let shrunk = ftruncate(held, 32);
    let grown = ftruncate(held, 128);

    assert_eq!(
      shrunk,
      Err(Errno::PERM),
      "the frame shrank while the compositor was reading it"
    );
    assert_eq!(grown, Err(Errno::PERM));

    Ok(())
  }
}
