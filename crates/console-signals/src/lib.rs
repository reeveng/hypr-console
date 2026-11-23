//! What this process does when it is sent a signal.
//!
//! rustix asks the kernel for everything else this tree does with a signal --
//! sends one, names one, writes a byte from inside a handler. Installing a
//! disposition is the exception: rustix lists `sigaction` among the operations
//! outside its supported API in `not_implemented::libc_internals`. Its
//! experimental `runtime::kernel_sigaction` is a primitive for implementing a
//! runtime, with a kernel layout and signal-return restorer the caller must
//! supply, rather than a supported replacement in a libc-linked program.
//! This crate keeps that one C-runtime boundary as glibc's `signal`, declared
//! below rather than borrowed from the `libc` crate for one symbol. Other
//! signal operations use rustix.
//!
//! `signal` rather than `sigaction` because glibc's `signal` is the BSD
//! semantics a handler here wants -- the handler stays installed and a read
//! it interrupts is restarted -- and it takes a function pointer where
//! `sigaction` takes a struct whose layout would have to be written out again.
//!
//! What a handler may do is the caller's to promise, which is why
//! [`install_handler`] is `unsafe`: allocate nothing, lock nothing, and call
//! only what is safe from a signal. Every handler in this tree stores an
//! atomic, or writes to a descriptor through rustix, and nothing else.
//!
//! Signals were also how programs here used to talk to each other, and that
//! is gone: the bar hears its files change and the keyboard hears a word down
//! a socket. What is left is being asked to stop, which is what the rest of
//! the machine says with a signal -- a Ctrl-C, systemd, the compositor, a
//! picker taking the screen -- and cannot be asked to say any other way.

use std::ffi::c_int;
use std::fmt;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

use console_core_never::Never;

pub use rustix::process::Signal;

pub const STOPPING: [Signal; 3] = [Signal::HUP, Signal::INT, Signal::TERM];

#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "the POSIX stop handler publishes the process stop request through an atomic shared with the cleanup loop"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  allow(
    explicit044_no_ambient_value,
    reason = "a POSIX handler cannot capture caller state and publishes the process stop request through this atomic"
  )
)]
static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
  Requested,
  No,
}

#[cfg_attr(
  dylint_lib = "explicit095_no_unsafe_in_a_handler",
  expect(
    explicit095_no_unsafe_in_a_handler,
    reason = "the handler restores SIG_DFL through POSIX signal and reads errno on failure, without allocating or locking"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "this is the existing POSIX signal boundary and the stop handler performs only async-signal-safe operations"
  )
)]
pub fn catch_stops() -> Result<(), SignalError> {
  // SAFETY: the handler uses an atomic, signal disposition, errno and kill.
  unsafe { install_handler(&STOPPING, on_stop) }
}

pub fn stop_state() -> Result<Stop, Never> {
  Ok(
    match bool::then_some(STOP_REQUESTED.load(Ordering::SeqCst), ()) {
      Some(()) => Stop::Requested,
      None => Stop::No,
    },
  )
}

pub fn reset_stop() -> Result<(), Never> {
  STOP_REQUESTED.store(false, Ordering::SeqCst);

  Ok(())
}

extern "C" fn on_stop(number: c_int) {
  match bool::then_some(STOP_REQUESTED.swap(true, Ordering::SeqCst), ()) {
    Some(()) => {}
    None => return,
  }

  let again = match Signal::from_named_raw(number) {
    Some(named) => named,
    None => return,
  };

  match restore_default(again) {
    Ok(()) => {}
    Err(_still_handled) => return,
  }

  match rustix::process::kill_process(rustix::process::getpid(), again) {
    Ok(()) => {}
    Err(_undelivered) => STOP_REQUESTED.store(true, Ordering::SeqCst),
  }
}

pub type Handler = extern "C" fn(c_int);

const DEFAULT: Option<Handler> = None;

unsafe extern "C" {
  fn signal(number: c_int, disposition: Option<Handler>) -> *const u8;
}

const REFUSED: *const u8 = std::ptr::null::<u8>().wrapping_sub(1);

enum Registration {
  Installed,
  Failed,
}

impl From<bool> for Registration {
  fn from(refused: bool) -> Self {
    match bool::then_some(refused, ()) {
      Some(()) => Self::Failed,
      None => Self::Installed,
    }
  }
}

#[derive(Debug)]
pub enum SignalError {
  Failed(Signal, io::Error),
}

impl fmt::Display for SignalError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      SignalError::Failed(which, fault) => {
        write!(
          to,
          "signal {} would not be answered: {fault}",
          which.as_raw()
        )
      }
    }
  }
}

impl std::error::Error for SignalError {}

