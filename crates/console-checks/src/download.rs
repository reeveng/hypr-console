//! The download panel: it opens, and it draws the tab it opened on.

use console_stage::checking::{Body, Check, Done};
use console_stage::desktop::Desktop;

use crate::panel::drew;

/// Opening on Audio with nothing looked for yet is the whole of what the first
/// press does.
///
/// Nothing here types a word or fetches anything: the nested desktop has no pad
/// to type with, and a check that reached a site would fail on a train. What it
/// can answer is the question this shape of panel has failed at before --
/// whether it raised a window and then drew anything in it -- and a tab with no
/// search behind it is exactly the state that has nothing of its own to draw.
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
