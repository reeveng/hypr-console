//! What this process does when it is sent a signal.
//!
//! rustix asks the kernel for everything else this tree does with a signal --
//! sends one, names one, writes a byte from inside a handler -- and stops
//! here on purpose. Its `runtime` module does have `kernel_sigaction`, and
//! says it is for a program no C library is running: the kernel returns from
//! a handler through a restorer that glibc supplies, and a handler installed
//! beneath glibc without one comes back into nothing. Every program here is
//! linked against glibc, so a disposition is set through glibc's own
//! `signal`, declared below rather than borrowed from the `libc` crate for one
//! symbol. That is the whole of the C this tree calls about signals, and it is
//! in one place with its reason beside it.
//!
//! `signal` rather than `sigaction` because glibc's `signal` is the BSD
//! semantics a handler here wants -- the handler stays installed and a read
//! it interrupts is restarted -- and it takes a function pointer where
//! `sigaction` takes a struct whose layout would have to be written out again.
//!
//! What a handler may do is the caller's to promise, which is why
//! [`answered`] is `unsafe`: allocate nothing, lock nothing, and call only what
//! is safe from a signal. Every handler in this tree stores an atomic, or
//! writes to a descriptor through rustix, and nothing else.
//!
//! Signals were also how programs here used to talk to each other, and that
//! is gone: the bar hears its files change and the keyboard hears a word down
//! a socket. What is left is being asked to stop, which is what the rest of
//! the machine says with a signal -- a Ctrl-C, systemd, the compositor, a
//! picker taking the screen -- and cannot be asked to say any other way.

use std::ffi::c_int;
use std::fmt;
use std::io;

pub use rustix::process::Signal;

pub const STOPPING: [Signal; 3] = [Signal::HUP, Signal::INT, Signal::TERM];

pub type Handler = extern "C" fn(c_int);

const DEFAULT: Option<Handler> = None;

unsafe extern "C" {
    fn signal(number: c_int, disposition: Option<Handler>) -> *const u8;
}

const REFUSED: *const u8 = std::ptr::null::<u8>().wrapping_sub(1);

#[derive(Debug)]
pub enum SignalError {
    Failed(Signal, io::Error),
}

impl fmt::Display for SignalError {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignalError::Failed(which, fault) => {
                write!(to, "signal {} would not be answered: {fault}", which.as_raw())
            }
        }
    }
}

impl std::error::Error for SignalError {}

#[allow(
    clippy::missing_safety_doc,
    reason = "EXPLICIT020 denies the doc comment this asks for; what a handler has to promise is the module head's third paragraph"
)]
pub unsafe fn answered(signals: &[Signal], by: Handler) -> Result<(), SignalError> {
    for which in signals {
        // SAFETY: `by` is an `extern "C" fn(c_int)`, which is what a
        // disposition is, and the caller has promised what it does.
        unsafe { set(*which, Some(by)) }?;
    }

    Ok(())
}

pub fn defaulted(which: Signal) -> Result<(), SignalError> {
    // SAFETY: the default disposition is the kernel's own and runs nothing of
    // this process's.
    unsafe { set(which, DEFAULT) }
}

unsafe fn set(which: Signal, disposition: Option<Handler>) -> Result<(), SignalError> {
    // SAFETY: a signal number rustix named, and a disposition the caller
    // vouched for; `None` is the null pointer glibc reads as SIG_DFL.
    let was = unsafe { signal(which.as_raw(), disposition) };

    match was == REFUSED {
        true => Err(SignalError::Failed(which, io::Error::last_os_error())),
        false => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    static HEARD: AtomicBool = AtomicBool::new(false);

    extern "C" fn heard(_number: c_int) {
        HEARD.store(true, Ordering::SeqCst);
    }

    #[test]
    fn a_signal_answered_runs_the_handler_rather_than_the_default() {
        // SAFETY: the handler stores one flag.
        let installed = unsafe { answered(&[Signal::USR2], heard) };

        assert!(installed.is_ok(), "{installed:?}");

        let _ = rustix::process::kill_process(rustix::process::getpid(), Signal::USR2);

        let ran = (0..1_000_000).any(|_| {
            std::thread::yield_now();

            HEARD.load(Ordering::SeqCst)
        });

        assert!(ran, "the handler never ran, so the signal went to its default");
        assert!(defaulted(Signal::USR2).is_ok());
    }

    #[test]
    fn a_signal_the_kernel_will_not_let_anyone_answer_is_said_to_be_refused() {
        // SAFETY: the kernel refuses this before the handler could ever run.
        let refused = unsafe { answered(&[Signal::KILL], heard) };

        assert!(matches!(refused, Err(SignalError::Failed(Signal::KILL, _))), "{refused:?}");
    }
}
