//! A stream that hands every byte read through it to a [`Monitor`] as it passes.
//!
//! Skipped bytes are not handed on, which is what the formats that checksum a header and
//! skip its padding expect.

use std::io;

use console_core_never::Never;
use console_core_number_conversion::index;

use super::{Monitor, ReadBytes};

pub struct MonitorStream<B: ReadBytes, M: Monitor> {
  inner: B,
  monitor: M,
}

impl<B: ReadBytes, M: Monitor> MonitorStream<B, M> {
  pub fn new(inner: B, monitor: M) -> Result<MonitorStream<B, M>, Never> {
    Ok(MonitorStream { inner, monitor })
  }

  pub fn inner(&self) -> Result<&B, Never> {
    Ok(&self.inner)
  }

  pub fn inner_mut(&mut self) -> Result<&mut B, Never> {
    Ok(&mut self.inner)
  }

  pub fn into_inner(self) -> Result<B, Never> {
    Ok(self.inner)
  }

  pub fn monitor(&self) -> Result<&M, Never> {
    Ok(&self.monitor)
  }
}

impl<B: ReadBytes, M: Monitor> ReadBytes for MonitorStream<B, M> {
  #[inline(always)]
  fn read_byte(&mut self) -> io::Result<u8> {
    let byte = self.inner.read_byte()?;
    let Ok(()) = self.monitor.process_byte(byte);

    Ok(byte)
  }

  #[inline(always)]
  fn read_double_bytes(&mut self) -> io::Result<[u8; 2]> {
    let bytes = self.inner.read_double_bytes()?;
    let Ok(()) = self.monitor.process_double_bytes(bytes);

    Ok(bytes)
  }

  #[inline(always)]
  fn read_triple_bytes(&mut self) -> io::Result<[u8; 3]> {
    let bytes = self.inner.read_triple_bytes()?;
    let Ok(()) = self.monitor.process_triple_bytes(bytes);

    Ok(bytes)
  }

  #[inline(always)]
  fn read_quad_bytes(&mut self) -> io::Result<[u8; 4]> {
    let bytes = self.inner.read_quad_bytes()?;
    let Ok(()) = self.monitor.process_quad_bytes(bytes);

    Ok(bytes)
  }

  fn read_buf(&mut self, buf: &mut [u8]) -> io::Result<u64> {
    let count = self.inner.read_buf(buf)?;
    let Ok(at) = index(count);

    let read = match buf.get(..at) {
      Some(read) => read,
      None => buf,
    };

    let Ok(()) = self.monitor.process_buf_bytes(read);

    Ok(count)
  }

  fn read_buf_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
    self.inner.read_buf_exact(buf)?;

    let Ok(()) = self.monitor.process_buf_bytes(buf);

    Ok(())
  }

  fn ignore_bytes(&mut self, count: u64) -> io::Result<()> {
    self.inner.ignore_bytes(count)
  }

  #[inline(always)]
  fn pos(&self) -> u64 {
    self.inner.pos()
  }
}
