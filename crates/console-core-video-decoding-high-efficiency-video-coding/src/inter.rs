//! Fractional sample interpolation (8.5.3.3.3) and weighted sample prediction (8.5.3.3.4).

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};

use crate::picture::{Component, Frame, SampleWindow, WindowOutput};
use crate::places::{read, tail, window, window_mut};
use crate::spec_tables::{CHROMA_FILTER, LUMA_FILTER};

const WINDOW_STRIDE: u64 = 71;

pub struct McScratch {
  win: Vec<i16>,
  tmp: Vec<i16>,
}

impl Default for McScratch {
  fn default() -> Self {
    Self {
      win: vec![0; 5041],
      tmp: vec![0; 5041],
    }
  }
}

#[derive(Clone, Copy)]
pub struct Block {
  pub origin: Point<i32>,
  pub size: Size<u64>,
}

pub struct Motion {
  pub block: Block,
  pub vector: [i16; 2],
  pub plane: Component,
}

pub fn compensate(
  reference: &Frame,
  motion: Motion,
  samples: &mut [i16],
  scratch: &mut McScratch,
) -> Result<(), Never> {
  match motion.plane {
    Component::Luma => {
      let kernel = PlaneKernel {
        fraction: 2,
        bit_depth: reference.bit_depth,
        filters: &LUMA_FILTER,
      };
      interpolate(reference, motion, kernel, samples, scratch)
    }
    Component::Blue | Component::Red => {
      let kernel = PlaneKernel {
        fraction: 3,
        bit_depth: reference.bit_depth_c,
        filters: &CHROMA_FILTER,
      };
      interpolate(reference, motion, kernel, samples, scratch)
    }
  }
}

struct PlaneKernel<'a, const T: usize> {
  fraction: u32,
  bit_depth: u32,
  filters: &'a [[i8; T]],
}

struct Phases {
  horizontal: u64,
  vertical: u64,
}

fn interpolate<const T: usize>(
  reference: &Frame,
  motion: Motion,
  kernel: PlaneKernel<'_, T>,
  samples: &mut [i16],
  scratch: &mut McScratch,
) -> Result<(), Never> {
  let [vx, vy] = motion.vector;
  let shift = kernel.fraction;
  let mask = 1i16.wrapping_shl(shift).wrapping_sub(1);
  let Ok(horizontal) = wrapped::<_, u64>(vx & mask);
  let Ok(vertical) = wrapped::<_, u64>(vy & mask);
  let phases = Phases {
    horizontal,
    vertical,
  };
  let Ok(taps) = fixed_index(T);
  let padding = taps.wrapping_sub(1);
  let Ok(margin) = wrapped::<_, i32>(taps.wrapping_div(2).wrapping_sub(1));
  let origin = Point {
    x: i64::from(
      motion
        .block
        .origin
        .x
        .wrapping_add(i32::from(vx).wrapping_shr(shift))
        .wrapping_sub(margin),
    ),
    y: i64::from(
      motion
        .block
        .origin
        .y
        .wrapping_add(i32::from(vy).wrapping_shr(shift))
        .wrapping_sub(margin),
    ),
  };
  let size = motion.block.size;
  let requested = SampleWindow {
    plane: motion.plane,
    origin,
    size: Size {
      width: size.width.wrapping_add(padding),
      height: size.height.wrapping_add(padding),
    },
  };
  let target = WindowOutput {
    samples: &mut scratch.win,
    stride: WINDOW_STRIDE,
  };

  match reference.copy_window(requested, target) {
    Ok(()) => {}
    Err(_unavailable_reference_samples) => return Ok(()),
  }

  let window = Interpolation {
    size,
    phases,
    kernel,
    padding,
  };
  apply_interpolation(&scratch.win, &mut scratch.tmp, window, samples)
}

struct Interpolation<'a, const T: usize> {
  size: Size<u64>,
  phases: Phases,
  kernel: PlaneKernel<'a, T>,
  padding: u64,
}

