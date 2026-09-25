//! The machine, asked to stay up for as long as something is going on.
//!
//! Two things on this desktop are ruined by the machine stopping underneath
//! them, and they are ruined by different amounts of it. An apply is undone by
//! a shutdown, by a sleep, and by the idle daemon reaching the end of its
//! timer, because what it is partway through is the contents of `/usr`. A song
//! is undone by a sleep alone: the screen going dark over a playing track is
//! what a person putting the device down on the arm of a chair is asking for,
//! and a suspend five minutes later is not.
//!
//! systemd already has the answer and neither of them was asking for it. A
//! block inhibitor is a promise from the manager that it will not begin a
//! shutdown, a sleep or an idle effect while somebody is holding one, and it is
//! released when the holder lets go or dies.
//!
//! A third is ruined from somewhere else. A deploy spends minutes on the laptop
//! in `just ready` and then waits on a card for a person, and the device's
//! idle timer runs through all of it: by the time the push arrives the screen
//! is out and the machine is asleep, and the apply that would have taken the
//! lock above never starts. So `taking_on` holds the same lock over ssh, for
//! as long as whoever asked is running -- the ssh ends with it, `cat` on the
//! device reads end-of-file, and the lock goes the way it always does. The
//! device checks hold it the same way, because a check reads a screen the
//! idle timer would otherwise dim underneath it.
//!
//! # Why the lock is not the idle one
//!
//! logind's `idle` inhibitor and hypridle's own reading of it are different
//! questions and only one of them is asked here. hypridle looks at the idle
//! inhibitors once for the whole of its config, so a player that took one would
//! hold the panel lit as well as the machine awake -- and the panel is the
//! expensive half of a handheld. `sleep` is the narrow lock that leaves the
//! dimming and the blanking exactly where they were.
//!
//! # Why this is a child process and not a dbus call
//!
//! An inhibitor is a file descriptor the manager hands back over dbus. Rather
//! than take a dbus dependency for one call -- and the apply engine has none at
//! all -- the lock is held the way `systemd-inhibit` was written to hold it: it
//! takes the lock, runs something, and releases it when that something ends.
//! What it runs here is `cat`, reading a pipe whose other end the holder keeps.
//!
//! That indirection is doing real work. The pipe closes when the holding
//! process ends -- returned, panicked, or killed outright -- because the kernel
//! closes it, not because any code here remembered to. `cat` reads end-of-file,
//! exits, and `systemd-inhibit` releases the lock on the way out. A program
//! killed partway therefore leaves no inhibitor behind, which matters more than
//! it sounds: a lock no one is holding and no one can find is a machine that
//! has quietly stopped being able to suspend.

use std::process::Stdio;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_words::Words;
use console_session::reaching::quoted;
use console_program_lifetime::{BoundToParent, alongside};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Words)]
pub enum InhibitReason {
    #[words(
        what = "shutdown:sleep:idle",
        who = "console apply",
        why = "installing the desktop"
    )]
    FromStopping,
    #[words(what = "sleep", who = "console music", why = "a song is playing")]
    FromSleeping,
    #[words(
        what = "sleep:idle",
        who = "console-deploy",
        why = "a deploy from another machine is under way"
    )]
    FromDeploying,
    #[words(
        what = "sleep:idle",
        who = "console-check",
        why = "the checks are being pressed from another machine"
    )]
    FromChecking,
}

