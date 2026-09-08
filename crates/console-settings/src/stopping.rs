//! What a battery running out puts on the screen, and how the machine stops.
//! Where the three steps stand is `console_default_applications::battery`,
//! which is a setting. What each of them says, and what the last one does, is
//! here, because it is about this machine rather than about a number.  The last
//! one is the only part that had to be asked of the hardware. "Save everything
//! and stop" means hibernate: the session goes to disk and the machine goes
//! off, and plugging in and pressing the button puts everything back where it
//! was. This device cannot do that. Its only swap is zram, which is memory, and
//! nothing on the kernel command line names a device to come back from --
//! `/sys/power/resume` reads `0:0` -- so logind answers `na` when it is asked
//! whether it can hibernate, and it is right to.  So the machine stops instead
//! of saving, and the card says so rather than promising otherwise. That is the
//! honest half of it. The other half is why stopping is still better than the
//! two things it might have been. Sleeping keeps the session in the memory that
//! the battery about to run out is what powers, so a suspend at five per cent
//! is the session lost in an hour and a hard cut when the cell empties -- and
//! `hypridle.conf` already refuses to sleep this machine unattended, for the
//! separate reason that nothing here has ever proved it wakes. Doing nothing is
//! the same loss with a dirty filesystem and a cell taken to nought, which is
//! the one thing that damages a battery rather than merely emptying it.  A
//! device that can hibernate gets hibernation. Nothing here is written for this
//! handheld's answer; it asks, and the card follows what it was told.

use std::time::Duration;

use console_default_applications::battery::Step;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_notifications::saying::Notice;

pub const GRACE: Duration = Duration::from_secs(15);

pub const LOOKING: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stop {
    Hibernate,
    PowerOff,
}

impl Stop {
    pub fn of(state: &str, resume: &str) -> Result<Self, Never> {
        let can = state.split_whitespace().any(|word| word == "disk");
        let somewhere = !matches!(resume.trim(), "" | "0:0");

        Ok(match can && somewhere {
            true => Stop::Hibernate,
            false => Stop::PowerOff,
        })
    }

    pub fn argv(self) -> Result<Vec<String>, Never> {
        let what = match self {
            Stop::Hibernate => "hibernate",
            Stop::PowerOff => "poweroff",
        };

        Program::Systemctl.argv(&[what])
    }

    pub fn instead(self) -> Result<Option<Self>, Never> {
        Ok(match self {
            Stop::Hibernate => Some(Stop::PowerOff),
            Stop::PowerOff => None,
        })
    }
}

pub const STATE: &str = "/sys/power/state";
pub const RESUME: &str = "/sys/power/resume";

pub fn stop() -> Result<Stop, Never> {
    let said = |at: &str| match std::fs::read_to_string(at) {
        Ok(said) => said,
        Err(_) => String::new(),
    };

    Stop::of(&said(STATE), &said(RESUME))
}

pub fn card(step: Step, charge: i32, stop: Stop) -> Result<Notice, Never> {
    let left = format!("{charge}% left.");

    match step {
        Step::Low => {
            let Ok(notice) = Notice::new("Battery low", &left);
            let Ok(notice) = notice.lasting(6000);

            notice.valued(i64::from(charge))
        },
        Step::Lower => {
            let Ok(notice) = Notice::new("Battery very low", &format!("{left} Plug in soon."));
            let Ok(notice) = notice.urgent();
            let Ok(notice) = notice.staying();

            notice.valued(i64::from(charge))
        },
        Step::Protect => stopping(charge, stop),
    }
}

fn stopping(charge: i32, stop: Stop) -> Result<Notice, Never> {
    let seconds = GRACE.as_secs();
    let (summary, body) = match stop {
        Stop::Hibernate => (
            format!("Saving and stopping in {seconds} seconds"),
            format!("{charge}% left. Everything will be where you left it. Plug in to carry on."),
        ),
        Stop::PowerOff => (
            format!("Shutting down in {seconds} seconds"),
            format!("{charge}% left. Open work can't be saved. Plug in to carry on."),
        ),
    };
    let Ok(notice) = Notice::new(&summary, &body);
    let Ok(notice) = notice.urgent();
    let Ok(notice) = notice.staying();

    notice.valued(i64::from(charge))
}

pub fn saved() -> Result<Notice, Never> {
    let Ok(notice) = Notice::new("Plugged in", "Nothing was stopped.");

    notice.lasting(4000)
}

pub fn for_the_journal(charge: i32, stop: Stop) -> Result<String, Never> {
    let doing = match stop {
        Stop::Hibernate => "hibernating",
        Stop::PowerOff => "shutting down",
    };

    Ok(format!("battery at {charge}%: {doing} before it runs out"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kernel_that_can_and_a_machine_with_nowhere_to_write_cannot_hibernate() {
        assert_eq!(Stop::of("freeze mem disk", "0:0"), Ok(Stop::PowerOff));
        assert_eq!(Stop::of("freeze mem disk", "259:2"), Ok(Stop::Hibernate));
        assert_eq!(Stop::of("freeze mem", "259:2"), Ok(Stop::PowerOff));
        assert_eq!(Stop::of("", ""), Ok(Stop::PowerOff));
    }

    #[test]
    fn the_card_promises_only_what_the_machine_can_do() {
        let Ok(stopping) = card(Step::Protect, 5, Stop::PowerOff);

        assert!(stopping.summary.contains("Shutting down"), "{}", stopping.summary);
        assert!(stopping.body.contains("can't be saved"), "{}", stopping.body);

        let Ok(saving) = card(Step::Protect, 5, Stop::Hibernate);

        assert!(saving.summary.contains("Saving"), "{}", saving.summary);
        assert!(saving.body.contains("where you left it"), "{}", saving.body);
    }

    #[test]
    fn the_card_says_how_long_there_is() {
        let Ok(said) = card(Step::Protect, 4, Stop::PowerOff);
        let seconds = GRACE.as_secs().to_string();

        assert!(said.summary.contains(&seconds) || said.body.contains(&seconds));
    }

    #[test]
    fn only_the_first_card_takes_itself_away() {
        use console_notifications::saying::Expiry;

        let Ok(low) = card(Step::Low, 25, Stop::PowerOff);
        let Ok(lower) = card(Step::Lower, 10, Stop::PowerOff);
        let Ok(protect) = card(Step::Protect, 5, Stop::PowerOff);

        assert_eq!(low.expiry, Expiry::Milliseconds(6000));
        assert_eq!(lower.expiry, Expiry::Stays);
        assert_eq!(protect.expiry, Expiry::Stays);
    }

    #[test]
    fn every_card_draws_what_is_left() {
        for step in console_default_applications::battery::EVERY {
            let Ok(said) = card(step, 7, Stop::PowerOff);

            assert_eq!(said.value, Some(7));
        }
    }

    #[test]
    fn the_journal_says_why_the_machine_stopped() {
        let Ok(said) = for_the_journal(4, Stop::PowerOff);

        assert!(said.contains("4%") && said.contains("shutting down"), "{said}");
    }
}
