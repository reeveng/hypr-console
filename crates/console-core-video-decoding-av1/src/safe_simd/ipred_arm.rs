//! ARM NEON chroma-from-luma prediction and AC extraction.
//!
//! Extraction sums each group of adjacent luma samples, scales it by
//! eight divided by the group size and subtracts the rounded block average.
//! AV1 CfL widths are multiples of four. The row guard and residual iterator
//! lend complete groups. The kernel borrows one row at a time and validates
//! the shape before writing any pixels.

#![forbid(unsafe_code)]

use archmage::intrinsics::aarch64 as safe_simd;
use archmage::{Arm64, SimdToken, arcane};
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{index, indices_to};
use core::arch::aarch64::*;
use std::ffi::c_int;

use crate::include::common::bitdepth::{AsPrimitive, BPC, BitDepth, BitDepth8, BitDepth16};
use crate::include::dav1d::headers::Rav1dPixelLayoutSubSampled;
use crate::src::ipred::{CflAcExtraction, CflAcRegion, CflProjection, PredictionDispatch};
use crate::src::owned_recon::ReconDst;

#[derive(Clone, Copy)]
struct ProjectionCoefficients {
  alpha: int32x4_t,
  dc: int32x4_t,
  maximum: int32x4_t,
}

#[archmage::rite(neon)]
fn cfl_lane4(ac: int32x4_t, coeff: ProjectionCoefficients) -> Result<int32x4_t, Never> {
  let product = vmulq_s32(ac, coeff.alpha);
  let magnitude = vshrq_n_s32::<6>(vaddq_s32(vabsq_s32(product), vdupq_n_s32(32)));
  let sign = vshrq_n_s32::<31>(product);
  let correction = vsubq_s32(veorq_s32(magnitude, sign), sign);
  let predicted = vaddq_s32(coeff.dc, correction);

  Ok(vminq_s32(
    vmaxq_s32(predicted, vdupq_n_s32(0)),
    coeff.maximum,
  ))
}

#[arcane]
fn cfl_row_8bpc(
  _token: Arm64,
  dst: &mut [u8],
  ac: &[i16],
  projection: &CflProjection,
) -> Result<(), Never> {
  let coeff = ProjectionCoefficients {
    alpha: vdupq_n_s32(projection.alpha),
    dc: vdupq_n_s32(projection.dc),
    maximum: vdupq_n_s32(255),
  };
  let (residuals, _) = ac.as_chunks::<4>();
  let (pixels, _) = dst.as_chunks_mut::<4>();

  for (out, residual) in pixels.iter_mut().zip(residuals) {
    let acv = vmovl_s16(safe_simd::vld1_s16(residual));
    let Ok(predicted) = cfl_lane4(acv, coeff);
    let narrow = vmovn_u32(vreinterpretq_u32_s32(predicted));
    let bytes = vmovn_u16(vcombine_u16(narrow, narrow));
    let mut lanes = [0_u8; 8];
    safe_simd::vst1_u8(&mut lanes, bytes);
    let [first, second, third, fourth, ..] = lanes;
    *out = [first, second, third, fourth];
  }

  Ok(())
}

#[arcane]
fn cfl_row_16bpc(
  _token: Arm64,
  dst: &mut [u16],
  ac: &[i16],
  projection: &CflProjection,
  bitdepth_max: c_int,
) -> Result<(), Never> {
  let coeff = ProjectionCoefficients {
    alpha: vdupq_n_s32(projection.alpha),
    dc: vdupq_n_s32(projection.dc),
    maximum: vdupq_n_s32(bitdepth_max),
  };
  let (residuals, _) = ac.as_chunks::<4>();
  let (pixels, _) = dst.as_chunks_mut::<4>();

  for (out, residual) in pixels.iter_mut().zip(residuals) {
    let acv = vmovl_s16(safe_simd::vld1_s16(residual));
    let Ok(predicted) = cfl_lane4(acv, coeff);
    let narrow = vmovn_u32(vreinterpretq_u32_s32(predicted));
    safe_simd::vst1_u16(out, narrow);
  }

  Ok(())
}