#[derive(Debug)]
pub struct Staying {
    holding: Option<BoundToParent>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InhibitResult {
    Acquired(Staying),
    Failed(String),
}

impl PartialEq for Staying {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl Eq for Staying {}

pub fn taking(kept: InhibitReason) -> Result<InhibitResult, Never> {
    let Ok(mut asking) = Program::SystemdInhibit.command();
    let Ok(cat) = Program::Cat.name();
    let Ok(what) = kept.what();
    let Ok(who) = kept.who();
    let Ok(why) = kept.why();

    asking
        .args([
            &format!("--what={what}"),
            &format!("--who={who}"),
            &format!("--why={why}"),
            "--mode=block",
            cat,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let started = alongside(&mut asking);

    Ok(match started {
        Ok(holding) => InhibitResult::Acquired(Staying { holding: Some(holding) }),
        Err(fault) => InhibitResult::Failed(format!(
            "the machine could not be asked to stay up ({fault}), so {why} will be interrupted \
             by a machine that stops underneath it"
        )),
    })
}

pub fn taking_on(host: &str, kept: InhibitReason) -> Result<InhibitResult, Never> {
    let Ok(mut asking) = Program::Ssh.command();
    let Ok(inhibit) = Program::SystemdInhibit.name();
    let Ok(cat) = Program::Cat.name();
    let Ok(what) = kept.what();
    let Ok(who) = kept.who();
    let Ok(why) = kept.why();
    let Ok(who_quoted) = quoted(who);
    let Ok(why_quoted) = quoted(why);
    let there = format!(
        "{inhibit} --what={what} --who={who_quoted} --why={why_quoted} --mode=block {cat}"
    );

    asking
        .args([host, &there])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let started = alongside(&mut asking);

    Ok(match started {
        Ok(holding) => InhibitResult::Acquired(Staying { holding: Some(holding) }),
        Err(fault) => InhibitResult::Failed(format!(
            "{host} could not be asked to stay up ({fault}), so it may sleep while {why}"
        )),
    })
}

impl Drop for Staying {
    fn drop(&mut self) {
        let mut holding = match self.holding.take() {
            Some(holding) => holding,
            None => return,
        };

        let Ok(writing) = holding.writing();

        drop(writing);
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    #[test]
    fn an_apply_is_kept_from_stopping_sleeping_and_going_idle() {
        let Ok(what) = InhibitReason::FromStopping.what();

        assert!(what.contains("shutdown"), "{what}");
        assert!(what.contains("sleep"), "{what}");
        assert!(what.contains("idle"), "{what}");
    }

    #[test]
    fn a_song_is_kept_from_sleeping_and_from_nothing_else() {
        let Ok(what) = InhibitReason::FromSleeping.what();

        assert_eq!(what, "sleep");
    }

    #[test]
    fn the_lid_is_left_to_whoever_shut_it() {
        let Ok(stopping) = InhibitReason::FromStopping.what();
        let Ok(sleeping) = InhibitReason::FromSleeping.what();

        assert!(!stopping.contains("lid"), "{stopping}");
        assert!(!sleeping.contains("lid"), "{sleeping}");
    }

    #[test]
    fn a_machine_that_cannot_be_asked_says_so_rather_than_failing() {
        let said = match taking_with("this-is-not-a-program-on-any-machine") {
            InhibitResult::Failed(said) => said,
            InhibitResult::Acquired(_) => panic!("a program that does not exist held a lock"),
        };

        assert!(said.contains("stay up"), "{said}");
    }

fn taking_with(program: &str) -> InhibitResult {
        let mut asking = Command::new(program);
        asking.stdin(Stdio::piped());

        match alongside(&mut asking) {
            Ok(holding) => InhibitResult::Acquired(Staying { holding: Some(holding) }),
            Err(fault) => InhibitResult::Failed(format!(
                "the machine could not be asked to stay up ({fault}), so a song is playing will \
                 be interrupted by a machine that stops underneath it"
            )),
        }
    }

    #[test]
    fn letting_go_ends_the_child_that_was_holding_it() {
        let Ok(mut asking) = Program::Cat.command();

        asking.stdin(Stdio::piped()).stdout(Stdio::null());

        let holding = match alongside(&mut asking) {
            Ok(holding) => holding,
            Err(_fault) => return,
        };

        let Ok(id) = holding.id();
        let staying = Staying { holding: Some(holding) };
        drop(staying);

        let still = std::path::Path::new(&format!("/proc/{id}/stat")).exists();

        assert!(!still, "the child holding the lock outlived the lock");
    }
}
