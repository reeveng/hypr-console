use std::io;
use std::ops::Range;
use std::os::fd::OwnedFd;

use console_core_number_conversion::index;

use super::{InvalidSize, Map, MapProgram, Partition, Program, Reduction, Target, count, length};
use crate::program_runtime::compute::{
  Batch, MAX_PASSES, MAX_SHADERS, Pass, Vulkan, dispatch_partitioned,
};

#[derive(Default)]
pub(super) struct Plan<'a> {
  programs: Vec<Program<'a>>,
  passes: Vec<Pass>,
  partitions: Vec<Partition>,
}

impl<'a> Plan<'a> {
  pub(super) fn export(
    &self,
    device: &Vulkan,
    input: &[u32],
    output: Range<u32>,
  ) -> io::Result<OwnedFd> {
    let shaders = self
      .programs
      .iter()
      .map(|program| {
        program
          .spirv
          .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "portable kernel has no SPIR-V"))
      })
      .collect::<io::Result<Vec<_>>>()?;
    device.export(Batch {
      shaders: &shaders,
      passes: &self.passes,
      input,
      arena_words: output.end,
      output,
    })
  }

  pub(super) fn register(&mut self, map: MapProgram<'a>) -> io::Result<u32> {
    let Ok(program) = map.program();
    let found = (0..MAX_SHADERS)
      .zip(&self.programs)
      .find_map(|(shader, registered)| {
        bool::then_some(
          std::ptr::eq(registered.kernel, program.kernel) && registered.spirv == program.spirv,
          shader,
        )
      });

    match found {
      Some(shader) => return Ok(shader),
      None => {}
    }

    let shader = u32::try_from(self.programs.len()).map_err(|_too_many_shaders| InvalidSize)?;

    match shader.cmp(&MAX_SHADERS) {
      std::cmp::Ordering::Less => self.programs.push(program),
      std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => {
        return Err(io::Error::from(InvalidSize));
      }
    }

    Ok(shader)
  }

  pub(super) fn map(&mut self, work: &Map<'a>, output_at: u32) -> io::Result<Range<u32>> {
    let shader = self.register(work.program)?;
    let inputs = length(work.input)?;
    self.append(
      shader,
      Partition {
        output_at,
        dimensions: work.dimensions,
      },
      0..inputs,
      work.parameters,
    )
  }

  pub(super) fn append(
    &mut self,
    shader: u32,
    partition: Partition,
    input: Range<u32>,
    parameters: [u32; 2],
  ) -> io::Result<Range<u32>> {
    let Ok(limit) = index(MAX_PASSES);

    match self.passes.len().cmp(&limit) {
      std::cmp::Ordering::Less => {}
      std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => {
        return Err(io::Error::from(InvalidSize));
      }
    }

    let Ok(shader_index) = index(shader);
    let program = self.programs.get(shader_index).ok_or(InvalidSize)?;
    let Ok(workgroup) = program.kernel.workgroup();

    match bool::then_some(workgroup.contains(&0), ()) {
      Some(()) => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidInput,
          "invalid map workgroup",
        ));
      }
      None => {}
    }

    let output_words = count(partition.dimensions)?;
    let end = partition
      .output_at
      .checked_add(output_words)
      .ok_or(InvalidSize)?;
    let input_words = input.end.checked_sub(input.start).ok_or(InvalidSize)?;
    let [x, y, z] = partition.dimensions;
    let [tile_x, tile_y, tile_z] = workgroup;
    let [first, second] = parameters;
    self.passes.push(Pass {
      shader,
      groups: [x.div_ceil(tile_x), y.div_ceil(tile_y), z.div_ceil(tile_z)],
      push: [
        x,
        y,
        z,
        input.start,
        input_words,
        partition.output_at,
        first,
        second,
      ],
    });
    self.partitions.push(partition);

    Ok(partition.output_at..end)
  }

  pub(super) fn reduce(
    mut self,
    target: Target<'_>,
    input: &[u32],
    mut output: Range<u32>,
    reduction: Reduction<'a>,
  ) -> io::Result<u32> {
    let mut active = output.end.checked_sub(output.start).ok_or(InvalidSize)?;
    let mut arena_words = output.end;

    match active {
      0 => return Ok(reduction.empty),
      1 => {}
      2..=u32::MAX => {
        let shader = self.register(reduction.pairs)?;
        let mut destination = output.end;
        let scratch_end = destination
          .checked_add(active.div_ceil(2))
          .ok_or(InvalidSize)?;
        arena_words = scratch_end;

        for _ in 0..u32::BITS {
          match active {
            0 | 1 => break,
            2..=u32::MAX => {}
          }

          let parents = active.div_ceil(2);
          let previous = output.start;
          let written = self.append(
            shader,
            Partition {
              output_at: destination,
              dimensions: [parents, 1, 1],
            },
            output,
            [0; 2],
          )?;
          output = written;
          destination = previous;
          active = parents;
        }
      }
    }

    let words = self.run(target, input, arena_words, output)?;

    match words.as_slice() {
      [word] => Ok(*word),
      [] | [_, _, ..] => Err(io::Error::other(
        "compute reduction did not return one word",
      )),
    }
  }

  pub(super) fn run(
    self,
    target: Target<'_>,
    input: &[u32],
    arena_words: u32,
    output: Range<u32>,
  ) -> io::Result<Vec<u32>> {
    dispatch_partitioned(
      target,
      Batch {
        shaders: &self.programs,
        passes: &self.passes,
        input,
        arena_words,
        output,
      },
      &self.partitions,
    )
  }
}
