//! AArch64 4×4 DCT, ADST, flipped ADST and identity inverse transforms.
//! The first pass reads column-major coefficient lanes. The transpose makes
//! the second pass contiguous, and the final shift rounds before adding the
//! residual to 8-bit destination pixels.

#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]

use archmage::intrinsics::aarch64 as safe_simd;
use archmage::{Arm64, arcane, rite};
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_pixel_buffers::RowLayout;
use core::arch::aarch64::*;

use super::itx_arm_neon_common::{
  IADST4_COEFFS, IDCT_COEFFS, IDENTITY_SCALE, transpose_4x4h,
};
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use crate::src::levels::{self, TxfmType};

type Lanes = [int16x4_t; 4];

#[rite(neon)]
fn idct_4h([first, second, third, fourth]: Lanes) -> Result<Lanes, Never> {
  let [a, b, c, d, ..] = IDCT_COEFFS;
  let factors = safe_simd::vld1_s16(&[a, b, c, d]);
  let odd_sum = vmlal_lane_s16::<2>(vmull_lane_s16::<3>(second, factors), fourth, factors);
  let odd_difference = vmlsl_lane_s16::<3>(vmull_lane_s16::<2>(second, factors), fourth, factors);
  let even_sum = vmlal_lane_s16::<0>(vmull_lane_s16::<0>(first, factors), third, factors);
  let even_difference = vmlsl_lane_s16::<0>(vmull_lane_s16::<0>(first, factors), third, factors);
  let t0 = vqrshrn_n_s32::<12>(even_sum);
  let t1 = vqrshrn_n_s32::<12>(even_difference);
  let t2 = vqrshrn_n_s32::<12>(odd_difference);
  let t3 = vqrshrn_n_s32::<12>(odd_sum);

  Ok([vqadd_s16(t0, t3), vqadd_s16(t1, t2), vqsub_s16(t1, t2), vqsub_s16(t0, t3)])
}

#[rite(neon)]
fn iadst_4h([first, second, third, fourth]: Lanes) -> Result<Lanes, Never> {
  let [a, b, c, d, ..] = IADST4_COEFFS;
  let factors = safe_simd::vld1_s16(&[a, b, c, d]);
  let spread = vaddw_s16(vsubl_s16(first, third), fourth);
  let positive = vmlal_lane_s16::<2>(
    vmlal_lane_s16::<1>(vmull_lane_s16::<0>(first, factors), third, factors),
    fourth,
    factors,
  );
  let negative = vmlsl_lane_s16::<1>(
    vmlsl_lane_s16::<0>(vmull_lane_s16::<2>(first, factors), third, factors),
    fourth,
    factors,
  );
  let center = vmull_lane_s16::<3>(second, factors);
  let mixed = vsubq_s32(vaddq_s32(positive, negative), center);
  let alternate = vmulq_s32(spread, vdupq_n_s32(3344));

  Ok([
    vqrshrn_n_s32::<12>(vaddq_s32(positive, center)),
    vqrshrn_n_s32::<12>(vaddq_s32(negative, center)),
    vqrshrn_n_s32::<12>(alternate),
    vqrshrn_n_s32::<12>(mixed),
  ])
}

#[derive(Clone, Copy)]
enum TransformAxis {
  Dct,
  Adst,
  FlipAdst,
  Identity,
}

struct Axes {
  row: TransformAxis,
  column: TransformAxis,
}

