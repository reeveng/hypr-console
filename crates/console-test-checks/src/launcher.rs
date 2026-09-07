//! The top left paddle opens the menu.

use console_test_stages::checking::{Body, Check, Done, same};
use console_test_stages::device::Device;
use console_test_stages::here::{Here, TURNS};

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
    let Ok(()) = stage.settle(TURNS);
    let Ok(ran) = stage.names();

    same(&ran, &["launcher"], || format!("it ran {ran:?}"))
}

fn there(stage: &mut Device) -> Done {
    opens(stage, "left-paddle-top", "chooser")
}
