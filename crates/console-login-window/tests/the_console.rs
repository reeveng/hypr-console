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

#![forbid(unsafe_code)]

use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::process::Command;

use console_core_never::Never;
use console_login_window::system;
use rustix::{pty, termios};

const OPENING: &str = "CONSOLE_TEST_OPENING";
const TERMINAL: &str = "CONSOLE_TEST_TERMINAL";
const THE_WINDOW: &str = "the login window";
const PLAINLY: &str = "plainly";

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
  let master = OpenOptions::new()
    .read(true)
    .write(true)
    .open("/dev/ptmx")?;
  pty::unlockpt(&master)?;
  let name = pty::ptsname(&master, Vec::new())?;
  let name_2 = std::ffi::OsString::from_vec(name.into_bytes());

  Ok((master, PathBuf::from(name_2)))
}

fn owned(terminal: &File) -> Result<Owned, Never> {
  Ok(match termios::tcgetsid(terminal) {
    Ok(_session) => Owned::BySomebody,
    Err(_no_session) => Owned::ByNobody,
  })
}

fn opened_in_a_session_of_its_own(how: &str) -> Result<Owned, io::Error> {
  let (_master, at) = pseudo()?;
  let this_test = std::env::current_exe()?;
  let child = Command::new(this_test)
    .args([
      "--exact",
      "a_session_leader_that_opens_the_console",
      "--test-threads=1",
      "--nocapture",
    ])
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

  let _new_session = rustix::process::setsid()?;

  let terminal_2 = match how.as_str() {
    THE_WINDOW => system::console(&at)?,
    _ => OpenOptions::new().read(true).write(true).open(&at)?,
  };
  let Ok(owned) = owned(&terminal_2);

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
