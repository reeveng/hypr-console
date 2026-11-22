//! Intra prediction (8.3): Intra_4x4, Intra_8x8 (with reference filtering), Intra_16x16 and chroma.

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};

#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "The four neighboring positions are checked independently for frame edges, slice boundaries, and constrained intra prediction"
  )
)]
pub struct Avail {
  pub left: bool,
  pub top: bool,
  pub top_left: bool,
  pub top_right: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct IntraBlock {
  pub(crate) stride: u64,
  pub(crate) origin: Point<u64>,
  pub(crate) mode: u8,
  pub(crate) available: Avail,
}

#[derive(Clone, Copy)]
struct Edge {
  top: [i32; 16],
  left: [i32; 16],
  tl: i32,
}

impl Edge {
  #[inline(always)]
  fn t(&self, column: i32) -> core::result::Result<i32, Never> {
    edge_sample(
      &self.top,
      self::EdgeSampleArguments {
        position: column,
        corner: self.tl,
      },
    )
  }

  #[inline(always)]
  fn l(&self, row: i32) -> core::result::Result<i32, Never> {
    edge_sample(
      &self.left,
      self::EdgeSampleArguments {
        position: row,
        corner: self.tl,
      },
    )
  }
}

#[inline(always)]
fn edge_sample(
  edge: &[i32; 16],
  EdgeSampleArguments { position, corner }: EdgeSampleArguments,
) -> core::result::Result<i32, Never> {
  match position {
    ..=-1 => Ok(corner),
    0.. => {
      let Ok(native) = index(position);
      Ok(match edge.get(native) {
        Some(sample) => *sample,
        None => 128,
      })
    }
  }
}

struct EdgeSampleArguments {
  position: i32,
  corner: i32,
}

fn neighbor(plane: &[u8], offset: u64, fallback: i32) -> core::result::Result<i32, Never> {
  let Ok(position) = index(offset);
  Ok(match plane.get(position) {
    Some(pixel) => i32::from(*pixel),
    None => fallback,
  })
}

#[derive(Clone, Copy)]
enum IntraSize {
  Four,
  Eight,
  Sixteen,
  Chroma,
}

impl IntraSize {
  fn width(self) -> core::result::Result<u64, Never> {
    Ok(match self {
      Self::Four => 4,
      Self::Eight | Self::Chroma => 8,
      Self::Sixteen => 16,
    })
  }
}

fn load_edge(
  plane: &[u8],
  block: IntraBlock,
  size: IntraSize,
) -> core::result::Result<Edge, Never> {
  let Ok(width) = size.width();
  let missing = match size {
    IntraSize::Four | IntraSize::Eight => 128,
    IntraSize::Sixteen | IntraSize::Chroma => 0,
  };
  let mut edge = Edge {
    top: [missing; 16],
    left: [missing; 16],
    tl: 128,
  };
  let IntraBlock {
    stride,
    origin: Point { x, y },
    available,
    ..
  } = block;
  let top = y.wrapping_sub(1).wrapping_mul(stride).wrapping_add(x);

  match available.top.then_some(()) {
    Some(()) => {
      let mut last = missing;
      let limit = match size {
        IntraSize::Four | IntraSize::Eight => width.wrapping_mul(2),
        IntraSize::Sixteen | IntraSize::Chroma => width,
      };

      for (column, target) in (0..limit).zip(&mut edge.top) {
        match ::core::primitive::bool::then_some(column < width || available.top_right, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(sample) = neighbor(plane, top.wrapping_add(column), last);
            last = sample;
          }
          ::core::option::Option::None => {}
        }

        *target = last;
      }
    }
    None => {}
  }

  match available.left.then_some(()) {
    Some(()) => {
      for (row, target) in (0..width).zip(&mut edge.left) {
        let offset = y
          .wrapping_add(row)
          .wrapping_mul(stride)
          .wrapping_add(x)
          .wrapping_sub(1);
        let Ok(sample) = neighbor(plane, offset, missing);
        *target = sample;
      }
    }
    None => {}
  }

