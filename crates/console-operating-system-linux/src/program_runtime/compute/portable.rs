//! Portable kernel declarations and their serial or partitioned CPU dispatch.
//!
//! The compute_kernel macro declares a Rust function whose body can also be
//! lowered to GLSL. The body uses typed scalar locals, explicit wrapping integer
//! operations and floating-point arithmetic, assignments, conditionals, loops,
//! and early return. A `for` loop takes a half-open range starting at a `u32`
//! literal. The invocation provides coordinates, parameters and shared
//! storage. Work items in a pass must be independent. A later dispatch consumes
//! their results.
//! A program's optional SPIR-V is compiled from its kernel's GLSL. Partitioned
//! CPU passes give each item exclusive access to its output word and supply the
//! real grid and output start. Sufficiently large partitions use multiple cores.

use std::cell::Cell;
use std::io;
use std::path::Path;

use console_concurrency::Cores;
use console_core_never::Never;
use console_core_number_conversion::index;

use super::{
  Batch, MAX_WORDS, Partition, Vulkan,
  partition::{Layout, Parallelism},
  shader,
};

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "uses the same kernel body for the Rust invocation and the GLSL source so the CPU and GPU implementations cannot drift apart"
  )
)]
#[macro_export]
macro_rules! compute_kernel {
  ($visibility:vis static $name:ident = [$x:literal, $y:literal, $z:literal] |$item:ident| $body:block) => {
    $visibility static $name: $crate::program_runtime::compute::Kernel = {
      let Ok(kernel) = $crate::program_runtime::compute::Kernel::from_rust(
        [$x, $y, $z],
        |$item| $body,
        $crate::program_runtime::compute::KernelSource {
          binding: stringify!($item),
          body: stringify!($body),
        },
      );
      kernel
    };
  };
}

#[doc(hidden)]
pub struct KernelSource {
  pub binding: &'static str,
  pub body: &'static str,
}

pub struct Kernel {
  workgroup: [u32; 3],
  invoke: fn(&mut Invocation<'_>),
  source: KernelSource,
}

impl Kernel {
  #[doc(hidden)]
  #[must_use]
  pub const fn from_rust(
    workgroup: [u32; 3],
    invoke: fn(&mut Invocation<'_>),
    source: KernelSource,
  ) -> Result<Self, Never> {
    Ok(Self {
      workgroup,
      invoke,
      source,
    })
  }

  pub fn glsl(&self) -> io::Result<String> {
    shader::glsl(
      self.workgroup,
      shader::Source {
        binding: self.source.binding,
        body: self.source.body,
      },
      shader::Dialect::Batch,
    )
  }

  pub(super) fn map_glsl(&self) -> io::Result<String> {
    shader::glsl(
      self.workgroup,
      shader::Source {
        binding: self.source.binding,
        body: self.source.body,
      },
      shader::Dialect::Map,
    )
  }

  #[must_use]
  pub const fn workgroup(&self) -> Result<[u32; 3], Never> {
    Ok(self.workgroup)
  }
}

pub struct Invocation<'a> {
  grid: [u32; 3],
  params: &'a [u32; 8],
  storage: Storage<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WordAt(pub u32);

enum Storage<'a> {
  Whole(&'a mut [u32]),
  Partitioned {
    words: PartitionWords<'a>,
    access: Cell<PartitionAccess>,
  },
}

pub(super) struct PartitionWords<'a> {
  pub(super) before: &'a [u32],
  pub(super) after: &'a [u32],
  pub(super) position: u32,
  pub(super) end: u32,
  pub(super) slot: &'a mut u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PartitionAccess {
  Allowed,
  Violated,
}

impl<'a> Invocation<'a> {
  #[must_use]
  pub(super) fn partitioned(
    grid: [u32; 3],
    params: &'a [u32; 8],
    words: PartitionWords<'a>,
  ) -> Result<Self, Never> {
    Ok(Self {
      grid,
      params,
      storage: Storage::Partitioned {
        words,
        access: Cell::new(PartitionAccess::Allowed),
      },
    })
  }

