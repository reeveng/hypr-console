//! Loop restoration on aarch64: a safe NEON tier over `src/looprestoration.rs`.
//!
//! # History — what used to be here, and why measuring mattered
//!
//! Until 2026-08-07 this file held 1,531 lines that opened `//! Safe ARM NEON
//! implementations for Loop Restoration`, imported `core::arch::aarch64::*`,
//! and contained **zero aarch64 intrinsic calls**: a hand-written scalar
//! re-implementation of the reference, dispatched unconditionally. It was
//! *slower* than the code it shadowed (8bpc `00001147`: 204.42 vs 192.83
//! ms/frame whole-decode, ratio 1.060) and was deleted rather than ported on
//! faith. That measurement, and the corpus-wide activity scan that made it
//! possible, are recorded in
//! `benchmarks/lr_arm_vs_reference_2026-08-07.meta`.
//!
//! Two facts from that record shape this file:
//!
//! * **The 4K gap vectors do no loop restoration at all.** `v4k_8tile{,_10b}`
//!   measure LR at 0.0 ms because LR is switched off in those bitstreams, so
//!   nothing in the gap-to-dav1d table can see this code. LR is active in
//!   **696 of 768** dav1d-test-data vectors; `md5_inventory --activity`
//!   (needs `--features __ablate`) is the instrument that says which.
//! * The profile blamed `selfguided_filter` (9.5% of decode self time on
//!   `10-bit/issues/318_tx_4x4`) and the Wiener filter (2.0%), plus a large
//!   `_platform_memset` share from the per-unit scratch.
//!
//! # What this tier does
//!
//! Every variant the dispatcher can be handed is covered at both bit depths:
//! Wiener (the 7-tap window, which also expresses the 5-tap case with zero
//! outer coefficients) and self-guided 5x5 / 3x3 / mix.
//!
//! The three kernels, in decreasing order of measured cost:
//!
//! 1. **`boxsum{3,5}`, fused.** The reference walks each column top-to-bottom
//!    for the vertical sum (a 390-element stride per step) and then slides
//!    horizontally in place. This tier computes ONE row of vertical sums into
//!    a small row buffer and immediately slides it into the destination row.
//!    That turns a column-major pass over a 106 KB array into a row-major
//!    stream over three (or five) source rows, and — because the horizontal
//!    input is a separate buffer — removes the read-after-overwrite aliasing
//!    that forces the reference's scalar carry chain.
//! 2. **The `a`/`b` (`sgr_x_by_x`) loop.** Vectorised 16 columns at a time.
//!    The 256-entry `dav1d_sgr_x_by_x` lookup is done with four `vqtbl4q_u8`
//!    over the table's four 64-byte quarters: `vqtbl4q_u8` returns 0 for any
//!    index above 63, and the index is offset by a wrapping `vsubq_u8` per
//!    quarter, so exactly one quarter contributes per lane and the four
//!    results OR together. `z` is clamped to 255 before the narrow, so no
//!    index can alias into another quarter.
//! 3. **The six/eight-neighbour output pass**, and the final `w0/w1` blend
//!    into the picture.
//!
//! # Exactness
//!
//! The reference is the oracle, and the arithmetic here is written to match it
//! bit for bit, including where it relies on wrapping:
//!
//! * `x * b * sgr_one_by_x` is `c_uint` (u32) arithmetic in the reference, and
//!   the wrapping forms here (`vmulq_u32`, `wrapping_mul`) exist so the two
//!   agree bit for bit whatever the input. An earlier note here claimed it
//!   "genuinely overflows at 12bpc (255 * 102375 * 455 = 1.19e10)"; that mixed
//!   the `n == 25` sum bound with the `n == 9` constant, which cannot co-occur.
//!   Recomputed 2026-08-31, the true worst cases both FIT in u32, by under 0.5%:
//!   `n == 25`: 255 * (25 * 4095) * 164 = 4,281,322,500; `n == 9`:
//!   255 * (9 * 4095) * 455 = 4,276,101,375; `u32::MAX` = 4,294,967,295. Keep
//!   the wrapping form anyway — it is what the reference's `c_uint` does — and
//!   do not "fix" it to u64, which would silently diverge from the reference if
//!   an input ever did leave that range.
//! * The variance calculation `a * n - b * b` uses the same wrapping
//!   operations in the scalar and vector paths. Each coefficient row is checked
//!   against the active bit depth and window size before calculating variance.
//! * `dst[i] = (...).as_()` is a truncating cast, so the narrow is
//!   `vmovn_s32`, never a saturating `vqmovn_s32`.
//! * The 8bpc horizontal Wiener pass folds the separate `tmp[i+3] * 128` term
//!   into tap 3. `lr_apply.rs` builds `filter[0][3] = -2*(f0+f1+f2)` and only
//!   adds 128 itself for non-8bpc; the AV1 tap ranges bound the folded value
//!   to [0, 218], and this pass accumulates in i32, so the fold cannot
//!   overflow. (The comment in `lr_apply.rs` about handling +128 separately is
//!   about i16 accumulators in hand-written asm, which this is not.)
//!
//! Two gates enforce that: `--features __simd_test` re-runs the scalar
//! reference after every call and compares the `w`x`h` output region, and
//! `examples/md5_inventory` set-diffs all 768 corpus vectors BY NAME with the
//! actual MD5 in the key.
//!
//! # Guard discipline
//!
//! Reads of the picture happen only inside `looprestoration::padding`, which
//! already takes one guard per source row. The write side here is likewise one
//! guard per destination row, exactly `w` pixels wide — never a single wide
//! guard spanning `(h - 1) * stride + w`, which would overlap rows a
//! concurrent tile worker owns. Padded input, intermediate moments and filter
//! deltas live in a separate scratch allocation cached on each worker thread.

#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

#[cfg(target_arch = "aarch64")]
use archmage::intrinsics::aarch64 as safe_simd;
#[cfg(target_arch = "aarch64")]
use archmage::{Arm64, arcane, rite};

use std::cell::RefCell;
use std::thread::LocalKey;
use core::ops::Range;

use crate::include::common::bitdepth::{AsPrimitive, BitDepth};
use crate::include::dav1d::picture::PicOffset;
use crate::src::looprestoration::{RESTORATION_INTERIOR, RestorationDeltas, RestorationMoments, RestorationPixels, RestorationRow, restoration_rows, restoration_rows_mut, BoxRadius, LooprestorationParams, RestorationDispatch, RestorationKernel, RestorationPadding, RestorationRequest, SGR_STRIDE, SgrSelection, SgrStencil};
use crate::src::error::{Rav1dError::{EINVAL, ENOMEM}, Rav1dResult};
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};

#[cfg(target_arch = "aarch64")]
use crate::include::common::bitdepth::BitDepth8;
#[cfg(target_arch = "aarch64")]
use crate::include::common::bitdepth::BitDepth16;
#[cfg(target_arch = "aarch64")]
use crate::include::common::intops::iclip;
#[cfg(target_arch = "aarch64")]
use crate::src::looprestoration::padding;
#[cfg(target_arch = "aarch64")]
use crate::src::strided::Strided as _;
#[cfg(target_arch = "aarch64")]
use crate::src::tables::dav1d_sgr_x_by_x;

#[cfg(target_arch = "aarch64")]
fn boxed_zeroed<T: Copy + Default, const N: usize>() -> Rav1dResult<Box<[T; N]>> {
  let mut samples = Vec::new();
  samples.try_reserve_exact(N).map_err(|_allocation| ENOMEM)?;
  samples.resize(N, T::default());
  samples.into_boxed_slice().try_into().map_err(|_extent| EINVAL)
}

#[cfg(target_arch = "aarch64")]
struct RestorationScratch<Pixel, Coefficient> {
  tmp: Box<RestorationPixels<Pixel>>,
  hor: Box<RestorationPixels<u16>>,
  sumsq: Box<RestorationMoments<i32>>,
  sum: Box<RestorationMoments<Coefficient>>,
  d0: Box<RestorationDeltas<Coefficient>>,
  d1: Box<RestorationDeltas<Coefficient>>,
}

#[cfg(target_arch = "aarch64")]
type Scratch8 = RestorationScratch<u8, i16>;

#[cfg(target_arch = "aarch64")]
type Scratch16 = RestorationScratch<u16, i32>;

