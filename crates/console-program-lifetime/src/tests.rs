use std::error::Error;
use std::ffi::c_int;
use std::io::BufRead;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use console_core_external_programs::Program;
use console_waiting::{Outcome, Ready, Schedule, until, until_handed};

use super::*;

unsafe extern "C" {
    fn kill(process: c_int, signal: c_int) -> c_int;
}

const SIGKILL: c_int = 9;

fn listed(id: u32) -> Result<PathBuf, Never> {
    Ok(PathBuf::from(format!("/proc/{id}")))
}

fn holding() -> Result<Command, Never> {
    let Ok(mut command) = Program::Sh.command();

    command
        .args(["-c", "exec sleep 600"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    Ok(command)
}

fn killed(id: u32) -> Result<(), Box<dyn Error>> {
    let pid = i32::try_from(id)?;

    // SAFETY: a signal to a process this test started, by the pid it was
    // given, while it is still held here and its pid cannot be reused.
    unsafe {
        kill(pid, SIGKILL);
    }

    Ok(())
}

#[test]
fn dropping_it_ends_the_child() -> Result<(), Box<dyn Error>> {
    let Ok(mut command) = holding();
    let running = alongside(&mut command)?;
    let Ok(id) = running.id();
    let Ok(at) = listed(id);

    assert!(at.exists(), "it never started");

    drop(running);

    assert!(!at.exists(), "the child outlived the thing holding it");

    Ok(())
}

#[test]
fn a_child_let_go_is_still_running_after_the_drop() -> Result<(), Box<dyn Error>> {
    let Ok(mut command) = holding();
    let running = let_go(&mut command)?;
    let Ok(id) = running.id();
    let Ok(at) = listed(id);

    drop(running);

    assert!(at.exists(), "letting go ended it anyway");

    killed(id)
}

#[test]
fn the_lines_it_says_are_readable_while_it_is_held() -> Result<(), Box<dyn Error>> {
    let Ok(mut command) = Program::Sh.command();

    command
        .args(["-c", "printf 'one\\ntwo\\n'"])
        .stdout(Stdio::piped());

    let mut running = alongside(&mut command)?;
    let Ok(out) = running.take_stdout();
    let out = out.ok_or("no pipe from a piped command")?;
    let said = std::io::read_to_string(out)?;
    let Ok(again) = running.take_stdout();

    assert_eq!(said, "one\ntwo\n");
    assert!(again.is_none(), "the pipe was handed out twice");

    Ok(())
}

#[test]
fn one_that_ended_says_so_and_one_that_has_not_says_so() -> Result<(), Box<dyn Error>> {
    let Ok(mut command) = Program::True.command();

    command.stdout(Stdio::null());

    let mut done = alongside(&mut command)?;
    let _ = done.wait();
    let Ok(ended) = done.still();

    assert_eq!(ended, Still::Ended);

    let Ok(mut command) = holding();
    let mut going = alongside(&mut command)?;
    let Ok(running) = going.still();

    assert_eq!(running, Still::Running);

    Ok(())
}

#[test]
fn a_child_let_go_that_ended_is_reaped_and_one_still_going_is_kept() -> Result<(), Box<dyn Error>> {
    let Ok(mut command) = Program::True.command();

    command.stdout(Stdio::null());

    let done = let_go(&mut command)?;
    let Ok(ended) = done.id();

    let Ok(mut command) = holding();
    let going = let_go(&mut command)?;
    let Ok(held) = going.id();

    let mut started = vec![done, going];
    let Ok(patience) = Schedule::of(Duration::from_secs(2));
    let Ok(_over) = until_handed(patience, &mut started, |started| {
        let Ok(still) = reap(std::mem::take(started));

        *started = still;

        Ok(match started.len() {
            1 => Ready::Yes,
            _more => Ready::NotYet,
        })
    });

    let kept: Vec<u32> = started
        .iter()
        .map(|one| {
            let Ok(id) = one.id();

            id
        })
        .collect();
    let Ok(at) = listed(ended);

    assert_eq!(kept, [held]);
    assert!(!at.exists(), "{ended} is still in the process table");

    for mut one in started {
        let _ = one.child.kill();
        let _ = one.wait();
    }

    Ok(())
}

#[test]
fn a_parent_killed_outright_takes_the_child_with_it() -> Result<(), Box<dyn Error>> {
    let this_test = std::env::current_exe()?;
    let exe = this_test
        .to_str()
        .ok_or("this test is not at a path that can be said")?;
    let Ok(mut command) = Program::Sh.command();

    command
        .args([
            "-c",
            &format!("exec {exe} --nocapture the_child_this_test_starts"),
        ])
        .env(HOLDING, "yes")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut parent = let_go(&mut command)?;
    let out = parent
        .child
        .stdout
        .take()
        .ok_or("no pipe from the parent")?;
    let mut heard = Vec::new();
    let mut found = None;

    for line in std::io::BufReader::new(out).lines() {
        let line = line?;

        match line.trim().parse::<u32>() {
            Ok(id) => {
                found = Some(id);

                break;
            }
            Err(_not_a_number) => heard.push(line),
        }
    }

    let id = found.ok_or_else(|| format!("the parent never said what it started: {heard:?}"))?;
    let Ok(whose) = parent.id();

    killed(whose)?;

    let Ok(at) = listed(id);
    let Ok(patience) = Schedule::of(Duration::from_secs(10));
    let Ok(gone) = until(patience, || {
        Ok(match at.exists() {
            true => Ready::NotYet,
            false => Ready::Yes,
        })
    });

    assert_eq!(
        gone,
        Outcome::Happened,
        "the child outlived a killed parent"
    );

    Ok(())
}

const HOLDING: &str = "CONSOLE_CHILD_PROCESSES_HOLDING";

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "the test above starts this one with HOLDING set, and the environment is the only thing a test harness passes through to the test it runs"
    )
)]
#[test]
fn the_child_this_test_starts() -> Result<(), Box<dyn Error>> {
    match std::env::var(HOLDING) {
        Ok(_asked) => {}
        Err(_not) => return Ok(()),
    }

    let Ok(mut command) = holding();
    let running = alongside(&mut command)?;
    let Ok(id) = running.id();

    println!("{id}");

    let Ok(patience) = Schedule::of(Duration::from_secs(120));
    let Ok(_held) = until(patience, || Ok(Ready::NotYet));

    Ok(())
}
