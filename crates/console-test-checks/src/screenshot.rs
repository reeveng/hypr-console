//! Held with L2, the bottom right paddle takes a screenshot.
//!
//! The picture it takes it takes away again. What it asserts is that one
//! arrived, and a picture of somebody's desktop, taken by a machine they lent
//! out, in the folder they keep their own in, is the check leaving a thing
//! behind for them to find and delete. Only what appeared while it was looking
//! is removed, and only by the name the folder gave it, so nothing that was
//! there before is touched.

use console_test_stages::checking::{Body, Check, Done, empty, more_than, same};
use console_test_stages::device::{A_PICTURE, Device, Seen, quoted};
use console_test_stages::here::{Here, TURNS};
use console_core_never::Never;

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

    let Ok(_) = stage.until::<Never>(
        |seen| {
            let Ok(now) = seen.files(&shots);

            Ok(match now.len() > before {
                true => Seen::Yes,
                false => Seen::NotYet,
            })
        },
        A_PICTURE,
    );

    let Ok(now) = stage.files(&shots);
    let Ok(()) = taken_away(stage, &shots, &was, &now);

    more_than(now.len(), before, || format!("no picture appeared in {shots}"))
}

fn taken_away(
    stage: &mut Device,
    shots: &str,
    was: &[String],
    now: &[String],
) -> Result<(), Never> {
    for name in now.iter().filter(|name| !was.contains(name)) {
        let Ok(quoted) = quoted(&format!("{shots}/{name}"));
        let Ok(_) = stage.user(&format!("rm -f {quoted}"));
    }

    Ok(())
}

fn alone_there(stage: &mut Device) -> Done {
    let Ok(home) = stage.home();
    let shots = format!("{home}/Pictures");

    stage.trigger("l2", 0.0)?;

    let Ok(was) = stage.files(&shots);
    let before = was.len();

    let Ok(()) = stage.press("right-paddle-bottom");

    #[cfg_attr(
        dylint_lib = "explicit022_no_settling",
        allow(
            explicit022_no_settling,
            reason = "what is being checked is that no picture arrives, and a picture not arriving has no question to ask: the number is the same patience the check above gives one that should"
        )
    )]
    let Ok(()) = stage.settle(A_PICTURE);

    let Ok(now) = stage.files(&shots);

    same(&now.len(), &before, || format!("a picture arrived in {shots} unasked"))
}
