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
//!           screen is, and what it presses with is the pointer -- a finger
//!           needs uinput, and uinput belongs to the machine this is nested on
//!           rather than to the picture.
//!
//! panels    one panel, opened alone in the nested desktop and asked what it
//!           drew. The tier a change to a panel is tried in while it is being
//!           written: one program, a few seconds, no device. What it can answer
//!           is not whether a thing is drawn but whether a hand could use it.
//!
//! device    the Legion Go itself, over ssh. The pressing goes through
//!           InputPlumber's own SendEvent, so a button, a stick and a trigger
//!           all arrive exactly as the hardware's would, through the loaded
//!           profile; a finger arrives through the touchscreen `console-tap`
//!           makes, and the pointer through `console-point`. What can be seen
//!           is the machine: which workspace, which windows, how bright,
//!           whether the keyboard is up, which profile is loaded, and what
//!           colour any place on the screen is.
//! ```
//!
//! The same check runs in more than one of them. It cannot assert the same
//! things in each, so it says what it needs to be able to see by which stage it
//! is written for, and a stage nothing is written for skips it and says so
//! rather than passing quietly.
//!
//! The last of them is somebody's machine and is lent rather than given, so a
//! run there is bracketed by `putting_back`: what was true before anything was
//! pressed is read once, and put back once the last check has had its turn.
//! `stopping` is the same promise kept when a run is interrupted, which is the
//! moment it matters most.

pub mod baseline;
pub mod checking;
pub mod desktop;
pub mod device;
pub mod here;
pub mod lasting;
pub mod palette;
pub mod panels;
pub mod picture;
pub mod plug;
pub mod putting_back;
pub mod stopping;
pub mod watching;

pub fn beside(program: &str) -> Result<std::path::PathBuf, console_core_never::Never> {
    let running = match std::env::current_exe() {
        Ok(running) => running,
        Err(_nothing_says_where_this_is) => return Ok(std::path::PathBuf::from(program)),
    };

    let beside_it = running.parent().map(std::path::Path::to_path_buf);
    let above_that = running.parent().and_then(std::path::Path::parent).map(std::path::Path::to_path_buf);

    let built = [beside_it, above_that]
        .into_iter()
        .flatten()
        .map(|at| at.join(program))
        .find(|at| at.is_file());

    Ok(built.unwrap_or_else(|| std::path::PathBuf::from(program)))
}

pub fn screen() -> Result<console_screen::Screen, String> {
    let Ok(root) = root();
    let written = std::fs::read_to_string(root.join(console_screen::CONFIG))
        .map_err(|fault| fault.to_string())?;

    console_screen::Screen::read(&written)
}

pub fn root() -> Result<std::path::PathBuf, console_core_never::Never> {
    let from = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    Ok(match from.canonicalize() {
        Ok(tidied) => tidied,
        Err(_sandboxed) => from,
    })
}
