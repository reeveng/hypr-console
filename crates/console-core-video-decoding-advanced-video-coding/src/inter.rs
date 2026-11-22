//! Inter prediction sample interpolation (8.4.2.2) and weighted sample prediction (8.4.2.3).

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};

#[derive(Clone, Copy)]
pub(crate) struct BlockLayout {
  pub(crate) size: Size<u64>,
  pub(crate) source_stride: u64,
  pub(crate) destination_stride: u64,
}

#[inline(always)]
fn tap6([a, b, c, d, e, f]: [i32; 6]) -> Result<i32, Never> {
  Ok(
    a.wrapping_sub(5_i32.wrapping_mul(b))
      .wrapping_add(20_i32.wrapping_mul(c))
      .wrapping_add(20_i32.wrapping_mul(d))
      .wrapping_sub(5_i32.wrapping_mul(e))
      .wrapping_add(f),
  )
}

#[inline(always)]
fn clip1(value: i32) -> Result<u8, Never> {
  wrapped::<_, u8>(value.clamp(0, 255))
}

#[inline(always)]
fn average(values: [u8; 2]) -> Result<u8, Never> {
  let [a, b] = values.map(u16::from);
  wrapped::<_, u8>(a.wrapping_add(b).wrapping_add(1).wrapping_shr(1))
}

#[inline(always)]
fn plane_row(
  plane: &[u8],
  PlaneRowArguments { row, stride }: PlaneRowArguments,
) -> Result<&[u8], Never> {
  let Ok(position) = index(row.wrapping_mul(stride));
  Ok(match plane.get(position..) {
    Some(samples) => samples,
    None => &[],
  })
}

struct PlaneRowArguments {
  row: u64,
  stride: u64,
}

#[inline(always)]
fn output_row(plane: &mut [u8], row: u64, layout: BlockLayout) -> Result<&mut [u8], Never> {
  let start = row.wrapping_mul(layout.destination_stride);
  let Ok(from) = index(start);
  let Ok(to) = index(start.wrapping_add(layout.size.width));
  Ok(match plane.get_mut(from..to) {
    Some(samples) => samples,
    None => &mut [],
  })
}

#[inline(always)]
fn horizontal_taps(samples: &[u8], filtered: &mut [i32; 16], width: u64) -> Result<(), Never> {
  let Ok(count) = index(width);

  for (value, window) in filtered.iter_mut().take(count).zip(samples.windows(6)) {
    let taps = match window {
      &[a, b, c, d, e, f] => [a, b, c, d, e, f],
      _ => continue,
    };
    let Ok(sum) = tap6(taps.map(i32::from));
    *value = sum;
  }

  Ok(())
}

#[inline(always)]
fn vertical_taps<T: Copy + Into<i32>>(rows: [&[T]; 6], position: u64) -> Result<i32, Never> {
  let Ok(column) = index(position);
  let taps = rows.map(|row| match row.get(column) {
    Some(&value) => <_ as ::core::convert::Into<::core::primitive::i32>>::into(value),
    None => 0,
  });
  tap6(taps)
}

#[inline(always)]
fn sample(row: &[u8], position: u64) -> Result<u8, Never> {
  let Ok(column) = index(position);
  Ok(match row.get(column) {
    Some(&value) => value,
    None => 0,
  })
}

#[inline(always)]
fn horizontal_half(taps: &[i32; 16], position: u64) -> Result<u8, Never> {
  let Ok(column) = index(position);
  let raw = match taps.get(column) {
    Some(value) => *value,
    None => 0,
  };
  clip1(raw.wrapping_add(16).wrapping_shr(5))
}

#[inline(always)]
fn vertical_half(rows: [&[u8]; 6], position: u64) -> Result<u8, Never> {
  let Ok(taps) = vertical_taps(rows, position);
  clip1(taps.wrapping_add(16).wrapping_shr(5))
}

