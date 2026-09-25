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

use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI32, Ordering};

use console_core_never::Never;

pub const STOPPING: [i32; 3] = [libc::SIGHUP, libc::SIGINT, libc::SIGTERM];

const NOWHERE: i32 = -1;

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "a signal handler takes nothing and is handed nothing, so where to say a signal arrived is the one thing that cannot be passed in; it is the process's own pipe and it is written once"
    )
)]
static TELLING: AtomicI32 = AtomicI32::new(NOWHERE);

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the far end of that pipe, held for as long as the process is, because a closed write end would be a hangup on the loop rather than a signal"
    )
)]
static HELD: OnceLock<OwnedFd> = OnceLock::new();

pub fn told() -> Result<Option<OwnedFd>, Never> {
    let (hear, tell) = match rustix::pipe::pipe() {
        Ok(ends) => ends,
        Err(fault) => {
            eprintln!("console-panel: nothing to hear a signal on: {fault}");

            return Ok(None);
        }
    };

    let raw = tell.as_raw_fd();

    match HELD.set(tell) {
        Ok(()) => {},
        Err(_this_process_has_already_asked) => return Ok(Some(hear)),
    }

    TELLING.store(raw, Ordering::SeqCst);

    #[cfg_attr(
        dylint_lib = "explicit051_no_machine_width",
        allow(
            explicit051_no_machine_width,
            reason = "`signal` takes a `sighandler_t`, which is the machine's width by the C ABI and not by choice here"
        )
    )]
    #[cfg_attr(
        dylint_lib = "explicit011_no_as_cast",
        allow(
            explicit011_no_as_cast,
            reason = "no trait turns a function into the number `signal` takes, which is the same reason `picker` gives two files over"
        )
    )]
    let answer = asked as extern "C" fn(libc::c_int) as libc::sighandler_t;

    for number in STOPPING {
        // SAFETY: the handler allocates nothing and writes one byte to a pipe.
        unsafe { libc::signal(number, answer) };
    }

    Ok(Some(hear))
}

extern "C" fn asked(_number: libc::c_int) {
    let telling = TELLING.load(Ordering::SeqCst);

    match telling {
        NOWHERE => {},
        fd => {
            let said: [u8; 1] = [1];

            // SAFETY: one byte to a pipe this process opened and still holds.
            let _ = unsafe { libc::write(fd, said.as_ptr().cast(), 1) };
        }
    }
}
