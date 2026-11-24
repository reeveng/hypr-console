//! Compute OpenType table checksums with a selected backend and emit the
//! matching block-sum kernel for Vulkan.

use std::io;
use std::path::Path;

use console_core_never::Never;
use console_operating_system_linux::compute_map;
use console_operating_system_linux::program_runtime::compute::{
  self, Map, MapProgram, Reduction, Target,
};

pub fn compute_checksum_with_compute(
  table: &[u8],
  target: Target<'_>,
  words_shader: Option<&Path>,
  sum_shader: Option<&Path>,
) -> io::Result<u32> {
  match target {
    Target::PreferVulkan(device) => compute::run(
      Some(device),
      |device_2| {
        calculate(
          table,
          Target::RequireVulkan(device_2),
          words_shader,
          sum_shader,
        )
      },
      || Ok(super::compute_checksum(table)),
    ),
    Target::Cpu | Target::SerialCpu | Target::RequireVulkan(_) => {
      calculate(table, target, words_shader, sum_shader)
    }
  }
}

fn calculate(
  table: &[u8],
  target: Target<'_>,
  words_shader: Option<&Path>,
  sum_shader: Option<&Path>,
) -> io::Result<u32> {
  let count = u32::try_from(table.len().div_ceil(4)).map_err(|_| {
    io::Error::new(
      io::ErrorKind::InvalidInput,
      "OpenType table exceeds compute index range",
    )
  })?;
  let Ok(words) = console_core_checksums::opentype::words(table);
  let input: Vec<_> = words.collect();
  let Ok(reduction) = Reduction::wrapping_sum(sum_shader);
  compute::map_reduce(
    target,
    Map {
      program: MapProgram {
        kernel: &TABLE_SUMS,
        spirv: words_shader,
      },
      input: &input,
      dimensions: [count.div_ceil(64), 1, 1],
      parameters: [0; 2],
    },
    reduction,
  )
}

pub fn checksum_glsl() -> io::Result<String> {
  TABLE_SUMS.glsl()
}

compute_map! {
  static TABLE_SUMS = [64, 1, 1] |item| {
    let Ok(block): Result<u32, Never> = item.index();
    let start: u32 = u32::wrapping_mul(block, 64);
    let mut sum: u32 = 0;

    for offset in 0u32..64 {
      let Ok(word): Result<u32, Never> = item.word(u32::wrapping_add(start, offset));
      sum = u32::wrapping_add(sum, word);
    }

    sum
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn matches_the_font_reader_across_word_and_block_boundaries() -> io::Result<()> {
    let data: Vec<_> = (0u32..515)
      .map(|index| {
        let [byte, ..] = index.wrapping_mul(171).to_le_bytes();
        byte
      })
      .collect();

    for length in [0, 1, 2, 3, 4, 255, 256, 257, 511, 512, 513, 515] {
      let table = data
        .get(..length)
        .ok_or_else(|| io::Error::other("missing test table"))?;

      for target in [Target::SerialCpu, Target::Cpu] {
        let computed = compute_checksum_with_compute(table, target, None, None)?;
        assert_eq!(computed, super::super::compute_checksum(table));
      }
    }

    Ok(())
  }
}