pub(crate) fn cfl_pred_dispatch<BD: BitDepth>(
  dst: &mut ReconDst<'_>,
  ac: &[i16],
  projection: CflProjection,
  bd: BD,
) -> Result<PredictionDispatch, Never> {
  use crate::src::ablate::{self, Family};

  match ablate::is_off(Family::IntraPred).then_some(()) {
    Some(()) => return Ok(PredictionDispatch::ScalarFallback),
    None => {}
  }

  let token = match Arm64::summon() {
    Some(available) => available,
    None => return Ok(PredictionDispatch::ScalarFallback),
  };
  let Size {
    width: raw_width,
    height: raw_height,
  } = projection.size;
  let (width, height) = match (u32::try_from(raw_width), u32::try_from(raw_height)) {
    (Ok(columns @ 4..), Ok(rows @ 1..)) => (columns, rows),
    (_width, _height) => return Ok(PredictionDispatch::ScalarFallback),
  };

  match width.wrapping_rem(4) {
    0 => {}
    1.. => return Ok(PredictionDispatch::ScalarFallback),
  }

  let samples = u64::from(width).wrapping_mul(u64::from(height));
  let Ok(range) = indices_to(..samples);
  let active = match ac.get(range) {
    Some(block) => block,
    None => return Ok(PredictionDispatch::ScalarFallback),
  };
  let Ok(columns) = index(width);
  let extent = Size {
    width: u64::from(width),
    height: u64::from(height),
  };
  let mut residual_rows = active.chunks_exact(columns);
  let written = match BD::BPC {
    BPC::BPC8 => {
      dst.for_rows_mut::<BitDepth8, _>(extent, |_row, pixels| match residual_rows.next() {
        Some(residual) => {
          let Ok(()) = cfl_row_8bpc(token, pixels, residual, &projection);
        }
        None => {}
      })
    }
    BPC::BPC16 => {
      let bitdepth_max = bd.bitdepth_max().as_::<c_int>();

      dst.for_rows_mut::<BitDepth16, _>(extent, |_row, pixels| match residual_rows.next() {
        Some(residual) => {
          let Ok(()) = cfl_row_16bpc(token, pixels, residual, &projection, bitdepth_max);
        }
        None => {}
      })
    }
  };

  match written {
    Ok(()) => Ok(PredictionDispatch::Applied),
    Err(problem) => {
      console_core_logging::debug!("Invalid AV1 NEON chroma destination: {problem:?}");
      Ok(PredictionDispatch::ScalarFallback)
    }
  }
}

#[cfg(all(test, target_arch = "aarch64", not(feature = "c-ffi")))]
mod cfl_parity {
  //! Differential parity for `cfl_pred_dispatch` against the scalar
  //! `src/ipred.rs::cfl_pred` it replaces.
  //!
  //! The scalar path is the conformance oracle, and unlike itx there is no
  //! `__simd_test` dual-compute hook on this call — so this sweep is the
  //! only per-parameter evidence, and the corpus MD5 is the only other one.
  //!
  //! The parameter space is the real one: `alpha` is signalled in
  //! `[-16, 16]`, `ac` holds bitdepth-scaled AC residual, and every AV1 CfL
  //! block is 4..32 on a side. All four are swept, at both bit depths, plus
  //! the saturating extremes that decide whether the clip is right.

  use crate::include::common::bitdepth::{BitDepth, BitDepth8, BitDepth16};
  use crate::include::dav1d::picture::Rav1dPictureDataComponent;

  struct Rng(u64);
  impl Rng {
    fn in_range(&mut self, lo: i32, hi: i32) -> i32 {
      let Ok(seeded) = crate::src::safe_simd::next_fixture_sample(&mut self.0);
      lo + (seeded % ((hi - lo + 1) as u64)) as i32
    }
  }

