//! Safe ARM NEON WHT 4x4 inverse transform
//!
//! Port of the `inv_txfm_add_wht_wht_4x4_8bpc_neon` assembly function
//! from `src/arm/64/itx.S` to safe Rust NEON intrinsics.
#![allow(clippy::too_many_arguments)]
#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

#[cfg(target_arch = "aarch64")]
use archmage::{Arm64, arcane, rite};

#[cfg(target_arch = "aarch64")]
use archmage::intrinsics::aarch64 as safe_simd;

#[cfg(target_arch = "aarch64")]
use crate::src::error::{Rav1dError, Rav1dResult};
#[cfg(target_arch = "aarch64")]
use console_core_never::Never;
#[cfg(target_arch = "aarch64")]
use console_core_number_conversion::indices;
#[cfg(target_arch = "aarch64")]
use core::ops::Range;

#[cfg(target_arch = "aarch64")]
#[derive(Clone, Copy)]
pub(crate) struct WhtOutput {
  pub(crate) base: u64,
  pub(crate) pitch: i64,
}

#[cfg(target_arch = "aarch64")]
impl WhtOutput {
  fn row_window(self, row: u8) -> Rav1dResult<Range<u64>> {
    let offset = i64::from(row).wrapping_mul(self.pitch);
    let start = self.base.wrapping_add_signed(offset);
    let end = start.checked_add(4).ok_or(Rav1dError::EINVAL)?;
    Ok(start..end)
  }
}

/// Apply the 4x4 inverse Walsh–Hadamard transform in the source lane order.
#[cfg(target_arch = "aarch64")]
#[arcane]
pub(crate) fn inv_txfm_add_wht_wht_4x4_8bpc_neon_inner(
  _token: Arm64,
  dst: &mut [u8],
  output: WhtOutput,
  coeff: &mut [i16],
) -> Rav1dResult {
  let (coefficients, _) = coeff.split_first_chunk_mut::<16>().ok_or(Rav1dError::EINVAL)?;
  let (columns, _) = coefficients.as_chunks::<4>();
  let planes: &[[i16; 4]; 4] = columns.try_into().map_err(|_| Rav1dError::EINVAL)?;
  let shifted = planes.each_ref().map(|column| vshr_n_s16::<2>(safe_simd::vld1_s16(column)));
  let Ok(first) = iwht4(shifted);
  let Ok(transposed) = transpose_4x4h(first);
  let Ok([row0, row1, row2, row3]) = iwht4(transposed);
  let first_pair = vcombine_s16(row0, row1);
  let last_pair = vcombine_s16(row2, row3);
  let mut packed = [[0u8; 8]; 2];
  let (rows, _) = packed.as_flattened_mut().as_chunks_mut::<4>();

  for (ordinal, row) in (0u8..4).zip(rows) {
    let range = output.row_window(ordinal)?;
    let Ok(index_range) = indices(range);
    let bytes = dst.get(index_range).ok_or(Rav1dError::EINVAL)?;
    let samples: &[u8; 4] = bytes.try_into().map_err(|_| Rav1dError::EINVAL)?;
    *row = *samples;
  }

  let [initial_first, initial_last] = packed.each_ref().map(|bytes| safe_simd::vld1_u8(bytes));
  let sum_first = vreinterpretq_s16_u16(vaddw_u8(vreinterpretq_u16_s16(first_pair), initial_first));
  let sum_last = vreinterpretq_s16_u16(vaddw_u8(vreinterpretq_u16_s16(last_pair), initial_last));
  let [packed_first, packed_last] = &mut packed;
  safe_simd::vst1_u8(packed_first, vqmovun_s16(sum_first));
  safe_simd::vst1_u8(packed_last, vqmovun_s16(sum_last));
  let (result_rows, _) = packed.as_flattened().as_chunks::<4>();

  for (ordinal, result) in (0u8..4).zip(result_rows) {
    let range = output.row_window(ordinal)?;
    let Ok(index_range) = indices(range);
    let bytes = dst.get_mut(index_range).ok_or(Rav1dError::EINVAL)?;
    let samples: &mut [u8; 4] = bytes.try_into().map_err(|_| Rav1dError::EINVAL)?;
    *samples = *result;
  }

  coefficients.fill(0);
  Ok(())
}

