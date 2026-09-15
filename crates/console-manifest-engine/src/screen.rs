//! The monitor block this machine's own panel wants.
//!
//! Everything else an apply writes comes out of the tree. This comes out of the
//! machine, for the reason the controller's router profile does: what it holds
//! is read off the hardware in front of it, so a copy in the tree would be one
//! machine's answer carried to all of them -- which is what the block in
//! `hyprland.lua` was, and it is left there as the seed this is read after.
//!
//! An apply is where it belongs because an apply is the one thing that happens
//! on a machine before anybody logs into it. A session could write it, and the
//! first login would already have been on somebody else's panel by then.
//!
//! The file is in a person's home and this runs as root, so it is handed over
//! afterwards. A config file in somebody's home owned by root is a file they
//! cannot edit and a directory the next thing cannot write into, which is the
//! same fault `install` already answers for everything the manifest names.

use std::path::Path;

use console_core_never::Never;
use console_screen::{DRAWN_AT, kernel};

use crate::machine;

pub fn wrote(home: &Path) -> Result<Option<String>, Never> {
    let Ok(found) = kernel::here();

    let panel = match found {
        Some(panel) => panel,
        None => {
            eprintln!(
                "the kernel says nothing about a screen under {}, so the block in the \
                 compositor's own file is what stands",
                kernel::DRM
            );

            return Ok(None);
        }
    };

    let Ok(at) = kernel::at(home);
    let Ok(block) = panel.block(DRAWN_AT);

    let holding = match at.parent() {
        Some(holding) => holding,
        None => return Ok(None),
    };

    match std::fs::create_dir_all(holding) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("{}: {fault}", holding.display());

            return Ok(None);
        }
    }

    let Ok(()) = handed_over(holding);

    match console_core_atomic_writes::settled(&at, block.as_bytes()) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("{fault}");

            return Ok(None);
        }
    }

    let Ok(()) = handed_over(&at);

    Ok(Some(at.display().to_string()))
}

fn handed_over(at: &Path) -> Result<(), Never> {
    match machine::handed_over(at) {
        Ok(()) => {},
        Err(fault) => eprintln!("{fault}"),
    }

    Ok(())
}
