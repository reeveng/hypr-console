//! Say, on the screen, that the desktop is being rebuilt under it.
//!
//!     console-updating start   put a notification up, and leave it up
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
use console_notifications::saying::{StatePath, Notification, Content, raise_kept};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Start,
    Succeeded,
    Failed,
}

impl Step {
    pub fn named(word: &str) -> Result<Option<Self>, Never> {
        Ok(match word {
            "start" => Some(Step::Start),
            "done" => Some(Step::Succeeded),
            "failed" => Some(Step::Failed),
            _ => None,
        })
    }
}

pub fn notification(step: Step) -> Result<Notification, Never> {
    match step {
        Step::Start => {
            let Ok(notification) =
                Notification::new(Content { summary: "Updating", body: "This takes a few minutes." });

            notification.staying()
        }

        Step::Succeeded => {
            let Ok(notification) =
                Notification::new(Content { summary: "Up to date", body: "The desktop is up to date." });

            notification.lasting(4000)
        }

        Step::Failed => {
            let Ok(notification) = Notification::new(Content {
                summary: "Update didn't finish",
                body: "Some files are old. Run `console apply` again.",
            });
            let Ok(notification) = notification.urgent();

            notification.staying()
        }
    }
}

pub fn goes_on(step: Step) -> Result<Keeps, Never> {
    Ok(match step == Step::Start {
        true => Keeps::TheNumber,
        false => Keeps::None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keeps {
    TheNumber,
    None,
}

fn main() -> std::process::ExitCode {
    let word = match std::env::args().nth(1) {
        Some(word) => word,
        None => String::new(),
    };

    let Ok(named) = Step::named(&word);

    let step = match named {
        Some(step) => step,
        None => {
            eprintln!("usage: console-updating start|done|failed");

            return std::process::ExitCode::from(2);
        }
    };

    let Ok(kept) = StatePath::named("updating");
    let Ok(notification) = notification(step);
    let Ok(()) = raise_kept(notification, &kept);
    let Ok(goes_on) = goes_on(step);

    match goes_on {
        Keeps::TheNumber => {}

        Keeps::None => {
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
        let Ok(notification) = notification(Step::Start);

        assert_eq!(notification.expiry, Expiry::Stays);
    }

    #[test]
    fn the_one_that_worked_goes_by_itself() {
        let Ok(notification) = notification(Step::Succeeded);

        assert_eq!(notification.expiry, Expiry::Milliseconds(4000));
        assert_eq!(notification.urgency, Urgency::Normal);
    }

    #[test]
    fn the_one_that_did_not_finish_stays_and_says_so_loudly() {
        let Ok(notification) = notification(Step::Failed);

        assert_eq!(notification.expiry, Expiry::Stays);
        assert_eq!(notification.urgency, Urgency::Critical);
    }

    #[test]
    fn the_number_is_only_kept_while_an_apply_is_running() {
        assert_eq!(goes_on(Step::Start), Ok(Keeps::TheNumber));
        assert_eq!(goes_on(Step::Succeeded), Ok(Keeps::None));
        assert_eq!(goes_on(Step::Failed), Ok(Keeps::None));
    }
}