#[cfg(target_arch = "aarch64")]
impl<Pixel: Copy + Default, Coefficient: Copy + Default>
  RestorationScratch<Pixel, Coefficient>
{
  fn new() -> Rav1dResult<Self> {
    let tmp = boxed_zeroed()?;
    let hor = boxed_zeroed()?;
    let sumsq = boxed_zeroed()?;
    let sum = boxed_zeroed()?;
    let d0 = boxed_zeroed()?;
    let d1 = boxed_zeroed()?;

    Ok(Self {
      tmp,
      hor,
      sumsq,
      sum,
      d0,
      d1,
    })
  }
}

#[cfg(target_arch = "aarch64")]
thread_local! {
    static SCRATCH8: RefCell<Option<Scratch8>> = const { RefCell::new(None) };
    static SCRATCH16: RefCell<Option<Scratch16>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "aarch64")]
fn with_scratch<S: 'static, R>(
  cache: &'static LocalKey<RefCell<Option<S>>>,
  allocate: impl FnOnce() -> Rav1dResult<S>,
  filter: impl FnOnce(&mut S) -> Rav1dResult<R>,
) -> Rav1dResult<R> {
  cache.try_with(|entry| {
    let mut cached = entry.try_borrow_mut().map_err(|_occupied| EINVAL)?;
    let scratch = match &mut *cached {
      Some(scratch) => scratch,
      None => {
        let new = allocate()?;
        cached.insert(new)
      }
    };
    filter(scratch)
  }).map_err(|_thread_local| EINVAL)?
}

struct WienerPass {
  bias: i32,
  shift: u32,
  maximum: i32,
  taps: [i16; 7],
}

struct WienerPasses {
  horizontal: WienerPass,
  vertical: WienerPass,
}

fn wiener_passes(params: &LooprestorationParams, depth: u32, maximum: i32) -> Result<WienerPasses, Never> {
  let [horizontal, vertical] = params.filter;
  let [first, second, third, center, fifth, sixth, seventh, _] = horizontal;
  let eight_bit = i16::from(depth == 8).wrapping_mul(128);
  let horizontal_taps = [first, second, third, center.wrapping_add(eight_bit), fifth, sixth, seventh];
  let [a, b, c, d, e, f, g, _] = vertical;
  let extra = u32::from(depth == 12).wrapping_mul(2);
  let horizontal_shift = 3_u32.wrapping_add(extra);
  let vertical_shift = 11_u32.wrapping_sub(extra);
  let vertical_rounding = vertical_shift.wrapping_sub(1);

  Ok(WienerPasses {
    horizontal: WienerPass {
      bias: 1_i32.wrapping_shl(depth.wrapping_add(6)).wrapping_add(1_i32.wrapping_shl(horizontal_shift.wrapping_sub(1))),
      shift: horizontal_shift,
      maximum: 1_i32.wrapping_shl(depth.wrapping_add(8).wrapping_sub(horizontal_shift)).wrapping_sub(1),
      taps: horizontal_taps,
    },
    vertical: WienerPass {
      bias: 1_i32.wrapping_shl(depth.wrapping_add(vertical_rounding)).wrapping_neg().wrapping_add(1_i32.wrapping_shl(vertical_rounding)),
      shift: vertical_shift,
      maximum,
      taps: [a, b, c, d, e, f, g],
    },
  })
}

#[rite]
fn apply_wiener_taps(
  _token: Arm64,
  input: impl Iterator<Item = Rav1dResult<int16x8_t>>,
  pass: &WienerPass,
) -> Rav1dResult<uint16x8_t> {
  let mut low = vdupq_n_s32(pass.bias);
  let mut high = low;

  for (&weight, input_lanes) in pass.taps.iter().zip(input) {
    let pixels = input_lanes?;
    let factors = vdupq_n_s16(weight);
    low = vmlal_s16(low, vget_low_s16(pixels), vget_low_s16(factors));
    high = vmlal_high_s16(high, pixels, factors);
  }

  let shift = vdupq_n_s32(pass.shift.cast_signed().wrapping_neg());
  let maximum = vdupq_n_s32(pass.maximum);
  let zero = vdupq_n_s32(0);
  let low_clipped = vminq_s32(vmaxq_s32(vshlq_s32(low, shift), zero), maximum);
  let high_clipped = vminq_s32(vmaxq_s32(vshlq_s32(high, shift), zero), maximum);
  let low_packed = vmovn_u32(vreinterpretq_u32_s32(low_clipped));
  let high_packed = vmovn_u32(vreinterpretq_u32_s32(high_clipped));
  Ok(vcombine_u16(low_packed, high_packed))
}

fn arm_wiener_sample(input: [i32; 7], pass: &WienerPass) -> Result<i32, Never> {
  let sum = input.into_iter().zip(pass.taps).fold(pass.bias, |sum, (pixel, weight)| {
    sum.wrapping_add(pixel.wrapping_mul(i32::from(weight)))
  });
  Ok(iclip(sum.wrapping_shr(pass.shift), 0, pass.maximum))
}

struct WienerHorizontal<'a, BD: BitDepth> {
  source: &'a RestorationPixels<BD::Pixel>,
  output: &'a mut RestorationPixels<u16>,
  extent: Size<u64>,
  pass: &'a WienerPass,
}

#[arcane]
fn wiener_horizontal<BD: BitDepth>(token: Arm64, request: WienerHorizontal<'_, BD>) -> Rav1dResult {
  let Ok(source) = restoration_rows(request.source);
  let Ok(output) = restoration_rows_mut(request.output);
  let Ok(width) = index(request.extent.width);
  let Ok(rows) = index(request.extent.height.wrapping_add(6));
  let source_rows = source.get(..rows).ok_or(EINVAL)?;
  let target_rows = output.get_mut(..rows).ok_or(EINVAL)?;

  for (samples, target_row) in source_rows.iter().zip(target_rows) {
    let selected = target_row.get_mut(..width).ok_or(EINVAL)?;
    let (vectors, tail) = selected.as_chunks_mut::<8>();

    for (number, block) in (0_u64..).zip(vectors) {
      let first = number.wrapping_mul(8);
      let taps = (0..7_u64).map(|offset| {
        let window = moment_window(samples, first.wrapping_add(offset))?;
        load_pixel_lanes::<BD>(token, window)
      });
      let filtered = apply_wiener_taps(token, taps, request.pass)?;
      safe_simd::vst1q_u16(block, filtered);
    }

    let Ok(tail_len) = fixed_index(tail.len());
    let Ok(start) = index(request.extent.width.wrapping_sub(tail_len));
    let remaining = samples.get(start..).ok_or(EINVAL)?;

    for (destination, window) in tail.iter_mut().zip(remaining.array_windows::<7>()) {
      let Ok(rounded) = arm_wiener_sample(window.map(|pixel| pixel.as_::<i32>()), request.pass);
      let Ok(value) = console_core_number_conversion::wrapped::<_, u16>(rounded);
      *destination = value;
    }
  }

  Ok(())
}

#[rite]
fn load_pixel_lanes<BD: BitDepth>(
  _token: Arm64,
  source: &[BD::Pixel; 8],
) -> Rav1dResult<int16x8_t> {
  use crate::include::common::bitdepth::BPC;
  use zerocopy::{FromBytes, IntoBytes};

  let widened = match BD::BPC {
    BPC::BPC8 => {
      let pixels = FromBytes::ref_from_bytes(source.as_bytes()).map_err(|_shape| EINVAL)?;
      vmovl_u8(safe_simd::vld1_u8(pixels))
    }
    BPC::BPC16 => {
      let pixels = FromBytes::ref_from_bytes(source.as_bytes()).map_err(|_shape| EINVAL)?;
      safe_simd::vld1q_u16(pixels)
    }
  };

  Ok(vreinterpretq_s16_u16(widened))
}

struct WienerOutput<'a> {
  source: &'a RestorationPixels<u16>,
  destination: PicOffset<'a>,
  extent: Size<u64>,
  pass: &'a WienerPass,
}