  match available.top_left.then_some(()) {
    Some(()) => {
      let Ok(value) = neighbor(plane, top.wrapping_sub(1), 128);
      edge.tl = value;
    }
    None => {}
  }

  Ok(edge)
}

#[inline(always)]
fn put_sample(
  out: &mut [u8],
  stride: u64,
  position: Point<i32>,
  value: i32,
) -> core::result::Result<(), Never> {
  let Ok(x) = wrapped::<_, u64>(position.x);
  let Ok(y) = wrapped::<_, u64>(position.y);
  let Ok(offset) = index(y.wrapping_mul(stride).wrapping_add(x));
  let Ok(pixel) = wrapped::<_, u8>(value);

  match out.get_mut(offset) {
    Some(sample) => *sample = pixel,
    None => {}
  }

  Ok(())
}

fn vertical_right(edge: &Edge, point: Point<i32>) -> Result<i32, Never> {
  let Point { x, y } = point;
  let displacement = x.wrapping_mul(2).wrapping_sub(y);
  let column = x.wrapping_sub(y.wrapping_shr(1));

  match (displacement, displacement & 1) {
    (0.., 0) => {
      let Ok(left) = edge.t(column.wrapping_sub(1));
      let Ok(right) = edge.t(column);
      Ok(left.wrapping_add(right).wrapping_add(1).wrapping_shr(1))
    }
    (1.., _) => {
      let Ok(left) = edge.t(column.wrapping_sub(2));
      let Ok(center) = edge.t(column.wrapping_sub(1));
      let Ok(right) = edge.t(column);
      smooth([left, center, right])
    }
    (-1, _) => {
      let Ok(left) = edge.l(0);
      let Ok(top) = edge.t(0);
      smooth([left, edge.tl, top])
    }
    _ => {
      let right = displacement.wrapping_neg();
      let Ok(first) = edge.l(right.wrapping_sub(3));
      let Ok(center) = edge.l(right.wrapping_sub(2));
      let Ok(last) = edge.l(right.wrapping_sub(1));
      smooth([first, center, last])
    }
  }
}

fn horizontal_up(edge: &Edge, location: Point<i32>, side: i32) -> Result<i32, Never> {
  let Point { x, y } = location;
  let threshold = side.wrapping_mul(2).wrapping_sub(3);
  let distance = x.wrapping_add(y.wrapping_mul(2));
  let row = y.wrapping_add(x.wrapping_shr(1));

  match distance.cmp(&threshold) {
    core::cmp::Ordering::Less => {
      let Ok(first) = edge.l(row);
      let Ok(second) = edge.l(row.wrapping_add(1));

      match distance & 1 {
        0 => Ok(first.wrapping_add(second).wrapping_add(1).wrapping_shr(1)),
        _ => {
          let Ok(third) = edge.l(row.wrapping_add(2));
          smooth([first, second, third])
        }
      }
    }
    core::cmp::Ordering::Equal => {
      let Ok(previous) = edge.l(side.wrapping_sub(2));
      let Ok(last) = edge.l(side.wrapping_sub(1));
      smooth([previous, last, last])
    }
    core::cmp::Ordering::Greater => edge.l(side.wrapping_sub(1)),
  }
}

fn diagonal_down_right(edge: &Edge, at: Point<i32>) -> Result<i32, Never> {
  let Point { x, y } = at;
  let samples = match x.cmp(&y) {
    core::cmp::Ordering::Greater => {
      let offset = x.wrapping_sub(y);
      [offset.wrapping_sub(2), offset.wrapping_sub(1), offset].map(|position| {
        let Ok(sample) = edge.t(position);
        sample
      })
    }
    core::cmp::Ordering::Less => {
      let offset = y.wrapping_sub(x);
      [offset.wrapping_sub(2), offset.wrapping_sub(1), offset].map(|position| {
        let Ok(sample) = edge.l(position);
        sample
      })
    }
    core::cmp::Ordering::Equal => {
      let Ok(top) = edge.t(0);
      let Ok(left) = edge.l(0);
      [top, edge.tl, left]
    }
  };
  smooth(samples)
}