  fn run<BD: BitDepth>(
    w: usize,
    h: usize,
    dc: i32,
    alpha: i32,
    ac: &[i16],
    pixels: &[BD::Pixel],
    stride: usize,
    bd: BD,
  ) -> (Vec<BD::Pixel>, Vec<BD::Pixel>, bool)
  where
    BD::Pixel: Copy + Default,
  {
    let go = |simd: bool| -> (Vec<BD::Pixel>, bool) {
      let mut px = pixels.to_vec();
      let mut out = vec![BD::Pixel::default(); px.len()];
      let comp = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut px, stride);
      let dst = comp.with_offset::<BD>();
      let handled = if simd {
        let Ok(dispatched) = super::cfl_pred_dispatch::<BD>(
          &mut crate::src::owned_recon::ReconDst::Pic(dst),
          ac,
          super::CflProjection {
            size: console_core_geometry::Size {
              width: w as i32,
              height: h as i32,
            },
            dc,
            alpha,
          },
          bd,
        );
        matches!(dispatched, super::PredictionDispatch::Applied)
      } else {
        let mut fixed: crate::src::internal::CoefficientScratch<i16> = [0; 32 * 32];
        fixed[..ac.len()].copy_from_slice(ac);
        let result = crate::src::ipred::cfl_pred_scalar_for_test::<BD>(
          &mut crate::src::owned_recon::ReconDst::Pic(dst),
          &fixed,
          super::CflProjection {
            size: console_core_geometry::Size {
              width: w as i32,
              height: h as i32,
            },
            dc,
            alpha,
          },
          bd,
        );
        result.is_ok()
      };
      comp.copy_pixels_to::<BD>(&mut out);
      (out, handled)
    };
    let (neon, handled) = go(true);
    let (scalar, _) = go(false);
    (neon, scalar, handled)
  }

  fn sweep<BD: BitDepth>(bd: BD, seed: u64, what: &str)
  where
    BD::Pixel: Copy + Default + PartialEq + std::fmt::Debug,
  {
    use crate::include::common::bitdepth::AsPrimitive;
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let pmax = bd.bitdepth_max().as_::<i32>();
    let mut cells = 0usize;
    let mut live = 0usize;
    let mut bad: Vec<String> = Vec::new();

    for &w in &[4usize, 8, 16, 32] {
      for &h in &[4usize, 8, 16, 32] {
        let stride = (w + 16).next_multiple_of(16);
        let mut rng = Rng(seed ^ ((w as u64) << 32) ^ (h as u64));
        for &alpha in &[-16i32, -13, -8, -3, -1, 0, 1, 3, 8, 13, 16] {
          for &(dc, scale) in &[
            (pmax / 2, pmax / 4),
            (0, pmax),
            (pmax, pmax),
            (pmax / 2, 1 << 13),
            (1, 3),
          ] {
            let ac: Vec<i16> = (0..w * h)
              .map(|_| rng.in_range(-scale, scale).clamp(-32768, 32767) as i16)
              .collect();
            let pixels: Vec<BD::Pixel> = (0..stride * h)
              .map(|_| rng.in_range(0, pmax).as_())
              .collect();
            let (neon, scalar, ok) = run::<BD>(w, h, dc, alpha, &ac, &pixels, stride, bd);
            cells += 1;
            if ok {
              live += 1;
            }
            if let Some((x, y)) = (0..h)
              .flat_map(|y| (0..w).map(move |x| (x, y)))
              .find(|&(x, y)| neon[y * stride + x] != scalar[y * stride + x])
            {
              let msg = format!(
                "{w}x{h} alpha={alpha} dc={dc} scale={scale} at ({x},{y}): \
                                 neon={:?} scalar={:?}",
                neon[y * stride + x],
                scalar[y * stride + x]
              );
              if bad.len() < 4 {
                bad.push(msg);
              }
            }
          }
        }
      }
    }
    assert!(cells >= 800, "{what}: only {cells} cells ran");
    assert_eq!(
      live,
      cells,
      "{what}: {} of {cells} cells did NOT take the NEON path — those \
             compared the scalar reference against itself and proved nothing",
      cells - live
    );
    assert!(bad.is_empty(), "{what}: divergence\n  {}", bad.join("\n  "));
  }

  #[test]
  fn cfl_pred_8bpc_matches_scalar() {
    sweep(BitDepth8::new(()), 0x51DE_0001_0000_0001, "cfl 8bpc");
  }

  #[test]
  fn cfl_pred_10bpc_matches_scalar() {
    sweep(BitDepth16::new(1023), 0x51DE_0002_0000_0001, "cfl 10bpc");
  }

  #[test]
  fn cfl_pred_12bpc_matches_scalar() {
    sweep(BitDepth16::new(4095), 0x51DE_0003_0000_0001, "cfl 12bpc");
  }
}

struct ChromaSourceRows<'a, Sample> {
  top: &'a [Sample],
  bottom: &'a [Sample],
}

#[arcane]
fn ac_row_420_8bpc(
  _token: Arm64,
  out: &mut [i16],
  rows: ChromaSourceRows<'_, u8>,
) -> Result<(), Never> {
  let (words, rest) = out.as_chunks_mut::<8>();
  let (above, top_rest) = rows.top.as_chunks::<16>();
  let (below, bottom_rest) = rows.bottom.as_chunks::<16>();

  for (output, (top, bottom)) in words.iter_mut().zip(above.iter().zip(below)) {
    let t = safe_simd::vld1q_u8(top);
    let b = safe_simd::vld1q_u8(bottom);
    let contrast = vaddq_u16(vpaddlq_u8(t), vpaddlq_u8(b));
    safe_simd::vst1q_s16(output, vreinterpretq_s16_u16(vshlq_n_u16::<1>(contrast)));
  }

  let (short, tail) = rest.as_chunks_mut::<4>();
  let (top_short, top_tail) = top_rest.as_chunks::<8>();
  let (bottom_short, bottom_tail) = bottom_rest.as_chunks::<8>();

  for (output, (top, bottom)) in short.iter_mut().zip(top_short.iter().zip(bottom_short)) {
    let t = safe_simd::vld1_u8(top);
    let b = safe_simd::vld1_u8(bottom);
    let contrast = vadd_u16(vpaddl_u8(t), vpaddl_u8(b));
    safe_simd::vst1_s16(output, vreinterpret_s16_u16(vshl_n_u16::<1>(contrast)));
  }

  let top_pairs = top_tail.as_chunks::<2>().0;
  let bottom_pairs = bottom_tail.as_chunks::<2>().0;

  for (output, (&[a, b], &[c, d])) in tail.iter_mut().zip(top_pairs.iter().zip(bottom_pairs)) {
    let sum = u16::from(a)
      .wrapping_add(u16::from(b))
      .wrapping_add(u16::from(c))
      .wrapping_add(u16::from(d));
    *output = sum.wrapping_shl(1).cast_signed();
  }

  Ok(())
}

