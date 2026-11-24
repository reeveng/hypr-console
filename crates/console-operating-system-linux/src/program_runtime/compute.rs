//! Compute effects dispatch portable kernels on the CPU or Vulkan.
//!
//! Use [`map`], [`map_pipeline`], or [`reject`] with a Rust item defined by
//! [`crate::compute_map`]. A pipeline keeps a fixed output grid and passes each
//! stage's words to the next stage inside one batch. `reject` keeps the input
//! words whose predicate returns zero, in their original order. It expects one
//! predicate result per word and compacts the returned mask on the host.
//!
//! [`reduce`] combines existing words with the same pairwise tree as
//! [`map_reduce`]. Each [`Reduction`] shader merges consecutive pairs in input
//! order and carries forward an odd tail. [`scan`] returns inclusive prefixes
//! using an associative [`Scan`] shader. The shader combines the word one
//! stride to the left with the current word, carrying forward words before
//! the stride. The runtime supplies the stride through `MapItem::param(0)`.
//! [`Scan::wrapping_sum`] and [`Reduction::wrapping_sum`] have built-in kernels.
//!
//! For explicit passes, define a kernel with [`crate::compute_kernel`]. The CPU runs
//! the Rust function directly. For Vulkan, compile [`Kernel::glsl`] to SPIR-V
//! using the existing shader build tools. The interpreter configures trusted
//! program paths. A machine requests a calculation with `Effect::Custom`.
//!
//! Shaders are trusted programs. Each shader uses an entry point named `main`,
//! one storage buffer at set 0/binding 0, and up to eight 32-bit push words.
//! Input starts at word zero. The remainder of the arena starts zeroed. Passes
//! can exchange data in the arena and only the requested range is read back.
//! Dispatches are ordered. Workgroups in one pass run independently. [`Vulkan`]
//! keeps one worker session alive across calls on the same thread. Use
//! [`Vulkan::dispatch`] to require the device during parity checks or [`run`]
//! to run the CPU implementation if a device attempt fails.
//! [`Vulkan::export`] produces a dedicated linear DMA-BUF for the requested output.

use std::io::{self, Write};
use std::ops::Range;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, TryLockError};
use std::time::Duration;

use console_core_never::Never;
use console_waiting::{Schedule, until_some_handed};

mod iteration;
mod partition;
mod portable;
mod routing;
mod shader;
mod worker;
pub use partition::Partition;
pub use portable::{
  Invocation, Kernel, KernelSource, Program, Target, WordAt, dispatch, dispatch_partitioned,
};
pub use routing::{Backend, ComputeId, History, Routed, Router, RoutingPolicy, Workload};

const MAX_WORDS: u32 = 16 * 1024 * 1024;
const MAX_PASSES: u32 = 64;
const MAX_SHADERS: u32 = 16;
const EMPTY_TRANSFER_BUFFER: [[u8; 4]; 4096] = [[0; 4]; 4096];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pass {
  pub shader: u32,
  pub groups: [u32; 3],
  pub push: [u32; 8],
}

#[derive(Debug)]
pub struct Batch<'a, K = &'a Path> {
  pub shaders: &'a [K],
  pub passes: &'a [Pass],
  pub input: &'a [u32],
  pub arena_words: u32,
  pub output: Range<u32>,
}

pub struct Vulkan {
  worker: PathBuf,
  session: Mutex<Option<worker::Session>>,
}

impl Vulkan {
  #[must_use]
  pub fn new(worker: PathBuf) -> Result<Self, Never> {
    Ok(Self {
      worker,
      session: Mutex::new(None),
    })
  }

  pub fn dispatch(&self, batch: Batch<'_>) -> io::Result<Vec<u32>> {
    self.dispatch_programs(&batch, batch.shaders.iter().copied())
  }

  pub fn export(&self, batch: Batch<'_>) -> io::Result<OwnedFd> {
    let header = batch.header()?;

    match batch.output.is_empty().then_some(()) {
      Some(()) => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidInput,
          "empty compute export",
        ));
      }
      None => {}
    }

    let paths = worker::Paths::new(batch.shaders.iter().copied())?;
    self.with_session(|live| live.export(paths, &batch, header))
  }

  fn dispatch_programs<'a, K>(
    &self,
    batch: &Batch<'_, K>,
    modules: impl IntoIterator<Item = &'a Path>,
  ) -> io::Result<Vec<u32>> {
    let header = batch.header()?;
    let paths = worker::Paths::new(modules)?;
    self.with_session(|live| live.dispatch(paths, batch, header))
  }

  fn with_session<T>(
    &self,
    operation: impl FnOnce(&mut worker::Session) -> io::Result<T>,
  ) -> io::Result<T> {
    let mut session = self.claim_session()?;

    let mut live = match session.take() {
      Some(mut running) => {
        let reuse = running.reuse()?;

        match reuse {
          worker::Reuse::Keep => running,
          worker::Reuse::Restart => {
            drop(running);
            worker::Session::start(&self.worker)?
          }
        }
      }
      None => worker::Session::start(&self.worker)?,
    };
    let output = operation(&mut live)?;
    *session = Some(live);
    Ok(output)
  }

  fn claim_session(&self) -> io::Result<MutexGuard<'_, Option<worker::Session>>> {
    let Ok(available) = self.available_session();

    match available {
      Some(session) => return Ok(session),
      None => {}
    }

    let Ok(schedule) =
      Schedule::asking_every(worker::WORKER_IO_TIMEOUT, Duration::from_millis(10));
    let Ok(claimed) = until_some_handed(schedule, &mut (), |()| self.available_session());

    claimed.ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "Vulkan session is still busy"))
  }

  fn available_session(&self) -> Result<Option<MutexGuard<'_, Option<worker::Session>>>, Never> {
    Ok(match self.session.try_lock() {
      Ok(session) => Some(session),
      Err(TryLockError::WouldBlock) => None,
      Err(TryLockError::Poisoned(poisoned)) => {
        console_core_logging::warn!("recovered a poisoned Vulkan session lock");
        Some(poisoned.into_inner())
      }
    })
  }
}

