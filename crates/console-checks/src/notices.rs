//! The notices panel: it opens, and it draws what the bell was about.

use console_stage::checking::{Body, Check, Done};
use console_stage::desktop::Desktop;

use crate::panel::drew;

/// Opening on Waiting and listing it is the whole of what the tap does.
///
/// Nothing here presses a button, because the nested desktop has no pad to
/// press one with. What it can answer is the question that has caught this
/// shape of panel before: whether it raised a window and then drew anything in
/// it. Three subprocesses stand between this panel opening and knowing what is
/// on it, and one of them is a mako that may not be running at all -- a panel
/// that came up blank on a machine with nothing waiting would look exactly
/// like one that could not read the daemon, and both of those are this check
/// failing.
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
