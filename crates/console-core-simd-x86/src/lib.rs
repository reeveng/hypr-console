//! x86-64 SIMD kernels. Capability tokens guard the target-feature entry points.

#![cfg(target_arch = "x86_64")]

use console_core_number_conversion::{fixed_index, index};
use core::arch::x86_64::*;
use core::convert::Infallible;

mod composite;
mod filter_8tap;
mod floats;

pub use floats::FusedF32;

#[derive(Clone, Copy)]
pub struct FilterTaps {
  pub step: u64,
  pub bias: u16,
  pub maximum: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct Avx2(());

#[derive(Debug, Clone, Copy)]
pub struct Avx512(());

impl Avx2 {
  pub fn detect() -> Result<Option<Self>, Infallible> {
    Ok(std::is_x86_feature_detected!("avx2").then_some(Self(())))
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "Only AVX2 detection constructs this token, including the AVX-512 token that also requires AVX2"
    )
  )]
  pub fn source_over_rgba16(
    self,
    source: [&[u16; 16]; 4],
    pixels: &mut [[u8; 4]; 16],
  ) -> Result<(), Infallible> {
    // SAFETY: This token guarantees AVX2.
    unsafe { composite::source_over_rgba16(source, pixels) }
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "The token constructor detects the instruction set required by this filter entry point"
    )
  )]
  pub fn filter_8tap_u16<const SHIFT: i32>(
    self,
    source: &[u16],
    coefficients: &[i16; 8],
    taps: FilterTaps,
    output: &mut [u16],
  ) -> Result<u64, Infallible> {
    // SAFETY: This token guarantees AVX2.
    unsafe { filter_8tap::avx2::<SHIFT>(source, coefficients, taps, output) }
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "The token constructor detects the instruction set required by this pair kernel"
    )
  )]
  pub fn weighted_pairs<const SWAP: bool>(
    self,
    source: &[[u8; 4]],
    offsets: &[u32],
    weights: &[u16],
    output: &mut [[u16; 4]],
  ) -> Result<u64, Infallible> {
    // SAFETY: Only runtime detection can construct this token.
    unsafe { pairs_avx2::<SWAP>(source, offsets, weights, output) }
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "This token guarantees AVX2 and, for the AVX-512 path, both AVX-512F and AVX-512BW"
    )
  )]
  pub fn blend_rows<const SHIFT: i32>(
    self,
    top: &[u16],
    bottom: &[u16],
    weight: u16,
    output: &mut [u8],
  ) -> Result<u64, Infallible> {
    // SAFETY: Only runtime detection can construct this token.
    unsafe { rows_avx2::<SHIFT>(top, bottom, weight, output) }
  }
}

impl Avx512 {
  pub fn detect() -> Result<Option<Self>, Infallible> {
    Ok(
      (std::is_x86_feature_detected!("avx2")
        && std::is_x86_feature_detected!("avx512f")
        && std::is_x86_feature_detected!("avx512bw"))
      .then_some(Self(())),
    )
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "Only AVX2 detection constructs this token, including the AVX-512 token that also requires AVX2"
    )
  )]
  pub fn source_over_rgba16(
    self,
    source: [&[u16; 16]; 4],
    pixels: &mut [[u8; 4]; 16],
  ) -> Result<(), Infallible> {
    // SAFETY: This token also guarantees AVX2.
    unsafe { composite::source_over_rgba16(source, pixels) }
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "The token constructor detects the instruction set required by this filter entry point"
    )
  )]
  pub fn filter_8tap_u16<const SHIFT: i32>(
    self,
    source: &[u16],
    coefficients: &[i16; 8],
    taps: FilterTaps,
    output: &mut [u16],
  ) -> Result<u64, Infallible> {
    // SAFETY: This token guarantees AVX2, AVX-512F and AVX-512BW.
    unsafe { filter_8tap::avx512::<SHIFT>(source, coefficients, taps, output) }
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "The token constructor detects the instruction set required by this pair kernel"
    )
  )]
  pub fn weighted_pairs<const SWAP: bool>(
    self,
    source: &[[u8; 4]],
    offsets: &[u32],
    weights: &[u16],
    output: &mut [[u16; 4]],
  ) -> Result<u64, Infallible> {
    // SAFETY: Only runtime detection can construct this token.
    unsafe { pairs_avx512::<SWAP>(source, offsets, weights, output) }
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "This token guarantees AVX2 and, for the AVX-512 path, both AVX-512F and AVX-512BW"
    )
  )]
  pub fn blend_rows<const SHIFT: i32>(
    self,
    top: &[u16],
    bottom: &[u16],
    weight: u16,
    output: &mut [u8],
  ) -> Result<u64, Infallible> {
    // SAFETY: Only runtime detection can construct this token.
    unsafe {
      match (SHIFT == 12 && output.len() < 32).then_some(()) {
        Some(()) => rows_avx2::<SHIFT>(top, bottom, weight, output),
        None => rows_avx512::<SHIFT>(top, bottom, weight, output),
      }
    }
  }
}

