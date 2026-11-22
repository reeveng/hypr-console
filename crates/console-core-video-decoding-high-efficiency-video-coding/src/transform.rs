//! Scaling (8.6.3), transform skip and inverse transforms (8.6.4): DCT 4..32 and DST 4x4.

use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};

use crate::places::{read, table_row, window_mut};
use crate::tables::{DST_MATRIX, TRANS_MATRIX};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InverseKind {
  Cosine,
  Sine,
}

impl From<bool> for InverseKind {
  fn from(sine: bool) -> Self {
    match sine.then_some(()) {
      Some(()) => Self::Sine,
      None => Self::Cosine,
    }
  }
}

#[derive(Clone, Copy)]
pub struct CoefficientBounds {
  pub x: u64,
  pub y: u64,
}

#[derive(Clone, Copy)]
pub struct Block {
  pub side: u64,
  pub shift: u32,
}

#[derive(Clone, Copy)]
pub struct InverseTransform {
  pub block: Block,
  pub kind: InverseKind,
  pub nonzero: CoefficientBounds,
}

pub fn inverse_transform(coefficients: &mut [i32], terms: InverseTransform) -> Result<(), Never> {
  let cosine = InverseTransform {
    kind: InverseKind::Cosine,
    ..terms
  };

  match terms.block.side {
    4 => inverse::<4>(coefficients, terms),
    8 => inverse::<8>(coefficients, cosine),
    16 => inverse::<16>(coefficients, cosine),
    _ => inverse::<32>(coefficients, cosine),
  }
}

struct Basis {
  kind: InverseKind,
  step: u64,
}

impl Basis {
  fn row(&self, index: u64) -> Result<&[i8], Never> {
    match self.kind {
      InverseKind::Sine => table_row(&DST_MATRIX, index),
      InverseKind::Cosine => table_row(&TRANS_MATRIX, index.wrapping_mul(self.step)),
    }
  }
}

#[inline(always)]
fn inverse<const N: usize>(
  coefficients: &mut [i32],
  terms: InverseTransform,
) -> Result<(), Never> {
  let Ok(width) = fixed_index(N);
  let basis = Basis {
    kind: terms.kind,
    step: 32u64.wrapping_div(width.max(1)),
  };
  let x_end = terms.nonzero.x.min(width.saturating_sub(1)).wrapping_add(1);
  let y_end = terms.nonzero.y.min(width.saturating_sub(1)).wrapping_add(1);
  let Ok(cols) = index(x_end);
  let Ok(rows) = index(y_end);
  let mut intermediate = [[0i32; N]; N];

  for (x, accumulation) in (0u64..).zip(intermediate.iter_mut().take(cols)) {
    for (k, row) in (0u64..).zip(coefficients.chunks(N.max(1)).take(rows)) {
      let Ok(value) = read(row, x);
      let Ok(()) = accumulate(&basis, accumulation, k, value);
    }

    for value in accumulation {
      *value = value.wrapping_add(64).wrapping_shr(7).clamp(-32768, 32767);
    }
  }

  let shift = terms.block.shift;
  let round = 1i32.wrapping_shl(shift.wrapping_sub(1));

  for (y, row) in (0u64..).zip(coefficients.chunks_mut(N.max(1))).take(N) {
    let mut result = [round; N];

    for (k, column) in (0u64..).zip(intermediate.iter().take(cols)) {
      let Ok(value) = read(column, y);
      let Ok(()) = accumulate(&basis, &mut result, k, value);
    }

    for (output, value) in row.iter_mut().zip(result) {
      *output = value.wrapping_shr(shift);
    }
  }

  Ok(())
}

#[inline]
fn accumulate(basis: &Basis, buffer: &mut [i32], k: u64, value: i32) -> Result<(), Never> {
  match value {
    0 => {}
    value_2 => {
      let Ok(terms) = basis.row(k);

      for (sum, &factor) in buffer.iter_mut().zip(terms) {
        *sum = sum.wrapping_add(value_2.wrapping_mul(i32::from(factor)));
      }
    }
  }

  Ok(())
}

pub fn inverse_dc(dc: i32, shift: u32) -> Result<i32, Never> {
  let intermediate = dc
    .wrapping_mul(64)
    .wrapping_add(64)
    .wrapping_shr(7)
    .clamp(-32768, 32767);
  let round = 1i32.wrapping_shl(shift.wrapping_sub(1));
  Ok(
    intermediate
      .wrapping_mul(64)
      .wrapping_add(round)
      .wrapping_shr(shift),
  )
}

