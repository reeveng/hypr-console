//! Block inter prediction process (8.5.2.4): separable 8-tap sub-sample interpolation with edge
//! clamping, for unscaled (step 16) and scaled references.

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};
use console_core_pixel_buffers::{RowLayout, RowLayoutError, copy_clamped_window};
use console_core_simd::Simd;
use console_core_size::Size;
use console_core_video_interpolation::{
  FilterTaps, TapSpanOutsideSource, convolve8, filter_8tap_u16_row, round_shift,
};

use crate::frame::{Frame, Line};
use crate::places::{after, read, span, span_mut};
use crate::tables::SUBPEL_FILTERS;

struct FilterPhase {
  filter: u8,
  phase: u8,
}

#[inline]
fn taps(FilterPhase { filter, phase }: FilterPhase) -> Result<&'static [i16; 8], Never> {
  let Ok(position) = index(
    u64::from(filter)
      .wrapping_mul(16)
      .wrapping_add(u64::from(phase)),
  );
  Ok(match SUBPEL_FILTERS.as_chunks::<8>().0.get(position) {
    Some(taps) => taps,
    None => &[0; 8],
  })
}

pub(crate) struct RefPlane<'a> {
  pub(crate) frame: &'a Frame,
  pub(crate) plane: u64,
  pub(crate) last_x: i32,
  pub(crate) last_y: i32,
}

impl RefPlane<'_> {
  #[inline]
  fn row(&self, y: u64) -> Result<&[u16], Never> {
    self.frame.row(Line {
      plane: self.plane,
      y,
    })
  }
}

#[derive(Clone, Copy)]
pub(crate) struct Motion {
  pub(crate) x: i32,
  pub(crate) y: i32,
  pub(crate) step_x: i32,
  pub(crate) step_y: i32,
  pub(crate) width: u64,
  pub(crate) height: u64,
  pub(crate) filter: u8,
  pub(crate) bit_depth: u8,
}

pub(crate) fn predict(
  reference: &RefPlane<'_>,
  motion: Motion,
  output: &mut [u16],
  intermediate: &mut [u16],
  window: &mut [u16],
) -> Result<(), Never> {
  match (motion.step_x, motion.step_y) {
    (16, 16) => predict_unscaled(reference, motion, output, intermediate, window),
    _ => predict_scaled(reference, motion, output, intermediate),
  }
}

type WindowOrigin = console_core_geometry::Point<i32>;

struct SourceWindow<'a> {
  samples: &'a [u16],
  stride: u64,
  start: u64,
}

impl SourceWindow<'_> {
  fn row_start(&self, row: u64) -> Result<u64, Never> {
    Ok(self.start.wrapping_add(row.wrapping_mul(self.stride)))
  }
}

fn fetch_window<'a>(
  reference: &RefPlane<'_>,
  origin: WindowOrigin,
  block: Size<u64>,
  window: &'a mut [u16],
) -> Result<SourceWindow<'a>, RowLayoutError> {
  let columns = block.width.checked_add(7).ok_or(RowLayoutError::Bounds)?;
  let rows = block.height.checked_add(7).ok_or(RowLayoutError::Bounds)?;
  let stride = i64::try_from(columns).map_err(|_overflow| RowLayoutError::Stride)?;
  let width = u64::try_from(reference.last_x).map_err(|_negative| RowLayoutError::Bounds)?;
  let height = u64::try_from(reference.last_y).map_err(|_negative| RowLayoutError::Bounds)?;
  let layout = RowLayout::new(
    Size {
      width: columns,
      height: rows,
    },
    0,
    stride,
  )?;
  let Ok(source_width) = index(width.wrapping_add(1));
  let Ok(source_height) = index(height.wrapping_add(1));
  let top_left = Point {
    x: i64::from(origin.x).wrapping_sub(3),
    y: i64::from(origin.y).wrapping_sub(3),
  };
  copy_clamped_window(
    Size {
      width: source_width,
      height: source_height,
    },
    top_left,
    (&layout, window),
    |y, columns_2| {
      let Ok(row_number) = fixed_index(y);
      let Ok(samples) = reference.row(row_number);
      samples.get(columns_2)
    },
    |source, into| {
      for (out, sample) in into.iter_mut().zip(source) {
        *out = *sample;
      }
    },
  )?;

  Ok(SourceWindow {
    samples: window,
    stride: columns,
    start: 0,
  })
}