/// WHT inverse butterfly: the iwht4 macro from itx.S lines 426-435.
///
/// Input:  (v16=in0, v17=in1, v18=in2, v19=in3)
/// Output: (out0, out1, out2, out3)
///
/// ```text
/// t0 = in0 + in1
/// t2 = in2 - in3
/// t4 = (t0 - t2) >> 1
/// t1 = t4 - in1      (stored in out2 position)
/// t3 = t4 - in3      (stored in out1 position)
/// out0 = t0 - t3
/// out1 = t3
/// out2 = t1
/// out3 = t2 + t1
/// ```
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn iwht4([v16, v17, v18, v19]: [int16x4_t; 4]) -> Result<[int16x4_t; 4], Never> {


  let t0 = vadd_s16(v16, v17);

  let t2 = vsub_s16(v18, v19);

  let diff = vsub_s16(t0, t2);

  let t4 = vshr_n_s16::<1>(diff);

  let t1 = vsub_s16(t4, v17);

  let t3 = vsub_s16(t4, v19);

  let out3 = vadd_s16(t2, t1);

  let out0 = vsub_s16(t0, t3);

  Ok([out0, t3, t1, out3])
}

/// 4x4 transpose for int16x4_t vectors.
///
/// Matches the `transpose_4x4h` macro from util.S lines 230-239.
/// Uses trn1/trn2 at 16-bit level, then trn1/trn2 at 32-bit level.
#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn transpose_4x4h([r0, r1, r2, r3]: [int16x4_t; 4]) -> Result<[int16x4_t; 4], Never> {


  let t4 = vtrn1_s16(r0, r1);

  let t5 = vtrn2_s16(r0, r1);

  let t6 = vtrn1_s16(r2, r3);

  let t7 = vtrn2_s16(r2, r3);



  let t4_32 = vreinterpret_s32_s16(t4);
  let t5_32 = vreinterpret_s32_s16(t5);
  let t6_32 = vreinterpret_s32_s16(t6);
  let t7_32 = vreinterpret_s32_s16(t7);


  let o0_32 = vtrn1_s32(t4_32, t6_32);

  let o2_32 = vtrn2_s32(t4_32, t6_32);

  let o1_32 = vtrn1_s32(t5_32, t7_32);

  let o3_32 = vtrn2_s32(t5_32, t7_32);


  let o0 = vreinterpret_s16_s32(o0_32);
  let o1 = vreinterpret_s16_s32(o1_32);
  let o2 = vreinterpret_s16_s32(o2_32);
  let o3 = vreinterpret_s16_s32(o3_32);

  Ok([o0, o1, o2, o3])
}

#[cfg(test)]
#[cfg(target_arch = "aarch64")]
mod tests {
  use super::*;
  use archmage::SimdToken;

