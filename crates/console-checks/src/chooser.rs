//! What every chooser does: it draws, it takes the pad, and B gives it back.
//!
//! The menu, the guide and the settings panel are three programs and one
//! contract. Written once here so a fourth chooser is a line of data rather
//! than a fourth copy of the same four assertions.

use console_stage::checking::{Done, ought};
use console_stage::device::{Device, PATIENCE};

/// The profiles a chooser puts the pad into while it is up.
pub const HOLDING: [&str; 2] = ["Menu", "Tabs"];

/// Press it, and ask everything a chooser is asked.
pub fn opens(stage: &mut Device, button: &str, what: &str) -> Done {
    stage.press(button);
    ought(stage.drawn(PATIENCE), || format!("the {what} did not draw"))?;
    let held = stage.profile();
    ought(HOLDING.contains(&held.as_str()), || {
        format!("the {what} did not take the pad; profile is {held}")
    })?;
    closes(stage, what)
}

/// B closes it, and the pad comes back.
pub fn closes(stage: &mut Device, what: &str) -> Done {
    stage.press("b");
    ought(stage.gone(PATIENCE), || {
        let left = stage.menus();
        format!("B did not close the {what}: {left:?}")
    })?;
    let held = stage.profile();
    ought(held == "Desktop", || format!("the pad was not handed back; profile is {held}"))
}
