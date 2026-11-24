//! A typed checksum effect for the hosted program runtime.
//!
//! Vulkan builds place the shared `wrapping-sum.spv` beside the checksum shader.

extern crate std;

use alloc::vec::Vec;
use std::{io, path::Path};

use console_core_never::Never;
use console_core_state_machine::{EffectDependencies, Service};
use console_operating_system_linux::{
  compute_map,
  program_runtime::compute::{self, Map, MapProgram, Reduction, Target, Workload},
};

use super::{BatchError, Block, ReductionPlan};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checksums<'blocks> {
  blocks: &'blocks [Block],
  count: u32,
  result: Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
  Blocks,
  Sum,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Checksummed {
  Blocks(Vec<u32>),
  Sum(u32),
}

impl EffectDependencies for Checksums<'_> {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(&[Service::Compute])
  }
}

impl<'blocks> Checksums<'blocks> {
  pub fn new(blocks: &'blocks [Block], result: Output) -> Result<Self, BatchError> {
    let count = u32::try_from(blocks.len()).map_err(|_| BatchError::TooManyBlocks)?;
    ReductionPlan::new(count)?;
    Ok(Self {
      blocks,
      count,
      result,
    })
  }

  #[must_use]
  pub fn workload(&self) -> Result<Workload, Never> {
    Ok(Workload {
      effect: match self.result {
        Output::Blocks => "checksum blocks",
        Output::Sum => "checksum sum",
      },
      size: u64::from(self.count),
    })
  }

  pub fn calculate(&self, target: Target<'_>, spirv: Option<&Path>) -> io::Result<Checksummed> {
    let input: Vec<_> = self
      .blocks
      .as_flattened()
      .as_chunks::<4>()
      .0
      .iter()
      .copied()
      .map(u32::from_le_bytes)
      .collect();
    let work = Map {
      program: MapProgram {
        kernel: &CHECKSUM_KERNEL,
        spirv,
      },
      input: &input,
      dimensions: [self.count, 1, 1],
      parameters: [0; 2],
    };

    match self.result {
      Output::Blocks => compute::map(target, work).map(Checksummed::Blocks),
      Output::Sum => {
        let sum = spirv.map(|shader| shader.with_file_name("wrapping-sum.spv"));
        let Ok(reduction) = Reduction::wrapping_sum(sum.as_deref());
        compute::map_reduce(target, work, reduction).map(Checksummed::Sum)
      }
    }
  }
}

compute_map! {
  pub static CHECKSUM_KERNEL = [64, 1, 1] |item| {
    let Ok(block): Result<u32, Never> = item.index();
    let start: u32 = u32::wrapping_mul(block, 64);
    let mut state: u32 = 0x811c_9dc5u32;

    for index in 0u32..64u32 {
      let Ok(word): Result<u32, Never> = item.word(u32::wrapping_add(start, index));

      for byte in 0u32..4u32 {
        let shift: u32 = u32::wrapping_mul(byte, 8);
        state = u32::wrapping_mul(state ^ (u32::wrapping_shr(word, shift) & 255), 0x0100_0193u32);
      }
    }

    state
  }
}