#[arcane]
fn ac_row_422_8bpc(_token: Arm64, out: &mut [i16], top: &[u8]) -> Result<(), Never> {
  let (words, rest) = out.as_chunks_mut::<8>();
  let (above, top_rest) = top.as_chunks::<16>();

  for (output, input) in words.iter_mut().zip(above) {
    let pairs = vpaddlq_u8(safe_simd::vld1q_u8(input));
    safe_simd::vst1q_s16(output, vreinterpretq_s16_u16(vshlq_n_u16::<2>(pairs)));
  }

  let (short, tail) = rest.as_chunks_mut::<4>();
  let (top_short, top_tail) = top_rest.as_chunks::<8>();

  for (output, input) in short.iter_mut().zip(top_short) {
    let pairs = vpaddl_u8(safe_simd::vld1_u8(input));
    safe_simd::vst1_s16(output, vreinterpret_s16_u16(vshl_n_u16::<2>(pairs)));
  }

  for (output, &[a, b]) in tail.iter_mut().zip(top_tail.as_chunks::<2>().0) {
    *output = u16::from(a)
      .wrapping_add(u16::from(b))
      .wrapping_shl(2)
      .cast_signed();
  }

  Ok(())
}

#[arcane]
fn ac_row_444_8bpc(_token: Arm64, out: &mut [i16], top: &[u8]) -> Result<(), Never> {
  let (words, tail) = out.as_chunks_mut::<8>();
  let (above, top_tail) = top.as_chunks::<8>();

  for (output, input) in words.iter_mut().zip(above) {
    let values = vmovl_u8(safe_simd::vld1_u8(input));
    safe_simd::vst1q_s16(output, vreinterpretq_s16_u16(vshlq_n_u16::<3>(values)));
  }

  for (output, &sample) in tail.iter_mut().zip(top_tail) {
    *output = u16::from(sample).wrapping_shl(3).cast_signed();
  }

  Ok(())
}

#[arcane]
fn ac_row_420_16bpc(
  _token: Arm64,
  out: &mut [i16],
  rows: ChromaSourceRows<'_, u16>,
) -> Result<(), Never> {
  let (words, rest) = out.as_chunks_mut::<8>();
  let (top_vectors, top_tail) = rows.top.as_chunks::<8>();
  let (bottom_vectors, bottom_tail) = rows.bottom.as_chunks::<8>();
  let (above, top_short) = top_vectors.as_chunks::<2>();
  let (below, bottom_short) = bottom_vectors.as_chunks::<2>();

  for (output, ([top0, top1], [bot0, bot1])) in words.iter_mut().zip(above.iter().zip(below)) {
    let top_pairs = vpaddq_u16(safe_simd::vld1q_u16(top0), safe_simd::vld1q_u16(top1));
    let bottom_pairs = vpaddq_u16(safe_simd::vld1q_u16(bot0), safe_simd::vld1q_u16(bot1));
    let sum = vaddq_u16(top_pairs, bottom_pairs);
    safe_simd::vst1q_s16(output, vreinterpretq_s16_u16(vshlq_n_u16::<1>(sum)));
  }

  let (short, tail) = rest.as_chunks_mut::<4>();

  for (output, (top, bottom)) in short.iter_mut().zip(top_short.iter().zip(bottom_short)) {
    let t = safe_simd::vld1q_u16(top);
    let b = safe_simd::vld1q_u16(bottom);
    let sum = vadd_u16(
      vpadd_u16(vget_low_u16(t), vget_high_u16(t)),
      vpadd_u16(vget_low_u16(b), vget_high_u16(b)),
    );
    safe_simd::vst1_s16(output, vreinterpret_s16_u16(vshl_n_u16::<1>(sum)));
  }

  let top_pairs = top_tail.as_chunks::<2>().0;
  let bottom_pairs = bottom_tail.as_chunks::<2>().0;

  for (output, (&[a, b], &[c, d])) in tail.iter_mut().zip(top_pairs.iter().zip(bottom_pairs)) {
    let sum = a.wrapping_add(b).wrapping_add(c).wrapping_add(d);
    *output = sum.wrapping_shl(1).cast_signed();
  }

  Ok(())
}

