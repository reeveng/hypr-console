//! Framed requests to the Vulkan worker. One request is in flight per worker.
//! Each request starts with the shader count and length-prefixed paths, followed
//! by a GPU1 batch. Each reply starts with the output word count. A count of
//! `u32::MAX` introduces a length-prefixed error message. All counts are little-endian.
//! GPU2 uses the same request header and replies with a `D` byte and a DMA-BUF
//! descriptor, or the same error frame. Requests and replies are serialized.

use std::ffi::CStr;
use std::io::{self, BufWriter, IoSliceMut, Write};
use std::mem::MaybeUninit;
use std::num::NonZeroU32;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::{self, ThreadId};
use std::time::Duration;

use console_core_number_conversion::{fitted, fixed_index, index};
use rustix::net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, recvmsg};

use crate::program_lifetime::{TimedPipe, TimedReadPipe, hooks};

use super::{Batch, EMPTY_TRANSFER_BUFFER, MAX_WORDS};

const MAX_PATH_BYTES: u32 = 4095;
const FAULT_PREFIX: [u8; 4] = u32::MAX.to_le_bytes();
pub(super) const WORKER_IO_TIMEOUT: Duration = Duration::from_secs(120);

pub(super) struct Paths(Vec<PathBuf>);

impl Paths {
  pub(super) fn new<'a>(modules: impl IntoIterator<Item = &'a Path>) -> io::Result<Self> {
    let paths: Vec<_> = modules
      .into_iter()
      .map(std::path::absolute)
      .collect::<io::Result<_>>()?;

    let invalid = paths.iter().any(|path| {
      let bytes = path.as_os_str().as_bytes();
      let Ok(length) = console_core_number_conversion::fixed_index(bytes.len());

      match length.cmp(&u64::from(MAX_PATH_BYTES)) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Equal | std::cmp::Ordering::Less => {
          match CStr::from_bytes_until_nul(bytes) {
            Ok(_truncated_path) => true,
            Err(_no_nul) => false,
          }
        }
      }
    });

    match bool::then_some(invalid, ()) {
      Some(()) => Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "invalid Vulkan shader path",
      )),
      None => Ok(Self(paths)),
    }
  }

  fn write(self, to: &mut impl Write) -> io::Result<()> {
    to.write_all(
      &match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u32>(self.0.len())
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      }
      .to_le_bytes(),
    )?;

    for path in &self.0 {
      let path_2 = path.as_os_str().as_bytes();
      to.write_all(
        &match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u32>(
          path_2.len(),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        }
        .to_le_bytes(),
      )?;
      to.write_all(path_2)?;
    }

    Ok(())
  }
}

pub(super) enum Reuse {
  Keep,
  Restart,
}

pub(super) struct Session {
  child: Child,
  input: TimedPipe,
  output: TimedReadPipe,
  started_by: ThreadId,
}

impl Session {
  pub(super) fn start(path: &Path) -> io::Result<Self> {
    let (input, child_input) = UnixStream::pair()?;
    let (output, child_output) = UnixStream::pair()?;
    let writer = TimedPipe::new(OwnedFd::from(input))?;
    let reader = TimedReadPipe::new(OwnedFd::from(output))?;
    let mut command = Command::new(path);
    command
      .arg("--stream")
      .stdin(Stdio::from(OwnedFd::from(child_input)))
      .stdout(Stdio::from(OwnedFd::from(child_output)))
      .stderr(Stdio::null());
    let Ok(()) = hooks::bind(&mut command);
    let child = command.spawn()?;

    Ok(Self {
      child,
      input: writer,
      output: reader,
      started_by: thread::current().id(),
    })
  }

  pub(super) fn reuse(&mut self) -> io::Result<Reuse> {
    match bool::then_some(self.started_by == thread::current().id(), ()) {
      Some(()) => {
        let status = self.child.try_wait()?;
        Ok(match status {
          Some(_) => Reuse::Restart,
          None => Reuse::Keep,
        })
      }
      None => Ok(Reuse::Restart),
    }
  }

  pub(super) fn dispatch<K>(
    &mut self,
    paths: Paths,
    batch: &Batch<'_, K>,
    header: [u32; 6],
  ) -> io::Result<Vec<u32>> {
    self.send(paths, batch, header)?;
    read_reply(
      &self.output,
      batch.output.end.saturating_sub(batch.output.start),
    )
  }

