//! Scaling (dequantisation) and inverse transforms (8.5.10 - 8.5.13).

use crate::params::ScalingMatrices;
use crate::tables::{QuantizerRemainder, norm_adjust4, norm_adjust8};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, wrapped};

#[derive(Clone)]
pub struct LevelScale {
  pub ls4: [[[i32; 16]; 6]; 6],

  pub ls8: [[[i32; 64]; 6]; 6],
}

impl LevelScale {
  pub fn new(m: &ScalingMatrices) -> core::result::Result<Box<Self>, console_core_never::Never> {
    let mut s = Box::new(LevelScale {
      ls4: [[[0; 16]; 6]; 6],
      ls8: [[[0; 64]; 6]; 6],
    });

    for (matrix, scales) in m.m4.iter().zip(&mut s.ls4) {
      for (remainder, coefficients) in (0_u8..6).zip(scales) {
        for ((position, &weight), level) in (0_u8..16).zip(matrix).zip(coefficients) {
          let Ok(norm) = norm_adjust4(QuantizerRemainder(remainder), position);
          *level = i32::from(weight).wrapping_mul(i32::from(norm));
        }
      }
    }

    for (matrix, scales) in m.m8.iter().zip(&mut s.ls8) {
      for (remainder, coefficients) in (0_u8..6).zip(scales) {
        for ((position, &weight), level) in (0_u8..64).zip(matrix).zip(coefficients) {
          let Ok(norm) = norm_adjust8(QuantizerRemainder(remainder), position);
          *level = i32::from(weight).wrapping_mul(i32::from(norm));
        }
      }
    }

    Ok(s)
  }
}

#[derive(Clone, Copy)]
pub(crate) struct Dequantization {
  pub(crate) weight: i32,
  pub(crate) quantizer: i32,
}

#[inline(always)]
pub(crate) fn scale4(coefficient: i32, settings: Dequantization) -> Result<i32, Never> {
  scale(coefficient, settings, 4)
}

#[inline(always)]
pub(crate) fn scale8(coefficient: i32, settings: Dequantization) -> Result<i32, Never> {
  scale(coefficient, settings, 6)
}

#[inline(always)]
fn scale(coefficient: i32, settings: Dequantization, fractional_bits: u8) -> Result<i32, Never> {
  let weighted = coefficient.wrapping_mul(settings.weight);
  let exponent = settings
    .quantizer
    .wrapping_div(6)
    .wrapping_sub(i32::from(fractional_bits));
  Ok(match exponent {
    0.. => {
      let Ok(shift) = wrapped::<_, u32>(exponent);
      weighted.wrapping_shl(shift)
    }
    ..=-1 => {
      let Ok(shift) = wrapped::<_, u32>(exponent.wrapping_neg());
      let bias = 1_i32.wrapping_shl(shift.wrapping_sub(1));
      weighted.wrapping_add(bias).wrapping_shr(shift)
    }
  })
}

pub(crate) fn luma_dc_dequant(
  coefficients: &mut [i32; 16],
  settings: Dequantization,
) -> Result<(), Never> {
  let Ok(block) = pass_twice(coefficients.as_chunks::<4>().0, hadamard4);

  for (row, values) in coefficients.as_chunks_mut::<4>().0.iter_mut().zip(block) {
    for (output, value) in row.iter_mut().zip(values) {
      let Ok(scaled) = scale8(value, settings);
      *output = scaled;
    }
  }

  Ok(())
}

fn hadamard4([a, b, c, d]: [i32; 4]) -> Result<[i32; 4], Never> {
  let s01 = a.wrapping_add(b);
  let d01 = a.wrapping_sub(b);
  let s23 = c.wrapping_add(d);
  let d23 = c.wrapping_sub(d);
  Ok([
    s01.wrapping_add(s23),
    s01.wrapping_sub(s23),
    d01.wrapping_sub(d23),
    d01.wrapping_add(d23),
  ])
}

pub(crate) fn chroma_dc_dequant_420(
  coefficients: &mut [i32; 4],
  settings: Dequantization,
) -> Result<(), Never> {
  let [a, b, c, d] = *coefficients;
  let s0 = a.wrapping_add(b);
  let d0 = a.wrapping_sub(b);
  let s1 = c.wrapping_add(d);
  let d1 = c.wrapping_sub(d);
  let transformed = [
    s0.wrapping_add(s1),
    d0.wrapping_add(d1),
    s0.wrapping_sub(s1),
    d0.wrapping_sub(d1),
  ];
  let Ok(shift) = wrapped::<_, u32>(settings.quantizer.wrapping_div(6));

  for (out, value) in coefficients.iter_mut().zip(transformed) {
    *out = value
      .wrapping_mul(settings.weight)
      .wrapping_shl(shift)
      .wrapping_shr(5);
  }

  Ok(())
}