fn axes(kind: TxfmType) -> Rav1dResult<Axes> {
  use TransformAxis::{Adst, Dct, FlipAdst, Identity};

  let (row, column) = match kind {
    levels::DCT_DCT => (Dct, Dct),
    levels::ADST_ADST => (Adst, Adst),
    levels::FLIPADST_FLIPADST => (FlipAdst, FlipAdst),
    levels::IDTX => (Identity, Identity),
    levels::ADST_DCT => (Dct, Adst),
    levels::DCT_ADST => (Adst, Dct),
    levels::FLIPADST_DCT => (Dct, FlipAdst),
    levels::DCT_FLIPADST => (FlipAdst, Dct),
    levels::ADST_FLIPADST => (FlipAdst, Adst),
    levels::FLIPADST_ADST => (Adst, FlipAdst),
    levels::H_DCT => (Dct, Identity),
    levels::V_DCT => (Identity, Dct),
    levels::H_ADST => (Adst, Identity),
    levels::V_ADST => (Identity, Adst),
    levels::H_FLIPADST => (FlipAdst, Identity),
    levels::V_FLIPADST => (Identity, FlipAdst),
    _unsupported => return Err(EINVAL),
  };

  Ok(Axes { row, column })
}

#[rite(neon)]
fn apply_axis(lanes: Lanes, axis: TransformAxis) -> Result<Lanes, Never> {
  match axis {
    TransformAxis::Dct => idct_4h(lanes),
    TransformAxis::Adst => iadst_4h(lanes),
    TransformAxis::FlipAdst => {
      let Ok([first, second, third, fourth]) = iadst_4h(lanes);
      Ok([fourth, third, second, first])
    }
    TransformAxis::Identity => {
      let scale = vdup_n_s16(IDENTITY_SCALE);
      Ok(lanes.map(|value| vqadd_s16(value, vqrdmulh_s16(value, scale))))
    }
  }
}

#[rite(neon)]
fn add_pair(pixels: [&mut [u8; 4]; 2], residuals: [int16x4_t; 2]) -> Result<(), Never> {
  let [first, second] = pixels;
  let [a, b, c, d] = *first;
  let [e, f, g, h] = *second;
  let [primary, secondary] = residuals;
  let residual = vrshrq_n_s16::<4>(vcombine_s16(primary, secondary));
  let mut bytes = [a, b, c, d, e, f, g, h];
  let current = safe_simd::vld1_u8(&bytes);
  let wide = vaddw_u8(vreinterpretq_u16_s16(residual), current);
  safe_simd::vst1_u8(&mut bytes, vqmovun_s16(vreinterpretq_s16_u16(wide)));
  let [r0, r1, r2, r3, r4, r5, r6, r7] = bytes;
  *first = [r0, r1, r2, r3];
  *second = [r4, r5, r6, r7];
  Ok(())
}

pub(crate) struct Transform4<'a> {
  pub(crate) pixels: &'a mut [u8],
  pub(crate) origin: (u64, i64),
  pub(crate) coefficients: &'a mut [i16],
  pub(crate) eob: i32,
}

#[arcane]
pub(crate) fn transform_4x4_neon(_token: Arm64, block: Transform4<'_>, kind: TxfmType) -> Rav1dResult {
  let passes = axes(kind)?;
  let (base, pitch) = block.origin;
  let layout = RowLayout::new(Size { width: 4, height: 4 }, base, pitch).map_err(|_extent| EINVAL)?;
  let mut rows = layout.rows_mut(block.pixels).map_err(|_bounds| EINVAL)?;
  let first = rows.next().and_then(<[_]>::first_chunk_mut).ok_or(EINVAL)?;
  let second = rows.next().and_then(<[_]>::first_chunk_mut).ok_or(EINVAL)?;
  let third = rows.next().and_then(<[_]>::first_chunk_mut).ok_or(EINVAL)?;
  let fourth = rows.next().and_then(<[_]>::first_chunk_mut).ok_or(EINVAL)?;

  let [a, b, c, d] = match (kind, block.eob) {
    (levels::DCT_DCT, 0) => {
      let first_coefficient = block.coefficients.first_mut().ok_or(EINVAL)?;
      let scale = vdup_n_s16(23168);
      let initial = vdup_n_s16(*first_coefficient);
      let row = vqrdmulh_s16(initial, scale);
      let result = vqrdmulh_s16(row, scale);
      *first_coefficient = 0;
      [result; 4]
    }
    _full => {
      let coefficients = block.coefficients.first_chunk_mut::<16>().ok_or(EINVAL)?;
      let (groups, _unused) = coefficients.as_chunks::<4>();
      let input = match groups {
        [g0, g1, g2, g3] => [safe_simd::vld1_s16(g0), safe_simd::vld1_s16(g1), safe_simd::vld1_s16(g2), safe_simd::vld1_s16(g3)],
        _wrong_shape => return Err(EINVAL),
      };
      let Ok([r0, r1, r2, r3]) = apply_axis(input, passes.row);
      let (t0, t1, t2, t3) = transpose_4x4h(r0, r1, r2, r3);
      let Ok(result) = apply_axis([t0, t1, t2, t3], passes.column);
      coefficients.fill(0);
      result
    }
  };
  let Ok(()) = add_pair([first, second], [a, b]);
  let Ok(()) = add_pair([third, fourth], [c, d]);

  Ok(())
}

