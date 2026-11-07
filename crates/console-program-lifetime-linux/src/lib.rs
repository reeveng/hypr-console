//! Binds a Linux child to the lifetime of the thread that starts it.
//! The binding is installed before the child executes its program.

#[cfg(not(target_os = "linux"))]
compile_error!("console-program-lifetime-linux requires Linux");

use console_core_never::Never;
use std::ffi::c_int;
use std::io;
use std::os::unix::process::CommandExt;
use std::process::Command;

unsafe extern "C" {
    fn prctl(option: c_int, ...) -> c_int;
    fn getppid() -> c_int;
}

const PR_SET_PDEATHSIG: c_int = 1;
const SIGTERM: c_int = 15;

pub fn bind(command: &mut Command) -> Result<(), Never> {
    let whose = std::process::id();

    // SAFETY: what runs between the fork and the exec is two calls, prctl and
    // getppid, both async-signal-safe, and nothing here allocates or takes a
    // lock. The error built on the way out is the child's own exit and never
    // runs in the parent.
    unsafe {
        command.pre_exec(move || {
            let _ = prctl(PR_SET_PDEATHSIG, SIGTERM);

            match u32::try_from(getppid()) {
                Ok(now) => match now == whose {
                    true => Ok(()),
                    false => Err(io::Error::other("whoever wanted this is already gone")),
                },
                Err(_fault) => Err(io::Error::other("this has no parent left to outlive")),
            }
        });
    }

    Ok(())
}