  #[must_use]
  pub(super) fn partition_access(&self) -> Result<PartitionAccess, Never> {
    Ok(match &self.storage {
      Storage::Whole(_) => PartitionAccess::Allowed,
      Storage::Partitioned { access, .. } => access.get(),
    })
  }

  #[must_use]
  pub fn id(&self, axis: u32) -> Result<u32, Never> {
    let Ok(native) = index(axis);
    Ok(match self.grid.get(native) {
      Some(coordinate) => *coordinate,
      None => 0,
    })
  }

  #[must_use]
  pub fn param(&self, at: u32) -> Result<u32, Never> {
    let Ok(native) = index(at);
    Ok(match self.params.get(native) {
      Some(parameter) => *parameter,
      None => 0,
    })
  }

  #[must_use]
  pub fn word(&self, at: u32) -> Result<u32, Never> {
    let Ok(native) = index(at);

    Ok(match &self.storage {
      Storage::Whole(words) => match words.get(native) {
        Some(word) => *word,
        None => 0,
      },
      Storage::Partitioned { words, access } => {
        match words.before.get(native) {
          Some(word) => return Ok(*word),
          None => {}
        }

        match at.checked_sub(words.end) {
          Some(relative) => {
            let Ok(after) = index(relative);
            return Ok(match words.after.get(after) {
              Some(word) => *word,
              None => 0,
            });
          }
          None => {}
        }

        match (at == words.position).then_some(()) {
          Some(()) => *words.slot,
          None => {
            access.set(PartitionAccess::Violated);
            0
          }
        }
      }
    })
  }

  pub fn set_word(&mut self, WordAt(at): WordAt, value: u32) -> Result<(), Never> {
    match &mut self.storage {
      Storage::Whole(words) => {
        let Ok(native) = index(at);

        match words.get_mut(native) {
          Some(slot) => {
            *slot = value;
          }
          None => {}
        }
      }
      Storage::Partitioned { words, access } => match (at == words.position).then_some(()) {
        Some(()) => *words.slot = value,
        None => access.set(PartitionAccess::Violated),
      },
    }

    Ok(())
  }
}

#[derive(Clone, Copy)]
pub struct Program<'a> {
  pub kernel: &'a Kernel,
  pub spirv: Option<&'a Path>,
}

#[derive(Clone, Copy)]
pub enum Target<'a> {
  Cpu,
  SerialCpu,
  PreferVulkan(&'a Vulkan),
  RequireVulkan(&'a Vulkan),
}

pub fn dispatch(target: Target<'_>, batch: Batch<'_, Program<'_>>) -> io::Result<Vec<u32>> {
  dispatch_with(target, batch, None)
}

pub fn dispatch_partitioned(
  target: Target<'_>,
  batch: Batch<'_, Program<'_>>,
  partitions: &[Partition],
) -> io::Result<Vec<u32>> {
  dispatch_with(target, batch, Some(partitions))
}

fn dispatch_with(
  target: Target<'_>,
  batch: Batch<'_, Program<'_>>,
  partitions: Option<&[Partition]>,
) -> io::Result<Vec<u32>> {
  let _ = batch.header()?;
  let grids = batch
    .passes
    .iter()
    .map(|pass| {
      let kernel = kernel_for(batch.shaders, pass.shader)?;
      let [groups_x, groups_y, groups_z] = pass.groups;
      let [local_x, local_y, local_z] = kernel.workgroup;
      let [wide, tall, deep] = [
        groups_x.checked_mul(local_x),
        groups_y.checked_mul(local_y),
        groups_z.checked_mul(local_z),
      ];

      match (wide, tall, deep) {
        (Some(width @ 1..), Some(height @ 1..), Some(depth @ 1..)) => Ok([width, height, depth]),
        _ => Err(io::Error::from(InvalidGrid)),
      }
    })
    .collect::<io::Result<Vec<_>>>()?;

  let layouts = match partitions {
    Some(declared) => {
      match (declared.len() == batch.passes.len()).then_some(()) {
        Some(()) => {}
        None => {
          return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "partition count differs from compute pass count",
          ));
        }
      }

      let layouts = declared
        .iter()
        .zip(batch.passes)
        .map(|(&partition, pass)| {
          let kernel = kernel_for(batch.shaders, pass.shader)?;
          partition.layout(batch.arena_words, pass.groups, kernel.workgroup)
        })
        .collect::<io::Result<Vec<_>>>()?;
      layouts
    }
    None => Vec::new(),
  };

  let cpu = || on_cpu(&batch, &grids, &layouts);
  let gpu = |device: &Vulkan| {
    let modules = batch
      .shaders
      .iter()
      .map(|program| {
        program
          .spirv
          .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "portable kernel has no SPIR-V"))
      })
      .collect::<io::Result<Vec<_>>>()?;
    device.dispatch_programs(&batch, modules)
  };

  match target {
    Target::Cpu => cpu(),
    Target::SerialCpu => on_cpu(&batch, &grids, &[]),
    Target::PreferVulkan(device) => super::run(Some(device), gpu, cpu),
    Target::RequireVulkan(device) => gpu(device),
  }
}