fn vertical_left(edge: &Edge, location: Point<i32>) -> Result<i32, Never> {
  let Point { x, y } = location;
  let start = x.wrapping_add(y.wrapping_shr(1));
  let Ok(first) = edge.t(start);
  let Ok(center) = edge.t(start.wrapping_add(1));

  match y & 1 {
    0 => Ok(first.wrapping_add(center).wrapping_add(1).wrapping_shr(1)),
    _ => {
      let Ok(last) = edge.t(start.wrapping_add(2));
      smooth([first, center, last])
    }
  }
}

fn pred_nxn(
  mode: u8,
  e: &Edge,
  n: i32,
  a: Avail,
  out: &mut [u8],
  stride: u64,
) -> core::result::Result<(), console_core_never::Never> {
  match mode {
    0 => {
      for y in 0..n {
        for x in 0..n {
          let Ok(()) = put_sample(out, stride, Point { x, y }, {
            let Ok(explicit_value) = e.t(x);
            explicit_value
          });
        }
      }
    }
    1 => {
      for y in 0..n {
        for x in 0..n {
          let Ok(()) = put_sample(out, stride, Point { x, y }, {
            let Ok(explicit_value) = e.l(y);
            explicit_value
          });
        }
      }
    }
    2 => {
      let shift = match ::core::primitive::bool::then_some(n == 4, ()) {
        ::core::option::Option::Some(()) => 2,
        ::core::option::Option::None => 3,
      };
      let st: i32 = (0..n)
        .map(|x| {
          let Ok(explicit_value) = e.t(x);
          explicit_value
        })
        .sum();
      let sl: i32 = (0..n)
        .map(|y| {
          let Ok(explicit_value) = e.l(y);
          explicit_value
        })
        .sum();
      let dc = match (a.top, a.left) {
        (true, true) => i32::wrapping_shr(i32::wrapping_add(i32::wrapping_add(st, sl), n), {
          let Ok(numeric_value) =
            console_core_number_conversion::wrapped::<_, u32>(i32::wrapping_add(shift, 1));
          numeric_value
        }),
        (false, true) => i32::wrapping_shr(i32::wrapping_add(sl, i32::wrapping_shr(n, 1_u32)), {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(shift);
          numeric_value
        }),
        (true, false) => i32::wrapping_shr(i32::wrapping_add(st, i32::wrapping_shr(n, 1_u32)), {
          let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(shift);
          numeric_value
        }),
        _ => 128,
      };

      for y in 0..n {
        for x in 0..n {
          let Ok(()) = put_sample(out, stride, Point { x, y }, dc);
        }
      }
    }
    3 => {
      let end = n.wrapping_mul(2).wrapping_sub(1);

      for y in 0..n {
        for x in 0..n {
          let start = x.wrapping_add(y);
          let Ok(first) = e.t(start);
          let Ok(center) = e.t(start.wrapping_add(1));
          let Ok(last) = e.t(start.wrapping_add(2).min(end));
          let Ok(value) = smooth([first, center, last]);
          let Ok(()) = put_sample(out, stride, Point { x, y }, value);
        }
      }
    }
    4 => {
      for y in 0..n {
        for x in 0..n {
          let Ok(value) = diagonal_down_right(e, Point { x, y });
          let Ok(()) = put_sample(out, stride, Point { x, y }, value);
        }
      }
    }
    5 | 6 => {
      let references = match mode {
        5 => *e,
        _ => Edge {
          top: e.left,
          left: e.top,
          tl: e.tl,
        },
      };

      for y in 0..n {
        for x in 0..n {
          let location = match mode {
            5 => Point { x, y },
            _ => Point { x: y, y: x },
          };
          let Ok(value) = vertical_right(&references, location);
          let Ok(()) = put_sample(out, stride, Point { x, y }, value);
        }
      }
    }
    7 => {
      for y in 0..n {
        for x in 0..n {
          let Ok(value) = vertical_left(e, Point { x, y });
          let Ok(()) = put_sample(out, stride, Point { x, y }, value);
        }
      }
    }
    _ => {
      for y in 0..n {
        for x in 0..n {
          let Ok(v) = horizontal_up(e, Point { x, y }, n);
          let Ok(()) = put_sample(out, stride, Point { x, y }, v);
        }
      }
    }
  };

  Ok(())
}

