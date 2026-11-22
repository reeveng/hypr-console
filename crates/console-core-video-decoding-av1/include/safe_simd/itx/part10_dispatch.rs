use crate::include::common::bitdepth::BPC;
use crate::src::levels::ADST_ADST;
use crate::src::levels::ADST_DCT;
use crate::src::levels::ADST_FLIPADST;
use crate::src::levels::DCT_ADST;
use crate::src::levels::DCT_DCT;
use crate::src::levels::DCT_FLIPADST;
use crate::src::levels::FLIPADST_ADST;
use crate::src::levels::FLIPADST_DCT;
use crate::src::levels::FLIPADST_FLIPADST;
use crate::src::levels::H_ADST;
use crate::src::levels::H_DCT;
use crate::src::levels::H_FLIPADST;
use crate::src::levels::IDTX;
use crate::src::levels::TxfmType;
use crate::src::levels::V_ADST;
use crate::src::levels::V_DCT;
use crate::src::levels::V_FLIPADST;
use crate::src::levels::WHT_WHT;

#[cfg(target_arch = "x86_64")]
#[inline]
fn dc_only_compute(
  coeff: i32,
  size: crate::src::levels::TxfmSize,
) -> Result<i32, console_core_never::Never> {
  use crate::src::levels::TxfmSize::*;

  let Ok(scale) = RectangularScale::for_transform(size);
  let scaled = match scale {
    RectangularScale::Apply => coeff.wrapping_mul(181).wrapping_add(128).wrapping_shr(8),
    RectangularScale::Skip => coeff,
  };
  let shift = match size {
    S4x4 | R4x8 | R8x4 => 0,
    R4x16 | S8x8 | R8x16 | R16x4 | R16x8 | R32x16 | R16x32 | R32x64 | R64x32 => 1,
    R8x32 | S16x16 | R16x64 | R32x8 | S32x32 | R64x16 | S64x64 => 2,
  };
  let rounding = 1_i32.wrapping_shl(shift).wrapping_shr(1);
  let row = scaled.wrapping_mul(181).wrapping_add(128).wrapping_shr(8);
  let column = row.wrapping_add(rounding).wrapping_shr(shift);
  let residual = column.wrapping_mul(181).wrapping_add(128).wrapping_add(2048).wrapping_shr(12);

  Ok(residual)
}

