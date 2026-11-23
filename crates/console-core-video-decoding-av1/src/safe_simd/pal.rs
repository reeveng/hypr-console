//! AVX2 packing for palette indices. Each byte stores one pair of four-bit
//! indices. The visible rows finish first, then the block repeats its edge
//! index and its final row to fill the allocated rectangle.

#![forbid(unsafe_code)]

use archmage::intrinsics::x86_64::{
  _mm_loadu_si128, _mm_storeu_si128, _mm256_loadu_si256, _mm256_storeu_si256,
};
use archmage::{Desktop64, arcane};
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::index;
use core::arch::x86_64::*;

use super::partial_simd;
use crate::src::error::{Rav1dError, Rav1dResult};

use crate::src::pal::{PaletteBlock, contiguous_layout};

pub(crate) enum PaletteDispatch {
  Packed,
  ScalarFallback,
}

pub(crate) fn pal_idx_finish_dispatch(
  dst: &mut [u8],
  src: &[u8],
  shape: PaletteBlock,
) -> Result<PaletteDispatch, Never> {
  let Ok(avx2) = crate::src::cpu::summon_avx2();

  match avx2 {
    Some(token) => match pack_palette(token, dst, src, shape) {
      Ok(()) => Ok(PaletteDispatch::Packed),
      Err(_layout) => Ok(PaletteDispatch::ScalarFallback),
    },
    None => Ok(PaletteDispatch::ScalarFallback),
  }
}

fn pack_palette(
  token: Desktop64,
  dst: &mut [u8],
  src: &[u8],
  shape: PaletteBlock,
) -> Rav1dResult {
  let PaletteBlock { block, visible } = shape;

  match (visible.height <= block.height && visible.width <= block.width).then_some(()) {
    Some(()) => {}
    None => return Err(Rav1dError::EINVAL),
  }

  let packed_layout = contiguous_layout(Size {
    width: block.width.wrapping_div(2),
    height: block.height,
  })?;
  let source_layout = contiguous_layout(Size {
    width: block.width,
    height: visible.height,
  })?;
  let mut dst_rows = packed_layout
    .rows_mut(dst)
    .map_err(|_layout| Rav1dError::EINVAL)?;
  let src_rows = source_layout
    .rows(src)
    .map_err(|_layout| Rav1dError::EINVAL)?;
  let mut last_packed_row = None;

  for (source_row, packed_row) in src_rows.zip(dst_rows.by_ref()) {
    pack_visible_row(token, packed_row, source_row, visible.width)?;
    last_packed_row = Some(packed_row);
  }

  let last = last_packed_row.ok_or(Rav1dError::EINVAL)?;

  for padding_row in dst_rows {
    for (target, sample) in padding_row.iter_mut().zip(last.iter()) {
      *target = *sample;
    }
  }

  Ok(())
}

fn pack_visible_row(
  token: Desktop64,
  dst: &mut [u8],
  src: &[u8],
  visible_width: u64,
) -> Rav1dResult {
  let Ok(pairs) = index(visible_width.wrapping_div(2));
  let Ok(width) = index(visible_width);
  let (packed, padding) = dst.split_at_mut_checked(pairs).ok_or(Rav1dError::EINVAL)?;
  let (indices, source_padding) = src.split_at_checked(width).ok_or(Rav1dError::EINVAL)?;

  pack_row(token, PaletteRow { packed, indices })?;

  match padding.is_empty().then_some(()) {
    Some(()) => {}
    None => {
      let edge = source_padding.first().ok_or(Rav1dError::EINVAL)?;
      padding.fill(edge.wrapping_mul(0x11));
    }
  }

  Ok(())
}

struct PaletteRow<'a> {
  packed: &'a mut [u8],
  indices: &'a [u8],
}

impl<'a> PaletteRow<'a> {
  #[inline]
  fn blocks<const OUTPUT: usize, const INPUT: usize>(
    &mut self,
  ) -> Result<impl Iterator<Item = (&'a mut [u8; OUTPUT], &'a [u8; INPUT])>, Never> {
    let (outputs, output_tail) = core::mem::take(&mut self.packed).as_chunks_mut::<OUTPUT>();
    let (inputs, input_tail) = self.indices.as_chunks::<INPUT>();
    self.packed = output_tail;
    self.indices = input_tail;

    Ok(outputs.iter_mut().zip(inputs))
  }
}

#[arcane]
fn pack_row(token: Desktop64, row: PaletteRow<'_>) -> Rav1dResult {
  let mut remaining = row;
  let coeff = _mm256_set1_epi16(0x1001);
  let Ok(wide) = remaining.blocks::<32, 64>();

  for (output, input) in wide {
    let (left, right) = match input.as_chunks::<32>() {
      ([left, right], []) => (left, right),
      (_chunks, _remainder) => return Err(Rav1dError::EINVAL),
    };
    let a = _mm256_maddubs_epi16(_mm256_loadu_si256(left), coeff);
    let b = _mm256_maddubs_epi16(_mm256_loadu_si256(right), coeff);
    let packed = _mm256_packus_epi16(a, b);
    _mm256_storeu_si256(output, _mm256_permute4x64_epi64::<0xD8>(packed));
  }

  let Ok(half) = remaining.blocks::<16, 32>();

  for (output, input) in half {
    let a = _mm256_maddubs_epi16(_mm256_loadu_si256(input), coeff);
    let packed = _mm256_packus_epi16(a, _mm256_setzero_si256());
    let ordered = _mm256_permute4x64_epi64::<0xD8>(packed);
    _mm_storeu_si128(output, _mm256_castsi256_si128(ordered));
  }

  let Ok(()) = pack_remainder(token, remaining);

  Ok(())
}

#[arcane]
fn pack_remainder(_token: Desktop64, row: PaletteRow<'_>) -> Result<(), Never> {
  let mut remaining = row;
  let coeff = _mm_set1_epi16(0x1001);
  let Ok(eights) = remaining.blocks::<8, 16>();

  for (output, input) in eights {
    let a = _mm_maddubs_epi16(_mm_loadu_si128(input), coeff);
    let packed = _mm_packus_epi16(a, _mm_setzero_si128());
    let Ok(()) = partial_simd::mm_storel_epi64(output, packed);
  }

  let Ok(fours) = remaining.blocks::<4, 8>();

  for (output, input) in fours {
    let Ok(values) = partial_simd::mm_loadl_epi64(input);
    let a = _mm_maddubs_epi16(values, coeff);
    let packed = _mm_packus_epi16(a, _mm_setzero_si128());
    *output = _mm_cvtsi128_si32(packed).to_le_bytes();
  }

  let Ok(pairs) = remaining.blocks::<1, 2>();

  for ([output], [low, high]) in pairs {
    *output = low | high.wrapping_shl(4);
  }

  Ok(())
}
