//! Validation and execution for independent words in a compute output.
//!
//! Each active item owns one output word in row-major order. Items can read
//! their own word and the arena outside the output range. Padded GPU items
//! must return without writing.

use std::io;
use std::num::NonZeroU32;
use std::ops::Range;
use std::sync::Mutex;

use console_concurrency::{self as concurrency, Cores};
use console_core_never::Never;
use console_core_number_conversion::index;

use super::Invocation;
use super::portable::{PartitionAccess, PartitionWords};

#[derive(Debug, Clone, Copy)]
pub struct Partition {
  pub output_at: u32,
  pub dimensions: [u32; 3],
}

pub(super) struct Layout {
  output: Range<u32>,
  dimensions: [NonZeroU32; 3],
}

const TILE_WORDS: u32 = 512;

#[derive(PartialEq, Eq)]
pub(super) enum Parallelism {
  Serial,
  Tiled,
}

impl Partition {
  pub(super) fn layout(
    self,
    arena_words: u32,
    groups: [u32; 3],
    workgroup: [u32; 3],
  ) -> io::Result<Layout> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, "invalid compute partition");

    let [wide, tall, deep] = self.dimensions;
    let width = NonZeroU32::new(wide).ok_or_else(invalid)?;
    let height = NonZeroU32::new(tall).ok_or_else(invalid)?;
    let depth = NonZeroU32::new(deep).ok_or_else(invalid)?;
    let dimensions = [width, height, depth];

    for ((workgroup_size, size), dispatched) in workgroup.into_iter().zip(dimensions).zip(groups)
    {
      let group = NonZeroU32::new(workgroup_size).ok_or_else(invalid)?;

      match (size.get().div_ceil(group.get()) == dispatched).then_some(()) {
        Some(()) => {}
        None => return Err(invalid()),
      }
    }

    let words = wide
      .checked_mul(tall)
      .and_then(|area| area.checked_mul(deep))
      .ok_or_else(invalid)?;
    let end = self
      .output_at
      .checked_add(words)
      .filter(|&end| end <= arena_words)
      .ok_or_else(invalid)?;
    Ok(Layout {
      output: self.output_at..end,
      dimensions,
    })
  }
}

impl Layout {
  #[must_use]
  pub(super) fn parallelism(&self) -> Result<Parallelism, Never> {
    let words = self.output.end.saturating_sub(self.output.start);

    Ok(match (words > TILE_WORDS).then_some(()) {
      Some(()) => Parallelism::Tiled,
      None => Parallelism::Serial,
    })
  }

  #[must_use]
  fn coordinate(&self, item: u32) -> Result<[u32; 3], Never> {
    let [width, height, _] = self.dimensions;
    let row = item.wrapping_div(width.get());
    Ok([
      item.wrapping_rem(width.get()),
      row.wrapping_rem(height.get()),
      row.wrapping_div(height.get()),
    ])
  }

  pub(super) fn execute(
    &self,
    arena: &mut [u32],
    params: &[u32; 8],
    invoke: fn(&mut Invocation<'_>),
    cores: Cores,
  ) -> io::Result<()> {
    let invalid = || {
      io::Error::new(
        io::ErrorKind::InvalidInput,
        "compute partition exceeds arena",
      )
    };
    let Ok(start) = index(self.output.start);
    let Ok(length) = index(self.output.end.saturating_sub(self.output.start));
    let (before, tail) = arena.split_at_mut_checked(start).ok_or_else(invalid)?;
    let (output, after) = tail.split_at_mut_checked(length).ok_or_else(invalid)?;
    let run = |first: u32, words: &mut [u32]| {
      for (item, slot) in (first..u32::MAX).zip(words) {
        let Ok(coordinate) = self.coordinate(item);
        let Ok(mut lane) = Invocation::partitioned(
          coordinate,
          params,
          PartitionWords {
            before,
            after,
            position: self.output.start.saturating_add(item),
            end: self.output.end,
            slot,
          },
        );
        invoke(&mut lane);
        let Ok(access) = lane.partition_access();

        match access {
          PartitionAccess::Violated => {
            return Err(io::Error::new(
              io::ErrorKind::InvalidInput,
              format!("compute item {item} accessed another output word"),
            ));
          }
          PartitionAccess::Allowed => {}
        }
      }

      Ok(())
    };

    let Ok(parallelism) = self.parallelism();

    match parallelism {
      Parallelism::Tiled => {}
      Parallelism::Serial => return run(0, output),
    }

    let (blocks, remainder) = output.as_chunks_mut::<512>();
    let words = blocks
      .iter_mut()
      .map(|block| block.as_mut_slice())
      .chain(std::iter::once(remainder))
      .filter(|tile| !tile.is_empty());
    let tiles: Vec<_> = (0..u32::MAX)
      .zip(words)
      .map(|(tile, slots)| (tile.saturating_mul(TILE_WORDS), Mutex::new(slots)))
      .collect();
    let outcomes = concurrency::map(cores, &tiles, |(first, slots)| {
      let mut tile = slots
        .try_lock()
        .map_err(|fault| io::Error::other(format!("could not claim compute tile: {fault}")))?;
      run(*first, &mut tile)
    })
    .map_err(io::Error::other)?;
    outcomes.into_iter().collect()
  }
}

#[cfg(kani)]
#[path = "../../../proofs/compute_partition.rs"]
mod proofs;
