//! 2D inverse transforms and reconstruction (8.6.2, 8.7.2). The 1D DCT / ADST butterflies are
//! generated from the specification's step lists (`transform_gen.rs`); ADST4 (8.7.1.6) and the
//! Walsh-Hadamard transform (8.7.1.10) are written out here.
//!
//! The butterflies are written once over a lane, which is a sample at thirty-two bits for an
//! eight-bit stream, at sixty-four bits for a deeper one, or four or eight columns at once for
//! the column pass of an eight-bit stream, which runs the same arithmetic per lane and so
//! vectorises. The decoder this came from spelled the four with a macro over the same text;
//! here they are one generic function each, and the lane says what its arithmetic is.

use console_core_never::Never;
use console_core_number_conversion::{fitted, fixed_index, index, wrapped};
use console_core_video_inverse_transforms::inverse_walsh_hadamard4;

use crate::places::{read, span_mut};
use crate::tables::{
  ADST_ADST, ADST_DCT, DCT_ADST, DCT_DCT, TX_4X4, TX_8X8, TX_16X16, Transform,
};
use crate::transform_gen::{iadst8, iadst16, idct4, idct8, idct16, idct32};

const SINPI_1_9: i32 = 5283;
const SINPI_2_9: i32 = 9929;
const SINPI_3_9: i32 = 13377;
const SINPI_4_9: i32 = 15212;

const NARROW_LIMIT: i32 = 1 << 24;
const WIDE_LIMIT: i64 = 1 << 40;
const NARROW_RESIDUAL: i32 = 1 << 20;
const WIDE_RESIDUAL: i64 = 1 << 30;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Weights {
  pub near: i32,
  pub far: i32,
}

pub(crate) trait Lane: Copy {
  fn sum(self, other: Self) -> Result<Self, Never>;

  fn difference(self, other: Self) -> Result<Self, Never>;

  fn negated(self) -> Result<Self, Never>;

  fn scaled(self, weight: i32) -> Result<Self, Never>;

  fn rotated(self, weights: Weights, other: Self) -> Result<Self, Never>;

  fn rounded(self) -> Result<Self, Never>;
}

impl Lane for i32 {
  #[inline(always)]
  fn sum(self, other: Self) -> Result<Self, Never> {
    Ok(self.wrapping_add(other))
  }

  #[inline(always)]
  fn difference(self, other: Self) -> Result<Self, Never> {
    Ok(self.wrapping_sub(other))
  }

  #[inline(always)]
  fn negated(self) -> Result<Self, Never> {
    Ok(self.wrapping_neg())
  }

  #[inline(always)]
  fn scaled(self, weight: i32) -> Result<Self, Never> {
    Ok(self.wrapping_mul(weight))
  }

  #[inline(always)]
  fn rotated(self, weights: Weights, other: Self) -> Result<Self, Never> {
    Ok(
      self
        .wrapping_mul(weights.near)
        .wrapping_add(other.wrapping_mul(weights.far)),
    )
  }

  #[inline(always)]
  fn rounded(self) -> Result<Self, Never> {
    Ok(
      self
        .wrapping_add(::core::primitive::i32::wrapping_shl(1, 13_u32))
        .wrapping_shr(14),
    )
  }
}

impl Lane for i64 {
  #[inline(always)]
  fn sum(self, other: Self) -> Result<Self, Never> {
    Ok(self.wrapping_add(other))
  }

  #[inline(always)]
  fn difference(self, other: Self) -> Result<Self, Never> {
    Ok(self.wrapping_sub(other))
  }

  #[inline(always)]
  fn negated(self) -> Result<Self, Never> {
    Ok(self.wrapping_neg())
  }

  #[inline(always)]
  fn scaled(self, weight: i32) -> Result<Self, Never> {
    Ok(self.wrapping_mul(i64::from(weight)))
  }

