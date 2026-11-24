//! The command contract can run against a platform with no host processes.

use console_core_never::Never;
use console_program_contract::{Command, Executable, ExitStatus};
use console_program_runtime::commands::{
  self, Completion, Launch, Platform, Report, Reporter, Streams,
};

type Invocation = commands::Invocation<u64>;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Operation {
  Run(Invocation, Streams),
  Spawn(Invocation, Streams, Launch),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlatformError {
  Resolution,
  Execution,
}

impl std::fmt::Display for PlatformError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(to, "{self:?}")
  }
}

impl std::error::Error for PlatformError {}

struct OtherOs {
  executable: u64,
  failure: Option<PlatformError>,
  operations: Vec<Operation>,
}

impl Platform for OtherOs {
  type Executable = u64;
  type Error = PlatformError;

  fn resolve(&mut self, _executable: Executable) -> Result<u64, PlatformError> {
    match self.failure {
      Some(PlatformError::Resolution) => Err(PlatformError::Resolution),
      None | Some(PlatformError::Execution) => Ok(self.executable),
    }
  }

  fn run(
    &mut self,
    invocation: &Invocation,
    streams: Streams,
    report: Option<Report<'_>>,
  ) -> Result<Completion, PlatformError> {
    self
      .operations
      .push(Operation::Run(invocation.clone(), streams));

    match self.failure {
      Some(PlatformError::Execution) => return Err(PlatformError::Execution),
      None | Some(PlatformError::Resolution) => {}
    }

    match report {
      Some(report_2) => {
        let Ok(()) = report_2.line("pairing code\n");
      }
      None => {}
    }

    Ok(Completion {
      output: "pairing code\n".to_string(),
      error_output: "refused\n".to_string(),
      status: ExitStatus::Failure(Some(7)),
    })
  }

  fn spawn(
    &mut self,
    invocation: &Invocation,
    streams: Streams,
    launch: Launch,
  ) -> Result<(), PlatformError> {
    self
      .operations
      .push(Operation::Spawn(invocation.clone(), streams, launch));

    Ok(())
  }
}

struct Recording(Vec<String>);

impl Reporter for Recording {
  fn line(&mut self, line: &str) -> Result<(), Never> {
    self.0.push(line.to_string());

    Ok(())
  }
}

fn platform() -> Result<OtherOs, Never> {
  Ok(OtherOs {
    executable: 42,
    failure: None,
    operations: Vec::new(),
  })
}

fn command() -> Result<Command, Never> {
  use console_program_contract::Executable;

  let command = Command {
    program: Executable::Internal(console_core_internal_programs::InternalProgram::Files),
    arguments: vec![
      "two words".to_string(),
      "$(untouched); &".to_string(),
      String::new(),
    ],
  };

  Ok(command)
}

#[test]
fn captured_attached_and_detached_execution_use_the_same_resolver_and_literal_arguments()
-> Result<(), PlatformError> {
  let Ok(mut platform) = platform();
  let Ok(command) = command();
  let invocation = Invocation {
    program: platform.executable,
    arguments: command.arguments.clone(),
  };

  let captured = commands::run(&mut platform, &command, Streams::COLLECT)?;
  let attached = commands::run(&mut platform, &command, Streams::INHERIT)?;
  commands::spawn(&mut platform, &command, Streams::QUIET, Launch::Application)?;

  assert_eq!(captured, attached);
  assert_eq!(captured.command, command);
  assert_eq!(captured.error_output, "refused\n");
  assert_eq!(captured.status, ExitStatus::Failure(Some(7)));
  assert_eq!(
    platform.operations,
    vec![
      Operation::Run(invocation.clone(), Streams::COLLECT),
      Operation::Run(invocation.clone(), Streams::INHERIT),
      Operation::Spawn(invocation, Streams::QUIET, Launch::Application),
    ]
  );

  Ok(())
}

#[test]
fn progress_and_final_output_are_both_delivered() -> Result<(), PlatformError> {
  let Ok(mut platform) = platform();
  let Ok(command) = command();
  let mut recording = Recording(Vec::new());
  let answer = commands::run_reporting(
    &mut platform,
    &command,
    Streams::COLLECT,
    Some(&mut recording),
  )?;

  assert_eq!(recording.0, vec!["pairing code\n"]);
  assert_eq!(answer.output, "pairing code\n");

  Ok(())
}

#[test]
fn failures_remain_failures_and_an_unresolved_program_is_never_started() {
  let Ok(mut platform) = platform();
  let Ok(command) = command();
  platform.failure = Some(PlatformError::Resolution);

  assert_eq!(
    commands::run(&mut platform, &command, Streams::COLLECT),
    Err(PlatformError::Resolution)
  );
  assert_eq!(
    commands::spawn(&mut platform, &command, Streams::QUIET, Launch::Application),
    Err(PlatformError::Resolution)
  );
  assert!(platform.operations.is_empty());

  platform.failure = Some(PlatformError::Execution);

  assert_eq!(
    commands::run(&mut platform, &command, Streams::COLLECT),
    Err(PlatformError::Execution)
  );
}