#[inline(always)]
fn horizontal_slot(phase: u32) -> Result<u64, Never> {
  Ok(match phase {
    1 => 2,
    _ => 3,
  })
}

fn luma_sample(
  source: [&[u8]; 6],
  horizontal: &[[i32; 16]; 6],
  fraction: Point<u32>,
  column: u64,
) -> Result<u8, Never> {
  let [_, _, row2, row3, _, _] = source;
  let [_, _, h2, h3, _, _] = horizontal;

  match (fraction.x, fraction.y) {
    (0, 0) => sample(row2, column.wrapping_add(2)),
    (x, 0) => {
      let Ok(half) = horizontal_half(h2, column);

      match x {
        2 => Ok(half),
        _ => {
          let Ok(offset) = horizontal_slot(x);
          let Ok(whole) = sample(row2, column.wrapping_add(offset));
          average([half, whole])
        }
      }
    }
    (0, y) => {
      let position = column.wrapping_add(2);
      let Ok(half) = vertical_half(source, position);

      match y {
        2 => Ok(half),
        _ => {
          let row = match y {
            1 => row2,
            _ => row3,
          };
          let Ok(whole) = sample(row, position);
          average([half, whole])
        }
      }
    }
    (2, _) | (_, 2) => diagonal_half(source, horizontal, fraction, column),
    (x, y) => {
      let taps = match y {
        1 => h2,
        _ => h3,
      };
      let Ok(h) = horizontal_half(taps, column);
      let Ok(offset) = horizontal_slot(x);
      let Ok(v) = vertical_half(source, column.wrapping_add(offset));
      average([h, v])
    }
  }
}

fn diagonal_half(
  source: [&[u8]; 6],
  horizontal: &[[i32; 16]; 6],
  fraction: Point<u32>,
  column: u64,
) -> Result<u8, Never> {
  let Ok(raw) = vertical_taps(horizontal.each_ref().map(<[i32; 16]>::as_slice), column);
  let Ok(diagonal) = clip1(raw.wrapping_add(512).wrapping_shr(10));
  let [_, _, middle, next, _, _] = horizontal;

  match (fraction.x, fraction.y) {
    (2, 2) => Ok(diagonal),
    (2, 1 | 3) => {
      let adjacent = match fraction.y {
        1 => middle,
        _ => next,
      };
      let Ok(half) = horizontal_half(adjacent, column);
      average([diagonal, half])
    }
    (x, _) => {
      let Ok(offset) = horizontal_slot(x);
      let Ok(half) = vertical_half(source, column.wrapping_add(offset));
      average([diagonal, half])
    }
  }
}

