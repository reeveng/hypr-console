//! The Linux command contract, including a child that cannot exit until its
//! progress callback opens a FIFO, and output larger than a pipe can hold.

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use console_core_never::Never;
use console_operating_system_linux::program_runtime::commands::{Invocation, Linux, Timing};
use console_program_contract::ExitStatus;
use console_program_runtime::commands::{Launch, Platform, Reporter, Streams};
use console_waiting::{Outcome, Ready, Schedule, until};

type Failure = Box<dyn std::error::Error>;

#[cfg_attr(
  dylint_lib = "explicit082_no_built_shell_line",
  expect(
    explicit082_no_built_shell_line,
    reason = "the integration fixtures run explicitly chosen shell programs and pass the inputs as separate arguments"
  )
)]
fn shell(script: &str, arguments: &[String]) -> Result<Invocation, Never> {
  Ok(Invocation {
    program: PathBuf::from("/bin/sh"),
    arguments: [
      "-c".to_string(),
      script.to_string(),
      "command-test".to_string(),
    ]
    .into_iter()
    .chain(arguments.iter().cloned())
    .collect(),
  })
}

#[test]
fn a_failed_command_keeps_both_outputs_and_literal_arguments() -> Result<(), Failure> {
  let mut platform = Linux {
    timing: Timing::Background,
  };
  let literal = "two words; $(not a command)".to_string();
  let Ok(invocation) = shell(
    "printf '%s\\n' \"$1\"; printf 'reason\\n' >&2; exit 7",
    std::slice::from_ref(&literal),
  );
  let completion = platform.run(&invocation, Streams::COLLECT, None)?;

  assert_eq!(completion.output, format!("{literal}\n"));
  assert_eq!(completion.error_output, "reason\n");
  assert_eq!(completion.status, ExitStatus::Failure(Some(7)));

  Ok(())
}

struct Progress {
  gate: PathBuf,
  opened: io::Result<()>,
  lines: Vec<String>,
}

impl Reporter for Progress {
  #[cfg_attr(
    dylint_lib = "explicit040_no_torn_write",
    allow(
      explicit040_no_torn_write,
      reason = "the gate is a FIFO: the child waits for bytes on that pipe and an atomic replacement would leave its reader blocked"
    )
  )]
  fn line(&mut self, line: &str) -> Result<(), Never> {
    match ::core::primitive::bool::then_some(line == "first\n", ()) {
      ::core::option::Option::Some(()) => self.opened = std::fs::write(&self.gate, b"continue\n"),
      ::core::option::Option::None => {}
    }

    self.lines.push(line.to_string());

    Ok(())
  }
}

#[test]
fn live_output_arrives_before_exit_and_stderr_is_drained_concurrently() -> Result<(), Failure> {
  let directory = console_core_temporary_directories::fresh("command-progress")?;
  let gate = directory.join("gate");
  let gate_text = gate.to_str().ok_or("the fixture path is not UTF-8")?;
  let mut platform = Linux {
    timing: Timing::Background,
  };
  let Ok(invocation) = shell(
    "mkfifo \"$1\"; printf 'first\\n'; IFS= read -r reply < \"$1\"; head -c 1048576 /dev/zero >&2; printf 'last'",
    &[gate_text.to_string()],
  );
  let mut progress = Progress {
    gate,
    opened: Ok(()),
    lines: Vec::new(),
  };
  let completion = platform.run(&invocation, Streams::COLLECT, Some(&mut progress))?;
  progress.opened?;

  assert_eq!(progress.lines, vec!["first\n", "last"]);
  assert_eq!(completion.output, "first\nlast");
  assert_eq!(completion.error_output.len(), 1_048_576);
  assert_eq!(completion.status, ExitStatus::Success);
  std::fs::remove_dir_all(directory)?;

  Ok(())
}

#[test]
fn a_background_launch_is_reaped_without_another_launch() -> Result<(), Failure> {
  let directory = console_core_temporary_directories::fresh("command-reaping")?;
  let pid_file = directory.join("pid");
  let pid_path = pid_file.to_str().ok_or("the fixture path is not UTF-8")?;
  let Ok(invocation) = shell("printf '%s' $$ > \"$1\"", &[pid_path.to_string()]);
  let mut platform = Linux {
    timing: Timing::Background,
  };

  platform.spawn(&invocation, Streams::QUIET, Launch::Background)?;
  let schedule = Schedule::of(Duration::from_secs(5))?;
  let gone = until(schedule, || {
    let pid = match std::fs::read_to_string(&pid_file) {
      Ok(pid) => match pid.parse::<i32>() {
        Ok(pid_2) => rustix::process::Pid::from_raw(pid_2),
        Err(_not_fully_written) => None,
      },
      Err(_not_written_yet) => None,
    };

    Ok(match pid {
      Some(process) => match rustix::process::test_kill_process(process) {
        Err(rustix::io::Errno::SRCH) => Ready::Yes,
        Ok(()) => Ready::NotYet,
        Err(fault) => {
          console_core_logging::trace!("waiting for the child to be reaped: {fault}");

          Ready::NotYet
        }
      },
      None => Ready::NotYet,
    })
  });

  assert_eq!(gone, Ok(Outcome::Happened));
  std::fs::remove_dir_all(directory)?;

  Ok(())
}

#[test]
fn capture_finishes_while_a_descendant_still_holds_an_output_pipe() -> Result<(), Failure> {
  use rustix::event::{PollFd, PollFlags, Timespec, poll};
  use rustix::process::{Pid, PidfdFlags, pidfd_open};

  let Ok(invocation) = shell("sleep 3 & printf '%s' $!", &[]);
  let mut platform = Linux {
    timing: Timing::Background,
  };
  let completion = platform.run(&invocation, Streams::COLLECT, None)?;
  let raw = completion.output.parse::<i32>()?;
  let descendant = Pid::from_raw(raw).ok_or("the shell did not report a child process")?;
  let descriptor = pidfd_open(descendant, PidfdFlags::empty())?;
  let mut watched = [PollFd::new(&descriptor, PollFlags::IN)];
  let now = Timespec {
    tv_sec: 0,
    tv_nsec: 0,
  };
  let exited = poll(&mut watched, Some(&now)).map(|count| {
    let Ok(measured) = console_core_number_conversion::fitted::<_, u32>(count);

    measured
  })?;

  assert_eq!(completion.status, ExitStatus::Success);
  assert_eq!(exited, 0, "the output waited for the descendant to exit");

  Ok(())
}

#[test]
fn the_text_contract_reports_non_utf8_output() -> Result<(), Failure> {
  let Ok(invocation) = shell("printf '\\377'", &[]);
  let mut platform = Linux {
    timing: Timing::Background,
  };

  match platform.run(&invocation, Streams::COLLECT, None) {
    Ok(output) => return Err(Box::from(format!("accepted invalid UTF-8: {output:?}"))),
    Err(fault) => assert_eq!(fault.kind(), io::ErrorKind::InvalidData),
  }

  Ok(())
}

#[test]
fn missing_programs_and_undrained_detached_pipes_are_errors() {
  let mut platform = Linux {
    timing: Timing::Background,
  };
  let invocation = Invocation {
    program: PathBuf::from("/no-such-command/program"),
    arguments: Vec::new(),
  };

  assert_eq!(
    platform
      .run(&invocation, Streams::COLLECT, None)
      .map_err(|error| error.kind()),
    Err(io::ErrorKind::NotFound)
  );
  assert_eq!(
    platform
      .spawn(&invocation, Streams::COLLECT, Launch::Background)
      .map_err(|error| error.kind()),
    Err(io::ErrorKind::InvalidInput)
  );
}
