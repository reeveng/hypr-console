//! Held with L2, a shoulder carries the window rather than leaving it.

use console_test_stages::checking::{Body, Check, Done, happened, not_same, same};
use console_test_stages::device::{Device, OPENING, PATIENCE};
use console_test_stages::here::{Here, TURNS};

pub const CARRY: Check = Check {
    name: "020-carry-a-window",
    about: "Held with L2, a shoulder carries the window rather than leaving it.",
    feature: "carry",
    since: "2026-08-25",
    bodies: &[Body::Here(carry_here), Body::Device(carry_there)],
};

pub const HALF: Check = Check {
    name: "021-a-half-pull-is-not-a-hold",
    about: "A trigger short of held moves you, and leaves the window where it was.",
    feature: "carry",
    since: "2026-08-25",
    bodies: &[Body::Here(half_here), Body::Device(half_there)],
};

fn carry_here(stage: &mut Here) -> Done {
    stage.trigger("l2", 1.0)?;
    stage.press("r1")?;

    let Ok(()) = stage.settle(TURNS);
    let Ok(asked) = stage.dispatches();

    same(&asked, &[r#"hl.dsp.window.move({workspace = "+1"})"#], || {
        format!("it asked for {asked:?}")
    })
}

fn carry_there(stage: &mut Device) -> Done {
    something_open(stage)?;

    let Ok(where_) = stage.workspace();
    let Ok(set_out) = stage.windows_here();

    stage.trigger("l2", 1.0)?;

    let Ok(()) = stage.press("r1");

    stage.trigger("l2", 0.0)?;

    let Ok(_) = stage.changed(Device::workspace, &where_, PATIENCE);
    let Ok(there) = stage.workspace();
    let Ok(arrived) = stage.windows_here();

    stage.trigger("l2", 1.0)?;

    let Ok(()) = stage.press("l1");

    stage.trigger("l2", 0.0)?;

    let Ok(_) = stage.changed(Device::workspace, &there, PATIENCE);

    not_same(&there, &where_, || "it did not move".to_string())?;

    same(&arrived, &set_out, || format!("{set_out} window(s) set out and {arrived} arrived"))
}

fn half_here(stage: &mut Here) -> Done {
    stage.trigger("l2", 0.4)?;
    stage.press("r1")?;

    let Ok(()) = stage.settle(TURNS);
    let Ok(asked) = stage.dispatches();

    same(&asked, &[r#"hl.dsp.focus({workspace = "+1"})"#], || format!("it asked for {asked:?}"))
}

fn half_there(stage: &mut Device) -> Done {
    something_open(stage)?;

    let Ok(where_) = stage.workspace();

    stage.trigger("l2", 0.4)?;

    let Ok(()) = stage.press("r1");

    stage.trigger("l2", 0.0)?;

    let Ok(moved) = stage.changed(Device::workspace, &where_, PATIENCE);
    let Ok(there) = stage.workspace();
    let Ok(came) = stage.windows_here();
    let Ok(()) = stage.press("l1");
    let Ok(_) = stage.changed(Device::workspace, &there, PATIENCE);

    happened(moved, || format!("a half pull left us on {where_}, and it should move"))?;

    same(&came, &0, || format!("{came} window(s) came along and none should have"))
}

pub fn something_open(stage: &mut Device) -> Done {
    let Ok(here) = stage.windows_here();

    match here > 0 {
        true => Ok(()),
        false => {
            let Ok(waited) = stage.open("alacritty", OPENING);

            happened(waited, || "nothing would open on the device".to_string())
        }
    }
}
