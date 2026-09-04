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


use console_number::fitted;
use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

/// The keymap, in a file the compositor can read once and never again.
///
/// The virtual-keyboard protocol takes a descriptor and a length rather than a
/// string, because the keymap is large -- a full xkb keymap is tens of
/// kilobytes -- and it is handed over by mapping rather than by copying. What
/// goes in it is the text and a nul, and the length includes the nul: that is
/// what every compositor expects, and one that reads to the length and finds
/// no terminator will refuse the whole keymap.
///
/// Written first and sealed after. The seal is the promise the compositor is
/// relying on -- that nothing rewrites the keymap under it -- and it can only
/// be made once the keymap is in there.
pub fn keymap_file(text: &str) -> io::Result<(OwnedFd, usize)> {
    let long = text.len() + 1;
    let held = made("virtual-keyboard-keymap", long)?;

    {
        let mut mapped = Mapped::of(&held, long)?;
        let into = mapped.pixels();
        into[..text.len()].copy_from_slice(text.as_bytes());
        into[text.len()] = 0;
    }

    // Shut for good: shrinking, growing and writing all sealed, and the seal
    // itself sealed, so what the compositor reads is what was written above.
    //
    // Sealing is asked for twice and both halves are easy to leave out. The
    // file has to be made with `MFD_ALLOW_SEALING` -- `made` does that -- and
    // the seals are added with `F_ADD_SEALS`, which is 1033 and not one of the
    // numbers a file-descriptor call usually takes. This was written as
    // `fcntl(fd, 4, 0xf)` once, and 4 is `F_SETFL`: it set file status flags,
    // returned success, and sealed nothing at all.
    const F_ADD_SEALS: i32 = 1033;
    const EVERYTHING: i32 = 0x0001 | 0x0002 | 0x0004 | 0x0008;

    // SAFETY: a flag word against a descriptor this owns.
    if unsafe { libc::fcntl(held.as_raw_fd(), F_ADD_SEALS, EVERYTHING) } < 0 {
        // A kernel that will not seal is not a reason to refuse to type. The
        // seal says the compositor cannot rewrite the keymap under us, and
        // nothing on this machine was going to.
        let why = io::Error::last_os_error();

        if why.raw_os_error() != Some(libc::EINVAL) && why.raw_os_error() != Some(libc::EPERM) {
            return Err(why);
        }
    }

    Ok((held, long))
}

/// A buffer the keyboard draws into and the compositor reads out of.
///
/// The same anonymous file as above and sealed differently, which is the whole
/// difference between the two things a keyboard hands a compositor. A keymap
/// is written once and read once, so it is sealed shut on the way out and
/// nothing can touch it again. A frame is written on every press for as long
/// as the keyboard is up, by this process, into memory the compositor is
/// reading at the same time -- so it is sealed against growing and shrinking,
/// which is what would move the pixels out from under the compositor, and not
/// against writing, which is the point of it.
pub fn drawing_buffer(len: usize) -> io::Result<OwnedFd> {
    let held = made("virtual-keyboard-pixels", len)?;
    // SHRINK and GROW without WRITE. The seal can be refused by an old
    // kernel, and a buffer that is merely unsealed still draws.
    const F_ADD_SEALS: i32 = 1033;
    const SHRINK_AND_GROW: i32 = 0x0002 | 0x0004;

    // SAFETY: one call on a descriptor this function owns and is still
    // holding, with the argument `F_ADD_SEALS` takes. A refusal comes back as
    // a return value, which the comment above says why nothing reads.
    unsafe { libc::fcntl(held.as_raw_fd(), F_ADD_SEALS, SHRINK_AND_GROW) };

    Ok(held)
}

/// That buffer, in this process's own address space.
///
/// Wayland's shared memory is shared in the literal sense: the compositor
/// mmaps the same pages, so what is drawn here is what is on the screen
/// without anything being copied anywhere. This is the other end of that, and
/// it is unmapped when it goes, because a keyboard that is resized every time
/// the screen turns would otherwise leak a screenful per turn.
pub struct Mapped {
    at: *mut libc::c_void,
    long: usize,
}

