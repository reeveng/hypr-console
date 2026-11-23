//! Safe ARM NEON implementations for CDEF (Constrained Directional Enhancement Filter)
//!
//! CDEF applies direction-dependent filtering to remove coding artifacts
//! while preserving edges.
//!
//! # Scratch-buffer representation
//!
//! `tmp` is a `[u16; 12 * 12]` window holding the block plus two pixels of
//! context on every side, with unavailable positions set to [`CDEF_VERY_LARGE`]
//! `== 0x8000`. That constant is `i16::MIN` reinterpreted, and it is chosen —
//! as it is in dav1d's own `src/arm/64/cdef_tmpl.S` and in `src/cdef.rs`'s
//! scalar reference, which fills with `i16::MIN` — so that a sentinel is
//! *self-neutralising* in all three places it is read:
//!
//! * `min` is taken with an UNSIGNED compare (`umin` / [`vminq_u16`]), where
//!   `0x8000` is larger than any pixel, so it never lowers the minimum;
//! * `max` is taken with a SIGNED compare (`smax` / [`vmaxq_s16`]), where
//!   `0x8000` is `-32768`, so it never raises the maximum;
//! * `constrain` computes `clip = max(0, threshold - (|diff| >> shift))` with an
//!   unsigned saturating subtract, and a sentinel's `|diff|` is ~32768, which
//!   drives `clip` to 0 for every strength/damping pair the spec can signal
//!   (proved by `constrain_neutralises_sentinel_over_full_param_space`), so the
//!   tap contributes exactly 0 to `sum`.
//!
//! Using a smaller sentinel (e.g. 8191) satisfies the first and third bullets
//! but NOT the second: an edge block would take `max = 8191`, disabling the
//! upper half of the `iclip` that the scalar reference performs. That is a real
//! divergence, and `sentinel_must_not_raise_max` is the regression test for it.
//!
//! # Exact-window padding guards
//!
//! The top/bottom padding loops read `x_start..x_end` of a row that sits two
//! rows outside the block. `x_start` is 2 when `HAVE_LEFT` is absent, i.e. the
//! block is at the left edge of the frame, so the two left-padding columns are
//! *skipped* — but they live at `offset - 2`, which is the **tail of the
//! previous row**. Guarding from `offset` instead of `offset + x_start` would
//! therefore lock 2 pixels this code never reads, in a row a concurrent tile
//! worker may legitimately be writing (`backup2lines` saves whole rows of
//! `cdef_line_buf`), producing a false `DisjointMut` overlap panic. So every
//! guard here starts at `offset + x_start` and is `x_end - x_start` long.
//! `src/cdef.rs`'s scalar reference has carried this discipline since the
//! i686 report; these kernels are held to the same rule.
//!
//! The WRITE side is one guard per destination row, exactly `w` pixels wide —
//! never a single wide guard spanning `(h - 1) * stride + w`. The 12-pixel tap
//! window is a READ of `tmp`, a private stack buffer, so it never straddles a
//! picture row at all: the only picture reads are the padding copies above, and
//! each of those is bounded to the columns it actually touches.

#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

#[cfg(target_arch = "aarch64")]
use archmage::intrinsics::aarch64 as safe_simd;
#[cfg(target_arch = "aarch64")]
use archmage::{Arm64, arcane, rite};

use std::ffi::c_uint;

use crate::include::common::bitdepth::{BPC, BitDepth};
use crate::include::dav1d::picture::PicOffset;
use crate::src::cdef::{
  CdefBlockShape, CdefDispatch, CdefFilter, CdefTapOffsets, cdef_best_direction, cdef_line_costs,
  cdef_padding, cdef_tap_offsets, cdef_widen_direction_lines,
};
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{index, wrapped};

