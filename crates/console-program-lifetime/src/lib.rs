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
//! [`in_a_scope_of_its_own`] is the answer to what neither type can reach. A
//! transient scope is a control group with a name this tree chose, so what a
//! program starts at any depth is in it, and [`nothing_left_in`] ends all of it
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

use console_core_never::Never;
use std::io;
use std::os::unix::process::CommandExt;
use console_core_external_programs::{Installed, Program, installed};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::ffi::c_int;

unsafe extern "C" {
    fn prctl(option: c_int, ...) -> c_int;

    fn getppid() -> c_int;
}

const PR_SET_PDEATHSIG: c_int = 1;
const SIGTERM: c_int = 15;

#[derive(Debug)]
pub struct BoundToParent {
    child: Child,
}

pub fn signal(pid: i32, signal: rustix::process::Signal) -> Result<(), Never> {
    let sent = rustix::process::Pid::from_raw(pid).map(|pid| rustix::process::kill_process(pid, signal));

    match sent {
        Some(Ok(())) => {},
        Some(Err(_already_gone)) => {},
        None => {},
    }

    Ok(())
}

pub fn alongside(command: &mut Command) -> io::Result<BoundToParent> {
    let Ok(()) = dying_with_us(command);

    let child = command.spawn()?;

    Ok(BoundToParent { child })
}

impl BoundToParent {
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
    pub fn writing(&mut self) -> Result<Option<ChildStdin>, Never> {
        Ok(self.child.stdin.take())
    }

    pub fn still(&mut self) -> Result<Still, Never> {
        still(&mut self.child)
    }

    pub fn waiting(&mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }

    pub fn id(&self) -> Result<u32, Never> {
        Ok(self.child.id())
    }
}

pub fn reaped(started: Vec<Detached>) -> Result<Vec<Detached>, Never> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrapped {
    InAScope,
    AsItWasHandedIn,
}

pub fn in_a_scope_of_its_own(named: Option<&str>, arguments: &[String]) -> Result<(Wrapped, Vec<String>), Never> {
    let Ok(scopes) = scopes();

    match scopes {
        Scopes::Unavailable => return Ok((Wrapped::AsItWasHandedIn, arguments.to_vec())),
        Scopes::Available => {},
    }

    let unit: Vec<String> = named
        .into_iter()
        .map(|named| format!("--unit={named}"))
        .collect();

    let Ok(said) = Program::SystemdRun.words(
        ["--user".to_string(), "--scope".to_string(), "--quiet".to_string()]
            .into_iter()
            .chain(unit)
            .chain(std::iter::once("--".to_string()))
            .chain(arguments.iter().cloned())
            .collect(),
    );

    Ok((Wrapped::InAScope, said))
}

