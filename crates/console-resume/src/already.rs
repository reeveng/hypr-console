//! Whether this compositor has already had its windows put back.
//!
//! The unit that runs this carries `Restart=always`, which is right for a thing
//! that should be saving whenever the desktop is up. It is disastrous for a
//! thing whose first act is to close every window: a crash, a `systemctl
//! restart`, a deploy that reloads the units -- each of those swept the screen
//! and rebuilt it out of a file, five seconds later, in front of somebody who
//! was using it.
//!
//! Putting back is what a desktop *starting* means, and a restart is not a
//! desktop starting. So it happens once for each compositor, and which
//! compositor is a thing the compositor already says:
//! `HYPRLAND_INSTANCE_SIGNATURE` is different for every one and the same for
//! every program inside it. The mark lives in the runtime directory, which the
//! login session takes away with it, so a machine that has been off overnight
//! has no mark and the desktop comes back.
//!
//! Without the signature there is nothing to key a mark to, and the honest
//! answer is that this cannot tell a first start from a restart. It says so and
//! declines to put anything back, because the failure that matters here is not a
//! desktop that did not come back -- it is one that was taken away.

use std::path::PathBuf;

use console_core_never::Never;

use crate::Unresumed;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Already {
    PutBack,
    NotYet,
    CannotTell,
}

fn mark() -> Result<Option<PathBuf>, Never> {
    let Ok(said) = console_core_places::runtime();

    let runtime = match said {
        Some(runtime) => runtime,
        None => return Ok(None),
    };

    let Ok(named) = console_compositor::instance();

    let instance = match named {
        Some(instance) => instance,
        None => return Ok(None),
    };

    Ok(Some(runtime.join(crate::OURS).join(instance)))
}

pub fn asked() -> Result<Already, Never> {
    let Ok(mark) = mark();

    let at = match mark {
        Some(at) => at,
        None => return Ok(Already::CannotTell),
    };

    Ok(match at.exists() {
        true => Already::PutBack,
        false => Already::NotYet,
    })
}

pub fn said() -> Result<(), Unresumed> {
    let Ok(mark) = mark();

    let at = match mark {
        Some(at) => at,
        None => return Ok(()),
    };

    match at.parent() {
        Some(above) => std::fs::create_dir_all(above)
            .map_err(|fault| Unresumed::Making(above.to_path_buf(), fault))?,
        None => {},
    }

    console_core_atomic_writes::whole(&at, b"").map_err(Unresumed::Unwritten)
}
