//! The top right paddle closes what is in front of you.
//!
//! It opens the window it closes. The paddle closes whatever is in front, and
//! what is in front on somebody's machine is something of theirs -- so a check
//! that pressed it on whatever it found asserted the right thing by taking a
//! window away from the person who lent the device, and there is no putting
//! that back. Opening one first costs a second and asks a better question
//! besides: not that the count of windows went down, which is true whichever
//! window went, but that the one that was in front is the one that is gone.

use console_test_stages::checking::{Body, Check, Done, failed, happened, same};
use console_test_stages::device::{Device, OPENING, PATIENCE};
use console_test_stages::here::{Here, TURNS};

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
    let Ok(ours) = stage.opening("alacritty", OPENING);

    let Some(ours) = ours else {
        return failed("nothing would open on the device to be closed".to_string());
    };

    let Ok(()) = stage.press("right-paddle-top");
    let Ok(went) = stage.window_gone(&ours, PATIENCE);

    happened(went, || format!("the paddle left the window at {ours} where it was"))
}
