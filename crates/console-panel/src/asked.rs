//! Being asked to stop, as a descriptor a loop is already watching.
//!
//! A picker hands the screen over by sending whoever holds it a SIGTERM and
//! then waiting for the lock. Answering that signal is not tidiness, it is how
//! two of these take turns, and a picker that sleeps through it holds the
//! screen shut against the next one.
//!
//! What a loop of our own wants is not a callback: it is a descriptor beside
//! the socket and the surface, so being asked to stop arrives the same way
//! everything else does and is answered in the same place. The handler writes
//! one byte to a pipe, which is the one thing a signal handler may do here, and
//! the loop reads it and puts the panel down.
//!
//! Blocking the signals and waiting for them on a thread was the other way, and
//! it is the wrong one twice over: a blocked mask is inherited by every child
//! the panel starts, and a thread that answers a signal cannot touch what the
//! loop is holding.
//!
//! The write end never closes, because the process it speaks for is what it
//! outlives. The read end is handed to whoever asked and the loop owns it.

use std::os::fd::OwnedFd;
use std::sync::OnceLock;

use console_core_never::Never;

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  allow(
    explicit044_no_ambient_value,
    reason = "the far end of that pipe, held for as long as the process is, because a closed write end would be a hangup on the loop rather than a signal"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "the process keeps the published signal pipe owned for the full lifetime of its signal handler"
  )
)]
static HELD: OnceLock<OwnedFd> = OnceLock::new();

pub fn signal_pipe() -> Result<Option<OwnedFd>, Never> {
  let (hear, tell) = match rustix::pipe::pipe_with(
    rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
  ) {
    Ok(ends) => ends,
    Err(fault) => {
      console_core_logging::warn!("console-panel: nothing to hear a signal on: {fault}");

      return Ok(None);
    }
  };

  match HELD.set(tell) {
    Ok(()) => {}
    Err(_this_process_has_already_asked) => return Ok(Some(hear)),
  }

  #[cfg_attr(
    dylint_lib = "explicit095_no_unsafe_in_a_handler",
    expect(
      explicit095_no_unsafe_in_a_handler,
      reason = "OnceLock::get is documented to never block and does not run initialization"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the OS registration takes an extern C handler; this handler only borrows the process-owned pipe and writes a nonblocking byte"
    )
  )]
  let answering = {
    // SAFETY: OnceLock::get never blocks or initializes; the handler only looks up the already owned descriptor and writes a nonblocking byte.
    unsafe { console_signals::install_handler(&console_signals::STOPPING, on_signal) }
  };

  match answering {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("console-panel: {fault}"),
  }

  Ok(Some(hear))
}

extern "C" fn on_signal(_number: core::ffi::c_int) {
  match HELD.get() {
    Some(telling) => {
      let _one_stop_is_enough_when_the_pipe_is_full = rustix::io::write(telling, &[1]);
    }
    None => {}
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_signal_pipe_is_not_handed_to_the_programs_a_panel_starts()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(heard) = signal_pipe();
    let hear = heard.ok_or("no pipe to hear a signal on")?;
    let flags = rustix::io::fcntl_getfd(&hear)?;

    assert!(
      flags.contains(rustix::io::FdFlags::CLOEXEC),
      "every app a panel starts would keep this pipe open for its whole life"
    );

    Ok(())
  }
}
