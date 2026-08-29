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

/// The repository this is all read out of.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository")
}

/// The device's screen, read out of the file the device reads: the mode, the
/// quarter turn and the density everything is drawn at. Nothing here is a
/// number about the screen; `console-screen` has them all, once.
pub fn screen() -> console_screen::Screen {
    let said = std::fs::read_to_string(root().join(console_screen::CONFIG))
        .expect("the compositor's config");
    console_screen::Screen::read(&said).expect("a screen")
}

/// Where a session's own copy lives, and where the sessions' copies live.
pub fn stages() -> PathBuf {
    root().join(".stage")
}

/// One session's copy, and no other session's.
///
/// Several of these run on this machine at once, and the first thing staging
/// does is delete what was there, so one path shared between them is one
/// session deleting the desktop another is in the middle of photographing. The
/// name carries the process it belongs to, which is also how one left behind by
/// a session that has ended is recognised. Set CONSOLE_STAGE to keep a copy of
/// your own that survives the command and is never swept up.
pub fn stage() -> PathBuf {
    let named =
        std::env::var("CONSOLE_STAGE").unwrap_or_else(|_| format!("session-{}", std::process::id()));
    stages().join(named)
}

/// Where this machine keeps what a session is running.
pub fn runtime() -> PathBuf {
    // SAFETY: getuid cannot fail and touches nothing.
    let uid = unsafe { libc::getuid() };
    let said = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| format!("/run/user/{uid}"));
    PathBuf::from(said)
}

/// The home the tree writes, which is a mark rather than a name.
///
/// `console apply` fills it in with whoever the machine belongs to. Nothing is
/// applied to stage a desktop, so what is staged still carries the mark, and
/// this is what the staging rewrites to point back inside itself.
pub const HOME: &str = "/home/@user@";
