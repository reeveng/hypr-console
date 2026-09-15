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

use std::fmt;
use std::path::{Path, PathBuf};

use console_core_never::Never;

#[derive(Debug)]
pub enum Unnested {
    Machine(std::io::Error),
    Unreadable(PathBuf, std::io::Error),
    Undeclared(console_screen::Undeclared),
    Staging(&'static str, std::io::Error),
    Unwritten(&'static str, console_core_atomic_writes::Unwritten),
    NoExecStart(PathBuf),
    NoCompositor,
    NoScreen,
    NowhereToWrite,
    NothingToPress,
}

impl fmt::Display for Unnested {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unnested::Machine(fault) => write!(to, "{fault}"),
            Unnested::Unreadable(at, fault) => write!(to, "{}: {fault}", at.display()),
            Unnested::Undeclared(fault) => write!(to, "{fault}"),
            Unnested::Staging(what, fault) => write!(to, "{what}: {fault}"),
            Unnested::Unwritten(what, fault) => write!(to, "{what}: {fault}"),
            Unnested::NoExecStart(at) => write!(
                to,
                "{} names no absolute ExecStart, so the stage would start a keyboard \
                 nothing could raise",
                at.display()
            ),
            Unnested::NoCompositor => write!(to, "the nested compositor never came up"),
            Unnested::NoScreen => write!(to, "the screen never appeared"),
            Unnested::NowhereToWrite => {
                write!(to, "a picture wants somewhere to be written")
            }
            Unnested::NothingToPress => write!(
                to,
                "a press wants the file the panel writes its draws to and a --then to press"
            ),
        }
    }
}

impl std::error::Error for Unnested {}

impl From<console_screen::Undeclared> for Unnested {
    fn from(fault: console_screen::Undeclared) -> Self {
        Unnested::Undeclared(fault)
    }
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "CONSOLE_STAGE names which stage tree this run is using, and a stage is what this crate is. What it used to read as well -- PATH, the runtime directory, the compositor's signature -- belongs to other crates and is asked of them now"
    )
)]
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

pub fn screen() -> Result<console_screen::Screen, Unnested> {
    let Ok(root) = root();

    let at = root.join(console_screen::CONFIG);
    let said = std::fs::read_to_string(&at)
        .map_err(|why| Unnested::Unreadable(at.clone(), why))?;

    console_screen::Screen::read(&said).map_err(Unnested::Undeclared)
}

pub fn stages() -> Result<PathBuf, Never> {
    let Ok(root) = root();

    Ok(root.join(".stage"))
}

pub fn stage() -> Result<PathBuf, Never> {
    let Ok(told) = said("CONSOLE_STAGE");
    let named = match told {
        Some(named) => named,
        None => format!("session-{}", std::process::id()),
    };
    let Ok(stages) = stages();

    Ok(stages.join(named))
}

pub fn runtime() -> Result<PathBuf, Never> {
    let told = console_core_places::runtime()?;

    Ok(match told {
        Some(told) => told,
        None => {
            // SAFETY: getuid cannot fail and touches nothing.
            let uid = unsafe { libc::getuid() };

            PathBuf::from(format!("/run/user/{uid}"))
        }
    })
}

pub const HOME: &str = "/home/@user@";
