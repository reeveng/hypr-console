//! What a battery running out puts on the screen, and how the machine stops.
//!
//! Where the three steps stand is `console_defaults::battery`, which is a
//! setting. What each of them says, and what the last one does, is here,
//! because it is about this machine rather than about a number.
//!
//! The last one is the only part that had to be asked of the hardware. "Save
//! everything and stop" means hibernate: the session goes to disk and the
//! machine goes off, and plugging in and pressing the button puts everything
//! back where it was. This device cannot do that. Its only swap is zram, which
//! is memory, and nothing on the kernel command line names a device to come
//! back from -- `/sys/power/resume` reads `0:0` -- so logind answers `na` when
//! it is asked whether it can hibernate, and it is right to.
//!
//! So the machine stops instead of saving, and the card says so rather than
//! promising otherwise. That is the honest half of it. The other half is why
//! stopping is still better than the two things it might have been. Sleeping
//! keeps the session in the memory that the battery about to run out is what
//! powers, so a suspend at five per cent is the session lost in an hour and a
//! hard cut when the cell empties -- and `hypridle.conf` already refuses to
//! sleep this machine unattended, for the separate reason that nothing here
//! has ever proved it wakes. Doing nothing is the same loss with a dirty
//! filesystem and a cell taken to nought, which is the one thing that damages
//! a battery rather than merely emptying it.
//!
//! A device that can hibernate gets hibernation. Nothing here is written for
//! this handheld's answer; it asks, and the card follows what it was told.

use std::time::Duration;

use console_defaults::battery::Step;
use console_notices::saying::Notice;

/// How long there is between the card and the machine stopping.
///
/// Fifteen seconds, and the number is on the card. At five per cent there are
/// minutes left rather than seconds, so this costs nothing and is the whole
/// difference between a machine that warns and a machine that snatches --
/// somebody holding it can reach a cable, and somebody who is not holding it
/// loses nothing by the wait.
pub const GRACE: Duration = Duration::from_secs(15);

/// How often the cable is looked for while the wait runs.
pub const LOOKING: Duration = Duration::from_secs(1);

/// What this machine can do when the battery is nearly gone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stop {
    /// The session to disk and the machine off. Everything comes back.
    Hibernate,
    /// The machine off, cleanly. What is open is not saved, and the card says
    /// so before it happens.
    PowerOff,
}

impl Stop {
    /// Which of the two this machine has, out of what the kernel says.
    ///
    /// Two questions and both have to answer. `/sys/power/state` says whether
    /// the kernel was built to suspend to disk at all; `/sys/power/resume`
    /// says which device it would come back from, and `0:0` is the answer of a
    /// machine that has never been told. A kernel that can and a machine with
    /// nowhere to write is a hibernate that fails at the moment it is needed.
    pub fn of(state: &str, resume: &str) -> Self {
        let can = state.split_whitespace().any(|word| word == "disk");
        let somewhere = !matches!(resume.trim(), "" | "0:0");
        match can && somewhere {
            true => Stop::Hibernate,
            false => Stop::PowerOff,
        }
    }

    /// What this machine is asked to do.
    pub fn argv(self) -> Vec<String> {
        let what = match self {
            Stop::Hibernate => "hibernate",
            Stop::PowerOff => "poweroff",
        };
        ["systemctl", what].iter().map(|word| (*word).to_string()).collect()
    }

    /// What to do instead where that would not run.
    ///
    /// Only ever one deep, and only for the disagreement this cannot settle
    /// from here: the kernel says it could hibernate and logind refuses --
    /// because there is not swap enough, or because the policy says no. A
    /// machine that answered that by doing nothing would be a machine whose
    /// self-protection is a card and no more.
    pub fn instead(self) -> Option<Self> {
        match self {
            Stop::Hibernate => Some(Stop::PowerOff),
            Stop::PowerOff => None,
        }
    }
}

/// Where the kernel keeps the two answers.
pub const STATE: &str = "/sys/power/state";
pub const RESUME: &str = "/sys/power/resume";

/// The same, off this machine.
pub fn stop() -> Stop {
    let said = |at: &str| std::fs::read_to_string(at).unwrap_or_default();
    Stop::of(&said(STATE), &said(RESUME))
}

/// What a step puts on the screen.
///
/// One card, replaced rather than stacked, which is why all three are one
/// shape: a battery that falls through two steps in a minute leaves one card
/// saying the later thing, not two cards where the older one is the one being
/// read.
///
/// The first is ordinary and goes by itself, because getting low is news and
/// not a fault. The other two stay until somebody takes them: one is asking
/// for a cable and the other is saying what is about to happen, and a card
/// that times out is a card that was not read.
pub fn card(step: Step, charge: i32, stop: Stop) -> Notice {
    let left = format!("{charge}% left.");
    match step {
        Step::Low => Notice::new("Battery getting low", &left).lasting(6000).valued(charge.into()),
        Step::Lower => Notice::new("Battery getting really low", &format!("{left} Time to find the cable."))
            .urgent()
            .staying()
            .valued(charge.into()),
        Step::Protect => stopping(charge, stop),
    }
}

