//! Intra prediction process (8.5.1).

use console_core_never::Never;
use console_core_number_conversion::{wrapped, wrapped_index};

use crate::places::{read, span_mut, write};
use crate::tables::*;

#[derive(Clone, Copy)]
pub(crate) enum AdjacentEdges {
  Neither,
  Left,
  Above,
  Both,
}

impl From<(bool, bool)> for AdjacentEdges {
  fn from((left, above): (bool, bool)) -> Self {
    match (left, above) {
      (false, false) => Self::Neither,
      (true, false) => Self::Left,
      (false, true) => Self::Above,
      (true, true) => Self::Both,
    }
  }
}

#[derive(Clone, Copy)]
pub(crate) enum RightExtension {
  ReadSamples,
  RepeatEdge,
}

pub(crate) struct IntraEdge {
  pub(crate) adjacent: AdjacentEdges,
  pub(crate) right: RightExtension,
  pub(crate) tx_size: u8,
  pub(crate) max_x: u64,
  pub(crate) max_y: u64,
  pub(crate) bit_depth: u8,
}

#[derive(Clone, Copy)]
pub(crate) struct IntraBlock {
  pub(crate) stride: u64,
  pub(crate) band_start: u64,
  pub(crate) x: u64,
  pub(crate) y: u64,
}

impl IntraBlock {
  fn origin(self) -> Result<u64, Never> {
    Ok(
      self
        .y
        .wrapping_mul(self.stride)
        .wrapping_add(self.x)
        .wrapping_sub(self.band_start),
    )
  }
}

pub(crate) fn predict(
  plane: &mut [u16],
  block: IntraBlock,
  mode: u8,
  edge: &IntraEdge,
) -> Result<(), Never> {
  let exponent = match edge.tx_size {
    TX_4X4..=TX_32X32 => u32::from(edge.tx_size).wrapping_add(2),
    _ => return Ok(()),
  };
  let size = 1i32.wrapping_shl(exponent);
  let Ok(neighbors) = PredictionEdges::new(plane, block, edge, size);
  let Ok(origin) = block.origin();

  let prediction = match mode {
    V_PRED => Prediction::Vertical,
    H_PRED => Prediction::Horizontal,
    D45_PRED => Prediction::D45,
    D63_PRED => Prediction::D63,
    D117_PRED => Prediction::D117,
    D135_PRED => Prediction::D135,
    D153_PRED => Prediction::D153,
    D207_PRED => Prediction::D207,
    TM_PRED => Prediction::TrueMotion,
    _ => {
      let Ok(sample) = neighbors.dc(edge.adjacent, exponent);
      Prediction::Constant(sample)
    }
  };

  for row in 0..size {
    let Ok(row_offset) = wrapped::<_, u64>(row);
    let dst = origin.wrapping_add(row_offset.wrapping_mul(block.stride));

    for column in 0..size {
      let Ok(sample) = prediction.sample(&neighbors, IntraSample { row, column });
      let Ok(value) = wrapped(sample);
      let Ok(column_offset) = wrapped::<_, u64>(column);
      let Ok(()) = write(plane, dst.wrapping_add(column_offset), value);
    }
  }

  Ok(())
}

struct PredictionEdges {
  above: [i32; 65],
  left: [i32; 32],
  size: i32,
  midpoint: i32,
  max_sample: i32,
}

