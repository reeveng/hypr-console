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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Already {
    PutBack,
    NotYet,
    CannotTell,
}

fn mark() -> Result<Option<PathBuf>, Never> {
    let runtime = match std::env::var("XDG_RUNTIME_DIR") {
        Ok(runtime) => runtime,
        Err(_nothing_says_where) => return Ok(None),
    };

    let instance = match std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        Ok(instance) => instance,
        Err(_no_compositor_to_be_the_same_one_as) => return Ok(None),
    };

    Ok(Some(PathBuf::from(runtime).join(crate::OURS).join(instance)))
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

pub fn said() -> Result<(), String> {
    let Ok(mark) = mark();

    let at = match mark {
        Some(at) => at,
        None => return Ok(()),
    };

    match at.parent() {
        Some(above) => std::fs::create_dir_all(above)
            .map_err(|fault| format!("{}: making it: {fault}", above.display()))?,
        None => {},
    }

    std::fs::write(&at, "")
        .map_err(|fault| format!("{}: writing it: {fault}", at.display()))
}