  pub(super) fn export<K>(
    &mut self,
    paths: Paths,
    batch: &Batch<'_, K>,
    mut header: [u32; 6],
  ) -> io::Result<OwnedFd> {
    let [magic, ..] = &mut header;
    *magic = u32::from_le_bytes(*b"GPU2");
    self.send(paths, batch, header)?;
    read_descriptor(&self.output)
  }

  fn send<K>(&mut self, paths: Paths, batch: &Batch<'_, K>, header: [u32; 6]) -> io::Result<()> {
    let mut to = BufWriter::new(Transfer(&self.input));
    paths.write(&mut to)?;
    batch.write(header, to)
  }
}

struct Transfer<'a>(&'a TimedPipe);

impl Write for Transfer<'_> {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "std::io::Write fixes the adapter's return type; the pipe counts transferred bytes in u32"
    )
  )]
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    let written = self.0.write_some(bytes, WORKER_IO_TIMEOUT)?;
    let Ok(native) = index(written);
    Ok(native)
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

fn fill(from: &TimedReadPipe, mut bytes: &mut [u8]) -> io::Result<()> {
  let Ok(receiving) = fixed_index(bytes.len());

  for _ in 0..receiving {
    match bytes {
      [] => return Ok(()),
      _ => {}
    }

    let read = from.read_some(bytes, WORKER_IO_TIMEOUT)?;

    match read {
      0 => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
      _ => {}
    }

    let Ok(native) = index(read);
    let (_, remaining) = bytes
      .split_at_mut_checked(native)
      .ok_or_else(|| io::Error::other("Vulkan worker read exceeded its destination"))?;
    bytes = remaining;
  }

  Ok(())
}

fn read_descriptor(from: &TimedReadPipe) -> io::Result<OwnedFd> {
  let invalid = || io::Error::new(io::ErrorKind::InvalidData, "invalid Vulkan DMA-BUF reply");
  let mut tag = [0];
  let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
  let mut ancillary = RecvAncillaryBuffer::new(&mut storage);
  from.wait_readable(WORKER_IO_TIMEOUT)?;
  let reply = recvmsg(
    from,
    &mut [IoSliceMut::new(&mut tag)],
    &mut ancillary,
    RecvFlags::CMSG_CLOEXEC,
  )?;

  match reply.bytes {
    0 => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
    _ => {}
  }

  match reply
    .flags
    .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
    .then_some(())
  {
    Some(()) => return Err(invalid()),
    None => {}
  }

  let mut descriptor = None;

  for message in ancillary.drain() {
    #[cfg_attr(
      dylint_lib = "explicit016_no_wildcard_arm",
      expect(
        explicit016_no_wildcard_arm,
        reason = "Rustix's non-exhaustive ancillary message enum requires rejecting every kind outside the DMA-BUF descriptor protocol"
      )
    )]
    let rights = match message {
      RecvAncillaryMessage::ScmRights(rights) => rights,
      _ => return Err(invalid()),
    };

    for received in rights {
      match descriptor.replace(received) {
        Some(_) => return Err(invalid()),
        None => {}
      }
    }
  }

  match (tag, descriptor) {
    ([b'D'], Some(fd)) => Ok(fd),
    ([0xff], None) => {
      let mut remaining = [0; 3];
      fill(from, &mut remaining)?;

      match remaining {
        [0xff, 0xff, 0xff] => {
          let fault = read_worker_fault(from)?;
          Err(fault)
        }
        _ => Err(invalid()),
      }
    }
    _ => Err(invalid()),
  }
}

impl Drop for Session {
  fn drop(&mut self) {
    let ::core::result::Result::Ok(()) = stop(&mut self.child);
  }
}

fn stop(child: &mut Child) -> ::core::result::Result<(), ::console_core_never::Never> {
  let _already_exited = child.kill();
  let _reaped = child.wait();
  ::core::result::Result::Ok(())
}

fn read_reply(from: &TimedReadPipe, expected_words: u32) -> io::Result<Vec<u32>> {
  let mut prefix = [0; 4];
  fill(from, &mut prefix)?;

  match prefix {
    FAULT_PREFIX => {
      let fault = read_worker_fault(from)?;
      Err(fault)
    }
    count => read_words(
      from,
      ReplyLength {
        reported: u32::from_le_bytes(count),
        expected: expected_words,
      },
    ),
  }
}

