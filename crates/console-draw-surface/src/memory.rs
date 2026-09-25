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
use console_core_number_conversion::index;
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
    pub fn of(long: u64) -> Result<Shared, io::Error> {
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

        Ok(Shared { held, at, long })
    }

    pub fn held(&self) -> Result<&OwnedFd, Never> {
        Ok(&self.held)
    }

    pub fn long(&self) -> Result<u64, Never> {
        Ok(self.long)
    }

    pub fn pixels(&mut self) -> Result<&mut [u8], Never> {
        let Ok(span) = index(self.long);

        // SAFETY: `long` bytes mapped writable by `of`, and never unmapped
        // while this borrow is alive because `Drop` takes `self` by value.
        Ok(unsafe { std::slice::from_raw_parts_mut(self.at.cast::<u8>(), span) })
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        let Ok(span) = index(self.long);

        // SAFETY: the mapping this struct made, unmapped exactly once.
        let _ = unsafe { munmap(self.at, span) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_written_is_what_is_read_back() {
        let mut shared = match Shared::of(64) {
            Ok(shared) => shared,
            Err(why) => panic!("no shared memory for a test: {why}"),
        };
        let Ok(pixels) = shared.pixels();

        pixels.fill(0xab);

        let Ok(again) = shared.pixels();

        assert!(again.iter().all(|byte| *byte == 0xab));
    }

    #[test]
    fn the_mapping_is_as_long_as_it_was_asked_for() {
        let mut shared = match Shared::of(4096) {
            Ok(shared) => shared,
            Err(why) => panic!("no shared memory for a test: {why}"),
        };
        let Ok(pixels) = shared.pixels();

        assert_eq!(pixels.len(), 4096);
    }
}
