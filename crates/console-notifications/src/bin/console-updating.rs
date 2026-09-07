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

use console_core_never::Never;
use console_notifications::saying::{Kept, Notice, raise_kept};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Start,
    Done,
    Failed,
}

impl Step {
    pub fn named(word: &str) -> Result<Option<Self>, Never> {
        Ok(match word {
            "start" => Some(Step::Start),
            "done" => Some(Step::Done),
            "failed" => Some(Step::Failed),
            _ => None,
        })
    }
}

pub fn notice(step: Step) -> Result<Notice, Never> {
    match step {
        Step::Start => {
            let Ok(notice) = Notice::new("Updating", "This takes a few minutes.");

            notice.staying()
        }

        Step::Done => {
            let Ok(notice) = Notice::new("Up to date", "Everything is in place.");

            notice.lasting(4000)
        }

        Step::Failed => {
            let Ok(notice) = Notice::new(
                "Update didn't finish",
                "Some files are new and some are old. Run `console apply` again.",
            );
            let Ok(notice) = notice.urgent();

            notice.staying()
        }
    }
}

pub fn goes_on(step: Step) -> Result<Keeps, Never> {
    Ok(match step == Step::Start {
        true => Keeps::TheNumber,
        false => Keeps::Nothing,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keeps {
    TheNumber,
    Nothing,
}

fn main() -> std::process::ExitCode {
    let word = std::env::args().nth(1).unwrap_or_default();
    let Ok(named) = Step::named(&word);

    let Some(step) = named else {
        eprintln!("usage: console-updating start|done|failed");

        return std::process::ExitCode::from(2);
    };

    let Ok(kept) = Kept::named("updating");
    let Ok(notice) = notice(step);
    let Ok(()) = raise_kept(notice, &kept);
    let Ok(goes_on) = goes_on(step);

    match goes_on {
        Keeps::TheNumber => {}

        Keeps::Nothing => {
            let Ok(()) = kept.forget();
        }
    }

    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_notifications::saying::{Expiry, Urgency};

    #[test]
    fn nothing_but_the_three_words_is_a_step() {
        assert_eq!(Step::named("start"), Ok(Some(Step::Start)));
        assert_eq!(Step::named("Done"), Ok(None));
        assert_eq!(Step::named(""), Ok(None));
    }

    #[test]
    fn the_one_that_is_still_happening_does_not_go_by_itself() {
        let Ok(notice) = notice(Step::Start);

        assert_eq!(notice.expiry, Expiry::Stays);
    }

    #[test]
    fn the_one_that_worked_goes_by_itself() {
        let Ok(notice) = notice(Step::Done);

        assert_eq!(notice.expiry, Expiry::Milliseconds(4000));
        assert_eq!(notice.urgency, Urgency::Normal);
    }

    #[test]
    fn the_one_that_did_not_finish_stays_and_says_so_loudly() {
        let Ok(notice) = notice(Step::Failed);

        assert_eq!(notice.expiry, Expiry::Stays);
        assert_eq!(notice.urgency, Urgency::Critical);
    }

    #[test]
    fn the_number_is_only_kept_while_an_apply_is_running() {
        assert_eq!(goes_on(Step::Start), Ok(Keeps::TheNumber));
        assert_eq!(goes_on(Step::Done), Ok(Keeps::Nothing));
        assert_eq!(goes_on(Step::Failed), Ok(Keeps::Nothing));
    }
}
