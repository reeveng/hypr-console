//! Hand the session's environment to systemd, then start the desktop.
//!
//! Run by the compositor, once, as it comes up.
//!
//! The radio is put back last, after the desktop has been asked for, because
//! powering an adapter that Steam turned off is the better part of two seconds
//! and none of it is anything somebody is waiting to see.

use console_core_never::Never;
use console_session::{radio, run_each, starting};

fn main() {
    let Ok(starting) = starting();
    let Ok(()) = run_each("starting", &starting);
    let Ok(()) = putting_back();
}

fn putting_back() -> Result<(), Never> {
    let Ok(runtime) = console_core_places::runtime_ours();

    let runtime = match runtime {
        Some(runtime) => runtime,
        None => return Ok(()),
    };

    let before = match radio::remembered(&runtime) {
        Ok(before) => before,
        Err(fault) => {
            eprintln!("session-start: {fault}");

            return Ok(());
        }
    };

    match before {
        Some(_) => {},
        None => return Ok(()),
    }

    let asked = match radio::asked() {
        Ok(asked) => asked,
        Err(fault) => {
            eprintln!("session-start: {fault}");

            return Ok(());
        }
    };

    let Ok(steps) = radio::putting_back(before, asked);

    match steps {
        Some(steps) => {
            let Ok(()) = run_each("radio", &[steps]);
        },
        None => {},
    }

    match radio::forget(&runtime) {
        Ok(()) => {},
        Err(fault) => eprintln!("session-start: {fault}"),
    }

    Ok(())
}
