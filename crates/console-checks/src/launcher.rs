//! The top left paddle opens the menu.

use console_stage::checking::{Body, Check, Done, same};
use console_stage::device::Device;
use console_stage::here::{Here, TURNS};

use crate::chooser::opens;

pub const MENU: Check = Check {
    name: "040-the-menu",
    about: "The top left paddle opens the menu.",
    feature: "launcher",
    since: "2026-08-24",
    bodies: &[Body::Here(here), Body::Device(there)],
};

fn here(stage: &mut Here) -> Done {
    stage.press("left-paddle-top")?;
    stage.settle(TURNS);
    let ran = stage.names();
    same(&ran, &["launcher"], || format!("it ran {ran:?}"))
}

fn there(stage: &mut Device) -> Done {
    opens(stage, "left-paddle-top", "chooser")
}
