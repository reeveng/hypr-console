//! The shoulders move between workspaces, and so does a thumb on the bar.
//!
//! The tap is written for the nested desktop because a bar that answers a
//! finger can only be answered by a bar: the daemon is not in this press at
//! all, and what the bar sends the compositor is hyprctl's business rather
//! than the arithmetic's. It went untapped for as long as the bar has drawn
//! workspaces, and the fault was a dispatcher spelled in hyprctl's own words
//! against a lua config, which answers with a lua syntax error nobody was
//! reading.
//!
//! Where the slab is, is measured rather than counted in. The workspace you
//! are on wears a pink slab and the rest wear nothing, so the run of pink
//! along a row of the bar is the one in front and its width is what a
//! workspace slab is on this screen; the other one is a slab away, on the side
//! its number puts it. Which of the two windows the session chose to show is
//! read off the same picture rather than assumed, because the nested session
//! brings a window to the front itself and either of them can be it. A check
//! with the places written in it would be pressing wherever the last font
//! change left the bar.
//!
//! The pointer is stood in the middle of the screen before the picture is
//! taken, because the compositor draws a cursor where the last motion left it
//! and a colour read under it is the cursor's.

use console_compositor::Window;
use console_core_external_programs::Program;
use console_core_geometry::Point;
use console_test_stages::checking::{Body, Check, Done, Why, cannot, not_same, same};
use console_test_stages::desktop::{Desktop, Installed};
use console_test_stages::device::{Device, PATIENCE};
use console_test_stages::here::{Here, TURNS};
use console_test_stages::palette::palette;

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

pub const TAPPED: Check = Check {
    name: "012-a-workspace-on-the-bar-is-tapped",
    about: "A tap on a workspace along the bar moves to it.",
    feature: "workspaces",
    since: "2026-09-19",
    bodies: &[Body::Desktop(tapped_here)],
};

pub const ANOTHER: Check = Check {
    name: "013-the-plus-on-the-bar-opens-a-workspace",
    about: "A tap on the + past the workspaces moves to one that was not there.",
    feature: "workspaces",
    since: "2026-09-20",
    bodies: &[Body::Desktop(another_here)],
};

const WINDOWS: usize = 2;

const ROW: u32 = 4;

const ALONG: u32 = 400;

