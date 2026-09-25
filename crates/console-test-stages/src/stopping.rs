//! Being asked to stop, so a run that is interrupted still hands the device back.
//!
//! Ctrl-C used to be the worst way to end a device run. The run died where it
//! stood, on whatever workspace the last press had left it, with whatever it
//! had opened still open, and the person holding the machine got back a
//! desktop someone else had been driving. That is also the state a run is
//! most likely to be interrupted from: it is stopped because it is doing
//! something to someone's device they would rather it did not.
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

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "a signal handler is handed nothing and may allocate nothing, and what it has to say -- someone asked this run to stop -- is read by every wait in the run"
    )
)]
static ASKED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    Requested,
    No,
}

pub fn caught() -> Result<(), Never> {
    // SAFETY: the handler stores one flag, and the second time puts the
    // default back and sends the signal again, all of it through calls that
    // allocate nothing.
    let answering = unsafe { console_signals::answered(&console_signals::STOPPING, answered) };

    match answering {
        Ok(()) => {},
        Err(fault) => eprintln!("console-check: an interrupted run will not hand the device back: {fault}"),
    }

    Ok(())
}

extern "C" fn answered(number: core::ffi::c_int) {
    match ASKED.swap(true, Ordering::SeqCst) {
        true => match rustix::process::Signal::from_named_raw(number) {
            Some(again) => {
                let _ = console_signals::defaulted(again);
                let _ = rustix::process::kill_process(rustix::process::getpid(), again);
            },
            None => {},
        },
        false => {},
    }
}

pub fn asked() -> Result<Stop, Never> {
    Ok(match ASKED.load(Ordering::SeqCst) {
        true => Stop::Requested,
        false => Stop::No,
    })
}

pub fn no_longer() -> Result<(), Never> {
    ASKED.store(false, Ordering::SeqCst);

    Ok(())
}
