//! The console is opened without being taken.
//!
//! systemd starts the login window as the leader of a session with no
//! terminal, and a session leader with no terminal that opens one is given
//! it. The desktop then asks for the same terminal from a session of its own,
//! and the kernel refuses a terminal that is already somebody's: the first
//! boot to run the login window ended at a text login that way, three times
//! over, with `Operation not permitted`.
//!
//! So this is that session, made for real. The test runs itself again as a
//! child, the child makes itself a session leader with no terminal, opens a
//! pseudo-terminal through `system::console` and asks the kernel whether the
//! terminal belongs to a session now. A plain open is asked the same, so the
//! test is seen telling a taken terminal from a free one.

use std::error::Error;
use std::ffi::{c_int, c_ulong};
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::process::Command;

use console_core_never::Never;
use console_login_window::system;

const OPENING: &str = "CONSOLE_TEST_OPENING";
const TERMINAL: &str = "CONSOLE_TEST_TERMINAL";
const THE_WINDOW: &str = "the login window";
const PLAINLY: &str = "plainly";

const TIOCGPTN: c_ulong = 0x8004_5430;
const TIOCSPTLCK: c_ulong = 0x4004_5431;
const TIOCGSID: c_ulong = 0x5429;

unsafe extern "C" {
    fn setsid() -> c_int;

    fn ioctl(descriptor: c_int, request: c_ulong, ...) -> c_int;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owned {
    BySomebody,
    ByNobody,
}

#[cfg_attr(
    dylint_lib = "explicit040_no_torn_write",
    allow(
        explicit040_no_torn_write,
        reason = "the multiplexer is a device that hands out terminals, and opening it to read and write is how one is asked for; nothing is written to a file"
    )
)]
fn pseudo() -> Result<(File, PathBuf), io::Error> {
    let master = OpenOptions::new().read(true).write(true).open("/dev/ptmx")?;
    let mut number: c_int = 0;
    let mut locked: c_int = 0;

    // SAFETY: a descriptor this frame owns, and a pointer to an int it owns.
    let unlocked = unsafe { ioctl(master.as_raw_fd(), TIOCSPTLCK, &mut locked) };

    assert_eq!(unlocked, 0, "the pseudo-terminal would not unlock: {}", io::Error::last_os_error());

    // SAFETY: as above.
    let named = unsafe { ioctl(master.as_raw_fd(), TIOCGPTN, &mut number) };

    assert_eq!(named, 0, "the pseudo-terminal has no number: {}", io::Error::last_os_error());

    Ok((master, PathBuf::from(format!("/dev/pts/{number}"))))
}

fn owned(terminal: &File) -> Result<Owned, Never> {
    let mut session: c_int = 0;

    // SAFETY: a descriptor the caller owns, and a pointer to an int this frame owns.
    let asked = unsafe { ioctl(terminal.as_raw_fd(), TIOCGSID, &mut session) };

    Ok(match asked {
        0 => Owned::BySomebody,
        _ => Owned::ByNobody,
    })
}

fn opened_in_a_session_of_its_own(how: &str) -> Result<Owned, io::Error> {
    let (_master, at) = pseudo()?;
    let this_test = std::env::current_exe()?;
    let child = Command::new(this_test)
        .args(["--exact", "a_session_leader_that_opens_the_console", "--test-threads=1", "--nocapture"])
        .env(OPENING, how)
        .env(TERMINAL, &at)
        .status()?;

    Ok(match child.code() {
        Some(0) => Owned::ByNobody,
        Some(_) | None => Owned::BySomebody,
    })
}

#[test]
#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "this test is its own child, and the environment it is started with is the only thing the parent can hand it"
    )
)]
#[cfg_attr(
    dylint_lib = "explicit040_no_torn_write",
    allow(
        explicit040_no_torn_write,
        reason = "the terminal is a device being opened so the kernel can be asked who owns it; nothing is written to it"
    )
)]
fn a_session_leader_that_opens_the_console() -> Result<(), Box<dyn Error>> {
    let how = match std::env::var(OPENING) {
        Ok(how) => how,
        Err(_run_by_cargo_rather_than_by_a_test) => return Ok(()),
    };
    let terminal = std::env::var(TERMINAL)?;
    let at = PathBuf::from(terminal);

    // SAFETY: a call with no arguments, in a process nothing else shares.
    let led = unsafe { setsid() };

    assert!(led > 0, "no session of its own: {}", io::Error::last_os_error());

    let terminal = match how.as_str() {
        THE_WINDOW => system::console(&at)?,
        _ => OpenOptions::new().read(true).write(true).open(&at)?,
    };
    let Ok(owned) = owned(&terminal);

    match owned {
        Owned::ByNobody => Ok(()),
        Owned::BySomebody => Err(Box::from("the terminal was taken")),
    }
}

#[test]
fn the_login_window_opens_the_console_without_taking_it() -> Result<(), io::Error> {
    let window = opened_in_a_session_of_its_own(THE_WINDOW)?;

    assert_eq!(
        window,
        Owned::ByNobody,
        "the login window made the console its own terminal, and the desktop will be refused it"
    );
    Ok(())
}

#[test]
fn a_plain_open_would_have_taken_it() -> Result<(), io::Error> {
    let plain = opened_in_a_session_of_its_own(PLAINLY)?;

    assert_eq!(
        plain,
        Owned::BySomebody,
        "a plain open no longer takes the terminal, so the test beside this one proves nothing"
    );
    Ok(())
}