#[arcane]
fn ac_row_422_16bpc(_token: Arm64, out: &mut [i16], top: &[u16]) -> Result<(), Never> {
  let (words, rest) = out.as_chunks_mut::<8>();
  let (top_vectors, top_tail) = top.as_chunks::<8>();
  let (above, top_short) = top_vectors.as_chunks::<2>();

  for (output, [lo, hi]) in words.iter_mut().zip(above) {
    let pairs = vpaddq_u16(safe_simd::vld1q_u16(lo), safe_simd::vld1q_u16(hi));
    safe_simd::vst1q_s16(output, vreinterpretq_s16_u16(vshlq_n_u16::<2>(pairs)));
  }

  let (short, tail) = rest.as_chunks_mut::<4>();

  for (output, input) in short.iter_mut().zip(top_short) {
    let t = safe_simd::vld1q_u16(input);
    let pairs = vpadd_u16(vget_low_u16(t), vget_high_u16(t));
    safe_simd::vst1_s16(output, vreinterpret_s16_u16(vshl_n_u16::<2>(pairs)));
  }

  for (output, &[a, b]) in tail.iter_mut().zip(top_tail.as_chunks::<2>().0) {
    *output = a.wrapping_add(b).wrapping_shl(2).cast_signed();
  }

  Ok(())
}

#[arcane]
fn ac_row_444_16bpc(_token: Arm64, out: &mut [i16], top: &[u16]) -> Result<(), Never> {
  let (words, tail) = out.as_chunks_mut::<8>();
  let (above, top_tail) = top.as_chunks::<8>();

  for (output, input) in words.iter_mut().zip(above) {
    let values = safe_simd::vld1q_u16(input);
    safe_simd::vst1q_s16(output, vreinterpretq_s16_u16(vshlq_n_u16::<3>(values)));
  }

  for (output, &sample) in tail.iter_mut().zip(top_tail) {
    *output = sample.wrapping_shl(3).cast_signed();
  }

  Ok(())
}

#[arcane]
fn ac_remove_dc(_token: Arm64, ac: &mut [i16], base: i32, log2sz: u32) -> Result<(), Never> {
  let (vectors, tail) = ac.as_chunks_mut::<8>();
  let mut accumulation = vdupq_n_s32(0);

  for input in vectors.iter() {
    accumulation = vpadalq_s16(accumulation, safe_simd::vld1q_s16(input));
  }

  let mut sum = base.wrapping_add(vaddvq_s32(accumulation));

  for &sample in tail.iter() {
    sum = sum.wrapping_add(i32::from(sample));
  }

  let Ok(dc) = console_core_number_conversion::wrapped::<_, i16>(sum.wrapping_shr(log2sz));
  let offset = vdupq_n_s16(dc);

  for input in vectors {
    let values = safe_simd::vld1q_s16(&*input);
    safe_simd::vst1q_s16(input, vsubq_s16(values, offset));
  }

  for sample in tail {
    *sample = sample.wrapping_sub(dc);
  }

  Ok(())
}