pub fn pred4x4(plane: &mut [u8], block: IntraBlock) -> core::result::Result<(), Never> {
  let Ok(edge) = load_edge(plane, block, IntraSize::Four);
  predict_small(plane, block, &edge, 4)
}

pub fn pred8x8(plane: &mut [u8], block: IntraBlock) -> core::result::Result<(), Never> {
  let Ok(raw) = load_edge(plane, block, IntraSize::Eight);
  let Ok(edge) = filtered_edge(raw, block.available);
  predict_small(plane, block, &edge, 8)
}

fn smooth([previous, current, following]: [i32; 3]) -> core::result::Result<i32, Never> {
  Ok(
    previous
      .wrapping_add(current.wrapping_mul(2))
      .wrapping_add(following)
      .wrapping_add(2)
      .wrapping_shr(2),
  )
}

fn smooth_edge<const N: usize>(
  original: &[i32; N],
  corner: i32,
) -> core::result::Result<[i32; N], Never> {
  let mut source = *original;
  let mut previous = corner;

  for (target, (&current, following)) in source.iter_mut().zip(
    original
      .iter()
      .zip(original.iter().skip(1).copied().map(Some).chain([None])),
  ) {
    let next = match following {
      Some(next) => next,
      None => current,
    };
    let Ok(value) = smooth([previous, current, next]);
    *target = value;
    previous = current;
  }

  Ok(source)
}

fn filtered_edge(raw: Edge, available: Avail) -> core::result::Result<Edge, Never> {
  let mut edge = raw;
  let [top, ..] = raw.top;
  let [left, ..] = raw.left;
  let (top_prior, left_prior) = match ::core::primitive::bool::then_some(available.top_left, ()) {
    ::core::option::Option::Some(()) => (raw.tl, raw.tl),
    ::core::option::Option::None => (top, left),
  };

  match available.top.then_some(()) {
    Some(()) => {
      let Ok(filtered) = smooth_edge(&raw.top, top_prior);
      edge.top = filtered;
    }
    None => {}
  }

  match available.left.then_some(()) {
    Some(()) => {
      let [a, b, c, d, e, f, g, h, ..] = raw.left;
      let near = [a, b, c, d, e, f, g, h];
      let Ok(filtered) = smooth_edge(&near, left_prior);

      for (target, value) in edge.left.iter_mut().zip(filtered) {
        *target = value;
      }
    }
    None => {}
  }

  match (available.top_left, available.top, available.left) {
    (true, true, true) => {
      let Ok(corner) = smooth([top, raw.tl, left]);
      edge.tl = corner;
    }
    (true, true, false) => {
      let Ok(corner) = smooth([top, raw.tl, raw.tl]);
      edge.tl = corner;
    }
    (true, false, true) => {
      let Ok(corner) = smooth([raw.tl, raw.tl, left]);
      edge.tl = corner;
    }
    (true, false, false) | (false, _, _) => {}
  }

  Ok(edge)
}

fn predict_small(
  plane: &mut [u8],
  block: IntraBlock,
  edge: &Edge,
  size: i32,
) -> core::result::Result<(), Never> {
  let Ok(start) = index(
    block
      .origin
      .y
      .wrapping_mul(block.stride)
      .wrapping_add(block.origin.x),
  );

  match plane.get_mut(start..) {
    Some(out) => pred_nxn(block.mode, edge, size, block.available, out, block.stride),
    None => Ok(()),
  }
}

