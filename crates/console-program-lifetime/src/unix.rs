//! Unix process signals.

use console_core_never::Never;

pub fn signal(pid: i32, signal: rustix::process::Signal) -> Result<(), Never> {
    let sent =
        rustix::process::Pid::from_raw(pid).map(|pid| rustix::process::kill_process(pid, signal));

    match sent {
        Some(Ok(())) => {}
        Some(Err(_already_gone)) => {}
        None => {}
    }

    Ok(())
}
