//! Where the device is named, and the one place it is read.
//!
//! `CONSOLE_HOST` has no default, because an address is someone's machine and
//! this tree does not carry one. The programs in `console-device` may not read
//! it: they are `console_program_contract::Program`s and a value that arrives
//! through the side of one is a value no transcript can put a different answer
//! in. So it is read out here, by the binaries, and handed in as the first word
//! of an arguments. It is its own crate because the publish and the device
//! stage want the name and nothing else a deploy is made of.
//!
//! Absent and unreadable are not the same thing. Nothing set means there is no
//! device to talk to, which the programs say for themselves and much better
//! than this could; a variable that is set to something no name could be is a
//! fault, and saying "there is no device" about it would send someone looking
//! at the wrong end of the machine.

pub const HOST: &str = "CONSOLE_HOST";

#[derive(Debug)]
pub struct Unnamed(pub std::env::VarError);

impl std::fmt::Display for Unnamed {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(to, "{HOST}: {}", self.0)
    }
}

impl std::error::Error for Unnamed {}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "CONSOLE_HOST names the device, and the head above says why it is read out here beside the binaries rather than inside a program. Three other crates read it before this was the one place"
    )
)]
pub fn device() -> Result<String, Unnamed> {
    match std::env::var(HOST) {
        Ok(said) => Ok(said),
        Err(std::env::VarError::NotPresent) => Ok(String::new()),
        Err(fault) => Err(Unnamed(fault)),
    }
}
