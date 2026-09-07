//! POSIX shims the C keyboard needed to talk to Wayland.
//!
//! `os-compatibility.c` carried the things that were missing or awkward in
//! POSIX circa the year wlroots was written: `epoll_create_cloexec`,
//! `socketpair_cloexec`, `mkostemp`, `strchrnul`. Most of those have made
//! it into Rust's std by now — `OwnedFd`, `UnixListener::pair`, `cvt` — and
//! the ones that haven't are short enough to write inline.
//!
//! What remains is the one thing the Wayland virtual-keyboard protocol
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
use console_core_number_conversion::fitted;
use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

pub fn keymap_file(text: &str) -> io::Result<(OwnedFd, usize)> {
    let long = text.len().saturating_add(1);
    let held = made("virtual-keyboard-keymap", long)?;

    {
        let mut mapped = Mapped::of(&held, long)?;
        let Ok(into) = mapped.pixels();

        let Some(words) = into.get_mut(..text.len()) else {
            return Err(io::Error::other("the keymap file came back too short to write"));
        };

        words.copy_from_slice(text.as_bytes());

        let Some(terminator) = into.get_mut(text.len()) else {
            return Err(io::Error::other("the keymap file has no room for its terminator"));
        };

        *terminator = 0;
    }

    const F_ADD_SEALS: i32 = 1033;
    const EVERYTHING: i32 = 0x0001 | 0x0002 | 0x0004 | 0x0008;

    // SAFETY: a flag word against a descriptor this owns.
    match unsafe { libc::fcntl(held.as_raw_fd(), F_ADD_SEALS, EVERYTHING) } < 0 {
        true => {
            let why = io::Error::last_os_error();

            match why.raw_os_error() != Some(libc::EINVAL)
                && why.raw_os_error() != Some(libc::EPERM)
            {
                true => return Err(why),
                false => {},
            }
        }
        false => {},
    }

    Ok((held, long))
}

pub fn drawing_buffer(len: usize) -> io::Result<OwnedFd> {
    let held = made("virtual-keyboard-pixels", len)?;
    const F_ADD_SEALS: i32 = 1033;
    const SHRINK_AND_GROW: i32 = 0x0002 | 0x0004;

    // SAFETY: one call on a descriptor this function owns and is still
    unsafe { libc::fcntl(held.as_raw_fd(), F_ADD_SEALS, SHRINK_AND_GROW) };

    Ok(held)
}

pub struct Mapped {
    at: *mut libc::c_void,
    long: usize,
}

impl Mapped {
    pub fn of(fd: &OwnedFd, len: usize) -> io::Result<Mapped> {
        // SAFETY: the fd is a memfd of at least `len` bytes, made above.
        let at = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd.as_raw_fd(),
                0,
            )
        };

        match at == libc::MAP_FAILED {
            true => Err(io::Error::last_os_error()),
            false => Ok(Mapped { at, long: len }),
        }
    }

    pub fn reading(fd: &OwnedFd, len: usize) -> io::Result<Mapped> {
        // SAFETY: as above, and read-only.
        let at = unsafe {
            libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ, libc::MAP_SHARED, fd.as_raw_fd(), 0)
        };

        match at == libc::MAP_FAILED {
            true => Err(io::Error::last_os_error()),
            false => Ok(Mapped { at, long: len }),
        }
    }

    pub fn bytes(&self) -> Result<&[u8], Never> {
        // SAFETY: `at` is a live mapping of `long` bytes.
        let seen = unsafe { std::slice::from_raw_parts(self.at.cast::<u8>(), self.long) };

        Ok(seen)
    }

    pub fn pixels(&mut self) -> Result<&mut [u8], Never> {
        // SAFETY: `at` is a live mapping of `long` bytes, and this borrows it
        let seen = unsafe { std::slice::from_raw_parts_mut(self.at.cast::<u8>(), self.long) };

        Ok(seen)
    }
}

impl Drop for Mapped {
    fn drop(&mut self) {
        // SAFETY: unmapping exactly what was mapped, once.
        unsafe { libc::munmap(self.at, self.long) };
    }
}

fn made(called: &str, len: usize) -> io::Result<OwnedFd> {
    let name = CString::new(called)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a name with a nul byte in it"))?;
    // SAFETY: a name that lives across the call, and a flag word.
    let raw = unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING) };

    match raw < 0 {
        true => return Err(io::Error::last_os_error()),
        false => {},
    }

    // SAFETY: `raw` is a fresh descriptor this owns.
    let owned = unsafe { OwnedFd::from_raw_fd(raw) };

    let Ok(long) = fitted::<usize, i64>(len);

    // SAFETY: sizing the file this just made.
    match unsafe { libc::ftruncate(owned.as_raw_fd(), long) } < 0 {
        true => return Err(io::Error::last_os_error()),
        false => {},
    }

    Ok(owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_keymap_is_written_and_reads_back_with_its_terminator() {
        let text = "xkb_keymap { }";
        let (held, long) = keymap_file(text).expect("a keymap file");
        assert_eq!(long, text.len() + 1);
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
        // SAFETY: a size against a descriptor this owns.
        let shrunk = unsafe { libc::ftruncate(held.as_raw_fd(), 32) };
        assert_eq!(shrunk, -1, "the frame shrank while the compositor was reading it");
    }
}