struct PairOutsideSource;

#[inline]
fn adjacent(source: &[[u8; 4]], at: u32) -> Result<i64, PairOutsideSource> {
  let Ok(start) = index(at);
  let &[[a, b, c, d], [e, f, g, h]] = source
    .get(start..)
    .and_then(<[[u8; 4]]>::first_chunk::<2>)
    .ok_or(PairOutsideSource)?;
  Ok(i64::from_le_bytes([a, b, c, d, e, f, g, h]))
}

#[inline]
fn repeat(weight: u16) -> Result<i64, Infallible> {
  let [lo, hi] = weight.to_le_bytes();
  Ok(i64::from_le_bytes([lo, hi, lo, hi, lo, hi, lo, hi]))
}

#[target_feature(enable = "avx2")]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "The output iterator supplies writable groups of two RGBA16 pixels, exactly one unaligned 16-byte store"
  )
)]
fn pairs_avx2<const SWAP: bool>(
  source: &[[u8; 4]],
  offsets: &[u32],
  weights: &[u16],
  output: &mut [[u16; 4]],
) -> Result<u64, Infallible> {
  let swap = _mm_setr_epi8(2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15);
  let weights_order = _mm256_setr_epi64x(
    0x0100_0100_0100_0100,
    0x0100_0100_0100_0100,
    0x0302_0302_0302_0302,
    0x0302_0302_0302_0302,
  );
  let one = _mm256_set1_epi16(256);
  let mut written = 0_u64;

  for ((&[x0, x1], &[w0, w1]), target) in offsets
    .as_chunks::<2>()
    .0
    .iter()
    .zip(weights.as_chunks::<2>().0)
    .zip(output.as_chunks_mut::<2>().0)
  {
    let src = match (adjacent(source, x0), adjacent(source, x1)) {
      (Ok(first), Ok(second)) => _mm_set_epi64x(second, first),
      _ => break,
    };
    let src_2 = match SWAP.then_some(()) {
      Some(()) => _mm_shuffle_epi8(src, swap),
      None => src,
    };
    let [a, b] = w0.to_le_bytes();
    let [c, d] = w1.to_le_bytes();
    let packed = _mm_cvtsi32_si128(i32::from_le_bytes([a, b, c, d]));
    let weights_2 = _mm256_shuffle_epi8(_mm256_broadcastsi128_si256(packed), weights_order);
    let weights_3 = _mm256_blend_epi16::<0x0f>(weights_2, _mm256_sub_epi16(one, weights_2));
    let products = _mm256_mullo_epi16(_mm256_cvtepu8_epi16(src_2), weights_3);
    let sums = _mm256_add_epi16(products, _mm256_srli_si256::<8>(products));
    let packed_2 = _mm256_castsi256_si128(_mm256_permute4x64_epi64::<0xd8>(sums));

    // SAFETY: Two four-channel u16 pixels cover the entire 16-byte store.
    unsafe { _mm_storeu_si128(target.as_mut_ptr().cast(), packed_2) };

    written = written.wrapping_add(2);
  }

  Ok(written)
}

