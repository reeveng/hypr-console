//! Where the device is named, and the one place it is read.
//!
//! `CONSOLE_HOST` has no default, because an address is somebody's machine and
//! this tree does not carry one. Neither program here may read it: both are
//! `console_program_contract::Program`s and a value that arrives through the
//! side of one is a value no transcript can put a different answer in. So it
//! is read out here, beside the binaries, and handed in as the first word of
//! an argv.
//!
//! Absent and unreadable are not the same thing. Nothing set means there is no
//! device to talk to, which the programs say for themselves and much better
//! than this could; a variable that is set to something no name could be is a
//! fault, and saying "there is no device" about it would send somebody looking
//! at the wrong end of the machine.

pub const HOST: &str = "CONSOLE_HOST";

pub fn device() -> Result<String, String> {
    match std::env::var(HOST) {
        Ok(said) => Ok(said),
        Err(std::env::VarError::NotPresent) => Ok(String::new()),
        Err(fault) => Err(format!("{HOST}: {fault}")),
    }
}
