//! Every service the desktop is made of is running.

use console_test_stages::checking::{Body, Check, Done, every};
use console_test_stages::device::Device;

pub const SERVICES: Check = Check {
    name: "140-the-desktop-is-up",
    about: "Every service the desktop is made of is running.",
    feature: "services",
    since: "2026-08-24",
    bodies: &[Body::Device(there)],
};

fn there(stage: &mut Device) -> Done {
    let Ok(states) = stage.services();

    every(&states, "active", || format!("the desktop is missing a piece: {states:?}"))
}

pub const STEADY: Check = Check {
    name: "210-nothing-has-had-to-be-started-again",
    about: "No piece of the desktop has died and been started again since it came up.",
    feature: "services",
    since: "2026-08-29",
    bodies: &[Body::Device(steady)],
};

fn steady(stage: &mut Device) -> Done {
    let Ok(counts) = stage.restarts();

    every(&counts, "0", || {
        format!("a piece of the desktop has been dying and starting again: {counts:?}")
    })
}