fn apply_interpolation<const T: usize>(
  source: &[i16],
  scratch: &mut [i16],
  info: Interpolation<'_, T>,
  output: &mut [i16],
) -> Result<(), Never> {
  let Interpolation {
    size,
    phases,
    kernel,
    padding,
  } = info;
  let Phases {
    horizontal,
    vertical,
  } = phases;
  let midpoint = padding.wrapping_div(2);
  let reduction = kernel.bit_depth.min(12).wrapping_sub(8);
  let scaled = 14u32.wrapping_sub(kernel.bit_depth);
  let Ok(h) = filter_taps(kernel.filters, horizontal);
  let Ok(v) = filter_taps(kernel.filters, vertical);
  let horizontal_filter = TapFilter {
    weights: h,
    entries_apart: 1,
    reduction,
  };
  let vertical_filter = TapFilter {
    weights: v,
    entries_apart: WINDOW_STRIDE,
    reduction,
  };
  let width = size.width;

  match (horizontal, vertical) {
    (0, 0) => {
      for row in 0..size.height {
        let start = row
          .wrapping_add(midpoint)
          .wrapping_mul(WINDOW_STRIDE)
          .wrapping_add(midpoint);
        let Ok(input) = window(source, start..start.wrapping_add(width));
        let destination = row.wrapping_mul(width);
        let Ok(pixels) = window_mut(output, destination..destination.wrapping_add(width));

        for (sample, &value) in pixels.iter_mut().zip(input) {
          *sample = value.wrapping_shl(scaled);
        }
      }

      Ok(())
    }
    (_, 0) => {
      let layout = PassLayout {
        first: midpoint.wrapping_mul(WINDOW_STRIDE),
        stride: width,
        size,
      };
      filter_rows(source, output, layout, horizontal_filter)
    }
    (0, _) => {
      let layout = PassLayout {
        first: midpoint,
        stride: width,
        size,
      };
      filter_rows(source, output, layout, vertical_filter)
    }
    (_, _) => {
      let extra = Size {
        width,
        height: size.height.wrapping_add(padding),
      };
      let horizontal_2 = PassLayout {
        first: 0,
        stride: WINDOW_STRIDE,
        size: extra,
      };
      let vertical_2 = PassLayout {
        first: 0,
        stride: width,
        size,
      };
      let Ok(()) = filter_rows(source, scratch, horizontal_2, horizontal_filter);
      filter_rows(
        scratch,
        output,
        vertical_2,
        TapFilter {
          reduction: 6,
          ..vertical_filter
        },
      )
    }
  }
}

fn filter_taps<const T: usize>(filters: &[[i8; T]], phase: u64) -> Result<&[i8; T], Never> {
  let Ok(at) = index(phase);
  Ok(match filters.get(at) {
    Some(weights) => weights,
    None => &[0; T],
  })
}

struct PassLayout {
  first: u64,
  stride: u64,
  size: Size<u64>,
}

#[derive(Clone, Copy)]
struct TapFilter<'a, const T: usize> {
  weights: &'a [i8; T],
  entries_apart: u64,
  reduction: u32,
}

fn filter_rows<const T: usize>(
  input: &[i16],
  output: &mut [i16],
  layout: PassLayout,
  filter: TapFilter<'_, T>,
) -> Result<(), Never> {
  for row in 0..layout.size.height {
    let start = row.wrapping_mul(WINDOW_STRIDE).wrapping_add(layout.first);
    let position = row.wrapping_mul(layout.stride);
    let Ok(source) = tail(input, start);
    let Ok(target) = window_mut(output, position..position.wrapping_add(layout.size.width));
    let Ok(()) = convolve(source, target, filter);
  }

  Ok(())
}

#[inline(always)]
fn convolve<const T: usize>(
  input: &[i16],
  output: &mut [i16],
  filter: TapFilter<'_, T>,
) -> Result<(), Never> {
  match output.len() {
    2 => convolve_fixed::<T, 2>(input, output, filter),
    4 => convolve_fixed::<T, 4>(input, output, filter),
    6 => convolve_fixed::<T, 6>(input, output, filter),
    8 => convolve_fixed::<T, 8>(input, output, filter),
    12 => convolve_fixed::<T, 12>(input, output, filter),
    16 => convolve_fixed::<T, 16>(input, output, filter),
    24 => convolve_fixed::<T, 24>(input, output, filter),
    32 => convolve_fixed::<T, 32>(input, output, filter),
    48 => convolve_fixed::<T, 48>(input, output, filter),
    64 => convolve_fixed::<T, 64>(input, output, filter),
    _ => convolve_other(input, output, filter),
  }
}

#[inline(always)]
fn convolve_fixed<const T: usize, const W: usize>(
  input: &[i16],
  output: &mut [i16],
  filter: TapFilter<'_, T>,
) -> Result<(), Never> {
  let mut accumulator = [0i32; W];

  for (tap, &weight) in (0u64..).zip(filter.weights) {
    let Ok(run) = tail(input, tap.wrapping_mul(filter.entries_apart));
    let active = match run.first_chunk::<W>() {
      Some(active) => active,
      None => return Ok(()),
    };

    for (sum, &value) in accumulator.iter_mut().zip(active) {
      *sum = sum.wrapping_add(i32::from(weight).wrapping_mul(i32::from(value)));
    }
  }

  for (sample, sum) in output.iter_mut().zip(accumulator) {
    let Ok(value) = wrapped::<_, i16>(sum.wrapping_shr(filter.reduction));
    *sample = value;
  }

  Ok(())
}