const CDEF_VERY_LARGE: u16 = 0x8000;

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn cdef_tap(tmp: &[u16; 144], coordinate: i32) -> Rav1dResult<uint16x8_t> {
  let column = u16::try_from(coordinate).map_err(|_outside_padding| EINVAL)?;
  let Ok(position) = index(column);
  let samples = tmp
    .get(position..)
    .and_then(<[_]>::first_chunk)
    .ok_or(EINVAL)?;
  Ok(safe_simd::vld1q_u16(samples))
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn cdef_constrain(
  p: uint16x8_t,
  px: uint16x8_t,
  threshold: uint16x8_t,
  neg_shift: int16x8_t,
) -> Result<int16x8_t, Never> {
  let adiff = vabdq_u16(p, px);
  let clip = vreinterpretq_s16_u16(vqsubq_u16(threshold, vshlq_u16(adiff, neg_shift)));
  let diff = vsubq_s16(vreinterpretq_s16_u16(p), vreinterpretq_s16_u16(px));
  Ok(vmaxq_s16(vminq_s16(diff, clip), vnegq_s16(clip)))
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn cdef_apply(px: int16x8_t, sum: int16x8_t) -> Result<int16x8_t, Never> {
  let neg = vreinterpretq_s16_u16(vcltq_s16(sum, vdupq_n_s16(0)));
  let biased = vaddq_s16(vaddq_s16(sum, neg), vdupq_n_s16(8));
  Ok(vaddq_s16(px, vshrq_n_s16::<4>(biased)))
}

#[cfg(target_arch = "aarch64")]
struct FilterParams {
  primary_threshold: u16,
  primary_shift: i16,
  secondary_threshold: u16,
  secondary_shift: i16,
  primary_tap: i16,
}

#[cfg(target_arch = "aarch64")]
#[derive(Clone, Copy)]
struct CdefStrengths {
  primary: u32,
  secondary: u32,
  damping: i32,
  depth_shift: u32,
}

#[cfg(target_arch = "aarch64")]
fn filter_params(strengths: CdefStrengths) -> Result<FilterParams, Never> {
  let shift = |threshold: u32| {
    let log = threshold.checked_ilog2();
    let difference = log.map_or(0, |power| {
      strengths.damping.wrapping_sub(power.cast_signed())
    });
    let Ok(negative) = wrapped::<_, i16>(difference.max(0).wrapping_neg());
    negative
  };
  let Ok(primary_threshold) = wrapped(strengths.primary);
  let Ok(secondary_threshold) = wrapped(strengths.secondary);
  let Ok(parity) = wrapped::<_, i16>(strengths.primary.wrapping_shr(strengths.depth_shift) & 1);

  Ok(FilterParams {
    primary_threshold,
    primary_shift: shift(strengths.primary),
    secondary_threshold,
    secondary_shift: shift(strengths.secondary),
    primary_tap: 4_i16.wrapping_sub(parity),
  })
}

#[cfg(target_arch = "aarch64")]
struct CdefVectors {
  primary_threshold: uint16x8_t,
  primary_shift: int16x8_t,
  secondary_threshold: uint16x8_t,
  secondary_shift: int16x8_t,
  primary_tap: i16,
}

#[cfg(target_arch = "aarch64")]
#[arcane]
fn cdef_vectors(_token: Arm64, params: &FilterParams) -> Result<CdefVectors, Never> {
  Ok(CdefVectors {
    primary_threshold: vdupq_n_u16(params.primary_threshold),
    primary_shift: vdupq_n_s16(params.primary_shift),
    secondary_threshold: vdupq_n_u16(params.secondary_threshold),
    secondary_shift: vdupq_n_s16(params.secondary_shift),
    primary_tap: params.primary_tap,
  })
}

#[cfg(target_arch = "aarch64")]
struct CdefRowContext<'a> {
  pixels: &'a [u16; 144],
  offsets: &'a CdefTapOffsets,
  vectors: CdefVectors,
}

#[cfg(target_arch = "aarch64")]
struct CdefRange {
  minimum: uint16x8_t,
  maximum: int16x8_t,
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn include_cdef_taps(range: &mut CdefRange, taps: [uint16x8_t; 2]) -> Result<(), Never> {
  let [first, second] = taps;
  range.minimum = vminq_u16(range.minimum, vminq_u16(first, second));
  range.maximum = vmaxq_s16(
    range.maximum,
    vmaxq_s16(vreinterpretq_s16_u16(first), vreinterpretq_s16_u16(second)),
  );
  Ok(())
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn cdef_tap_pair(
  samples: &[u16; 144],
  position: CdefTapPosition,
) -> Rav1dResult<[uint16x8_t; 2]> {
  let first = cdef_tap(samples, position.center.wrapping_add(position.distance))?;
  let second = cdef_tap(samples, position.center.wrapping_sub(position.distance))?;
  Ok([first, second])
}

#[cfg(target_arch = "aarch64")]
struct CdefTapPosition {
  center: i32,
  distance: i32,
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn cdef_filter_row<const PRI: bool, const SEC: bool>(
  context: &CdefRowContext<'_>,
  base: i32,
  pixels: uint16x8_t,
) -> Rav1dResult<int16x8_t> {
  let mut sum = vdupq_n_s16(0);
  let original = vreinterpretq_s16_u16(pixels);
  let mut range = CdefRange {
    minimum: pixels,
    maximum: original,
  };
  let offsets = context.offsets;
  let vectors = &context.vectors;

  let first = vectors.primary_tap;
  let primary_weights = [first, first & 3 | 2];
  let secondary_weights = [2, 1];

  for (primary, weight) in offsets.primary.into_iter().zip(primary_weights) {
    match ::core::primitive::bool::then_some(PRI, ()) {
      ::core::option::Option::Some(()) => {
        let position = CdefTapPosition {
          center: base,
          distance: primary,
        };
        let taps = cdef_tap_pair(context.pixels, position)?;

        for tap in taps {
          let Ok(delta) = cdef_constrain(
            tap,
            pixels,
            vectors.primary_threshold,
            vectors.primary_shift,
          );
          sum = vmlaq_n_s16(sum, delta, weight);
        }

        match ::core::primitive::bool::then_some(SEC, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(()) = include_cdef_taps(&mut range, taps);
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {}
    }
  }

  for directions in [offsets.secondary, offsets.other] {
    for (distance, weight) in directions.into_iter().zip(secondary_weights) {
      match ::core::primitive::bool::then_some(SEC, ()) {
        ::core::option::Option::Some(()) => {
          let position = CdefTapPosition {
            center: base,
            distance,
          };
          let taps = cdef_tap_pair(context.pixels, position)?;
          let Ok(()) = include_cdef_taps(&mut range, taps);

          for tap in taps {
            let Ok(delta) = cdef_constrain(
              tap,
              pixels,
              vectors.secondary_threshold,
              vectors.secondary_shift,
            );
            sum = vmlaq_n_s16(sum, delta, weight);
          }
        }
        ::core::option::Option::None => {}
      }
    }
  }

  let Ok(output) = cdef_apply(original, sum);
  Ok(match ::core::primitive::bool::then_some(SEC, ()) {
    ::core::option::Option::Some(()) => vminq_s16(
      vmaxq_s16(output, vreinterpretq_s16_u16(range.minimum)),
      range.maximum,
    ),
    ::core::option::Option::None => output,
  })
}

#[cfg(target_arch = "aarch64")]
fn cdef_row_base(row: u64) -> Rav1dResult<i32> {
  let row_2 = i32::try_from(row).map_err(|_invalid_row| EINVAL)?;
  Ok(row_2.wrapping_mul(12).wrapping_add(26))
}

#[cfg(target_arch = "aarch64")]
#[arcane]
fn cdef_filter_block_8bpc_neon<const W: u8, const H: u8, const PRI: bool, const SEC: bool>(
  _token: Arm64,
  dst: PicOffset,
  context: &mut CdefRowContext<'_>,
) -> Rav1dResult {
  use crate::include::common::bitdepth::BitDepth8;

  let extent = Size {
    width: u64::from(W),
    height: u64::from(H),
  };
  dst.for_rows_mut_with_context::<BitDepth8, _, _>(extent, context, |row, output, state| {
    let base = cdef_row_base(row)?;
    let pixels = cdef_tap(state.pixels, base)?;
    let filtered = cdef_filter_row::<PRI, SEC>(state, base, pixels)?;
    let packed = vmovn_u16(vreinterpretq_u16_s16(filtered));

    match W {
      8 => {
        let destination = output.first_chunk_mut::<8>().ok_or(EINVAL)?;
        safe_simd::vst1_u8(destination, packed);
      }
      4 => {
        let mut lanes = [0_u8; 8];
        safe_simd::vst1_u8(&mut lanes, packed);
        let [first, second, third, fourth, ..] = lanes;
        let destination = output.first_chunk_mut::<4>().ok_or(EINVAL)?;
        *destination = [first, second, third, fourth];
      }
      _invalid_width => return Err(EINVAL),
    }

    Ok(())
  })
}

#[cfg(target_arch = "aarch64")]
#[arcane]
fn cdef_filter_block_16bpc_neon<const W: u8, const H: u8, const PRI: bool, const SEC: bool>(
  _token: Arm64,
  dst: PicOffset,
  context: &mut CdefRowContext<'_>,
) -> Rav1dResult {
  use crate::include::common::bitdepth::BitDepth16;

  let extent = Size {
    width: u64::from(W),
    height: u64::from(H),
  };
  dst.for_rows_mut_with_context::<BitDepth16, _, _>(extent, context, |row, output, state| {
    let base = cdef_row_base(row)?;
    let pixels = cdef_tap(state.pixels, base)?;
    let filtered = cdef_filter_row::<PRI, SEC>(state, base, pixels)?;
    let packed = vreinterpretq_u16_s16(filtered);

    match W {
      8 => {
        let destination = output.first_chunk_mut::<8>().ok_or(EINVAL)?;
        safe_simd::vst1q_u16(destination, packed);
      }
      4 => {
        let destination = output.first_chunk_mut::<4>().ok_or(EINVAL)?;
        safe_simd::vst1_u16(destination, vget_low_u16(packed));
      }
      _invalid_width => return Err(EINVAL),
    }

    Ok(())
  })
}

#[cfg(target_arch = "aarch64")]
#[derive(Clone, Copy)]
enum CdefFilterStrength {
  Primary,
  Secondary,
  Both,
}

#[cfg(target_arch = "aarch64")]
struct CdefNeonBlock<'a, 'samples> {
  depth: BPC,
  destination: PicOffset<'a>,
  context: CdefRowContext<'samples>,
  strength: CdefFilterStrength,
}

#[cfg(target_arch = "aarch64")]
fn filter_neon_block<const W: u8, const H: u8>(
  token: Arm64,
  block: &mut CdefNeonBlock<'_, '_>,
) -> Rav1dResult {
  let dst = block.destination;
  let context = &mut block.context;

  match (block.depth, block.strength) {
    (BPC::BPC8, CdefFilterStrength::Primary) => {
      cdef_filter_block_8bpc_neon::<W, H, true, false>(token, dst, context)
    }
    (BPC::BPC8, CdefFilterStrength::Secondary) => {
      cdef_filter_block_8bpc_neon::<W, H, false, true>(token, dst, context)
    }
    (BPC::BPC8, CdefFilterStrength::Both) => {
      cdef_filter_block_8bpc_neon::<W, H, true, true>(token, dst, context)
    }
    (BPC::BPC16, CdefFilterStrength::Primary) => {
      cdef_filter_block_16bpc_neon::<W, H, true, false>(token, dst, context)
    }
    (BPC::BPC16, CdefFilterStrength::Secondary) => {
      cdef_filter_block_16bpc_neon::<W, H, false, true>(token, dst, context)
    }
    (BPC::BPC16, CdefFilterStrength::Both) => {
      cdef_filter_block_16bpc_neon::<W, H, true, true>(token, dst, context)
    }
  }
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn accumulate8(sums: &mut [i16; 16], start: u8, samples: int16x8_t) -> Rav1dResult {
  let Ok(offset) = index(start);
  let remaining = sums.get_mut(offset..).ok_or(EINVAL)?;
  let chunk = remaining.first_chunk_mut::<8>().ok_or(EINVAL)?;
  let current = safe_simd::vld1q_s16(chunk);
  safe_simd::vst1q_s16(chunk, vaddq_s16(current, samples));

  Ok(())
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn accumulate4(sums: &mut [i16; 16], start: u8, samples: int16x4_t) -> Rav1dResult {
  let Ok(offset) = index(start);
  let remaining = sums.get_mut(offset..).ok_or(EINVAL)?;
  let chunk = remaining.first_chunk_mut::<4>().ok_or(EINVAL)?;
  let current = safe_simd::vld1_s16(chunk);
  safe_simd::vst1_s16(chunk, vadd_s16(current, samples));

  Ok(())
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn reverse_lanes(samples: int16x8_t) -> Result<int16x8_t, Never> {
  let halves = vrev64q_s16(samples);

  Ok(vextq_s16::<4>(halves, halves))
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn cdef_dir_core(rows: &[int16x8_t; 8], variance: &mut c_uint) -> Rav1dResult<i32> {
  let mut diagonal_0 = [0_i16; 16];
  let mut diagonal_1 = [0_i16; 16];
  let mut offset_0 = [0_i16; 16];
  let mut offset_1 = [0_i16; 16];
  let mut offset_2 = [0_i16; 16];
  let mut offset_3 = [0_i16; 16];
  let mut horizontal = [0_i16; 8];
  let mut vertical = vdupq_n_s16(0);

  for (y, (&row, sum)) in (0..8_u8).zip(rows.iter().zip(&mut horizontal)) {
    *sum = vaddvq_s16(row);
    vertical = vaddq_s16(vertical, row);
    let Ok(backwards) = reverse_lanes(row);
    let half_y = y.wrapping_shr(1);
    accumulate8(&mut diagonal_0, y, row)?;
    accumulate8(&mut diagonal_1, y, backwards)?;
    accumulate8(&mut offset_2, 3_u8.wrapping_sub(half_y), row)?;
    accumulate8(&mut offset_3, half_y, row)?;

    let pairs = vget_low_s16(vpaddq_s16(row, row));
    accumulate4(&mut offset_0, y, pairs)?;
    accumulate4(&mut offset_1, y, vrev64_s16(pairs))?;
  }

  let mut vertical_sums = [0; 8];
  safe_simd::vst1q_s16(&mut vertical_sums, vertical);
  let Ok(line_0) = cdef_widen_direction_lines(diagonal_0);
  let Ok(line_1) = cdef_widen_direction_lines(offset_0);
  let Ok(line_2) = cdef_widen_direction_lines(horizontal);
  let Ok(line_3) = cdef_widen_direction_lines(offset_1);
  let Ok(line_4) = cdef_widen_direction_lines(diagonal_1);
  let Ok(line_5) = cdef_widen_direction_lines(offset_2);
  let Ok(line_6) = cdef_widen_direction_lines(vertical_sums);
  let Ok(line_7) = cdef_widen_direction_lines(offset_3);
  let Ok(costs) = cdef_line_costs([
    line_0, line_1, line_2, line_3, line_4, line_5, line_6, line_7,
  ]);

  cdef_best_direction(costs, variance)
}

#[cfg(target_arch = "aarch64")]
#[arcane]
fn cdef_find_dir_8bpc_neon(
  _token: Arm64,
  image: PicOffset,
  variance: &mut c_uint,
) -> Rav1dResult<i32> {
  use crate::include::common::bitdepth::BitDepth8;

  let mut rows = [vdupq_n_s16(0); 8];
  let mut context = (rows.iter_mut(), Ok(()), vdupq_n_s16(128));
  let Ok(()) = image.for_rows_with_context::<BitDepth8, _, _>(
    Size {
      width: 8,
      height: 8,
    },
    &mut context,
    |_row, pixels, (outputs, result, center)| match (outputs.next(), pixels.first_chunk::<8>()) {
      (Some(output), Some(chunk)) => {
        let values = safe_simd::vld1_u8(chunk);
        *output = vsubq_s16(vreinterpretq_s16_u16(vmovl_u8(values)), *center);
      }
      (None, None | Some(_)) | (Some(_), None) => *result = Err(EINVAL),
    },
  );
  context.1?;

  cdef_dir_core(&rows, variance)
}

#[cfg(target_arch = "aarch64")]
#[arcane]
fn cdef_find_dir_16bpc_neon(
  _token: Arm64,
  image: PicOffset,
  variance: &mut c_uint,
  reduced_bitdepth: u8,
) -> Rav1dResult<i32> {
  use crate::include::common::bitdepth::BitDepth16;

  let mut rows = [vdupq_n_s16(0); 8];
  let shift = vdupq_n_s16(i16::from(reduced_bitdepth).wrapping_neg());
  let mut context = (rows.iter_mut(), Ok(()), vdupq_n_s16(128), shift);
  let Ok(()) = image.for_rows_with_context::<BitDepth16, _, _>(
    Size {
      width: 8,
      height: 8,
    },
    &mut context,
    |_row, pixels, (outputs, result, center, lanes_shift)| match (
      outputs.next(),
      pixels.first_chunk::<8>(),
    ) {
      (Some(output), Some(chunk)) => {
        let values = safe_simd::vld1q_u16(chunk);
        *output = vsubq_s16(
          vreinterpretq_s16_u16(vshlq_u16(values, *lanes_shift)),
          *center,
        );
      }
      (None, None | Some(_)) | (Some(_), None) => *result = Err(EINVAL),
    },
  );
  context.1?;

  cdef_dir_core(&rows, variance)
}

#[cfg(target_arch = "aarch64")]
pub(crate) fn cdef_filter_dispatch<BD: BitDepth>(
  request: &CdefFilter<'_, '_, BD>,
) -> Rav1dResult<CdefDispatch> {
  use crate::src::ablate::{Family, is_off, note};
  use archmage::SimdToken as _;

  match is_off(Family::Cdef).then_some(()) {
    Some(()) => return Ok(CdefDispatch::Unavailable),
    None => {}
  }

  let Ok(size) = request.shape.size();
  let area = u64::from(size.width).wrapping_mul(u64::from(size.height));
  let Ok(()) = note(Family::Cdef, area);

  let strength = match (request.primary, request.secondary) {
    (0, 0) => return Ok(CdefDispatch::Applied),
    (0, _secondary) => CdefFilterStrength::Secondary,
    (_primary, 0) => CdefFilterStrength::Primary,
    (_primary, _secondary) => CdefFilterStrength::Both,
  };

  let token = match Arm64::summon() {
    Some(token) => token,
    None => return Ok(CdefDispatch::Unavailable),
  };

  let padding = cdef_padding(request, CDEF_VERY_LARGE.cast_signed())?;
  let offsets = cdef_tap_offsets(request.direction)?;
  let Ok(parameters) = filter_params(CdefStrengths {
    primary: request.primary.cast_unsigned(),
    secondary: u32::from(request.secondary),
    damping: i32::from(request.damping),
    depth_shift: u32::from(request.bitdepth.bitdepth().wrapping_sub(8)),
  });
  let Ok(vectors) = cdef_vectors(token, &parameters);
  let mut block = CdefNeonBlock {
    depth: BD::BPC,
    destination: request.destination,
    context: CdefRowContext {
      pixels: zerocopy::transmute_ref!(&padding),
      offsets: &offsets,
      vectors,
    },
    strength,
  };

  match request.shape {
    CdefBlockShape::EightByEight => filter_neon_block::<8, 8>(token, &mut block),
    CdefBlockShape::FourByEight => filter_neon_block::<4, 8>(token, &mut block),
    CdefBlockShape::FourByFour => filter_neon_block::<4, 4>(token, &mut block),
  }?;

  Ok(CdefDispatch::Applied)
}

#[cfg(target_arch = "aarch64")]
pub fn cdef_dir_dispatch<BD: BitDepth>(
  destination: PicOffset,
  variance: &mut c_uint,
  bitdepth: BD,
) -> Rav1dResult<Option<i32>> {
  use crate::include::common::bitdepth::BPC;
  use archmage::SimdToken as _;

  match crate::src::ablate::is_off(crate::src::ablate::Family::Cdef).then_some(()) {
    Some(()) => return Ok(None),
    None => {}
  }

  let token = match Arm64::summon() {
    Some(token) => token,
    None => return Ok(None),
  };

  let direction = match BD::BPC {
    BPC::BPC8 => cdef_find_dir_8bpc_neon(token, destination, variance)?,
    BPC::BPC16 => {
      let reduced_bitdepth = bitdepth.bitdepth().saturating_sub(8);
      let found = cdef_find_dir_16bpc_neon(token, destination, variance, reduced_bitdepth)?;
      found
    }
  };

  Ok(Some(direction))
}

#[cfg(all(test, target_arch = "aarch64"))]
mod tests {
  use super::*;
  use crate::src::cdef::CdefEdgeFlags;
  use crate::src::tables::dav1d_cdef_directions;
  use archmage::SimdToken as _;

  #[derive(Clone, Copy)]
  struct TestCdefRow {
    coordinate: u64,
    direction: i32,
    strengths: CdefStrengths,
  }

  fn neon_filter(tmp: &[i16; 144], dst: &[i32; 8], input: TestCdefRow) -> Rav1dResult<[i16; 8]> {
    let token = Arm64::summon().ok_or(EINVAL)?;
    let px = dst.map(|pixel| {
      let Ok(component) = wrapped::<_, u16>(pixel);
      component
    });
    let strength = match (input.strengths.primary, input.strengths.secondary) {
      (0, 0) => return Ok(px.map(u16::cast_signed)),
      (0, _secondary) => CdefFilterStrength::Secondary,
      (_primary, 0) => CdefFilterStrength::Primary,
      (_primary, _secondary) => CdefFilterStrength::Both,
    };
    let offsets = cdef_tap_offsets(input.direction)?;
    let Ok(parameters) = filter_params(input.strengths);
    let Ok(vectors) = cdef_vectors(token, &parameters);
    let context = CdefRowContext {
      pixels: zerocopy::transmute_ref!(tmp),
      offsets: &offsets,
      vectors,
    };
    let base = cdef_row_base(input.coordinate)?;
    run_row(token, &context, base, &px, strength)
  }

  #[arcane]
  fn run_row(
    _token: Arm64,
    context: &CdefRowContext<'_>,
    base: i32,
    samples: &[u16; 8],
    strength: CdefFilterStrength,
  ) -> Rav1dResult<[i16; 8]> {
    let pixels = safe_simd::vld1q_u16(samples);
    let output = match strength {
      CdefFilterStrength::Both => cdef_filter_row::<true, true>(context, base, pixels),
      CdefFilterStrength::Primary => cdef_filter_row::<true, false>(context, base, pixels),
      CdefFilterStrength::Secondary => cdef_filter_row::<false, true>(context, base, pixels),
    }?;
    let mut result = [0i16; 8];
    safe_simd::vst1q_s16(&mut result, output);
    Ok(result)
  }

  #[derive(Clone, Copy)]
  enum OracleSentinel {
    SignedMinimum,
    FormerPositive,
  }

  struct OracleDelta {
    difference: i32,
    threshold: i32,
    shift: u32,
  }

  fn reference_constrain(input: OracleDelta) -> Result<i32, Never> {
    let magnitude = input.difference.unsigned_abs();
    let limit = input
      .threshold
      .cast_unsigned()
      .saturating_sub(magnitude.wrapping_shr(input.shift));
    let limited = magnitude.min(limit).cast_signed();

    Ok(match input.difference.cmp(&0) {
      core::cmp::Ordering::Less => limited.wrapping_neg(),
      core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => limited,
    })
  }

  struct OracleGroup {
    distances: [i8; 2],
    weights: [i32; 2],
    threshold: i32,
    shift: u32,
  }

  fn oracle_group(
    strengths: CdefStrengths,
    direction: i32,
    channel: u8,
  ) -> Rav1dResult<OracleGroup> {
    let (offset, weight, threshold) = match channel {
      0 => (
        2_i32,
        4_i32.wrapping_sub(
          strengths
            .primary
            .wrapping_shr(strengths.depth_shift)
            .cast_signed()
            & 1,
        ),
        strengths.primary,
      ),
      1 => (0, 2, strengths.secondary),
      2 => (4, 2, strengths.secondary),
      _invalid => return Err(EINVAL),
    };
    let shift = threshold.checked_ilog2().map_or(0, |log| {
      strengths
        .damping
        .wrapping_sub(log.cast_signed())
        .max(0)
        .cast_unsigned()
    });
    let Ok(position) = index(direction.wrapping_add(offset).cast_unsigned());
    let &distances = dav1d_cdef_directions.get(position).ok_or(EINVAL)?;
    let weights = match channel {
      0 => [weight, weight & 3 | 2],
      _secondary => [2, 1],
    };

    Ok(OracleGroup {
      distances,
      weights,
      threshold: threshold.cast_signed(),
      shift,
    })
  }

  #[derive(Clone, Copy)]
  enum OracleClipping {
    None,
    NeighbourRange,
  }

  #[derive(Clone, Copy)]
  struct OraclePolicy {
    sentinel: OracleSentinel,
    clipping: OracleClipping,
  }

  fn oracle_sample(
    padding: &[i16; 144],
    center: i32,
    groups: &[OracleGroup; 3],
    policy: OraclePolicy,
  ) -> Rav1dResult<i32> {
    let Ok(position) = index(center.cast_unsigned());
    let &sample = padding.get(position).ok_or(EINVAL)?;
    let pixel = i32::from(sample);
    let mut sum = 0_i32;
    let mut minimum = pixel.cast_unsigned();
    let mut maximum = pixel;

    for group in groups.iter().filter(|group| group.threshold > 0) {
      for (distance, weight) in group.distances.into_iter().zip(group.weights) {
        let delta = i32::from(distance);

        for offset in [delta, delta.wrapping_neg()] {
          let Ok(tap_index) = index(center.wrapping_add(offset).cast_unsigned());
          let &raw = padding.get(tap_index).ok_or(EINVAL)?;
          let tap = match (raw, policy.sentinel) {
            (i16::MIN, OracleSentinel::FormerPositive) => 8191,
            (value, _mode) => i32::from(value),
          };
          let Ok(difference) = reference_constrain(OracleDelta {
            difference: tap.wrapping_sub(pixel),
            threshold: group.threshold,
            shift: group.shift,
          });
          sum = sum.wrapping_add(weight.wrapping_mul(difference));
          minimum = minimum.min(tap.cast_unsigned());
          maximum = maximum.max(tap);
        }
      }
    }

    let sign = i32::from(sum < 0);
    let output = pixel.wrapping_add(sum.wrapping_sub(sign).wrapping_add(8).wrapping_shr(4));

    Ok(match policy.clipping {
      OracleClipping::None => output,
      OracleClipping::NeighbourRange => output.max(minimum.cast_signed()).min(maximum),
    })
  }

  fn oracle_row(
    padding: &[i16; 144],
    input: TestCdefRow,
    width: u8,
    sentinel: OracleSentinel,
  ) -> Rav1dResult<[i32; 8]> {
    let primary = oracle_group(input.strengths, input.direction, 0)?;
    let left = oracle_group(input.strengths, input.direction, 1)?;
    let right = oracle_group(input.strengths, input.direction, 2)?;
    let groups = [primary, left, right];
    let first = cdef_row_base(input.coordinate)?;
    let mut row = [0_i32; 8];
    let clipping = match input.strengths.secondary {
      0 => OracleClipping::None,
      _nonzero => OracleClipping::NeighbourRange,
    };
    let policy = OraclePolicy { sentinel, clipping };

    for (x, output) in (0..width).zip(&mut row) {
      let center = first.wrapping_add(i32::from(x));
      let sample = oracle_sample(padding, center, &groups, policy)?;
      *output = sample;
    }

    Ok(row)
  }

  struct Rng(u64);

  enum RandomBit {
    Zero,
    One,
  }

  impl Rng {
    fn next(&mut self) -> Result<u32, Never> {
      self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
      wrapped(self.0.wrapping_shr(33))
    }

    fn below(&mut self, limit: u32) -> Result<u32, Never> {
      let Ok(value) = self.next();
      Ok(match value.checked_rem(limit) {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => 0,
      })
    }

    fn bit(&mut self) -> Result<RandomBit, Never> {
      let Ok(value) = self.next();
      Ok(match value & 1 {
        0 => RandomBit::Zero,
        _one => RandomBit::One,
      })
    }

    fn choose<T: Copy>(&mut self, values: &[T]) -> Rav1dResult<T> {
      let Ok(length) = wrapped::<_, u32>(values.len());
      let Ok(number) = self.below(length);
      let Ok(position) = index(number);
      values.get(position).copied().ok_or(EINVAL)
    }
  }

  struct ParameterSpace {
    primary_maximum: u32,
    secondary: [u32; 4],
    damping: [i32; 5],
  }

  fn param_space(depth_shift: u32) -> Result<ParameterSpace, Never> {
    let primary_maximum = 15_u32.wrapping_shl(depth_shift);
    let secondary = [0, 1, 2, 4].map(|strength: u32| strength.wrapping_shl(depth_shift));
    let damping =
      [2, 3, 4, 5, 6].map(|strength: i32| strength.wrapping_add(depth_shift.cast_signed()));

    Ok(ParameterSpace {
      primary_maximum,
      secondary,
      damping,
    })
  }

  fn assert_neutral_sentinel(space: &ParameterSpace, maximum: i32) -> Result<(), Never> {
    for &damping in &space.damping {
      for threshold in
        (1..=space.primary_maximum).chain(space.secondary.into_iter().filter(|&value| value != 0))
      {
        let log = match threshold.checked_ilog2() {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => 0,
        };
        let shift = damping
          .wrapping_sub(log.cast_signed())
          .max(0)
          .cast_unsigned();

        for pixel in [0, 1, 128, maximum.wrapping_div(2), maximum] {
          let Ok(scalar) = reference_constrain(OracleDelta {
            difference: i32::from(i16::MIN).wrapping_sub(pixel),
            threshold: threshold.cast_signed(),
            shift,
          });
          let absolute = 32768_u32.wrapping_sub(pixel.cast_unsigned());
          let vector = threshold.saturating_sub(absolute.wrapping_shr(shift.min(15)));
          assert_eq!(
            scalar, 0,
            "scalar sentinel threshold={threshold} damping={damping} pixel={pixel}"
          );
          assert_eq!(
            vector, 0,
            "vector sentinel threshold={threshold} damping={damping} pixel={pixel}"
          );
        }
      }
    }

    Ok(())
  }

  #[test]
  fn constrain_neutralises_sentinel_over_full_param_space() -> Rav1dResult {
    for shift in [0_u32, 2, 4] {
      let Ok(space) = param_space(shift);
      let maximum = 1_i32.wrapping_shl(shift.wrapping_add(8)).wrapping_sub(1);
      let Ok(()) = assert_neutral_sentinel(&space, maximum);
    }

    Ok(())
  }

  fn random_edges(random: &mut Rng) -> Result<CdefEdgeFlags, Never> {
    let mut edges = CdefEdgeFlags::empty();

    for edge in [
      CdefEdgeFlags::HAVE_TOP,
      CdefEdgeFlags::HAVE_BOTTOM,
      CdefEdgeFlags::HAVE_LEFT,
      CdefEdgeFlags::HAVE_RIGHT,
    ] {
      let Ok(bit) = random.bit();

      match bit {
        RandomBit::Zero => edges.insert(edge),
        RandomBit::One => {}
      }
    }

    Ok(edges)
  }

  fn test_padding(
    size: Size<u8>,
    edges: CdefEdgeFlags,
    random: &mut Rng,
    context: i32,
    mut generate: impl FnMut(&mut Rng, i32) -> Result<i32, Never>,
  ) -> Rav1dResult<[i16; 144]> {
    let mut pad = [[i16::MIN; 12]; 12];
    let start_y =
      2_u8.wrapping_sub(u8::from(edges.contains(CdefEdgeFlags::HAVE_TOP)).wrapping_mul(2));
    let start_x =
      2_u8.wrapping_sub(u8::from(edges.contains(CdefEdgeFlags::HAVE_LEFT)).wrapping_mul(2));
    let end_y = size
      .height
      .wrapping_add(2)
      .wrapping_add(u8::from(edges.contains(CdefEdgeFlags::HAVE_BOTTOM)).wrapping_mul(2));
    let end_x = size
      .width
      .wrapping_add(2)
      .wrapping_add(u8::from(edges.contains(CdefEdgeFlags::HAVE_RIGHT)).wrapping_mul(2));
    let Ok(top) = index(start_y);
    let Ok(bottom) = index(end_y);
    let Ok(left) = index(start_x);
    let Ok(right) = index(end_x);
    let body = pad.get_mut(top..bottom).ok_or(EINVAL)?;

    for row in body {
      let selected = row.get_mut(left..right).ok_or(EINVAL)?;

      for sample in selected {
        let Ok(pixel) = generate(random, context);
        let Ok(converted) = wrapped::<_, i16>(pixel);
        *sample = converted;
      }
    }

    Ok(zerocopy::transmute!(pad))
  }

  fn random_scenario(
    random: &mut Rng,
    space: &ParameterSpace,
    depth_shift: u32,
  ) -> Rav1dResult<Option<TestCdefRow>> {
    let Ok(primary) = random.below(space.primary_maximum.wrapping_add(1));
    let secondary = random.choose(&space.secondary)?;

    match (primary, secondary) {
      (0, 0) => return Ok(None),
      (_primary, _secondary) => {}
    }

    let damping = random.choose(&space.damping)?;
    let Ok(direction) = random.below(8);
    Ok(Some(TestCdefRow {
      coordinate: 0,
      direction: direction.cast_signed(),
      strengths: CdefStrengths {
        primary,
        secondary,
        damping,
        depth_shift,
      },
    }))
  }

  fn fixture_input(padding: &[i16; 144], row: TestCdefRow) -> Rav1dResult<[i32; 8]> {
    let start = cdef_row_base(row.coordinate)?;
    let Ok(position) = index(start.cast_unsigned());
    let samples = padding
      .get(position..)
      .and_then(<[_]>::first_chunk::<8>)
      .ok_or(EINVAL)?;
    Ok(samples.map(i32::from))
  }

  fn compare_filter_row(
    padding: &[i16; 144],
    row: TestCdefRow,
    width: u8,
  ) -> Rav1dResult<[i32; 8]> {
    let source = fixture_input(padding, row)?;
    let neon = neon_filter(padding, &source, row)?;
    let expected = oracle_row(padding, row, width, OracleSentinel::SignedMinimum)?;
    let Ok(columns) = index(width);
    let pair = neon.iter().zip(&expected);

    for (&got, &want) in pair.take(columns) {
      let actual = match row.strengths.depth_shift {
        0 => {
          let Ok(low) = wrapped::<_, u8>(got);
          i32::from(low)
        }
        _high_depth => i32::from(got.cast_unsigned()),
      };
      let reference = match row.strengths.depth_shift {
        0 => {
          let Ok(low) = wrapped::<_, u8>(want);
          i32::from(low)
        }
        _high_depth => {
          let Ok(word) = wrapped::<_, u16>(want);
          i32::from(word)
        }
      };
      assert_eq!(
        actual, reference,
        "filter row {} dir={}",
        row.coordinate, row.direction
      );
    }

    Ok(expected)
  }

  fn random_pixel(random: &mut Rng, maximum: i32) -> Result<i32, Never> {
    let Ok(pattern) = random.below(4);
    Ok(match pattern {
      0 => 0,
      1 => maximum,
      2 => {
        let Ok(offset) = random.below(3);
        maximum
          .wrapping_div(2)
          .wrapping_add(offset.cast_signed())
          .wrapping_sub(1)
      }
      _uniform => {
        let Ok(output) = random.below(maximum.cast_unsigned().wrapping_add(1));
        output.cast_signed()
      }
    })
  }

  struct DifferentialRows {
    depth_shift: u32,
    seed: u64,
    iterations: u32,
  }

  fn differential_rows(
    DifferentialRows {
      depth_shift,
      seed,
      iterations,
    }: DifferentialRows,
  ) -> Rav1dResult {
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let maximum = 1_i32
      .wrapping_shl(8_u32.wrapping_add(depth_shift))
      .wrapping_sub(1);
    let Ok(space) = param_space(depth_shift);
    let mut random = Rng(seed);
    let mut checked = 0_u32;

    for _case in 0..iterations {
      let Ok(narrow) = random.bit();
      let width = match narrow {
        RandomBit::Zero => 4,
        RandomBit::One => 8,
      };
      let height = match width {
        4 => {
          let Ok(short) = random.bit();

          match short {
            RandomBit::Zero => 4,
            RandomBit::One => 8,
          }
        }
        _large => 8,
      };
      let Ok(edges) = random_edges(&mut random);
      let padding = test_padding(
        Size { width, height },
        edges,
        &mut random,
        maximum,
        random_pixel,
      )?;
      let scenario = random_scenario(&mut random, &space, depth_shift)?;
      let mut row = match scenario {
        Some(row) => row,
        None => continue,
      };

      for coordinate in 0..height {
        row.coordinate = u64::from(coordinate);
        let _expected = compare_filter_row(&padding, row, width)?;
        checked = checked.wrapping_add(u32::from(width));
      }
    }

    assert!(checked > 1000, "compared only {checked} filter outputs");
    Ok(())
  }

  #[test]
  fn neon_row_matches_oracle_8bpc() -> Rav1dResult {
    differential_rows(DifferentialRows {
      depth_shift: 0,
      seed: 0x1234_5678_9abc_def0,
      iterations: 4000,
    })
  }

  #[test]
  fn neon_row_matches_oracle_10bpc() -> Rav1dResult {
    differential_rows(DifferentialRows {
      depth_shift: 2,
      seed: 0x0fed_cba9_8765_4321,
      iterations: 4000,
    })
  }

  #[test]
  fn neon_row_matches_oracle_12bpc() -> Rav1dResult {
    differential_rows(DifferentialRows {
      depth_shift: 4,
      seed: 0xdead_beef_cafe_f00d,
      iterations: 4000,
    })
  }

  struct FlatPixels {
    level: i32,
    spread: u32,
  }

  fn flat_padding(
    random: &mut Rng,
    edges: CdefEdgeFlags,
    params: FlatPixels,
  ) -> Rav1dResult<[i16; 144]> {
    let size = Size {
      width: 8,
      height: 8,
    };
    test_padding(size, edges, random, params.level, |random_2, level| {
      let Ok(value) = random_2.below(params.spread.wrapping_add(1));
      Ok(level.wrapping_add(value.cast_signed()))
    })
  }

  #[test]
  fn sentinel_must_not_raise_max() -> Rav1dResult {
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let Ok(space) = param_space(0);
    let mut random = Rng(0xa5a5_0f0f_1234_9999);
    let mut divergences = 0_u32;

    for _case in 0..200_000 {
      let Ok(center) = random.below(240);
      let Ok(range) = random.below(4);
      let Ok(edges) = random_edges(&mut random);

      match edges.is_all().then_some(()) {
        Some(()) => continue,
        None => {}
      }

      let padding = flat_padding(
        &mut random,
        edges,
        FlatPixels {
          level: 8_i32.wrapping_add(center.cast_signed()),
          spread: 1_u32.wrapping_add(range),
        },
      )?;
      let scenario = random_scenario(&mut random, &space, 0)?;
      let mut row = match scenario {
        Some(row) => row,
        None => continue,
      };

      for coordinate in 0..8 {
        row.coordinate = coordinate;
        let expected = compare_filter_row(&padding, row, 8)?;
        let old = oracle_row(&padding, row, 8, OracleSentinel::FormerPositive)?;

        for (previous, current) in old.into_iter().zip(expected) {
          divergences = divergences.wrapping_add(u32::from(previous != current));
        }
      }

      match (divergences > 32).then_some(()) {
        Some(()) => break,
        None => {}
      }
    }

    assert!(
      divergences > 0,
      "previous sentinel behavior must diverge on flat blocks"
    );
    Ok(())
  }

  fn compare_direction(token: Arm64, picture: [[i32; 8]; 8], ties: &mut u32) -> Rav1dResult {
    for depth_shift in [0_i32, 2, 4] {
      let rows = picture.map(|row| {
        row.map(|value| {
          let Ok(pixel) = wrapped::<_, u16>(value.wrapping_shl(depth_shift.cast_unsigned()));
          pixel
        })
      });
      let exact = dir_oracle(&rows, depth_shift)?;
      let mut variance = 0_u32;
      let direction = run_dir(token, &rows, depth_shift, &mut variance)?;
      assert_eq!(
        (direction, variance),
        exact,
        "projection scores with depth shift {depth_shift}"
      );
      *ties = ties.wrapping_add(u32::from(exact.0 == 0));
    }

    Ok(())
  }

  #[test]
  fn neon_dir_matches_scalar() -> Rav1dResult {
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let token = Arm64::summon().ok_or(EINVAL)?;
    let mut random = Rng(0xfeed_face_0000_0001);
    let mut ties = 0_u32;

    for level in [0, 1, 128, 254, 255] {
      compare_direction(token, [[level; 8]; 8], &mut ties)?;
    }

    for edge in 0..4 {
      let mut values = [[0_i32; 8]; 8];

      for (y, row) in (0..8_u8).zip(&mut values) {
        for (x, value) in (0..8_u8).zip(row) {
          let high = match edge {
            0 => y >= 4,
            1 => x >= 4,
            2 => x >= y,
            _reverse => x.wrapping_add(y) >= 7,
          };
          *value = i32::from(high).wrapping_mul(255);
        }
      }

      compare_direction(token, values, &mut ties)?;
    }

    for shift in [0_u32, 1, 2] {
      let mut values = [[0_i32; 8]; 8];

      for (y, row) in (0..8_u8).zip(&mut values) {
        for (x, value) in (0..8_u8).zip(row) {
          let parity = x.wrapping_shr(shift).wrapping_add(y.wrapping_shr(shift)) & 1;
          *value = 128_i32.wrapping_add(i32::from(parity));
        }
      }

      compare_direction(token, values, &mut ties)?;
    }

    for _case in 0..3000 {
      let mut values = [[0_i32; 8]; 8];
      let Ok(strength) = random.below(255);
      let amplitude = 1_u32.wrapping_add(strength);
      let Ok(dc) = random.below(256_u32.wrapping_sub(amplitude));

      for pixel in values.iter_mut().flatten() {
        let Ok(value) = random.below(amplitude.wrapping_add(1));
        *pixel = dc.wrapping_add(value).cast_signed();
      }

      compare_direction(token, values, &mut ties)?;
    }

    assert!(ties > 0, "no tie-break case exercised");
    Ok(())
  }

  #[arcane]
  fn run_dir(
    _token: Arm64,
    rows: &[[u16; 8]; 8],
    bitdepth_min_8: i32,
    variance: &mut c_uint,
  ) -> Rav1dResult<i32> {
    let c128 = vdupq_n_s16(128);
    let neg_shift = vdupq_n_s16(::core::primitive::i16::wrapping_neg(
      match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(bitdepth_min_8)
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      },
    ));
    let mut v = [vdupq_n_s16(0); 8];

    for (rows_element, v_element) in <[_]>::iter(rows).zip(<[_]>::iter_mut(&mut v)) {
      let px = safe_simd::vld1q_u16(&*rows_element);
      *v_element = vsubq_s16(vreinterpretq_s16_u16(vshlq_u16(px, neg_shift)), c128);
    }

    cdef_dir_core(&v, variance)
  }

  fn oracle_add_projection(lines: &mut [i32], bin: u8, pixel: i32) -> Rav1dResult {
    let Ok(position) = index(bin);
    let sum = lines.get_mut(position).ok_or(EINVAL)?;
    *sum = sum.wrapping_add(pixel);

    Ok(())
  }

  fn oracle_sum_squares(lines: &[i32]) -> Result<u32, Never> {
    Ok(lines.iter().fold(0_u32, |sum, &line| {
      sum.wrapping_add(line.wrapping_mul(line).cast_unsigned())
    }))
  }

  fn oracle_edge_cost(lines: &[i32], weights: &[u32]) -> Result<u32, Never> {
    let pairs = lines.iter().zip(lines.iter().rev());

    Ok(
      pairs
        .zip(weights)
        .fold(0_u32, |sum, ((&first, &last), &weight)| {
          let first_square = first.wrapping_mul(first).cast_unsigned();
          let last_square = last.wrapping_mul(last).cast_unsigned();
          sum.wrapping_add(first_square.wrapping_add(last_square).wrapping_mul(weight))
        }),
    )
  }

  fn dir_oracle(rows: &[[u16; 8]; 8], bitdepth_min_8: i32) -> Rav1dResult<(i32, u32)> {
    let mut axes = [[0_i32; 8]; 2];
    let mut diagonals = [[0_i32; 15]; 2];
    let mut half_lines = [[0_i32; 11]; 4];
    let [horizontal_sum, vertical_sum] = &mut axes;
    let [down_sum, up_sum] = &mut diagonals;
    let [
      shallow_down_sum,
      shallow_up_sum,
      steep_down_sum,
      steep_up_sum,
    ] = &mut half_lines;

    for (y, row) in (0..8_u8).zip(rows) {
      for (x, &sample) in (0..8_u8).zip(row) {
        let pixel = i32::from(sample)
          .wrapping_shr(bitdepth_min_8.cast_unsigned())
          .wrapping_sub(128);
        let half_x = x.wrapping_shr(1);
        let half_y = y.wrapping_shr(1);
        oracle_add_projection(horizontal_sum, y, pixel)?;
        oracle_add_projection(vertical_sum, x, pixel)?;
        oracle_add_projection(down_sum, y.wrapping_add(x), pixel)?;
        oracle_add_projection(up_sum, 7_u8.wrapping_add(y).wrapping_sub(x), pixel)?;
        oracle_add_projection(shallow_down_sum, y.wrapping_add(half_x), pixel)?;
        oracle_add_projection(
          shallow_up_sum,
          3_u8.wrapping_add(y).wrapping_sub(half_x),
          pixel,
        )?;
        oracle_add_projection(
          steep_down_sum,
          3_u8.wrapping_sub(half_y).wrapping_add(x),
          pixel,
        )?;
        oracle_add_projection(steep_up_sum, half_y.wrapping_add(x), pixel)?;
      }
    }

    let [horizontal, vertical] = axes;
    let [down, up] = diagonals;
    let [shallow_down, shallow_up, steep_down, steep_up] = half_lines;
    let Ok(cost_horizontal) = oracle_sum_squares(&horizontal);
    let Ok(cost_vertical) = oracle_sum_squares(&vertical);
    let mut costs = [
      0,
      0,
      cost_horizontal.wrapping_mul(105),
      0,
      0,
      0,
      cost_vertical.wrapping_mul(105),
      0,
    ];
    let [
      cost_down,
      cost_shallow_down,
      _,
      cost_shallow_up,
      cost_up,
      cost_steep_down,
      _,
      cost_steep_up,
    ] = &mut costs;

    for (lines, cost) in [(down, cost_down), (up, cost_up)] {
      let [_, _, _, _, _, _, _, center, ..] = lines;
      let Ok(edge) = oracle_edge_cost(&lines, &[840, 420, 280, 210, 168, 140, 120]);
      *cost = edge.wrapping_add(
        center
          .wrapping_mul(center)
          .cast_unsigned()
          .wrapping_mul(105),
      );
    }

    for (lines, cost) in [
      (shallow_down, cost_shallow_down),
      (shallow_up, cost_shallow_up),
      (steep_down, cost_steep_down),
      (steep_up, cost_steep_up),
    ] {
      let middle = lines.get(3..8).ok_or(EINVAL)?;
      let Ok(interior) = oracle_sum_squares(middle);
      let Ok(edge) = oracle_edge_cost(&lines, &[420, 210, 140]);
      *cost = edge.wrapping_add(interior.wrapping_mul(105));
    }

    let [first, ..] = costs;
    let mut best = (0, first);

    for (direction, cost) in (0..8_u8).zip(costs) {
      match (cost > best.1).then_some(()) {
        Some(()) => best = (direction, cost),
        None => {}
      }
    }

    let Ok(opposite) = index(best.0 ^ 4);
    let &reference = costs.get(opposite).ok_or(EINVAL)?;
    let difference = best.1.wrapping_sub(reference).wrapping_shr(10);

    Ok((i32::from(best.0), difference))
  }
}
