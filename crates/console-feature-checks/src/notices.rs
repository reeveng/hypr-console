//! The notices panel: it opens, and it draws what the bell was about.

use console_test_stages::checking::{Body, Check, Done};
use console_test_stages::desktop::Desktop;

use crate::panel::drew;

pub const DRAWS: Check = Check {
    name: "220-the-notices-draw",
    about: "The notices panel opens, and draws what the desktop has said.",
    feature: "notices",
    since: "2026-08-30",
    bodies: &[Body::Desktop(draws)],
};

fn draws(stage: &mut Desktop) -> Done {
    stage.open("notices-panel")?;
    drew(stage)
}