#[cfg(test)]
pub(super) fn benchmark<const KIND: TxfmType>() -> Result<
  impl Fn(
    Arm64,
    &mut [u8],
    console_core_api_compatibility::NativeSize,
    console_core_api_compatibility::NativeOffset,
    &mut [i16],
    i32,
    i32,
  ),
  Never,
> {
  Ok(|token, pixels: &mut [u8], base, stride, coefficients: &mut [i16], eob, _bitdepth| {
    let Ok(first) = console_core_number_conversion::fixed_index(base);
    let Ok(pitch) = console_core_number_conversion::fixed_offset(stride);
    let block = Transform4 { pixels, origin: (first, pitch), coefficients, eob };
    assert_eq!(transform_4x4_neon(token, block, KIND), Ok(()));
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use archmage::SimdToken;
  use console_core_api_compatibility::{NativeOffset, NativeSize};
  use console_core_number_conversion::wrapped;

  type ScalarReference = fn(&mut [u8], NativeSize, NativeOffset, &mut [i16], i32, i32);

  fn compare(kind: TxfmType, scalar: ScalarReference) -> Rav1dResult {
    let token = match Arm64::summon() {
      Some(enabled) => enabled,
      None => return Ok(()),
    };
    let coefficients = [
      [64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
      [50, -25, 15, -10, 20, -15, 10, -5, 8, -4, 2, -1, 4, -2, 1, 0],
      [32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 16],
      [0; 16],
    ];
    let mut initial = [0_u8; 64];

    for (index, pixel) in (0_u32..).zip(&mut initial) {
      let Ok(value) = wrapped::<_, u8>(index.wrapping_mul(7).wrapping_add(128));
      *pixel = value;
    }

    for samples in coefficients {
      let (mut expected, mut actual) = (initial, initial);
      let mut reference_coefficients = samples;
      let mut vector_coefficients = samples;
      scalar(&mut expected, 0, 16, &mut reference_coefficients, 1, 255);
      let request = Transform4 {
        pixels: &mut actual,
        origin: (0, 16),
        coefficients: &mut vector_coefficients,
        eob: 1,
      };
      transform_4x4_neon(token, request, kind)?;

      for (output, reference) in actual.as_chunks::<16>().0.iter().zip(expected.as_chunks::<16>().0) {
        let [a, b, c, d, ..] = output;
        let [e, f, g, h, ..] = reference;

        for (ours, theirs) in [a, b, c, d].into_iter().zip([e, f, g, h]) {
          let difference = i32::from(*ours).abs_diff(i32::from(*theirs));
          assert!(difference <= 15, "transform {kind}: {ours} versus {theirs}");
        }
      }

      assert_eq!(vector_coefficients, [0; 16]);
    }

    Ok(())
  }

  #[test]
  fn test_dct_dct_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::DCT_DCT, super::super::itx_arm::inv_txfm_add_dct_dct_4x4_8bpc_inner)
  }

  #[test]
  fn test_adst_adst_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::ADST_ADST, super::super::itx_arm::inv_txfm_add_adst_adst_4x4_8bpc_inner)
  }

  #[test]
  fn test_flipadst_flipadst_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::FLIPADST_FLIPADST, super::super::itx_arm::inv_txfm_add_flipadst_flipadst_4x4_8bpc_inner)
  }

  #[test]
  fn test_identity_identity_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::IDTX, super::super::itx_arm::inv_txfm_add_identity_identity_4x4_8bpc_inner)
  }

  #[test]
  fn test_dct_adst_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::ADST_DCT, super::super::itx_arm::inv_txfm_add_dct_adst_4x4_8bpc_inner)
  }

  #[test]
  fn test_adst_dct_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::DCT_ADST, super::super::itx_arm::inv_txfm_add_adst_dct_4x4_8bpc_inner)
  }

  #[test]
  fn test_dct_flipadst_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::FLIPADST_DCT, super::super::itx_arm::inv_txfm_add_dct_flipadst_4x4_8bpc_inner)
  }

  #[test]
  fn test_flipadst_dct_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::DCT_FLIPADST, super::super::itx_arm::inv_txfm_add_flipadst_dct_4x4_8bpc_inner)
  }

  #[test]
  fn test_adst_flipadst_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::FLIPADST_ADST, super::super::itx_arm::inv_txfm_add_adst_flipadst_4x4_8bpc_inner)
  }

  #[test]
  fn test_flipadst_adst_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::ADST_FLIPADST, super::super::itx_arm::inv_txfm_add_flipadst_adst_4x4_8bpc_inner)
  }

  #[test]
  fn test_dct_identity_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::H_DCT, super::super::itx_arm::inv_txfm_add_dct_identity_4x4_8bpc_inner)
  }

  #[test]
  fn test_identity_dct_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::V_DCT, super::super::itx_arm::inv_txfm_add_identity_dct_4x4_8bpc_inner)
  }

  #[test]
  fn test_adst_identity_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::H_ADST, super::super::itx_arm::inv_txfm_add_adst_identity_4x4_8bpc_inner)
  }

  #[test]
  fn test_identity_adst_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::V_ADST, super::super::itx_arm::inv_txfm_add_identity_adst_4x4_8bpc_inner)
  }

  #[test]
  fn test_flipadst_identity_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::H_FLIPADST, super::super::itx_arm::inv_txfm_add_flipadst_identity_4x4_8bpc_inner)
  }

  #[test]
  fn test_identity_flipadst_4x4_neon_matches_scalar() -> Rav1dResult {
    compare(levels::V_FLIPADST, super::super::itx_arm::inv_txfm_add_identity_flipadst_4x4_8bpc_inner)
  }

  #[test]
  fn test_dct_dct_dc_only_4x4() -> Rav1dResult {
    let token = match Arm64::summon() {
      Some(enabled) => enabled,
      None => return Ok(()),
    };

    for dc in [0, 1, -1, 100, -100, 500, -500, 1000, -1000] {
      let mut coefficients = [0_i16; 16];
      let [first, ..] = &mut coefficients;
      *first = dc;
      let mut pixels = [128_u8; 64];
      let request = Transform4 { pixels: &mut pixels, origin: (0, 16), coefficients: &mut coefficients, eob: 0 };
      transform_4x4_neon(token, request, levels::DCT_DCT)?;
      let [expected, ..] = pixels;

      for row in pixels.as_chunks::<16>().0 {
        let [a, b, c, d, ..] = row;
        assert_eq!([*a, *b, *c, *d], [expected; 4], "DC {dc}");
      }

      assert!(i32::from(expected).abs_diff(128) < 50);
      assert_eq!(coefficients, [0; 16]);
    }

    Ok(())
  }
}
