//! How long a child lives, said where it is started rather than left to the
//! paths that happen to reach a kill.
//!
//! A program here starts another for one of two reasons. Either it wants
//! something running for exactly as long as it is running itself -- a `pactl
//! subscribe` under a settings panel, an `nmcli monitor` under the bar, the
//! inhibitor that holds a machine awake across an apply -- or it is starting
//! something on somebody's behalf and has no business ending it, which is what
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
//! reached it from one of them. [`Alongside`] is stopped instead by two things
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
//! [`LetGo`] is the other answer, and deliberately has none of that. A program
//! launched by a press is the person's, not ours -- a controller daemon
//! restarting is not a reason for what somebody started to close -- so this
//! holds the child only so that it can be reaped, and never ends it. Both types
//! exist so that a caller has to say which it meant, in a word that is still
//! there to read a year later.
//!
//! Between them they are the only places in the workspace that name a
//! [`std::process::Child`], which is what
//! `console-manifest-engine/tests/the_children.rs` holds shut. A `Command` that
//! is run to completion with `status` or `output` is nobody's business here: it
//! is over before the call returns and cannot be left behind.

use console_core_never::Never;
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus};

#[derive(Debug)]
pub struct Alongside {
    child: Child,
}

pub fn alongside(command: &mut Command) -> io::Result<Alongside> {
    let Ok(()) = dying_with_us(command);

    let child = command.spawn()?;

    Ok(Alongside { child })
}

impl Alongside {
    pub fn reading(&mut self) -> Result<Option<ChildStdout>, Never> {
        Ok(self.child.stdout.take())
    }

    pub fn writing(&mut self) -> Result<Option<ChildStdin>, Never> {
        Ok(self.child.stdin.take())
    }

    pub fn erring(&mut self) -> Result<Option<ChildStderr>, Never> {
        Ok(self.child.stderr.take())
    }

    pub fn id(&self) -> Result<u32, Never> {
        Ok(self.child.id())
    }

    pub fn waiting(&mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }

    pub fn still(&mut self) -> Result<Still, Never> {
        still(&mut self.child)
    }
}

impl Drop for Alongside {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Debug)]
pub struct LetGo {
    child: Child,
}

pub fn let_go(command: &mut Command) -> io::Result<LetGo> {
    let child = command.spawn()?;

    Ok(LetGo { child })
}

impl LetGo {
    pub fn still(&mut self) -> Result<Still, Never> {
        still(&mut self.child)
    }

    pub fn id(&self) -> Result<u32, Never> {
        Ok(self.child.id())
    }
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
        Err(_fault) => Still::Ended,
    })
}

