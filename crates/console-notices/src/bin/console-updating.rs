//! Say, on the screen, that the desktop is being rebuilt under it.
//!
//!     console-updating start   put a notice up, and leave it up
//!     console-updating done    replace it with one that goes by itself
//!     console-updating failed  replace it with one that says it did not
//!
//! `console apply` rewrites files, restarts services and compiles every program
//! the manifest names. On this device that is the better part of a minute, and
//! for all of it the screen said nothing. So whether the thing about to be
//! tested was the new copy or the one before it was a question answered by
//! remembering how long ago the deploy was pressed, and a fault reported
//! against a binary that had already been replaced costs an evening at both
//! ends of the wire.
//!
//! Nothing here is worth failing an apply over: an apply that worked and could
//! not say so is an apply that worked.

use console_notices::saying::{Kept, Notice, raise_kept};

/// Where an apply has got to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Start,
    Done,
    Failed,
}

impl Step {
    pub fn named(word: &str) -> Option<Self> {
        match word {
            "start" => Some(Step::Start),
            "done" => Some(Step::Done),
            "failed" => Some(Step::Failed),
            _ => None,
        }
    }
}

/// What the screen says at this step.
pub fn notice(step: Step) -> Notice {
    match step {
        Step::Start => Notice::new(
            "Updating the console",
            "Writing files, restarting services and building every program the manifest names.",
        )
        .staying(),
        Step::Done => Notice::new(
            "The console is up to date",
            "Everything the manifest asks for is what is on the machine.",
        )
        .lasting(4000),
        // It stays. An apply that stopped halfway leaves a machine that is
        // neither what it was nor what it was going to be, and that is worth
        // more than four seconds of somebody's attention.
        Step::Failed => Notice::new(
            "The update did not finish",
            "The machine is part-way. journalctl -t console, or run console apply again.",
        )
        .urgent()
        .staying(),
    }
}

/// Whether the number this notice came back under is still worth keeping.
///
/// Only while an apply is running. Once it has finished either way, the next
/// apply is a new notice rather than a replacement of the last one's, or a
/// deploy tomorrow quietly edits a card from tonight.
pub fn goes_on(step: Step) -> bool {
    step == Step::Start
}

fn main() -> std::process::ExitCode {
    let Some(step) = std::env::args().nth(1).as_deref().and_then(Step::named) else {
        eprintln!("usage: console-updating start|done|failed");
        return std::process::ExitCode::from(2);
    };

    let kept = Kept::named("updating");
    raise_kept(notice(step), &kept);
    if !goes_on(step) {
        kept.forget();
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_notices::saying::{Expiry, Urgency};

    #[test]
    fn nothing_but_the_three_words_is_a_step() {
        assert_eq!(Step::named("start"), Some(Step::Start));
        assert_eq!(Step::named("Done"), None);
        assert_eq!(Step::named(""), None);
    }

    /// It stands for as long as the apply does, however long that turns out to
    /// be, because an apply that is still running is the whole point of it.
    #[test]
    fn the_one_that_is_still_happening_does_not_go_by_itself() {
        assert_eq!(notice(Step::Start).expiry, Expiry::Stays);
    }

    #[test]
    fn the_one_that_worked_goes_by_itself() {
        assert_eq!(notice(Step::Done).expiry, Expiry::Milliseconds(4000));
        assert_eq!(notice(Step::Done).urgency, Urgency::Normal);
    }

    /// A machine left part-way is worth more than four seconds of somebody's
    /// attention, so this one waits to be taken away.
    #[test]
    fn the_one_that_did_not_finish_stays_and_says_so_loudly() {
        assert_eq!(notice(Step::Failed).expiry, Expiry::Stays);
        assert_eq!(notice(Step::Failed).urgency, Urgency::Critical);
    }

    /// Otherwise a deploy tomorrow replaces a card from tonight.
    #[test]
    fn the_number_is_only_kept_while_an_apply_is_running() {
        assert!(goes_on(Step::Start));
        assert!(!goes_on(Step::Done));
        assert!(!goes_on(Step::Failed));
    }
}