#[rite]
fn store_pixel_lanes<BD: BitDepth>(
  _token: Arm64,
  target: &mut [BD::Pixel; 8],
  values: uint16x8_t,
) -> Rav1dResult {
  use crate::include::common::bitdepth::BPC;
  use zerocopy::{FromBytes, IntoBytes};

  match BD::BPC {
    BPC::BPC8 => {
      let output = FromBytes::mut_from_bytes(target.as_mut_bytes()).map_err(|_shape| EINVAL)?;
      safe_simd::vst1_u8(output, vmovn_u16(values));
    }
    BPC::BPC16 => {
      let output = FromBytes::mut_from_bytes(target.as_mut_bytes()).map_err(|_shape| EINVAL)?;
      safe_simd::vst1q_u16(output, values);
    }
  }

  Ok(())
}

#[arcane]
fn wiener_vertical<BD: BitDepth>(token: Arm64, request: WienerOutput<'_>) -> Rav1dResult {
  let Ok(source) = restoration_rows(request.source);
  let Ok(pitch) = request.destination.fixed_pixel_stride::<BD>();
  let Ok(width) = index(request.extent.width);

  for y in 0..request.extent.height {
    let displacement = y.cast_signed().wrapping_mul(pitch);
    let Ok(position) = console_core_number_conversion::wrapped_offset(displacement);
    let mut output = (request.destination + position).slice_mut::<BD>(width);
    let (vectors, tail) = output.as_chunks_mut::<8>();
    let neighbors = moment_window::<_, 7>(source, y)?;

    for (number, target) in (0_u64..).zip(vectors) {
      let column = number.wrapping_mul(8);
      let taps = neighbors.iter().map(|samples| {
        let input = moment_window(samples, column)?;
        Ok(vreinterpretq_s16_u16(safe_simd::vld1q_u16(input)))
      });
      let result = apply_wiener_taps(token, taps, request.pass)?;
      store_pixel_lanes::<BD>(token, target, result)?;
    }

    let Ok(tail_len) = fixed_index(tail.len());
    let first = request.extent.width.wrapping_sub(tail_len);

    for (column, target) in (first..request.extent.width).zip(tail) {
      let Ok(column_index) = index(column);
      let mut taps = [0_i32; 7];

      for (sample, source_row) in taps.iter_mut().zip(neighbors) {
        let &pixel = source_row.get(column_index).ok_or(EINVAL)?;
        *sample = i32::from(pixel);
      }

      let Ok(rounded) = arm_wiener_sample(taps, request.pass);
      *target = rounded.as_::<BD::Pixel>();
    }
  }

  Ok(())
}

fn wiener_8bpc(token: Arm64, request: RestorationPadding<'_, BitDepth8>, params: &LooprestorationParams) -> Rav1dResult {
  with_scratch(&SCRATCH8, Scratch8::new, |scratch| {
    let extent = request.size;
    let destination = request.picture;
    let Ok(passes) = wiener_passes(params, 8, 255);
    padding::<BitDepth8>(scratch.tmp.as_mut(), request)?;
    wiener_horizontal::<BitDepth8>(token, WienerHorizontal {
      source: &scratch.tmp, output: &mut scratch.hor, extent, pass: &passes.horizontal,
    })?;
    wiener_vertical::<BitDepth8>(token, WienerOutput {
      source: &scratch.hor, destination, extent, pass: &passes.vertical,
    })
  })
}

fn wiener_16bpc(
  token: Arm64,
  request: RestorationPadding<'_, BitDepth16>,
  params: &LooprestorationParams,
  maximum: i32,
) -> Rav1dResult {
  with_scratch(&SCRATCH16, Scratch16::new, |scratch| {
    let extent = request.size;
    let destination = request.picture;
    let depth = match maximum {
      1023 => 10,
      _high_depth => 12,
    };
    let Ok(passes) = wiener_passes(params, depth, maximum);
    padding::<BitDepth16>(scratch.tmp.as_mut(), request)?;
    wiener_horizontal::<BitDepth16>(token, WienerHorizontal {
      source: &scratch.tmp, output: &mut scratch.hor, extent, pass: &passes.horizontal,
    })?;
    wiener_vertical::<BitDepth16>(token, WienerOutput {
      source: &scratch.hor, destination, extent, pass: &passes.vertical,
    })
  })
}

struct BoxMoments<'a, Sum> {
  sums: &'a mut [Sum],
  squares: &'a mut [u32],
}

struct BoxMomentsOutput<'a, Sum> {
  sums: &'a mut [Sum],
  squares: &'a mut [i32],
}

fn moment_window<T, const N: usize>(values: &[T], first: u64) -> Rav1dResult<&[T; N]> {
  let Ok(start) = index(first);
  values.get(start..).and_then(<[_]>::first_chunk).ok_or(EINVAL)
}

fn moment_window_mut<T, const N: usize>(values: &mut [T], first: u64) -> Rav1dResult<&mut [T; N]> {
  let Ok(start) = index(first);
  values.get_mut(start..).and_then(<[_]>::first_chunk_mut).ok_or(EINVAL)
}

fn vertical_moments_tail<P, Sum, const N: usize>(
  source: &[RestorationRow<P>; N],
  scratch: &mut BoxMoments<'_, Sum>,
  columns: Range<u64>,
) -> Rav1dResult
where
  P: Copy,
  u32: From<P>,
  Sum: TryFrom<u32>,
{
  for x in columns {
    let Ok(position) = index(x);
    let mut sum = 0_u32;
    let mut squared = 0_u32;

    for row in source {
      let &sample = row.get(position).ok_or(EINVAL)?;
      let value = u32::from(sample);
      sum = sum.wrapping_add(value);
      squared = squared.wrapping_add(value.wrapping_mul(value));
    }

    let output_sum = scratch.sums.get_mut(position).ok_or(EINVAL)?;
    let output_squared = scratch.squares.get_mut(position).ok_or(EINVAL)?;
    let converted_sum = Sum::try_from(sum).map_err(|_overflow| EINVAL)?;
    *output_sum = converted_sum;
    *output_squared = squared;
  }

  Ok(())
}

fn horizontal_moments_tail<Sum, Out, const N: usize>(
  columns: Range<u64>,
  moments: &BoxMoments<'_, Sum>,
  output: &mut BoxMomentsOutput<'_, Out>,
) -> Rav1dResult
where
  Sum: Copy,
  u32: From<Sum>,
  Out: TryFrom<u32>,
{
  let Ok(taps) = fixed_index(N);

  for x in columns {
    let start = x.wrapping_sub(taps.wrapping_div(2));
    let values = moment_window::<_, N>(moments.sums, start)?;
    let squares = moment_window::<_, N>(moments.squares, start)?;
    let mut sum = 0_u32;
    let mut squared = 0_u32;

    for (&pixel, &square) in values.iter().zip(squares) {
      sum = sum.wrapping_add(u32::from(pixel));
      squared = squared.wrapping_add(square);
    }

    let Ok(position) = index(x);
    let target_sum = output.sums.get_mut(position).ok_or(EINVAL)?;
    let target_square = output.squares.get_mut(position).ok_or(EINVAL)?;
    let converted_sum = Out::try_from(sum).map_err(|_overflow| EINVAL)?;
    *target_sum = converted_sum;
    *target_square = squared.cast_signed();
  }

  Ok(())
}

#[rite]
fn box_vertical_8<const N: usize>(
  _token: Arm64,
  source: &[RestorationRow<u8>; N],
  scratch: &mut BoxMoments<'_, u16>,
  width: u64,
) -> Rav1dResult {
  let blocks = width.wrapping_div(8);

  for block in 0..blocks {
    let x = block.wrapping_mul(8);
    let mut value = vdupq_n_u16(0);
    let mut low = vdupq_n_u32(0);
    let mut high = vdupq_n_u32(0);

    for row in source {
      let samples = moment_window(row, x)?;
      let bytes = safe_simd::vld1_u8(samples);
      let lanes = vmovl_u8(bytes);
      value = vaddq_u16(value, lanes);
      let first = vget_low_u16(lanes);
      low = vmlal_u16(low, first, first);
      high = vmlal_high_u16(high, lanes, lanes);
    }

    let sum = moment_window_mut(scratch.sums, x)?;
    let squared = moment_window_mut::<_, 8>(scratch.squares, x)?;
    let [first, second]: &mut [[u32; 4]; 2] = zerocopy::transmute_mut!(squared);
    safe_simd::vst1q_u16(sum, value);
    safe_simd::vst1q_u32(first, low);
    safe_simd::vst1q_u32(second, high);
  }

  vertical_moments_tail(source, scratch, blocks.wrapping_mul(8)..width)
}

