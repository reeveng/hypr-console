//! The Menu button opens the guide to what every button does.

use console_test_stages::checking::{Body, Check, Done, same};
use console_test_stages::device::Device;
use console_test_stages::here::{Here, TURNS};

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
    let Ok(()) = stage.settle(TURNS);
    let Ok(commands) = stage.commands();
    let ran = commands.to_vec();

    same(&ran, &[["/usr/local/bin/console-buttons", "--menu"]], || format!("it ran {ran:?}"))
}

fn there(stage: &mut Device) -> Done {
    opens(stage, "menu", "guide")
}
