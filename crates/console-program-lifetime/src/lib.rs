//! How long a child lives, said where it is started rather than left to the
//! paths that happen to reach a kill.
//!
//! A program here starts another for one of two reasons. Either it wants
//! something running for exactly as long as it is running itself -- a `pactl
//! subscribe` under a settings panel, an `nmcli monitor` under the bar, the
//! inhibitor that holds a machine awake across an apply -- or it is starting
//! something on someone's behalf and has no business ending it, which is what
//! a button press does. At the call site the two are the same three lines: a
//! `Command`, a `spawn`, and a `Child` put somewhere. Afterwards they are not
//! the same at all, and the difference is invisible until the day it is not.
//!
//! What it looked like on the day it was not: sixty-five `pactl subscribe`
//! processes on a laptop, one for every settings panel that had ever been
//! opened in a nested desktop and torn down with it. pipewire-pulse serves
//! sixty-four clients and refuses the sixty-fifth, so the volume keys stopped
//! working -- every one of them ends in `pactl`, and every `pactl` was turned
//! away at the door. Nothing was wrong with the volume keys, nothing was wrong
//! with pipewire, and the panel that leaked had a `stop_watching` that killed
//! its watchers correctly.
//!
//! That last part is the whole argument for this crate. A child stopped by a
//! line of code is stopped on the paths that reach that line, and the panel
//! reached it from one of them. [`BoundToParent`] is stopped instead by two things
//! that nothing has to remember to reach:
//!
//! 1. `PR_SET_PDEATHSIG`, so the kernel signals the child when the thread that
//!    started it goes. This is the one that holds when the parent is killed
//!    outright and no code of ours runs at all, which is how those panels died.
//! 2. `Drop`, so the child also goes when whatever held it is dropped while
//!    this program carries on: a panel closed, a page left, an apply finished.
//!
//! Neither is enough by itself. The signal is armed per thread and says nothing
//! when a program merely stops wanting the child; the drop never happens when
//! the program is killed. Together they cover the ways a parent ends.
//!
//! [`Detached`] is the other answer, and deliberately has none of that. A program
//! launched by a press is the person's, not ours -- a controller daemon
//! restarting is not a reason for what someone started to close -- so this
//! holds the child only so that it can be reaped, and never ends it. Both types
//! exist so that a caller has to say which it meant, in a word that is still
//! there to read a year later.
//!
//! Between them they are the only places in the workspace that name a
//! [`std::process::Child`], which is what
//! `console-manifest-engine/tests/the_children.rs` holds shut. A `Command` that
//! is run to completion with `status` or `output` is no one's business here: it
//! is over before the call returns and cannot be left behind.
//!
//! [`threads`] is the same question asked about a thread, which was left out of
//! this for years because a handle is cheap to drop. Only half of it carries
//! over -- a thread cannot be ended from outside -- so what is there is the
//! `Detached` half and the word for it, and EXPLICIT035 asks at every `spawn`.
//!
//! Both answers reach one process, and a process is not always one process.
//! [`BoundToParent`] kills the child it was handed; what that child started is
//! reparented to the user manager and carries on, in the control group of
//! whoever is logged in, where nothing can tell it from the desktop someone is
//! using. A nested desktop is the whole of that fault: the compositor dies with
//! the run and its session, its bar and its keyboard are still there an hour
//! later, and the only way anyone found them was `ps`.
//!
//! `in_a_scope_of_its_own` is the answer to what neither type can reach. A
//! transient scope is a control group with a name this tree chose, so what a
//! program starts at any depth is in it, and `nothing_left_in` ends all of it
//! at once without naming a single pid -- which matters, because the names are
//! the session's own: `pgrep -x Hyprland` on this laptop matches the compositor
//! the person is looking at.
//!
//! It is a thing to reach for deliberately and not a thing to wrap every
//! `alongside` in: a scope is a round trip to the user manager, which is worth
//! paying once for a run and not once for every watcher a panel starts. Where a
//! run is wrapped and why is in the justfile.
//!
//! The other place it is paid is the nested compositor itself, which wraps its
//! own session in `console-test-desktop`. A run is only wrapped when someone
//! typed `just`, and the panel tier is a `cargo test` like any other: for an
//! afternoon the justfile's scope was taken for the whole of this, and the
//! count went on climbing underneath it -- eight hundred processes, because
//! what had been run was `cargo test` and not `just test`. A scope inside a
//! scope is a sibling rather than a child, so the two do not reach each other
//! and the inner one is the one that holds.
//!
//! A machine with no user manager gets the arguments it handed in. That is most of
//! what is not a desktop, and it is not a fault: `BoundToParent` still holds, and a
//! machine with no manager to leave something behind in mostly has no session
//! to leave it in.

pub mod threads;

#[cfg(target_os = "linux")]
pub use console_program_lifetime_linux as linux;

#[cfg(target_os = "linux")]
pub use console_program_scopes_systemd as systemd;
#[cfg(target_os = "linux")]
pub use systemd::{Scopes, Wrapped, in_a_scope_of_its_own, nothing_left_in, scopes};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::signal;

use console_core_never::Never;
use std::io;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus};

#[derive(Debug)]
pub struct BoundToParent {
    child: Child,
}

#[cfg(target_os = "linux")]
pub fn alongside(command: &mut Command) -> io::Result<BoundToParent> {
    let Ok(()) = linux::bind(command);
    let child = command.spawn()?;
    Ok(BoundToParent { child })
}

impl BoundToParent {
    pub fn take_stdout(&mut self) -> Result<Option<ChildStdout>, Never> {
        Ok(self.child.stdout.take())
    }

    pub fn take_stdin(&mut self) -> Result<Option<ChildStdin>, Never> {
        Ok(self.child.stdin.take())
    }

    pub fn take_stderr(&mut self) -> Result<Option<ChildStderr>, Never> {
        Ok(self.child.stderr.take())
    }

    pub fn id(&self) -> Result<u32, Never> {
        Ok(self.child.id())
    }

    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }

    pub fn still(&mut self) -> Result<Still, Never> {
        still(&mut self.child)
    }
}

impl Drop for BoundToParent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Debug)]
pub struct Detached {
    child: Child,
}

pub fn let_go(command: &mut Command) -> io::Result<Detached> {
    let child = command.spawn()?;

    Ok(Detached { child })
}

impl Detached {
    pub fn take_stdin(&mut self) -> Result<Option<ChildStdin>, Never> {
        Ok(self.child.stdin.take())
    }

    pub fn still(&mut self) -> Result<Still, Never> {
        still(&mut self.child)
    }

    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }

    pub fn id(&self) -> Result<u32, Never> {
        Ok(self.child.id())
    }
}

pub fn reap(started: Vec<Detached>) -> Result<Vec<Detached>, Never> {
    Ok(started
        .into_iter()
        .filter_map(|mut one| {
            let Ok(still) = one.still();

            match still {
                Still::Running => Some(one),
                Still::Ended => None,
            }
        })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Still {
    Running,
    Ended,
}

fn still(child: &mut Child) -> Result<Still, Never> {
    Ok(match child.try_wait() {
        Ok(None) => Still::Running,
        Ok(Some(_ended)) => Still::Ended,
        Err(_unasked) => Still::Ended,
    })
}

#[cfg(all(test, target_os = "linux"))]
mod tests;