impl PredictionEdges {
  fn new(plane: &[u16], block: IntraBlock, edge: &IntraEdge, size: i32) -> Result<Self, Never> {
    let max_sample = 1i32.wrapping_shl(u32::from(edge.bit_depth)).wrapping_sub(1);
    let midpoint = 1i32.wrapping_shl(u32::from(edge.bit_depth.wrapping_sub(1)));
    let mut samples = Self {
      above: [0; 65],
      left: [0; 32],
      size,
      midpoint,
      max_sample,
    };
    let Ok(width) = wrapped::<_, u64>(size);
    let Ok(upper) = span_mut(&mut samples.above, 1..width.wrapping_mul(2).wrapping_add(1));
    let top_y = block.y.wrapping_sub(1);
    let left_x = block.x.wrapping_sub(1);

    match edge.adjacent {
      AdjacentEdges::Neither | AdjacentEdges::Left => upper.fill(midpoint.wrapping_sub(1)),
      AdjacentEdges::Above | AdjacentEdges::Both => {
        let last_column = match (edge.tx_size, edge.right) {
          (TX_4X4, RightExtension::ReadSamples) => width.wrapping_mul(2).wrapping_sub(1),
          _ => width.wrapping_sub(1),
        };
        let top_row = top_y.wrapping_mul(block.stride);

        for (column, sample) in (0..).zip(upper) {
          let x = block
            .x
            .wrapping_add(column.min(last_column))
            .min(edge.max_x);
          let offset = top_row.wrapping_add(x).wrapping_sub(block.band_start);
          let Ok(source) = read(plane, offset);
          *sample = i32::from(source);
        }
      }
    }

    let Ok(left) = span_mut(&mut samples.left, 0..width);

    match edge.adjacent {
      AdjacentEdges::Neither | AdjacentEdges::Above => left.fill(midpoint.wrapping_add(1)),
      AdjacentEdges::Left | AdjacentEdges::Both => {
        for (row, sample) in (0..).zip(left) {
          let y = block.y.wrapping_add(row).min(edge.max_y);
          let offset = y
            .wrapping_mul(block.stride)
            .wrapping_add(left_x)
            .wrapping_sub(block.band_start);
          let Ok(source) = read(plane, offset);
          *sample = i32::from(source);
        }
      }
    }

    let [corner, ..] = &mut samples.above;
    *corner = match edge.adjacent {
      AdjacentEdges::Both => {
        let offset = top_y
          .wrapping_mul(block.stride)
          .wrapping_add(left_x.min(edge.max_x))
          .wrapping_sub(block.band_start);
        let Ok(value) = read(plane, offset);
        i32::from(value)
      }
      AdjacentEdges::Above => midpoint.wrapping_add(1),
      AdjacentEdges::Neither | AdjacentEdges::Left => midpoint.wrapping_sub(1),
    };

    Ok(samples)
  }

  #[inline]
  fn above(&self, column: i32) -> Result<i32, Never> {
    let Ok(slot) = wrapped::<_, u64>(column.wrapping_add(1));
    read(&self.above, slot)
  }

  #[inline]
  fn left(&self, row: i32) -> Result<i32, Never> {
    let Ok(at) = wrapped::<_, u64>(row);
    read(&self.left, at)
  }

  fn dc(&self, adjacent: AdjacentEdges, exponent: u32) -> Result<i32, Never> {
    let Ok(width) = wrapped_index(self.size);
    let left = self.left.iter().take(width).copied().sum::<i32>();
    let above = self.above.iter().skip(1).take(width).copied().sum::<i32>();
    let half = self.size.wrapping_div(2);

    Ok(match adjacent {
      AdjacentEdges::Neither => self.midpoint,
      AdjacentEdges::Left => left.wrapping_add(half).wrapping_shr(exponent),
      AdjacentEdges::Above => above.wrapping_add(half).wrapping_shr(exponent),
      AdjacentEdges::Both => left
        .wrapping_add(above)
        .wrapping_add(self.size)
        .wrapping_shr(exponent.wrapping_add(1)),
    })
  }

  #[inline]
  fn line_sample(&self, line: BoundaryLine, at: i32) -> Result<i32, Never> {
    match (line, at) {
      (_, -1) => self.above(-1),
      (BoundaryLine::Above, 0..) => self.above(at),
      (BoundaryLine::Left, 0..) => self.left(at),
      (BoundaryLine::Above, _) => self.left((-2i32).wrapping_sub(at)),
      (BoundaryLine::Left, _) => self.above((-2i32).wrapping_sub(at)),
    }
  }

  #[inline]
  fn line_pair(&self, line: BoundaryLine, at: i32) -> Result<i32, Never> {
    let Ok(a) = self.line_sample(line, at);
    let Ok(b) = self.line_sample(line, at.wrapping_add(1));
    average_pair([a, b])
  }

  #[inline]
  fn line_triple(&self, line: BoundaryLine, middle: i32) -> Result<i32, Never> {
    let Ok(a) = self.line_sample(line, middle.wrapping_sub(1));
    let Ok(b) = self.line_sample(line, middle);
    let Ok(c) = self.line_sample(line, middle.wrapping_add(1));
    average_triple([a, b, c])
  }
}

