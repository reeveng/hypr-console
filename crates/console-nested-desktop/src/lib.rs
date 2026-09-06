//! The Legion Go's desktop, running on this machine, in a window.
//!
//! The device is a screen you have to pick up and a session you have to log
//! into. Most of what is worth looking at, the bar, the menu, the keyboard, the
//! panel, the colours everything wears, is ordinary Wayland software reading
//! ordinary config files, and this machine can run all of it. So it does: a
//! compositor of its own, inside a window, at the size the device's screen
//! actually is, reading the files the device reads.
//!
//! What it is not is the device. Read `## What this is not` in docs/desktop.md
//! before trusting it about anything.
//!
//! The staged copy is the whole of the trick. Every file the desktop reads is
//! copied out of files/ into one directory of this session's own, and every
//! absolute path inside those files is rewritten to point back into it. So a
//! stylesheet that says /usr/share/backgrounds/console.webp finds the picture
//! that is going to be installed there, without this machine having a
//! /usr/share/backgrounds it is allowed to write to. The copy is deleted when
//! the command ends, so there is nothing in it worth being careful with.

pub mod nested;
pub mod session;
pub mod staging;
pub mod talking;

use std::path::{Path, PathBuf};

use console_never::Never;

pub(crate) fn said(name: &str) -> Result<Option<String>, Never> {
    Ok(match std::env::var(name) {
        Ok(said) => Some(said),
        Err(std::env::VarError::NotPresent) => None,
        Err(fault) => {
            eprintln!("console-desktop: {name}: {fault}");

            None
        }
    })
}

pub fn root() -> Result<PathBuf, Never> {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    Ok(match from.canonicalize() {
        Ok(tidied) => tidied,
        Err(_sandboxed) => from,
    })
}

pub fn screen() -> Result<console_screen::Screen, String> {
    let Ok(root) = root();

    let at = root.join(console_screen::CONFIG);
    let said = std::fs::read_to_string(&at)
        .map_err(|why| format!("{}: {why}", at.display()))?;
    console_screen::Screen::read(&said)
}

pub fn stages() -> Result<PathBuf, Never> {
    let Ok(root) = root();

    Ok(root.join(".stage"))
}

pub fn stage() -> Result<PathBuf, Never> {
    let Ok(told) = said("CONSOLE_STAGE");
    let named = told.unwrap_or_else(|| format!("session-{}", std::process::id()));
    let Ok(stages) = stages();

    Ok(stages.join(named))
}

pub fn runtime() -> Result<PathBuf, Never> {
    // SAFETY: getuid cannot fail and touches nothing.
    let uid = unsafe { libc::getuid() };
    let Ok(told) = said("XDG_RUNTIME_DIR");
    let at = told.unwrap_or_else(|| format!("/run/user/{uid}"));

    Ok(PathBuf::from(at))
}

pub const HOME: &str = "/home/@user@";