#[cfg(target_arch = "x86_64")]
fn dc_output_layout(
  size: crate::src::levels::TxfmSize,
  first: u64,
  stride: i64,
) -> Rav1dResult<console_core_pixel_buffers::RowLayout> {
  let Ok(dimensions) = size.extent();
  let Ok(extent) = dimensions.map(u64::from);

  console_core_pixel_buffers::RowLayout::new(extent, first, stride).map_err(|_| Rav1dError::EINVAL)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn dc_only_add_8bpc(
  _token: Desktop64,
  dst: &mut [u8],
  layout: &console_core_pixel_buffers::RowLayout,
  delta: i32,
) -> Rav1dResult {
  let rows = layout.rows_mut(dst).map_err(|_| Rav1dError::EINVAL)?;
  let Ok(delta_i16) = console_core_number_conversion::wrapped::<_, i16>(
    delta.clamp(i32::from(i16::MIN), i32::from(i16::MAX)),
  );
  let wide_delta = _mm256_set1_epi16(delta_i16);
  let wide_zero = _mm256_setzero_si256();
  let narrow_delta = _mm_set1_epi16(delta_i16);
  let narrow_zero = _mm_setzero_si128();

  for row in rows {
    match row.len() {
      4 => {
        for sample in row {
          let value = i32::from(*sample).wrapping_add(delta).clamp(0, 255);
          let Ok(pixel) = console_core_number_conversion::wrapped::<_, u8>(value);
          *sample = pixel;
        }
      }
      8 => {
        let lane = row.first_chunk_mut::<8>().ok_or(Rav1dError::EINVAL)?;
        let d = _mm_set_epi64x(0, i64::from_le_bytes(*lane));
        let unpacked = _mm_unpacklo_epi8(d, narrow_zero);
        let sum = _mm_add_epi16(unpacked, narrow_delta);
        *lane = _mm_cvtsi128_si64(_mm_packus_epi16(sum, sum)).to_le_bytes();
      }
      16 => {
        let lane = row.first_chunk_mut::<16>().ok_or(Rav1dError::EINVAL)?;
        let d = loadu_128!(&*lane);
        let lo = _mm_add_epi16(_mm_unpacklo_epi8(d, narrow_zero), narrow_delta);
        let hi = _mm_add_epi16(_mm_unpackhi_epi8(d, narrow_zero), narrow_delta);
        storeu_128!(lane, _mm_packus_epi16(lo, hi));
      }
      32 | 64 => {
        for lane in row.as_chunks_mut::<32>().0 {
          let d = loadu_256!(&*lane);
          let lo = _mm256_add_epi16(_mm256_unpacklo_epi8(d, wide_zero), wide_delta);
          let hi = _mm256_add_epi16(_mm256_unpackhi_epi8(d, wide_zero), wide_delta);
          storeu_256!(lane, _mm256_packus_epi16(lo, hi));
        }
      }
      _ => return Err(Rav1dError::EINVAL),
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
struct HighDepthDc {
  delta: i32,
  maximum: i32,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn dc_only_add_16bpc(
  _token: Desktop64,
  dst: &mut [u16],
  layout: &console_core_pixel_buffers::RowLayout,
  residual: HighDepthDc,
) -> Rav1dResult {
  let rows = layout.rows_mut(dst).map_err(|_| Rav1dError::EINVAL)?;
  let wide_delta = _mm256_set1_epi32(residual.delta);
  let wide_max = _mm256_set1_epi32(residual.maximum);
  let wide_zero = _mm256_setzero_si256();
  let narrow_delta = _mm_set1_epi32(residual.delta);
  let narrow_max = _mm_set1_epi32(residual.maximum);
  let narrow_zero = _mm_setzero_si128();

  for row in rows {
    match row.len() {
      4 => {
        for sample in row {
          let value = i32::from(*sample).wrapping_add(residual.delta).clamp(0, residual.maximum);
          let Ok(pixel) = console_core_number_conversion::wrapped::<_, u16>(value);
          *sample = pixel;
        }
      }
      8 => {
        let lane = row.first_chunk_mut::<8>().ok_or(Rav1dError::EINVAL)?;
        let d = loadu_128!(&*lane);
        let sum_lo = _mm_add_epi32(_mm_unpacklo_epi16(d, narrow_zero), narrow_delta);
        let sum_hi = _mm_add_epi32(_mm_unpackhi_epi16(d, narrow_zero), narrow_delta);
        let lo = _mm_max_epi32(_mm_min_epi32(sum_lo, narrow_max), narrow_zero);
        let hi = _mm_max_epi32(_mm_min_epi32(sum_hi, narrow_max), narrow_zero);
        storeu_128!(lane, _mm_packus_epi32(lo, hi));
      }
      16 | 32 | 64 => {
        for lane in row.as_chunks_mut::<16>().0 {
          let d = loadu_256!(&*lane);
          let lo = _mm256_unpacklo_epi16(d, wide_zero);
          let hi = _mm256_unpackhi_epi16(d, wide_zero);
          let first = _mm256_permute2x128_si256::<0x20>(lo, hi);
          let second = _mm256_permute2x128_si256::<0x31>(lo, hi);
          let lo_sum = _mm256_add_epi32(first, wide_delta);
          let hi_sum = _mm256_add_epi32(second, wide_delta);
          let clipped_lo = _mm256_max_epi32(_mm256_min_epi32(lo_sum, wide_max), wide_zero);
          let clipped_hi = _mm256_max_epi32(_mm256_min_epi32(hi_sum, wide_max), wide_zero);
          let packed = _mm256_packus_epi32(clipped_lo, clipped_hi);
          storeu_256!(lane, _mm256_permute4x64_epi64::<0xd8>(packed));
        }
      }
      _ => return Err(Rav1dError::EINVAL),
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
use crate::src::levels::TxfmSize;

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
struct SimdTransform {
  size: TxfmSize,
  kind: TxfmType,
  end_of_block: i32,
}

#[cfg(target_arch = "x86_64")]
struct TransformDestination {
  first: u64,
  byte_stride: i64,
}

#[cfg(target_arch = "x86_64")]
type DirectKernel<P, C> = fn(
  Desktop64, &mut [P], u64, &mut [C], i32,
);

#[cfg(target_arch = "x86_64")]
type CheckedKernel<P, C> = fn(
  Desktop64, &mut [P], u64, &mut [C], i32,
) -> Rav1dResult;

#[cfg(target_arch = "x86_64")]
enum SimdKernel<P, C> {
  Direct(DirectKernel<P, C>),
  Checked(CheckedKernel<P, C>),
}

#[cfg(target_arch = "x86_64")]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TransformDispatch {
  Applied,
  Fallback,
}

#[cfg(target_arch = "x86_64")]
fn apply_simd_kernel<P, C>(
  token: Desktop64,
  kernel: SimdKernel<P, C>,
  dst: &mut [P],
  window: TransformDestination,
  coeff: &mut [C],
  bitdepth_max: i32,
) -> Rav1dResult<TransformDispatch> {
  let stride = u64::try_from(window.byte_stride).map_err(|_| Rav1dError::EINVAL)?;
  let Ok(native_first) = console_core_number_conversion::index(window.first);
  let pixels = dst.get_mut(native_first..).ok_or(Rav1dError::EINVAL)?;

  match kernel {
    SimdKernel::Direct(transform) => {
      transform(token, pixels, stride, coeff, bitdepth_max);
    }
    SimdKernel::Checked(transform) => {
      transform(token, pixels, stride, coeff, bitdepth_max)?;
    }
  }

  Ok(TransformDispatch::Applied)
}

#[cfg(target_arch = "x86_64")]
fn select_8bpc_kernel(
  plan: SimdTransform,
) -> Result<Option<SimdKernel<u8, i16>>, console_core_never::Never> {
  use TxfmSize::*;

  let kernel = match (plan.size, plan.kind) {
    (S4x4, WHT_WHT) => SimdKernel::Checked(inv_txfm_add_wht_wht_4x4_8bpc_avx2_inner),
    (S4x4, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_4x4_8bpc_avx2_inner),
    (R4x8, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_4x8_8bpc_avx2_inner),
    (R8x4, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_8x4_8bpc_avx2_inner),
    (R4x16, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_4x16_8bpc_avx2_inner),
    (R16x4, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_16x4_8bpc_avx2_inner),
    (S8x8, DCT_DCT) => SimdKernel::Direct(inv_txfm_add_dct_dct_8x8_8bpc_avx2_inner),
    (R8x16, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_8x16_8bpc_avx2_inner),
    (R16x8, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_16x8_8bpc_avx2_inner),
    (R8x32, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_8x32_8bpc_avx2_inner),
    (R32x8, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_32x8_8bpc_avx2_inner),
    (S16x16, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_16x16_8bpc_avx2_inner),
    (R16x32, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_16x32_8bpc_avx2_inner),
    (R32x16, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_32x16_8bpc_avx2_inner),
    (R16x64, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_16x64_8bpc_avx2_inner),
    (R64x16, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_64x16_8bpc_avx2_inner),
    (S32x32, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_32x32_8bpc_avx2_inner),
    (R32x64, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_32x64_8bpc_avx2_inner),
    (R64x32, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_64x32_8bpc_avx2_inner),
    (S64x64, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_64x64_8bpc_avx2_inner),
    (S4x4, IDTX) => SimdKernel::Direct(inv_identity_add_4x4_8bpc_avx2),
    (S8x8, IDTX) => SimdKernel::Direct(inv_identity_add_8x8_8bpc_avx2),
    (S16x16, IDTX) => SimdKernel::Direct(inv_identity_add_16x16_8bpc_avx2),
    (R8x32, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_8x32_8bpc_avx2_inner),
    (R32x8, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_32x8_8bpc_avx2_inner),
    (R16x32, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_16x32_8bpc_avx2_inner),
    (R32x16, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_32x16_8bpc_avx2_inner),
    (S32x32, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_32x32_8bpc_avx2_inner),
    (S16x16, ADST_DCT) => SimdKernel::Checked(inv_txfm_add_dct_adst_16x16_8bpc_avx2_inner),
    (S16x16, DCT_ADST) => SimdKernel::Checked(inv_txfm_add_adst_dct_16x16_8bpc_avx2_inner),
    (S16x16, ADST_ADST) => SimdKernel::Checked(inv_txfm_add_adst_adst_16x16_8bpc_avx2_inner),
    (S16x16, FLIPADST_DCT) => SimdKernel::Checked(inv_txfm_add_dct_flipadst_16x16_8bpc_avx2_inner),
    (S16x16, DCT_FLIPADST) => SimdKernel::Checked(inv_txfm_add_flipadst_dct_16x16_8bpc_avx2_inner),
    (S16x16, FLIPADST_FLIPADST) => SimdKernel::Checked(inv_txfm_add_flipadst_flipadst_16x16_8bpc_avx2_inner),
    (S16x16, ADST_FLIPADST) => SimdKernel::Checked(inv_txfm_add_flipadst_adst_16x16_8bpc_avx2_inner),
    (S16x16, FLIPADST_ADST) => SimdKernel::Checked(inv_txfm_add_adst_flipadst_16x16_8bpc_avx2_inner),
    (S16x16, H_DCT) => SimdKernel::Checked(inv_txfm_add_dct_identity_16x16_8bpc_avx2_inner),
    (S16x16, V_DCT) => SimdKernel::Checked(inv_txfm_add_identity_dct_16x16_8bpc_avx2_inner),
    (S16x16, H_ADST) => SimdKernel::Checked(inv_txfm_add_adst_identity_16x16_8bpc_avx2_inner),
    (S16x16, V_ADST) => SimdKernel::Checked(inv_txfm_add_identity_adst_16x16_8bpc_avx2_inner),
    (S16x16, H_FLIPADST) => SimdKernel::Checked(inv_txfm_add_flipadst_identity_16x16_8bpc_avx2_inner),
    (S16x16, V_FLIPADST) => SimdKernel::Checked(inv_txfm_add_identity_flipadst_16x16_8bpc_avx2_inner),
    _ => return Ok(None),
  };
  Ok(Some(kernel))
}

#[cfg(target_arch = "x86_64")]
fn select_16bpc_kernel(
  plan: SimdTransform,
) -> Result<Option<SimdKernel<u16, i32>>, console_core_never::Never> {
  use TxfmSize::*;

  let kernel = match (plan.size, plan.kind) {
    (S4x4, WHT_WHT) => SimdKernel::Checked(inv_txfm_add_wht_wht_4x4_16bpc_avx2_inner),
    (S4x4, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_4x4_16bpc_avx2_inner),
    (R4x8, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_4x8_16bpc_avx2_inner),
    (R8x4, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_8x4_16bpc_avx2_inner),
    (R4x16, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_4x16_16bpc_avx2_inner),
    (R16x4, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_16x4_16bpc_avx2_inner),
    (S8x8, DCT_DCT) => SimdKernel::Direct(inv_txfm_add_dct_dct_8x8_16bpc_avx2_inner),
    (R8x16, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_8x16_16bpc_avx2_inner),
    (R16x8, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_16x8_16bpc_avx2_inner),
    (R8x32, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_8x32_16bpc_avx2_inner),
    (R32x8, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_32x8_16bpc_avx2_inner),
    (S16x16, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_16x16_16bpc_avx2_inner),
    (R16x32, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_16x32_16bpc_avx2_inner),
    (R32x16, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_32x16_16bpc_avx2_inner),
    (R16x64, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_16x64_16bpc_avx2_inner),
    (R64x16, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_64x16_16bpc_avx2_inner),
    (S32x32, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_32x32_16bpc_avx2_inner),
    (R32x64, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_32x64_16bpc_avx2_inner),
    (R64x32, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_64x32_16bpc_avx2_inner),
    (S64x64, DCT_DCT) => SimdKernel::Checked(inv_txfm_add_dct_dct_64x64_16bpc_avx2_inner),
    (S4x4, IDTX) => SimdKernel::Checked(inv_identity_add_4x4_16bpc_avx2),
    (R4x8, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_4x8_16bpc_avx2_inner),
    (R8x4, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_8x4_16bpc_avx2_inner),
    (R4x16, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_4x16_16bpc_avx2_inner),
    (R16x4, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_16x4_16bpc_avx2_inner),
    (S8x8, IDTX) => SimdKernel::Checked(inv_identity_add_8x8_16bpc_avx2),
    (R8x16, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_8x16_16bpc_avx2_inner),
    (R16x8, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_16x8_16bpc_avx2_inner),
    (R8x32, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_8x32_16bpc_avx2_inner),
    (R32x8, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_32x8_16bpc_avx2_inner),
    (S16x16, IDTX) => SimdKernel::Checked(inv_identity_add_16x16_16bpc_avx2),
    (R16x32, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_16x32_16bpc_avx2_inner),
    (R32x16, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_32x16_16bpc_avx2_inner),
    (S32x32, IDTX) => SimdKernel::Checked(inv_txfm_add_identity_identity_32x32_16bpc_avx2_inner),
    _ => return Ok(None),
  };
  Ok(Some(kernel))
}

#[cfg(target_arch = "x86_64")]
fn mixed_transform_passes(
  kind: TxfmType,
) -> Result<Option<Mixed8Passes>, console_core_never::Never> {
  use Mixed8Pass::*;

  let [rows, columns] = match kind {
    ADST_DCT => [Dct, Adst],
    DCT_ADST => [Adst, Dct],
    ADST_ADST => [Adst, Adst],
    FLIPADST_DCT => [Dct, FlipAdst],
    DCT_FLIPADST => [FlipAdst, Dct],
    FLIPADST_FLIPADST => [FlipAdst, FlipAdst],
    ADST_FLIPADST => [FlipAdst, Adst],
    FLIPADST_ADST => [Adst, FlipAdst],
    H_DCT => [Dct, Identity],
    V_DCT => [Identity, Dct],
    H_ADST => [Adst, Identity],
    V_ADST => [Identity, Adst],
    H_FLIPADST => [FlipAdst, Identity],
    V_FLIPADST => [Identity, FlipAdst],
    _ => return Ok(None),
  };
  Ok(Some(Mixed8Passes { rows, columns }))
}

#[cfg(target_arch = "x86_64")]
impl Mixed8Pass {
  fn scalar_4x4(self) -> Result<ScalarItx4, console_core_never::Never> {
    Ok(match self {
      Self::Dct => dct4_1d_scalar,
      Self::Adst => adst4_1d_scalar,
      Self::FlipAdst => flipadst4_1d_scalar,
      Self::Identity => identity4_1d_scalar,
    })
  }
}

#[cfg(target_arch = "x86_64")]
fn itxfm_dispatch_8bpc(
  token: Desktop64,
  plan: SimdTransform,
  dst: &mut [u8],
  window: TransformDestination,
  coeff: &mut [i16],
  bitdepth_max: i32,
) -> Rav1dResult<TransformDispatch> {
  match (plan.end_of_block, plan.kind) {
    (0, DCT_DCT) => {
      let coefficient = coeff.first_mut().ok_or(Rav1dError::EINVAL)?;
      let layout = dc_output_layout(plan.size, window.first, window.byte_stride)?;
      let Ok(delta) = dc_only_compute(i32::from(*coefficient), plan.size);

      dc_only_add_8bpc(token, dst, &layout, delta)?;
      *coefficient = 0;
      return Ok(TransformDispatch::Applied);
    }
    _ => {}
  }

  let Ok(regular) = select_8bpc_kernel(plan);

  match regular {
    Some(kernel) => return apply_simd_kernel(token, kernel, dst, window, coeff, bitdepth_max),
    None => {}
  }

  let Ok(mixed) = mixed_transform_passes(plan.kind);

  let passes = match mixed {
    Some(passes) => passes,
    None => return Ok(TransformDispatch::Fallback),
  };

  match plan.size {
    TxfmSize::S4x4 => {
      let Ok(rows) = passes.rows.scalar_4x4();
      let Ok(columns) = passes.columns.scalar_4x4();
      let scalar_passes = ScalarItx4Passes { rows, columns };

      inv_txfm_add_4x4_8bpc(dst, window.first, window.byte_stride, coeff, scalar_passes)?;
    }
    TxfmSize::S8x8 => {
      let stride = u64::try_from(window.byte_stride).map_err(|_| Rav1dError::EINVAL)?;
      let Ok(first) = console_core_number_conversion::index(window.first);
      let pixels = dst.get_mut(first..).ok_or(Rav1dError::EINVAL)?;

      inv_txfm_add_mixed_8x8_8bpc(token, pixels, stride, coeff, passes)?;
    }
    TxfmSize::S16x16 | TxfmSize::S32x32 | TxfmSize::S64x64 | TxfmSize::R4x8
    | TxfmSize::R8x4 | TxfmSize::R8x16 | TxfmSize::R16x8 | TxfmSize::R16x32
    | TxfmSize::R32x16 | TxfmSize::R32x64 | TxfmSize::R64x32 | TxfmSize::R4x16
    | TxfmSize::R16x4 | TxfmSize::R8x32 | TxfmSize::R32x8 | TxfmSize::R16x64
    | TxfmSize::R64x16 => return Ok(TransformDispatch::Fallback),
  }

  Ok(TransformDispatch::Applied)
}

#[cfg(target_arch = "x86_64")]
fn itxfm_dispatch_16bpc(
  token: Desktop64,
  plan: SimdTransform,
  dst: &mut [u16],
  window: TransformDestination,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult<TransformDispatch> {
  match (plan.end_of_block, plan.kind) {
    (0, DCT_DCT) => {
      let coefficient = coeff.first_mut().ok_or(Rav1dError::EINVAL)?;
      let layout = dc_output_layout(plan.size, window.first, window.byte_stride.wrapping_div(2))?;
      let Ok(delta) = dc_only_compute(*coefficient, plan.size);
      let residual = HighDepthDc { delta, maximum: bitdepth_max };

      dc_only_add_16bpc(token, dst, &layout, residual)?;
      *coefficient = 0;
      return Ok(TransformDispatch::Applied);
    }
    _ => {}
  }

  let Ok(regular) = select_16bpc_kernel(plan);

  match regular {
    Some(kernel) => apply_simd_kernel(token, kernel, dst, window, coeff, bitdepth_max),
    None => Ok(TransformDispatch::Fallback),
  }
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn itxfm_add_dispatch<BD: BitDepth>(
  size: TxfmSize,
  kind: TxfmType,
  dst: &mut crate::src::owned_recon::ReconDst<'_>,
  coeff: &mut [BD::Coef],
  eob: i32,
  bd: BD,
) -> Result<TransformDispatch, console_core_never::Never> {
  let Ok(avx2) = crate::src::cpu::summon_avx2();

  let token = match avx2 {
    Some(token) => token,
    None => return Ok(TransformDispatch::Fallback),
  };

  let Ok(extent) = size.extent();
  let Ok(width) = console_core_number_conversion::index(extent.width);
  let Ok(height) = console_core_number_conversion::index(extent.height);
  let plan = SimdTransform { size, kind, end_of_block: eob };
  let bitdepth_max = bd.into_c();
  let coefficient_bytes = zerocopy::IntoBytes::as_mut_bytes(coeff);
  let Ok(filtered) = dst.with_block_mut::<BD, _, _>(width, height, coefficient_bytes, |pixels, offset, step, coefficients_bytes| {
    let Ok(first) = console_core_number_conversion::fixed_index(offset);
    let Ok(byte_stride) = console_core_number_conversion::fixed_offset(step);

    match BD::BPC {
      BPC::BPC8 => {
        let window = TransformDestination { first, byte_stride };
        let coefficients = zerocopy::FromBytes::mut_from_bytes(coefficients_bytes)
          .map_err(|_| Rav1dError::EINVAL)?;
        itxfm_dispatch_8bpc(token, plan, pixels, window, coefficients, bitdepth_max)
      }
      BPC::BPC16 => {
        let [first_remainder, stride_remainder] = [first.wrapping_rem(2), byte_stride.unsigned_abs().wrapping_rem(2)];

        match (first_remainder, stride_remainder) {
          (0, 0) => {}
          _ => return Err(Rav1dError::EINVAL),
        }

        let window = TransformDestination { first: first.wrapping_div(2), byte_stride };
        let words = zerocopy::FromBytes::mut_from_bytes(pixels).map_err(|_| Rav1dError::EINVAL)?;
        let coefficients = zerocopy::FromBytes::mut_from_bytes(coefficients_bytes)
          .map_err(|_| Rav1dError::EINVAL)?;
        itxfm_dispatch_16bpc(token, plan, words, window, coefficients, bitdepth_max)
      }
    }
  });

  Ok(match filtered {
    Ok(outcome) => outcome,
    Err(problem) => {
      console_core_logging::debug!("Invalid AV1 SIMD inverse-transform region: {problem:?}");
      TransformDispatch::Fallback
    }
  })
}
