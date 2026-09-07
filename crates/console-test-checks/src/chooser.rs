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

use console_test_stages::checking::{Done, happened, same};
use console_test_stages::device::{Device, PATIENCE};

pub const WORN: &str = "Router";

pub fn opens(stage: &mut Device, button: &str, what: &str) -> Done {
    let Ok(()) = stage.press(button);
    let Ok(drawn) = stage.drawn(PATIENCE);

    happened(drawn, || format!("the {what} did not draw"))?;

    let Ok(held) = stage.profile();

    same(&held, WORN, || {
        format!("the {what} changed the profile to {held}, and a chooser has nothing to change")
    })?;

    closes(stage, what)
}

pub fn closes(stage: &mut Device, what: &str) -> Done {
    let Ok(()) = stage.press("b");
    let Ok(gone) = stage.gone(PATIENCE);

    happened(gone, || {
        let Ok(left) = stage.menus();

        format!("B did not close the {what}: {left:?}")
    })?;

    let Ok(held) = stage.profile();

    same(&held, WORN, || format!("the {what} left the pad wearing {held}"))
}
