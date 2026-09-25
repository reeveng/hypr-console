//! POSIX shims the C keyboard needed to talk to Wayland.
//!
//! `os-compatibility.c` carried the things that were missing or awkward in
//! POSIX circa the year wlroots was written: `epoll_create_cloexec`,
//! `socketpair_cloexec`, `mkostemp`, `strchrnul`. Most of those have made
//! it into Rust's std by now — `OwnedFd`, `UnixListener::pair`, `cvt` — and
//! the ones that haven't are short enough to write inline.
//!
//! What remains is the one thing the Wayland console-keyboard protocol
//! actually asks for: a file descriptor pointing at a buffer the compositor
//! can mmap. The C version built it via `shm_open` + `ftruncate` + `mmap`,
//! which is fine, but Linux has had `memfd_create` for a decade and it is
//! the right tool here — an anonymous file backed by RAM, no name on the
//! filesystem, sealed so the compositor cannot grow it.
//!
//! The keymap string is written into the memfd and the fd is passed to
//! `zwp_virtual_keyboard_v1_keymap`. The compositor reads it once and that
//! is the end of it.


use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use std::ffi::c_void;
use std::io;
use std::os::fd::OwnedFd;

use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};
use rustix::io::Errno;
use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};

pub fn keymap_file(text: &str) -> io::Result<(OwnedFd, u64)> {
    let Ok(written) = fitted::<_, u64>(text.len());
    let long = written.saturating_add(1);
    let held = made("console-keyboard-keymap", long)?;

    {
        let mut mapped = Mapped::of(&held, long)?;
        let Ok(into) = mapped.pixels();

        let words = match into.get_mut(..text.len()) {
            Some(words) => words,
            None => return Err(io::Error::other("the keymap file came back too short to write")),
        };

        words.copy_from_slice(text.as_bytes());

        let terminator = match into.get_mut(text.len()) {
            Some(terminator) => terminator,
            None => return Err(io::Error::other("the keymap file has no room for its terminator")),
        };

        *terminator = 0;
    }

    let everything = SealFlags::SEAL | SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE;

    match fcntl_add_seals(&held, everything) {
        Ok(()) => {},
        Err(Errno::INVAL | Errno::PERM) => {},
        Err(why) => return Err(io::Error::from(why)),
    }

    Ok((held, long))
}

pub fn drawing_buffer(length: u64) -> io::Result<OwnedFd> {
    let held = made("console-keyboard-pixels", length)?;

    match fcntl_add_seals(&held, SealFlags::SHRINK | SealFlags::GROW) {
        Ok(()) => {},
        Err(_a_frame_that_can_be_resized_is_still_a_frame) => {},
    }

    Ok(held)
}

pub struct Mapped {
    at: *mut c_void,
    long: u64,
}

impl Mapped {
    pub fn of(fd: &OwnedFd, length: u64) -> io::Result<Mapped> {
        Mapped::with(fd, length, ProtFlags::READ | ProtFlags::WRITE)
    }

    pub fn reading(fd: &OwnedFd, length: u64) -> io::Result<Mapped> {
        Mapped::with(fd, length, ProtFlags::READ)
    }

    fn with(fd: &OwnedFd, length: u64, protection: ProtFlags) -> io::Result<Mapped> {
        let Ok(mapping) = index(length);

        // SAFETY: the fd is a memfd of at least `len` bytes, and `protection` is what the caller may do with it.
        let at = unsafe { mmap(std::ptr::null_mut(), mapping, protection, MapFlags::SHARED, fd, 0) }?;

        Ok(Mapped { at, long: length })
    }

    pub fn bytes(&self) -> Result<&[u8], Never> {
        let Ok(long) = index(self.long);

        // SAFETY: `at` is a live mapping of `long` bytes.
        let seen = unsafe { std::slice::from_raw_parts(self.at.cast::<u8>(), long) };

        Ok(seen)
    }

    pub fn pixels(&mut self) -> Result<&mut [u8], Never> {
        let Ok(long) = index(self.long);

        // SAFETY: `at` is a live mapping of `long` bytes, and this borrows it
        let seen = unsafe { std::slice::from_raw_parts_mut(self.at.cast::<u8>(), long) };

        Ok(seen)
    }
}

impl Drop for Mapped {
    fn drop(&mut self) {
        let Ok(long) = index(self.long);

        // SAFETY: unmapping exactly what was mapped, once.
        let _ = unsafe { munmap(self.at, long) };
    }
}

fn made(called: &str, length: u64) -> io::Result<OwnedFd> {
    let owned = memfd_create(called, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)?;

    ftruncate(&owned, length)?;

    Ok(owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_keymap_is_written_and_reads_back_with_its_terminator() {
        let text = "xkb_keymap { }";
        let (held, long) = keymap_file(text).expect("a keymap file");
        assert_eq!(long, text.len() as u64 + 1);
        let mapped = Mapped::reading(&held, long).expect("map it back");
        let Ok(got) = mapped.bytes();
        assert_eq!(&got[..text.len()], text.as_bytes());
        assert_eq!(got[text.len()], 0, "the compositor reads to the length and wants a nul");
    }

    #[test]
    fn a_keymap_that_has_been_handed_over_cannot_be_rewritten() {
        let (held, long) = keymap_file("xkb_keymap { }").expect("a keymap file");
        let again = Mapped::of(&held, long);
        assert!(
            again.is_err(),
            "a sealed keymap mapped writable again: the seal is not being applied"
        );
    }

    #[test]
    fn a_frame_can_be_drawn_into_more_than_once() {
        let held = drawing_buffer(64).expect("a frame");
        let mut mapped = Mapped::of(&held, 64).expect("map it");
        let Ok(first) = mapped.pixels();
        first[0] = 1;
        let Ok(again) = mapped.pixels();
        again[0] = 2;
        let Ok(now) = mapped.pixels();
        assert_eq!(now[0], 2);
    }

    #[test]
    fn a_frame_cannot_be_grown_or_shrunk() {
        let held = drawing_buffer(64).expect("a frame");
        let shrunk = ftruncate(&held, 32);
        assert_eq!(shrunk, Err(Errno::PERM), "the frame shrank while the compositor was reading it");
    }
}
