//! The machine, asked to stay up until the apply has finished.
//!
//! `enough` keeps an apply from starting on a battery that will not last it.
//! This is the other half, and it is about the things that stop a machine for
//! reasons that have nothing to do with the battery: the idle daemon reaching
//! the end of its timer, a lid, somebody's own `systemctl suspend`, and the
//! protect step arriving anyway on an apply that was long rather than
//! ill-timed.
//!
//! systemd already has the answer and nothing here was asking for it. A delay
//! inhibitor is a promise from the manager that it will not begin a shutdown,
//! a sleep or an idle action while somebody is holding one, and it is released
//! when the holder lets go or dies. That is the same shape as the lock in
//! `alone`, for the same span, and it is taken on the same line.
//!
//! # Why this is a child process and not a dbus call
//!
//! An inhibitor is a file descriptor the manager hands back over dbus, and
//! this crate speaks to systemd by running `systemctl`. Rather than take a dbus
//! dependency for one call, the lock is held the way `systemd-inhibit` was
//! written to hold it: it takes the lock, runs something, and releases it when
//! that something ends. What it runs here is `cat`, reading a pipe whose other
//! end this process holds.
//!
//! That indirection is doing real work. The pipe closes when this process ends
//! -- returned, panicked, or killed outright -- because the kernel closes it,
//! not because any code here remembered to. `cat` reads end-of-file, exits, and
//! `systemd-inhibit` releases the lock on the way out. An apply that is killed
//! partway therefore leaves no inhibitor behind, which matters more than it
//! sounds: a lock nobody is holding and nobody can find is a machine that has
//! quietly stopped being able to suspend.

use std::process::Stdio;

use console_program_lifetime::{Alongside, alongside};
use console_core_external_programs::Program;
use console_core_never::Never;

pub const WHAT: &str = "shutdown:sleep:idle";

#[derive(Debug)]
pub struct Staying {
    holding: Option<Alongside>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    Held(Staying),
    NotHeld(String),
}

impl PartialEq for Staying {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl Eq for Staying {}

pub fn taking(why: &str) -> Result<Asked, Never> {
    let Ok(mut asking) = Program::SystemdInhibit.command();
    let Ok(cat) = Program::Cat.name();

    asking
        .args([
            &format!("--what={WHAT}"),
            "--who=console apply",
            &format!("--why={why}"),
            "--mode=block",
            cat,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let started = alongside(&mut asking);

    Ok(match started {
        Ok(holding) => Asked::Held(Staying { holding: Some(holding) }),
        Err(fault) => Asked::NotHeld(format!(
            "the machine could not be asked to stay up ({fault}), so an apply on a device that \
             suspends or runs out partway will be interrupted"
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
    fn the_lock_covers_stopping_sleeping_and_going_idle() {
        assert!(WHAT.contains("shutdown"), "{WHAT}");
        assert!(WHAT.contains("sleep"), "{WHAT}");
        assert!(WHAT.contains("idle"), "{WHAT}");
    }

    #[test]
    fn the_lid_is_left_to_whoever_shut_it() {
        assert!(!WHAT.contains("lid"), "{WHAT}");
    }

    #[test]
    fn a_machine_that_cannot_be_asked_says_so_rather_than_failing() {
        let said = match taking_with("this-is-not-a-program-on-any-machine") {
            Asked::NotHeld(said) => said,
            Asked::Held(_) => panic!("a program that does not exist held a lock"),
        };
        assert!(said.contains("stay up"), "{said}");
    }

    fn taking_with(program: &str) -> Asked {
        let mut asking = Command::new(program);
        asking.stdin(Stdio::piped());

        match alongside(&mut asking) {
            Ok(holding) => Asked::Held(Staying { holding: Some(holding) }),
            Err(fault) => Asked::NotHeld(format!(
                "the machine could not be asked to stay up ({fault}), so an apply on a device \
                 that suspends or runs out partway will be interrupted"
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
