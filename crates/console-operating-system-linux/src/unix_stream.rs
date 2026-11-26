//! Deadlines for local stream requests and the frames in persistent subscriptions.
//!
//! A full listener backlog is reported immediately so the owner can retry.
//! A subscription may wait between messages. The remaining fragments of a
//! started message share a deadline. An exchange can also cap the first wait.

use std::io::{self, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use console_core_bounded_reads::{Line, LinePart, ReadError};
use rustix::net::{AddressFamily, SocketAddrUnix, SocketFlags, SocketType, socket_with};

pub fn connect(at: &Path) -> io::Result<UnixStream> {
  let address = SocketAddrUnix::new(at)?;

  connect_address(&address)
}

pub fn connect_abstract(name: &[u8]) -> io::Result<UnixStream> {
  let address = SocketAddrUnix::new_abstract_name(name)?;

  connect_address(&address)
}

fn connect_address(address: &SocketAddrUnix) -> io::Result<UnixStream> {
  let descriptor = socket_with(
    AddressFamily::UNIX,
    SocketType::STREAM,
    SocketFlags::NONBLOCK | SocketFlags::CLOEXEC,
    None,
  )?;
  rustix::net::connect(&descriptor, address)?;
  let stream = UnixStream::from(descriptor);
  stream.set_nonblocking(false)?;

  Ok(stream)
}

pub struct Within<'a> {
  stream: &'a UnixStream,
  deadline: Instant,
}

impl<'a> Within<'a> {
  #[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    expect(
      explicit039_no_reading_the_clock,
      reason = "the request deadline starts at this socket I/O boundary"
    )
  )]
  pub fn new(stream: &'a UnixStream, budget: Duration) -> io::Result<Self> {
    let deadline = Instant::now()
      .checked_add(budget)
      .ok_or(io::ErrorKind::InvalidInput)?;

    Ok(Self { stream, deadline })
  }
}

impl Read for Within<'_> {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    allow(
      explicit051_no_machine_width,
      reason = "the Read trait uses usize for byte counts"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "the remaining absolute deadline is set as the kernel read timeout before every read"
    )
  )]
  fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
    let remaining = time_left(self.deadline)?;
    self.stream.set_read_timeout(Some(remaining))?;

    timed_io(self.stream.read(bytes))
  }
}

impl Write for Within<'_> {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    allow(
      explicit051_no_machine_width,
      reason = "the Write trait uses usize for byte counts"
    )
  )]
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    let remaining = time_left(self.deadline)?;
    self.stream.set_write_timeout(Some(remaining))?;

    timed_io(self.stream.write(bytes))
  }

  fn flush(&mut self) -> io::Result<()> {
    self.stream.flush()
  }
}

fn timed_io<T>(result: io::Result<T>) -> io::Result<T> {
  result.map_err(
    |fault| match (fault.kind() == io::ErrorKind::WouldBlock).then_some(()) {
      Some(()) => io::Error::from(io::ErrorKind::TimedOut),
      None => fault,
    },
  )
}

#[cfg_attr(
  dylint_lib = "explicit039_no_reading_the_clock",
  expect(
    explicit039_no_reading_the_clock,
    reason = "this computes the remaining kernel timeout at the socket boundary"
  )
)]
fn time_left(deadline: Instant) -> io::Result<Duration> {
  let remaining = deadline.saturating_duration_since(Instant::now());

  match remaining.is_zero().then_some(()) {
    Some(()) => Err(io::Error::from(io::ErrorKind::TimedOut)),
    None => Ok(remaining),
  }
}

#[cfg_attr(
  dylint_lib = "explicit039_no_reading_the_clock",
  expect(
    explicit039_no_reading_the_clock,
    reason = "a partial frame starts its deadline at the socket I/O boundary"
  )
)]
pub fn receive_line(
  input: &mut BufReader<UnixStream>,
  frame: &mut Line,
  mut deadline: Option<Instant>,
  frame_time: Duration,
) -> Result<LinePart, ReadError> {
  let Ok(()) = frame.clear();

  #[cfg_attr(
    dylint_lib = "explicit054_no_loop",
    expect(
      explicit054_no_loop,
      reason = "each partial chunk spends the bounded frame byte count and the first starts a finite deadline"
    )
  )]
  loop {
    let remaining = deadline.map(time_left).transpose().map_err(ReadError::Io)?;

    match input.buffer().is_empty().then_some(()) {
      Some(()) => input
        .get_ref()
        .set_read_timeout(remaining)
        .map_err(ReadError::Io)?,
      None => {}
    }

    let part = frame.read_part(input)?;

    match part {
      LinePart::Partial => {
        deadline = deadline.or_else(|| Instant::now().checked_add(frame_time));
      }
      LinePart::Complete | LinePart::End => return Ok(part),
    }
  }
}
