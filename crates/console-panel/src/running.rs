//! Starting things, and not waiting for them where waiting would be felt.
//!
//! `left_running` is the one call here that a person made happen: somebody
//! pressed a row and an application starts. So it stamps the press, and the
//! three that nobody pressed -- asking a program a question, saying something
//! on the bar, drawing a picture in the background -- take the stamp back off,
//! because the environment is inherited and a stamp handed on is a wait
//! measured from a press that was already answered.
//!
//! **A program let go of is still a child of whoever started it.** [`LetGo`]
//! holds one so that it can be reaped and never ends it, and nothing here was
//! doing the reaping: every application started from a row and every notice
//! said stayed in the process table as a dead entry until the panel that
//! started it exited. Read on the device that was eleven notices panels, a
//! browser and everything else somebody had opened and closed that afternoon.
//!
//! There is nowhere here to sweep from. A press is a callback and then nothing,
//! and a panel has no loop of its own the way `console-program-runtime` and the
//! controller daemon do -- which is why those two reap where they wait and this
//! cannot. So what has been started is held, and the sweep happens the next
//! time something is started: what is kept is bounded by whatever has ended
//! since the last press, and a desktop nobody is pressing starts nothing and
//! holds nothing new.

use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use console_program_lifetime::{LetGo, Still, let_go};
use console_core_external_programs::Program;
use console_core_never::Never;

static STARTED: Mutex<Vec<LetGo>> = Mutex::new(Vec::new());

pub fn kept(started: LetGo) -> Result<(), Never> {
    let mut held = match STARTED.lock() {
        Ok(held) => held,
        Err(poisoned) => poisoned.into_inner(),
    };

    let Ok(running) = reaped(std::mem::take(&mut *held));

    *held = running;
    held.push(started);

    Ok(())
}

fn reaped(started: Vec<LetGo>) -> Result<Vec<LetGo>, Never> {
    let mut running = Vec::new();

    for mut child in started {
        let Ok(still) = child.still();

        match still {
            Still::Running => running.push(child),
            Still::Ended => {},
        }
    }

    Ok(running)
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

pub fn say(kind: &str, summary: &str, body: &str) -> Result<(), Never> {
    let mut saying = Command::new("console-say");
    saying.args([kind, summary, body]).stdout(Stdio::null()).stderr(Stdio::null());
    let Ok(()) = console_response_times::not_a_press(&mut saying);

    let started = let_go(&mut saying);

    match started {
        Ok(saying) => {
            let Ok(()) = kept(saying);
        },
        Err(fault) => {
            eprintln!("console-say: {fault}");
            eprintln!("{kind}: {summary} - {body}");
        }
    }

    Ok(())
}

pub fn left_running(argv: &[String]) -> Result<(), Never> {
    use std::os::unix::process::CommandExt;

    let (program, rest) = match argv.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Ok(()),
    };

    let Ok((scope, scope_argv)) = scope_around(argv);
    let Ok(has) = has_systemd_run();

    match (scope && has == Has::Yes, scope_argv.split_first()) {
        (true, Some((wrapper, wrapped))) => {
            let mut starting = Command::new(wrapper);
            starting
                .args(wrapped)
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let Ok(()) = console_response_times::pressed_here(&mut starting);

            let Ok(()) = holding(&mut starting);

            return Ok(());
        }
        (true, None) | (false, _) => {},
    }

    match !scope {
        true => {
            eprintln!("left_running: not wrapping {} in a scope: {}", program, argv.join(" "));
        }
        false => {},
    }

    let mut starting = Command::new(program);
    starting
        .args(rest)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let Ok(()) = console_response_times::pressed_here(&mut starting);

    // SAFETY: between the fork and the exec, and setsid is one call that
    unsafe {
        starting.pre_exec(|| {
            libc::setsid();
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

pub fn scope_around(argv: &[String]) -> Result<(bool, Vec<String>), Never> {
    let Ok(systemd_run) = Program::SystemdRun.name();

    let mut wrapped = Vec::with_capacity(argv.len().saturating_add(5));
    wrapped.push(systemd_run.to_string());
    wrapped.push("--user".to_string());
    wrapped.push("--scope".to_string());
    wrapped.push("--".to_string());
    wrapped.extend(argv.iter().cloned());

    Ok((true, wrapped))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Has {
    Yes,
    No,
}

fn has_systemd_run() -> Result<Has, Never> {
    let path = match std::env::var("PATH") {
        Ok(path) => path,
        Err(_fault) => return Ok(Has::No),
    };

    let Ok(systemd_run) = Program::SystemdRun.name();

    let found = path
        .split(':')
        .filter(|at| !at.is_empty())
        .any(|at| std::path::Path::new(at).join(systemd_run).exists());

    Ok(match found {
        true => Has::Yes,
        false => Has::No,
    })
}

pub const PATIENCE: Duration = Duration::from_secs(45);

pub const SETTLING: Duration = Duration::from_millis(250);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wrap_is_in_front_of_everything_else() {
        let argv = vec![
            "firefox".to_string(),
            "--new-window".to_string(),
            "https://example.com".to_string(),
        ];
        let Ok((wrapped, made)) = scope_around(&argv);

        assert!(wrapped, "the wrap was not applied");
        assert_eq!(
            made,
            vec![
                "systemd-run".to_string(),
                "--user".to_string(),
                "--scope".to_string(),
                "--".to_string(),
                "firefox".to_string(),
                "--new-window".to_string(),
                "https://example.com".to_string(),
            ]
        );
    }

    #[test]
    fn a_program_named_like_a_flag_is_kept_a_program() {
        let argv = vec!["--something".to_string()];
        let Ok((_, made)) = scope_around(&argv);

        assert_eq!(made[3], "--", "the terminator is what keeps the program a program");
        assert_eq!(made[4], "--something");
    }

    #[test]
    fn an_empty_argv_still_makes_a_wrapped_one() {
        let Ok((_, made)) = scope_around(&[]);

        let Ok(systemd_run) = Program::SystemdRun.name();

        assert_eq!(made, vec![systemd_run, "--user", "--scope", "--"]);
    }
}
