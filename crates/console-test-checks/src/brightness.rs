//! L2 and the d-pad move the screen's brightness.

use console_test_stages::checking::{Body, Check, Done, failed, less_than, more_than, same};
use console_test_stages::device::{Device, PATIENCE};
use console_test_stages::here::{Here, TURNS};

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

    let Ok(started) = stage.brightness();
    let Ok(()) = stage.press("dpad-left");
    let Ok(_) = stage.changed(Device::brightness, &started, PATIENCE);
    let Ok(was) = stage.brightness();
    let Ok(()) = stage.press("dpad-right");
    let Ok(_) = stage.changed(Device::brightness, &was, PATIENCE);

    stage.trigger("l2", 0.0)?;

    let Ok(now) = stage.brightness();

    let (was, now) = match (was.told(), now.told()) {
        (Ok(Some(was)), Ok(Some(now))) => (was, now),
        (Ok(None) | Err(_), _) | (_, Ok(None) | Err(_)) => return failed(UNSAID.to_string()),
    };

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

    let Ok(started) = stage.brightness();
    let Ok(()) = stage.press("dpad-right");
    let Ok(_) = stage.changed(Device::brightness, &started, PATIENCE);
    let Ok(was) = stage.brightness();
    let Ok(()) = stage.press("dpad-left");
    let Ok(_) = stage.changed(Device::brightness, &was, PATIENCE);
    let Ok(now) = stage.brightness();
    let Ok(()) = stage.press("dpad-right");
    let Ok(_) = stage.changed(Device::brightness, &now, PATIENCE);

    stage.trigger("l2", 0.0)?;

    let (was, now) = match (was.told(), now.told()) {
        (Ok(Some(was)), Ok(Some(now))) => (was, now),
        (Ok(None) | Err(_), _) | (_, Ok(None) | Err(_)) => return failed(UNSAID.to_string()),
    };

    less_than(now, was, || format!("it was {was} and is {now}"))
}
