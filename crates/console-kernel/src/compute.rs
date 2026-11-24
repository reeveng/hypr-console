//! CPU reference for the first portable compute workload.
//!
//! Each block consists of 256 bytes. The result at each index is the FNV-1a
//! fingerprint of that block, in input order. A reduction job then joins
//! adjacent fingerprints with wrapping addition and carries an unpaired
//! fingerprint forward. The empty job yields zero. Device transfers pack
//! four bytes per little-endian word. FNV-1a is a fingerprint, with no
//! security guarantees against collisions.
//!
//! [`ReductionPlan`] lists dispatches in dependency order and ends with a
//! publication step. [`checksums`] computes one fingerprint per block and
//! leaves the output intact on a size error. [`checksum_sum`] uses one
//! temporary workspace word per block to compute the wrapping sum.

use console_core_checksums::fnv1a;
use console_core_number_conversion::index;

#[cfg(all(target_os = "linux", feature = "hosted-compute"))]
pub mod hosted;

#[cfg(kani)]
#[path = "../proofs/compute.rs"]
mod proofs;

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "Rust array lengths require a usize constant"
  )
)]
pub const BLOCK_BYTES: usize = 256;
pub const MAX_BLOCKS: u32 = 65_536;

pub type Block = [u8; BLOCK_BYTES];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchError {
  TooManyBlocks,
  OutputLength,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReductionStep {
  Join { inputs: u32 },
  Publish,
}

pub struct ReductionPlan {
  remaining: Option<u32>,
}

impl ReductionPlan {
  pub fn new(blocks: u32) -> Result<Self, BatchError> {
    match blocks {
      0..=MAX_BLOCKS => Ok(Self {
        remaining: Some(blocks),
      }),
      _ => Err(BatchError::TooManyBlocks),
    }
  }
}

impl Iterator for ReductionPlan {
  type Item = ReductionStep;

  fn next(&mut self) -> Option<Self::Item> {
    let inputs = self.remaining.take()?;

    match inputs {
      0 | 1 => Some(ReductionStep::Publish),
      2.. => {
        self.remaining = Some(inputs.div_ceil(2));

        Some(ReductionStep::Join { inputs })
      }
    }
  }
}

pub fn checksums(blocks: &[Block], output: &mut [u32]) -> Result<(), BatchError> {
  let count = u32::try_from(blocks.len()).map_err(|_| BatchError::TooManyBlocks)?;

  match count {
    0..=MAX_BLOCKS => {}
    _ => return Err(BatchError::TooManyBlocks),
  }

  match { ::core::primitive::bool::then_some(blocks.len() != output.len(), ()) } {
    ::core::option::Option::Some(()) => {
      return Err(BatchError::OutputLength);
    }
    ::core::option::Option::None => {}
  }

  for (block, checksum) in blocks.iter().zip(output) {
    let Ok(value) = fnv1a::of32(block);
    *checksum = value;
  }

  Ok(())
}

pub fn checksum_sum(blocks: &[Block], workspace: &mut [u32]) -> Result<u32, BatchError> {
  checksums(blocks, workspace)?;
  let count = u32::try_from(blocks.len()).map_err(|_| BatchError::TooManyBlocks)?;
  let plan = ReductionPlan::new(count)?;

  reduce(workspace, plan)
}

fn joined(level: &[u32], parent: u32) -> Result<u32, BatchError> {
  let Ok(first) = index(parent.saturating_mul(2));
  let children = level.get(first..).ok_or(BatchError::OutputLength)?;

  match children {
    [left, right, ..] => Ok(left.wrapping_add(*right)),
    [left] => Ok(*left),
    [] => Err(BatchError::OutputLength),
  }
}

fn reduce(workspace: &mut [u32], plan: ReductionPlan) -> Result<u32, BatchError> {
  for step in plan {
    match step {
      ReductionStep::Join { inputs } => {
        let Ok(length) = index(inputs);
        let active = workspace
          .get_mut(..length)
          .ok_or(BatchError::OutputLength)?;

        for parent in 0..inputs.div_ceil(2) {
          let sum = joined(active, parent)?;
          let Ok(position) = index(parent);
          let destination = active.get_mut(position).ok_or(BatchError::OutputLength)?;
          *destination = sum;
        }
      }
      ReductionStep::Publish => break,
    }
  }

  Ok(match workspace.first() {
    Some(sum) => *sum,
    None => 0,
  })
}
