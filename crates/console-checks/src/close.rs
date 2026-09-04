//! The top right paddle closes what is in front of you.

use console_stage::checking::{Body, Check, Done, less_than, same};
use console_stage::device::Device;
use console_stage::here::{Here, TURNS};

use crate::carry::something_open;

pub const CLOSE: Check = Check {
    name: "030-close-the-window",
    about: "The top right paddle closes what is in front of you.",
    feature: "close",
    since: "2026-08-24",
    bodies: &[Body::Here(here), Body::Device(there)],
};

/// The paddle asks for the same program whatever is on screen, and that
/// program is where a chooser and a window are told apart. What it does with
/// nothing up is the lock's own test, in console-panel.
fn here(stage: &mut Here) -> Done {
    stage.press("right-paddle-top")?;
    stage.settle(TURNS);
    let asked = stage.names();
    same(&asked, &["put-away"], || format!("it asked for {asked:?}"))
}

/// Only ever run with something open that can be lost without regret.
///
/// Counted on the workspace being looked at, because that is the one the paddle
/// acts on. Counted across the machine, a window sitting on some other
/// workspace is enough to say there was something to close.
fn there(stage: &mut Device) -> Done {
    something_open(stage)?;
    let before = stage.windows_here();
    stage.press("right-paddle-top");
    stage.settle(1.2);
    let now = stage.windows_here();
    less_than(now, before, || format!("{before} window(s) before and {now} after"))
}