#[allow(
  clippy::missing_safety_doc,
  reason = "EXPLICIT020 requires the handler's async-signal-safety contract to be documented in the module head rather than a function doc comment"
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "this is the signal-disposition boundary; only the caller can promise the handler is async-signal-safe, and that promise is passed to the private glibc wrapper"
  )
)]
pub unsafe fn install_handler(signals: &[Signal], by: Handler) -> Result<(), SignalError> {
  for which in signals {
    // SAFETY: `by` is an `extern "C" fn(c_int)`, which is what a
    // disposition is, and the caller has promised what it does.
    unsafe { set(*which, Some(by)) }?;
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "restoring SIG_DFL through glibc invokes no user handler, so this boundary can uphold the private setter's safety contract without a caller promise"
  )
)]
pub fn restore_default(which: Signal) -> Result<(), SignalError> {
  // SAFETY: the default disposition is the kernel's own and runs nothing of
  // this process's.
  unsafe { set(which, DEFAULT) }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "rustix exposes no supported signal-disposition API for libc-linked processes; this single glibc call supplies the signal-return restorer, and the caller vouches for the handler's async-signal safety"
  )
)]
unsafe fn set(which: Signal, disposition: Option<Handler>) -> Result<(), SignalError> {
  // SAFETY: a signal number rustix named, and a disposition the caller
  // vouched for; `None` is the null pointer glibc reads as SIG_DFL.
  let was = unsafe { signal(which.as_raw(), disposition) };

  match Registration::from(was == REFUSED) {
    Registration::Failed => Err(SignalError::Failed(which, io::Error::last_os_error())),
    Registration::Installed => Ok(()),
  }
}

#[cfg(test)]
mod tests {
  use std::sync::atomic::{AtomicBool, Ordering};

  use super::*;

  #[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
      explicit044_no_ambient_value,
      reason = "a signal handler is handed nothing but the signal's number, so what it heard has to be somewhere it can store with one write"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit068_no_global_mutable_state",
    expect(
      explicit068_no_global_mutable_state,
      reason = "the C signal handler receives only a signal number and cannot capture test state; this lock-free atomic is its only communication with the delivery test"
    )
  )]
  static HEARD: AtomicBool = AtomicBool::new(false);

  extern "C" fn record_signal(_number: c_int) {
    HEARD.store(true, Ordering::SeqCst);
  }

  #[test]
  fn stopping_twice_restores_the_default_disposition() -> Result<(), io::Error> {
    use std::os::unix::process::ExitStatusExt;

    let me = std::env::current_exe()?;
    let child = std::process::Command::new(me)
      .args(["--exact", "tests::request_stop_in_child", "--ignored"])
      .output()?;

    assert_eq!(
      child.status.signal(),
      Some(Signal::TERM.as_raw()),
      "{child:?}"
    );

    Ok(())
  }

  #[test]
  #[ignore = "the parent test checks this subprocess's terminating signal"]
  fn request_stop_in_child() -> Result<(), Box<dyn std::error::Error>> {
    catch_stops()?;
    rustix::process::kill_process(rustix::process::getpid(), Signal::TERM)?;
    assert_eq!(wait_for_request(), Ok(Stop::Requested));

    let Ok(()) = reset_stop();

    assert_eq!(stop_state(), Ok(Stop::No));
    rustix::process::kill_process(rustix::process::getpid(), Signal::TERM)?;
    assert_eq!(wait_for_request(), Ok(Stop::Requested));

    rustix::process::kill_process(rustix::process::getpid(), Signal::TERM)?;

    for _ in 0..1_000_000 {
      std::thread::yield_now();
    }

    Err(Box::<dyn std::error::Error>::from(
      "the second signal should have stopped the subprocess",
    ))
  }

  fn wait_for_request() -> Result<Stop, Never> {
    let requested = (0..1_000_000).any(|_| {
      std::thread::yield_now();

      stop_state() == Ok(Stop::Requested)
    });

    Ok(match bool::then_some(requested, ()) {
      Some(()) => Stop::Requested,
      None => Stop::No,
    })
  }

  #[test]
  fn a_signal_answered_runs_the_handler_rather_than_the_default() -> Result<(), SignalError> {
    #[cfg_attr(
      dylint_lib = "explicit058_contain_unsafe",
      expect(
        explicit058_contain_unsafe,
        reason = "this test exercises the signal boundary with a handler that only stores to a lock-free atomic"
      )
    )]
    // SAFETY: the handler stores one flag.
    let installed = unsafe { install_handler(&[Signal::USR2], record_signal) };

    installed?;

    let sent = rustix::process::kill_process(rustix::process::getpid(), Signal::USR2);

    let ran = (0..1_000_000).any(|_| {
      std::thread::yield_now();

      HEARD.load(Ordering::SeqCst)
    });

    restore_default(Signal::USR2)?;

    sent.map_err(|fault| SignalError::Failed(Signal::USR2, io::Error::from(fault)))?;

    assert!(
      ran,
      "the handler never ran, so the signal went to its default"
    );

    Ok(())
  }

  #[test]
  fn a_signal_the_kernel_will_not_let_anyone_answer_is_said_to_be_refused() {
    #[cfg_attr(
      dylint_lib = "explicit058_contain_unsafe",
      expect(
        explicit058_contain_unsafe,
        reason = "this test exercises the kernel's refusal to install a SIGKILL handler; the supplied handler only stores to a lock-free atomic even if installation unexpectedly succeeds"
      )
    )]
    // SAFETY: the kernel refuses this before the handler could ever run.
    let refused = unsafe { install_handler(&[Signal::KILL], record_signal) };

    assert!(
      matches!(refused, Err(SignalError::Failed(Signal::KILL, _))),
      "{refused:?}"
    );
  }
}
