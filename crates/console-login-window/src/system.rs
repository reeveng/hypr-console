//! The C library calls greetd reached through `nix` and `libc`, declared here
//! against the C library std already links, and only the ones it made.
//!
//! Who a person is comes from `getpwnam_r`, so a name resolves the way every
//! other login on the machine resolves it, NSS and all. What a session is
//! started as is set between fork and exec, in the order greetd found it has
//! to be: a session of its own, the console as the controlling terminal, the
//! person's groups, then the group, then the user -- the user last, because
//! after it nothing else may be changed.

use std::ffi::{CStr, CString, c_char, c_int, c_ulong};
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_never::Never;
use console_core_number_conversion::index;

#[repr(C)]
struct Passwd {
    name: *mut c_char,
    password: *mut c_char,
    user: u32,
    group: u32,
    gecos: *mut c_char,
    home: *mut c_char,
    shell: *mut c_char,
}

unsafe extern "C" {
    #[cfg_attr(dylint_lib = "explicit051_no_machine_width", allow(explicit051_no_machine_width, reason = "the buffer's length is C's `size_t`, which is the machine's width by the C ABI and not by choice here"))]
    fn getpwnam_r(
        name: *const c_char,
        entry: *mut Passwd,
        buffer: *mut c_char,
        long: usize,
        found: *mut *mut Passwd,
    ) -> c_int;

    fn initgroups(name: *const c_char, group: u32) -> c_int;

    fn setgid(group: u32) -> c_int;

    fn setuid(user: u32) -> c_int;

    fn setsid() -> c_int;

    fn ioctl(descriptor: c_int, request: c_ulong, ...) -> c_int;
}

const NOT_OURS: c_int = 0o400;

const TIOCSCTTY: c_ulong = 0x540E;
const KDSETMODE: c_ulong = 0x4B3A;
const KD_TEXT: c_ulong = 0x00;
const VT_ACTIVATE: c_ulong = 0x5606;
const VT_WAITACTIVE: c_ulong = 0x5607;

const ROOM: u32 = 16384;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub name: String,
    pub user: u32,
    pub group: u32,
    pub home: PathBuf,
    pub shell: PathBuf,
}

pub fn person(name: &str) -> io::Result<Option<Person>> {
    let asked = match CString::new(name) {
        Ok(asked) => asked,
        Err(_) => return Ok(None),
    };
    let mut entry = Passwd {
        name: std::ptr::null_mut(),
        password: std::ptr::null_mut(),
        user: 0,
        group: 0,
        gecos: std::ptr::null_mut(),
        home: std::ptr::null_mut(),
        shell: std::ptr::null_mut(),
    };
    let Ok(room) = index(ROOM);
    let mut buffer = vec![0_u8; room];
    let mut found: *mut Passwd = std::ptr::null_mut();

    // SAFETY: every pointer is to memory this frame owns for the whole call,
    // and the length handed in is the buffer's own.
    let code = unsafe {
        getpwnam_r(asked.as_ptr(), &mut entry, buffer.as_mut_ptr().cast::<c_char>(), buffer.len(), &mut found)
    };

    match (code, found.is_null()) {
        (0, true) => Ok(None),
        (0, false) => {
            let Ok(home) = owned(entry.home);
            let Ok(shell) = owned(entry.shell);

            Ok(Some(Person { name: name.to_string(), user: entry.user, group: entry.group, home: PathBuf::from(home), shell: PathBuf::from(shell) }))
        }
        (code, _) => Err(io::Error::from_raw_os_error(code)),
    }
}

fn owned(text: *const c_char) -> Result<String, Never> {
    Ok(match text.is_null() {
        true => String::new(),
        false => {
            // SAFETY: a NUL-ended string inside the buffer `person` still holds.
            unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
        }
    })
}

fn checked(code: c_int) -> io::Result<()> {
    match code {
        -1 => Err(io::Error::last_os_error()),
        _ => Ok(()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terminal {
    Controlling,
    None,
}

pub fn started_as(command: &mut Command, person: &Person, terminal: Terminal) -> Result<(), Never> {
    let name = match CString::new(person.name.as_str()) {
        Ok(name) => name,
        Err(_) => CString::default(),
    };
    let (user, group) = (person.user, person.group);

    // SAFETY: what runs between fork and exec is system calls on values moved
    // into the closure before the fork, and no allocation or lock.
    unsafe {
        command.pre_exec(move || {
            checked(setsid())?;

            match terminal {
                Terminal::Controlling => checked(ioctl(0, TIOCSCTTY, 0_u64))?,
                Terminal::None => {}
            }

            checked(initgroups(name.as_ptr(), group))?;
            checked(setgid(group))?;
            checked(setuid(user))?;

            Ok(())
        })
    };

    Ok(())
}

#[cfg_attr(
    dylint_lib = "explicit040_no_torn_write",
    allow(
        explicit040_no_torn_write,
        reason = "the console is a device, not a file: writing it is putting letters on a screen, and there is nothing beside it to commit"
    )
)]
pub fn console(at: &Path) -> io::Result<File> {
    std::fs::OpenOptions::new().read(true).write(true).custom_flags(NOT_OURS).open(at)
}

pub fn shown(console: &File, number: u32) -> io::Result<()> {
    let descriptor = console.as_raw_fd();
    let number = c_ulong::from(number);

    // SAFETY: a console this process opened, and requests that take a number.
    let set = unsafe { ioctl(descriptor, KDSETMODE, KD_TEXT) };

    checked(set)?;

    // SAFETY: as above.
    let activated = unsafe { ioctl(descriptor, VT_ACTIVATE, number) };

    checked(activated)?;

    // SAFETY: as above.
    let waited = unsafe { ioctl(descriptor, VT_WAITACTIVE, number) };

    checked(waited)?;

    Ok(())
}
