//! Collection effects over a grid of independent items.
//! The compute_map macro declares an independent item that reads input words and
//! returns one output word. A final scalar expression or early return supplies it.
//! Each invocation can read its coordinates and the words relative to the current
//! pass's input. Reads outside that input yield zero. Optional SPIR-V comes from
//! the same kernel's GLSL, and a map can export its output in a linear DMA-BUF.

use std::io;
use std::os::fd::OwnedFd;
use std::path::Path;

use console_core_never::Never;

use super::{Invocation, Kernel, KernelSource, Partition, Program, Target, Vulkan, WordAt};

mod plan;

use plan::Plan;

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "uses the same map body for the Rust item function and the GLSL source and keeps one implementation of each kernel"
  )
)]
#[macro_export]
macro_rules! compute_map {
  ($(#[$attribute:meta])* $visibility:vis static $name:ident = [$x:literal, $y:literal, $z:literal] |$item:ident| $body:block) => {
    $(#[$attribute])*
    $visibility static $name: $crate::program_runtime::compute::MapKernel = {
      let Ok(kernel) = $crate::program_runtime::compute::MapKernel::from_rust(
        [$x, $y, $z],
        |lane| {
          let Ok(()) = $crate::program_runtime::compute::MapKernel::write(lane, |$item| $body);
        },
        $crate::program_runtime::compute::KernelSource {
          binding: stringify!($item),
          body: stringify!($body),
        },
      );
      kernel
    };
  };
}

pub struct MapKernel(Kernel);

impl MapKernel {
  #[doc(hidden)]
  #[must_use]
  pub const fn from_rust(
    workgroup: [u32; 3],
    invoke: fn(&mut Invocation<'_>),
    source: KernelSource,
  ) -> Result<Self, Never> {
    let Ok(kernel) = Kernel::from_rust(workgroup, invoke, source);
    Ok(Self(kernel))
  }

  pub fn glsl(&self) -> io::Result<String> {
    self.0.map_glsl()
  }

  #[doc(hidden)]
  pub fn write(
    lane: &mut Invocation<'_>,
    item: impl FnOnce(MapItem<'_, '_>) -> u32,
  ) -> Result<(), Never> {
    let [Ok(x), Ok(y), Ok(z)] = [lane.id(0), lane.id(1), lane.id(2)];
    let [Ok(width), Ok(height), Ok(depth)] = [lane.param(0), lane.param(1), lane.param(2)];

    match (x < width && y < height && z < depth).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    let row = y.wrapping_add(height.wrapping_mul(z));
    let index = x.wrapping_add(width.wrapping_mul(row));
    let value = item(MapItem { lane, index });
    let Ok(output) = lane.param(5);
    lane.set_word(WordAt(output.wrapping_add(index)), value)
  }
}

pub struct MapItem<'item, 'arena> {
  lane: &'item Invocation<'arena>,
  index: u32,
}

impl MapItem<'_, '_> {
  #[must_use]
  pub fn id(&self, axis: u32) -> Result<u32, Never> {
    self.lane.id(axis)
  }

  #[must_use]
  pub fn extent(&self, axis: u32) -> Result<u32, Never> {
    match axis {
      0..=2 => self.lane.param(axis),
      _ => Ok(0),
    }
  }

  #[must_use]
  pub fn index(&self) -> Result<u32, Never> {
    Ok(self.index)
  }

  #[must_use]
  pub fn input_len(&self) -> Result<u32, Never> {
    self.lane.param(4)
  }

  #[must_use]
  pub fn param(&self, index: u32) -> Result<u32, Never> {
    match index {
      0..=1 => self.lane.param(6_u32.wrapping_add(index)),
      _ => Ok(0),
    }
  }

  #[must_use]
  pub fn word(&self, index: u32) -> Result<u32, Never> {
    let Ok(length) = self.input_len();

    match (index < length).then_some(()) {
      Some(()) => {
        let Ok(base) = self.lane.param(3);
        self.lane.word(base.wrapping_add(index))
      }
      None => Ok(0),
    }
  }
}

#[derive(Clone, Copy)]
pub struct MapProgram<'a> {
  pub kernel: &'a MapKernel,
  pub spirv: Option<&'a Path>,
}

impl<'a> MapProgram<'a> {
  #[must_use]
  fn program(self) -> Result<Program<'a>, Never> {
    Ok(Program {
      kernel: &self.kernel.0,
      spirv: self.spirv,
    })
  }
}

pub struct Map<'a> {
  pub program: MapProgram<'a>,
  pub input: &'a [u32],
  pub dimensions: [u32; 3],
  pub parameters: [u32; 2],
}

