//! Linux process execution, including the desktop's launch and timing policy.
//!
//! systemd scopes are used when available. Otherwise the Linux lifetime adapter
//! starts a new session. Every detached child is reaped as soon as it exits.
//! Captured runs close standard input and drain both output pipes without blocking
//! either one. A read failure still closes both pipes and reaps the child.
//!
//! A fork is a captured run somebody may want to stop. [`Linux::fork`] starts it
//! bound to whoever started it and in a process group of its own, and hands it
//! back before anything is read, so its [`ProcessIdentity`] can be kept where an
//! interrupt is decided. [`finished`] then reads both streams while it waits
//! rather than before: something the fork started and left holding the pipe
//! would otherwise hold back the answer, and waiting ends the group the moment
//! the fork itself has ended.
//!
//! [`ProcessIdentity`]: crate::program_lifetime::ProcessIdentity

use std::io;
use std::os::fd::OwnedFd;
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::time::Duration;

use crate::program_lifetime::{BoundToParent, Forked, PipeReady, TimedReadPipe, alongside, forked, hooks};
use crate::systemd::{Wrapped, command_in_a_scope_of_its_own};
use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_program_contract::{Executable, ExitStatus};
use console_program_lifetime::{Detached, let_go, threads};
use console_program_runtime::commands::{Completion, Launch, Platform, Report, Stream, Streams};

pub type Invocation = console_program_runtime::commands::Invocation<PathBuf>;

pub fn invocation(words: &[String]) -> Result<Option<Invocation>, Never> {
  Ok(words.split_first().map(|(program, arguments)| Invocation {
    program: PathBuf::from(program),
    arguments: arguments.to_vec(),
  }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
  Pressed,
  Background,
  Inherited,
}

pub struct Linux {
  pub timing: Timing,
}

impl Linux {
  pub fn alongside(
    &self,
    invocation: &Invocation,
    streams: Streams,
  ) -> io::Result<BoundToParent> {
    let Ok(mut command) = self.prepare(invocation, streams);

    alongside(&mut command)
  }

  pub fn fork(&self, invocation: &Invocation, streams: Streams) -> io::Result<Forked> {
    let Ok(mut command) = self.prepare(invocation, streams);

    command.stdin(Stdio::null());

    forked(&mut command)
  }

  fn prepare(&self, invocation: &Invocation, streams: Streams) -> Result<Command, Never> {
    let mut command = Command::new(&invocation.program);
    command.args(&invocation.arguments);
    let Ok(()) = self.configure(&mut command, streams);

    Ok(command)
  }

  fn configure(&self, command: &mut Command, streams: Streams) -> Result<(), Never> {
    let (Ok(stdout), Ok(stderr)) = (stdio(streams.stdout), stdio(streams.stderr));
    command.stdout(stdout).stderr(stderr);

    match self.timing {
      Timing::Pressed => console_response_times::pressed_here(command),
      Timing::Background => console_response_times::not_a_press(command),
      Timing::Inherited => Ok(()),
    }
  }
}

impl Platform for Linux {
  type Executable = PathBuf;
  type Error = io::Error;

  fn resolve(&self, executable: Executable) -> io::Result<PathBuf> {
    Ok(match executable {
      Executable::External(program) => {
        let Ok(name) = program.name();

        PathBuf::from(name)
      }
      Executable::Internal(program) => {
        let Ok(at) = program.at();

        at
      }
    })
  }

  fn run(
    &mut self,
    invocation: &Invocation,
    streams: Streams,
    report: Option<Report<'_>>,
  ) -> io::Result<Completion> {
    let Ok(mut command) = self.prepare(invocation, streams);

    match streams.stdout {
      Stream::Capture => {
        command.stdin(Stdio::null());
      }
      Stream::Inherit | Stream::Discard => {}
    }

    let mut started = command.spawn()?;
    let printing = started.stdout.take();
    let complaints = started.stderr.take();
    let captured = collect(&mut started, printing, complaints, report, Child::try_wait);
    let completed = started.wait();
    let captured_streams = captured?;
    let status = completed?;

    completion(captured_streams, status)
  }

  fn spawn(
    &mut self,
    invocation: &Invocation,
    streams: Streams,
    launch: Launch,
  ) -> io::Result<()> {
    match ::core::primitive::bool::then_some(streams.stdout == Stream::Capture || streams.stderr == Stream::Capture, ()) {
      ::core::option::Option::Some(()) => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidInput,
          "a detached launch cannot capture output",
        ));
      }
      ::core::option::Option::None => {}
    }

    let mut command = match launch {
      Launch::Background => {
        let Ok(command) = self.prepare(invocation, streams);

        command
      }
      Launch::Application => {
        let Ok((wrapped, mut command)) = command_in_a_scope_of_its_own(
          None,
          invocation.program.as_os_str(),
          &invocation.arguments,
        );

        match wrapped {
          Wrapped::InAScope => {}
          Wrapped::AsItWasHandedIn => {
            let Ok(()) = hooks::detach(&mut command);
          }
        }

        let Ok(()) = self.configure(&mut command, streams);

        command
      }
    };

    let started = let_go(&mut command)?;

    let Ok(()) = reap_in_background(started);

    Ok(())
  }
}

