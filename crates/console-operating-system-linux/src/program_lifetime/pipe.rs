//! Child pipes that give control back when the other end stops responding.

use std::io;
use std::os::fd::OwnedFd;
use std::time::Duration;

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::io::{Errno, read, write};

pub struct TimedPipe(OwnedFd);

impl TimedPipe {
  pub fn new(writer: OwnedFd) -> io::Result<Self> {
    nonblocking(&writer)?;

    Ok(Self(writer))
  }

  pub fn write_some(&self, bytes: &[u8], wait: Duration) -> io::Result<u32> {
    let converted = match write(&self.0, bytes).map(u32::try_from) {
      Ok(checked) => checked,
      Err(Errno::AGAIN) => {
        let mut watched = [PollFd::new(&self.0, PollFlags::OUT)];

        wait_ready(&mut watched, wait)?;

        let retry = write(&self.0, bytes).map(u32::try_from)?;

        retry
      }
      Err(fault) => return Err(io::Error::from(fault)),
    };

    converted.map_err(io::Error::other)
  }
}

fn nonblocking(endpoint: &OwnedFd) -> rustix::io::Result<()> {
  let flags = fcntl_getfl(endpoint)?;

  fcntl_setfl(endpoint, flags | OFlags::NONBLOCK)
}

fn wait_ready(watched: &mut [PollFd<'_>], wait: Duration) -> io::Result<()> {
  let timeout = Timespec::try_from(wait).map_err(io::Error::other)?;

  match poll(watched, Some(&timeout)) {
    Ok(0) => Err(io::Error::from(io::ErrorKind::TimedOut)),
    Ok(_) => Ok(()),
    Err(fault) => Err(io::Error::from(fault)),
  }
}

pub struct TimedReadPipe(OwnedFd);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeReady {
  Readable,
  Pending,
}

impl TimedReadPipe {
  pub fn new(reader: OwnedFd) -> io::Result<Self> {
    nonblocking(&reader)?;

    Ok(Self(reader))
  }

  pub fn read_some(&self, bytes: &mut [u8], wait: Duration) -> io::Result<u32> {
    let converted = match read(&self.0, &mut *bytes).map(u32::try_from) {
      Ok(checked) => checked,
      Err(Errno::AGAIN) => {
        let mut watched = [PollFd::new(&self.0, PollFlags::IN)];

        wait_ready(&mut watched, wait)?;

        let retry = read(&self.0, bytes).map(u32::try_from)?;

        retry
      }
      Err(fault) => return Err(io::Error::from(fault)),
    };

    converted.map_err(io::Error::other)
  }

  pub fn ready_with(&self, second: &Self, wait: Duration) -> io::Result<[PipeReady; 2]> {
    let mut descriptors = [
      PollFd::new(&self.0, PollFlags::IN),
      PollFd::new(&second.0, PollFlags::IN),
    ];

    wait_ready(&mut descriptors, wait)?;

    Ok(
      descriptors.map(|fd| match fd.revents().is_empty().then_some(()) {
        Some(()) => PipeReady::Pending,
        None => PipeReady::Readable,
      }),
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use rustix::pipe::{PipeFlags, pipe_with};

  const WAIT: Duration = Duration::from_millis(20);

  #[test]
  fn the_reader_distinguishes_timeout_data_and_eof() -> io::Result<()> {
    let (output_read, _still_writing) = pipe_with(PipeFlags::CLOEXEC)?;
    let (error_read, error_writer) = pipe_with(PipeFlags::CLOEXEC)?;
    let output = TimedReadPipe::new(output_read)?;
    let errors = TimedReadPipe::new(error_read)?;
    let mut buffer = [0; 2];

    assert!(
      matches!(output.read_some(&mut buffer, WAIT), Err(fault) if fault.kind() == io::ErrorKind::TimedOut)
    );
    let count = write(&error_writer, b"hi").map(u32::try_from)?;
    let written = count.map_err(io::Error::other)?;
    assert_eq!(written, 2);
    let ready = output.ready_with(&errors, WAIT)?;
    assert_eq!(ready, [PipeReady::Pending, PipeReady::Readable]);
    let heard = errors.read_some(&mut buffer, WAIT)?;
    assert_eq!(heard, 2);
    assert_eq!(&buffer, b"hi");

    drop(error_writer);
    let ended = errors.read_some(&mut buffer, WAIT)?;
    assert_eq!(ended, 0);

    Ok(())
  }

  #[test]
  fn a_full_child_pipe_returns_control_and_resumes_in_order() -> io::Result<()> {
    let (reader, output) = pipe_with(PipeFlags::CLOEXEC)?;
    let size = rustix::pipe::fcntl_getpipe_size(&output).map(u32::try_from)?;
    let capacity = size.map_err(io::Error::other)?;
    let output_2 = TimedPipe::new(output)?;
    let bytes = (0..capacity).map(|_| b'a').collect::<Vec<_>>();
    let written = output_2.write_some(&bytes, WAIT)?;

    assert_eq!(written, capacity);
    assert_eq!(write(&output_2.0, b"b"), Err(Errno::AGAIN));
    assert!(matches!(
      output_2.write_some(b"b", WAIT),
      Err(fault) if fault.kind() == io::ErrorKind::TimedOut
    ));

    let mut drained = bytes.iter().map(|_| 0).collect::<Vec<_>>();
    let count = rustix::io::read(&reader, drained.as_mut_slice()).map(u32::try_from)?;
    let counted = count.map_err(io::Error::other)?;

    assert_eq!(counted, capacity);
    assert_eq!(drained, bytes);

    let resumed = output_2.write_some(b"bc", WAIT)?;

    assert_eq!(resumed, 2);

    let mut last = [0; 2];
    let _bytes_read = rustix::io::read(&reader, &mut last)?;

    assert_eq!(&last, b"bc");

    drop(reader);

    assert!(matches!(
      output_2.write_some(b"z", WAIT),
      Err(fault) if fault.kind() == io::ErrorKind::BrokenPipe
    ));

    Ok(())
  }
}