#[must_use]
pub fn run<T>(
  accelerator: Option<&Vulkan>,
  gpu: impl FnOnce(&Vulkan) -> io::Result<T>,
  cpu: impl FnOnce() -> io::Result<T>,
) -> io::Result<T> {
  match accelerator {
    Some(device) => match gpu(device) {
      Ok(result) => return Ok(result),
      Err(fault) => console_core_logging::debug!("using CPU compute: {fault}"),
    },
    ::core::option::Option::None => {}
  }

  cpu()
}

impl<K> Batch<'_, K> {
  fn header(&self) -> io::Result<[u32; 6]> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, "invalid compute batch");
    let inputs = u32::try_from(self.input.len()).map_err(|_| invalid())?;
    let shaders = u32::try_from(self.shaders.len()).map_err(|_| invalid())?;
    let passes = u32::try_from(self.passes.len()).map_err(|_| invalid())?;

    let invalid_dimensions = self.arena_words == 0
      || self.arena_words > MAX_WORDS
      || inputs > self.arena_words
      || self.output.start > self.output.end
      || self.output.end > self.arena_words
      || shaders == 0
      || shaders > MAX_SHADERS
      || passes == 0
      || passes > MAX_PASSES
      || self
        .passes
        .iter()
        .any(|pass| pass.shader >= shaders || pass.groups.contains(&0));

    match bool::then_some(invalid_dimensions, ()) {
      Some(()) => return Err(invalid()),
      None => {}
    }

    Ok([
      u32::from_le_bytes(*b"GPU1"),
      self.arena_words,
      inputs,
      self.output.start,
      self.output.end.saturating_sub(self.output.start),
      passes,
    ])
  }

  fn write(&self, header: [u32; 6], mut to: impl Write) -> io::Result<()> {
    for word in header {
      to.write_all(&word.to_le_bytes())?;
    }

    for pass in self.passes {
      for word in [pass.shader]
        .into_iter()
        .chain(pass.groups)
        .chain(pass.push)
      {
        to.write_all(&word.to_le_bytes())?;
      }
    }

    let mut bytes = EMPTY_TRANSFER_BUFFER;
    let (blocks, tail) = self.input.as_chunks::<{ EMPTY_TRANSFER_BUFFER.len() }>();

    for words in blocks
      .iter()
      .map(|block| block.as_slice())
      .chain(std::iter::once(tail))
    {
      let chunk = bytes.get_mut(..words.len()).ok_or_else(|| {
        io::Error::new(
          io::ErrorKind::InvalidInput,
          "compute batch exceeds its transfer buffer",
        )
      })?;

      for (bytes_2, &word) in chunk.iter_mut().zip(words) {
        *bytes_2 = word.to_le_bytes();
      }

      to.write_all(chunk.as_flattened())?;
    }

    to.flush()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn an_invalid_result_range_or_shader_index_cannot_reach_the_worker() -> io::Result<()> {
    let valid = Pass {
      shader: 0,
      groups: [3, 2, 1],
      push: [0; 8],
    };
    let unregistered = [Pass { shader: 1, ..valid }];
    let mut batch = Batch {
      shaders: &[Path::new("trusted.spv")],
      passes: &[valid],
      input: &[1, 2],
      arena_words: 4,
      output: 2..4,
    };
    let _valid_header = batch.header()?;
    batch.output.end = 5;
    assert_eq!(
      batch.header().unwrap_err().kind(),
      io::ErrorKind::InvalidInput
    );
    batch.output.end = 4;
    batch.passes = &unregistered;
    assert_eq!(
      batch.header().unwrap_err().kind(),
      io::ErrorKind::InvalidInput
    );
    Ok(())
  }

  #[test]
  fn fallback_computes_one_result_if_device_work_fails() -> io::Result<()> {
    let Ok(device) = Vulkan::new(PathBuf::new());
    let result = run(
      Some(&device),
      |_| Err(io::Error::other("device lost")),
      || Ok(42),
    )?;
    assert_eq!(result, 42);
    let result_2 = run(Some(&device), |_| Ok(42), || Ok(0))?;
    assert_eq!(result_2, 42);
    Ok(())
  }
}

pub use iteration::{
  Map, MapItem, MapKernel, MapProgram, MapStage, Reduction, Scan, WRAPPING_PREFIX_SUM,
  WRAPPING_SUM, map, map_export, map_pipeline, map_reduce, reduce, reject, scan,
};