#[rite]
fn box_vertical_16<const N: usize>(
  _token: Arm64,
  source: &[RestorationRow<u16>; N],
  scratch: &mut BoxMoments<'_, u32>,
  width: u64,
) -> Rav1dResult {
  let blocks = width.wrapping_div(8);

  for block in 0..blocks {
    let x = block.wrapping_mul(8);
    let mut sum_low = vdupq_n_u32(0);
    let mut sum_high = vdupq_n_u32(0);
    let mut square_low = vdupq_n_u32(0);
    let mut square_high = vdupq_n_u32(0);

    for row in source {
      let samples = moment_window(row, x)?;
      let lanes = safe_simd::vld1q_u16(samples);
      let first = vget_low_u16(lanes);
      sum_low = vaddw_u16(sum_low, first);
      sum_high = vaddw_high_u16(sum_high, lanes);
      square_low = vmlal_u16(square_low, first, first);
      square_high = vmlal_high_u16(square_high, lanes, lanes);
    }

    let sum = moment_window_mut::<_, 8>(scratch.sums, x)?;
    let squared = moment_window_mut::<_, 8>(scratch.squares, x)?;
    let [sum_left, sum_right]: &mut [[u32; 4]; 2] = zerocopy::transmute_mut!(sum);
    let [square_left, square_right]: &mut [[u32; 4]; 2] = zerocopy::transmute_mut!(squared);
    safe_simd::vst1q_u32(sum_left, sum_low);
    safe_simd::vst1q_u32(sum_right, sum_high);
    safe_simd::vst1q_u32(square_left, square_low);
    safe_simd::vst1q_u32(square_right, square_high);
  }

  vertical_moments_tail(source, scratch, blocks.wrapping_mul(8)..width)
}

#[rite]
fn box_horizontal_8<const N: usize>(
  _token: Arm64,
  scratch: &BoxMoments<'_, u16>,
  output: &mut BoxMomentsOutput<'_, i16>,
  width: u64,
) -> Rav1dResult {
  let Ok(taps) = fixed_index(N);
  let half = taps.wrapping_div(2);
  let limit = width.wrapping_sub(2);
  let chunks = limit.wrapping_sub(2).wrapping_div(8);

  for chunk in 0..chunks {
    let x = 2_u64.wrapping_add(chunk.wrapping_mul(8));
    let mut value = vdupq_n_u16(0);
    let mut low = vdupq_n_u32(0);
    let mut high = vdupq_n_u32(0);

    for tap in 0..taps {
      let first = x.wrapping_add(tap).wrapping_sub(half);
      let sums = moment_window(scratch.sums, first)?;
      let squares = moment_window::<_, 8>(scratch.squares, first)?;
      let [left, right]: &[[u32; 4]; 2] = zerocopy::transmute_ref!(squares);
      value = vaddq_u16(value, safe_simd::vld1q_u16(sums));
      low = vaddq_u32(low, safe_simd::vld1q_u32(left));
      high = vaddq_u32(high, safe_simd::vld1q_u32(right));
    }

    let sum = moment_window_mut(output.sums, x)?;
    let squares = moment_window_mut::<_, 8>(output.squares, x)?;
    let [left, right]: &mut [[i32; 4]; 2] = zerocopy::transmute_mut!(squares);
    safe_simd::vst1q_s16(sum, vreinterpretq_s16_u16(value));
    safe_simd::vst1q_s32(left, vreinterpretq_s32_u32(low));
    safe_simd::vst1q_s32(right, vreinterpretq_s32_u32(high));
  }

  let tail = 2_u64.wrapping_add(chunks.wrapping_mul(8));
  horizontal_moments_tail::<u16, i16, N>(tail..limit, scratch, output)
}

#[rite]
fn box_horizontal_16<const N: usize>(
  _token: Arm64,
  scratch: &BoxMoments<'_, u32>,
  output: &mut BoxMomentsOutput<'_, i32>,
  width: u64,
) -> Rav1dResult {
  let Ok(taps) = fixed_index(N);
  let half = taps.wrapping_div(2);
  let limit = width.wrapping_sub(2);
  let chunks = limit.wrapping_sub(2).wrapping_div(4);

  for chunk in 0..chunks {
    let x = 2_u64.wrapping_add(chunk.wrapping_mul(4));
    let mut sum = vdupq_n_u32(0);
    let mut squared = vdupq_n_u32(0);

    for tap in 0..taps {
      let first = x.wrapping_add(tap).wrapping_sub(half);
      let sums = moment_window(scratch.sums, first)?;
      let squares = moment_window(scratch.squares, first)?;
      sum = vaddq_u32(sum, safe_simd::vld1q_u32(sums));
      squared = vaddq_u32(squared, safe_simd::vld1q_u32(squares));
    }

    let output_sum = moment_window_mut(output.sums, x)?;
    let output_squares = moment_window_mut(output.squares, x)?;
    safe_simd::vst1q_s32(output_sum, vreinterpretq_s32_u32(sum));
    safe_simd::vst1q_s32(output_squares, vreinterpretq_s32_u32(squared));
  }

  let tail = 2_u64.wrapping_add(chunks.wrapping_mul(4));
  horizontal_moments_tail::<u32, i32, N>(tail..limit, scratch, output)
}

struct BoxSumPass<'a, Pixel, Sum> {
  source: &'a RestorationPixels<Pixel>,
  sum: &'a mut RestorationMoments<Sum>,
  squared: &'a mut RestorationMoments<i32>,
  extent: Size<u64>,
  radius: BoxRadius,
}

struct BoxArea<'a, Pixel, Sum, const N: usize> {
  source: &'a [RestorationRow<Pixel>; N],
  output: BoxMomentsOutput<'a, Sum>,
  width: u64,
}