#[derive(Clone, Copy)]
pub struct MapStage<'a> {
  pub program: MapProgram<'a>,
  pub parameters: [u32; 2],
}

#[derive(Clone, Copy)]
pub struct Reduction<'a> {
  pub pairs: MapProgram<'a>,
  pub empty: u32,
}

impl<'a> Reduction<'a> {
  #[must_use]
  pub const fn wrapping_sum(spirv: Option<&'a Path>) -> Result<Self, Never> {
    Ok(Self {
      pairs: MapProgram {
        kernel: &WRAPPING_SUM,
        spirv,
      },
      empty: 0,
    })
  }
}

compute_map! {
  pub static WRAPPING_SUM = [64, 1, 1] |item| {
    let Ok(item_index): Result<u32, Never> = item.index();
    let left: u32 = u32::wrapping_mul(2, item_index);
    let Ok(first): Result<u32, Never> = item.word(left);
    let Ok(second): Result<u32, Never> = item.word(u32::wrapping_add(left, 1));
    u32::wrapping_add(first, second)
  }
}

#[derive(Clone, Copy)]
pub struct Scan<'a> {
  pub step: MapProgram<'a>,
}

impl<'a> Scan<'a> {
  pub const fn wrapping_sum(spirv: Option<&'a Path>) -> Result<Self, Never> {
    Ok(Self {
      step: MapProgram {
        kernel: &WRAPPING_PREFIX_SUM,
        spirv,
      },
    })
  }
}

compute_map! {
  pub static WRAPPING_PREFIX_SUM = [64, 1, 1] |item| {
    let Ok(at): Result<u32, Never> = item.index();
    let Ok(stride): Result<u32, Never> = item.param(0);
    let Ok(left): Result<u32, Never> = item.word(u32::wrapping_sub(at, stride));
    let Ok(right): Result<u32, Never> = item.word(at);
    u32::wrapping_add(left, right)
  }
}

pub fn map(target: Target<'_>, work: Map<'_>) -> io::Result<Vec<u32>> {
  map_pipeline(target, work, &[])
}

pub fn map_export(device: &Vulkan, work: Map<'_>) -> io::Result<OwnedFd> {
  let mut plan = Plan::default();
  let input_words = length(work.input)?;
  let output = plan.map(&work, input_words)?;
  plan.export(device, work.input, output)
}