  #[inline(always)]
  fn rotated(self, weights: Weights, other: Self) -> Result<Self, Never> {
    Ok(
      self
        .wrapping_mul(i64::from(weights.near))
        .wrapping_add(other.wrapping_mul(i64::from(weights.far))),
    )
  }

  #[inline(always)]
  fn rounded(self) -> Result<Self, Never> {
    Ok(
      self
        .wrapping_add(::core::primitive::i64::wrapping_shl(1, 13_u32))
        .wrapping_shr(14),
    )
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Columns<const W: usize>(pub [i32; W]);

impl<const W: usize> Columns<W> {
  #[inline(always)]
  fn paired(self, other: Self, step: impl Fn(i32, i32) -> i32) -> Result<Self, Never> {
    let mut lanes = self.0;

    for (lane, theirs) in lanes.iter_mut().zip(other.0) {
      *lane = step(*lane, theirs);
    }

    Ok(Columns(lanes))
  }
}

impl<const W: usize> Lane for Columns<W> {
  #[inline(always)]
  fn sum(self, other: Self) -> Result<Self, Never> {
    self.paired(other, i32::wrapping_add)
  }

  #[inline(always)]
  fn difference(self, other: Self) -> Result<Self, Never> {
    self.paired(other, i32::wrapping_sub)
  }

  #[inline(always)]
  fn negated(self) -> Result<Self, Never> {
    Ok(Columns(self.0.map(i32::wrapping_neg)))
  }

  #[inline(always)]
  fn scaled(self, weight: i32) -> Result<Self, Never> {
    Ok(Columns(self.0.map(|lane| lane.wrapping_mul(weight))))
  }

  #[inline(always)]
  fn rotated(self, weights: Weights, other: Self) -> Result<Self, Never> {
    self.paired(other, |near, far| {
      near
        .wrapping_mul(weights.near)
        .wrapping_add(far.wrapping_mul(weights.far))
    })
  }

  #[inline(always)]
  fn rounded(self) -> Result<Self, Never> {
    Ok(Columns(self.0.map(|lane| {
      lane
        .wrapping_add(::core::primitive::i32::wrapping_shl(1, 13_u32))
        .wrapping_shr(14)
    })))
  }
}

#[inline(always)]
fn iadst4<T: Lane>(x: [T; 4]) -> Result<[T; 4], Never> {
  let [x0, x1, x2, x3] = x;

  let Ok(s0) = x0.scaled(SINPI_1_9);
  let Ok(s1) = x0.scaled(SINPI_2_9);
  let Ok(s2) = x1.scaled(SINPI_3_9);
  let Ok(s3) = x2.scaled(SINPI_4_9);
  let Ok(s4) = x2.scaled(SINPI_1_9);
  let Ok(s5) = x3.scaled(SINPI_2_9);
  let Ok(s6) = x3.scaled(SINPI_4_9);
  let Ok(v) = x0.difference(x2).and_then(|partial| partial.sum(x3));
  let Ok(s7) = v.scaled(SINPI_3_9);
  let Ok(y0) = s0.sum(s3).and_then(|partial| partial.sum(s5));
  let Ok(y1) = s1.difference(s4).and_then(|partial| partial.difference(s6));
  let Ok(first) = y0.sum(s2).and_then(Lane::rounded);
  let Ok(second) = y1.sum(s2).and_then(Lane::rounded);
  let Ok(third) = s7.rounded();
  let Ok(fourth) = y0
    .sum(y1)
    .and_then(|partial| partial.difference(s2))
    .and_then(Lane::rounded);

  Ok([first, second, third, fourth])
}

fn iwht4(x: [i32; 4], shift: u32) -> Result<[i32; 4], Never> {
  inverse_walsh_hadamard4(x.map(|value| value.wrapping_shr(shift)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kernel {
  Cosine,
  Sine,
}

#[derive(Clone, Copy, Debug)]
struct Kernels {
  rows: Kernel,
  columns: Kernel,
}

fn kernels(kind: u8) -> Result<Kernels, Never> {
  Ok(match kind {
    DCT_ADST => Kernels {
      rows: Kernel::Sine,
      columns: Kernel::Cosine,
    },
    ADST_DCT => Kernels {
      rows: Kernel::Cosine,
      columns: Kernel::Sine,
    },
    ADST_ADST => Kernels {
      rows: Kernel::Sine,
      columns: Kernel::Sine,
    },
    _cosine => Kernels {
      rows: Kernel::Cosine,
      columns: Kernel::Cosine,
    },
  })
}

#[inline(always)]
fn through<T: Lane, const N: usize>(
  values: &mut [T],
  transform: impl Fn([T; N]) -> Result<[T; N], Never>,
) -> Result<(), Never> {
  match values.first_chunk_mut::<N>() {
    Some(chunk) => {
      let Ok(transformed) = transform(*chunk);

      *chunk = transformed;
    }
    None => {}
  }

  Ok(())
}

#[inline(always)]
fn transformed<T: Lane>(values: &mut [T], size: u8, kernel: Kernel) -> Result<(), Never> {
  match (size, kernel) {
    (TX_4X4, Kernel::Cosine) => through(values, idct4),
    (TX_4X4, Kernel::Sine) => through(values, iadst4),
    (TX_8X8, Kernel::Cosine) => through(values, idct8),
    (TX_8X8, Kernel::Sine) => through(values, iadst8),
    (TX_16X16, Kernel::Cosine) => through(values, idct16),
    (TX_16X16, Kernel::Sine) => through(values, iadst16),
    (_largest, Kernel::Cosine | Kernel::Sine) => through(values, idct32),
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quantization {
  Lossless,
  Lossy,
}

pub struct Residual<'a> {
  pub coefficients: &'a [i32],
  pub transform: Transform,
  pub quantization: Quantization,
  pub end: u64,
  pub rows: u64,
  pub bit_depth: u8,
}

pub struct Destination<'a> {
  pub samples: &'a mut [u16],
  pub stride: u64,
}

#[derive(Clone, Copy, Debug)]
struct Pass {
  kernels: Kernels,
  size: u8,
  rows: u64,
  shift: u32,
  round: i32,
  max: i32,
}

pub fn inverse_transform_add(
  residual: &Residual<'_>,
  destination: Destination<'_>,
) -> Result<(), Never> {
  let max = 1_i32
    .wrapping_shl(u32::from(residual.bit_depth))
    .wrapping_sub(1);
  let shift = u32::from(residual.transform.size).wrapping_add(4).min(6);
  let round = 1_i32.wrapping_shl(shift.wrapping_sub(1));
  let Ok(first) = read(residual.coefficients, 0);

  match (residual.quantization, residual.end, residual.transform.kind) {
    (Quantization::Lossless, _end, _kind) => lossless(residual.coefficients, max, destination),
    (Quantization::Lossy, 1, DCT_DCT) => only_dc(
      first,
      Pass {
        kernels: Kernels {
          rows: Kernel::Cosine,
          columns: Kernel::Cosine,
        },
        size: residual.transform.size,
        rows: 1,
        shift,
        round,
        max,
      },
      destination,
    ),

    (Quantization::Lossy, _end, kind) => {
      let Ok(chosen) = kernels(kind);
      let pass = Pass {
        kernels: chosen,
        size: residual.transform.size,
        rows: residual.rows,
        shift,
        round,
        max,
      };

      match (residual.transform.size, residual.bit_depth) {
        (TX_4X4, 8) => narrow::<4>(residual.coefficients, pass, destination),
        (TX_8X8, 8) => narrow::<8>(residual.coefficients, pass, destination),
        (TX_16X16, 8) => narrow::<16>(residual.coefficients, pass, destination),
        (_largest, 8) => narrow::<32>(residual.coefficients, pass, destination),
        (TX_4X4, _deeper) => wide::<4>(residual.coefficients, pass, destination),
        (TX_8X8, _deeper) => wide::<8>(residual.coefficients, pass, destination),
        (TX_16X16, _deeper) => wide::<16>(residual.coefficients, pass, destination),
        (_largest, _deeper) => wide::<32>(residual.coefficients, pass, destination),
      }
    }
  }
}

fn added(
  sample: &mut u16,
  AddedArguments { residual, max }: AddedArguments,
) -> Result<(), Never> {
  let Ok(written) = fitted::<i32, u16>(i32::from(*sample).saturating_add(residual).clamp(0, max));

  *sample = written;

  Ok(())
}

struct AddedArguments {
  residual: i32,
  max: i32,
}

fn lossless(coefficients: &[i32], max: i32, destination: Destination<'_>) -> Result<(), Never> {
  let mut rows = [[0_i32; 4]; 4];

  for (row, source) in rows.iter_mut().zip(coefficients.chunks(4)) {
    let Ok(four) = read_four(source);
    let Ok(transformed) = iwht4(four, 2);

    *row = transformed;
  }

  for (column, place) in (0_u64..4).enumerate() {
    let gathered = rows.map(|row| match row.get(column) {
      Some(value) => *value,
      None => 0,
    });
    let Ok(transformed) = iwht4(gathered, 0);

    for (line, value) in (0_u64..).zip(transformed) {
      let at = line.wrapping_mul(destination.stride).wrapping_add(place);
      let Ok(target) = span_mut(destination.samples, at..at.wrapping_add(1));

      for sample in target {
        let Ok(()) = added(
          sample,
          self::AddedArguments {
            residual: value,
            max,
          },
        );
      }
    }
  }

  Ok(())
}

fn read_four(source: &[i32]) -> Result<[i32; 4], Never> {
  Ok(match source.first_chunk::<4>() {
    Some(found) => *found,
    None => [0; 4],
  })
}

fn only_dc(dc: i32, pass: Pass, destination: Destination<'_>) -> Result<(), Never> {
  let a = i64::from(dc)
    .wrapping_mul(11585)
    .wrapping_add(::core::primitive::i64::wrapping_shl(1, 13_u32))
    .wrapping_shr(14);
  let b = a
    .wrapping_mul(11585)
    .wrapping_add(::core::primitive::i64::wrapping_shl(1, 13_u32))
    .wrapping_shr(14);
  let lowest = i64::from(NARROW_RESIDUAL.wrapping_neg());
  let Ok(value) = fitted::<i64, i32>(
    b.wrapping_add(i64::from(pass.round))
      .wrapping_shr(pass.shift)
      .clamp(lowest, i64::from(NARROW_RESIDUAL)),
  );
  let size = 4_u64.wrapping_shl(u32::from(pass.size));

  for line in 0..size {
    let at = line.wrapping_mul(destination.stride);
    let Ok(target) = span_mut(destination.samples, at..at.wrapping_add(size));

    for sample in target {
      let Ok(()) = added(
        sample,
        self::AddedArguments {
          residual: value,
          max: pass.max,
        },
      );
    }
  }

  Ok(())
}

fn narrow<const N: usize>(
  coefficients: &[i32],
  pass: Pass,
  destination: Destination<'_>,
) -> Result<(), Never> {
  let mut rows = [[0_i32; N]; N];
  let Ok(taken) = index(pass.rows);

  for (row, source) in rows
    .iter_mut()
    .zip(coefficients.chunks(N.max(1)))
    .take(taken)
  {
    for (value, coefficient) in row.iter_mut().zip(source) {
      *value = (*coefficient).clamp(NARROW_LIMIT.wrapping_neg(), NARROW_LIMIT);
    }

    let Ok(()) = transformed(row.as_mut_slice(), pass.size, pass.kernels.rows);

    for value in row.iter_mut() {
      *value = (*value).clamp(NARROW_LIMIT.wrapping_neg(), NARROW_LIMIT);
    }
  }

  match pass.size {
    TX_4X4 => {
      let mut columns = rows.map(|row| {
        let Ok(lanes) = read_lanes::<N, 4>(&row, 0);

        Columns(lanes)
      });
      let Ok(()) = transformed(columns.as_mut_slice(), pass.size, pass.kernels.columns);

      added_columns(&columns, 0, pass, destination)
    }

    _wider => {
      let Destination { samples, stride } = destination;
      let Ok(width) = fixed_index(N);

      for start in (0..width).step_by(8) {
        let mut columns = rows.map(|row| {
          let Ok(lanes) = read_lanes::<N, 8>(&row, start);

          Columns(lanes)
        });
        let Ok(()) = transformed(columns.as_mut_slice(), pass.size, pass.kernels.columns);
        let Ok(()) = added_columns(
          &columns,
          start,
          pass,
          Destination {
            samples: &mut *samples,
            stride,
          },
        );
      }

      Ok(())
    }
  }
}

fn read_lanes<const N: usize, const W: usize>(
  row: &[i32; N],
  start: u64,
) -> Result<[i32; W], Never> {
  let mut lanes = [0_i32; W];
  let Ok(skipped) = index(start);

  for (lane, value) in lanes.iter_mut().zip(row.iter().skip(skipped)) {
    *lane = *value;
  }

  Ok(lanes)
}

fn added_columns<const N: usize, const W: usize>(
  columns: &[Columns<W>; N],
  start: u64,
  pass: Pass,
  destination: Destination<'_>,
) -> Result<(), Never> {
  let Ok(width) = fixed_index(W);

  for (line, lanes) in (0_u64..).zip(columns) {
    let at = line.wrapping_mul(destination.stride).wrapping_add(start);
    let Ok(target) = span_mut(destination.samples, at..at.wrapping_add(width));

    for (sample, lane) in target.iter_mut().zip(lanes.0) {
      let residual = lane
        .wrapping_add(pass.round)
        .wrapping_shr(pass.shift)
        .clamp(NARROW_RESIDUAL.wrapping_neg(), NARROW_RESIDUAL);
      let Ok(()) = added(
        sample,
        self::AddedArguments {
          residual,
          max: pass.max,
        },
      );
    }
  }

  Ok(())
}

fn wide<const N: usize>(
  coefficients: &[i32],
  pass: Pass,
  destination: Destination<'_>,
) -> Result<(), Never> {
  let mut rows = [[0_i64; N]; N];
  let Ok(taken) = index(pass.rows);

  for (row, source) in rows
    .iter_mut()
    .zip(coefficients.chunks(N.max(1)))
    .take(taken)
  {
    for (value, coefficient) in row.iter_mut().zip(source) {
      *value = i64::from(*coefficient);
    }

    let Ok(()) = transformed(row.as_mut_slice(), pass.size, pass.kernels.rows);

    for value in row.iter_mut() {
      *value = (*value).clamp(WIDE_LIMIT.wrapping_neg(), WIDE_LIMIT);
    }
  }

  let Ok(width) = fixed_index(N);

  for place in 0..width {
    let Ok(at) = index(place);
    let mut column = rows.map(|row| match row.get(at) {
      Some(value) => *value,
      None => 0,
    });
    let Ok(()) = transformed(column.as_mut_slice(), pass.size, pass.kernels.columns);

    for (line, value) in (0_u64..).zip(column) {
      let offset = line.wrapping_mul(destination.stride).wrapping_add(place);
      let Ok(target) = span_mut(destination.samples, offset..offset.wrapping_add(1));
      let rounded = value
        .clamp(WIDE_LIMIT.wrapping_neg(), WIDE_LIMIT)
        .wrapping_add(i64::from(pass.round))
        .wrapping_shr(pass.shift);
      let Ok(residual) =
        wrapped::<i64, i32>(rounded.clamp(WIDE_RESIDUAL.wrapping_neg(), WIDE_RESIDUAL));

      for sample in target {
        let Ok(()) = added(
          sample,
          self::AddedArguments {
            residual,
            max: pass.max,
          },
        );
      }
    }
  }

  Ok(())
}
