//! Host comparison for the optional Vulkan backend. Run with `just kernel-gpu-checksums`.

use std::ffi::OsStr;
use std::io::{self, Write};
use std::process::{Command, Stdio};

use console_kernel::compute::{
  BLOCK_BYTES, Block, MAX_BLOCKS, ReductionPlan, ReductionStep, checksum_sum, checksums,
};

fn input(count: usize) -> Vec<Block> {
  let mut seed = 0x7a11_5039_u32;

  (0..count)
    .map(|index| match index % 5 {
      0 => [0; BLOCK_BYTES],
      1 => [255; BLOCK_BYTES],
      _ => core::array::from_fn(|_| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;

        seed.to_le_bytes()[0]
      }),
    })
    .collect()
}

fn dispatch(
  backend: &OsStr,
  shader: &OsStr,
  blocks: &[Block],
  reduction: Option<ReductionPlan>,
) -> io::Result<Vec<u8>> {
  let mut command = Command::new(backend);
  command.arg(shader).arg(blocks.len().to_string());
  if let Some(plan) = reduction {
    command.arg("--reduce");
    for step in plan {
      match step {
        ReductionStep::Join { inputs } => {
          command.arg(inputs.to_string());
        }
        ReductionStep::Publish => break,
      }
    }
  }

  let mut child = command
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()?;
  let sent = child
    .stdin
    .take()
    .ok_or_else(|| io::Error::other("the Vulkan backend has no input pipe"))
    .and_then(|mut pipe| pipe.write_all(blocks.as_flattened()));
  let response = child.wait_with_output()?;
  if !response.status.success() {
    return Err(io::Error::other(format!(
      "the Vulkan backend exited with {}",
      response.status
    )));
  }
  sent?;
  Ok(response.stdout)
}

fn compare(backend: &OsStr, shader: &OsStr, count: usize) -> io::Result<()> {
  let blocks = input(count);
  let mut expected = vec![0; count];
  checksums(&blocks, &mut expected).map_err(|error| io::Error::other(format!("{error:?}")))?;

  let ordinary = dispatch(backend, shader, &blocks, None)?;
  let (results, trailing) = ordinary.as_chunks::<4>();
  if !trailing.is_empty() || results.len() != expected.len() {
    return Err(io::Error::other(format!(
      "incorrect Vulkan output length for {count} blocks"
    )));
  }
  for (index, (bytes, &cpu)) in results.iter().zip(&expected).enumerate() {
    let gpu = u32::from_le_bytes(*bytes);
    if gpu != cpu {
      return Err(io::Error::other(format!(
        "block {index} of {count} produced CPU {cpu:08x} and GPU {gpu:08x}"
      )));
    }
  }

  let serial_sum = expected.iter().copied().fold(0, u32::wrapping_add);
  let cpu_tree = checksum_sum(&blocks, &mut expected)
    .map_err(|error| io::Error::other(format!("{error:?}")))?;
  let plan = ReductionPlan::new(count).map_err(|error| io::Error::other(format!("{error:?}")))?;
  let reduced = dispatch(backend, shader, &blocks, Some(plan))?;
  if cpu_tree != serial_sum || reduced.as_slice() != serial_sum.to_le_bytes() {
    return Err(io::Error::other(format!(
      "reduction of {count} blocks produced serial {serial_sum:08x}, CPU tree {cpu_tree:08x} and Vulkan {reduced:02x?}"
    )));
  }

  println!("CPU and Vulkan matched for {count} blocks and their reduction {serial_sum:08x}");
  Ok(())
}

fn main() -> io::Result<()> {
  let arguments: Vec<_> = std::env::args_os().skip(1).collect();
  let [backend, shader] = arguments.as_slice() else {
    return Err(io::Error::other(
      "usage: gpu-checksums VULKAN_BACKEND SHADER.spv",
    ));
  };

  for count in [0, 1, 2, 3, 63, 64, 65, 127, 128, 129, 4097, MAX_BLOCKS] {
    compare(backend, shader, count)?;
  }

  Ok(())
}