fn box_rows<Pixel, Sum, Context, const N: usize>(
  pass: BoxSumPass<'_, Pixel, Sum>,
  context: &mut Context,
  mut measure: impl FnMut(BoxArea<'_, Pixel, Sum, N>, &mut Context) -> Rav1dResult,
) -> Rav1dResult {
  let Ok(pixels) = restoration_rows(pass.source);
  let Ok(sums) = restoration_rows_mut(pass.sum);
  let Ok(squares) = restoration_rows_mut(pass.squared);
  let last = pass.extent.height.wrapping_sub(4);
  let top_extension = match pass.radius {
    BoxRadius::One => 0,
    BoxRadius::Two => 1,
  };

  for row in 1_u64..=last {
    let source = moment_window(pixels, row.wrapping_sub(top_extension))?;
    let Ok(position) = index(row);
    let sum = sums.get_mut(position).ok_or(EINVAL)?;
    let squared = squares.get_mut(position).ok_or(EINVAL)?;
    measure(BoxArea {
      source,
      output: BoxMomentsOutput { sums: sum, squares: squared },
      width: pass.extent.width,
    }, context)?;
  }

  Ok(())
}

#[arcane]
fn boxsum_8bpc(token: Arm64, pass: BoxSumPass<'_, u8, i16>) -> Rav1dResult {
  let mut sums: RestorationRow<_> = [0_u16; _];
  let mut squared: RestorationRow<_> = [0_u32; _];
  let mut scratch = BoxMoments { sums: &mut sums, squares: &mut squared };

  match pass.radius {
    BoxRadius::One => box_rows::<_, _, _, 3>(pass, &mut scratch, |mut area, workspace| {
      box_vertical_8(token, area.source, workspace, area.width)?;
      box_horizontal_8::<3>(token, workspace, &mut area.output, area.width)
    }),
    BoxRadius::Two => box_rows::<_, _, _, 5>(pass, &mut scratch, |mut area, workspace| {
      box_vertical_8(token, area.source, workspace, area.width)?;
      box_horizontal_8::<5>(token, workspace, &mut area.output, area.width)
    }),
  }
}

#[arcane]
fn boxsum_16bpc(token: Arm64, pass: BoxSumPass<'_, u16, i32>) -> Rav1dResult {
  let mut sums: RestorationRow<_> = [0_u32; _];
  let mut squared: RestorationRow<_> = [0_u32; _];
  let mut scratch = BoxMoments { sums: &mut sums, squares: &mut squared };

  match pass.radius {
    BoxRadius::One => box_rows::<_, _, _, 3>(pass, &mut scratch, |mut area, workspace| {
      box_vertical_16(token, area.source, workspace, area.width)?;
      box_horizontal_16::<3>(token, workspace, &mut area.output, area.width)
    }),
    BoxRadius::Two => box_rows::<_, _, _, 5>(pass, &mut scratch, |mut area, workspace| {
      box_vertical_16(token, area.source, workspace, area.width)?;
      box_horizontal_16::<5>(token, workspace, &mut area.output, area.width)
    }),
  }
}

#[cfg(target_arch = "aarch64")]
#[rite]
fn sgr_lut_block(
  _token: Arm64,
  table: &[[u8; 16]; 4],
  lanes: uint8x16_t,
  start: u8,
) -> Result<uint8x16_t, Never> {
  let [a, b, c, d] = table;
  let block = uint8x16x4_t(
    safe_simd::vld1q_u8(a),
    safe_simd::vld1q_u8(b),
    safe_simd::vld1q_u8(c),
    safe_simd::vld1q_u8(d),
  );

  Ok(vqtbl4q_u8(block, vsubq_u8(lanes, vdupq_n_u8(start))))
}

#[cfg(target_arch = "aarch64")]
#[rite]
fn sgr_lut16(token: Arm64, lanes: uint8x16_t) -> Result<uint8x16_t, Never> {
  let samples: &[u8; 256] = &dav1d_sgr_x_by_x;
  let [first, second, third, fourth]: &[[[u8; 16]; 4]; 4] = zerocopy::transmute_ref!(samples);
  let Ok(a) = sgr_lut_block(token, first, lanes, 0);
  let Ok(b) = sgr_lut_block(token, second, lanes, 64);
  let Ok(c) = sgr_lut_block(token, third, lanes, 128);
  let Ok(d) = sgr_lut_block(token, fourth, lanes, 192);

  Ok(vorrq_u8(vorrq_u8(a, b), vorrq_u8(c, d)))
}

#[cfg(target_arch = "aarch64")]
#[rite]
fn sgr_z(_token: Arm64, a: int32x4_t, b: int32x4_t, n: i32, s: u32) -> Result<uint32x4_t, Never> {
  let variance = vmaxq_s32(
    vsubq_s32(vmulq_n_s32(a, n), vmulq_s32(b, b)),
    vdupq_n_s32(0),
  );
  let positive_variance = vreinterpretq_u32_s32(variance);
  let z = vshrq_n_u32::<20>(vaddq_u32(
    vmulq_u32(positive_variance, vdupq_n_u32(s)),
    vdupq_n_u32(::core::primitive::u32::wrapping_shl(1, 19_u32)),
  ));
  Ok(vminq_u32(z, vdupq_n_u32(255)))
}

#[cfg(target_arch = "aarch64")]
#[rite]
fn sgr_aa(_token: Arm64, x: uint32x4_t, b: int32x4_t, one_by_x: u32) -> Result<int32x4_t, Never> {
  let prod = vmulq_u32(
    vmulq_u32(x, vreinterpretq_u32_s32(b)),
    vdupq_n_u32(one_by_x),
  );
  Ok(vreinterpretq_s32_u32(vshrq_n_u32::<12>(vaddq_u32(prod, vdupq_n_u32(2048)))))
}

#[cfg(target_arch = "aarch64")]
#[rite]
fn sgr_pack_idx(_token: Arm64, z: [uint32x4_t; 4]) -> Result<uint8x16_t, Never> {
  let [first, second, third, fourth] = z;
  let low = vcombine_u16(vmovn_u32(first), vmovn_u32(second));
  let high = vcombine_u16(vmovn_u32(third), vmovn_u32(fourth));
  Ok(vcombine_u8(vmovn_u16(low), vmovn_u16(high)))
}

#[cfg(target_arch = "aarch64")]
#[rite]
fn sgr_unpack_x(_token: Arm64, x: uint8x16_t) -> Result<[uint32x4_t; 4], Never> {
  let lo = vmovl_u8(vget_low_u8(x));
  let hi = vmovl_high_u8(x);
  Ok([
    vmovl_u16(vget_low_u16(lo)),
    vmovl_high_u16(lo),
    vmovl_u16(vget_low_u16(hi)),
    vmovl_high_u16(hi),
  ])
}

#[derive(Clone, Copy)]
struct SgrModel {
  radius: BoxRadius,
  strength: u32,
  depth_shift: u32,
}

struct SgrNormalization {
  samples: i32,
  multiplier: u32,
  reciprocal: u32,
  depth_shift: u32,
}

impl SgrModel {
  fn normalization(&self) -> Result<SgrNormalization, Never> {
    let (samples, reciprocal) = match self.radius {
      BoxRadius::One => (9, 455),
      BoxRadius::Two => (25, 164),
    };
    Ok(SgrNormalization { samples, multiplier: self.strength, reciprocal, depth_shift: self.depth_shift })
  }
}

struct SgrModelRequest<'a, Coef> {
  squares: &'a mut RestorationMoments<i32>,
  sums: &'a mut RestorationMoments<Coef>,
  extent: Size<u64>,
  model: SgrModel,
}

struct ModelMoments<'a, Coef, const N: usize> {
  squared: &'a mut [i32; N],
  samples: &'a mut [Coef; N],
}

#[rite]
fn model_sum_lanes<BD: BitDepth>(
  _token: Arm64,
  sums: &[BD::Coef; 4],
) -> Rav1dResult<int32x4_t> {
  use crate::include::common::bitdepth::BPC;
  use zerocopy::{FromBytes, IntoBytes};

  Ok(match BD::BPC {
    BPC::BPC8 => {
      let pixels = FromBytes::ref_from_bytes(sums.as_bytes()).map_err(|_layout| EINVAL)?;
      vmovl_s16(safe_simd::vld1_s16(pixels))
    }
    BPC::BPC16 => {
      let pixels = FromBytes::ref_from_bytes(sums.as_bytes()).map_err(|_layout| EINVAL)?;
      safe_simd::vld1q_s32(pixels)
    }
  })
}

#[rite]
fn store_model_weights<BD: BitDepth>(
  _token: Arm64,
  sums: &mut [BD::Coef; 4],
  weights: uint32x4_t,
) -> Rav1dResult {
  use crate::include::common::bitdepth::BPC;
  use zerocopy::{FromBytes, IntoBytes};

  match BD::BPC {
    BPC::BPC8 => {
      let pixels = FromBytes::mut_from_bytes(sums.as_mut_bytes()).map_err(|_layout| EINVAL)?;
      safe_simd::vst1_s16(pixels, vmovn_s32(vreinterpretq_s32_u32(weights)));
    }
    BPC::BPC16 => {
      let pixels = FromBytes::mut_from_bytes(sums.as_mut_bytes()).map_err(|_layout| EINVAL)?;
      safe_simd::vst1q_s32(pixels, vreinterpretq_s32_u32(weights));
    }
  }

  Ok(())
}

#[rite]
fn sgr_model_group<BD: BitDepth>(
  token: Arm64,
  moments: ModelMoments<'_, BD::Coef, 16>,
  norm: &SgrNormalization,
) -> Rav1dResult {
  let square_shift = norm.depth_shift.wrapping_mul(2);
  let sum_shift = norm.depth_shift;
  let square_round = vdupq_n_s32(1_i32.wrapping_shl(square_shift).wrapping_shr(1));
  let sum_round = vdupq_n_s32(1_i32.wrapping_shl(sum_shift).wrapping_shr(1));
  let square_right = vdupq_n_s32(square_shift.cast_signed().wrapping_neg());
  let sum_right = vdupq_n_s32(sum_shift.cast_signed().wrapping_neg());
  let squares: &mut [[i32; 4]; 4] = zerocopy::transmute_mut!(moments.squared);
  let sums: &mut [[BD::Coef; 4]; 4] = zerocopy::transmute_mut!(moments.samples);
  let mut raw_sums = [vdupq_n_s32(0); 4];
  let mut indices = [vdupq_n_u32(0); 4];

  for ((squared, sum), (raw, position)) in squares.iter().zip(&*sums).zip(raw_sums.iter_mut().zip(&mut indices)) {
    let square = safe_simd::vld1q_s32(squared);
    let values = model_sum_lanes::<BD>(token, sum)?;
    *raw = values;
    let a = vshlq_s32(vaddq_s32(square, square_round), square_right);
    let b = vshlq_s32(vaddq_s32(values, sum_round), sum_right);
    let Ok(index_vector) = sgr_z(token, a, b, norm.samples, norm.multiplier);
    *position = index_vector;
  }

  let Ok(packed_indices) = sgr_pack_idx(token, indices);
  let Ok(looked_up) = sgr_lut16(token, packed_indices);
  let Ok(weights) = sgr_unpack_x(token, looked_up);

  for ((square, sum), (raw, weight)) in squares.iter_mut().zip(sums).zip(raw_sums.into_iter().zip(weights)) {
    let Ok(intercept) = sgr_aa(token, weight, raw, norm.reciprocal);
    safe_simd::vst1q_s32(square, intercept);
    store_model_weights::<BD>(token, sum, weight)?;
  }

  Ok(())
}