fn fill_block(
  plane: &mut [u8],
  block: IntraBlock,
  size: IntraSize,
  value: impl Fn(Point<i32>) -> core::result::Result<i32, Never>,
) -> core::result::Result<(), Never> {
  let Ok(width) = size.width();

  for row in 0..width {
    let start = block
      .origin
      .y
      .wrapping_add(row)
      .wrapping_mul(block.stride)
      .wrapping_add(block.origin.x);
    let Ok(from) = index(start);
    let Ok(to) = index(start.wrapping_add(width));
    let destination = match plane.get_mut(from..to) {
      Some(line) => line,
      None => return Ok(()),
    };
    let Ok(y) = wrapped::<_, i32>(row);

    for (column, sample) in (0..width).zip(destination) {
      let Ok(x) = wrapped::<_, i32>(column);
      let Ok(pixel) = value(Point { x, y });
      let Ok(clipped) = wrapped::<_, u8>(pixel.clamp(0, 255));
      *sample = clipped;
    }
  }

  Ok(())
}

fn plane_prediction(
  plane: &mut [u8],
  block: IntraBlock,
  edge: &Edge,
  size: IntraSize,
) -> core::result::Result<(), Never> {
  let (center, scale) = match size {
    IntraSize::Sixteen => (7_i32, 5_i32),
    IntraSize::Four | IntraSize::Eight | IntraSize::Chroma => (3, 34),
  };
  let mut horizontal = 0_i32;
  let mut vertical = 0_i32;

  for step in 1..=center.wrapping_add(1) {
    let low = center.wrapping_sub(step);
    let high = center.wrapping_add(step);
    let Ok(t0) = edge.t(low);
    let Ok(t1) = edge.t(high);
    let Ok(l0) = edge.l(low);
    let Ok(l1) = edge.l(high);
    horizontal = horizontal.wrapping_add(step.wrapping_mul(t1.wrapping_sub(t0)));
    vertical = vertical.wrapping_add(step.wrapping_mul(l1.wrapping_sub(l0)));
  }

  let edge_end = center.wrapping_mul(2).wrapping_add(1);
  let Ok(top) = edge.t(edge_end);
  let Ok(left) = edge.l(edge_end);
  let base = top.wrapping_add(left).wrapping_mul(16);
  let slope_x = scale
    .wrapping_mul(horizontal)
    .wrapping_add(32)
    .wrapping_shr(6);
  let slope_y = scale
    .wrapping_mul(vertical)
    .wrapping_add(32)
    .wrapping_shr(6);
  fill_block(plane, block, size, |position| {
    Ok(
      base
        .wrapping_add(slope_x.wrapping_mul(position.x.wrapping_sub(center)))
        .wrapping_add(slope_y.wrapping_mul(position.y.wrapping_sub(center)))
        .wrapping_add(16)
        .wrapping_shr(5),
    )
  })
}

pub fn pred16x16(plane: &mut [u8], block: IntraBlock) -> core::result::Result<(), Never> {
  let Ok(edge) = load_edge(plane, block, IntraSize::Sixteen);

  match block.mode {
    0 => fill_block(plane, block, IntraSize::Sixteen, |position| {
      edge.t(position.x)
    }),
    1 => fill_block(plane, block, IntraSize::Sixteen, |position| {
      edge.l(position.y)
    }),
    2 => {
      let top = edge.top.into_iter().fold(0_i32, i32::wrapping_add);
      let left = edge.left.into_iter().fold(0_i32, i32::wrapping_add);
      let dc = match (block.available.top, block.available.left) {
        (true, true) => top.wrapping_add(left).wrapping_add(16).wrapping_shr(5),
        (true, false) => top.wrapping_add(8).wrapping_shr(4),
        (false, true) => left.wrapping_add(8).wrapping_shr(4),
        (false, false) => 128,
      };
      fill_block(plane, block, IntraSize::Sixteen, |_| Ok(dc))
    }
    _ => plane_prediction(plane, block, &edge, IntraSize::Sixteen),
  }
}