fn source_window<'a>(
  reference: &'a RefPlane<'_>,
  origin: WindowOrigin,
  block: Size<u64>,
  buffer: &'a mut [u16],
) -> Result<SourceWindow<'a>, RowLayoutError> {
  let x_start = origin.x.wrapping_sub(3);
  let y_start = origin.y.wrapping_sub(3);
  let Ok(width) = wrapped::<_, i32>(block.width);
  let Ok(height) = wrapped::<_, i32>(block.height);
  let x_end = origin.x.wrapping_add(width).wrapping_add(4);
  let y_end = origin.y.wrapping_add(height).wrapping_add(4);
  let within =
    x_start >= 0 && y_start >= 0 && x_end <= reference.last_x && y_end <= reference.last_y;

  match within.then_some(()) {
    Some(()) => {
      let Ok(first) = wrapped::<_, u64>(y_start);
      let Ok(last) = wrapped::<_, u64>(y_end.wrapping_add(1));
      let Ok(band) = reference.frame.span(reference.plane, first..last);

      match band {
        Some((samples, row_offset)) => {
          let Ok(layout) = reference.frame.layout(reference.plane);
          let Ok(column) = wrapped::<_, u64>(x_start);
          return Ok(SourceWindow {
            samples,
            stride: layout.stride,
            start: row_offset.wrapping_add(column),
          });
        }
        None => {}
      }
    }
    None => {}
  }

  fetch_window(reference, origin, block, buffer)
}

#[doc = "Uses narrow SIMD kernels on short 8-bit rows and the 16-bit accumulator on wide 8-bit rows."]
fn predict_unscaled(
  reference: &RefPlane<'_>,
  motion: Motion,
  output: &mut [u16],
  intermediate: &mut [u16],
  window: &mut [u16],
) -> Result<(), Never> {
  let origin = WindowOrigin {
    x: motion.x.wrapping_shr(4),
    y: motion.y.wrapping_shr(4),
  };
  let shape = Size {
    width: motion.width,
    height: motion.height,
  };
  let source = match source_window(reference, origin, shape, window) {
    Ok(window_2) => window_2,
    Err(_invalid_layout) => return Ok(()),
  };
  let Ok(horizontal_phase) = wrapped::<_, u8>(motion.x & 15);
  let Ok(vertical_phase) = wrapped::<_, u8>(motion.y & 15);
  let maximum = 1i32
    .wrapping_shl(u32::from(motion.bit_depth))
    .wrapping_sub(1);
  let simd = match (horizontal_phase, vertical_phase, maximum, shape.width) {
    (0, 0, _, _) | (_, _, 255, 32..) => None,
    _ => {
      let Ok(available) = Simd::available();
      available.skip(1).last()
    }
  };
  let Ok(horizontal) = taps(FilterPhase {
    filter: motion.filter,
    phase: horizontal_phase,
  });
  let Ok(vertical) = taps(FilterPhase {
    filter: motion.filter,
    phase: vertical_phase,
  });

  match (horizontal_phase, vertical_phase) {
    (0, 0) => {
      for row in 0..shape.height {
        let Ok(start) = source.row_start(row.wrapping_add(3));
        let position = start.wrapping_add(3);
        let Ok(samples) = span(source.samples, position..position.wrapping_add(shape.width));
        let Ok(destination) = destination_row(
          output,
          RowPosition {
            row,
            width: shape.width,
          },
        );

        for (out, sample) in destination.iter_mut().zip(samples) {
          *out = *sample;
        }
      }
    }
    (_, 0) => {
      for row in 0..shape.height {
        let Ok(start) = source.row_start(row.wrapping_add(3));
        let Ok(samples) = after(source.samples, start);
        let Ok(destination) = destination_row(
          output,
          RowPosition {
            row,
            width: shape.width,
          },
        );
        let Ok(()) = filter_row(simd, samples, 1, horizontal, maximum, destination);
      }
    }
    (0, _) => {
      for row in 0..shape.height {
        let Ok(start) = source.row_start(row);
        let Ok(samples) = after(source.samples, start.wrapping_add(3));
        let Ok(destination) = destination_row(
          output,
          RowPosition {
            row,
            width: shape.width,
          },
        );
        let Ok(()) = filter_row(simd, samples, source.stride, vertical, maximum, destination);
      }
    }
    _ => {
      for row in 0..shape.height.wrapping_add(7) {
        let Ok(start) = source.row_start(row);
        let Ok(samples) = after(source.samples, start);
        let Ok(destination) = destination_row(
          intermediate,
          RowPosition {
            row,
            width: shape.width,
          },
        );
        let Ok(()) = filter_row(simd, samples, 1, horizontal, maximum, destination);
      }

      for row in 0..shape.height {
        let Ok(samples) = after(intermediate, row.wrapping_mul(shape.width));
        let Ok(destination) = destination_row(
          output,
          RowPosition {
            row,
            width: shape.width,
          },
        );
        let Ok(()) = filter_row(simd, samples, shape.width, vertical, maximum, destination);
      }
    }
  }

  Ok(())
}

