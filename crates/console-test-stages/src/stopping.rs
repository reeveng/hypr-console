//! Being asked to stop, so a run that is interrupted still hands the device back.
//!
//! Ctrl-C used to be the worst way to end a device run. The run died where it
//! stood, on whatever workspace the last press had left it, with whatever it
//! had opened still open, and the person holding the machine got back a
//! desktop somebody else had been driving. That is also the state a run is
//! most likely to be interrupted from: it is stopped because it is doing
//! something to somebody's device they would rather it did not.
//!
//! So the signal is caught rather than fatal, and what it sets is one flag
//! that two places read. The loop over the checks reads it and stops asking
//! for more; `Device::until` reads it and stops waiting for something that is
//! not going to happen now. Without the second the first is no use -- a run
//! interrupted inside the twenty restarts of `240` would go on restarting for
//! minutes after being told to stop.
//!
//! Asked twice it does not argue. The second signal puts the default
//! disposition back and raises it again, so a run that will not come out
//! quietly comes out the way anything else does.
//!
//! The flag is put down again before the putting back, because every wait in
//! that goes through the same `until`, and a run that has stopped waiting
//! cannot see whether what it asked for arrived.

use std::sync::atomic::{AtomicBool, Ordering};

use console_core_never::Never;

static ASKED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    Asked,
    No,
}

pub fn caught() -> Result<(), Never> {
    #[cfg_attr(
        dylint_lib = "explicit011_no_as_cast",
        allow(
            explicit011_no_as_cast,
            reason = "no trait turns a function into the number `signal` takes; the way out is a signalfd, which is its own decision and is the one `console-panel` did not take either"
        )
    )]
    let answer = answered as extern "C" fn(libc::c_int) as libc::sighandler_t;

    for number in [libc::SIGHUP, libc::SIGINT, libc::SIGTERM] {
        // SAFETY: the handler stores one flag and calls nothing that allocates.
        unsafe { libc::signal(number, answer) };
    }

    Ok(())
}

extern "C" fn answered(number: libc::c_int) {
    match ASKED.swap(true, Ordering::SeqCst) {
        true => {
            // SAFETY: the default disposition put back and the same signal
            unsafe {
                libc::signal(number, libc::SIG_DFL);
                libc::raise(number);
            }
        }
        false => {},
    }
}

pub fn asked() -> Result<Stop, Never> {
    Ok(match ASKED.load(Ordering::SeqCst) {
        true => Stop::Asked,
        false => Stop::No,
    })
}

pub fn no_longer() -> Result<(), Never> {
    ASKED.store(false, Ordering::SeqCst);

    Ok(())
}
