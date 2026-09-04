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

use std::process::{Child, Command, Stdio};

/// What the machine is asked not to do while an apply is running.
///
/// `shutdown` and `sleep` are the two that would interrupt it. `idle` is here
/// because the idle daemon's own timer does not know an apply from an empty
/// desk, and a device left applying on a table is exactly the case where
/// nobody is touching it.
///
/// Not `handle-lid-switch`: shutting the lid of a handheld is a person saying
/// they are done with it, and an apply is not a reason to argue.
pub const WHAT: &str = "shutdown:sleep:idle";

/// The inhibitor, held for as long as this is alive.
#[derive(Debug)]
pub struct Staying {
    holding: Option<Child>,
}

/// Whether the machine could be asked.
///
/// Not `Clone`: it holds the child that holds the lock, and a second one would
/// be a second owner of a thing that is let go exactly once.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    /// It was, and the lock is held.
    Held(Staying),
    /// It could not be, and this is why.
    ///
    /// Not a failure that stops an apply. Before any of this there was no
    /// inhibitor at all and applies happened anyway; a machine where
    /// `systemd-inhibit` is missing is that machine, and it should be told
    /// rather than refused.
    NotHeld(String),
}

impl PartialEq for Staying {
    /// Two of these are never the same lock, and nothing compares them. It is
    /// here so `Asked` can be compared in a test, where only `NotHeld` is ever
    /// looked at.
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl Eq for Staying {}

/// Ask the machine to stay up.
pub fn taking(why: &str) -> Asked {
    let started = Command::new("systemd-inhibit")
        .args([
            &format!("--what={WHAT}"),
            "--who=console apply",
            &format!("--why={why}"),
            "--mode=block",
            "cat",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    match started {
        Ok(holding) => Asked::Held(Staying { holding: Some(holding) }),
        Err(fault) => Asked::NotHeld(format!(
            "the machine could not be asked to stay up ({fault}), so an apply on a device that \
             suspends or runs out partway will be interrupted"
        )),
    }
}

impl Drop for Staying {
    /// Let go, closing the pipe first.
    ///
    /// The pipe is what `cat` is waiting on and therefore what the lock is
    /// really held by, so it is closed before anything else. The kill and the
    /// wait after it are for tidiness and for not leaving a child nobody
    /// reaped: closing the pipe is already enough on a machine where `cat`
    /// behaves like `cat`.
    fn drop(&mut self) {
        let Some(mut holding) = self.holding.take() else { return };

        drop(holding.stdin.take());
        let _ = holding.kill();
        let _ = holding.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three the machine is asked not to do, named rather than assumed.
    ///
    /// A list that lost `shutdown` would still pass every other test here and
    /// would let the exact thing this exists to prevent happen again.
    #[test]
    fn the_lock_covers_stopping_sleeping_and_going_idle() {
        assert!(WHAT.contains("shutdown"), "{WHAT}");
        assert!(WHAT.contains("sleep"), "{WHAT}");
        assert!(WHAT.contains("idle"), "{WHAT}");
    }

    /// Shutting the lid is a person saying they are done, and an apply does not
    /// get to argue with that.
    #[test]
    fn the_lid_is_left_to_whoever_shut_it() {
        assert!(!WHAT.contains("lid"), "{WHAT}");
    }

    /// A machine with no `systemd-inhibit` is told, not refused. Before this
    /// there was no inhibitor at all and applies happened anyway.
    #[test]
    fn a_machine_that_cannot_be_asked_says_so_rather_than_failing() {
        let Asked::NotHeld(said) = taking_with("this-is-not-a-program-on-any-machine") else {
            panic!("a program that does not exist held a lock");
        };
        assert!(said.contains("stay up"), "{said}");
    }

    /// The same, under a program of the test's choosing, so the missing-program
    /// case can be reached without uninstalling systemd.
    fn taking_with(program: &str) -> Asked {
        match Command::new(program).stdin(Stdio::piped()).spawn() {
            Ok(holding) => Asked::Held(Staying { holding: Some(holding) }),
            Err(fault) => Asked::NotHeld(format!(
                "the machine could not be asked to stay up ({fault}), so an apply on a device \
                 that suspends or runs out partway will be interrupted"
            )),
        }
    }

    /// Letting go closes the pipe and reaps the child, so nothing is left
    /// behind holding a lock nobody can find.
    #[test]
    fn letting_go_ends_the_child_that_was_holding_it() {
        let Ok(holding) = Command::new("cat").stdin(Stdio::piped()).stdout(Stdio::null()).spawn()
        else {
            return;
        };

        let id = holding.id();
        let staying = Staying { holding: Some(holding) };
        drop(staying);

        // Reaped, so the pid is not a process this machine still has waiting.
        let still = std::path::Path::new(&format!("/proc/{id}/stat")).exists();
        assert!(!still, "the child holding the lock outlived the lock");
    }
}
