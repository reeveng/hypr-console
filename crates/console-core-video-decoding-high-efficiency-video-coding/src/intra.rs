//! Intra sample prediction (8.4.4.2): reference substitution, filtering (including strong
//! intra smoothing), planar, DC and the 33 angular modes.

use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};

use crate::params::PictureTool;
use crate::places::{read, window_mut, write};
use crate::spec_tables::{INTRA_PRED_ANGLE, INV_ANGLE};

#[derive(Clone)]
pub struct Refs {
  pub left: [u16; 129],
  pub top: [u16; 129],
}

impl Default for Refs {
  fn default() -> Self {
    Self {
      left: [0; 129],
      top: [0; 129],
    }
  }
}

#[derive(Clone, Copy)]
pub struct Block {
  pub side: u64,
  pub bit_depth: u32,
}

pub struct Available<'a> {
  pub left: &'a [bool; 129],
  pub top: &'a [bool; 129],
}

pub fn substitute(refs: &mut Refs, available: Available<'_>, block: Block) -> Result<(), Never> {
  let end = block.side.wrapping_mul(2).min(128).wrapping_add(1);
  let Ok(left) = window_mut(&mut refs.left, 0..end);
  let Ok(top) = window_mut(&mut refs.top, 0..end);
  let Ok(limit) = index(end);
  let left_available = available.left.iter().take(limit);
  let top_available = available.top.iter().take(limit);
  let first = left
    .iter()
    .zip(left_available.clone())
    .rev()
    .chain(top.iter().zip(top_available.clone()).skip(1))
    .find_map(|(&sample, &present)| present.then_some(sample));

  let mut previous = match first {
    Some(sample) => sample,
    None => {
      let middle = 1u16.wrapping_shl(block.bit_depth.wrapping_sub(1));
      left.fill(middle);
      top.fill(middle);
      return Ok(());
    }
  };

  for (sample, &present) in left.iter_mut().zip(left_available).rev() {
    match present.then_some(()) {
      Some(()) => previous = *sample,
      None => *sample = previous,
    }
  }

  let Ok(()) = write(top, 0, previous);

  for (sample, &present) in top.iter_mut().zip(top_available).skip(1) {
    match present.then_some(()) {
      Some(()) => previous = *sample,
      None => *sample = previous,
    }
  }

  Ok(())
}

pub fn filter(
  refs: &mut Refs,
  mode: u32,
  block: Block,
  strong: PictureTool,
) -> Result<(), Never> {
  match (mode, block.side) {
    (1, _) | (_, 4) => return Ok(()),
    _ => {}
  }

  let distance = mode.abs_diff(26).min(mode.abs_diff(10));
  let threshold = match block.side {
    8 => 7,
    16 => 1,
    _ => 0,
  };

  match (distance <= threshold).then_some(()) {
    Some(()) => return Ok(()),
    None => {}
  }

  let Ok(corner) = read(&refs.left, 0);
  let end = block.side.wrapping_mul(2).min(128);
  let threshold_2 = 1i32.wrapping_shl(block.bit_depth.wrapping_sub(5));

  match (strong, block.side) {
    (PictureTool::Enabled, 32) => {
      let Ok(top) = curvature(&refs.top, corner);
      let Ok(left) = curvature(&refs.left, corner);

      match (top < threshold_2 && left < threshold_2).then_some(()) {
        Some(()) => {
          let Ok(()) = interpolate_edge(&mut refs.top, corner);
          return interpolate_edge(&mut refs.left, corner);
        }
        None => {}
      }
    }
    _ => {}
  }

  let [_, left, ..] = refs.left;
  let [_, top, ..] = refs.top;
  let Ok(new_corner) = filtered_sample([left, corner, top]);
  let Ok(()) = filter_edge(&mut refs.left, end);
  let Ok(()) = filter_edge(&mut refs.top, end);
  let Ok(()) = write(&mut refs.left, 0, new_corner);
  write(&mut refs.top, 0, new_corner)
}

fn curvature(edge: &[u16; 129], corner: u16) -> Result<i32, Never> {
  let Ok(start) = read(edge, 32);
  let Ok(end) = read(edge, 64);
  Ok(
    i32::from(corner)
      .wrapping_add(i32::from(end))
      .wrapping_sub(i32::from(start).wrapping_mul(2))
      .abs(),
  )
}