pub fn finished(mut started: Forked, report: Option<Report<'_>>) -> io::Result<Completion> {
  let Ok(printing) = started.take_stdout();
  let Ok(complaints) = started.take_stderr();
  let captured = collect(&mut started, printing, complaints, report, Forked::try_wait);
  let completed = started.wait();
  let streams = captured?;
  let status = completed?;

  completion(streams, status)
}

const PIPE_POLL: Duration = Duration::from_millis(50);
const MAX_STDERR_BYTES: u64 = 16 * 1024 * 1024;
const REMAINING_PASSES: u32 = 1024;

struct Capture {
  printing: Option<TimedReadPipe>,
  complaints: Option<TimedReadPipe>,
  stdout: Vec<u8>,
  stderr: Vec<u8>,
  report_line: Vec<u8>,
  stderr_size: u64,
}

fn completion(capture: Capture, status: std::process::ExitStatus) -> io::Result<Completion> {
  let output = String::from_utf8(capture.stdout)
    .map_err(|fault| io::Error::new(io::ErrorKind::InvalidData, format!("stdout: {fault}")))?;
  let error_output = String::from_utf8(capture.stderr)
    .map_err(|fault| io::Error::new(io::ErrorKind::InvalidData, format!("stderr: {fault}")))?;

  Ok(Completion {
    output,
    error_output,
    status: match status.success().then_some(()) {
      Some(()) => ExitStatus::Success,
      None => ExitStatus::Failure(status.code()),
    },
  })
}

type CheckExit<P> = fn(&mut P) -> io::Result<Option<std::process::ExitStatus>>;

struct Reading<'p, 'r, P> {
  child: &'p mut P,
  capture: Capture,
  report: Option<Report<'r>>,
  remaining: Option<u32>,
}

fn collect<P>(
  child: &mut P,
  printing: Option<ChildStdout>,
  complaints: Option<ChildStderr>,
  report: Option<Report<'_>>,
  try_wait: CheckExit<P>,
) -> io::Result<Capture> {
  let output = printing.map(|pipe| TimedReadPipe::new(OwnedFd::from(pipe))).transpose()?;
  let errors = complaints.map(|pipe| TimedReadPipe::new(OwnedFd::from(pipe))).transpose()?;
  let capture = Capture {
    printing: output,
    complaints: errors,
    stdout: Vec::new(),
    stderr: Vec::new(),
    report_line: Vec::new(),
    stderr_size: 0,
  };
  let reading = Reading { child, capture, report, remaining: None };
  let drained = iterate(reading, |mut current| {
    Ok(match current.pass(try_wait) {
      Ok(Drain::More) => Step::Again(current),
      Ok(Drain::Done) => Step::Halt(current.capture.finish_report(current.report)),
      Err(fault) => Step::Halt(Err(fault)),
    })
  });

  match drained {
    Ok(captured) => captured,
    Err(fault) => Err(io::Error::other(fault)),
  }
}

enum Drain {
  More,
  Done,
}

impl<P> Reading<'_, '_, P> {
  fn pass(&mut self, try_wait: CheckExit<P>) -> io::Result<Drain> {
    match (self.capture.printing.as_ref(), self.capture.complaints.as_ref()) {
      (None, None) => return Ok(Drain::Done),
      (Some(_), _) | (None, Some(_)) => {}
    }

    let remaining = match self.remaining {
      Some(passes) => Some(passes),
      None => {
        let stopped = try_wait(self.child)?;

        stopped.map(|_status| REMAINING_PASSES)
      }
    };
    let wait = match remaining {
      Some(0) => return Ok(Drain::Done),
      Some(passes) => {
        self.remaining = passes.checked_sub(1);

        Duration::ZERO
      }
      None => PIPE_POLL,
    };
    let progress = self.capture.drain(&mut self.report, wait)?;

    Ok(match (self.remaining, progress) {
      (Some(_), Progress::Quiet) => Drain::Done,
      (Some(_), Progress::Read) | (None, _) => Drain::More,
    })
  }
}

enum Progress {
  Quiet,
  Read,
}

struct ReadWait {
  stdout: Option<Duration>,
  stderr: Option<Duration>,
}

impl Capture {
  fn available(&self, wait: Duration) -> io::Result<ReadWait> {
    let ready = match (&self.printing, &self.complaints) {
      (Some(output), Some(errors)) => output.ready_with(errors, wait),
      (Some(_), None) => return Ok(ReadWait { stdout: Some(wait), stderr: None }),
      (None, Some(_)) => return Ok(ReadWait { stdout: None, stderr: Some(wait) }),
      (None, None) => return Ok(ReadWait { stdout: None, stderr: None }),
    };
    let [stdout, stderr] = match ready {
      Ok(readable) => readable,
      Err(fault) => match pipe_fault(&fault) {
        Ok(PipeFault::NotReady) => return Ok(ReadWait { stdout: None, stderr: None }),
        Ok(PipeFault::Failed) => return Err(fault),
      },
    };
    let ready_now = |readable| match readable {
      PipeReady::Readable => Some(Duration::ZERO),
      PipeReady::Quiet => None,
    };

    Ok(ReadWait {
      stdout: ready_now(stdout),
      stderr: ready_now(stderr),
    })
  }