struct InvalidGrid;

impl From<InvalidGrid> for io::Error {
  fn from(_: InvalidGrid) -> Self {
    Self::new(io::ErrorKind::InvalidInput, "invalid portable kernel grid")
  }
}

fn kernel_for<'a>(programs: &[Program<'a>], shader: u32) -> io::Result<&'a Kernel> {
  let Ok(at) = index(shader);

  programs
    .get(at)
    .map(|program| program.kernel)
    .ok_or_else(|| io::Error::from(InvalidGrid))
}

fn on_cpu(
  batch: &Batch<'_, Program<'_>>,
  grids: &[[u32; 3]],
  layouts: &[Layout],
) -> io::Result<Vec<u32>> {
  let Ok(length) = index(batch.arena_words.min(MAX_WORDS));
  let mut words = Vec::new();
  words
    .try_reserve_exact(length.min(16_777_216))
    .map_err(io::Error::other)?;
  words.extend_from_slice(batch.input);
  words.resize(words.capacity().min(length), 0);
  let tiled = layouts
    .iter()
    .any(|layout| layout.parallelism() == Ok(Parallelism::Tiled));
  let Ok(cores) = match tiled.then_some(()) {
    Some(()) => Cores::counted(),
    None => Cores::one(),
  };

  for (step, (pass, &[width, height, depth])) in batch.passes.iter().zip(grids).enumerate() {
    let kernel = kernel_for(batch.shaders, pass.shader)?;

    match layouts.get(step) {
      Some(layout) => {
        layout.execute(&mut words, &pass.push, kernel.invoke, cores)?;
        continue;
      }
      ::core::option::Option::None => {}
    }

    for z in 0..depth {
      for y in 0..height {
        for x in 0..width {
          (kernel.invoke)(&mut Invocation {
            grid: [x, y, z],
            params: &pass.push,
            storage: Storage::Whole(&mut words),
          });
        }
      }
    }
  }

  let Ok(start) = index(batch.output.start);
  let Ok(end) = index(batch.output.end);
  let wanted = words.get(start..end).ok_or(InvalidGrid)?;

  Ok(wanted.to_vec())
}

#[cfg(test)]
mod tests {
  use super::*;

  compute_kernel! {
    static SCALE = [2, 1, 1] |lane| {
      let Ok(layer): Result<u32, Never> = lane.id(2);
      let Ok(column): Result<u32, Never> = lane.id(0);
      let index: u32 = u32::wrapping_add(u32::wrapping_mul(2, layer), column);
      let Ok(word): Result<u32, Never> = lane.word(index);
      let mut value: f32 = f32::from_bits(word) * 2.5e-1f32;

      match (index & 1 == 0).then_some(()) {
        Some(()) => { value = -value; }
        None => {}
      }

      let Ok(()) = lane.set_word(WordAt(u32::wrapping_add(index, 4)), f32::to_bits(value));
    }
  }

