//! The download panel: it opens, and it draws the tab it opened on.

use console_test_stages::checking::{Body, Check, Done};
use console_test_stages::desktop::Desktop;

use crate::panel::drew;

pub const DRAWS: Check = Check {
    name: "230-the-download-draws",
    about: "The download panel opens, and draws the tab it opened on.",
    feature: "download",
    since: "2026-08-30",
    bodies: &[Body::Desktop(draws)],
};

fn draws(stage: &mut Desktop) -> Done {
    stage.open("download-panel")?;
    drew(stage)
}
