//! L2 and the d-pad move the screen's brightness.

use console_test_stages::checking::{Body, Check, Done, less_than, more_than, same};
use console_test_stages::device::Device;
use console_test_stages::here::{Here, TURNS};

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
    let Ok(()) = stage.settle(TURNS);
    let Ok(commands) = stage.commands();
    let ran = commands.to_vec();

    same(&ran, &[["/usr/local/bin/console-brightness", "up"]], || format!("it ran {ran:?}"))
}

fn brighter_there(stage: &mut Device) -> Done {
    stage.trigger("l2", 1.0)?;
    let Ok(()) = stage.press("dpad-left");
    let Ok(()) = stage.settle(1.0);
    let Ok(was) = stage.brightness();
    let Ok(()) = stage.press("dpad-right");
    let Ok(()) = stage.settle(1.0);
    stage.trigger("l2", 0.0)?;
    let Ok(now) = stage.brightness();
    more_than(now, was, || format!("it was {was} and is {now}"))
}

fn dimmer_here(stage: &mut Here) -> Done {
    stage.trigger("l2", 1.0)?;
    stage.press("dpad-left")?;
    let Ok(()) = stage.settle(TURNS);
    let Ok(commands) = stage.commands();
    let ran = commands.to_vec();

    same(&ran, &[["/usr/local/bin/console-brightness", "down"]], || format!("it ran {ran:?}"))
}

fn dimmer_there(stage: &mut Device) -> Done {
    stage.trigger("l2", 1.0)?;
    let Ok(()) = stage.press("dpad-right");
    let Ok(()) = stage.settle(1.0);
    let Ok(was) = stage.brightness();
    let Ok(()) = stage.press("dpad-left");
    let Ok(()) = stage.settle(1.0);
    let Ok(now) = stage.brightness();
    let Ok(()) = stage.press("dpad-right");
    let Ok(()) = stage.settle(1.0);
    stage.trigger("l2", 0.0)?;
    less_than(now, was, || format!("it was {was} and is {now}"))
}
