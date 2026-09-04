//! L2 and the d-pad move the screen's brightness.

use console_stage::checking::{Body, Check, Done, less_than, more_than, same};
use console_stage::device::Device;
use console_stage::here::{Here, TURNS};

pub const BRIGHTER: Check = Check {
    name: "090-brighter",
    about: "L2 and the d-pad right make the screen brighter.",
    feature: "brightness",
    since: "2026-08-26",
    bodies: &[Body::Here(brighter_here), Body::Device(brighter_there)],
};

pub const DIMMER: Check = Check {
    name: "091-dimmer",
    about: "L2 and the d-pad left make it darker.",
    feature: "brightness",
    since: "2026-08-26",
    bodies: &[Body::Here(dimmer_here), Body::Device(dimmer_there)],
};

fn brighter_here(stage: &mut Here) -> Done {
    stage.trigger("l2", 1.0)?;
    stage.press("dpad-right")?;
    stage.settle(TURNS);
    let ran = stage.commands().to_vec();
    same(&ran, &[["/usr/local/bin/console-brightness", "up"]], || format!("it ran {ran:?}"))
}

/// Brightness has a ceiling and the screen usually sits on it.
///
/// `console-brightness` clamps, so on a screen already at the top there is no
/// higher number to arrive at, and asserting that one does fails on a machine
/// doing exactly what it should. Room is made first, by one step down, and the
/// step up gives it back: the screen ends where it was found.
///
/// What full is stays the script's to know. A number here would be a second
/// opinion about this panel, and two numbers about one screen part company the
/// day either of them moves.
fn brighter_there(stage: &mut Device) -> Done {
    stage.trigger("l2", 1.0)?;
    stage.press("dpad-left");
    stage.settle(1.0);
    let was = stage.brightness();
    stage.press("dpad-right");
    stage.settle(1.0);
    stage.trigger("l2", 0.0)?;
    let now = stage.brightness();
    more_than(now, was, || format!("it was {was} and is {now}"))
}

fn dimmer_here(stage: &mut Here) -> Done {
    stage.trigger("l2", 1.0)?;
    stage.press("dpad-left")?;
    stage.settle(TURNS);
    let ran = stage.commands().to_vec();
    same(&ran, &[["/usr/local/bin/console-brightness", "down"]], || format!("it ran {ran:?}"))
}

/// The floor is a clamp as much as the ceiling is.
///
/// So room is made above before the screen is asked to fall, and given back
/// afterwards, before the assertion rather than after it, so that a screen
/// somebody is reading by is left alone whichever way this ends.
///
/// Both numbers are said. "still at 64000" was the value from before the press,
/// which made a clamp and a press that never arrived read alike, and two checks
/// went undiagnosed on it for days.
fn dimmer_there(stage: &mut Device) -> Done {
    stage.trigger("l2", 1.0)?;
    stage.press("dpad-right");
    stage.settle(1.0);
    let was = stage.brightness();
    stage.press("dpad-left");
    stage.settle(1.0);
    let now = stage.brightness();
    stage.press("dpad-right");
    stage.settle(1.0);
    stage.trigger("l2", 0.0)?;
    less_than(now, was, || format!("it was {was} and is {now}"))
}