impl Mapped {
    /// Map the whole of `len` bytes of `fd`, readable and writable.
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

    /// Map it to read, which is the only way to look at a keymap once it has
    /// been sealed: a sealed file refuses a writable mapping outright, and
    /// asking for one is how the seal is checked.
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

    /// The bytes, to read.
    pub fn bytes(&self) -> &[u8] {
        // SAFETY: `at` is a live mapping of `long` bytes.
        unsafe { std::slice::from_raw_parts(self.at.cast::<u8>(), self.long) }
    }

    /// The bytes, to draw into.
    pub fn pixels(&mut self) -> &mut [u8] {
        // SAFETY: `at` is a live mapping of `long` bytes, and this borrows it
        // for no longer than the mapping lives.
        unsafe { std::slice::from_raw_parts_mut(self.at.cast::<u8>(), self.long) }
    }
}

impl Drop for Mapped {
    fn drop(&mut self) {
        // SAFETY: unmapping exactly what was mapped, once.
        unsafe { libc::munmap(self.at, self.long) };
    }
}

/// An anonymous file of `len` bytes, backed by RAM and named only in /proc.
fn made(called: &str, len: usize) -> io::Result<OwnedFd> {
    let name = CString::new(called)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a name with a nul byte in it"))?;
    // SAFETY: a name that lives across the call, and a flag word.
    let raw = unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING) };

    if raw < 0 {
        return Err(io::Error::last_os_error());
    }

    // SAFETY: `raw` is a fresh descriptor this owns.
    let owned = unsafe { OwnedFd::from_raw_fd(raw) };

    // SAFETY: sizing the file this just made.
    if unsafe { libc::ftruncate(owned.as_raw_fd(), fitted::<usize, i64>(len)) } < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The keymap goes in and comes back out, terminated, which is the whole
    /// of what the compositor is promised.
    #[test]
    fn a_keymap_is_written_and_reads_back_with_its_terminator() {
        let text = "xkb_keymap { }";
        let (held, long) = keymap_file(text).expect("a keymap file");
        assert_eq!(long, text.len() + 1);
        let mapped = Mapped::reading(&held, long).expect("map it back");
        let got = mapped.bytes();
        assert_eq!(&got[..text.len()], text.as_bytes());
        assert_eq!(got[text.len()], 0, "the compositor reads to the length and wants a nul");
    }

    /// And cannot be written again. This is the seal, and the test is here
    /// because the seal was silently not being applied at all: the call went
    /// to the wrong `fcntl` command, returned success, and left the keymap
    /// writable for the life of the session.
    #[test]
    fn a_keymap_that_has_been_handed_over_cannot_be_rewritten() {
        let (held, long) = keymap_file("xkb_keymap { }").expect("a keymap file");
        let again = Mapped::of(&held, long);
        assert!(
            again.is_err(),
            "a sealed keymap mapped writable again: the seal is not being applied"
        );
    }

    /// A frame is the other shape: written on every press, for as long as the
    /// keyboard is up, into memory the compositor is reading at the same time.
    #[test]
    fn a_frame_can_be_drawn_into_more_than_once() {
        let held = drawing_buffer(64).expect("a frame");
        let mut mapped = Mapped::of(&held, 64).expect("map it");
        mapped.pixels()[0] = 1;
        mapped.pixels()[0] = 2;
        assert_eq!(mapped.pixels()[0], 2);
    }

    /// And cannot be resized under the compositor, which is the half of the
    /// seal a frame does keep.
    #[test]
    fn a_frame_cannot_be_grown_or_shrunk() {
        let held = drawing_buffer(64).expect("a frame");
        // SAFETY: a size against a descriptor this owns.
        let shrunk = unsafe { libc::ftruncate(held.as_raw_fd(), 32) };
        assert_eq!(shrunk, -1, "the frame shrank while the compositor was reading it");
    }
}