fn dying_with_us(command: &mut Command) -> Result<(), Never> {
    let whose = std::process::id();

    // SAFETY: what runs between the fork and the exec is two calls, prctl and
    // getppid, both async-signal-safe, and nothing here allocates or takes a
    // lock. The error built on the way out is the child's own exit and never
    // runs in the parent.
    unsafe {
        command.pre_exec(move || {
            let _ = libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);

            match u32::try_from(libc::getppid()) {
                Ok(now) => match now == whose {
                    true => Ok(()),
                    false => Err(io::Error::other("whoever wanted this is already gone")),
                },
                Err(_fault) => Err(io::Error::other("this has no parent left to outlive")),
            }
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    use console_core_external_programs::Program;

    use super::*;

    fn still_there(id: u32) -> bool {
        std::path::Path::new(&format!("/proc/{id}")).exists()
    }

    fn gone_within(id: u32, waiting: Duration) -> bool {
        let by = Instant::now() + waiting;

        while Instant::now() < by {
            match still_there(id) {
                true => std::thread::sleep(Duration::from_millis(10)),
                false => return true,
            }
        }

        !still_there(id)
    }

    fn holding() -> Command {
        let Ok(mut command) = Program::Sh.command();

        command
            .args(["-c", "exec sleep 600"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    #[test]
    fn dropping_it_ends_the_child() {
        let mut command = holding();
        let running = match alongside(&mut command) {
            Ok(running) => running,
            Err(_fault) => return,
        };

        let Ok(id) = running.id();

        assert!(still_there(id), "it never started");

        drop(running);

        assert!(!still_there(id), "the child outlived the thing holding it");
    }

    #[test]
    fn a_child_let_go_is_still_running_after_the_drop() {
        let mut command = holding();
        let running = match let_go(&mut command) {
            Ok(running) => running,
            Err(_fault) => return,
        };

        let Ok(id) = running.id();

        drop(running);

        assert!(still_there(id), "letting go ended it anyway");

        // SAFETY: a signal to a child this test started, by the pid it was
        // given, and nothing else can have that pid while it is unreaped.
        match i32::try_from(id) {
            Ok(pid) => unsafe {
                libc::kill(pid, libc::SIGKILL);
            },
            Err(_fault) => {},
        }
    }

    #[test]
    fn the_lines_it_says_are_readable_while_it_is_held() {
        let Ok(mut command) = Program::Sh.command();

        command.args(["-c", "printf 'one\\ntwo\\n'"]).stdout(Stdio::piped());

        let mut running = match alongside(&mut command) {
            Ok(running) => running,
            Err(_fault) => return,
        };
        let out = match running.reading() {
            Ok(Some(out)) => out,
            Ok(None) | Err(_) => panic!("no pipe from a piped command"),
        };

        let said = std::io::read_to_string(out).unwrap_or_default();
        let Ok(again) = running.reading();

        assert_eq!(said, "one\ntwo\n");
        assert!(again.is_none(), "the pipe was handed out twice");
    }

    #[test]
    fn one_that_ended_says_so_and_one_that_has_not_says_so() {
        let Ok(mut command) = Program::True.command();

        command.stdout(Stdio::null());

        let mut done = match alongside(&mut command) {
            Ok(done) => done,
            Err(_fault) => return,
        };
        let _ = done.waiting();

        let Ok(ended) = done.still();

        assert_eq!(ended, Still::Ended);

        let mut command = holding();
        let mut going = match alongside(&mut command) {
            Ok(going) => going,
            Err(_fault) => return,
        };

        let Ok(running) = going.still();

        assert_eq!(running, Still::Running);
    }

    #[test]
    fn a_parent_killed_outright_takes_the_child_with_it() {
        let ours = std::env::current_exe().unwrap_or_default();
        let exe = match ours.to_str() {
            Some(exe) => exe,
            None => return,
        };

        let Ok(mut command) = Program::Sh.command();

        command
            .args(["-c", &format!("exec {exe} --nocapture the_child_this_test_starts")])
            .env(HOLDING, "yes")
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut parent = match let_go(&mut command) {
            Ok(parent) => parent,
            Err(_fault) => return,
        };

        let out = match parent.child.stdout.take() {
            Some(out) => out,
            None => return,
        };

        let mut heard = Vec::new();
        let by = Instant::now() + Duration::from_secs(20);
        let mut lines = std::io::BufReader::new(out);
        let mut found = None;

        while Instant::now() < by && found.is_none() {
            let mut said = String::new();

            match std::io::BufRead::read_line(&mut lines, &mut said) {
                Ok(0) => break,
                Ok(_read) => {
                    found = said.trim().parse::<u32>().ok();
                    heard.push(said);
                }
                Err(_fault) => break,
            }
        }

        let id = match found {
            Some(id) => id,
            None => panic!("the parent never said what it started: {heard:?}"),
        };

        let Ok(whose) = parent.id();

        // SAFETY: a signal to a process this test started, by the pid it was
        // given, while it is still held here and its pid cannot be reused.
        match i32::try_from(whose) {
            Ok(pid) => unsafe {
                libc::kill(pid, libc::SIGKILL);
            },
            Err(_fault) => return,
        }

        assert!(gone_within(id, Duration::from_secs(10)), "the child outlived a killed parent");
    }

    const HOLDING: &str = "CONSOLE_CHILD_PROCESSES_HOLDING";

    #[test]
    fn the_child_this_test_starts() {
        match std::env::var(HOLDING) {
            Ok(_asked) => {},
            Err(_not) => return,
        }

        let mut command = holding();
        let running = match alongside(&mut command) {
            Ok(running) => running,
            Err(_fault) => return,
        };

        let Ok(id) = running.id();

        println!("{id}");

        std::thread::sleep(Duration::from_secs(120));
    }
}
