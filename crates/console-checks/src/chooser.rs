//! What every chooser does: it draws, it leaves the pad alone, and B closes it.
//!
//! The menu, the guide and the settings panel are three programs and one
//! contract. Written once here so a fourth chooser is a line of data rather
//! than a fourth copy of the same four assertions.
//!
//! The middle assertion used to be the other way round. A chooser took the pad
//! -- loaded a profile of its own on the way in and put the desktop's back on
//! the way out -- and this file watched it happen. Every one of those swaps
//! destroyed the pad and built another, which is the fault most of this
//! repository's comments are about, so what is asked now is that opening a
//! menu changes nothing about what the machine is wearing.

use console_stage::checking::{Done, ought};
use console_stage::device::{Device, PATIENCE};

/// What the pad wears on the desktop, in a menu, and everywhere between.
pub const WORN: &str = "Router";

/// Press it, and ask everything a chooser is asked.
pub fn opens(stage: &mut Device, button: &str, what: &str) -> Done {
    stage.press(button);
    ought(stage.drawn(PATIENCE), || format!("the {what} did not draw"))?;
    let held = stage.profile();
    ought(held == WORN, || {
        format!("the {what} changed the profile to {held}, and a chooser has nothing to change")
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
    ought(held == WORN, || format!("the {what} left the pad wearing {held}"))
}
