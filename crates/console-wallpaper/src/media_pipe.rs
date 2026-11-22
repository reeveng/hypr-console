//! A renderer reads stdout while draining stderr from the same child.
//! The idle limit bounds pipe backpressure without capping active renders.

use std::io::{self, ErrorKind};
use std::os::fd::OwnedFd;
use std::time::{Duration, Instant};

use console_core_iteration::{Step, iterate};
use console_core_number_conversion::index;
use console_operating_system_linux::program_lifetime::{
  BoundToParent, PipeReady, TimedPipe, TimedReadPipe,
};

const WAIT: Duration = Duration::from_secs(30);
const DIAGNOSTIC_BYTES: u32 = 16_384;

pub(crate) struct Output {
  stdout: Option<TimedReadPipe>,
  stderr: Option<TimedReadPipe>,
  diagnostics: Vec<u8>,
}

impl Output {
  pub(crate) fn take(child: &mut BoundToParent) -> Result<Self, crate::Unpainted> {
    let Ok(stdout) = child.take_stdout();
    let Ok(stderr) = child.take_stderr();
    let out = stdout.ok_or(crate::Unpainted::NoPipeOut)?;
    let errors = stderr.ok_or(crate::Unpainted::NoPipeOut)?;
    let stdout_pipe =
      TimedReadPipe::new(OwnedFd::from(out)).map_err(crate::Unpainted::Stopped)?;
    let stderr_pipe =
      TimedReadPipe::new(OwnedFd::from(errors)).map_err(crate::Unpainted::Stopped)?;

    Ok(Self {
      stdout: Some(stdout_pipe),
      stderr: Some(stderr_pipe),
      diagnostics: Vec::new(),
    })
  }

  pub(crate) fn details(&self) -> Result<String, console_core_never::Never> {
    crate::diagnostic(&self.diagnostics)
  }

  #[cfg_attr(
    all(not(test), dylint_lib = "explicit039_no_reading_the_clock"),
    expect(
      explicit039_no_reading_the_clock,
      reason = "this pipe boundary measures the inactivity time while waiting for the child"
    )
  )]
  pub(crate) fn read(&mut self, target: &mut [u8]) -> io::Result<u32> {
    let began = Instant::now();
    let reading = iterate((self, target), |(output, bytes)| {
      let remaining = WAIT.saturating_sub(began.elapsed());

      Ok(match remaining.is_zero().then_some(()) {
        Some(()) => Step::Halt(Err(io::Error::from(ErrorKind::TimedOut))),
        None => match output.next(bytes, remaining) {
          Ok(Some(count)) => Step::Halt(Ok(count)),
          Ok(None) => Step::Again((output, bytes)),
          Err(fault) => match (fault.kind() == ErrorKind::Interrupted).then_some(()) {
            Some(()) => Step::Again((output, bytes)),
            None => Step::Halt(Err(fault)),
          },
        },
      })
    });

    match reading {
      Ok(read) => read,
      Err(endless) => Err(io::Error::other(endless)),
    }
  }

  fn next(&mut self, target: &mut [u8], wait: Duration) -> io::Result<Option<u32>> {
    let [stdout_ready, stderr_ready] = match (&self.stdout, &self.stderr) {
      (Some(stdout), Some(stderr)) => stdout.ready_with(stderr, wait)?,
      (Some(_), None) => [PipeReady::Readable, PipeReady::Quiet],
      (None, Some(_)) => [PipeReady::Quiet, PipeReady::Readable],
      (None, None) => return Ok(Some(0)),
    };

    match stderr_ready {
      PipeReady::Readable => self.drain(wait)?,
      PipeReady::Quiet => {}
    }

    match stdout_ready {
      PipeReady::Readable => {
        let count = read_from(&mut self.stdout, target, wait)?;
        Ok((count != 0).then_some(count))
      }
      PipeReady::Quiet => Ok(None),
    }
  }

  fn drain(&mut self, wait: Duration) -> io::Result<()> {
    let mut buffer = [0; 4096];
    let count = read_from(&mut self.stderr, &mut buffer, wait)?;
    let Ok(limit) = index(DIAGNOSTIC_BYTES);
    let Ok(length) = index(count);

    match buffer.get(..length.min(limit.saturating_sub(self.diagnostics.len()))) {
      Some(bytes) => self.diagnostics.extend_from_slice(bytes),
      None => {}
    }

    Ok(())
  }
}

fn read_from(
  pipe: &mut Option<TimedReadPipe>,
  target: &mut [u8],
  wait: Duration,
) -> io::Result<u32> {
  let outlet = pipe
    .as_ref()
    .ok_or(io::Error::from(ErrorKind::BrokenPipe))?;
  let count = outlet.read_some(target, wait)?;

  match count {
    0 => *pipe = None,
    _ => {}
  }

  Ok(count)
}

pub(crate) enum FrameRead {
  Whole,
  Eof,
}

pub(crate) fn read_frame(output: &mut Output, frame: &mut [u8]) -> io::Result<FrameRead> {
  let state = iterate((output, frame), |(out, remaining)| {
    let count = match out.read(remaining) {
      Ok(0) => return Ok(Step::Halt(Ok(FrameRead::Eof))),
      Ok(count) => count,
      Err(fault) => return Ok(Step::Halt(Err(fault))),
    };
    let Ok(written) = index(count);

    Ok(match remaining.get_mut(written..) {
      Some([]) => Step::Halt(Ok(FrameRead::Whole)),
      Some(next) => Step::Again((out, next)),
      None => Step::Halt(Err(io::Error::from(ErrorKind::InvalidData))),
    })
  });

  match state {
    Ok(read) => read,
    Err(endless) => Err(io::Error::other(endless)),
  }
}

pub(crate) fn write_frame(input: &TimedPipe, frame: &[u8]) -> io::Result<()> {
  let written = iterate(frame, |remaining| {
    let count = match input.write_some(remaining, WAIT) {
      Ok(0) => return Ok(Step::Halt(Err(io::Error::from(ErrorKind::WriteZero)))),
      Ok(count) => count,
      Err(fault) => return Ok(Step::Halt(Err(fault))),
    };
    let Ok(sent) = index(count);

    Ok(match remaining.get(sent..) {
      Some([]) => Step::Halt(Ok(())),
      Some(rest) => Step::Again(rest),
      None => Step::Halt(Err(io::Error::from(ErrorKind::InvalidData))),
    })
  });

  match written {
    Ok(done) => done,
    Err(endless) => Err(io::Error::other(endless)),
  }
}

#[cfg(test)]
mod tests {
  use std::process::Stdio;

  use console_core_external_programs::Program;
  use console_operating_system_linux::program_lifetime::alongside;

  use super::*;

  #[test]
  fn a_child_can_fill_its_diagnostic_pipe_before_sending_the_frame()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(mut command) = Program::Sh.command();
    command
      .args(["-c", "head -c 131072 /dev/zero >&2; printf RGB"])
      .stdout(Stdio::piped())
      .stderr(Stdio::piped());
    let mut child = alongside(&mut command)?;
    let mut output = Output::take(&mut child)?;
    let mut frame = [0; 3];

    let complete = read_frame(&mut output, &mut frame)?;
    assert!(matches!(complete, FrameRead::Whole));
    assert_eq!(&frame, b"RGB");

    let ended = read_frame(&mut output, &mut frame)?;
    assert!(matches!(ended, FrameRead::Eof));
    assert_eq!(
      u32::try_from(output.diagnostics.len()),
      Ok(DIAGNOSTIC_BYTES)
    );
    let status = child.wait()?;
    assert!(status.success());

    Ok(())
  }
}