fn convolve_other<const T: usize>(
  input: &[i16],
  output: &mut [i16],
  filter: TapFilter<'_, T>,
) -> Result<(), Never> {
  for (position, out) in (0u64..).zip(output) {
    let mut sum = 0i32;

    for (tap, &factor) in (0u64..).zip(filter.weights) {
      let Ok(value) = read(
        input,
        position.wrapping_add(tap.wrapping_mul(filter.entries_apart)),
      );
      sum = sum.wrapping_add(i32::from(factor).wrapping_mul(i32::from(value)));
    }

    let Ok(value) = wrapped::<_, i16>(sum.wrapping_shr(filter.reduction));
    *out = value;
  }

  Ok(())
}

pub struct Surface<'a> {
  pub samples: &'a mut [u16],
  pub size: Size<u64>,
  pub stride: u64,
  pub bit_depth: u32,
}

#[derive(Clone, Copy)]
pub struct Weight {
  pub value: i32,
  pub offset: i32,
}

pub struct WeightedSamples<'a> {
  pub samples: &'a [i16],
  pub weight: Weight,
}

pub enum Blending<'a> {
  Single(&'a [i16]),
  Average {
    first: &'a [i16],
    second: &'a [i16],
  },
  Weighted {
    first: WeightedSamples<'a>,
    second: Option<WeightedSamples<'a>>,
    denominator: u32,
  },
}

pub fn put(source: Blending<'_>, destination: Surface<'_>) -> Result<(), Never> {
  let shift = 14u32.wrapping_sub(destination.bit_depth);

  match source {
    Blending::Single(input) => {
      let Ok(round) = rounding(shift);
      write_blended(input, None, destination, |first, _| {
        first.wrapping_add(round).wrapping_shr(shift)
      })
    }
    Blending::Average { first, second } => {
      let shift_2 = shift.wrapping_add(1);
      let round = 1i32.wrapping_shl(shift_2.wrapping_sub(1));
      write_blended(first, Some(second), destination, |a, b| {
        a.wrapping_add(b).wrapping_add(round).wrapping_shr(shift_2)
      })
    }
    Blending::Weighted {
      first,
      second,
      denominator,
    } => {
      let exponent = denominator.wrapping_add(shift);

      match second {
        Some(second_2) => {
          let offset = first
            .weight
            .offset
            .wrapping_add(second_2.weight.offset)
            .wrapping_add(1)
            .wrapping_shl(exponent);
          write_blended(
            first.samples,
            Some(second_2.samples),
            destination,
            |a, b| {
              a.wrapping_mul(first.weight.value)
                .wrapping_add(b.wrapping_mul(second_2.weight.value))
                .wrapping_add(offset)
                .wrapping_shr(exponent.wrapping_add(1))
            },
          )
        }
        None => {
          let Ok(round) = rounding(exponent);
          write_blended(first.samples, None, destination, |a, _| {
            a.wrapping_mul(first.weight.value)
              .wrapping_add(round)
              .wrapping_shr(exponent)
              .wrapping_add(first.weight.offset)
          })
        }
      }
    }
  }
}

fn rounding(shift: u32) -> Result<i32, Never> {
  Ok(match shift {
    0 => 0,
    _ => 1i32.wrapping_shl(shift.wrapping_sub(1)),
  })
}

fn write_blended(
  first: &[i16],
  second: Option<&[i16]>,
  destination: Surface<'_>,
  blend: impl Fn(i32, i32) -> i32,
) -> Result<(), Never> {
  let max = 1i32
    .wrapping_shl(destination.bit_depth)
    .wrapping_sub(1)
    .max(0);
  let width = destination.size.width;

  for line in 0..destination.size.height {
    let offset = line.wrapping_mul(width);
    let range = offset..offset.wrapping_add(width);
    let Ok(primary) = window(first, range.clone());
    let start = line.wrapping_mul(destination.stride);
    let Ok(row) = window_mut(destination.samples, start..start.wrapping_add(width));

    match second {
      Some(plane) => {
        let Ok(compound) = window(plane, range);

        for ((sample, &a), &b) in row.iter_mut().zip(primary).zip(compound) {
          let Ok(value) = wrapped::<_, u16>(blend(i32::from(a), i32::from(b)).clamp(0, max));
          *sample = value;
        }
      }
      None => {
        for (sample, &a) in row.iter_mut().zip(primary) {
          let Ok(value) = wrapped::<_, u16>(blend(i32::from(a), 0).clamp(0, max));
          *sample = value;
        }
      }
    }
  }

  Ok(())
}