fn sgr_model_scalar<BD: BitDepth>(
  squares: &mut [i32],
  samples: &mut [BD::Coef],
  norm: &SgrNormalization,
) -> Rav1dResult {
  let square_shift = norm.depth_shift.wrapping_mul(2);
  let square_round = 1_i32.wrapping_shl(square_shift).wrapping_shr(1);
  let sum_round = 1_i32.wrapping_shl(norm.depth_shift).wrapping_shr(1);

  for (square, sum) in squares.iter_mut().zip(samples) {
    let raw = (*sum).as_::<i32>();
    let a = square.wrapping_add(square_round).wrapping_shr(square_shift);
    let b = raw.wrapping_add(sum_round).wrapping_shr(norm.depth_shift);
    let variance = a.wrapping_mul(norm.samples).wrapping_sub(b.wrapping_mul(b)).max(0).cast_unsigned();
    let z = variance.wrapping_mul(norm.multiplier).wrapping_add(::core::primitive::u32::wrapping_shl(1, 19_u32)).wrapping_shr(20);
    let Ok(position) = index(z.min(255));
    let &weight = dav1d_sgr_x_by_x.get(position).ok_or(EINVAL)?;
    let coefficient = u32::from(weight);
    let product = coefficient.wrapping_mul(raw.cast_unsigned()).wrapping_mul(norm.reciprocal);
    *square = product.wrapping_add(::core::primitive::u32::wrapping_shl(1, 11_u32)).wrapping_shr(12).cast_signed();
    *sum = i32::from(weight).as_::<BD::Coef>();
  }

  Ok(())
}

fn validate_sgr_sums<BD: BitDepth>(
  squares: &[i32],
  sums: &[BD::Coef],
  norm: &SgrNormalization,
) -> Rav1dResult {
  let maximum = 1_i32.wrapping_shl(norm.depth_shift.wrapping_add(8)).wrapping_sub(1);
  let sample_total = norm.samples.wrapping_mul(maximum);
  let square_total = sample_total.wrapping_mul(maximum);

  for (&square, &sample) in squares.iter().zip(sums) {
    match ((0..=square_total).contains(&square) && (0..=sample_total).contains(&sample.as_::<i32>())).then_some(()) {
      Some(()) => {}
      None => return Err(EINVAL),
    }
  }

  Ok(())
}

