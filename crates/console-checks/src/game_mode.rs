//! Legion left leaves the desktop for Game Mode.
//!
//! Only here. On the device this would put Steam on the screen and take the
//! desktop the rest of the checks are running against with it, so what is asked
//! is that the button reaches the one script that knows how to leave: the
//! controller goes back to being a gamepad, and the session is switched.

use console_stage::checking::{Body, Check, Done, same};
use console_stage::here::{Here, TURNS};

pub const GAME_MODE: Check = Check {
    name: "190-game-mode",
    about: "Legion left leaves the desktop for Game Mode.",
    feature: "game-mode",
    since: "2026-08-28",
    bodies: &[Body::Here(here)],
};

fn here(stage: &mut Here) -> Done {
    stage.press("legion-left")?;
    stage.settle(TURNS);
    let ran = stage.names();
    same(&ran, &["game-mode"], || format!("it ran {ran:?}"))
}