/// The card that says the machine is about to stop, and what that will cost.
///
/// The number of seconds is in it, because the whole of what this card is for
/// is somebody deciding what to do with them.
fn stopping(charge: i32, stop: Stop) -> Notice {
    let seconds = GRACE.as_secs();
    let (summary, body) = match stop {
        Stop::Hibernate => (
            "Saving everything and stopping".to_string(),
            format!(
                "{charge}% left. Everything will be where you left it. \
                 Plug in within {seconds} seconds to carry on."
            ),
        ),
        Stop::PowerOff => (
            format!("Stopping in {seconds} seconds"),
            format!(
                "{charge}% left. This machine cannot save what is open, so it will shut down \
                 rather than run the battery flat. Plug in to carry on."
            ),
        ),
    };
    Notice::new(&summary, &body).urgent().staying().valued(charge.into())
}

/// The card that replaces it when the cable goes in.
///
/// Said rather than let go quietly. A machine that announced it was about to
/// stop and then said nothing more is a machine somebody goes on watching.
pub fn saved() -> Notice {
    Notice::new("The cable went in", "Nothing was stopped.").lasting(4000)
}

/// What the journal is told, whatever the screen is doing.
///
/// The one line somebody reads afterwards, when the question is why the
/// machine was off. It names the charge and the doing, because "the battery"
/// on its own is the answer they already had.
pub fn for_the_journal(charge: i32, stop: Stop) -> String {
    let doing = match stop {
        Stop::Hibernate => "hibernating",
        Stop::PowerOff => "shutting down",
    };
    format!("battery at {charge}%: {doing} before it runs out")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The machine this was written for. Its kernel can suspend to disk and it
    /// has nowhere to come back from, which is the pair that reads as yes to
    /// half a question.
    #[test]
    fn a_kernel_that_can_and_a_machine_with_nowhere_to_write_cannot_hibernate() {
        assert_eq!(Stop::of("freeze mem disk", "0:0"), Stop::PowerOff);
        assert_eq!(Stop::of("freeze mem disk", "259:2"), Stop::Hibernate);
        assert_eq!(Stop::of("freeze mem", "259:2"), Stop::PowerOff);
        assert_eq!(Stop::of("", ""), Stop::PowerOff);
    }

    /// A card that cannot keep a promise does not make it. On a machine that
    /// stops without saving, the words say stopping and say what it costs.
    #[test]
    fn the_card_promises_only_what_the_machine_can_do() {
        let stopping = card(Step::Protect, 5, Stop::PowerOff);
        assert!(stopping.summary.contains("Stopping"), "{}", stopping.summary);
        assert!(stopping.body.contains("cannot save"), "{}", stopping.body);

        let saving = card(Step::Protect, 5, Stop::Hibernate);
        assert!(saving.summary.contains("Saving"), "{}", saving.summary);
        assert!(saving.body.contains("where you left it"), "{}", saving.body);
    }

    /// However many seconds there are, the card is the only place a person can
    /// find that out, so the two cannot drift.
    #[test]
    fn the_card_says_how_long_there_is() {
        let said = card(Step::Protect, 4, Stop::PowerOff);
        let seconds = GRACE.as_secs().to_string();
        assert!(said.summary.contains(&seconds) || said.body.contains(&seconds));
    }

    /// Getting low is news and goes by itself. The other two are asking for
    /// something and stay until they are answered.
    #[test]
    fn only_the_first_card_takes_itself_away() {
        assert_eq!(card(Step::Low, 25, Stop::PowerOff).expiry, console_notices::saying::Expiry::Milliseconds(6000));
        assert_eq!(card(Step::Lower, 10, Stop::PowerOff).expiry, console_notices::saying::Expiry::Stays);
        assert_eq!(card(Step::Protect, 5, Stop::PowerOff).expiry, console_notices::saying::Expiry::Stays);
    }

    /// Every one of them carries the charge as a length, so the card fills to
    /// what is left the way the volume and the screen do.
    #[test]
    fn every_card_draws_what_is_left() {
        for step in console_defaults::battery::EVERY {
            assert_eq!(card(step, 7, Stop::PowerOff).value, Some(7));
        }
    }

    /// A machine found off in the morning is a question, and this is the line
    /// that answers it.
    #[test]
    fn the_journal_says_why_the_machine_stopped() {
        let said = for_the_journal(4, Stop::PowerOff);
        assert!(said.contains("4%") && said.contains("shutting down"), "{said}");
    }
}