fn interpolate_edge(edge: &mut [u16; 129], corner: u16) -> Result<(), Never> {
  let Ok(far) = read(edge, 64);
  let a = i32::from(corner);
  let b = i32::from(far);

  for i in 1i32..64 {
    let mid = 64i32
      .wrapping_sub(i)
      .wrapping_mul(a)
      .wrapping_add(i.wrapping_mul(b))
      .wrapping_add(32)
      .wrapping_shr(6);
    let Ok(position) = wrapped::<_, u64>(i);
    let Ok(sample) = wrapped::<_, u16>(mid);
    let Ok(()) = write(edge, position, sample);
  }

  Ok(())
}

fn filter_edge(edge: &mut [u16; 129], end: u64) -> Result<(), Never> {
  let original = *edge;
  let Ok(samples) = window_mut(edge, 1..end);

  for (output, neighborhood) in samples.iter_mut().zip(original.windows(3)) {
    let triplet = match neighborhood {
      &[a, b, c] => [a, b, c],
      _ => continue,
    };
    let Ok(sample) = filtered_sample(triplet);
    *output = sample;
  }

  Ok(())
}

fn filtered_sample([first, middle, last]: [u16; 3]) -> Result<u16, Never> {
  let total = u32::from(first)
    .wrapping_add(u32::from(middle).wrapping_mul(2))
    .wrapping_add(u32::from(last))
    .wrapping_add(2);
  wrapped(total.wrapping_shr(2))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PredictionEdges {
  Filtered,
  Unfiltered,
}

pub struct Prediction {
  pub mode: u32,
  pub block: Block,
  pub edges: PredictionEdges,
}

pub struct Surface<'a> {
  pub samples: &'a mut [u16],
  pub stride: u64,
}

pub fn predict(refs: &Refs, mode: Prediction, mut output: Surface<'_>) -> Result<(), Never> {
  match mode.mode {
    0 => planar(refs, mode.block, output),
    1 => dc(refs, mode, output),
    direction => {
      let Ok(()) = angular(refs, mode.block, direction, &mut output);
      filter_prediction_edge(refs, mode, output)
    }
  }
}

fn planar(refs: &Refs, block: Block, output: Surface<'_>) -> Result<(), Never> {
  let n = block.side.min(64);
  let shift = n.trailing_zeros().wrapping_add(1);
  let Ok(right) = read(&refs.top, n.wrapping_add(1));
  let Ok(bottom) = read(&refs.left, n.wrapping_add(1));
  let Ok(size) = wrapped::<_, i32>(n);

  for y in 0..n {
    let Ok(left) = read(&refs.left, y.wrapping_add(1));
    let row_base = y.wrapping_mul(output.stride);
    let Ok(row) = window_mut(output.samples, row_base..row_base.wrapping_add(n));
    let Ok(row_weight) = wrapped::<_, i32>(y.wrapping_add(1));
    let vertical_limit = row_weight.wrapping_mul(i32::from(bottom));

    for (x, sample) in (0u64..).zip(row) {
      let Ok(top) = read(&refs.top, x.wrapping_add(1));
      let Ok(column_weight) = wrapped::<_, i32>(x.wrapping_add(1));
      let horizontal = size
        .wrapping_sub(column_weight)
        .wrapping_mul(i32::from(left))
        .wrapping_add(column_weight.wrapping_mul(i32::from(right)));
      let vertical = size
        .wrapping_sub(row_weight)
        .wrapping_mul(i32::from(top))
        .wrapping_add(vertical_limit);
      let Ok(pixel) = wrapped::<_, u16>(
        horizontal
          .wrapping_add(vertical)
          .wrapping_add(size)
          .wrapping_shr(shift),
      );
      *sample = pixel;
    }
  }

  Ok(())
}

fn dc(refs: &Refs, mode: Prediction, output: Surface<'_>) -> Result<(), Never> {
  let n = mode.block.side.min(64);
  let Ok(count) = index(n);
  let samples = refs
    .top
    .iter()
    .skip(1)
    .take(count)
    .chain(refs.left.iter().skip(1).take(count));
  let sum = samples.fold(n, |sum, &sample| sum.wrapping_add(u64::from(sample)));
  let Ok(dc) = wrapped::<_, u16>(sum.wrapping_shr(n.trailing_zeros().wrapping_add(1)));

  for row in 0..n {
    let start = row.wrapping_mul(output.stride);
    let Ok(line) = window_mut(output.samples, start..start.wrapping_add(n));
    line.fill(dc);
  }

  match (mode.edges, n) {
    (PredictionEdges::Filtered, ..=31) => filter_dc_edges(refs, n, dc, output),
    _ => Ok(()),
  }
}