pub fn mc_luma_win(
  source: &[u8],
  destination: &mut [u8],
  layout: BlockLayout,
  fraction: Point<u32>,
) -> Result<(), Never> {
  match ::core::primitive::bool::then_some((fraction.x, fraction.y) == (0, 0), ()) {
    ::core::option::Option::Some(()) => {
      for row in 0..layout.size.height {
        let Ok(samples) = plane_row(
          source,
          self::PlaneRowArguments {
            row: row.wrapping_add(2),
            stride: layout.source_stride,
          },
        );
        let Ok(line) = output_row(destination, row, layout);

        for (output, &value) in line.iter_mut().zip(samples.iter().skip(2)) {
          *output = value;
        }
      }

      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  let mut sample_rows: [&[u8]; 21] = [&[]; 21];
  let mut horizontal = [[0_i32; 16]; 21];
  let input_height = layout.size.height.wrapping_add(5);

  for (row, samples) in (0..input_height).zip(&mut sample_rows) {
    let Ok(stripe) = plane_row(
      source,
      self::PlaneRowArguments {
        row,
        stride: layout.source_stride,
      },
    );
    *samples = stripe;
  }

  match ::core::primitive::bool::then_some(fraction.x != 0, ()) {
    ::core::option::Option::Some(()) => {
      for (stripe, values) in sample_rows.iter().zip(&mut horizontal) {
        let Ok(()) = horizontal_taps(stripe, values, layout.size.width);
      }
    }
    ::core::option::Option::None => {}
  }

  for (row, (samples, filtered)) in
    (0..layout.size.height).zip(sample_rows.windows(6).zip(horizontal.windows(6)))
  {
    let stripes = match samples.first_chunk::<6>() {
      Some(rows) => *rows,
      None => break,
    };
    let taps = match filtered.first_chunk::<6>() {
      Some(rows) => rows,
      None => break,
    };
    let Ok(line) = output_row(destination, row, layout);

    for (column, pixel) in (0..layout.size.width).zip(line) {
      let Ok(value) = luma_sample(stripes, taps, fraction, column);
      *pixel = value;
    }
  }

  Ok(())
}

pub fn mc_chroma_win(
  source: &[u8],
  destination: &mut [u8],
  layout: BlockLayout,
  fraction: Point<u32>,
) -> Result<(), Never> {
  let Ok(x) = wrapped::<_, u16>(fraction.x);
  let Ok(y) = wrapped::<_, u16>(fraction.y);
  let left = 8_u16.wrapping_sub(x);
  let top = 8_u16.wrapping_sub(y);
  let weights = [
    left.wrapping_mul(top),
    x.wrapping_mul(top),
    left.wrapping_mul(y),
    x.wrapping_mul(y),
  ];

  for row in 0..layout.size.height {
    let Ok(first) = plane_row(
      source,
      self::PlaneRowArguments {
        row,
        stride: layout.source_stride,
      },
    );
    let Ok(line) = output_row(destination, row, layout);

    match (x, y) {
      (0, 0) => {
        for (pixel, &value) in line.iter_mut().zip(first) {
          *pixel = value;
        }
      }
      _ => {
        let Ok(next) = plane_row(
          source,
          self::PlaneRowArguments {
            row: row.wrapping_add(1),
            stride: layout.source_stride,
          },
        );

        'inner_for_2: for (pixel, (upper, lower)) in
          line.iter_mut().zip(first.windows(2).zip(next.windows(2)))
        {
          let inputs = match (upper, lower) {
            (&[a, b], &[c, d]) => [a, b, c, d],
            _ => continue 'inner_for_2,
          };
          let sum = weights
            .into_iter()
            .zip(inputs)
            .fold(32_u16, |total, (weight, value)| {
              total.wrapping_add(weight.wrapping_mul(u16::from(value)))
            });
          let Ok(value) = wrapped::<_, u8>(sum.wrapping_shr(6));
          *pixel = value;
        }
      }
    }
  }

  Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Weight {
  Default,
  Weighted {
    log_wd: i32,
    w0: i32,
    w1: i32,
    o0: i32,
    o1: i32,
  },
}

struct WeightedSample {
  scale: i32,
  bias: i32,
  shift: u32,
  offset: i32,
}

impl WeightedSample {
  fn apply(&self, sum: i32) -> Result<u8, Never> {
    clip1(
      sum
        .wrapping_add(self.bias)
        .wrapping_shr(self.shift)
        .wrapping_add(self.offset),
    )
  }
}

pub fn weighted_store(
  destination: &mut [u8],
  [left, right]: [Option<&[u8]>; 2],
  layout: BlockLayout,
  weight: Weight,
) -> Result<(), Never> {
  match (left, right, weight) {
    (Some(a), Some(b), Weight::Default) => {
      for row in 0..layout.size.height {
        let Ok(line) = output_row(destination, row, layout);
        let Ok(first) = plane_row(
          a,
          self::PlaneRowArguments {
            row,
            stride: layout.source_stride,
          },
        );
        let Ok(second) = plane_row(
          b,
          self::PlaneRowArguments {
            row,
            stride: layout.source_stride,
          },
        );

        for ((output, &x), &y) in line.iter_mut().zip(first).zip(second) {
          let Ok(value) = average([x, y]);
          *output = value;
        }
      }
    }
    (
      Some(a),
      Some(b),
      Weight::Weighted {
        log_wd,
        w0,
        w1,
        o0,
        o1,
      },
    ) => {
      let Ok(exponent) = wrapped::<_, u32>(log_wd);
      let filter = WeightedSample {
        scale: w0,
        bias: 1_i32.wrapping_shl(exponent),
        shift: exponent.wrapping_add(1),
        offset: o0.wrapping_add(o1).wrapping_add(1).wrapping_shr(1),
      };

      for row in 0..layout.size.height {
        let Ok(line) = output_row(destination, row, layout);
        let Ok(first) = plane_row(
          a,
          self::PlaneRowArguments {
            row,
            stride: layout.source_stride,
          },
        );
        let Ok(second) = plane_row(
          b,
          self::PlaneRowArguments {
            row,
            stride: layout.source_stride,
          },
        );

        for ((output, &x), &y) in line.iter_mut().zip(first).zip(second) {
          let sum = i32::from(x)
            .wrapping_mul(filter.scale)
            .wrapping_add(i32::from(y).wrapping_mul(w1));
          let Ok(value) = filter.apply(sum);
          *output = value;
        }
      }
    }
    (Some(src), None, Weight::Default) | (None, Some(src), Weight::Default) => {
      for row in 0..layout.size.height {
        let Ok(line) = output_row(destination, row, layout);
        let Ok(input) = plane_row(
          src,
          self::PlaneRowArguments {
            row,
            stride: layout.source_stride,
          },
        );

        for (output, &value) in line.iter_mut().zip(input) {
          *output = value;
        }
      }
    }
    (Some(src), None, Weight::Weighted { log_wd, w0, o0, .. }) => {
      let Ok(()) = weight_one(destination, src, layout, log_wd, [w0, o0]);
    }
    (None, Some(src), Weight::Weighted { log_wd, w1, o1, .. }) => {
      let Ok(()) = weight_one(destination, src, layout, log_wd, [w1, o1]);
    }
    (None, None, _) => {}
  }

  Ok(())
}

fn weight_one(
  destination: &mut [u8],
  source: &[u8],
  layout: BlockLayout,
  log_wd: i32,
  [weight, offset]: [i32; 2],
) -> Result<(), Never> {
  let Ok(shift) = wrapped::<_, u32>(log_wd);
  let round = match log_wd {
    1.. => 1_i32.wrapping_shl(shift.wrapping_sub(1)),
    ..=0 => 0,
  };
  let filter = WeightedSample {
    scale: weight,
    bias: round,
    shift,
    offset,
  };

  for row in 0..layout.size.height {
    let Ok(line) = output_row(destination, row, layout);
    let Ok(input) = plane_row(
      source,
      self::PlaneRowArguments {
        row,
        stride: layout.source_stride,
      },
    );

    for (output, &value) in line.iter_mut().zip(input) {
      let Ok(weighted) = filter.apply(i32::from(value).wrapping_mul(filter.scale));
      *output = weighted;
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Clone, Copy)]
  struct Plane<'a> {
    samples: &'a [u8],
    size: Size<u64>,
    stride: u64,
  }

  impl Plane<'_> {
    fn sample(self, origin: Point<i32>) -> Result<i32, Never> {
      let Ok(last_x) = wrapped::<_, i32>(self.size.width.saturating_sub(1));
      let Ok(last_y) = wrapped::<_, i32>(self.size.height.saturating_sub(1));
      let Ok(x) = wrapped::<_, u64>(origin.x.clamp(0, last_x));
      let Ok(y) = wrapped::<_, u64>(origin.y.clamp(0, last_y));
      let Ok(position) = index(y.wrapping_mul(self.stride).wrapping_add(x));
      Ok(match self.samples.get(position) {
        Some(&sample) => i32::from(sample),
        None => 0,
      })
    }
  }

  #[derive(Clone, Copy)]
  struct Region {
    origin: Point<i32>,
    size: Size<u64>,
  }

  fn fetch(
    plane: Plane<'_>,
    origin: Point<i32>,
    layout: BlockLayout,
    out: &mut [u8],
  ) -> Result<(), Never> {
    for y in 0..layout.size.height {
      let Ok(row) = output_row(out, y, layout);
      let Ok(offset_y) = wrapped::<_, i32>(y);

      for (x, pixel) in (0u64..).zip(row) {
        let Ok(offset_x) = wrapped::<_, i32>(x);
        let Ok(sample) = plane.sample(Point {
          x: origin.x.wrapping_add(offset_x),
          y: origin.y.wrapping_add(offset_y),
        });
        let Ok(value) = wrapped(sample);
        *pixel = value;
      }
    }

    Ok(())
  }

  fn mc_luma(
    plane: Plane<'_>,
    region: Region,
    phase: Point<u32>,
    stride: u64,
    out: &mut [u8],
  ) -> Result<(), Never> {
    let mut input = [0; 441];
    let source_stride = 21;
    let size = Size {
      width: region.size.width.wrapping_add(5),
      height: region.size.height.wrapping_add(5),
    };
    let origin = Point {
      x: region.origin.x.wrapping_sub(2),
      y: region.origin.y.wrapping_sub(2),
    };
    let Ok(()) = fetch(
      plane,
      origin,
      BlockLayout {
        size,
        source_stride: 0,
        destination_stride: source_stride,
      },
      &mut input,
    );
    mc_luma_win(
      &input,
      out,
      BlockLayout {
        size: region.size,
        source_stride,
        destination_stride: stride,
      },
      phase,
    )
  }

  fn mc_chroma(
    plane: Plane<'_>,
    region: Region,
    phase: Point<u32>,
    stride: u64,
    out: &mut [u8],
  ) -> Result<(), Never> {
    let mut input = [0; 81];
    let source_stride = 9;
    let size = Size {
      width: region.size.width.wrapping_add(1),
      height: region.size.height.wrapping_add(1),
    };
    let Ok(()) = fetch(
      plane,
      region.origin,
      BlockLayout {
        size,
        source_stride: 0,
        destination_stride: source_stride,
      },
      &mut input,
    );
    mc_chroma_win(
      &input,
      out,
      BlockLayout {
        size: region.size,
        source_stride,
        destination_stride: stride,
      },
      phase,
    )
  }

  fn reference_taps(samples: [i32; 6]) -> Result<i32, Never> {
    use core::num::Wrapping as W;
    let [a, b, c, d, e, f] = samples.map(W);
    Ok((a - W(5) * b + W(20) * c + W(20) * d - W(5) * e + f).0)
  }

  fn reference_horizontal(plane: Plane<'_>, at: Point<i32>) -> Result<i32, Never> {
    let stripe = [-2, -1, 0, 1, 2, 3].map(|x| {
      let Ok(pixel) = plane.sample(Point {
        x: at.x.wrapping_add(x),
        y: at.y,
      });
      pixel
    });
    reference_taps(stripe)
  }

  fn reference_vertical(plane: Plane<'_>, at: Point<i32>) -> Result<i32, Never> {
    let stripe = [-2, -1, 0, 1, 2, 3].map(|y| {
      let Ok(pixel) = plane.sample(Point {
        x: at.x,
        y: at.y.wrapping_add(y),
      });
      pixel
    });
    reference_taps(stripe)
  }

  fn reference_diagonal(plane: Plane<'_>, at: Point<i32>) -> Result<i32, Never> {
    let stripes = [-2, -1, 0, 1, 2, 3].map(|y| {
      let Ok(tap) = reference_horizontal(
        plane,
        Point {
          x: at.x,
          y: at.y.wrapping_add(y),
        },
      );
      tap
    });
    let Ok(filtered) = reference_taps(stripes);
    Ok(filtered.wrapping_add(512).wrapping_shr(10).clamp(0, 255))
  }

  fn reference_sample(plane: Plane<'_>, at: Point<i32>, phase: Point<u32>) -> Result<u8, Never> {
    let x = at.x.wrapping_add(i32::from(phase.x == 3));
    let y = at.y.wrapping_add(i32::from(phase.y == 3));
    let Ok(horizontal_raw) = reference_horizontal(plane, Point { x: at.x, y });
    let Ok(vertical_raw) = reference_vertical(plane, Point { x, y: at.y });
    let horizontal = horizontal_raw
      .wrapping_add(16)
      .wrapping_shr(5)
      .clamp(0, 255);
    let vertical = vertical_raw.wrapping_add(16).wrapping_shr(5).clamp(0, 255);
    let mean = |pair: [i32; 2]| {
      let [first, second] = pair;
      first.wrapping_add(second).wrapping_add(1).wrapping_shr(1)
    };
    let sample = match (phase.x, phase.y) {
      (0, 0) => {
        let Ok(pixel) = plane.sample(at);
        pixel
      }
      (2, 0) => horizontal,
      (0, 2) => vertical,
      (_, 0) => {
        let Ok(pixel) = plane.sample(Point { x, y: at.y });
        mean([pixel, horizontal])
      }
      (0, _) => {
        let Ok(pixel) = plane.sample(Point { x: at.x, y });
        mean([pixel, vertical])
      }
      (2, 2) => {
        let Ok(diagonal) = reference_diagonal(plane, at);
        diagonal
      }
      (2, _) => {
        let Ok(diagonal) = reference_diagonal(plane, at);
        mean([diagonal, horizontal])
      }
      (_, 2) => {
        let Ok(diagonal) = reference_diagonal(plane, at);
        mean([diagonal, vertical])
      }
      _ => mean([horizontal, vertical]),
    };
    wrapped(sample)
  }

  fn reference_block(
    plane: Plane<'_>,
    region: Region,
    phase: Point<u32>,
    out: &mut [u8],
  ) -> Result<(), Never> {
    let layout = BlockLayout {
      size: region.size,
      source_stride: 0,
      destination_stride: 16,
    };

    for row in 0..region.size.height {
      let Ok(line) = output_row(out, row, layout);
      let Ok(offset_y) = wrapped::<_, i32>(row);

      for (x, pixel) in (0u64..).zip(line) {
        let Ok(offset_x) = wrapped::<_, i32>(x);
        let at = Point {
          x: region.origin.x.wrapping_add(offset_x),
          y: region.origin.y.wrapping_add(offset_y),
        };
        let Ok(value) = reference_sample(plane, at, phase);
        *pixel = value;
      }
    }

    Ok(())
  }

  #[test]
  fn full_sample_copy_and_clamp() {
    let mut samples = [0; 256];

    for (value, position) in samples.iter_mut().zip(0u8..=255) {
      *value = position;
    }

    let plane = Plane {
      samples: &samples,
      size: Size {
        width: 16,
        height: 16,
      },
      stride: 16,
    };
    let size = Size {
      width: 4,
      height: 4,
    };
    let phase = Point { x: 0, y: 0 };
    let mut out = [0; 16];
    let Ok(()) = mc_luma(
      plane,
      Region {
        origin: Point { x: 2, y: 3 },
        size,
      },
      phase,
      4,
      &mut out,
    );
    assert_eq!(out.first_chunk::<4>(), Some(&[50, 51, 52, 53]));
    let Ok(()) = mc_luma(
      plane,
      Region {
        origin: Point { x: -40, y: -40 },
        size,
      },
      phase,
      4,
      &mut out,
    );
    assert_eq!(out, [0; 16]);
  }

  #[test]
  fn half_sample_constant_is_constant() {
    let samples = [100; 1024];
    let plane = Plane {
      samples: &samples,
      size: Size {
        width: 32,
        height: 32,
      },
      stride: 32,
    };
    let region = Region {
      origin: Point { x: 8, y: 8 },
      size: Size {
        width: 8,
        height: 8,
      },
    };

    for x in 0u32..4 {
      for y in 0u32..4 {
        let mut out = [0; 64];
        let Ok(()) = mc_luma(plane, region, Point { x, y }, 8, &mut out);
        assert_eq!(out, [100; 64], "{x},{y}");
        let Ok(()) = mc_chroma(
          plane,
          region,
          Point {
            x: x.wrapping_mul(2),
            y: y.wrapping_mul(2),
          },
          8,
          &mut out,
        );
        assert_eq!(out, [100; 64]);
      }
    }
  }

  #[test]
  fn half_sample_horizontal_ramp() {
    let mut samples = [0; 1024];

    for line in samples.as_chunks_mut::<32>().0 {
      for (pixel, column) in line.iter_mut().zip(0u8..32) {
        *pixel = column.wrapping_mul(4);
      }
    }

    let plane = Plane {
      samples: &samples,
      size: Size {
        width: 32,
        height: 32,
      },
      stride: 32,
    };
    let region = Region {
      origin: Point { x: 8, y: 8 },
      size: Size {
        width: 4,
        height: 4,
      },
    };
    let mut out = [0; 16];
    let Ok(()) = mc_luma(plane, region, Point { x: 2, y: 0 }, 4, &mut out);
    assert_eq!(out.first_chunk::<4>(), Some(&[34, 38, 42, 46]));
    let Ok(()) = mc_luma(plane, region, Point { x: 1, y: 0 }, 4, &mut out);
    assert_eq!(out.first_chunk::<4>(), Some(&[33, 37, 41, 45]));
  }

  fn compare_phases(plane: Plane<'_>, size: Size<u64>) -> Result<(), Never> {
    for fx in 0u32..4 {
      for fy in 0u32..4 {
        let phase = Point { x: fx, y: fy };

        for (x, y) in [(10, 9), (-3, -5), (30, 30), (0, 0), (2, 2), (38, 1)] {
          let region = Region {
            origin: Point { x, y },
            size,
          };
          let mut fast = [0; 256];
          let mut scalar = [0; 256];
          let Ok(()) = mc_luma(plane, region, phase, 16, &mut fast);
          let Ok(()) = reference_block(plane, region, phase, &mut scalar);
          assert_eq!(fast, scalar, "{size:?} phase {phase:?} at {x},{y}");
        }
      }
    }

    Ok(())
  }

  #[test]
  fn fast_luma_matches_reference() {
    let mut seed = 7u32;
    let mut samples = [0; 1440];

    for pixel in &mut samples {
      seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
      let Ok(value) = wrapped::<_, u8>(seed.wrapping_shr(24));
      *pixel = value;
    }

    let plane = Plane {
      samples: &samples,
      size: Size {
        width: 40,
        height: 36,
      },
      stride: 40,
    };

    for (width, height) in [(16, 16), (16, 8), (8, 16), (8, 8), (8, 4), (4, 8), (4, 4)] {
      let Ok(()) = compare_phases(plane, Size { width, height });
    }
  }

  #[test]
  fn weighting() {
    let a = [100u8; 4];
    let b = [50u8; 4];
    let mut d = [0u8; 4];
    let Ok(()) = weighted_store(
      &mut d,
      [Some(&a), Some(&b)],
      BlockLayout {
        size: Size {
          width: 4,
          height: 1,
        },
        source_stride: 4,
        destination_stride: 4,
      },
      Weight::Default,
    );
    assert_eq!(d, [75; 4]);
    let Ok(()) = weighted_store(
      &mut d,
      [Some(&a), None],
      BlockLayout {
        size: Size {
          width: 4,
          height: 1,
        },
        source_stride: 4,
        destination_stride: 4,
      },
      Weight::Weighted {
        log_wd: 5,
        w0: 16,
        w1: 0,
        o0: 3,
        o1: 0,
      },
    );
    assert_eq!(d, [53; 4]);
  }
}
