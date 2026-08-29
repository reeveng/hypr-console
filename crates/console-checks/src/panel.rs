//! The settings panel: it opens, it draws, and it lets go.

use std::collections::BTreeSet;

use console_stage::checking::{Body, Check, Done, ought};
use console_stage::desktop::Desktop;
use console_stage::device::{Device, PATIENCE};
use console_stage::here::{Here, TURNS};
use console_stage::palette::palette;

use crate::chooser::opens;

pub const PANEL: Check = Check {
    name: "080-the-panel",
    about: "Legion right opens the settings panel.",
    feature: "panel",
    since: "2026-08-28",
    bodies: &[Body::Here(here), Body::Device(there)],
};

/// This is the check that was missing when the panel raised before drawing
/// anything and the whole suite stayed green. Nothing else builds a panel:
/// building one wants a compositor, so a file that could not survive its own
/// first screenful passed everything there was to pass.
pub const DRAWS: Check = Check {
    name: "160-the-panel-draws",
    about: "The settings panel opens, and draws itself.",
    feature: "panel",
    since: "2026-08-28",
    bodies: &[Body::Desktop(draws)],
};

pub const WITH_THE_KEYBOARD: Check = Check {
    name: "180-out-of-a-panel-with-the-keyboard-up",
    about: "B closes a panel with the keyboard over it, and leaves the pad usable.",
    feature: "panel",
    since: "2026-08-28",
    bodies: &[Body::Device(with_the_keyboard)],
};

/// Down the left of the panel, past the tab strip and through the rows, in the
/// coordinates the compositor lays out in. A band rather than a point: the
/// question is whether the panel is there at all, and a point is a question
/// about where a row happens to be.
///
/// Shared, because every panel on this device is the same shape. The menu, the
/// settings, the guide and the files are one card at one size, worked out in
/// `console_panel::shape`, and a second copy of this band would be a second
/// opinion about where that card is.
pub const ACROSS: f64 = 200.0;
pub const DOWN: std::ops::Range<i32> = 150..520;
pub const EVERY: usize = 6;

/// Whether a band down a panel has the panel's own colours in it.
///
/// Two questions in one: that something was drawn where the card should be, and
/// that a row of it is highlighted. A panel that raises without drawing is the
/// fault this shape of check exists for, and it looks exactly like a panel that
/// drew nothing but its ground.
pub fn drew(stage: &mut Desktop) -> Done {
    let wanted = palette();
    let down: BTreeSet<String> = DOWN
        .step_by(EVERY)
        .map(|y| stage.colour(ACROSS, f64::from(y)))
        .collect::<Result<_, _>>()?;

    let is = |name: &str| down.contains(&wanted[name]);
    ought(is("panel") || is("ground"), || {
        format!("nothing of the panel is on the screen where it should be: {down:?}")
    })?;
    ought(is("pink"), || format!("the panel drew but nothing on it is highlighted: {down:?}"))
}

fn here(stage: &mut Here) -> Done {
    stage.press("legion-right")?;
    stage.settle(TURNS);
    let ran = stage.names();
    ought(ran == ["settings-panel"], || format!("it ran {ran:?}"))
}

fn there(stage: &mut Device) -> Done {
    opens(stage, "legion-right", "panel")
}

fn draws(stage: &mut Desktop) -> Done {
    stage.open("settings-panel Sound")?;
    drew(stage)
}

/// The keyboard is over the panel and B still means back.
///
/// Nothing translates it: while wvkbd is up the keyboard profile maps nothing,
/// so B arrives as the keyboard's backspace, and the panel, which holds the
/// keyboard focus, reads backspace as back. The thumb's habit works without
/// anybody being told.
///
/// Two faults hid behind each other here. The panel used to be stopped for as
/// long as the keyboard was up, since the signal that takes the pad from the
/// daemon went to everything in its control group, so the press was answered a
/// minute later when the keyboard came down. And a panel that closes under the
/// keyboard puts the desktop back as it goes, which the hook then covered with
/// the panel's own profile: the pad answered to a panel that was not there and
/// the button drew nothing. So this presses the way out and then asks whether
/// the machine still works.
fn with_the_keyboard(stage: &mut Device) -> Done {
    stage.press("legion-right");
    ought(stage.drawn(PATIENCE), || "the panel did not draw".to_string())?;

    stage.press("x");
    ought(stage.until(Device::keyboard, PATIENCE), || {
        "the keyboard did not come up over the panel".to_string()
    })?;
    ought(!stage.menus().is_empty(), || "the keyboard came up and the panel went".to_string())?;

    stage.press("b");
    ought(stage.gone(PATIENCE), || "B did not close the panel".to_string())?;

    stage.press("x");
    ought(stage.until(|seen| !seen.keyboard(), PATIENCE), || {
        "the keyboard would not go away".to_string()
    })?;
    let held = stage.profile();
    ought(held == "Desktop", || {
        format!("the pad still answers to the panel that closed; profile is {held}")
    })?;

    stage.press("legion-right");
    ought(stage.drawn(PATIENCE), || "the settings button stopped drawing anything".to_string())?;
    stage.press("b");
    ought(stage.gone(PATIENCE), || "the panel would not close again".to_string())
}
