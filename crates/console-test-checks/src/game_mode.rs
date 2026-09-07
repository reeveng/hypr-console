//! Legion left leaves the desktop for Game Mode.
//!
//! Pressing it all the way through would put Steam on the screen and take the
//! desktop the rest of the checks are running against with it, so what is asked
//! here is that the button reaches the one script that knows how to leave: the
//! controller goes back to being a gamepad, and the session is switched.
//!
//! That was the whole of it, and it stayed green through a day in which the
//! button did nothing on either side of the door. The script it reaches ends in
//! `pkexec`, and a daemon that cannot become root gets "pkexec must be setuid
//! root" and leaves the session unrecorded. Nothing here could see that: an
//! emulated press cannot fail at a setuid bit, and the bit is not in the unit
//! file either -- systemd puts the no-new-privileges flag on a process for
//! carrying any seccomp at all, so `NoNewPrivileges=no` and `systemctl show`
//! both said what was not true of the process. The one place it can be read is
//! the running daemon on the machine, which is what the device stage asks.

use console_test_stages::checking::{Body, Check, Done, same};
use console_test_stages::device::Device;
use console_test_stages::here::{Here, TURNS};

pub const GAME_MODE: Check = Check {
    name: "190-game-mode",
    about: "Legion left leaves the desktop for Game Mode.",
    feature: "game-mode",
    since: "2026-08-28",
    bodies: &[Body::Here(here), Body::Device(there)],
};

const DAEMON: &str = "stick-scroll";

fn here(stage: &mut Here) -> Done {
    stage.press("legion-left")?;
    let Ok(()) = stage.settle(TURNS);
    let Ok(ran) = stage.names();

    same(&ran, &["game-mode"], || format!("it ran {ran:?}"))
}

fn there(stage: &mut Device) -> Done {
    let Ok(said) =
        stage.user(&format!("grep NoNewPrivs /proc/$(pgrep -x {DAEMON})/status || true"));
    let bit = said.split_whitespace().last().unwrap_or_default();

    same(bit, "0", || {
        format!(
            "the daemon the button runs from is {DAEMON} and its no-new-privileges bit is {bit:?}, \
             so the pkexec that records the session will be refused and the button will do nothing"
        )
    })
}