fn right_here(stage: &mut Here) -> Done {
    stage.press("r1")?;

    let Ok(()) = stage.settle(TURNS);
    let Ok(asked) = stage.dispatches();

    same(&asked, &[r#"hl.dsp.focus({workspace = "+1"})"#], || format!("R1 asked for {asked:?}"))
}

fn right_there(stage: &mut Device) -> Done {
    let Ok(was) = stage.workspace();
    let Ok(()) = stage.press("r1");
    let Ok(_) = stage.changed(Device::workspace, &was, PATIENCE);
    let Ok(now) = stage.workspace();

    not_same(&now, &was, || format!("still on workspace {was}"))
}

fn left_here(stage: &mut Here) -> Done {
    stage.press("l1")?;

    let Ok(()) = stage.settle(TURNS);
    let Ok(asked) = stage.dispatches();

    same(&asked, &[r#"hl.dsp.focus({workspace = "-1"})"#], || format!("L1 asked for {asked:?}"))
}

fn left_there(stage: &mut Device) -> Done {
    let Ok(was) = stage.workspace();
    let Ok(()) = stage.press("l1");
    let Ok(_) = stage.changed(Device::workspace, &was, PATIENCE);
    let Ok(there) = stage.workspace();
    let Ok(()) = stage.press("r1");
    let Ok(_) = stage.changed(Device::workspace, &there, PATIENCE);

    not_same(&there, &was, || format!("L1 left us on {was}"))
}

fn another_here(stage: &mut Desktop) -> Done {
    let Ok(window) = Program::Alacritty.name();
    let Ok(installed) = stage.installed(window);

    match installed {
        Installed::No => return cannot("there is no window to open on this machine"),
        Installed::Yes => {},
    }

    let wearing = pink()?;

    two_workspaces(stage)?;

    let windows = both_opened(stage)?;
    let front = stage.front()?;
    let behind = windows.iter().filter(|window| window.workspace > front.id).count();
    let Ok(past) = console_core_number_conversion::fitted::<usize, u32>(behind);

    let (from, wide) = lit(stage, &wearing)?;
    let half = wide.saturating_div(2);
    let at = from
        .saturating_add(wide.saturating_mul(past.saturating_add(1)))
        .saturating_add(half);
    let room = stage.logical()?;

    stage.click_in(console_onscreen::BAR, (at, ROW))?;
    stage.point((room.wide.saturating_div(2), room.tall.saturating_div(2)))?;

    let now = stage.front()?;
    let opened = windows.iter().any(|window| window.workspace == now.id);

    match opened {
        true => Err(Why::Failed(format!(
            "the + left workspace {} in front, which is one a window was already on",
            now.id
        ))),
        false => Ok(()),
    }
}

fn pink() -> Result<String, Why> {
    let Ok(wanted) = palette();

    wanted
        .get("pink")
        .cloned()
        .ok_or_else(|| Why::Cannot("the palette says no pink".to_string()))
}

fn two_workspaces(stage: &mut Desktop) -> Result<(), Why> {
    let Ok(window) = Program::Alacritty.name();

    stage.open(window)?;
    stage.open(window)?;

    Ok(())
}

fn both_opened(stage: &mut Desktop) -> Result<Vec<Window>, Why> {
    let windows = stage.windows()?;

    match windows.len() {
        WINDOWS => Ok(windows),
        _ => Err(Why::Cannot(format!(
            "{} windows opened rather than {WINDOWS}, so there was no second workspace to tap",
            windows.len()
        ))),
    }
}

fn lit(stage: &mut Desktop, wearing: &str) -> Result<(u32, u32), Why> {
    let room = stage.logical()?;
    let along = ALONG.min(room.wide);
    let mut widest: (u32, u32) = (0, 0);
    let mut from: u32 = 0;
    let mut wide: u32 = 0;

    for across in 0..along {
        let read = stage.colour(Point { across: f64::from(across), down: f64::from(ROW) })?;

        match read == wearing {
            true => {
                from = match wide {
                    0 => across,
                    _ => from,
                };
                wide = wide.saturating_add(1);
            }
            false => wide = 0,
        }

        match wide > widest.1 {
            true => widest = (from, wide),
            false => {},
        }
    }

    match widest.1 {
        0 => Err(Why::Failed(
            "no workspace along the bar was lit, so there was nothing beside it to tap"
                .to_string(),
        )),
        _ => Ok(widest),
    }
}

fn tapped_here(stage: &mut Desktop) -> Done {
    let Ok(window) = Program::Alacritty.name();
    let Ok(installed) = stage.installed(window);

    match installed {
        Installed::No => return cannot("there is no window to open on this machine"),
        Installed::Yes => {},
    }

    let wearing = pink()?;

    two_workspaces(stage)?;

    let windows = both_opened(stage)?;
    let front = stage.front()?;
    let beside = windows
        .iter()
        .map(|window| window.workspace)
        .find(|workspace| *workspace != front.id)
        .ok_or_else(|| {
            Why::Cannot("both windows opened on one workspace, so there is only one slab".to_string())
        })?;

    let (from, wide) = lit(stage, &wearing)?;
    let half = wide.saturating_div(2);
    let at = match beside < front.id {
        true => from.saturating_sub(half),
        false => from.saturating_add(wide).saturating_add(half),
    };

    let quiet = stage.colour(Point { across: f64::from(at), down: f64::from(ROW) })?;

    not_same(&quiet, &wearing, || {
        format!("the workspace at {at} is lit as well as the one in front")
    })?;

    let room = stage.logical()?;

    let Ok(()) = stage.fresh();

    two_workspaces(stage)?;
    stage.click_in(console_onscreen::BAR, (at, ROW))?;
    stage.point((room.wide.saturating_div(2), room.tall.saturating_div(2)))?;

    let _both = both_opened(stage)?;
    let now = stage.front()?;

    same(&now.id, &beside, || {
        format!("a tap on workspace {beside} left workspace {} in front", now.id)
    })?;

    let lit_now = stage.colour(Point { across: f64::from(at), down: f64::from(ROW) })?;

    same(&lit_now, &wearing, || {
        format!("workspace {beside} is in front and its slab is {lit_now} rather than {wearing}")
    })
}