pub(crate) fn cfl_ac_dispatch<BD: BitDepth>(
  ac: &mut [i16],
  y_src: &crate::src::owned_recon::ReconSrc<'_>,
  extraction: CflAcExtraction,
) -> Result<PredictionDispatch, Never> {
  use crate::src::safe_simd::pixel_access::reinterpret_slice;
  use crate::src::strided::Strided as _;

  match crate::src::ablate::is_off(crate::src::ablate::Family::IntraPred) {
    true => return Ok(PredictionDispatch::ScalarFallback),
    false => {}
  }

  let Ok(region) = CflAcRegion::from_extraction(extraction);
  let (token, region) = match (Arm64::summon(), region) {
    (Some(token), Some(region)) => (token, region),
    _ => return Ok(PredictionDispatch::ScalarFallback),
  };
  let Size { width, height } = region.size;
  let area = width.checked_mul(height);
  let available = area.and_then(|count| {
    let Ok(range) = indices_to(..count);
    ac.get_mut(range)
  });
  let coefficients = match available {
    Some(region) => region,
    None => return Ok(PredictionDispatch::ScalarFallback),
  };

  let (horizontal, vertical) = match region.layout {
    Rav1dPixelLayoutSubSampled::I420 => (1, 1),
    Rav1dPixelLayoutSubSampled::I422 => (1, 0),
    Rav1dPixelLayoutSubSampled::I444 => (0, 0),
  };
  let Ok(pixel_stride) = y_src.pixel_stride::<BD>();
  let Ok(stride) = console_core_number_conversion::fixed_offset(pixel_stride);
  let row_advance = stride.wrapping_shl(vertical);
  let Ok(source_width) = index(region.visible.width.wrapping_shl(horizontal));
  let Ok(destination_width) = index(width);
  let Ok(written_width) = index(region.visible.width);

  for (ordinal, row) in
    (0u64..region.visible.height).zip(coefficients.chunks_exact_mut(destination_width.max(1)))
  {
    let Ok(row_number) = console_core_number_conversion::wrapped::<_, i64>(ordinal);
    let Ok(displacement) =
      console_core_number_conversion::wrapped_offset(row_number.wrapping_mul(row_advance));
    let Ok(picture) = y_src.at(displacement);
    let above = match picture.slice::<BD>(source_width) {
      Ok(samples) => samples,
      Err(problem) => {
        console_core_logging::debug!("Invalid AV1 NEON chroma source: {problem:?}");
        return Ok(PredictionDispatch::ScalarFallback);
      }
    };
    let below = match region.layout {
      Rav1dPixelLayoutSubSampled::I420 => {
        let Ok(next) = picture.at(pixel_stride);
        let guard = match next.slice::<BD>(source_width) {
          Ok(samples) => samples,
          Err(problem) => {
            console_core_logging::debug!("Invalid AV1 NEON chroma source: {problem:?}");
            return Ok(PredictionDispatch::ScalarFallback);
          }
        };
        Some(guard)
      }
      Rav1dPixelLayoutSubSampled::I422 | Rav1dPixelLayoutSubSampled::I444 => None,
    };
    let (visible, padding) = match row.split_at_mut_checked(written_width) {
      Some(parts) => parts,
      None => return Ok(PredictionDispatch::ScalarFallback),
    };

    match BD::BPC {
      BPC::BPC8 => {
        let Ok(samples) = reinterpret_slice::<_, u8>(&above);
        let upper = match samples {
          Some(pixels) => pixels,
          None => return Ok(PredictionDispatch::ScalarFallback),
        };

        match (region.layout, below.as_deref()) {
          (Rav1dPixelLayoutSubSampled::I420, Some(reference)) => {
            let Ok(converted) = reinterpret_slice::<_, u8>(reference);
            let lower = match converted {
              Some(pixels) => pixels,
              None => return Ok(PredictionDispatch::ScalarFallback),
            };
            let taps = ChromaSourceRows {
              top: upper,
              bottom: lower,
            };
            let Ok(()) = ac_row_420_8bpc(token, visible, taps);
          }
          (Rav1dPixelLayoutSubSampled::I422, _) => {
            let Ok(()) = ac_row_422_8bpc(token, visible, upper);
          }
          (Rav1dPixelLayoutSubSampled::I444, _) => {
            let Ok(()) = ac_row_444_8bpc(token, visible, upper);
          }
          (Rav1dPixelLayoutSubSampled::I420, None) => {
            return Ok(PredictionDispatch::ScalarFallback);
          }
        }
      }
      BPC::BPC16 => {
        let Ok(samples) = reinterpret_slice::<_, u16>(&above);
        let upper = match samples {
          Some(pixels) => pixels,
          None => return Ok(PredictionDispatch::ScalarFallback),
        };

        match (region.layout, below.as_deref()) {
          (Rav1dPixelLayoutSubSampled::I420, Some(reference)) => {
            let Ok(converted) = reinterpret_slice::<_, u16>(reference);
            let lower = match converted {
              Some(pixels) => pixels,
              None => return Ok(PredictionDispatch::ScalarFallback),
            };
            let taps = ChromaSourceRows {
              top: upper,
              bottom: lower,
            };
            let Ok(()) = ac_row_420_16bpc(token, visible, taps);
          }
          (Rav1dPixelLayoutSubSampled::I422, _) => {
            let Ok(()) = ac_row_422_16bpc(token, visible, upper);
          }
          (Rav1dPixelLayoutSubSampled::I444, _) => {
            let Ok(()) = ac_row_444_16bpc(token, visible, upper);
          }
          (Rav1dPixelLayoutSubSampled::I420, None) => {
            return Ok(PredictionDispatch::ScalarFallback);
          }
        }
      }
    }

    match visible.last() {
      Some(&last) => padding.fill(last),
      None => return Ok(PredictionDispatch::ScalarFallback),
    }
  }

  match region.repeat_last_row(coefficients) {
    Ok(()) => {}
    Err(_) => return Ok(PredictionDispatch::ScalarFallback),
  }

  let shift = width.trailing_zeros().wrapping_add(height.trailing_zeros());
  let round = 1_i32.wrapping_shl(shift).wrapping_shr(1);
  let Ok(()) = ac_remove_dc(token, coefficients, round, shift);
  Ok(PredictionDispatch::Applied)
}

#[cfg(all(test, target_arch = "aarch64"))]
mod cfl_ac_parity {
  //! Differential parity for `cfl_ac_dispatch` against the scalar
  //! `src/ipred.rs::cfl_ac_rust` it replaces.
  //!
  //! `cfl_ac` has no `__simd_test` dual-compute hook, so this sweep plus the
  //! corpus MD5 are the only per-parameter evidence there is. The parameter
  //! space swept is the real one: all three chroma layouts, every chroma
  //! transform size AV1 defines (4..32 on a side), both padding axes over
  //! their full signalled range, both bit depths, and source pixels driven to
  //! 0 / `bitdepth_max` as well as random, because a shift that overflowed
  //! `i16` would only show at the top of the range.
  //!
  //! The live-cell assertion matters as much as the equality one: a cell the
  //! NEON dispatch declined would compare the reference against itself and
  //! prove nothing.

