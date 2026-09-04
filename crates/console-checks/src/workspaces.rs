//! The shoulders move between workspaces.

use console_stage::checking::{Body, Check, Done, not_same, same};
use console_stage::device::{Device, SETTLED};
use console_stage::here::{Here, TURNS};

pub const RIGHT: Check = Check {
    name: "010-workspaces-right",
    about: "R1 moves to the next workspace.",
    feature: "workspaces",
    since: "2026-08-24",
    bodies: &[Body::Here(right_here), Body::Device(right_there)],
};

pub const LEFT: Check = Check {
    name: "011-workspaces-left",
    about: "L1 moves to the workspace before.",
    feature: "workspaces",
    since: "2026-08-24",
    bodies: &[Body::Here(left_here), Body::Device(left_there)],
};

fn right_here(stage: &mut Here) -> Done {
    stage.press("r1")?;
    stage.settle(TURNS);
    let asked = stage.dispatches();
    same(&asked, &[r#"hl.dsp.focus({workspace = "+1"})"#], || format!("R1 asked for {asked:?}"))
}

fn right_there(stage: &mut Device) -> Done {
    let was = stage.workspace();
    stage.press("r1");
    stage.settle(SETTLED);
    not_same(&stage.workspace(), &was, || format!("still on workspace {was}"))
}

fn left_here(stage: &mut Here) -> Done {
    stage.press("l1")?;
    stage.settle(TURNS);
    let asked = stage.dispatches();
    same(&asked, &[r#"hl.dsp.focus({workspace = "-1"})"#], || format!("L1 asked for {asked:?}"))
}

/// Where it went is asked before it is put back, so a failure leaves the desk
/// where it found it.
fn left_there(stage: &mut Device) -> Done {
    let was = stage.workspace();
    stage.press("l1");
    stage.settle(SETTLED);
    let there = stage.workspace();
    stage.press("r1");
    stage.settle(SETTLED);
    not_same(&there, &was, || format!("L1 left us on {was}"))
}
