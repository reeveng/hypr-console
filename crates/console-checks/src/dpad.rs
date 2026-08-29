//! The d-pad on its own moves between things and does nothing else.

use console_stage::checking::{Body, Check, Done, ought};
use console_stage::device::Device;
use console_stage::here::{Here, TURNS};

/// Every way it points.
const WAYS: [&str; 4] = ["dpad-down", "dpad-left", "dpad-right", "dpad-up"];

pub const DPAD: Check = Check {
    name: "100-the-dpad-does-not-act",
    about: "The d-pad on its own moves between things and does nothing else.",
    feature: "dpad",
    since: "2026-08-26",
    bodies: &[Body::Here(here), Body::Device(there)],
};

fn here(stage: &mut Here) -> Done {
    for way in WAYS {
        stage.press(way)?;
    }
    stage.settle(TURNS);
    let ran = stage.names();
    ought(ran.is_empty(), || format!("the d-pad ran {ran:?}"))
}

/// On the desktop the d-pad is the arrow keys, which move a selection inside
/// whatever has focus. What it must not do is move the desktop.
fn there(stage: &mut Device) -> Done {
    let (where_, windows) = (stage.workspace(), stage.windows());
    for way in WAYS {
        stage.press(way);
    }
    stage.settle(1.2);
    let now = stage.workspace();
    ought(now == where_, || format!("the d-pad moved the desktop to {now}"))?;
    ought(stage.windows() == windows, || "the d-pad opened or closed something".to_string())
}
