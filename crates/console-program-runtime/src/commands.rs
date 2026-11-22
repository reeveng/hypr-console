//! Command execution with resolution and process operations supplied by a platform.
//!
//! Arguments remain separate values. This module neither looks up a program nor
//! opens a process, assumes a shell, chooses a service, or reads the environment.
//! The same resolver is used for a captured run, an attached run and a launch.
//! Its result is the platform's own executable type, including a native handle.
//! An application launch requests an independent lifetime; a background helper
//! requests no separate application session. The platform arranges child reaping
//! before reporting a successful launch, without needing a later command.

use console_core_never::Never;
use console_program_contract::{Answer, Command, Executable, ExitStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation<Executable> {
  pub program: Executable,
  pub arguments: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
  Capture,
  Inherit,
  Discard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Streams {
  pub stdout: Stream,
  pub stderr: Stream,
}

impl Streams {
  pub const CAPTURE: Self = Self {
    stdout: Stream::Capture,
    stderr: Stream::Inherit,
  };
  pub const COLLECT: Self = Self {
    stdout: Stream::Capture,
    stderr: Stream::Capture,
  };
  pub const INHERIT: Self = Self {
    stdout: Stream::Inherit,
    stderr: Stream::Inherit,
  };
  pub const QUIET: Self = Self {
    stdout: Stream::Discard,
    stderr: Stream::Discard,
  };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
  pub output: String,
  pub error_output: String,
  pub status: ExitStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Launch {
  Application,
  Background,
}

pub trait Reporter {
  fn line(&mut self, line: &str) -> Result<(), Never>;
}

pub type Report<'a> = &'a mut dyn Reporter;

pub trait Platform {
  type Executable;
  type Error;

  fn resolve(&self, executable: Executable) -> Result<Self::Executable, Self::Error>;

  fn run(
    &mut self,
    invocation: &Invocation<Self::Executable>,
    streams: Streams,
    report: Option<Report<'_>>,
  ) -> Result<Completion, Self::Error>;

  fn spawn(
    &mut self,
    invocation: &Invocation<Self::Executable>,
    streams: Streams,
    launch: Launch,
  ) -> Result<(), Self::Error>;
}

pub fn resolve<P: Platform>(
  platform: &P,
  command: &Command,
) -> Result<Invocation<P::Executable>, P::Error> {
  let program = platform.resolve(command.program)?;

  Ok(Invocation {
    program,
    arguments: command.arguments.clone(),
  })
}

pub fn run<P: Platform>(
  platform: &mut P,
  command: &Command,
  streams: Streams,
) -> Result<Answer, P::Error> {
  run_reporting(platform, command, streams, None)
}

pub fn run_reporting<P: Platform>(
  platform: &mut P,
  command: &Command,
  streams: Streams,
  report: Option<Report<'_>>,
) -> Result<Answer, P::Error> {
  let invocation = resolve(platform, command)?;
  let completion = platform.run(&invocation, streams, report)?;

  Ok(Answer {
    job: None,
    command: command.clone(),
    output: completion.output,
    error_output: completion.error_output,
    status: completion.status,
  })
}

pub fn spawn<P: Platform>(
  platform: &mut P,
  command: &Command,
  streams: Streams,
  launch: Launch,
) -> Result<(), P::Error> {
  let invocation = resolve(platform, command)?;

  platform.spawn(&invocation, streams, launch)
}
