//! L2 and the d-pad move the screen's brightness.

use console_test_stages::checking::{Body, Check, CheckResult};
use console_test_stages::device::{Device, Way};
use console_test_stages::here::Here;

const UNSAID: &str = "the machine would not say how bright the screen is";

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

fn brighter_here(stage: &mut Here) -> CheckResult {
    stage.trigger("l2", 1.0)?;
    stage.press("dpad-right")?;
    stage.ran(&[&["/usr/local/bin/console-brightness", "up"]])
}

fn brighter_there(stage: &mut Device) -> CheckResult {
    stage.trigger("l2", 1.0)?;

    let Ok(_) = stage.stepped("dpad-left", Device::brightness);
    let Ok(was) = stage.stepped("dpad-right", Device::brightness);

    stage.trigger("l2", 0.0)?;

    let Ok(now) = stage.brightness();

    now.went(Way::Up, was, UNSAID)
}

fn dimmer_here(stage: &mut Here) -> CheckResult {
    stage.trigger("l2", 1.0)?;
    stage.press("dpad-left")?;
    stage.ran(&[&["/usr/local/bin/console-brightness", "down"]])
}

fn dimmer_there(stage: &mut Device) -> CheckResult {
    stage.trigger("l2", 1.0)?;

    let Ok(_) = stage.stepped("dpad-right", Device::brightness);
    let Ok(was) = stage.stepped("dpad-left", Device::brightness);
    let Ok(now) = stage.stepped("dpad-right", Device::brightness);

    stage.trigger("l2", 0.0)?;

    now.went(Way::Down, was, UNSAID)
}
