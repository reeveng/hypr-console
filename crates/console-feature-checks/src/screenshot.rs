//! Held with L2, the bottom right paddle takes a screenshot.

use console_test_stages::checking::{Body, Check, Done, empty, more_than, same};
use console_test_stages::device::Device;
use console_test_stages::here::{Here, TURNS};

pub const SHOT: Check = Check {
    name: "070-a-screenshot",
    about: "Held with L2, the bottom right paddle takes a screenshot.",
    feature: "screenshot",
    since: "2026-08-26",
    bodies: &[Body::Here(here), Body::Device(there)],
};

pub const ALONE: Check = Check {
    name: "071-the-paddle-alone-takes-nothing",
    about: "Without L2, the same paddle takes no picture at all.",
    feature: "screenshot",
    since: "2026-09-01",
    bodies: &[Body::Here(alone_here), Body::Device(alone_there)],
};

fn here(stage: &mut Here) -> Done {
    stage.trigger("l2", 1.0)?;
    stage.press("right-paddle-bottom")?;

    let Ok(()) = stage.settle(TURNS);
    let Ok(commands) = stage.commands();
    let ran = commands.to_vec();

    same(&ran, &[["/usr/local/bin/console-screenshot"]], || format!("it ran {ran:?}"))
}

fn alone_here(stage: &mut Here) -> Done {
    stage.press("right-paddle-bottom")?;

    let Ok(()) = stage.settle(TURNS);
    let Ok(commands) = stage.commands();
    let ran = commands.to_vec();

    empty(&ran, || format!("it ran {ran:?}"))
}

fn there(stage: &mut Device) -> Done {
    let Ok(home) = stage.home();
    let shots = format!("{home}/Pictures");
    let Ok(was) = stage.files(&shots);
    let before = was.len();

    stage.trigger("l2", 1.0)?;

    let Ok(()) = stage.press("right-paddle-bottom");

    stage.trigger("l2", 0.0)?;

    let Ok(()) = stage.settle(2.5);
    let Ok(now) = stage.files(&shots);

    more_than(now.len(), before, || format!("no picture appeared in {shots}"))
}

fn alone_there(stage: &mut Device) -> Done {
    let Ok(home) = stage.home();
    let shots = format!("{home}/Pictures");

    stage.trigger("l2", 0.0)?;

    let Ok(was) = stage.files(&shots);
    let before = was.len();

    let Ok(()) = stage.press("right-paddle-bottom");
    let Ok(()) = stage.settle(2.5);
    let Ok(now) = stage.files(&shots);

    same(&now.len(), &before, || format!("a picture arrived in {shots} unasked"))
}
