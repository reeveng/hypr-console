//! The Menu button opens the guide to what every button does.

use console_stage::checking::{Body, Check, Done, same};
use console_stage::device::Device;
use console_stage::here::{Here, TURNS};

use crate::chooser::opens;

pub const GUIDE: Check = Check {
    name: "050-the-guide",
    about: "The Menu button opens the guide to what every button does.",
    feature: "guide",
    since: "2026-08-26",
    bodies: &[Body::Here(here), Body::Device(there)],
};

fn here(stage: &mut Here) -> Done {
    stage.press("menu")?;
    stage.settle(TURNS);
    let ran = stage.commands().to_vec();
    same(&ran, &[["/usr/local/bin/console-buttons", "--menu"]], || format!("it ran {ran:?}"))
}

/// The guide is a chooser, so the pad goes to the chooser profile and comes
/// back. B is what closes it, which is the contract.
fn there(stage: &mut Device) -> Done {
    opens(stage, "menu", "guide")
}