fn read_worker_fault(from: &TimedReadPipe) -> io::Result<io::Error> {
  let mut prefix = [0; 4];
  fill(from, &mut prefix)?;
  let Ok(length) = index(u32::from_le_bytes(prefix));
  let mut details = [0; 1024];
  let message = details.get_mut(..length).ok_or_else(|| {
    io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid Vulkan worker fault length",
    )
  })?;
  fill(from, message)?;
  let text = std::str::from_utf8(message)
    .map_err(|fault| io::Error::new(io::ErrorKind::InvalidData, fault))?;
  Ok(io::Error::other(format!("Vulkan worker failed: {text}")))
}

struct ReplyLength {
  reported: u32,
  expected: u32,
}

fn read_words(
  from: &TimedReadPipe,
  ReplyLength { reported, expected }: ReplyLength,
) -> io::Result<Vec<u32>> {
  match (reported == expected && reported <= MAX_WORDS).then_some(()) {
    Some(()) => {}
    None => {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Vulkan worker returned an unexpected output size",
      ));
    }
  }

  let mut result = Vec::new();
  let mut bytes = EMPTY_TRANSFER_BUFFER;
  let Ok(transfer_words) = fitted::<_, u32>(EMPTY_TRANSFER_BUFFER.len());
  let chunk_words = NonZeroU32::new(transfer_words)
    .ok_or_else(|| io::Error::other("Vulkan transfer buffer is empty"))?;
  let chunks = reported.div_ceil(chunk_words.get());

  for block in 0..chunks {
    let previous = block.saturating_mul(chunk_words.get());
    let words = reported.saturating_sub(previous).min(chunk_words.get());
    let Ok(length) = index(words);
    let chunk = bytes.get_mut(..length).ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::InvalidData,
        "Vulkan worker chunk exceeds its buffer",
      )
    })?;
    fill(from, chunk.as_flattened_mut())?;
    result.try_reserve(chunk.len()).map_err(io::Error::other)?;
    result.extend(chunk.iter().copied().map(u32::from_le_bytes));
  }

  Ok(result)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn fixture(bytes: &[u8]) -> io::Result<TimedReadPipe> {
    let (receiving, writing) = UnixStream::pair()?;
    let writer = TimedPipe::new(OwnedFd::from(writing))?;
    Transfer(&writer).write_all(bytes)?;
    TimedReadPipe::new(OwnedFd::from(receiving))
  }

  #[test]
  fn replies_end_at_their_declared_length() -> io::Result<()> {
    let bytes: Vec<_> = [0u32, 2, 0x1234_5678, 0x9876_5432, 1]
      .into_iter()
      .flat_map(u32::to_le_bytes)
      .collect();
    let replies = fixture(&bytes)?;
    let reply = read_reply(&replies, 0)?;
    assert!(reply.is_empty());
    let values = read_reply(&replies, 2)?;
    assert_eq!(values, [0x1234_5678, 0x9876_5432]);
    assert_eq!(
      read_reply(&replies, 1).unwrap_err().kind(),
      io::ErrorKind::UnexpectedEof
    );
    Ok(())
  }

  #[test]
  fn worker_faults_keep_encoding_and_length_errors_distinct() -> io::Result<()> {
    for (bytes, kind) in [
      (b"device lost".as_slice(), io::ErrorKind::Other),
      (b"\xff".as_slice(), io::ErrorKind::InvalidData),
    ] {
      let length = u32::try_from(bytes.len()).map_err(io::Error::other)?;
      let mut frame = Vec::from(FAULT_PREFIX);
      frame.extend(length.to_le_bytes());
      frame.extend(bytes);

      let receiving = fixture(&frame)?;
      let reply = read_reply(&receiving, 0);

      match reply {
        Err(fault) => assert_eq!(fault.kind(), kind),
        Ok(_) => return Err(io::Error::other("expected a worker fault")),
      }
    }

    let mut frame = Vec::from(FAULT_PREFIX);
    frame.extend(1025u32.to_le_bytes());
    let receiving = fixture(&frame)?;
    let reply = read_reply(&receiving, 0);

    match reply {
      Err(fault) => assert_eq!(fault.kind(), io::ErrorKind::InvalidData),
      Ok(_) => return Err(io::Error::other("accepted an oversized worker fault")),
    }

    Ok(())
  }
}
