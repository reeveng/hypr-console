//! The settings panel: it opens, it draws, and it lets go.

use std::collections::BTreeSet;

use console_stage::checking::{Body, Check, Done, ought};
use console_stage::desktop::Desktop;
use console_stage::here::{Here, TURNS};
use console_stage::device::{Device, PATIENCE};
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
    bodies: &[Body::Here(without_a_screen), Body::Device(with_the_keyboard)],
};

/// What the compositor says while a keyboard is up over a panel, and again
/// once the panel has gone from under it.
///
/// The device's own words, kept short. What matters is which namespaces are
/// listed and that each has a height, because a layer with no height is a
/// keyboard that is started hidden and stays for the session.
const OVER_A_PANEL: &str = r#"{"eDP-1":{"levels":{
    "0":[{"namespace":"awww-daemon","h":1600}],
    "2":[{"namespace":"waybar","h":38}],
    "3":[{"namespace":"settings-panel","h":1562},{"namespace":"wvkbd-mobintl","h":520}]}}}"#;

const THE_PANEL_ALONE: &str = r#"{"eDP-1":{"levels":{
    "0":[{"namespace":"awww-daemon","h":1600}],
    "2":[{"namespace":"waybar","h":38}],
    "3":[{"namespace":"settings-panel","h":1562}]}}}"#;

const NOTHING_UP: &str = r#"{"eDP-1":{"levels":{
    "0":[{"namespace":"awww-daemon","h":1600}],
    "2":[{"namespace":"waybar","h":38}]}}}"#;

/// The same thing the device is asked, without a device.
///
/// This was on hardware only, and it is the check for the fault that took the
/// longest to find: a panel closed while the keyboard was over it had already
/// put the desktop back, and the hook then laid the remembered profile over
/// that, leaving the pad answering to a panel that was gone.
///
/// It can be asked here because the mode is read rather than remembered. The
/// compositor's answer is a string, `Mode::seen` turns it into where you are,
/// and everything after that -- whether this daemon acts, and which profile
/// the pad wants -- is arithmetic on that. So the whole of it is askable with
/// no compositor, no InputPlumber and no keyboard, against the real profiles
/// and the real pad.
///
/// What it cannot ask is whether wvkbd actually drew, whether X reached it, or
/// whether the pad survived the profile switch. Those are the device's, and
/// `with_the_keyboard` is still the one that asks them.
fn without_a_screen(stage: &mut Here) -> Done {
    stage.showing(THE_PANEL_ALONE)?;
    ought(stage.wanted() == console_pad::router::NAME, || {
        format!("a panel up wants the {} profile", stage.wanted())
    })?;

    // The keyboard over it. Both read the same pad, so the daemon stands down
    // -- which is what replaced one program sending another SIGSTOP.
    stage.showing(OVER_A_PANEL)?;
    stage.press("legion-right")?;
    stage.press("b")?;
    stage.settle(TURNS);
    ought(stage.commands().is_empty(), || {
        format!("the daemon acted under the keyboard: {:?}", stage.commands())
    })?;

    // The panel goes from under the keyboard. Nothing is restored, because
    // nothing was remembered: what the pad wants is what is in front now. It
    // is the same profile it was wearing under the panel, which is the whole
    // of why opening a menu no longer destroys the pad and builds another --
    // what a button means with a chooser up is the daemon's to say.
    stage.showing(NOTHING_UP)?;
    ought(stage.wanted() == console_pad::router::NAME, || {
        format!("the keyboard went and the pad wants {}", stage.wanted())
    })?;

    // And with the keyboard gone the daemon is reading again, or standing down
    // was a way of never starting.
    let before = stage.commands().len();
    stage.press("legion-right")?;
    stage.settle(TURNS);
    ought(stage.commands().len() > before, || {
        "the keyboard went and the daemon never started acting again".to_string()
    })
}

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
    ought(held == crate::chooser::WORN, || {
        format!("the keyboard went and the pad was left wearing {held}")
    })?;

    stage.press("legion-right");
    ought(stage.drawn(PATIENCE), || "the settings button stopped drawing anything".to_string())?;
    stage.press("b");
    ought(stage.gone(PATIENCE), || "the panel would not close again".to_string())
}