fn transpose<const N: usize>(block: &[[i32; N]; N]) -> Result<[[i32; N]; N], Never> {
  let mut transposed = [[0; N]; N];
  let Ok(width) = fixed_index(N);

  for (row_index, row) in (0..width).zip(block) {
    let Ok(position) = index(row_index);

    for (column, &value) in transposed.iter_mut().zip(row) {
      {
        match column.get_mut(position) {
          Some(sample) => {
            *sample = value;
          }
          ::core::option::Option::None => {}
        };
      }
    }
  }

  Ok(transposed)
}

fn pass_twice<const N: usize>(
  rows: &[[i32; N]],
  transform: impl Fn([i32; N]) -> Result<[i32; N], Never>,
) -> Result<[[i32; N]; N], Never> {
  let mut horizontal = [[0; N]; N];

  for (destination, &source) in horizontal.iter_mut().zip(rows) {
    let Ok(row) = transform(source);
    *destination = row;
  }

  let Ok(mut columns) = transpose(&horizontal);

  for column in &mut columns {
    let Ok(transformed) = transform(*column);
    *column = transformed;
  }

  transpose(&columns)
}

fn add_block<const N: usize>(
  block: &[[i32; N]; N],
  destination: &mut [u8],
  stride: u64,
) -> Result<(), Never> {
  let Ok(width) = fixed_index(N);

  for (row, line) in (0..width).zip(block) {
    let Ok(position) = index(row.wrapping_mul(stride));
    let samples = match destination
      .get_mut(position..)
      .and_then(<[u8]>::first_chunk_mut::<N>)
    {
      Some(samples) => samples,
      None => return Ok(()),
    };

    for (pixel, &delta) in samples.iter_mut().zip(line) {
      let value = i32::from(*pixel).wrapping_add(delta.wrapping_add(32).wrapping_shr(6));
      let Ok(clipped) = wrapped::<_, u8>(value.clamp(0, 255));
      *pixel = clipped;
    }
  }

  Ok(())
}

pub(crate) fn idct4_add(
  coefficients: &[i32; 16],
  destination: &mut [u8],
  stride: u64,
) -> Result<(), Never> {
  let Ok(block) = pass_twice(coefficients.as_chunks::<4>().0, idct4_1d);
  add_block(&block, destination, stride)
}

#[inline(always)]
fn idct4_1d([a, b, c, d]: [i32; 4]) -> Result<[i32; 4], Never> {
  let e = a.wrapping_add(c);
  let f = a.wrapping_sub(c);
  let g = b.wrapping_shr(1).wrapping_sub(d);
  let h = b.wrapping_add(d.wrapping_shr(1));
  Ok([
    e.wrapping_add(h),
    f.wrapping_add(g),
    f.wrapping_sub(g),
    e.wrapping_sub(h),
  ])
}

#[cfg(test)]
fn idct4_dc_add(dc: i32, destination: &mut [u8], stride: u64) -> Result<(), Never> {
  add_block(&[[dc; 4]; 4], destination, stride)
}

#[inline(always)]
fn idct8_1d([d0, d1, d2, d3, d4, d5, d6, d7]: [i32; 8]) -> Result<[i32; 8], Never> {
  let a0 = d0.wrapping_add(d4);
  let a4 = d0.wrapping_sub(d4);
  let a2 = d2.wrapping_shr(1).wrapping_sub(d6);
  let a6 = d2.wrapping_add(d6.wrapping_shr(1));
  let b0 = a0.wrapping_add(a6);
  let b2 = a4.wrapping_add(a2);
  let b4 = a4.wrapping_sub(a2);
  let b6 = a0.wrapping_sub(a6);

  let a1 = d3
    .wrapping_neg()
    .wrapping_add(d5)
    .wrapping_sub(d7)
    .wrapping_sub(d7.wrapping_shr(1));
  let a3 = d1
    .wrapping_add(d7)
    .wrapping_sub(d3)
    .wrapping_sub(d3.wrapping_shr(1));
  let a5 = d1
    .wrapping_neg()
    .wrapping_add(d7)
    .wrapping_add(d5)
    .wrapping_add(d5.wrapping_shr(1));
  let a7 = d3
    .wrapping_add(d5)
    .wrapping_add(d1)
    .wrapping_add(d1.wrapping_shr(1));
  let b1 = a1.wrapping_add(a7.wrapping_shr(2));
  let b7 = a7.wrapping_sub(a1.wrapping_shr(2));
  let b3 = a3.wrapping_add(a5.wrapping_shr(2));
  let b5 = a3.wrapping_shr(2).wrapping_sub(a5);

  Ok([
    b0.wrapping_add(b7),
    b2.wrapping_add(b5),
    b4.wrapping_add(b3),
    b6.wrapping_add(b1),
    b6.wrapping_sub(b1),
    b4.wrapping_sub(b3),
    b2.wrapping_sub(b5),
    b0.wrapping_sub(b7),
  ])
}

