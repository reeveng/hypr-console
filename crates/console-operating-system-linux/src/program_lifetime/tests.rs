use std::error::Error;
use std::io::{BufRead, Read};
use std::process::Stdio;
use std::time::Duration;

use console_core_external_programs::Program;
use console_program_lifetime::{let_go, reap};
use console_waiting::{Outcome, Ready, Schedule, until, until_handed};
use rustix::process::{Pid, Signal, WaitOptions, kill_process, waitpid};

use super::*;

fn process_of(id: u32) -> Result<Pid, Box<dyn Error>> {
  let raw = i32::try_from(id)?;
  let found = Pid::from_raw(raw).ok_or("the fixture did not give a process ID")?;

  Ok(found)
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
  let process = process_of(id)?;
  kill_process(process, Signal::KILL)?;

  Ok(())
}

#[test]
fn dropping_it_ends_the_child() -> Result<(), Box<dyn Error>> {
  let Ok(mut command) = holding();
  let running = alongside(&mut command)?;
  let Ok(id) = running.id();
  let process = process_of(id)?;

  assert_eq!(process_presence(process), Ok(Still::Running), "it never started");

  drop(running);

  assert_eq!(process_presence(process), Ok(Still::Ended), "the child outlived the thing holding it");

  Ok(())
}

#[test]
fn a_child_let_go_is_still_running_after_the_drop() -> Result<(), Box<dyn Error>> {
  let Ok(mut command) = holding();
  let running = let_go(&mut command)?;
  let Ok(id) = running.id();
  let process = process_of(id)?;

  drop(running);

  assert_eq!(process_presence(process), Ok(Still::Running), "letting go ended it anyway");

  killed(id)?;
  let _reaped = waitpid(Some(process), WaitOptions::empty())?;

  Ok(())
}

#[test]
fn the_lines_it_says_are_readable_while_it_is_held() -> Result<(), Box<dyn Error>> {
  let Ok(mut command) = Program::Sh.command();

  command
    .args(["-c", "printf 'one\\ntwo\\n'"])
    .stdout(Stdio::piped());

  let mut running = alongside(&mut command)?;
  let Ok(pipe) = running.take_stdout();
  let out = pipe.ok_or("no pipe from a piped command")?;
  let said = std::io::read_to_string(out.take(32))?;
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
  let exited = done.wait()?;
  let Ok(ended) = done.still();

  assert!(exited.success());
  assert_eq!(ended, Still::Ended);

  let Ok(mut command_2) = holding();
  let mut going = alongside(&mut command_2)?;
  let Ok(running) = going.still();

  assert_eq!(running, Still::Running);

  Ok(())
}

#[test]
fn a_child_let_go_that_ended_is_reaped_and_one_still_going_is_kept() -> Result<(), Box<dyn Error>>
{
  let Ok(mut command) = Program::True.command();

  command.stdout(Stdio::null());

  let done = let_go(&mut command)?;
  let Ok(ended) = done.id();

  let Ok(mut command_2) = holding();
  let going = let_go(&mut command_2)?;
  let Ok(held) = going.id();

  let mut started = vec![done, going];
  let Ok(patience) = Schedule::of(Duration::from_secs(2));
  let Ok(_over) = until_handed(patience, &mut started, |started_2| {
    let Ok(still) = reap(std::mem::take(started_2));

    *started_2 = still;

    Ok(match started_2.len() {
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
  let finished = process_of(ended)?;

  assert_eq!(kept, [held]);
  assert_eq!(process_presence(finished), Ok(Still::Ended), "{ended} is still in the process table");

  for mut one in started {
    let Ok(id) = one.id();
    killed(id)?;
    let _reaped = one.wait()?;
  }

  Ok(())
}

#[test]
fn a_parent_killed_outright_takes_the_child_with_it() -> Result<(), Box<dyn Error>> {
  let this_test = std::env::current_exe()?;
  let mut command = Command::new(this_test);

  command
    .args(["--nocapture", "the_child_this_test_starts"])
    .env(HOLDING, "yes")
    .stdout(Stdio::piped())
    .stderr(Stdio::null());

  let mut parent = let_go(&mut command)?;
  let Ok(stdout) = parent.take_stdout();
  let out = stdout.ok_or("no pipe from the parent")?;
  let mut heard = Vec::new();
  let mut found = None;

  for line in std::io::BufReader::new(out).lines() {
    let line_2 = line?;

    match line_2.trim().parse::<u32>() {
      Ok(id) => {
        found = Some(id);

        break;
      }
      Err(_not_a_number) => heard.push(line_2),
    }
  }

  let id = found.ok_or_else(|| format!("the parent never said what it started: {heard:?}"))?;
  let Ok(whose) = parent.id();

  killed(whose)?;
  let _reaped = parent.wait()?;

  let process = process_of(id)?;
  let Ok(patience) = Schedule::of(Duration::from_secs(10));
  let Ok(gone) = until(patience, || {
    let Ok(state) = process_presence(process);

    Ok(match state {
      Still::Running => Ready::NotYet,
      Still::Ended => Ready::Yes,
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
#[cfg_attr(
  dylint_lib = "explicit041_no_unsaid_printing",
  allow(
    explicit041_no_unsaid_printing,
    reason = "the child tells whoever started it which process to watch by printing the id, and a test binary is not a program written to the contract"
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