  compute_kernel! {
    static COPY_GRID = [4, 4, 2] |lane| {
      let Ok(x): Result<u32, Never> = lane.id(0);
      let Ok(y): Result<u32, Never> = lane.id(1);
      let Ok(z): Result<u32, Never> = lane.id(2);
      let Ok(width): Result<u32, Never> = lane.param(0);
      let Ok(height): Result<u32, Never> = lane.param(1);
      let Ok(depth): Result<u32, Never> = lane.param(2);

      match (x >= width || y >= height || z >= depth).then_some(()) {
        Some(()) => { return; }
        None => {}
      }

      let row: u32 = u32::wrapping_add(y, u32::wrapping_mul(height, z));
      let item: u32 = u32::wrapping_add(x, u32::wrapping_mul(width, row));
      let Ok(write_base): Result<u32, Never> = lane.param(3);
      let Ok(word): Result<u32, Never> = lane.word(item);
      let Ok(()) = lane.set_word(WordAt(u32::wrapping_add(write_base, item)), word);
    }
  }

  #[test]
  fn cpu_partitions_cover_a_padded_three_dimensional_grid() -> io::Result<()> {
    let input: Vec<_> =
      (0..::core::primitive::u32::wrapping_mul(::core::primitive::u32::wrapping_mul(17, 17), 3))
        .collect();
    let words = u32::try_from(input.len()).map_err(io::Error::other)?;
    let output_end = words.saturating_mul(2);
    let shaders = [Program {
      kernel: &COPY_GRID,
      spirv: None,
    }];
    let batch = || Batch {
      shaders: &shaders,
      passes: &[super::super::Pass {
        shader: 0,
        groups: [5, 5, 2],
        push: [17, 17, 3, 867, 0, 0, 0, 0],
      }],
      input: &input,
      arena_words: output_end.saturating_add(1),
      output: words..output_end,
    };
    let mut partition = Partition {
      output_at: words,
      dimensions: [17, 17, 3],
    };

    for target in [Target::Cpu, Target::SerialCpu] {
      let dispatched = dispatch_partitioned(target, batch(), &[partition])?;
      assert_eq!(dispatched, input);
    }

    partition.output_at = partition.output_at.saturating_add(1);
    assert_eq!(
      dispatch_partitioned(Target::Cpu, batch(), &[partition])
        .unwrap_err()
        .kind(),
      io::ErrorKind::InvalidInput
    );
    Ok(())
  }

  #[test]
  fn portable_float_kernel_shares_the_grid_and_rust_expression_order() -> io::Result<()> {
    let glsl = SCALE.glsl()?;
    assert!(glsl.contains("((index & 1) == 0)"));
    assert!(glsl.contains("uint word = console_word(index);"));
    assert!(glsl.contains("precise float value = (uintBitsToFloat(word) * 2.5e-1)"));
    let input = [4.0f32, 8.0, 12.0, 16.0].map(f32::to_bits);
    let shaders = [Program {
      kernel: &SCALE,
      spirv: None,
    }];
    let batch = || Batch {
      shaders: &shaders,
      passes: &[super::super::Pass {
        shader: 0,
        groups: [1, 1, 2],
        push: [0; 8],
      }],
      input: &input,
      arena_words: 8,
      output: 4..8,
    };
    let expected = [-1.0f32, 2.0, -3.0, 4.0].map(f32::to_bits);
    let Ok(unavailable) = Vulkan::new(<::std::path::PathBuf as ::core::convert::From<
      &::core::primitive::str,
    >>::from("unused-worker"));

    for target in [Target::Cpu, Target::PreferVulkan(&unavailable)] {
      let dispatched = dispatch(target, batch())?;
      assert_eq!(dispatched, expected);
    }

    assert_eq!(
      dispatch(Target::RequireVulkan(&unavailable), batch())
        .unwrap_err()
        .kind(),
      io::ErrorKind::NotFound
    );
    Ok(())
  }
}