fn chroma_dc(available: Avail, edge: &Edge) -> core::result::Result<[[i32; 2]; 2], Never> {
  let mut top = [0_i32; 2];
  let mut left = [0_i32; 2];

  for (sum, samples) in top.iter_mut().zip(edge.top.as_chunks::<4>().0) {
    *sum = samples.iter().copied().fold(0, i32::wrapping_add);
  }

  for (sum, samples) in left.iter_mut().zip(edge.left.as_chunks::<4>().0) {
    *sum = samples.iter().copied().fold(0, i32::wrapping_add);
  }

  let mut prediction = [[128; 2]; 2];

  for (y, (line, left_sum)) in (0_u8..2).zip(prediction.iter_mut().zip(left)) {
    for (x, (output, top_sum)) in (0_u8..2).zip(line.iter_mut().zip(top)) {
      *output = match (available.top, available.left, x, y) {
        (true, true, 0, 1) | (false, true, _, _) => left_sum.wrapping_add(2).wrapping_shr(2),
        (true, true, 1, 0) | (true, false, _, _) => top_sum.wrapping_add(2).wrapping_shr(2),
        (true, true, _, _) => top_sum
          .wrapping_add(left_sum)
          .wrapping_add(4)
          .wrapping_shr(3),
        (false, false, _, _) => 128,
      };
    }
  }

  Ok(prediction)
}

