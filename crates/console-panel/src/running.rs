//! Starting things, and not waiting for them where waiting would be felt.
//!
//! `left_running` is the one call here that a person made happen: someone
//! pressed a row and an application starts. So it stamps the press, and the
//! three that no one pressed -- asking a program a question, saying something
//! on the bar, drawing a picture in the background -- take the stamp back off,
//! because the environment is inherited and a stamp handed on is a wait
//! measured from a press that was already answered.
//!
//! **A program let go of is still a child of whoever started it.** [`Detached`]
//! holds one so that it can be reaped and never ends it, and nothing here was
//! doing the reaping: every application started from a row and every notification
//! said stayed in the process table as a dead entry until the panel that
//! started it exited. Read on the device that was eleven notifications panels, a
//! browser and everything else someone had opened and closed that afternoon.
//!
//! [`and_waited`] is the other half of that: a row that asks for something to
//! be run to the end rather than started and left. It is waited for on a thread
//! of its own because the thread that called it is the one holding the frame,
//! and a panel that stops drawing while a program runs is a panel nobody can
//! put away.
//!
//! What has been started is waited for on a thread of its own, and reaped
//! the moment it ends. It used to be held in a list and swept the next time
//! something was started, which was bounded while every panel was a process
//! that exited when it closed. The panels are held by one resident program
//! now, and a sweep that waits for the next press is a dead entry under the
//! host for as long as nobody presses anything -- which is exactly the state
//! a closed panel is meant to cost nothing in.

use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use console_core_internal_programs::InternalProgram;
use console_program_lifetime::threads;
use console_program_lifetime::{Detached, Wrapped, in_a_scope_of_its_own, let_go};
use console_core_external_programs::Program;
use console_core_never::Never;

pub fn kept(mut started: Detached) -> Result<(), Never> {
    let waiting = thread::spawn(move || {
        let _ = started.waiting();
    });

    threads::let_go(waiting)
}

pub fn said(program: Program, rest: &[&str]) -> Result<String, Never> {
    let Ok(mut asking) = program.command();

    asking.args(rest);

    let Ok(()) = console_response_times::not_a_press(&mut asking);

    let done = match asking.output() {
        Ok(done) => done,
        Err(_fault) => return Ok(String::new()),
    };

    Ok(String::from_utf8_lossy(&done.stdout).trim().to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Notification<'a> {
    pub summary: &'a str,
    pub body: &'a str,
}

pub fn say(kind: &str, said: Notification<'_>) -> Result<(), Never> {
    let Ok(mut saying) = InternalProgram::ConsoleSay.command();
    saying.args([kind, said.summary, said.body]).stdout(Stdio::null()).stderr(Stdio::null());
    let Ok(()) = console_response_times::not_a_press(&mut saying);

    let started = let_go(&mut saying);

    match started {
        Ok(saying) => {
            let Ok(()) = kept(saying);
        },
        Err(fault) => {
            eprintln!("console-say: {fault}");
            eprintln!("{kind}: {} - {}", said.summary, said.body);
        }
    }

    Ok(())
}

pub fn and_waited(arguments: Vec<String>) -> Result<(), Never> {
    let doing = thread::spawn(move || {
        let (program, rest) = match arguments.split_first() {
            Some((program, rest)) => (program, rest),
            None => return,
        };

        let mut doing = Command::new(program);
        doing.args(rest).stdout(Stdio::null()).stderr(Stdio::null());

        let Ok(()) = console_response_times::not_a_press(&mut doing);

        let _ = doing.status();
    });

    threads::let_go(doing)
}

pub fn left_running(arguments: &[String]) -> Result<(), Never> {
    use std::os::unix::process::CommandExt;

    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Ok(()),
    };

    let Ok((scope, scope_argv)) = in_a_scope_of_its_own(None, arguments);

    match (scope, scope_argv.split_first()) {
        (Wrapped::InAScope, Some((wrapper, wrapped))) => {
            let mut starting = Command::new(wrapper);
            starting
                .args(wrapped)
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let Ok(()) = console_response_times::pressed_here(&mut starting);

            let Ok(()) = holding(&mut starting);

            return Ok(());
        }
        (Wrapped::InAScope, None) | (Wrapped::AsItWasHandedIn, _) => {},
    }

    match scope {
        Wrapped::AsItWasHandedIn => {
            eprintln!("left_running: not wrapping {} in a scope: {}", program, arguments.join(" "));
        }
        Wrapped::InAScope => {},
    }

    let mut starting = Command::new(program);
    starting
        .args(rest)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let Ok(()) = console_response_times::pressed_here(&mut starting);

    // SAFETY: between the fork and the exec, and setsid is one call that
    // allocates nothing and touches nothing this side of the fork holds.
    unsafe {
        starting.pre_exec(|| {
            let _ = rustix::process::setsid();

            Ok(())
        })
    };

    let Ok(()) = holding(&mut starting);

    Ok(())
}

fn holding(starting: &mut Command) -> Result<(), Never> {
    match let_go(starting) {
        Ok(started) => {
            let Ok(()) = kept(started);
        },
        Err(fault) => {
            eprintln!("left_running: nothing started: {fault}");
        },
    }

    Ok(())
}

pub const PATIENCE: Duration = Duration::from_secs(45);

pub const SETTLING: Duration = Duration::from_millis(250);

#[cfg(test)]
mod tests {
    use super::*;

    use console_waiting::{Outcome, Ready, Schedule, until};

    #[test]
    fn a_program_let_go_is_reaped_when_it_ends_rather_than_at_the_next_press() {
        let mut ending = Program::True.command().expect("a command");
        let started = let_go(&mut ending).expect("started");
        let at = std::path::PathBuf::from(format!("/proc/{}", started.id().expect("an id")));

        kept(started).expect("kept");

        let gone = until(Schedule::of(Duration::from_secs(5)).expect("a patience"), || {
            Ok(match at.exists() {
                true => Ready::NotYet,
                false => Ready::Yes,
            })
        });

        assert_eq!(
            gone,
            Ok(Outcome::Happened),
            "{} is still in the process table after it ended, so a host that stays up holds a \
             dead entry for every picture it asked for until someone presses something",
            at.display()
        );
    }
}