#[target_feature(enable = "avx2,avx512f,avx512bw")]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "The output iterator supplies writable groups of four RGBA16 pixels, exactly one unaligned 32-byte store"
  )
)]
fn pairs_avx512<const SWAP: bool>(
  source: &[[u8; 4]],
  offsets: &[u32],
  weights: &[u16],
  output: &mut [[u16; 4]],
) -> Result<u64, Infallible> {
  let swap = _mm256_broadcastsi128_si256(_mm_setr_epi8(
    2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15,
  ));
  let even = _mm512_set_epi64(7, 5, 3, 1, 6, 4, 2, 0);
  let Ok(first_weight) = repeat(0);
  let Ok(second_weight) = repeat(1);
  let Ok(third_weight) = repeat(2);
  let Ok(fourth_weight) = repeat(3);
  let weights_order = _mm512_set_epi64(
    fourth_weight,
    fourth_weight,
    third_weight,
    third_weight,
    second_weight,
    second_weight,
    first_weight,
    first_weight,
  );
  let one = _mm512_set1_epi16(256);
  let mut written = 0_u64;

  for ((offset_group, &[w0, w1, w2, w3]), target) in offsets
    .as_chunks::<4>()
    .0
    .iter()
    .zip(weights.as_chunks::<4>().0)
    .zip(output.as_chunks_mut::<4>().0)
  {
    let src = match offset_group.map(|offset| adjacent(source, offset)) {
      [Ok(first), Ok(second), Ok(third), Ok(fourth)] => {
        _mm256_setr_epi64x(first, second, third, fourth)
      }
      _ => break,
    };
    let src_2 = match SWAP.then_some(()) {
      Some(()) => _mm256_shuffle_epi8(src, swap),
      None => src,
    };
    let [a, b] = w0.to_le_bytes();
    let [c, d] = w1.to_le_bytes();
    let [e, f] = w2.to_le_bytes();
    let [g, h] = w3.to_le_bytes();
    let packed = _mm_cvtsi64_si128(i64::from_le_bytes([a, b, c, d, e, f, g, h]));
    let weights_2 = _mm512_permutexvar_epi16(weights_order, _mm512_broadcastq_epi64(packed));
    let weights_3 = _mm512_mask_sub_epi16(weights_2, 0x0f0f_0f0f, one, weights_2);
    let products = _mm512_mullo_epi16(_mm512_cvtepu8_epi16(src_2), weights_3);
    let sums = _mm512_add_epi16(products, _mm512_bsrli_epi128::<8>(products));
    let packed_2 = _mm512_castsi512_si256(_mm512_permutexvar_epi64(even, sums));

    // SAFETY: Four four-channel u16 pixels cover the entire 32-byte store.
    unsafe { _mm256_storeu_si256(target.as_mut_ptr().cast(), packed_2) };

    written = written.wrapping_add(4);
  }

  Ok(written)
}

#[inline]
fn pair_weights(weight: u16) -> Result<i32, Infallible> {
  Ok(i32::from(weight).wrapping_shl(16) | i32::from(256_u16.wrapping_sub(weight)))
}

#[doc = "Restores the unsigned Q8 offset that the vector loop removes before signed multiply-add. The scaled Q8 can exceed i16::MAX."]
const fn row_rounding<const SHIFT: i32>() -> Result<i32, Infallible> {
  let unsigned_offset: i32 = match SHIFT {
    16 => 32768_i32.wrapping_mul(256),
    _ => 0,
  };
  let rounding_shift = u32::from_le_bytes(SHIFT.wrapping_sub(1).to_le_bytes());
  Ok(unsigned_offset.wrapping_add(1_i32.wrapping_shl(rounding_shift)))
}

