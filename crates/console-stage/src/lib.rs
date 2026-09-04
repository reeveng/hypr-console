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
///
/// Tidied by `canonicalize` where that works and left as it stands where it
/// does not: what `CARGO_MANIFEST_DIR` gives is already absolute and already
/// right, and canonicalizing only takes the `../..` out of the middle. It does
/// fail -- under a sandbox that will not let a process resolve a path it can
/// otherwise read -- and a check that stops dead there reports the sandbox as a
/// broken repository.
pub fn root() -> std::path::PathBuf {
    let from = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    match from.canonicalize() {
        Ok(tidied) => tidied,
        // The doc above, in one line: a sandbox that will not resolve a path it
        // will otherwise read is not a broken repository, and the path as it
        // stands is already absolute and already right.
        Err(_sandboxed) => from,
    }
}