  use crate::include::common::bitdepth::{BitDepth, BitDepth8, BitDepth16};
  use crate::include::dav1d::picture::Rav1dPictureDataComponent;
  use crate::src::internal::CoefficientScratch;

  struct Rng(u64);

  impl Rng {
    fn in_range(&mut self, lo: i32, hi: i32) -> i32 {
      let Ok(seeded) = crate::src::safe_simd::next_fixture_sample(&mut self.0);
      ::core::primitive::i32::wrapping_add(
        lo,
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(
          (seeded
            % match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u64>(
              ::core::primitive::i32::wrapping_add(
                ::core::primitive::i32::wrapping_sub(hi, lo),
                1,
              ),
            ) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            }),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        },
      )
    }
  }

  #[must_use]
  fn run<BD: BitDepth>(
    pixels: &[BD::Pixel],
    RunArguments {
      stride,
      w_pad,
      h_pad,
      cw,
      ch,
    }: RunArguments,
    ss_hor: bool,
    ss_ver: bool,
  ) -> (Vec<i16>, Vec<i16>, bool)
  where
    BD::Pixel: Copy + Default,
  {
    let go = |simd: bool| -> (Vec<i16>, bool) {
      let Ok(mut px) = crate::src::safe_simd::aligned_plane(pixels);
      let comp = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut px, stride);
      let dst = comp.with_offset::<BD>();

      let mut ac: CoefficientScratch<i16> = [0x5A5A; 32 * 32];
      let layout = match (ss_hor, ss_ver) {
        (true, true) => Some(super::Rav1dPixelLayoutSubSampled::I420),
        (true, false) => Some(super::Rav1dPixelLayoutSubSampled::I422),
        (false, false) => Some(super::Rav1dPixelLayoutSubSampled::I444),
        (false, true) => None,
      };
      let handled = match layout {
        Some(sampling) => {
          let Ok(width) = console_core_number_conversion::wrapped::<_, i32>(cw);
          let Ok(height) = console_core_number_conversion::wrapped::<_, i32>(ch);
          let extraction = super::CflAcExtraction {
            size: super::Size { width, height },
            padding: super::Size {
              width: w_pad,
              height: h_pad,
            },
            layout: sampling,
          };
          let source = crate::src::owned_recon::ReconSrc::Pic(dst);

          match simd {
            true => {
              let Ok(outcome) = super::cfl_ac_dispatch::<BD>(&mut ac, &source, extraction);

              matches!(outcome, super::PredictionDispatch::Applied)
            }
            false => {
              let result =
                crate::src::ipred::cfl_ac_scalar_for_test::<BD>(&mut ac, &source, extraction);
              assert_eq!(result, Ok(()));
              true
            }
          }
        }
        None => false,
      };
      (ac[..cw.wrapping_mul(ch)].to_vec(), handled)
    };
    let (neon, handled) = go(true);
    let (scalar, _) = go(false);
    (neon, scalar, handled)
  }

  struct RunArguments {
    stride: usize,
    w_pad: i32,
    h_pad: i32,
    cw: usize,
    ch: usize,
  }

  fn sweep<BD: BitDepth>(bd: BD, seed: u64, what: &str)
  where
    BD::Pixel: Copy + Default + PartialEq + std::fmt::Debug,
  {
    use crate::include::common::bitdepth::AsPrimitive;
    let Ok(_lock) = crate::src::safe_simd::token_test_lock();
    let pmax = bd.bitdepth_max().as_::<i32>();
    let mut cells = 0usize;
    let mut live = 0usize;
    let mut bad: Vec<String> = Vec::new();

    for &(ss_hor, ss_ver) in &[(true, true), (true, false), (false, false)] {
      for &cw in &[4usize, 8, 16, 32] {
        for &ch in &[4usize, 8, 16, 32] {
          let ::core::result::Result::Ok(src_w) = ::console_core_number_conversion::fixed_index(
            ::core::primitive::usize::wrapping_shl(
              cw,
              <::core::primitive::u32 as ::core::convert::From<::core::primitive::bool>>::from(
                (match ::console_core_number_conversion::index::<::core::primitive::u8>(
                  <::core::primitive::u8 as ::core::convert::From<::core::primitive::bool>>::from(
                    ss_hor,
                  ),
                ) {
                  ::core::result::Result::Ok(converted_position) => converted_position,
                }) != 0,
              ),
            ),
          );
          let ::core::result::Result::Ok(src_h) = ::console_core_number_conversion::fixed_index(
            ::core::primitive::usize::wrapping_shl(
              ch,
              <::core::primitive::u32 as ::core::convert::From<::core::primitive::bool>>::from(
                (match ::console_core_number_conversion::index::<::core::primitive::u8>(
                  <::core::primitive::u8 as ::core::convert::From<::core::primitive::bool>>::from(
                    ss_ver,
                  ),
                ) {
                  ::core::result::Result::Ok(converted_position) => converted_position,
                }) != 0,
              ),
            ),
          );

          let stride = ::core::primitive::usize::wrapping_add(
            (match ::console_core_number_conversion::index(src_w) {
              ::core::result::Result::Ok(native_position) => native_position,
            }),
            16,
          )
          .next_multiple_of(64);
          let mut rng = Rng(
            seed
              ^ ::core::primitive::u64::wrapping_shl(
                match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u64>(cw) {
                  ::core::result::Result::Ok(converted_position) => converted_position,
                },
                32_u32,
              )
              ^ ::core::primitive::u64::wrapping_shl(
                match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u64>(ch) {
                  ::core::result::Result::Ok(converted_position) => converted_position,
                },
                8_u32,
              )
              ^ <u64 as ::core::convert::From<::core::primitive::bool>>::from(ss_hor),
          );

          for &fill in &[-1i32, 0, 1] {
            let pixels: Vec<BD::Pixel> =
              (0..stride.wrapping_mul(::core::primitive::usize::wrapping_add(
                (match ::console_core_number_conversion::index(src_h) {
                  ::core::result::Result::Ok(native_position) => native_position,
                }),
                2,
              )))
                .map(|_| {
                  match fill {
                    -1 => rng.in_range(0, pmax),
                    0 => 0,
                    _ => pmax,
                  }
                  .as_()
                })
                .collect();

            for &wp in &[0i32, 1, 2] {
              'inner_for_6: for &hp in &[0i32, 1, 2] {
                match {
                  ::core::primitive::bool::then_some(
                    ::core::primitive::usize::wrapping_mul(
                      (match ::console_core_number_conversion::wrapped_index(wp) {
                        ::core::result::Result::Ok(converted_position) => converted_position,
                      }),
                      4,
                    ) >= cw
                      || ::core::primitive::usize::wrapping_mul(
                        (match ::console_core_number_conversion::wrapped_index(hp) {
                          ::core::result::Result::Ok(converted_position) => converted_position,
                        }),
                        4,
                      ) >= ch,
                    (),
                  )
                } {
                  ::core::option::Option::Some(()) => {
                    continue 'inner_for_6;
                  }
                  ::core::option::Option::None => {}
                }

                let (neon, scalar, ok) = run::<BD>(
                  &pixels,
                  self::RunArguments {
                    stride,
                    w_pad: wp,
                    h_pad: hp,
                    cw,
                    ch,
                  },
                  ss_hor,
                  ss_ver,
                );
                let ::core::result::Result::Ok(()) =
                  ::console_core_number_conversion::wrapping_add_index(1, &mut cells);

                match ::core::primitive::bool::then_some(ok, ()) {
                  ::core::option::Option::Some(()) => {
                    let ::core::result::Result::Ok(()) =
                      ::console_core_number_conversion::wrapping_add_index(1, &mut live);
                  }
                  ::core::option::Option::None => {}
                }

                if let Some(i) = (0..::core::primitive::usize::wrapping_mul(cw, ch))
                  .find(|&i| neon[i] != scalar[i])
                {
                  let msg = format!(
                    "{cw}x{ch} ss=({ss_hor},{ss_ver}) pad=({wp},{hp}) \
                                         fill={fill} at {i} ({},{}) : neon={} scalar={}",
                    i % cw,
                    i / cw,
                    neon[i],
                    scalar[i]
                  );

                  match { ::core::primitive::bool::then_some(bad.len() < 4, ()) } {
                    ::core::option::Option::Some(()) => {
                      bad.push(msg);
                    }
                    ::core::option::Option::None => {}
                  }
                }
              }
            }
          }
        }
      }
    }

    assert!(cells >= 300, "{what}: only {cells} cells ran");
    assert_eq!(
      live,
      cells,
      "{what}: {} of {cells} cells did NOT take the NEON path — those \
             compared the scalar reference against itself and proved nothing",
      cells.wrapping_sub(live)
    );
    assert!(bad.is_empty(), "{what}: divergence\n  {}", bad.join("\n  "));
  }

  #[test]
  fn cfl_ac_8bpc_matches_scalar() {
    sweep(BitDepth8::new(()), 0xAC00_0001_0000_0001, "cfl_ac 8bpc");
  }

  #[test]
  fn cfl_ac_10bpc_matches_scalar() {
    sweep(BitDepth16::new(1023), 0xAC00_0002_0000_0001, "cfl_ac 10bpc");
  }

  #[test]
  fn cfl_ac_12bpc_matches_scalar() {
    sweep(BitDepth16::new(4095), 0xAC00_0003_0000_0001, "cfl_ac 12bpc");
  }
}