pub fn transform_skip(coefficients: &mut [i32], block: Block) -> Result<(), Never> {
  let sample_shift = 5u32.wrapping_add(block.side.trailing_zeros());
  let round = 1i32.wrapping_shl(block.shift.wrapping_sub(1));
  let Ok(active) = window_mut(coefficients, 0..block.side.wrapping_mul(block.side));

  for value in active {
    *value = value
      .wrapping_shl(sample_shift)
      .wrapping_add(round)
      .wrapping_shr(block.shift);
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::places::write;
  use console_core_number_conversion::wrapped;

  fn reference(coefficients: &mut [i32], terms: InverseTransform) -> Result<(), Never> {
    let InverseTransform {
      block: Block { side, shift },
      kind,
      nonzero,
    } = terms;
    let step = 32u64.wrapping_div(side.max(1));
    let matrix = |j: u64| match kind {
      InverseKind::Sine => table_row(&DST_MATRIX, j),
      InverseKind::Cosine => table_row(&TRANS_MATRIX, j.wrapping_mul(step)),
    };
    let mut intermediate = [0i32; 1024];

    for x in 0..=nonzero.x {
      for y in 0..side {
        let mut sum = 0i32;

        for j in 0..=nonzero.y {
          let Ok(row) = matrix(j);
          let Ok(m) = read(row, y);
          let Ok(value) = read(coefficients, j.wrapping_mul(side).wrapping_add(x));
          sum = sum.wrapping_add(i32::from(m).wrapping_mul(value));
        }

        let value = sum.wrapping_add(64).wrapping_shr(7).clamp(-32768, 32767);
        let Ok(()) = write(
          &mut intermediate,
          y.wrapping_mul(side).wrapping_add(x),
          value,
        );
      }
    }

    let round = 1i32.wrapping_shl(shift.wrapping_sub(1));

    for y in 0..side {
      for x in 0..side {
        let mut sum = round;

        for j in 0..=nonzero.x {
          let Ok(row) = matrix(j);
          let Ok(m) = read(row, x);
          let Ok(value) = read(&intermediate, y.wrapping_mul(side).wrapping_add(j));
          sum = sum.wrapping_add(i32::from(m).wrapping_mul(value));
        }

        let Ok(()) = write(
          coefficients,
          y.wrapping_mul(side).wrapping_add(x),
          sum.wrapping_shr(shift),
        );
      }
    }

    Ok(())
  }

  struct TestBlock {
    side: u64,
    bounds: CoefficientBounds,
    density: u64,
  }

  fn coefficient_magnitude(next: &mut impl FnMut() -> u64) -> Result<i32, Never> {
    Ok(match next().wrapping_rem(8) {
      0 => 32767,
      1 => -32768,
      _ => {
        let Ok(value) = wrapped::<_, i32>(next().wrapping_rem(2001));
        value.wrapping_sub(1000)
      }
    })
  }

  fn coefficients(block: TestBlock, next: &mut impl FnMut() -> u64) -> Result<[i32; 1024], Never> {
    let mut c = [0i32; 1024];

    for y in 0..=block.bounds.y {
      for x in 0..=block.bounds.x {
        match (next().wrapping_rem(100) < block.density).then_some(()) {
          Some(()) => {
            let Ok(value) = coefficient_magnitude(next);
            let Ok(()) = write(&mut c, y.wrapping_mul(block.side).wrapping_add(x), value);
          }
          None => {}
        }
      }
    }

    Ok(c)
  }

  #[test]
  fn matches_the_direct_matrix_product() {
    let mut rng = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
      rng ^= rng.wrapping_shl(13);
      rng ^= rng.wrapping_shr(7);
      rng ^= rng.wrapping_shl(17);
      rng
    };

    for (side, kind) in [
      (4u64, InverseKind::Sine),
      (4, InverseKind::Cosine),
      (8, InverseKind::Cosine),
      (16, InverseKind::Cosine),
      (32, InverseKind::Cosine),
    ] {
      for case in 0u64..400 {
        let nonzero = CoefficientBounds {
          x: next().wrapping_rem(side.max(1)),
          y: next().wrapping_rem(side.max(1)),
        };
        let Ok(density) = read(&[2u64, 8, 50, 100], case.wrapping_rem(4));
        let Ok(input) = coefficients(TestBlock { side, bounds: nonzero, density }, &mut next);

        for shift in [12, 10] {
          let mut a = input;
          let mut b = input;
          let terms = InverseTransform {
            block: Block { side, shift },
            kind,
            nonzero,
          };
          let Ok(()) = inverse_transform(&mut a, terms);
          let Ok(()) = reference(&mut b, terms);
          assert_eq!(
            a, b,
            "side {side} transform {kind:?} extent ({}, {}) shift {shift}",
            nonzero.x, nonzero.y
          );
        }
      }
    }
  }

  #[test]
  fn dc_only_matches_full_transform() {
    for side in [4u64, 8, 16, 32] {
      for dc in [-300, -1, 1, 7, 64, 1000] {
        let mut input = [0i32; 1024];
        let Ok(samples) = window_mut(&mut input, 0..side.wrapping_mul(side));
        let Ok(()) = write(samples, 0, dc);
        let terms = InverseTransform {
          block: Block { side, shift: 12 },
          kind: InverseKind::Cosine,
          nonzero: CoefficientBounds { x: 0, y: 0 },
        };
        let Ok(()) = inverse_transform(samples, terms);
        let Ok(expected) = inverse_dc(dc, 12);
        assert!(
          samples.iter().all(|&value| value == expected),
          "side {side} dc {dc}"
        );
      }
    }
  }

  #[test]
  fn bounded_region_matches_full() {
    let mut input = [0i32; 256];
    let Ok(()) = write(&mut input, 0, 50);
    let Ok(()) = write(&mut input, 1, -20);
    let Ok(()) = write(&mut input, 35, 9);
    let mut a = input;
    let mut b = input;
    let terms = InverseTransform {
      block: Block {
        side: 16,
        shift: 12,
      },
      kind: InverseKind::Cosine,
      nonzero: CoefficientBounds { x: 3, y: 2 },
    };
    let Ok(()) = inverse_transform(&mut a, terms);
    let Ok(()) = inverse_transform(
      &mut b,
      InverseTransform {
        nonzero: CoefficientBounds { x: 15, y: 15 },
        ..terms
      },
    );
    assert_eq!(a, b);
  }
}