struct RowPosition {
  row: u64,
  width: u64,
}

fn destination_row(
  output: &mut [u16],
  RowPosition { row, width }: RowPosition,
) -> Result<&mut [u16], Never> {
  let start = row.wrapping_mul(width);
  span_mut(output, start..start.wrapping_add(width))
}

#[inline(always)]
fn filter_row(
  simd: Option<Simd>,
  source: &[u16],
  step: u64,
  coefficients: &[i16; 8],
  maximum: i32,
  output: &mut [u16],
) -> Result<(), Never> {
  match output.len() {
    4 => filter_row_n::<4>(simd, source, step, coefficients, maximum, output),
    8 => filter_row_n::<8>(simd, source, step, coefficients, maximum, output),
    16 => filter_row_n::<16>(simd, source, step, coefficients, maximum, output),
    32 => filter_row_n::<32>(simd, source, step, coefficients, maximum, output),
    64 => filter_row_n::<64>(simd, source, step, coefficients, maximum, output),
    _ => filter_scalar_row(source, step, coefficients, maximum, output),
  }
}

fn filter_scalar_row(
  source: &[u16],
  step: u64,
  coefficients: &[i16; 8],
  maximum: i32,
  output: &mut [u16],
) -> Result<(), Never> {
  for (column, pixel) in output.iter_mut().enumerate() {
    let Ok(x) = fixed_index(column);
    let sum = convolve8(coefficients, |tap| {
      let Ok(k) = fixed_index(tap);
      let Ok(sample) = read(source, x.wrapping_add(k.wrapping_mul(step)));
      i32::from(sample)
    });
    let Ok(sample) = finish_sample(SampleSum { sum, maximum });
    *pixel = sample;
  }

  Ok(())
}

#[inline(always)]
fn filter_row_n<const WIDTH: usize>(
  simd: Option<Simd>,
  source: &[u16],
  step: u64,
  coefficients: &[i16; 8],
  maximum: i32,
  output: &mut [u16],
) -> Result<(), Never> {
  let destination = match output.first_chunk_mut::<WIDTH>() {
    Some(samples) => samples,
    None => return Ok(()),
  };

  match (maximum, simd) {
    (255, None) => filter_row_8_bit(source, step, coefficients, destination),
    (sample_maximum, capability) => {
      let Ok(clip) = wrapped(sample_maximum);
      let backend = match capability {
        Some(selected) => selected,
        None => Simd::SCALAR,
      };

      match filter_8tap_u16_row::<7>(
        backend,
        source,
        coefficients,
        FilterTaps {
          step,
          bias: 64,
          maximum: clip,
        },
        destination,
      ) {
        Ok(()) => Ok(()),
        Err(_missing_taps) => {
          filter_scalar_row(source, step, coefficients, sample_maximum, destination)
        }
      }
    }
  }
}

#[inline(always)]
fn filter_row_8_bit<const WIDTH: usize>(
  source: &[u16],
  step: u64,
  coefficients: &[i16; 8],
  output: &mut [u16; WIDTH],
) -> Result<(), Never> {
  let mut sums = [64u16; WIDTH];

  for (tap, &coefficient) in coefficients.iter().enumerate() {
    let Ok(k) = fixed_index(tap);
    let Ok(at) = index(k.wrapping_mul(step));
    let samples = match source
      .get(at..)
      .and_then(|tail| tail.first_chunk::<WIDTH>())
    {
      Some(samples) => samples,
      None => return Ok(()),
    };
    let Ok(weight) = wrapped::<_, u16>(coefficient);

    for (sum, &sample) in sums.iter_mut().zip(samples) {
      *sum = sum.wrapping_add(weight.wrapping_mul(sample));
    }
  }

  for (pixel, sum) in output.iter_mut().zip(sums) {
    *pixel = match sum.cmp(&0xC000) {
      core::cmp::Ordering::Less => sum.wrapping_shr(7).min(255),
      core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => 0,
    };
  }

  Ok(())
}