fn filter_dc_edges(refs: &Refs, n: u64, dc: u16, output: Surface<'_>) -> Result<(), Never> {
  let [_, left, ..] = refs.left;
  let [_, top, ..] = refs.top;
  let Ok(corner) = filtered_sample([left, dc, top]);
  let Ok(()) = write(output.samples, 0, corner);

  for i in 1..n {
    let Ok(top_2) = read(&refs.top, i.wrapping_add(1));
    let Ok(left_2) = read(&refs.left, i.wrapping_add(1));
    let Ok(a) = filtered_sample([top_2, dc, dc]);
    let Ok(b) = filtered_sample([left_2, dc, dc]);
    let Ok(()) = write(output.samples, i, a);
    let Ok(()) = write(output.samples, i.wrapping_mul(output.stride), b);
  }

  Ok(())
}

enum AngularAxis {
  Vertical,
  Horizontal,
}

struct AngularReference {
  direction: u32,
  angle: i32,
  size: i32,
  edges: AngularAxis,
}

impl AngularReference {
  fn values(&self, refs: &Refs) -> Result<[i32; 193], Never> {
    let (main, side) = match self.edges {
      AngularAxis::Vertical => (&refs.top, &refs.left),
      AngularAxis::Horizontal => (&refs.left, &refs.top),
    };
    let mut samples = [0; 193];
    let Ok(base) = wrapped::<_, u64>(self.size);
    let length = match self.angle {
      ..=-1 => base,
      _ => base.wrapping_mul(2),
    };
    let Ok(take) = index(length.wrapping_add(1));

    for (x, &sample) in (0u64..).zip(main.iter().take(take)) {
      let Ok(()) = write(&mut samples, base.wrapping_add(x), i32::from(sample));
    }

    let limit = self.size.wrapping_mul(self.angle).wrapping_shr(5);

    match limit {
      ..=-2 => {
        let Ok(factor) = read(&INV_ANGLE, u64::from(self.direction.wrapping_sub(11)));

        for x in limit..=-1 {
          let projected = x
            .wrapping_mul(i32::from(factor))
            .wrapping_add(128)
            .wrapping_shr(8);
          let Ok(pos) = wrapped::<_, u64>(projected);
          let Ok(value) = read(side, pos);
          let Ok(destination) = wrapped::<_, u64>(self.size.wrapping_add(x));
          let Ok(()) = write(&mut samples, destination, i32::from(value));
        }
      }
      _ => {}
    }

    Ok(samples)
  }
}

fn angular(
  refs: &Refs,
  block: Block,
  direction: u32,
  output: &mut Surface<'_>,
) -> Result<(), Never> {
  let n = block.side.min(64);
  let Ok(size) = wrapped::<_, i32>(n);
  let Ok(angle) = read(&INTRA_PRED_ANGLE, u64::from(direction));
  let edges = match direction {
    ..=17 => AngularAxis::Horizontal,
    _ => AngularAxis::Vertical,
  };
  let layout = AngularReference {
    direction,
    angle: i32::from(angle),
    size,
    edges,
  };
  let Ok(samples) = layout.values(refs);
  let mut offset = i32::from(angle);

  for j in 0..n {
    let start = size.wrapping_add(offset.wrapping_shr(5)).wrapping_add(1);
    let phase = offset & 31;
    let Ok(base) = wrapped::<_, u64>(start);

    for i in 0..n {
      let at = base.wrapping_add(i);
      let Ok(first) = read(&samples, at);
      let value = match phase {
        0 => first,
        _ => {
          let Ok(second) = read(&samples, at.wrapping_add(1));
          32i32
            .wrapping_sub(phase)
            .wrapping_mul(first)
            .wrapping_add(phase.wrapping_mul(second))
            .wrapping_add(16)
            .wrapping_shr(5)
        }
      };
      let (x, y) = match layout.edges {
        AngularAxis::Vertical => (i, j),
        AngularAxis::Horizontal => (j, i),
      };
      let Ok(sample) = wrapped::<_, u16>(value);
      let Ok(()) = write(
        output.samples,
        y.wrapping_mul(output.stride).wrapping_add(x),
        sample,
      );
    }

    offset = offset.wrapping_add(i32::from(angle));
  }

  Ok(())
}

