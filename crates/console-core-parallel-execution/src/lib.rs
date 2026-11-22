//! A caller-supplied executor for workers that live across multiple operations.
//!
//! [`Workers::start`] starts work independently of the caller. A job can wait
//! for a later submission from the caller, so it must be able to run after
//! `start` returns. An error means the executor discarded the job before
//! starting it.

use std::io;
use std::num::NonZeroU32;

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Failed;

type Join = Box<dyn FnOnce() -> Result<(), Failed> + Send + Sync>;

pub struct Worker {
  join: Join,
}

impl Worker {
  pub fn new(
    join: impl FnOnce() -> Result<(), Failed> + Send + Sync + 'static,
  ) -> Result<Self, Never> {
    Ok(Self {
      join: Box::new(join),
    })
  }

  pub fn join(self) -> Result<(), Failed> {
    (self.join)()
  }
}

pub trait Workers {
  fn parallelism(&self) -> Result<NonZeroU32, Never>;

  fn start(&self, name: String, work: impl FnOnce() + Send + 'static) -> io::Result<Worker>;
}

pub struct Serial;

impl Workers for Serial {
  fn parallelism(&self) -> Result<NonZeroU32, Never> {
    Ok(NonZeroU32::MIN)
  }

  fn start(&self, _name: String, _work: impl FnOnce() + Send + 'static) -> io::Result<Worker> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
  }
}
