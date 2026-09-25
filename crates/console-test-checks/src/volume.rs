//! L2 and the d-pad move the volume.
//!
//! On the device the check starts at the bottom of the range rather than
//! where it found the level: louder from silence, quieter from one per cent.
//! Somebody may be listening to a song while the checks run, and a step up
//! and back from wherever they left it is heard twice. The run puts their
//! level back after the last check.

use console_test_stages::checking::{Body, Check, CheckResult};
use console_test_stages::device::{Device, Way};
use console_test_stages::here::Here;

const UNSAID: &str = "the machine would not say how loud it is";

const SILENT: i64 = 0;

const BARELY: i64 = 1;

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

fn louder_here(stage: &mut Here) -> CheckResult {
    stage.trigger("l2", 1.0)?;
    stage.press("dpad-up")?;
    stage.ran(&[&["/usr/local/bin/console-volume", "up"]])
}

fn louder_there(stage: &mut Device) -> CheckResult {
    stepped(stage, Step { from: SILENT, button: "dpad-up", way: Way::Up })
}

fn quieter_here(stage: &mut Here) -> CheckResult {
    stage.trigger("l2", 1.0)?;
    stage.press("dpad-down")?;
    stage.ran(&[&["/usr/local/bin/console-volume", "down"]])
}

fn quieter_there(stage: &mut Device) -> CheckResult {
    stepped(stage, Step { from: BARELY, button: "dpad-down", way: Way::Down })
}

struct Step {
    from: i64,
    button: &'static str,
    way: Way,
}

fn stepped(stage: &mut Device, Step { from, button, way }: Step) -> CheckResult {
    stage.trigger("l2", 1.0)?;

    let Ok(()) = stage.volume_to(from);
    let Ok(was) = stage.stepped(button, Device::volume);

    stage.trigger("l2", 0.0)?;

    let Ok(now) = stage.volume();

    now.went(way, was, UNSAID)
}