#[arcane]
fn sgr_ab<BD: BitDepth>(token: Arm64, request: SgrModelRequest<'_, BD::Coef>) -> Rav1dResult {
  let Ok(norm) = request.model.normalization();
  let stride = match request.model.radius {
    BoxRadius::One => 1_u64,
    BoxRadius::Two => 2_u64,
  };
  let height = request.extent.height.wrapping_add(2);
  let Ok(right) = index(request.extent.width.wrapping_add(4));
  let Ok(squares) = restoration_rows_mut(request.squares);
  let Ok(sums) = restoration_rows_mut(request.sums);
  let rows = match request.model.radius {
    BoxRadius::One => height,
    BoxRadius::Two => height.div_ceil(2),
  };

  for row_number in 0..rows {
    let Ok(position) = index(row_number.wrapping_mul(stride).wrapping_add(1));
    let squared = squares.get_mut(position).and_then(|row| row.get_mut(2..right)).ok_or(EINVAL)?;
    let samples = sums.get_mut(position).and_then(|row| row.get_mut(2..right)).ok_or(EINVAL)?;

    validate_sgr_sums::<BD>(squared, samples, &norm)?;

    let (vector_squares, tail_squares) = squared.as_chunks_mut::<16>();
    let (vector_samples, tail_samples) = samples.as_chunks_mut::<16>();

    for (square_lanes, sum_lanes) in vector_squares.iter_mut().zip(vector_samples) {
      sgr_model_group::<BD>(token, ModelMoments { squared: square_lanes, samples: sum_lanes }, &norm)?;
    }

    sgr_model_scalar::<BD>(tail_squares, tail_samples, &norm)?;
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum SgrMomentPlane<'a> {
  Narrow(&'a [i16]),
  Wide(&'a [i32]),
}

fn sgr_moment_at(plane: SgrMomentPlane<'_>, offset: u64) -> Rav1dResult<i32> {
  let Ok(position) = index(offset);

  match plane {
    SgrMomentPlane::Narrow(data) => {
      let &sample = data.get(position).ok_or(EINVAL)?;
      Ok(i32::from(sample))
    }
    SgrMomentPlane::Wide(data) => data.get(position).copied().ok_or(EINVAL),
  }
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn sgr_moment_vector(
  _token: Arm64,
  plane: SgrMomentPlane<'_>,
  offset: u64,
) -> Rav1dResult<int32x4_t> {
  let Ok(position) = index(offset);

  match plane {
    SgrMomentPlane::Narrow(data) => {
      let remaining = data.get(position..).ok_or(EINVAL)?;
      let lanes = remaining.first_chunk::<4>().ok_or(EINVAL)?;
      Ok(vmovl_s16(safe_simd::vld1_s16(lanes)))
    }
    SgrMomentPlane::Wide(data) => {
      let remaining = data.get(position..).ok_or(EINVAL)?;
      let lanes = remaining.first_chunk::<4>().ok_or(EINVAL)?;
      Ok(safe_simd::vld1q_s32(lanes))
    }
  }
}

fn sgr_weighted_scalar(
  plane: SgrMomentPlane<'_>,
  position: u64,
  stencil: SgrStencil,
) -> Rav1dResult<i32> {
  let Ok(groups) = stencil.taps();
  let mut total = 0_i32;

  for group in groups {
    let mut sum = 0_i32;

    for &offset in group.positions {
      let sample = sgr_moment_at(plane, position.wrapping_add_signed(offset))?;
      sum = sum.wrapping_add(sample);
    }

    total = total.wrapping_add(sum.wrapping_mul(group.weight));
  }

  Ok(total)
}

#[cfg(target_arch = "aarch64")]
#[rite(neon)]
fn sgr_weighted_vector(
  token: Arm64,
  plane: SgrMomentPlane<'_>,
  position: u64,
  stencil: SgrStencil,
) -> Rav1dResult<int32x4_t> {
  let Ok(groups) = stencil.taps();
  let mut total = vdupq_n_s32(0);

  for group in groups {
    let mut sum = vdupq_n_s32(0);

    for &offset in group.positions {
      let sample = sgr_moment_vector(token, plane, position.wrapping_add_signed(offset))?;
      sum = vaddq_s32(sum, sample);
    }

    total = vmlaq_n_s32(total, sum, group.weight);
  }

  Ok(total)
}

struct SgrOutput<'a, BD: BitDepth> {
  destination: &'a mut [BD::Coef],
  pixels: &'a [BD::Pixel],
  weights: SgrMomentPlane<'a>,
  intercepts: SgrMomentPlane<'a>,
  extent: Size<u64>,
  radius: i32,
}

#[cfg(target_arch = "aarch64")]
#[arcane]
fn sgr_output<BD: BitDepth>(token: Arm64, output: SgrOutput<'_, BD>) -> Rav1dResult {
  let stride = SGR_STRIDE.cast_unsigned();
  let target_stride = u64::from(RESTORATION_INTERIOR);

  for y in 0..output.extent.height {
    let radius = match output.radius {
      25 => BoxRadius::Two,
      _three_by_three => BoxRadius::One,
    };
    let Ok(stencil) = radius.stencil(y);
    let Ok((rounding, shift)) = stencil.rounding();
    let moment = y.wrapping_add(2).wrapping_mul(stride).wrapping_add(3);
    let pixel = moment.wrapping_add(stride);
    let into = y.wrapping_mul(target_stride);
    let Ok(start) = index(pixel);
    let Ok(end) = index(pixel.wrapping_add(output.extent.width));
    let source = output.pixels.get(start..end).ok_or(EINVAL)?;
    let Ok(target_start) = index(into);
    let Ok(target_end) = index(into.wrapping_add(output.extent.width));
    let destination = output.destination.get_mut(target_start..target_end).ok_or(EINVAL)?;
    let (source_vectors, scalar_pixels) = source.as_chunks::<4>();
    let (destination_vectors, scalar_output) = destination.as_chunks_mut::<4>();
    let blocks = output.extent.width.wrapping_div(4);

    for (block, (samples, deltas)) in (0..blocks).zip(source_vectors.iter().zip(destination_vectors)) {
      let position = moment.wrapping_add(block.wrapping_mul(4));
      let intercept = sgr_weighted_vector(token, output.intercepts, position, stencil)?;
      let weight = sgr_weighted_vector(token, output.weights, position, stencil)?;
      let lanes = safe_simd::vld1q_s32(&samples.map(|sample| sample.as_::<i32>()));
      let score = vaddq_s32(vsubq_s32(intercept, vmulq_s32(weight, lanes)), vdupq_n_s32(rounding));
      let shifted = match stencil {
        SgrStencil::Middle => vshrq_n_s32::<8>(score),
        SgrStencil::Alternate | SgrStencil::Full => vshrq_n_s32::<9>(score),
      };
      let mut values = [0_i32; 4];
      safe_simd::vst1q_s32(&mut values, shifted);
      *deltas = values.map(|value| value.as_::<BD::Coef>());
    }

    for (column, (&sample, delta)) in (blocks.wrapping_mul(4)..output.extent.width).zip(scalar_pixels.iter().zip(scalar_output)) {
      let position = moment.wrapping_add(column);
      let intercept = sgr_weighted_scalar(output.intercepts, position, stencil)?;
      let weight = sgr_weighted_scalar(output.weights, position, stencil)?;
      let result = intercept.wrapping_sub(weight.wrapping_mul(sample.as_::<i32>())).wrapping_add(rounding).wrapping_shr(shift);
      *delta = result.as_::<BD::Coef>();
    }
  }

  Ok(())
}

struct SgrResidual<'a, Coef> {
  values: &'a RestorationDeltas<Coef>,
  weight: i32,
}

struct SgrProjection<'a, Coef> {
  destination: PicOffset<'a>,
  extent: Size<u64>,
  maximum: i32,
  first: SgrResidual<'a, Coef>,
  second: Option<SgrResidual<'a, Coef>>,
}

#[rite]
fn sgr_residual_lanes<BD: BitDepth>(
  _token: Arm64,
  values: &[BD::Coef; 8],
) -> Rav1dResult<[int32x4_t; 2]> {
  use crate::include::common::bitdepth::BPC;
  use zerocopy::{FromBytes, IntoBytes};

  Ok(match BD::BPC {
    BPC::BPC8 => {
      let narrow_values = FromBytes::ref_from_bytes(values.as_bytes()).map_err(|_layout| EINVAL)?;
      let samples = safe_simd::vld1q_s16(narrow_values);
      [vmovl_s16(vget_low_s16(samples)), vmovl_high_s16(samples)]
    }
    BPC::BPC16 => {
      let [first, second]: &[[i32; 4]; 2] =
        FromBytes::ref_from_bytes(values.as_bytes()).map_err(|_layout| EINVAL)?;
      [safe_simd::vld1q_s32(first), safe_simd::vld1q_s32(second)]
    }
  })
}

#[rite]
fn apply_sgr_lanes<BD: BitDepth>(
  token: Arm64,
  output: &mut [BD::Pixel; 8],
  projection: &SgrProjection<'_, BD::Coef>,
  position: u64,
) -> Rav1dResult {
  let primary = moment_window(projection.first.values, position)?;
  let [primary_low, primary_high] = sgr_residual_lanes::<BD>(token, primary)?;
  let mut low = vmulq_n_s32(primary_low, projection.first.weight);
  let mut high = vmulq_n_s32(primary_high, projection.first.weight);

  match &projection.second {
    Some(second) => {
      let residual = moment_window(second.values, position)?;
      let [first, last] = sgr_residual_lanes::<BD>(token, residual)?;
      low = vmlaq_n_s32(low, first, second.weight);
      high = vmlaq_n_s32(high, last, second.weight);
    }
    None => {}
  }

  let pixels = load_pixel_lanes::<BD>(token, &*output)?;
  let rounding = vdupq_n_s32(1024);
  let low_delta = vshrq_n_s32::<11>(vaddq_s32(low, rounding));
  let high_delta = vshrq_n_s32::<11>(vaddq_s32(high, rounding));
  let low_output = vaddq_s32(vmovl_s16(vget_low_s16(pixels)), low_delta);
  let high_output = vaddq_s32(vmovl_high_s16(pixels), high_delta);
  let maximum = vdupq_n_s32(projection.maximum);
  let zero = vdupq_n_s32(0);
  let low_clipped = vminq_s32(vmaxq_s32(low_output, zero), maximum);
  let high_clipped = vminq_s32(vmaxq_s32(high_output, zero), maximum);
  let first = vmovn_u32(vreinterpretq_u32_s32(low_clipped));
  let last = vmovn_u32(vreinterpretq_u32_s32(high_clipped));
  store_pixel_lanes::<BD>(token, output, vcombine_u16(first, last))
}

fn sgr_delta<BD: BitDepth>(
  projection: &SgrProjection<'_, BD::Coef>,
  position: u64,
) -> Rav1dResult<i32> {
  let Ok(offset) = index(position);
  let &first = projection.first.values.get(offset).ok_or(EINVAL)?;
  let primary = first.as_::<i32>().wrapping_mul(projection.first.weight);
  let total = match &projection.second {
    Some(second) => {
      let &value = second.values.get(offset).ok_or(EINVAL)?;
      primary.wrapping_add(value.as_::<i32>().wrapping_mul(second.weight))
    }
    None => primary,
  };
  Ok(total.wrapping_add(1024).wrapping_shr(11))
}

#[arcane]
fn apply_sgr_projection<BD: BitDepth>(
  token: Arm64,
  projection: SgrProjection<'_, BD::Coef>,
) -> Rav1dResult {
  let Ok(width) = index(projection.extent.width);
  let Ok(pitch) = projection.destination.fixed_pixel_stride::<BD>();
  let workspace_width = u64::from(RESTORATION_INTERIOR);

  for row in 0..projection.extent.height {
    let displacement = row.cast_signed().wrapping_mul(pitch);
    let Ok(offset) = console_core_number_conversion::wrapped_offset(displacement);
    let mut output = (projection.destination + offset).slice_mut::<BD>(width);
    let (vectors, tail) = output.as_chunks_mut::<8>();
    let base = row.wrapping_mul(workspace_width);

    for (column, block) in (0_u64..).zip(vectors) {
      let position = base.wrapping_add(column.wrapping_mul(8));
      apply_sgr_lanes::<BD>(token, block, &projection, position)?;
    }

    let Ok(tail_len) = fixed_index(tail.len());
    let first = projection.extent.width.wrapping_sub(tail_len);

    for (column, pixel) in (first..projection.extent.width).zip(tail) {
      let delta = sgr_delta::<BD>(&projection, base.wrapping_add(column))?;
      let updated = (*pixel).as_::<i32>().wrapping_add(delta);
      *pixel = iclip(updated, 0, projection.maximum).as_::<BD::Pixel>();
    }
  }

  Ok(())
}

struct SgrCalculation<'a, BD: BitDepth> {
  source: &'a RestorationPixels<BD::Pixel>,
  destination: &'a mut RestorationDeltas<BD::Coef>,
  squares: &'a mut RestorationMoments<i32>,
  sums: &'a mut RestorationMoments<BD::Coef>,
  extent: Size<u64>,
  model: SgrModel,
}