pub(crate) fn idct8_add(
  coefficients: &[i32; 64],
  destination: &mut [u8],
  stride: u64,
) -> Result<(), Never> {
  let Ok(block) = pass_twice(coefficients.as_chunks::<8>().0, idct8_1d);
  add_block(&block, destination, stride)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Debug)]
  enum TestFailure {
    MalformedTransform,
    MissingSample,
  }

  fn reference_pass(input: &[i32]) -> Result<Vec<i32>, TestFailure> {
    match *input {
      [a, b, c, d] => {
        let e = a.wrapping_add(c);
        let f = a.wrapping_sub(c);
        let g = b.wrapping_shr(1).wrapping_sub(d);
        let h = b.wrapping_add(d.wrapping_shr(1));
        Ok(vec![
          e.wrapping_add(h),
          f.wrapping_add(g),
          f.wrapping_sub(g),
          e.wrapping_sub(h),
        ])
      }
      [a, b, c, d, e, f, g, h] => {
        let Ok(transformed) = idct8_1d([a, b, c, d, e, f, g, h]);
        Ok(transformed.to_vec())
      }
      _ => Err(TestFailure::MalformedTransform),
    }
  }

  fn reference_add(
    coefficients: &[i32],
    width: u8,
    destination: &mut [u8],
    stride: u64,
  ) -> Result<(), TestFailure> {
    let Ok(row_width) = index(width);
    let mut rows = Vec::new();

    for row in coefficients.chunks_exact(row_width.max(1)) {
      let result = reference_pass(row)?;
      rows.push(result);
    }

    for column in 0..width {
      let Ok(column_index) = index(column);
      let mut source = Vec::new();

      for row in &rows {
        let value = row
          .get(column_index)
          .ok_or(TestFailure::MalformedTransform)?;
        source.push(*value);
      }

      let transformed = reference_pass(&source)?;

      for (row, &delta) in (0..width).zip(&transformed) {
        let position = stride
          .wrapping_mul(u64::from(row))
          .wrapping_add(u64::from(column));
        let Ok(native) = index(position);
        let sample = destination
          .get_mut(native)
          .ok_or(TestFailure::MissingSample)?;
        let result = i32::from(*sample).wrapping_add(delta.wrapping_add(32).wrapping_shr(6));
        let Ok(clipped) = wrapped::<_, u8>(result.clamp(0, 255));
        *sample = clipped;
      }
    }

    Ok(())
  }

  fn next_random(seed: &mut u64) -> Result<u64, Never> {
    *seed ^= seed.wrapping_shl(13);
    *seed ^= seed.wrapping_shr(7);
    *seed ^= seed.wrapping_shl(17);
    Ok(*seed)
  }

  fn random_coefficients<const N: usize>(
    seed: &mut u64,
    magnitude: u64,
  ) -> Result<[i32; N], Never> {
    let mut block = [0; N];
    let Ok(amplitude) = wrapped::<_, i32>(magnitude);
    let modulus = magnitude.wrapping_mul(2).wrapping_add(1).max(1);

    for coefficient in &mut block {
      let Ok(random) = next_random(seed);
      let Ok(value) = wrapped::<_, i32>(random.wrapping_rem(modulus));
      *coefficient = value.wrapping_sub(amplitude);
    }

    Ok(block)
  }

  #[test]
  fn vectorised_transforms_match_reference() -> Result<(), TestFailure> {
    let mut rng = 0x1234_5678_9abc_def1_u64;

    for iteration in 0_u32..5000 {
      let magnitude = match iteration.wrapping_rem(4) {
        0 => 64,
        1 => 1024,
        2 => 8192,
        _ => 32_768,
      };
      let Ok(d4) = random_coefficients::<16>(&mut rng, magnitude);
      let Ok(d8) = random_coefficients::<64>(&mut rng, magnitude);
      let mut base = [0; 192];

      for sample in &mut base {
        let Ok(random) = next_random(&mut rng);
        let Ok(byte) = wrapped::<_, u8>(random);
        *sample = byte;
      }

      let mut copy4 = base;
      let mut reference4 = base;
      let [_, _, _, actual4 @ ..] = &mut copy4;
      let [_, _, _, expected4 @ ..] = &mut reference4;
      let Ok(()) = idct4_add(&d4, actual4, 24);
      reference_add(&d4, 4, expected4, 24)?;
      assert_eq!(actual4, expected4, "4x4 iteration {iteration}");
      let mut copy8 = base;
      let [_, _, _, _, _, actual8 @ ..] = &mut copy8;
      let [_, _, _, _, _, expected8 @ ..] = &mut base;
      let Ok(()) = idct8_add(&d8, actual8, 24);
      reference_add(&d8, 8, expected8, 24)?;
      assert_eq!(actual8, expected8, "8x8 iteration {iteration}");
    }

    Ok(())
  }

  #[test]
  fn idct4_dc_only_matches_shortcut() {
    let mut d = [0i32; 16];
    *{
      let [_explicit_part, ..] = &mut d;
      _explicit_part
    } = 640;
    let mut a = [100u8; 16];
    let mut b = [100u8; 16];
    let Ok(()) = idct4_add(&d, &mut a, 4);
    let Ok(()) = idct4_dc_add(640, &mut b, 4);
    assert_eq!(a, b);
    assert!(a.iter().all(|&v| v == 110));
  }

  #[test]
  fn idct4_hand_example() {
    let mut d = [0i32; 16];
    *{
      let [_explicit_part, ..] = &mut d;
      _explicit_part
    } = 64;
    *{
      let [_, _explicit_part, ..] = &mut d;
      _explicit_part
    } = 64;
    let mut out = [0u8; 16];
    let Ok(()) = idct4_add(&d, &mut out, 4);

    for out_row in <[_]>::iter(<[_]>::as_chunks::<4>(&out).0) {
      assert_eq!(out_row, &[2, 2, 1, 0]);
    }
  }

  #[test]
  fn idct8_dc() {
    let mut d = [0i32; 64];
    *{
      let [_explicit_part, ..] = &mut d;
      _explicit_part
    } = i32::wrapping_mul(64, 5);
    let mut out = [10u8; 64];
    let Ok(()) = idct8_add(&d, &mut out, 8);
    assert!(out.iter().all(|&v| v == 15));
  }

  #[test]
  fn luma_dc_flat() {
    let mut c = [0i32; 16];
    *{
      let [_explicit_part, ..] = &mut c;
      _explicit_part
    } = 4;
    let Ok(()) = luma_dc_dequant(
      &mut c,
      Dequantization {
        weight: 256,
        quantizer: 36,
      },
    );
    assert!(c.iter().all(|&v| v == i32::wrapping_mul(4, 256)));
  }

  #[test]
  fn chroma_dc() {
    let mut c = [1, 0, 0, 0];
    let Ok(()) = chroma_dc_dequant_420(
      &mut c,
      Dequantization {
        weight: 256,
        quantizer: 6,
      },
    );

    assert_eq!(c, [16, 16, 16, 16]);
  }

  #[test]
  fn scale_functions() {
    assert_eq!(
      ({
        let Ok(explicit_value) = scale4(
          2,
          Dequantization {
            weight: 160,
            quantizer: 24,
          },
        );
        explicit_value
      }),
      i32::wrapping_mul(2, 160)
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = scale4(
          2,
          Dequantization {
            weight: 160,
            quantizer: 12,
          },
        );
        explicit_value
      }),
      i32::wrapping_shr(i32::wrapping_add(i32::wrapping_mul(2, 160), 2), 2_u32)
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = scale8(
          1,
          Dequantization {
            weight: 320,
            quantizer: 36,
          },
        );
        explicit_value
      }),
      320
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = scale8(
          1,
          Dequantization {
            weight: 320,
            quantizer: 30,
          },
        );
        explicit_value
      }),
      i32::wrapping_shr(i32::wrapping_add(320, 1), 1_u32)
    );
  }

  #[test]
  fn huge_levels_dequantise_without_overflow_panics() {
    let mut c = [
      ::core::primitive::i32::wrapping_div(i32::MAX, 2),
      ::core::primitive::i32::wrapping_div(i32::MAX, 3),
      ::core::primitive::i32::wrapping_div((i32::MAX).wrapping_neg(), 2),
      7,
    ];
    let Ok(()) = chroma_dc_dequant_420(
      &mut c,
      Dequantization {
        weight: 400,
        quantizer: 51,
      },
    );
    let Ok(_) = scale4(
      i32::MAX,
      Dequantization {
        weight: 400,
        quantizer: 51,
      },
    );
    let Ok(_) = scale4(
      i32::MAX,
      Dequantization {
        weight: 400,
        quantizer: 0,
      },
    );
    let Ok(_) = scale8(
      i32::MIN,
      Dequantization {
        weight: 400,
        quantizer: 51,
      },
    );
    let Ok(_) = scale8(
      i32::MIN,
      Dequantization {
        weight: 400,
        quantizer: 0,
      },
    );
  }
}