pub fn map_pipeline(
  target: Target<'_>,
  first: Map<'_>,
  following: &[MapStage<'_>],
) -> io::Result<Vec<u32>> {
  let active = count(first.dimensions)?;

  match active {
    0 => return Ok(Vec::new()),
    1..=u32::MAX => {}
  }

  let inputs = length(first.input)?;
  let start = match following {
    [] => inputs,
    [_, ..] => inputs.max(active),
  };
  let mut plan = Plan::default();
  let mut output = plan.map(&first, start)?;
  let arena_words = output.end;
  let mut destination = 0;

  for stage in following {
    let shader = plan.register(stage.program)?;
    let previous = output.start;
    let written = plan.append(
      shader,
      Partition {
        output_at: destination,
        dimensions: first.dimensions,
      },
      output,
      stage.parameters,
    )?;
    output = written;
    destination = previous;
  }

  plan.run(target, first.input, arena_words, output)
}

pub fn map_reduce(
  target: Target<'_>,
  work: Map<'_>,
  reduction: Reduction<'_>,
) -> io::Result<u32> {
  let active = count(work.dimensions)?;

  match active {
    0 => return Ok(reduction.empty),
    1..=u32::MAX => {}
  }

  let inputs = length(work.input)?;
  let mut plan = Plan::default();
  let mapped = plan.map(&work, inputs)?;

  plan.reduce(target, work.input, mapped, reduction)
}

pub fn reduce(target: Target<'_>, input: &[u32], reduction: Reduction<'_>) -> io::Result<u32> {
  match input {
    [] => Ok(reduction.empty),
    [word] => Ok(*word),
    [_, _, ..] => {
      let inputs = length(input)?;
      Plan::default().reduce(target, input, 0..inputs, reduction)
    }
  }
}

pub fn scan(target: Target<'_>, input: &[u32], prefix: Scan<'_>) -> io::Result<Vec<u32>> {
  let inputs = length(input)?;

  match inputs {
    0 | 1 => return Ok(input.to_vec()),
    2..=u32::MAX => {}
  }

  let mut plan = Plan::default();
  let shader = plan.register(prefix.step)?;
  let arena_words = inputs.checked_mul(2).ok_or(InvalidSize)?;
  let mut output = 0..inputs;
  let mut destination = inputs;

  for bit in 0..u32::BITS {
    let stride = 1_u32.wrapping_shl(bit);

    match stride.cmp(&inputs) {
      std::cmp::Ordering::Less => {}
      std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => break,
    }

    let previous = output.start;
    let written = plan.append(
      shader,
      Partition {
        output_at: destination,
        dimensions: [inputs, 1, 1],
      },
      output,
      [stride, 0],
    )?;
    output = written;
    destination = previous;
  }

  plan.run(target, input, arena_words, output)
}

pub fn reject(target: Target<'_>, predicate: Map<'_>) -> io::Result<Vec<u32>> {
  let input = predicate.input;
  let active = count(predicate.dimensions)?;
  let inputs = length(input)?;

  match active.cmp(&inputs) {
    std::cmp::Ordering::Equal => {}
    std::cmp::Ordering::Less | std::cmp::Ordering::Greater => {
      return Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "reject requires one predicate result per input word",
      ));
    }
  }

  let flags = map(target, predicate)?;
  let mut survivors = Vec::new();

  for (&word, flag) in input.iter().zip(flags) {
    match flag {
      0 => survivors.push(word),
      1..=u32::MAX => {}
    }
  }

  Ok(survivors)
}

struct InvalidSize;

impl From<InvalidSize> for io::Error {
  fn from(_: InvalidSize) -> Self {
    Self::new(
      io::ErrorKind::InvalidInput,
      "invalid compute iteration size",
    )
  }
}

fn count(dimensions: [u32; 3]) -> io::Result<u32> {
  match bool::then_some(dimensions.contains(&0), ()) {
    Some(()) => return Ok(0),
    None => {}
  }

  let [width, height, depth] = dimensions;
  let words = width
    .checked_mul(height)
    .and_then(|area| area.checked_mul(depth))
    .ok_or(InvalidSize)?;
  Ok(words)
}