struct SampleSum {
  sum: i32,
  maximum: i32,
}

#[inline]
fn finish_sample(SampleSum { sum, maximum }: SampleSum) -> Result<u16, Never> {
  wrapped(round_shift(sum, 7).clamp(0, maximum.max(0)))
}

fn predict_scaled(
  reference: &RefPlane<'_>,
  motion: Motion,
  output: &mut [u16],
  intermediate: &mut [u16],
) -> Result<(), Never> {
  let maximum = 1i32
    .wrapping_shl(u32::from(motion.bit_depth))
    .wrapping_sub(1);
  let Ok(height) = wrapped::<_, i32>(motion.height);
  let rows = height
    .wrapping_sub(1)
    .wrapping_mul(motion.step_y)
    .wrapping_add(15)
    .wrapping_shr(4)
    .wrapping_add(8);
  let Ok(intermediate_rows) = wrapped::<_, u64>(rows.max(0));
  let Ok(()) = scaled_horizontal(reference, motion, intermediate_rows, maximum, intermediate);
  let Ok(simd) = Simd::detected();
  let Ok(clip) = wrapped::<_, u16>(maximum);

  for row in 0..motion.height {
    let Ok(vertical) = wrapped::<_, i32>(row);
    let pos = (motion.y & 15).wrapping_add(motion.step_y.wrapping_mul(vertical));
    let Ok(step) = wrapped::<_, u8>(pos & 15);
    let Ok(coefficients) = taps(FilterPhase {
      filter: motion.filter,
      phase: step,
    });
    let Ok(start) = wrapped::<_, u64>(pos.wrapping_shr(4));
    let Ok(destination) = destination_row(
      output,
      RowPosition {
        row,
        width: motion.width,
      },
    );

    let source = start.checked_mul(motion.width).and_then(|offset| {
      let Ok(at) = index(offset);
      intermediate.get(at..)
    });
    let filtered = match source {
      Some(readable) => Some(filter_8tap_u16_row::<7>(
        simd,
        readable,
        coefficients,
        FilterTaps {
          step: motion.width,
          bias: 64,
          maximum: clip,
        },
        destination,
      )),
      None => None,
    };

    match filtered {
      Some(Ok(())) => continue,
      Some(Err(TapSpanOutsideSource)) | None => {}
    }

    for (column, pixel) in destination.iter_mut().enumerate() {
      let Ok(x) = fixed_index(column);
      let sum = convolve8(coefficients, |tap| {
        let Ok(k) = fixed_index(tap);
        let offset = start
          .wrapping_add(k)
          .wrapping_mul(motion.width)
          .wrapping_add(x);
        let Ok(sample) = read(intermediate, offset);
        i32::from(sample)
      });
      let Ok(sample) = finish_sample(SampleSum { sum, maximum });
      *pixel = sample;
    }
  }

  Ok(())
}