#[doc = "Masks the shifted Q4 inputs to expose their byte range before packing the lanes."]
#[target_feature(enable = "avx2")]
#[inline]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "The minimum row length and remaining channel count prove each partial-vector load and the eight-byte store fit"
  )
)]
fn rows_q4_short(
  top: &[u16],
  bottom: &[u16],
  weight: u16,
  output: &mut [u8],
) -> Result<u64, Infallible> {
  let Ok(count) = fixed_index(top.len().min(bottom.len()).min(output.len()));
  let Ok(weights_word) = pair_weights(weight);
  let Ok(rounding_word) = row_rounding::<12>();
  let weights = _mm_set1_epi32(weights_word);
  let rounding = _mm_set1_epi32(rounding_word);
  let bytes = _mm_set1_epi32(255);

  let prefix = match count {
    8.. => {
      // SAFETY: Both inputs hold at least eight u16 channels.
      let (a, b) = unsafe {
        (
          _mm_loadu_si128(top.as_ptr().cast()),
          _mm_loadu_si128(bottom.as_ptr().cast()),
        )
      };
      let lo = _mm_madd_epi16(_mm_unpacklo_epi16(a, b), weights);
      let hi = _mm_madd_epi16(_mm_unpackhi_epi16(a, b), weights);
      let lo_2 = _mm_and_si128(_mm_srli_epi32::<12>(_mm_add_epi32(lo, rounding)), bytes);
      let hi_2 = _mm_and_si128(_mm_srli_epi32::<12>(_mm_add_epi32(hi, rounding)), bytes);
      let shorts = _mm_packs_epi32(lo_2, hi_2);

      // SAFETY: The destination has room for all eight bytes.
      unsafe { _mm_storel_epi64(output.as_mut_ptr().cast(), _mm_packus_epi16(shorts, shorts)) };

      8
    }
    _ => 0,
  };

  let Ok(at) = index(prefix);
  let tail = match output.get_mut(at..).and_then(<[u8]>::first_chunk_mut::<4>) {
    Some(output_bytes) => output_bytes,
    None => return Ok(prefix),
  };

  match count.saturating_sub(prefix) {
    4.. => {
      // SAFETY: Both inputs hold four u16 channels and loadl reads eight bytes.
      let (a, b) = unsafe {
        (
          _mm_loadl_epi64(top.as_ptr().add(at).cast()),
          _mm_loadl_epi64(bottom.as_ptr().add(at).cast()),
        )
      };
      let sums = _mm_madd_epi16(_mm_unpacklo_epi16(a, b), weights);
      let words = _mm_and_si128(_mm_srli_epi32::<12>(_mm_add_epi32(sums, rounding)), bytes);
      let shorts = _mm_packs_epi32(words, words);
      *tail = _mm_cvtsi128_si32(_mm_packus_epi16(shorts, shorts)).to_le_bytes();
      Ok(prefix.wrapping_add(4))
    }
    _ => Ok(prefix),
  }
}

#[target_feature(enable = "avx2")]
#[inline]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "The chunk iterators bound each unaligned load to sixteen u16 lanes and each store to sixteen bytes"
  )
)]
fn rows_avx2<const SHIFT: i32>(
  top: &[u16],
  bottom: &[u16],
  weight: u16,
  output: &mut [u8],
) -> Result<u64, Infallible> {
  let Ok(weights_word) = pair_weights(weight);
  let Ok(rounding_word) = row_rounding::<SHIFT>();
  let weights = _mm256_set1_epi32(weights_word);
  let bias = _mm256_set1_epi16(i16::MIN);
  let rounding = _mm256_set1_epi32(rounding_word);
  let bytes = _mm256_set1_epi32(255);
  let groups = top
    .as_chunks::<16>()
    .0
    .iter()
    .zip(bottom.as_chunks::<16>().0)
    .zip(output.as_chunks_mut::<16>().0);
  let Ok(group_count) = fixed_index(groups.len());
  let written = group_count.wrapping_mul(16);

  for ((a, b), target) in groups {
    // SAFETY: Each input holds 32 bytes and unaligned loads are permitted.
    let (a_2, b_2) = unsafe {
      (
        _mm256_loadu_si256(a.as_ptr().cast()),
        _mm256_loadu_si256(b.as_ptr().cast()),
      )
    };
    let (a_3, b_3) = match SHIFT {
      16 => (_mm256_xor_si256(a_2, bias), _mm256_xor_si256(b_2, bias)),
      _ => (a_2, b_2),
    };
    let lo = _mm256_add_epi32(
      _mm256_madd_epi16(_mm256_unpacklo_epi16(a_3, b_3), weights),
      rounding,
    );
    let hi = _mm256_add_epi32(
      _mm256_madd_epi16(_mm256_unpackhi_epi16(a_3, b_3), weights),
      rounding,
    );
    let (lo_2, hi_2) = match SHIFT {
      12 => (
        _mm256_and_si256(_mm256_srli_epi32::<12>(lo), bytes),
        _mm256_and_si256(_mm256_srli_epi32::<12>(hi), bytes),
      ),
      _ => (_mm256_srai_epi32::<16>(lo), _mm256_srai_epi32::<16>(hi)),
    };
    let words = _mm256_packus_epi32(lo_2, hi_2);
    let bytes_2 = _mm_packus_epi16(
      _mm256_castsi256_si128(words),
      _mm256_extracti128_si256::<1>(words),
    );

    // SAFETY: The destination is a complete 16-byte chunk.
    unsafe { _mm_storeu_si128(target.as_mut_ptr().cast(), bytes_2) };
  }

  let Ok(at) = index(written);

  match (SHIFT, top.get(at..), bottom.get(at..), output.get_mut(at..)) {
    (12, Some(above), Some(below), Some(tail)) => {
      let Ok(remainder) = rows_q4_short(above, below, weight, tail);
      Ok(written.wrapping_add(remainder))
    }
    _ => Ok(written),
  }
}