  /// Test that the NEON WHT 4x4 produces identical output to the scalar path.
  #[test]
  fn test_wht_4x4_neon_matches_scalar() -> Rav1dResult {

    let test_coeffs: &[[i16; 16]] = &[

      [
        100, -50, 30, -20, 40, -30, 20, -10, 15, -8, 4, -2, 7, -3, 1, 0,
      ],

      [64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],

      [
        32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32,
      ],

      [
        100, -100, 100, -100, -100, 100, -100, 100, 100, -100, 100, -100, -100, 100, -100, 100,
      ],

      [
        500, -400, 300, -200, 400, -300, 200, -100, 300, -200, 100, 0, 200, -100, 50, -25,
      ],

      [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ];

    let token = match Arm64::summon() {
      Some(t) => t,
      None => {
        eprintln!("Skipping NEON test: Arm64 not available");
        return Ok(());
      }
    };

    let stride: isize = 16;

    for (i, coeffs) in test_coeffs.iter().enumerate() {
      let mut coeff_scalar = *coeffs;
      let mut coeff_neon = *coeffs;


      let mut dst_scalar = [0u8; 64];

      for (j, byte) in dst_scalar.iter_mut().enumerate() {
        *byte = (j * 7 + 128).wrapping_rem(256) as u8;
      }

      let mut dst_neon = dst_scalar;


      super::super::itx_arm::inv_txfm_add_wht_wht_4x4_8bpc_inner(
        &mut dst_scalar,
        0,
        stride,
        &mut coeff_scalar,
        0,
        255,
      );


      let Ok(pitch) = console_core_number_conversion::fixed_offset(stride);
      let output = WhtOutput { base: 0, pitch };
      inv_txfm_add_wht_wht_4x4_8bpc_neon_inner(token, &mut dst_neon, output, &mut coeff_neon)?;


      for row in 0..4 {
        let off = (::core::num::Wrapping(row) * ::core::num::Wrapping(::core::primitive::isize::cast_unsigned(stride))).0;
        assert_eq!(
          &dst_neon[off..off.wrapping_add(4)],
          &dst_scalar[off..off.wrapping_add(4)],
          "Row {row} mismatch in test pattern {i}: \
                     neon={:?} scalar={:?}",
          &dst_neon[off..off.wrapping_add(4)],
          &dst_scalar[off..off.wrapping_add(4)],
        );
      }


      assert!(
        coeff_neon.iter().all(|&c| c == 0),
        "NEON coefficients not zeroed in test pattern {i}"
      );
      assert!(
        coeff_scalar.iter().all(|&c| c == 0),
        "Scalar coefficients not zeroed in test pattern {i}"
      );
    }
    Ok(())
  }

  /// Test that the NEON WHT handles saturation correctly at boundaries.
  #[test]
  fn test_wht_4x4_neon_saturation() -> Rav1dResult {
    let token = match Arm64::summon() {
      Some(t) => t,
      None => {
        eprintln!("Skipping NEON test: Arm64 not available");
        return Ok(());
      }
    };

    let stride: isize = 4;


    {
      let mut coeff_scalar = [400i16; 16];
      let mut coeff_neon = coeff_scalar;
      let mut dst_scalar = [250u8; 16];
      let mut dst_neon = dst_scalar;

      super::super::itx_arm::inv_txfm_add_wht_wht_4x4_8bpc_inner(
        &mut dst_scalar,
        0,
        stride,
        &mut coeff_scalar,
        0,
        255,
      );
      let Ok(pitch) = console_core_number_conversion::fixed_offset(stride);
      let output = WhtOutput { base: 0, pitch };
      inv_txfm_add_wht_wht_4x4_8bpc_neon_inner(token, &mut dst_neon, output, &mut coeff_neon)?;
      assert_eq!(dst_neon, dst_scalar, "Overflow saturation mismatch");
    }


    {
      let mut coeff_scalar = [-400i16; 16];
      let mut coeff_neon = coeff_scalar;
      let mut dst_scalar = [5u8; 16];
      let mut dst_neon = dst_scalar;

      super::super::itx_arm::inv_txfm_add_wht_wht_4x4_8bpc_inner(
        &mut dst_scalar,
        0,
        stride,
        &mut coeff_scalar,
        0,
        255,
      );
      let Ok(pitch) = console_core_number_conversion::fixed_offset(stride);
      let output = WhtOutput { base: 0, pitch };
      inv_txfm_add_wht_wht_4x4_8bpc_neon_inner(token, &mut dst_neon, output, &mut coeff_neon)?;
      assert_eq!(dst_neon, dst_scalar, "Underflow saturation mismatch");
    }
    Ok(())
  }
}