fn filter_prediction_edge(
  refs: &Refs,
  mode: Prediction,
  output: Surface<'_>,
) -> Result<(), Never> {
  let (edge, perpendicular, step) = match (mode.edges, mode.block.side, mode.mode) {
    (PredictionEdges::Filtered, ..=31, 26) => (&refs.top, &refs.left, output.stride),
    (PredictionEdges::Filtered, ..=31, 10) => (&refs.left, &refs.top, 1),
    _ => return Ok(()),
  };
  let [_, boundary, ..] = *edge;
  let [corner, ..] = *perpendicular;
  let maximum = 1i32
    .wrapping_shl(mode.block.bit_depth)
    .wrapping_sub(1)
    .max(0);
  let Ok(take) = index(mode.block.side);

  for (position, &sample) in (0u64..).zip(perpendicular.iter().skip(1).take(take)) {
    let adjust = i32::from(sample)
      .wrapping_sub(i32::from(corner))
      .wrapping_shr(1);
    let weighted = i32::from(boundary).wrapping_add(adjust).clamp(0, maximum);
    let Ok(value) = wrapped::<_, u16>(weighted);
    let Ok(()) = write(output.samples, position.wrapping_mul(step), value);
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn refs(f: impl Fn(i32) -> u16) -> Result<Refs, Never> {
    let mut output = Refs::default();

    for (i, (left, top)) in (0i32..).zip(output.left.iter_mut().zip(&mut output.top)) {
      *left = f(i.wrapping_neg());
      *top = f(i);
    }

    Ok(output)
  }

  fn predicted(
    refs: &Refs,
    direction: u32,
    target: Surface<'_>,
    edges: PredictionEdges,
  ) -> Result<(), Never> {
    let block = Block {
      side: target.stride,
      bit_depth: 8,
    };
    predict(
      refs,
      Prediction {
        mode: direction,
        block,
        edges,
      },
      target,
    )
  }

  #[test]
  fn dc_and_planar_flat() {
    let Ok(r) = refs(|_| 100);
    let mut out = [0u16; 64];

    for mode in [0, 1] {
      let Ok(()) = predicted(
        &r,
        mode,
        Surface {
          samples: &mut out,
          stride: 8,
        },
        PredictionEdges::Filtered,
      );
      assert_eq!(out, [100; 64]);
    }
  }

  #[test]
  fn pure_vertical_and_horizontal() {
    let Ok(r) = refs(|i| {
      let Ok(value) = wrapped::<_, u16>(128i32.wrapping_add(i));
      value
    });
    let mut out = [0u16; 16];
    let Ok(()) = predicted(
      &r,
      26,
      Surface {
        samples: &mut out,
        stride: 4,
      },
      PredictionEdges::Unfiltered,
    );

    for row in out.chunks_exact(4) {
      assert_eq!(row, [129, 130, 131, 132]);
    }

    let Ok(()) = predicted(
      &r,
      10,
      Surface {
        samples: &mut out,
        stride: 4,
      },
      PredictionEdges::Unfiltered,
    );

    for (row, value) in out.chunks_exact(4).zip([127, 126, 125, 124]) {
      assert_eq!(row, [value; 4]);
    }

    let Ok(()) = predicted(
      &r,
      34,
      Surface {
        samples: &mut out,
        stride: 4,
      },
      PredictionEdges::Unfiltered,
    );
    let Ok(diagonal) = read(&out, 5);
    assert_eq!(diagonal, 132);
  }

  #[test]
  fn substitution() {
    let mut r = Refs::default();
    let mut available = [false; 129];
    let Ok(()) = write(&mut r.left, 3, 77);
    let Ok(()) = write(&mut available, 3, true);
    let Ok(()) = substitute(
      &mut r,
      Available {
        left: &available,
        top: &[false; 129],
      },
      Block {
        side: 4,
        bit_depth: 8,
      },
    );
    assert!(r.left.iter().take(9).all(|&v| v == 77));
    assert!(r.top.iter().take(9).all(|&v| v == 77));

    let mut empty = Refs::default();
    let Ok(()) = substitute(
      &mut empty,
      Available {
        left: &[false; 129],
        top: &[false; 129],
      },
      Block {
        side: 4,
        bit_depth: 10,
      },
    );
    let Ok(sample) = read(&empty.top, 5);
    assert_eq!(sample, 512);
  }
}
