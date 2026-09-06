//! L2 and the d-pad move the volume.

use console_test_stages::checking::{Body, Check, Done, less_than, more_than, same};
use console_test_stages::device::Device;
use console_test_stages::here::{Here, TURNS};

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
    let Ok(()) = stage.settle(TURNS);
    let Ok(commands) = stage.commands();
    let ran = commands.to_vec();

    same(&ran, &[["/usr/local/bin/console-volume", "up"]], || format!("it ran {ran:?}"))
}

fn louder_there(stage: &mut Device) -> Done {
    stage.trigger("l2", 1.0)?;
    let Ok(()) = stage.press("dpad-down");
    let Ok(()) = stage.settle(1.0);
    let Ok(was) = stage.volume();
    let Ok(()) = stage.press("dpad-up");
    let Ok(()) = stage.settle(1.0);
    stage.trigger("l2", 0.0)?;
    let Ok(now) = stage.volume();
    more_than(now, was, || format!("it was {was} and is {now}"))
}

fn quieter_here(stage: &mut Here) -> Done {
    stage.trigger("l2", 1.0)?;
    stage.press("dpad-down")?;
    let Ok(()) = stage.settle(TURNS);
    let Ok(commands) = stage.commands();
    let ran = commands.to_vec();

    same(&ran, &[["/usr/local/bin/console-volume", "down"]], || format!("it ran {ran:?}"))
}

fn quieter_there(stage: &mut Device) -> Done {
    stage.trigger("l2", 1.0)?;
    let Ok(()) = stage.press("dpad-up");
    let Ok(()) = stage.settle(1.0);
    let Ok(was) = stage.volume();
    let Ok(()) = stage.press("dpad-down");
    let Ok(()) = stage.settle(1.0);
    stage.trigger("l2", 0.0)?;
    let Ok(now) = stage.volume();
    less_than(now, was, || format!("it was {was} and is {now}"))
}
