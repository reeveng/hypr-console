//! The top right paddle closes what is in front of you.

use console_test_stages::checking::{Body, Check, Done, less_than, same};
use console_test_stages::device::Device;
use console_test_stages::here::{Here, TURNS};

use crate::carry::something_open;

pub const CLOSE: Check = Check {
    name: "030-close-the-window",
    about: "The top right paddle closes what is in front of you.",
    feature: "close",
    since: "2026-08-24",
    bodies: &[Body::Here(here), Body::Device(there)],
};

fn here(stage: &mut Here) -> Done {
    stage.press("right-paddle-top")?;

    let Ok(()) = stage.settle(TURNS);
    let Ok(asked) = stage.names();

    same(&asked, &["put-away"], || format!("it asked for {asked:?}"))
}

fn there(stage: &mut Device) -> Done {
    something_open(stage)?;

    let Ok(before) = stage.windows_here();
    let Ok(()) = stage.press("right-paddle-top");
    let Ok(()) = stage.settle(1.2);
    let Ok(now) = stage.windows_here();

    less_than(now, before, || format!("{before} window(s) before and {now} after"))
}