  fn drain(&mut self, report: &mut Option<Report<'_>>, wait: Duration) -> io::Result<Progress> {
    let available = self.available(wait)?;
    let mut buffer = [0u8; 8192];
    let mut progress = Progress::Quiet;

    match available.stdout {
      Some(ready) => {
        let pending = read_pipe(&mut self.printing, &mut buffer, ready)?;

        match pending {
          Some(bytes) => {
            self.stdout.extend_from_slice(bytes);
            self.report_output(bytes, report)?;
            progress = Progress::Read;
          }
          None => {}
        }
      }
      None => {}
    }

    match available.stderr {
      Some(ready) => {
        let pending = read_pipe(&mut self.complaints, &mut buffer, ready)?;

        match pending {
          Some(bytes) => {
            let Ok(()) = self.record_stderr(bytes);
            progress = Progress::Read;
          }
          None => {}
        }
      }
      None => {}
    }

    Ok(progress)
  }

  fn record_stderr(&mut self, bytes: &[u8]) -> Result<(), Never> {
    let Ok(length) = console_core_number_conversion::fitted::<_, u64>(bytes.len());
    self.stderr_size = self.stderr_size.saturating_add(length);

    match (self.stderr_size <= MAX_STDERR_BYTES).then_some(()) {
      Some(()) => self.stderr.extend_from_slice(bytes),
      None => {}
    }

    Ok(())
  }

  fn report_output(&mut self, bytes: &[u8], report: &mut Option<Report<'_>>) -> io::Result<()> {
    let reporter = match report.as_deref_mut() {
      Some(reporter) => reporter,
      None => return Ok(()),
    };

    for part in bytes.split_inclusive(|byte| *byte == b'\n') {
      self.report_line.extend_from_slice(part);

      match part.last() {
        Some(b'\n') => {
          let line = std::str::from_utf8(&self.report_line).map_err(io::Error::other)?;
          let Ok(()) = reporter.line(line);

          self.report_line.clear();
        }
        Some(_) | None => {}
      }
    }

    Ok(())
  }

  fn finish_report(self, report: Option<Report<'_>>) -> io::Result<Self> {
    match (report, self.report_line.as_slice()) {
      (Some(reporter), [_, ..]) => {
        let line = std::str::from_utf8(&self.report_line).map_err(io::Error::other)?;
        let Ok(()) = reporter.line(line);
      }
      (None, _) | (Some(_), []) => {}
    }

    match (self.stderr_size > MAX_STDERR_BYTES).then_some(()) {
      Some(()) => Err(io::Error::new(io::ErrorKind::InvalidData, "the program wrote more than 16 MiB to stderr")),
      None => Ok(self),
    }
  }
}

enum PipeFault {
  NotReady,
  Failed,
}

fn pipe_fault(fault: &io::Error) -> Result<PipeFault, Never> {
  Ok(match matches!(fault.kind(), io::ErrorKind::TimedOut | io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock).then_some(()) {
    Some(()) => PipeFault::NotReady,
    None => PipeFault::Failed,
  })
}

fn read_pipe<'a>(
  pipe: &mut Option<TimedReadPipe>,
  buffer: &'a mut [u8],
  wait: Duration,
) -> io::Result<Option<&'a [u8]>> {
  let source = match pipe.as_ref() {
    Some(ready) => ready,
    None => return Ok(None),
  };
  let read = match source.read_some(buffer, wait) {
    Ok(length) => length,
    Err(fault) => {
      let Ok(outcome) = pipe_fault(&fault);

      return match outcome {
        PipeFault::NotReady => Ok(None),
        PipeFault::Failed => Err(fault),
      };
    }
  };

  match read {
    0 => {
      *pipe = None;

      Ok(None)
    }
    _ => {
      let Ok(length) = console_core_number_conversion::index(read);

      buffer.get(..length).map(Some).ok_or_else(|| io::Error::other("invalid pipe read length"))
    }
  }
}

pub fn reap_in_background(mut started: Detached) -> Result<(), Never> {
  let waiting = std::thread::spawn(move || {
    match started.wait() {
      Ok(_) => {}
      Err(fault) => console_core_logging::warn!("could not reap a detached child: {fault}"),
    }
  });

  threads::let_go(waiting)
}

fn stdio(stream: Stream) -> Result<Stdio, Never> {
  Ok(match stream {
    Stream::Capture => Stdio::piped(),
    Stream::Inherit => Stdio::inherit(),
    Stream::Discard => Stdio::null(),
  })
}