#[derive(Clone, Copy)]
enum BoundaryLine {
  Above,
  Left,
}

#[derive(Clone, Copy)]
enum Prediction {
  Horizontal,
  Vertical,
  D45,
  D63,
  D117,
  D135,
  D153,
  D207,
  TrueMotion,
  Constant(i32),
}

struct IntraSample {
  row: i32,
  column: i32,
}

impl Prediction {
  #[inline]
  fn sample(
    self,
    edges: &PredictionEdges,
    IntraSample { row, column }: IntraSample,
  ) -> Result<i32, Never> {
    match self {
      Self::Horizontal => edges.left(row),
      Self::Vertical => edges.above(column),
      Self::D45 => {
        let diagonal = row.wrapping_add(column);
        let end = edges.size.wrapping_mul(2);

        match (diagonal.wrapping_add(2)).cmp(&end) {
          core::cmp::Ordering::Less => {
            edges.line_triple(BoundaryLine::Above, diagonal.wrapping_add(1))
          }
          core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
            edges.above(end.wrapping_sub(1))
          }
        }
      }
      Self::D63 => {
        let start = column.wrapping_add(row.wrapping_div(2));

        match row.wrapping_rem(2) {
          0 => edges.line_pair(BoundaryLine::Above, start),
          _ => edges.line_triple(BoundaryLine::Above, start.wrapping_add(1)),
        }
      }
      Self::D117 => shallow_diagonal(edges, BoundaryLine::Above, IntraSample { row, column }),
      Self::D153 => shallow_diagonal(
        edges,
        BoundaryLine::Left,
        IntraSample {
          row: column,
          column: row,
        },
      ),
      Self::D135 => edges.line_triple(
        BoundaryLine::Above,
        column.wrapping_sub(row).wrapping_sub(1),
      ),
      Self::D207 => downward_left(edges, IntraSample { row, column }),
      Self::TrueMotion => {
        let Ok(above) = edges.above(column);
        let Ok(left) = edges.left(row);
        let Ok(corner) = edges.above(-1);
        Ok(
          above
            .wrapping_add(left)
            .wrapping_sub(corner)
            .clamp(0, edges.max_sample),
        )
      }
      Self::Constant(value) => Ok(value),
    }
  }
}

#[inline]
fn shallow_diagonal(
  edges: &PredictionEdges,
  line: BoundaryLine,
  IntraSample { row, column }: IntraSample,
) -> Result<i32, Never> {
  let diagonal = column.wrapping_mul(2).wrapping_sub(row);

  match diagonal {
    ..=-2 => edges.line_triple(line, diagonal),
    _ => match row.wrapping_rem(2) {
      0 => edges.line_pair(line, diagonal.wrapping_div(2).wrapping_sub(1)),
      _ => edges.line_triple(
        line,
        diagonal.wrapping_add(1).wrapping_div(2).wrapping_sub(1),
      ),
    },
  }
}

#[inline]
fn downward_left(
  edges: &PredictionEdges,
  IntraSample { row, column }: IntraSample,
) -> Result<i32, Never> {
  let last = edges.size.wrapping_sub(1);
  let first = row.wrapping_add(column.wrapping_div(2)).min(last);
  let Ok(a) = edges.left(first);
  let Ok(b) = edges.left(first.wrapping_add(1).min(last));

  match column.wrapping_rem(2) {
    0 => average_pair([a, b]),
    _ => {
      let Ok(c) = edges.left(first.wrapping_add(2).min(last));
      average_triple([a, b, c])
    }
  }
}

#[inline]
fn average_pair([a, b]: [i32; 2]) -> Result<i32, Never> {
  Ok(a.wrapping_add(b).wrapping_add(1).wrapping_shr(1))
}

#[inline]
fn average_triple([a, b, c]: [i32; 3]) -> Result<i32, Never> {
  Ok(
    a.wrapping_add(b.wrapping_mul(2))
      .wrapping_add(c)
      .wrapping_add(2)
      .wrapping_shr(2),
  )
}