fn length(words: &[u32]) -> io::Result<u32> {
  u32::try_from(words.len()).map_err(|_too_many_words| io::Error::from(InvalidSize))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::program_runtime::compute::Vulkan;

  compute_map! {
    static ADD = [4, 4, 2] |item| {
      let Ok(at): Result<u32, Never> = item.index();
      let Ok(length): Result<u32, Never> = item.input_len();

      match (at >= length).then_some(()) {
        Some(()) => {
          let Ok(fallback): Result<u32, Never> = item.param(1);
          return fallback;
        }
        None => {}
      }

      let Ok(word): Result<u32, Never> = item.word(at);
      let Ok(width): Result<u32, Never> = item.extent(0);
      let Ok(offset): Result<u32, Never> = item.param(0);
      u32::wrapping_add(word, u32::wrapping_add(width, offset))
    }
  }

  compute_map! {
    static COPY = [4, 1, 1] |item| {
      let Ok(at): Result<u32, Never> = item.index();
      let Ok(word): Result<u32, Never> = item.word(at);
      word
    }
  }

  compute_map! {
    static SCALE = [4, 2, 1] |item| {
      let Ok(at): Result<u32, Never> = item.index();
      let Ok(word): Result<u32, Never> = item.word(at);
      let Ok(factor): Result<u32, Never> = item.param(0);
      u32::wrapping_mul(word, factor)
    }
  }

  compute_map! {
    static BITS_PRESENT = [4, 1, 1] |item| {
      let Ok(at): Result<u32, Never> = item.index();
      let Ok(word): Result<u32, Never> = item.word(at);
      let Ok(mask): Result<u32, Never> = item.param(0);
      word & mask
    }
  }

  compute_map! {
    static FIRST = [4, 1, 1] |item| {
      let Ok(at): Result<u32, Never> = item.index();
      let Ok(stride): Result<u32, Never> = item.param(0);

      match (at < stride).then_some(()) {
        Some(()) => {
          let Ok(current): Result<u32, Never> = item.word(at);
          return current;
        }
        None => {}
      }

      let Ok(previous): Result<u32, Never> = item.word(u32::wrapping_sub(at, stride));
      previous
    }
  }

  compute_map! {
    static DIFFERENCE = [2, 1, 1] |item| {
      let Ok(at): Result<u32, Never> = item.index();
      let left: u32 = u32::wrapping_mul(at, 2);
      let Ok(value): Result<u32, Never> = item.word(left);
      let right: u32 = u32::wrapping_add(left, 1);
      let Ok(length): Result<u32, Never> = item.input_len();

      match (right == length).then_some(()) {
        Some(()) => { return value; }
        None => {}
      }

      let Ok(subtrahend): Result<u32, Never> = item.word(right);
      u32::wrapping_sub(value, subtrahend)
    }
  }

  #[test]
  fn map_orders_a_padded_grid_and_publishes_each_returned_word() -> io::Result<()> {
    let input: Vec<u32> = (0..::core::primitive::u32::wrapping_sub(
      ::core::primitive::u32::wrapping_mul(::core::primitive::u32::wrapping_mul(17, 9), 5),
      1,
    ))
      .collect();
    let work = || Map {
      program: MapProgram {
        kernel: &ADD,
        spirv: None,
      },
      input: &input,
      dimensions: [17, 9, 5],
      parameters: [7, 1234],
    };
    let mut expected: Vec<_> = input.iter().map(|value| value.wrapping_add(24)).collect();
    expected.push(1234);

    for target in [Target::SerialCpu, Target::Cpu] {
      let mapped = map(target, work())?;
      assert_eq!(mapped, expected);
    }

    let shader = ADD.glsl()?;
    assert!(shader.contains("uint fallback = console_map_param(1);"));
    assert!(shader.contains("return fallback;"));
    assert!(shader.contains("if ((at >= length))"));
    assert!(shader.contains("greaterThanEqual(gl_GlobalInvocationID"));
    Ok(())
  }

  #[test]
  fn reduction_preserves_pair_order_odd_tails_and_empty_results() -> io::Result<()> {
    let work = |input, width| Map {
      program: MapProgram {
        kernel: &COPY,
        spirv: None,
      },
      input,
      dimensions: [width, 1, 1],
      parameters: [0; 2],
    };
    let difference = Reduction {
      pairs: MapProgram {
        kernel: &DIFFERENCE,
        spirv: None,
      },
      empty: 17,
    };
    let inputs = [100, 10, 11, 3, 7];

    for target in [Target::SerialCpu, Target::Cpu] {
      let reduced = map_reduce(target, work(&inputs, 5), difference)?;
      assert_eq!(reduced, 75);
    }

    let singleton = map_reduce(Target::Cpu, work(&inputs, 1), difference)?;
    assert_eq!(singleton, 100);
    let reduced = reduce(Target::Cpu, &inputs, difference)?;
    assert_eq!(reduced, 75);
    let Ok(wrapping_sum) = Reduction::wrapping_sum(None);
    let wrapping = map_reduce(Target::Cpu, work(&[u32::MAX, 2, 3, 4, 5], 5), wrapping_sum)?;
    assert_eq!(wrapping, 13);
    let Ok(absent) = Vulkan::new(<::std::path::PathBuf as ::core::convert::From<
      &::core::primitive::str,
    >>::from("unused-worker"));
    let gpu = Target::RequireVulkan(&absent);
    let no_items = map(gpu, work(&inputs, 0))?;
    let initial = map_reduce(gpu, work(&inputs, 0), difference)?;
    assert!(no_items.is_empty());
    assert_eq!(initial, 17);
    let empty = reduce(gpu, &[], difference)?;
    let single = reduce(gpu, &[100], difference)?;
    assert_eq!(empty, 17);
    assert_eq!(single, 100);
    Ok(())
  }

  #[test]
  fn a_pipeline_passes_the_full_grid_to_each_successive_stage() -> io::Result<()> {
    let add = MapProgram {
      kernel: &ADD,
      spirv: None,
    };
    let scale = MapProgram {
      kernel: &SCALE,
      spirv: None,
    };

    for target in [Target::SerialCpu, Target::Cpu] {
      let result = map_pipeline(
        target,
        Map {
          program: add,
          input: &[1, 2, 3],
          dimensions: [3, 2, 1],
          parameters: [1, 9],
        },
        &[
          MapStage {
            program: scale,
            parameters: [2, 0],
          },
          MapStage {
            program: add,
            parameters: [0, 0],
          },
        ],
      )?;
      assert_eq!(result, [13, 15, 17, 21, 21, 21]);
    }

    Ok(())
  }

  #[test]
  fn a_pipeline_can_reuse_a_shader_up_to_the_pass_limit() -> io::Result<()> {
    let program = MapProgram {
      kernel: &COPY,
      spirv: None,
    };
    let work = || Map {
      program,
      input: &[23],
      dimensions: [1, 1, 1],
      parameters: [0; 2],
    };
    let copy = MapStage {
      program,
      parameters: [0; 2],
    };
    let result = map_pipeline(Target::Cpu, work(), &[copy; 63])?;
    assert_eq!(result, [23]);
    assert!(
      matches!(map_pipeline(Target::Cpu, work(), &[copy; 64]), Err(fault) if fault.kind() == io::ErrorKind::InvalidInput)
    );

    Ok(())
  }

  #[test]
  fn scan_matches_inclusive_prefixes_including_a_partial_workgroup() -> io::Result<()> {
    let Ok(prefix) = Scan::wrapping_sum(None);
    let many = vec![u32::MAX; 1025];
    let inputs: [&[u32]; 4] = [&[], &[17], &[u32::MAX, 2, 3, 4, 5], &many];

    for words in inputs {
      let expected: Vec<_> = words
        .iter()
        .scan(0_u32, |sum, &word| {
          *sum = sum.wrapping_add(word);
          Some(*sum)
        })
        .collect();

      for target in [Target::SerialCpu, Target::Cpu] {
        let result = scan(target, words, prefix)?;
        assert_eq!(result, expected);
      }
    }

    let leftmost = Scan {
      step: MapProgram {
        kernel: &FIRST,
        spirv: None,
      },
    };
    let result = scan(Target::Cpu, &[3, 8, 4, 7, 5], leftmost)?;
    assert_eq!(result, [3; 5]);
    let shader = WRAPPING_PREFIX_SUM.glsl()?;
    assert!(shader.contains("console_map_param(0)"));

    Ok(())
  }

  #[test]
  fn reject_keeps_the_original_order_and_requires_one_flag_per_word() -> io::Result<()> {
    let input = [8, 3, 8, 0, 4, 7];
    let work = |dimensions, mask| Map {
      program: MapProgram {
        kernel: &BITS_PRESENT,
        spirv: None,
      },
      input: &input,
      dimensions,
      parameters: [mask, 0],
    };

    for target in [Target::SerialCpu, Target::Cpu] {
      let result = reject(target, work([3, 2, 1], 1))?;
      assert_eq!(result, [8, 8, 0, 4]);
    }

    let all = reject(Target::Cpu, work([6, 1, 1], 0))?;
    let none = reject(
      Target::Cpu,
      Map {
        input: &[3, 7, 5],
        ..work([3, 1, 1], 1)
      },
    )?;
    let empty = reject(
      Target::Cpu,
      Map {
        input: &[],
        ..work([0, 1, 1], 1)
      },
    )?;
    assert_eq!(all, input);
    assert!(none.is_empty());
    assert!(empty.is_empty());
    assert!(
      matches!(reject(Target::Cpu, work([5, 1, 1], 1)), Err(fault) if fault.kind() == io::ErrorKind::InvalidInput)
    );

    Ok(())
  }

  #[test]
  fn an_overflowed_iteration_shape_is_rejected() {
    let work = Map {
      program: MapProgram {
        kernel: &COPY,
        spirv: None,
      },
      input: &[],
      dimensions: [u32::MAX, 2, 1],
      parameters: [0; 2],
    };
    assert_eq!(
      map(Target::Cpu, work).unwrap_err().kind(),
      io::ErrorKind::InvalidInput
    );
  }
}
