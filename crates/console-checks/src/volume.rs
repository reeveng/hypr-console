//! L2 and the d-pad move the volume.

use console_stage::checking::{Body, Check, Done, ought};
use console_stage::device::Device;
use console_stage::here::{Here, TURNS};

pub const LOUDER: Check = Check {
    name: "092-louder",
    about: "L2 and the d-pad up make it louder.",
    feature: "volume",
    since: "2026-09-01",
    bodies: &[Body::Here(louder_here), Body::Device(louder_there)],
};

pub const QUIETER: Check = Check {
    name: "093-quieter",
    about: "L2 and the d-pad down make it quieter.",
    feature: "volume",
    since: "2026-09-01",
    bodies: &[Body::Here(quieter_here), Body::Device(quieter_there)],
};

fn louder_here(stage: &mut Here) -> Done {
    stage.trigger("l2", 1.0)?;
    stage.press("dpad-up")?;
    stage.settle(TURNS);
    let ran = stage.commands().to_vec();
    ought(ran == [["/usr/local/bin/console-volume", "up"]], || format!("it ran {ran:?}"))
}

/// The volume has a ceiling and a machine sitting on it has no higher number
/// to arrive at, so a step down is taken first and the step up being checked is
/// what gives it back. It ends where it was found, which is why nothing is put
/// back after the reading.
///
/// Turning it up unsilences it, because that is what turning it up means here
/// (`console_settings::rocker`). So this does leave a muted device unmuted.
/// That is the feature being checked rather than a mess the check made, and it
/// is said out loud because it is the one thing here that does not put the
/// machine back exactly as it was.
fn louder_there(stage: &mut Device) -> Done {
    stage.trigger("l2", 1.0)?;
    stage.press("dpad-down");
    stage.settle(1.0);
    let was = stage.volume();
    stage.press("dpad-up");
    stage.settle(1.0);
    stage.trigger("l2", 0.0)?;
    let now = stage.volume();
    ought(now > was, || format!("it was {was} and is {now}"))
}

fn quieter_here(stage: &mut Here) -> Done {
    stage.trigger("l2", 1.0)?;
    stage.press("dpad-down")?;
    stage.settle(TURNS);
    let ran = stage.commands().to_vec();
    ought(ran == [["/usr/local/bin/console-volume", "down"]], || format!("it ran {ran:?}"))
}

/// The floor is a clamp as much as the ceiling is, so room is made above first
/// and the step down being checked is what spends it. Nothing is restored
/// afterwards because there is nothing left to restore: the machine is already
/// back where it was found by the time the reading is taken, which is what a
/// check run on a device somebody is listening to has to be able to say.
///
/// Both numbers are said. "still at 45" was the value from before the press,
/// which made a machine already at the floor and a press that never arrived
/// read exactly alike.
fn quieter_there(stage: &mut Device) -> Done {
    stage.trigger("l2", 1.0)?;
    stage.press("dpad-up");
    stage.settle(1.0);
    let was = stage.volume();
    stage.press("dpad-down");
    stage.settle(1.0);
    stage.trigger("l2", 0.0)?;
    let now = stage.volume();
    ought(now < was, || format!("it was {was} and is {now}"))
}
