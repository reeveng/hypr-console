//! The bottom right paddle takes a screenshot.

use console_stage::checking::{Body, Check, Done, ought};
use console_stage::device::Device;
use console_stage::here::{Here, TURNS};

pub const SHOT: Check = Check {
    name: "070-a-screenshot",
    about: "The bottom right paddle takes a screenshot.",
    feature: "screenshot",
    since: "2026-08-26",
    bodies: &[Body::Here(here), Body::Device(there)],
};

fn here(stage: &mut Here) -> Done {
    stage.press("right-paddle-bottom")?;
    stage.settle(TURNS);
    let ran = stage.commands().to_vec();
    ought(ran == [["/usr/local/bin/console-screenshot"]], || format!("it ran {ran:?}"))
}

fn there(stage: &mut Device) -> Done {
    // Where the pictures land, in the home of whoever the device belongs to.
    let shots = format!("{}/Pictures", stage.home());
    let before = stage.files(&shots).len();
    stage.press("right-paddle-bottom");
    stage.settle(2.5);
    let after = stage.files(&shots).len();
    ought(after > before, || format!("no picture appeared in {shots}"))
}