#[target_feature(enable = "avx2,avx512f,avx512bw")]
#[inline]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "The chunk iterators bound each unaligned load to thirty-two u16 lanes and each store to thirty-two bytes"
  )
)]
fn rows_avx512<const SHIFT: i32>(
  top: &[u16],
  bottom: &[u16],
  weight: u16,
  output: &mut [u8],
) -> Result<u64, Infallible> {
  let Ok(weights_word) = pair_weights(weight);
  let Ok(rounding_word) = row_rounding::<SHIFT>();
  let weights = _mm512_set1_epi32(weights_word);
  let bias = _mm512_set1_epi16(i16::MIN);
  let rounding = _mm512_set1_epi32(rounding_word);
  let bytes = _mm512_set1_epi32(255);

  let groups = top
    .as_chunks::<32>()
    .0
    .iter()
    .zip(bottom.as_chunks::<32>().0)
    .zip(output.as_chunks_mut::<32>().0);
  let Ok(group_count) = fixed_index(groups.len());
  let written = group_count.wrapping_mul(32);

  for ((a, b), target) in groups {
    // SAFETY: Each input holds 64 bytes and unaligned loads are permitted.
    let (a_2, b_2) = unsafe {
      (
        _mm512_loadu_si512(a.as_ptr().cast()),
        _mm512_loadu_si512(b.as_ptr().cast()),
      )
    };
    let (a_3, b_3) = match SHIFT {
      16 => (_mm512_xor_si512(a_2, bias), _mm512_xor_si512(b_2, bias)),
      _ => (a_2, b_2),
    };
    let lo = _mm512_add_epi32(
      _mm512_madd_epi16(_mm512_unpacklo_epi16(a_3, b_3), weights),
      rounding,
    );
    let hi = _mm512_add_epi32(
      _mm512_madd_epi16(_mm512_unpackhi_epi16(a_3, b_3), weights),
      rounding,
    );
    let (lo_2, hi_2) = match SHIFT {
      12 => (
        _mm512_and_si512(_mm512_srli_epi32::<12>(lo), bytes),
        _mm512_and_si512(_mm512_srli_epi32::<12>(hi), bytes),
      ),
      _ => (_mm512_srai_epi32::<16>(lo), _mm512_srai_epi32::<16>(hi)),
    };
    let bytes_2 = _mm512_cvtusepi16_epi8(_mm512_packus_epi32(lo_2, hi_2));

    // SAFETY: The destination is a complete 32-byte chunk.
    unsafe { _mm256_storeu_si256(target.as_mut_ptr().cast(), bytes_2) };
  }

  let Ok(at) = index(written);

  match (SHIFT, top.get(at..), bottom.get(at..), output.get_mut(at..)) {
    (12, Some(above), Some(below), Some(tail)) => {
      let Ok(remainder) = rows_avx2::<SHIFT>(above, below, weight, tail);
      Ok(written.wrapping_add(remainder))
    }
    _ => Ok(written),
  }
}