fn scaled_horizontal(
  reference: &RefPlane<'_>,
  motion: Motion,
  rows: u64,
  maximum: i32,
  output: &mut [u16],
) -> Result<(), Never> {
  let Ok(capacity) = fixed_index(output.len());
  let row_count = rows.min(capacity.wrapping_div(motion.width.max(1)));

  for row in 0..row_count {
    let Ok(row_number) = wrapped::<_, i32>(row);
    let y = motion
      .y
      .wrapping_shr(4)
      .wrapping_add(row_number)
      .wrapping_sub(3);
    let Ok(row_index) = wrapped::<_, u64>(y.clamp(0, reference.last_y.max(0)));
    let Ok(source) = reference.row(row_index);
    let Ok(destination) = destination_row(
      output,
      RowPosition {
        row,
        width: motion.width,
      },
    );

    for (column, pixel) in destination.iter_mut().enumerate() {
      let Ok(x) = wrapped::<_, i32>(column);
      let pos = motion.x.wrapping_add(motion.step_x.wrapping_mul(x));
      let Ok(phase) = wrapped::<_, u8>(pos & 15);
      let Ok(coefficients) = taps(FilterPhase {
        filter: motion.filter,
        phase: phase,
      });
      let sum = convolve8(coefficients, |tap| {
        let Ok(k) = wrapped::<_, i32>(tap);
        let x_2 = pos
          .wrapping_shr(4)
          .wrapping_add(k)
          .wrapping_sub(3)
          .clamp(0, reference.last_x.max(0));
        let Ok(offset) = wrapped::<_, u64>(x_2);
        let Ok(sample) = read(source, offset);
        i32::from(sample)
      });
      let Ok(sample) = finish_sample(SampleSum { sum, maximum });
      *pixel = sample;
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn reference_frame(bit_depth: u8) -> Result<Frame, Never> {
    let range = 1_u64.wrapping_shl(u32::from(bit_depth));
    let data: Vec<u16> = (0..1200u64)
      .map(|i| {
        let value = i
          .wrapping_mul(37)
          .wrapping_add(i.wrapping_div(40).wrapping_mul(11))
          .wrapping_rem(range.max(1));
        let Ok(sample) = wrapped::<_, u16>(value);
        sample
      })
      .collect();
    let size = Size {
      width: 40,
      height: 30,
    };
    let info = crate::frame::FrameInfo {
      size,
      subsampling: crate::frame::Subsampling { x: 1, y: 1 },
      bit_depth,
      color_space: 0,
      color_range: crate::frame::SampleRange::Limited,
      render: size,
      kind: crate::frame::FrameKind::Key,
    };
    let mut full = vec![0u16; 4096];

    for (destination, samples) in full.chunks_exact_mut(64).zip(data.chunks_exact(40)) {
      for (out, &value) in destination.iter_mut().zip(samples) {
        *out = value;
      }
    }

    let chroma = vec![0u16; 1024];
    Frame::from_planes(info, [&full, &chroma, &chroma])
  }

  #[test]
  fn scaled_path_with_unit_step_matches_unscaled() {
    for bit_depth in [8, 10, 12] {
      let Ok(()) = compare_scaled_and_unscaled(bit_depth);
    }
  }

  #[test]
  fn scaled_vertical_filter_keeps_zero_filled_taps_at_a_short_scratch_boundary() {
    let Ok(frame) = reference_frame(8);
    let reference = RefPlane {
      frame: &frame,
      plane: 0,
      last_x: 39,
      last_y: 29,
    };
    let motion = Motion {
      x: 0,
      y: 0,
      step_x: 16,
      step_y: 32,
      width: 8,
      height: 4,
      filter: 0,
      bit_depth: 8,
    };
    let mut output = [[u16::MAX; 8]; 4];
    let mut intermediate = [0; 64];
    let Ok(()) = predict_scaled(
      &reference,
      motion,
      output.as_flattened_mut(),
      &mut intermediate,
    );

    let [first, second, third, last] = &output;

    for (row, y) in [first, second, third].into_iter().zip([0, 2, 4]) {
      let Ok(reference_row) = reference.row(y);
      assert_eq!(Some(row), reference_row.first_chunk::<8>());
    }

    assert_eq!(*last, [0; 8]);
  }

  fn compare_scaled_and_unscaled(bit_depth: u8) -> Result<(), Never> {
    let Ok(frame) = reference_frame(bit_depth);
    let reference = RefPlane {
      frame: &frame,
      plane: 0,
      last_x: 39,
      last_y: 29,
    };
    let mut tmp = vec![0u16; 6400];

    for filter in 0..4 {
      for (x, y) in [
        (0, 0),
        (5, 3),
        (117, 160),
        (117, 155),
        (-40, -3),
        (577, 415),
        (160, 168),
      ] {
        for (width, height) in [(4u64, 4u64), (8, 8), (16, 8), (32, 8), (64, 8)] {
          let size = width.wrapping_mul(height);
          let Ok(mut a) = crate::frame::filled::<u16, 128>(size, 0);
          let Ok(mut b) = crate::frame::filled::<u16, 128>(size, 0);
          let motion = Motion {
            x,
            y,
            step_x: 16,
            step_y: 16,
            width,
            height,
            filter,
            bit_depth,
          };
          let Ok(()) = predict_unscaled(&reference, motion, &mut a, &mut tmp, &mut [0; 5041]);
          let Ok(()) = predict_scaled(&reference, motion, &mut b, &mut tmp);
          assert_eq!(
            a, b,
            "{bit_depth}-bit filter {filter} pos ({x},{y}) {width}x{height}"
          );
        }
      }
    }

    Ok(())
  }
}
