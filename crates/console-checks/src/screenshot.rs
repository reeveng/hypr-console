//! Held with L2, the bottom right paddle takes a screenshot.

use console_stage::checking::{Body, Check, Done, ought};
use console_stage::device::Device;
use console_stage::here::{Here, TURNS};

pub const SHOT: Check = Check {
    name: "070-a-screenshot",
    about: "Held with L2, the bottom right paddle takes a screenshot.",
    feature: "screenshot",
    since: "2026-08-26",
    bodies: &[Body::Here(here), Body::Device(there)],
};

/// The other half, and the reason the layer was put there at all.
///
/// A paddle is where a finger lies when it is only holding the machine up, and
/// on its own this one took ninety-six pictures in two days. A check that only
/// asks whether the picture arrives would pass just as well on the button that
/// took all of them.
pub const ALONE: Check = Check {
    name: "071-the-paddle-alone-takes-nothing",
    about: "Without L2, the same paddle takes no picture at all.",
    feature: "screenshot",
    since: "2026-09-01",
    bodies: &[Body::Here(alone_here), Body::Device(alone_there)],
};

fn here(stage: &mut Here) -> Done {
    stage.trigger("l2", 1.0)?;
    stage.press("right-paddle-bottom")?;
    stage.settle(TURNS);
    let ran = stage.commands().to_vec();
    ought(ran == [["/usr/local/bin/console-screenshot"]], || format!("it ran {ran:?}"))
}

fn alone_here(stage: &mut Here) -> Done {
    stage.press("right-paddle-bottom")?;
    stage.settle(TURNS);
    let ran = stage.commands().to_vec();
    ought(ran.is_empty(), || format!("it ran {ran:?}"))
}

fn there(stage: &mut Device) -> Done {
    // Where the pictures land, in the home of whoever the device belongs to.
    let shots = format!("{}/Pictures", stage.home());
    let before = stage.files(&shots).len();
    stage.trigger("l2", 1.0)?;
    stage.press("right-paddle-bottom");
    stage.trigger("l2", 0.0)?;
    stage.settle(2.5);
    let after = stage.files(&shots).len();
    ought(after > before, || format!("no picture appeared in {shots}"))
}

/// The trigger is let go before the paddle is pressed rather than never
/// pulled, because the state this is about is a machine somebody has just
/// finished using, not a machine fresh out of a boot.
fn alone_there(stage: &mut Device) -> Done {
    let shots = format!("{}/Pictures", stage.home());
    stage.trigger("l2", 0.0)?;
    let before = stage.files(&shots).len();
    stage.press("right-paddle-bottom");
    stage.settle(2.5);
    let after = stage.files(&shots).len();
    ought(after == before, || format!("a picture arrived in {shots} unasked"))
}