#[cfg(target_arch = "aarch64")]
fn selfguided_8bpc(token: Arm64, calculation: SgrCalculation<'_, BitDepth8>) -> Rav1dResult {
  let extent = calculation.extent;
  let padded = Size { width: extent.width.wrapping_add(6), height: extent.height.wrapping_add(6) };
  let radius = calculation.model.radius;
  boxsum_8bpc(token, BoxSumPass {
    source: calculation.source, sum: calculation.sums, squared: calculation.squares,
    extent: padded, radius,
  })?;

  let Ok(normalization) = calculation.model.normalization();
  sgr_ab::<BitDepth8>(token, SgrModelRequest {
    squares: calculation.squares, sums: calculation.sums, extent, model: calculation.model,
  })?;

  sgr_output::<BitDepth8>(token, SgrOutput {
    destination: calculation.destination, pixels: calculation.source,
    weights: SgrMomentPlane::Narrow(calculation.sums), intercepts: SgrMomentPlane::Wide(calculation.squares),
    extent, radius: normalization.samples,
  })
}

#[cfg(target_arch = "aarch64")]
fn sgr_8bpc(
  token: Arm64,
  request: RestorationPadding<'_, BitDepth8>,
  params: &LooprestorationParams,
  selection: SgrSelection,
) -> Rav1dResult {
  let depth_shift = 0;
  let maximum = 255;
  with_scratch(&SCRATCH8, Scratch8::new, |scratch| {
    let destination = request.picture;
    let extent = request.size;
    padding::<BitDepth8>(scratch.tmp.as_mut(), request)?;
    let sgr = params.sgr()?;
    let (radius, strength, weight) = match selection {
      SgrSelection::FiveByFive | SgrSelection::Mixed => (BoxRadius::Two, sgr.s0, i32::from(sgr.w0)),
      SgrSelection::ThreeByThree => (BoxRadius::One, sgr.s1, i32::from(sgr.w1)),
    };
    selfguided_8bpc(token, SgrCalculation {
      source: &scratch.tmp, destination: &mut scratch.d0, squares: &mut scratch.sumsq,
      sums: &mut scratch.sum, extent, model: SgrModel { radius, strength, depth_shift },
    })?;

    let secondary = match selection {
      SgrSelection::FiveByFive | SgrSelection::ThreeByThree => None,
      SgrSelection::Mixed => {
        selfguided_8bpc(token, SgrCalculation {
          source: &scratch.tmp, destination: &mut scratch.d1, squares: &mut scratch.sumsq,
          sums: &mut scratch.sum, extent,
          model: SgrModel { radius: BoxRadius::One, strength: sgr.s1, depth_shift },
        })?;
        Some(SgrResidual { values: &scratch.d1, weight: i32::from(sgr.w1) })
      }
    };

    apply_sgr_projection::<BitDepth8>(token, SgrProjection {
      destination, extent, maximum,
      first: SgrResidual { values: &scratch.d0, weight }, second: secondary,
    })
  })
}

#[cfg(target_arch = "aarch64")]
fn selfguided_16bpc(token: Arm64, calculation: SgrCalculation<'_, BitDepth16>) -> Rav1dResult {
  let extent = calculation.extent;
  let padded = Size { width: extent.width.wrapping_add(6), height: extent.height.wrapping_add(6) };
  let radius = calculation.model.radius;
  boxsum_16bpc(token, BoxSumPass {
    source: calculation.source, sum: calculation.sums, squared: calculation.squares,
    extent: padded, radius,
  })?;

  let Ok(normalization) = calculation.model.normalization();
  sgr_ab::<BitDepth16>(token, SgrModelRequest {
    squares: calculation.squares, sums: calculation.sums, extent, model: calculation.model,
  })?;

  sgr_output::<BitDepth16>(token, SgrOutput {
    destination: calculation.destination, pixels: calculation.source,
    weights: SgrMomentPlane::Wide(calculation.sums), intercepts: SgrMomentPlane::Wide(calculation.squares),
    extent, radius: normalization.samples,
  })
}

#[cfg(target_arch = "aarch64")]
fn sgr_16bpc(
  token: Arm64,
  request: RestorationPadding<'_, BitDepth16>,
  params: &LooprestorationParams,
  selection: SgrSelection,
  maximum: i32,
) -> Rav1dResult {
  let depth_shift = match maximum {
    1023 => 2,
    _high_depth => 4,
  };
  with_scratch(&SCRATCH16, Scratch16::new, |scratch| {
    let destination = request.picture;
    let extent = request.size;
    padding::<BitDepth16>(scratch.tmp.as_mut(), request)?;
    let sgr = params.sgr()?;
    let (radius, strength, weight) = match selection {
      SgrSelection::FiveByFive | SgrSelection::Mixed => (BoxRadius::Two, sgr.s0, i32::from(sgr.w0)),
      SgrSelection::ThreeByThree => (BoxRadius::One, sgr.s1, i32::from(sgr.w1)),
    };
    selfguided_16bpc(token, SgrCalculation {
      source: &scratch.tmp, destination: &mut scratch.d0, squares: &mut scratch.sumsq,
      sums: &mut scratch.sum, extent, model: SgrModel { radius, strength, depth_shift },
    })?;

    let secondary = match selection {
      SgrSelection::FiveByFive | SgrSelection::ThreeByThree => None,
      SgrSelection::Mixed => {
        selfguided_16bpc(token, SgrCalculation {
          source: &scratch.tmp, destination: &mut scratch.d1, squares: &mut scratch.sumsq,
          sums: &mut scratch.sum, extent,
          model: SgrModel { radius: BoxRadius::One, strength: sgr.s1, depth_shift },
        })?;
        Some(SgrResidual { values: &scratch.d1, weight: i32::from(sgr.w1) })
      }
    };

    apply_sgr_projection::<BitDepth16>(token, SgrProjection {
      destination, extent, maximum,
      first: SgrResidual { values: &scratch.d0, weight }, second: secondary,
    })
  })
}

#[cfg(target_arch = "aarch64")]
pub(crate) fn lr_filter_dispatch<BD: BitDepth>(
  request: &RestorationRequest<'_, BD>,
) -> Rav1dResult<RestorationDispatch> {
  use crate::include::common::bitdepth::BPC;
  use crate::src::ablate::{Family, is_off, note};
  use crate::src::safe_simd::pixel_access::reinterpret_slice;
  use archmage::SimdToken as _;

  let RestorationPadding { picture, backup, lines, offset, size, edges } = request.padding;
  let Ok(()) = note(Family::LoopRestoration, size.width.wrapping_mul(size.height));

  match is_off(Family::LoopRestoration).then_some(()) {
    Some(()) => return Ok(RestorationDispatch::Fallback),
    None => {}
  }

  let token = match Arm64::summon() {
    Some(proof) => proof,
    None => return Ok(RestorationDispatch::Fallback),
  };
  let params = request.parameters;

  let applied = match BD::BPC {
    BPC::BPC8 => {
      let left = match reinterpret_slice(backup) {
        Ok(Some(rows)) => rows,
        Ok(None) => return Ok(RestorationDispatch::Fallback),
      };

      let padding = RestorationPadding { picture, backup: left, lines, offset, size, edges };

      match request.kernel {
        RestorationKernel::Wiener => wiener_8bpc(token, padding, params),
        RestorationKernel::SelfGuided(selection) => sgr_8bpc(token, padding, params, selection),
      }
    }
    BPC::BPC16 => {
      let left = match reinterpret_slice(backup) {
        Ok(Some(rows)) => rows,
        Ok(None) => return Ok(RestorationDispatch::Fallback),
      };
      let maximum = request.bitdepth.into_c();

      let padding = RestorationPadding { picture, backup: left, lines, offset, size, edges };

      match request.kernel {
        RestorationKernel::Wiener => wiener_16bpc(token, padding, params, maximum),
        RestorationKernel::SelfGuided(selection) => sgr_16bpc(token, padding, params, selection, maximum),
      }
    }
  };
  applied?;

  Ok(RestorationDispatch::Applied)
}
