//! The bytes the compositor reads while we are writing them.
//!
//! A shm buffer is one mapping shared by two processes and there is no handing
//! it over: what is drawn is on the screen the moment the surface is committed,
//! and the compositor may be reading the same page in the middle of it. That is
//! what the sealing is for. A descriptor that can still be shrunk is one the
//! compositor has to defend itself against by checking every access, and a
//! compositor that declines to defend itself against it maps a hole instead --
//! so the seals go on before anybody is shown the descriptor, and after that the
//! size is a fact rather than a promise.

use console_core_never::Never;
use console_core_number_conversion::fitted;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

const ADD_SEALS: i32 = 1033;

const SHRINK_AND_GROW: i32 = 0x0002 | 0x0004;

pub struct Shared {
    held: OwnedFd,
    at: *mut libc::c_void,
    long: usize,
}

impl Shared {
    pub fn of(long: usize) -> Result<Shared, io::Error> {
        let name = c"console-draw-surface";
        let Ok(flags) = fitted::<u32, libc::c_uint>(libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING);

        // SAFETY: a name that lives for the program and a flag word libc spells.
        let made = unsafe { libc::memfd_create(name.as_ptr(), flags) };

        match made < 0 {
            true => return Err(io::Error::last_os_error()),
            false => {},
        }

        // SAFETY: memfd_create answered with a descriptor nothing else holds.
        let held = unsafe { OwnedFd::from_raw_fd(made) };
        let Ok(wanted) = fitted::<usize, libc::off_t>(long);

        // SAFETY: a descriptor this function owns and a length it was given.
        let cut = unsafe { libc::ftruncate(held.as_raw_fd(), wanted) };

        match cut < 0 {
            true => return Err(io::Error::last_os_error()),
            false => {},
        }

        // SAFETY: the same descriptor, before anybody else has seen it.
        unsafe { libc::fcntl(held.as_raw_fd(), ADD_SEALS, SHRINK_AND_GROW) };

        // SAFETY: a memfd of exactly `long` bytes, cut above.
        let at = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                long,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                held.as_raw_fd(),
                0,
            )
        };

        match std::ptr::eq(at, libc::MAP_FAILED) {
            true => Err(io::Error::last_os_error()),
            false => Ok(Shared { held, at, long }),
        }
    }

    pub fn held(&self) -> Result<&OwnedFd, Never> {
        Ok(&self.held)
    }

    pub fn long(&self) -> Result<usize, Never> {
        Ok(self.long)
    }

    pub fn pixels(&mut self) -> Result<&mut [u8], Never> {
        // SAFETY: `long` bytes mapped writable by `of`, and never unmapped
        // while this borrow is alive because `Drop` takes `self` by value.
        Ok(unsafe { std::slice::from_raw_parts_mut(self.at.cast::<u8>(), self.long) })
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        // SAFETY: the mapping this struct made, unmapped exactly once.
        unsafe { libc::munmap(self.at, self.long) };
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