pub fn pred_chroma(plane: &mut [u8], block: IntraBlock) -> core::result::Result<(), Never> {
  let Ok(edge) = load_edge(plane, block, IntraSize::Chroma);

  match block.mode {
    0 => {
      let Ok([[top_left, top_right], [bottom_left, bottom_right]]) =
        chroma_dc(block.available, &edge);
      fill_block(plane, block, IntraSize::Chroma, |position| {
        Ok(match (position.x, position.y) {
          (..=3, ..=3) => top_left,
          (4.., ..=3) => top_right,
          (..=3, 4..) => bottom_left,
          (4.., 4..) => bottom_right,
        })
      })
    }
    1 => fill_block(plane, block, IntraSize::Chroma, |position| {
      edge.l(position.y)
    }),
    2 => fill_block(plane, block, IntraSize::Chroma, |position| {
      edge.t(position.x)
    }),
    _ => plane_prediction(plane, block, &edge, IntraSize::Chroma),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const ALL: Avail = Avail {
    left: true,
    top: true,
    top_left: true,
    top_right: true,
  };

  fn sample_block(mode: u8, available: Avail) -> Result<IntraBlock, Never> {
    Ok(IntraBlock {
      stride: 16,
      origin: Point { x: 4, y: 4 },
      mode,
      available,
    })
  }

  fn setup() -> Result<[u8; 256], Never> {
    let mut plane = [0; 256];

    for x in 0i32..12 {
      let value = 10i32.wrapping_mul(x.wrapping_add(1));
      let Ok(()) = put_sample(
        &mut plane,
        16,
        Point {
          x: x.wrapping_add(4),
          y: 3,
        },
        value,
      );
    }

    for y in 0i32..8 {
      let Ok(()) = put_sample(
        &mut plane,
        16,
        Point {
          x: 3,
          y: y.wrapping_add(4),
        },
        y.wrapping_add(1),
      );
    }

    let Ok(()) = put_sample(&mut plane, 16, Point { x: 3, y: 3 }, 5);
    Ok(plane)
  }

  fn check_sample(plane: &[u8], at: Point<u64>, expected: u8) -> Result<(), Never> {
    let Ok(position) = index(at.y.wrapping_mul(16).wrapping_add(at.x));
    assert_eq!(plane.get(position), Some(&expected));
    Ok(())
  }

  fn check_row<const N: usize>(
    plane: &[u8],
    at: Point<u64>,
    expected: [u8; N],
  ) -> Result<(), Never> {
    let Ok(position) = index(at.y.wrapping_mul(16).wrapping_add(at.x));
    assert_eq!(
      plane.get(position..).and_then(|row| row.first_chunk::<N>()),
      Some(&expected)
    );
    Ok(())
  }

  #[test]
  fn i4_vertical_horizontal_dc() {
    let Ok(mut plane) = setup();
    let Ok(block) = sample_block(0, ALL);
    let Ok(()) = pred4x4(&mut plane, block);
    let Ok(()) = check_row(&plane, Point { x: 4, y: 4 }, [10, 20, 30, 40]);
    let Ok(()) = check_row(&plane, Point { x: 4, y: 7 }, [10, 20, 30, 40]);
    let Ok(horizontal) = sample_block(1, ALL);
    let Ok(()) = pred4x4(&mut plane, horizontal);
    let Ok(()) = check_row(&plane, Point { x: 4, y: 5 }, [2; 4]);
    let Ok(dc) = sample_block(2, ALL);
    let Ok(()) = pred4x4(&mut plane, dc);
    let Ok(()) = check_sample(&plane, Point { x: 4, y: 4 }, 14);
  }

  #[test]
  fn i4_diag_down_left_hand_computed() {
    let Ok(mut plane) = setup();
    let Ok(block) = sample_block(3, ALL);
    let Ok(()) = pred4x4(&mut plane, block);
    let Ok(()) = check_sample(&plane, Point { x: 4, y: 4 }, 20);
    let Ok(()) = check_sample(&plane, Point { x: 7, y: 7 }, 78);
  }

  #[test]
  fn i4_diag_down_right_hand_computed() {
    let Ok(mut plane) = setup();
    let Ok(block) = sample_block(4, ALL);
    let Ok(()) = pred4x4(&mut plane, block);

    for (x, y, value) in [(4, 4, 5), (5, 4, 11), (4, 5, 2)] {
      let Ok(()) = check_sample(&plane, Point { x, y }, value);
    }
  }

  #[test]
  fn i4_top_right_substitution() {
    let Ok(mut plane) = setup();
    let Ok(block) = sample_block(
      3,
      Avail {
        top_right: false,
        ..ALL
      },
    );
    let Ok(()) = pred4x4(&mut plane, block);
    let Ok(()) = check_sample(&plane, Point { x: 7, y: 7 }, 40);
  }

  #[test]
  fn i16_plane_flat_is_flat() {
    let mut plane = [77; 1024];
    let block = IntraBlock {
      stride: 32,
      origin: Point { x: 8, y: 8 },
      mode: 3,
      available: ALL,
    };
    let Ok(()) = pred16x16(&mut plane, block);
    assert_eq!(plane, [77; 1024]);
  }

  #[test]
  fn chroma_dc_rules() {
    let mut plane = [0; 256];
    let top = [40, 40, 40, 40, 80, 80, 80, 80];
    let left = [8, 8, 8, 8, 16, 16, 16, 16];

    for ((top_2, left_2), coordinate) in top.into_iter().zip(left).zip(4i32..12) {
      let Ok(()) = put_sample(
        &mut plane,
        16,
        Point {
          x: coordinate,
          y: 3,
        },
        top_2,
      );
      let Ok(()) = put_sample(
        &mut plane,
        16,
        Point {
          x: 3,
          y: coordinate,
        },
        left_2,
      );
    }

    let Ok(block) = sample_block(0, ALL);
    let Ok(()) = pred_chroma(&mut plane, block);

    for (x, y, value) in [(4, 4, 24), (8, 4, 80), (4, 8, 16), (8, 8, 48)] {
      let Ok(()) = check_sample(&plane, Point { x, y }, value);
    }
  }
}
