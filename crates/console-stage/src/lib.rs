//! Somewhere a check can be run, and what can be seen from there.
//!
//! A check says what somebody did and what should have happened. Where it is
//! run decides how the doing is done and how much of the happening can be seen
//! at all.
//!
//! ```text
//! here      emulated devices, the daemon in this process, no machine
//!           involved. What can be seen is what the daemon decided to run.
//!
//! desktop   the device's own desktop, nested on this machine, and looked at.
//!           What this can answer that nothing else can is what colour the
//!           screen is. It cannot press anything.
//!
//! device    the Legion Go itself, over ssh. The pressing goes through
//!           InputPlumber's own SendEvent, so a button arrives exactly as the
//!           hardware's would, through the loaded profile. What can be seen is
//!           the machine: which workspace, which windows, how bright, whether
//!           the keyboard is up, which profile is loaded.
//! ```
//!
//! The same check runs in more than one of them. It cannot assert the same
//! things in each, so it says what it needs to be able to see by which stage it
//! is written for, and a stage nothing is written for skips it and says so
//! rather than passing quietly.

pub mod checking;
pub mod desktop;
pub mod device;
pub mod here;
pub mod palette;
pub mod picture;
pub mod plug;

/// The repository this is all read out of.
pub fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository")
}
