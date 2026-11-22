//! CPU reference for the first portable compute workload.
//!
//! Each block consists of 256 bytes. The result at each index is the FNV-1a
//! fingerprint of that block, in input order. A reduction job then joins
//! adjacent fingerprints with wrapping addition and carries an unpaired
//! fingerprint forward. The empty job yields zero. Device transfers pack
//! four bytes per little-endian word. FNV-1a is a fingerprint, with no
//! security guarantees against collisions.

use console_core_checksums::fnv1a;

#[cfg(kani)]
#[path = "../proofs/compute.rs"]
mod proofs;

pub const BLOCK_BYTES: usize = 256;
pub const MAX_BLOCKS: usize = 65_536;

pub type Block = [u8; BLOCK_BYTES];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchError {
  TooManyBlocks,
  OutputLength,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReductionStep {
  Join { inputs: usize },
  Publish,
}

/// Lists the dispatches in dependency order, followed by one result boundary.
pub struct ReductionPlan {
  remaining: Option<usize>,
}

impl ReductionPlan {
  pub fn new(blocks: usize) -> Result<Self, BatchError> {
    if blocks > MAX_BLOCKS {
      return Err(BatchError::TooManyBlocks);
    }

    Ok(Self {
      remaining: Some(blocks),
    })
  }
}

impl Iterator for ReductionPlan {
  type Item = ReductionStep;

  fn next(&mut self) -> Option<Self::Item> {
    let inputs = self.remaining.take()?;
    if inputs <= 1 {
      return Some(ReductionStep::Publish);
    }

    self.remaining = Some(inputs.div_ceil(2));
    Some(ReductionStep::Join { inputs })
  }
}

/// Computes one fingerprint per block. A size error leaves the output intact.
pub fn checksums(blocks: &[Block], output: &mut [u32]) -> Result<(), BatchError> {
  if blocks.len() > MAX_BLOCKS {
    return Err(BatchError::TooManyBlocks);
  }
  if blocks.len() != output.len() {
    return Err(BatchError::OutputLength);
  }

  for (block, checksum) in blocks.iter().zip(output) {
    let Ok(value) = fnv1a::of32(block);
    *checksum = value;
  }

  Ok(())
}

/// Returns the wrapping sum of the block fingerprints. The workspace must
/// have one word per block and its contents are temporary.
pub fn checksum_sum(blocks: &[Block], workspace: &mut [u32]) -> Result<u32, BatchError> {
  checksums(blocks, workspace)?;
  let plan = ReductionPlan::new(blocks.len())?;

  Ok(reduce(workspace, plan))
}

fn joined(level: &[u32], parent: usize) -> u32 {
  let left = parent * 2;
  level[left].wrapping_add(level.get(left + 1).copied().unwrap_or(0))
}

fn reduce(workspace: &mut [u32], plan: ReductionPlan) -> u32 {
  for step in plan {
    match step {
      ReductionStep::Join { inputs } => {
        for parent in 0..inputs.div_ceil(2) {
          workspace[parent] = joined(&workspace[..inputs], parent);
        }
      }
      ReductionStep::Publish => return workspace.first().copied().unwrap_or(0),
    }
  }

  unreachable!("a new reduction plan always yields one publication step")
}
