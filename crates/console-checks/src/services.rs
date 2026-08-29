//! Every service the desktop is made of is running.

use console_stage::checking::{Body, Check, Done, ought};
use console_stage::device::Device;

pub const SERVICES: Check = Check {
    name: "140-the-desktop-is-up",
    about: "Every service the desktop is made of is running.",
    feature: "services",
    since: "2026-08-24",
    bodies: &[Body::Device(there)],
};

fn there(stage: &mut Device) -> Done {
    let states = stage.services();
    ought(
        !states.is_empty() && states.iter().all(|state| state == "active"),
        || format!("the desktop is missing a piece: {states:?}"),
    )
}

/// Nothing has died and been started again.
///
/// The check above cannot see this. Every service here restarts itself, so one
/// that dies every few minutes is `active` at almost every moment somebody
/// looks, and a desktop repairing itself all afternoon reads exactly like a
/// desktop that is well. The wallpaper daemon spent a day doing it before
/// anybody counted, and what said so in the end was this number.
///
/// A restart is also how the desktop is meant to survive a fault, so this is
/// not a fault in itself. It is the thing that has to be visible for somebody
/// to decide that.
pub const STEADY: Check = Check {
    name: "210-nothing-has-had-to-be-started-again",
    about: "No piece of the desktop has died and been started again since it came up.",
    feature: "services",
    since: "2026-08-29",
    bodies: &[Body::Device(steady)],
};

fn steady(stage: &mut Device) -> Done {
    let counts = stage.restarts();
    ought(
        !counts.is_empty() && counts.iter().all(|count| count == "0"),
        || format!("a piece of the desktop has been dying and starting again: {counts:?}"),
    )
}
