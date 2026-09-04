//! Held with L2, a shoulder carries the window rather than leaving it.

use console_stage::checking::{Body, Check, Done, happened, not_same, same};
use console_stage::device::{Device, OPENING};
use console_stage::here::{Here, TURNS};

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
    stage.settle(TURNS);
    let asked = stage.dispatches();
    same(&asked, &[r#"hl.dsp.window.move({workspace = "+1"})"#], || {
        format!("it asked for {asked:?}")
    })
}

/// The window comes along. Which is the only way to move one without a
/// keyboard, so it is worth knowing on the machine rather than in a model.
///
/// It used to count every client on the device and ask that the number had not
/// changed, which is true of a window that came and a window that stayed alike:
/// moving one between workspaces does not make or destroy it. So this passed
/// green with the trigger doing nothing at all, and said so in a check whose
/// whole subject is the trigger. Count where the window is instead.
fn carry_there(stage: &mut Device) -> Done {
    something_open(stage)?;
    let where_ = stage.workspace();
    let set_out = stage.windows_here();
    stage.trigger("l2", 1.0)?;
    stage.press("r1");
    stage.trigger("l2", 0.0)?;
    stage.settle(1.2);
    let there = stage.workspace();
    let arrived = stage.windows_here();
    stage.trigger("l2", 1.0)?;
    stage.press("l1");
    stage.trigger("l2", 0.0)?;
    stage.settle(1.2);
    not_same(&there, &where_, || "it did not move".to_string())?;
    same(&arrived, &set_out, || format!("{set_out} window(s) set out and {arrived} arrived"))
}

fn half_here(stage: &mut Here) -> Done {
    stage.trigger("l2", 0.4)?;
    stage.press("r1")?;
    stage.settle(TURNS);
    let asked = stage.dispatches();
    same(&asked, &[r#"hl.dsp.focus({workspace = "+1"})"#], || format!("it asked for {asked:?}"))
}

/// The count that answers this is the one for the workspace being looked at.
/// Every client on the machine is a different question, and the window this
/// check opens itself would answer that one wrongly for ever.
///
/// Going back is done before the assertion rather than after it, so that a
/// failure leaves the desk where it found it. It did not, and the next check
/// closed the active window on a workspace that no longer had one.
fn half_there(stage: &mut Device) -> Done {
    something_open(stage)?;
    stage.trigger("l2", 0.4)?;
    stage.press("r1");
    stage.trigger("l2", 0.0)?;
    stage.settle(1.2);
    let came = stage.windows_here();
    stage.press("l1");
    stage.settle(1.0);
    same(&came, &0, || format!("{came} window(s) came along and none should have"))
}

/// Something that can be carried, and lost without regret.
pub fn something_open(stage: &mut Device) -> Done {
    match stage.windows_here() > 0 {
        true => Ok(()),
        false => happened(stage.open("alacritty", OPENING), || {
            "nothing would open on the device".to_string()
        }),
    }
}