pub fn nothing_left_in(named: &str) -> Result<(), Never> {
    let Ok(scopes) = scopes();

    match scopes {
        Scopes::Unavailable => return Ok(()),
        Scopes::Available => {},
    }

    let Ok(mut asking) = Program::Systemctl.command();

    let _ended = asking
        .args(["--user", "stop", "--no-block", &format!("{named}.scope")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scopes {
    Available,
    Unavailable,
}

pub fn scopes() -> Result<Scopes, Never> {
    let Ok(systemd_run) = Program::SystemdRun.name();
    let Ok(found) = installed(systemd_run);

    match found {
        Installed::No => return Ok(Scopes::Unavailable),
        Installed::Yes => {},
    }

    let Ok(mut asking) = Program::Systemctl.command();
    let answered = asking
        .args(["--user", "show", "-p", "Version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    Ok(match answered {
        Ok(how) => match how.success() {
            true => Scopes::Available,
            false => Scopes::Unavailable,
        },
        Err(_no_systemctl) => Scopes::Unavailable,
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
            let _ = prctl(PR_SET_PDEATHSIG, SIGTERM);

            match u32::try_from(getppid()) {
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
mod scopes {
    use super::*;

    #[test]
    fn the_wrap_is_in_front_of_everything_else() {
        let arguments = vec![
            "firefox".to_string(),
            "--new-window".to_string(),
            "https://example.com".to_string(),
        ];
        let Ok((wrapped, made)) = in_a_scope_of_its_own(None, &arguments);

        match wrapped {
            Wrapped::AsItWasHandedIn => {
                eprintln!("skipped: no systemd-run on PATH; scopes cannot be made");

                return;
            }
            Wrapped::InAScope => {},
        }

        assert_eq!(
            made,
            vec![
                "systemd-run".to_string(),
                "--user".to_string(),
                "--scope".to_string(),
                "--quiet".to_string(),
                "--".to_string(),
                "firefox".to_string(),
                "--new-window".to_string(),
                "https://example.com".to_string(),
            ]
        );
    }

    #[test]
    fn a_program_named_like_a_flag_is_kept_a_program() {
        let arguments = vec!["--something".to_string()];
        let Ok((wrapped, made)) = in_a_scope_of_its_own(None, &arguments);

        match wrapped {
            Wrapped::AsItWasHandedIn => return,
            Wrapped::InAScope => {},
        }

        assert_eq!(made.get(4).map(String::as_str), Some("--"), "the terminator is what keeps the program a program");
        assert!(made.iter().take(4).all(|word| word != "--"), "the terminator is what keeps the program a program");
        assert_eq!(made.last().map(String::as_str), Some("--something"));
    }

    #[test]
    fn a_scope_that_is_named_can_be_stopped_by_that_name() {
        let arguments = vec!["sleep".to_string()];
        let Ok((wrapped, made)) = in_a_scope_of_its_own(Some("console-run-1"), &arguments);

        match wrapped {
            Wrapped::AsItWasHandedIn => return,
            Wrapped::InAScope => {},
        }

        assert!(
            made.contains(&"--unit=console-run-1".to_string()),
            "a scope nothing named is a scope nothing can stop: {made:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    use console_core_external_programs::Program;

    use super::*;

    unsafe extern "C" {
        fn kill(process: c_int, signal: c_int) -> c_int;
    }

    const SIGKILL: c_int = 9;

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
                kill(pid, SIGKILL);
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
    fn a_child_let_go_that_ended_is_reaped_and_one_still_going_is_kept() {
        let Ok(mut command) = Program::True.command();

        command.stdout(Stdio::null());

        let done = match let_go(&mut command) {
            Ok(done) => done,
            Err(_fault) => return,
        };
        let Ok(ended) = done.id();

        let mut command = holding();
        let going = match let_go(&mut command) {
            Ok(going) => going,
            Err(_fault) => return,
        };
        let Ok(held) = going.id();

        let mut started = vec![done, going];
        let Ok(patience) = console_waiting::Schedule::of(Duration::from_secs(2));
        let Ok(_over) = console_waiting::until(patience, || {
            let Ok(still) = reaped(std::mem::take(&mut started));

            started = still;

            Ok(match started.len() {
                1 => console_waiting::Ready::Yes,
                _more => console_waiting::Ready::NotYet,
            })
        });

        let kept: Vec<u32> = started.iter().filter_map(|one| one.id().ok()).collect();

        assert_eq!(kept, [held]);
        assert!(!std::path::Path::new(&format!("/proc/{ended}")).exists(), "{ended} is still in the process table");

        for mut one in started {
            let _ = one.child.kill();
            let _ = one.waiting();
        }
    }

    #[test]
    fn a_parent_killed_outright_takes_the_child_with_it() {
        let ours = std::env::current_exe().unwrap_or_default();
        let executable = match ours.to_str() {
            Some(executable) => executable,
            None => return,
        };

        let Ok(mut command) = Program::Sh.command();

        command
            .args(["-c", &format!("exec {executable} --nocapture the_child_this_test_starts")])
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
                kill(pid, SIGKILL);
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
