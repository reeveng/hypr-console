//! The files panel: it opens, and it lists what it opened on.

use console_stage::checking::{Body, Check, Done};
use console_stage::desktop::Desktop;

use crate::panel::drew;

/// Opening on Home and listing it is the whole of what the first press does.
///
/// Nothing here presses a button, because the nested desktop has no pad to
/// press one with. What it can answer is the question that has caught this
/// shape of panel before: whether it raised a window and then drew anything in
/// it. A files panel that comes up empty looks the same as one that cannot read
/// a home directory, and both of those are this check failing.
pub const DRAWS: Check = Check {
    name: "200-the-files-draw",
    about: "The files panel opens, and lists the folder it opened on.",
    feature: "files",
    since: "2026-08-29",
    bodies: &[Body::Desktop(draws)],
};

fn draws(stage: &mut Desktop) -> Done {
    stage.open("files-panel")?;
    drew(stage)
}
