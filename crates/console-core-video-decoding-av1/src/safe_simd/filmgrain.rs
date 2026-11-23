//! Fixed-size AVX2 film-grain application kernels.

#![forbid(unsafe_code)]

use archmage::intrinsics::x86_64 as simd;
use archmage::{Desktop64, arcane, rite};
use console_core_never::Never;
use core::arch::x86_64::*;

pub(crate) struct GrainByteBlock<'a, const N: usize> {
  pub source: &'a [u8; N],
  pub destination: &'a mut [u8; N],
  pub grain: &'a [i8; N],
  pub scaling: [i16; N],
  pub minimum: u8,
  pub maximum: u8,
  pub shift: u8,
}

struct ByteNoise<'a> {
  source: &'a [u8; 16],
  samples: &'a [i8; 16],
  scaling: &'a [i16; 16],
  shift: u8,
}

#[rite]
fn apply_byte_noise(_token: Desktop64, input: ByteNoise<'_>) -> Result<__m256i, Never> {
  let pixels = _mm256_cvtepu8_epi16(simd::_mm_loadu_si128(input.source));
  let grain = _mm256_cvtepi8_epi16(simd::_mm_loadu_si128(input.samples));
  let scaling = simd::_mm256_loadu_si256(input.scaling);
  let product = _mm256_mullo_epi16(grain, scaling);
  let power = u32::from(15_u8.wrapping_sub(input.shift));
  let multiplier = _mm256_set1_epi16(1_i16.wrapping_shl(power));
  let noise = _mm256_mulhrs_epi16(product, multiplier);
  Ok(_mm256_add_epi16(pixels, noise))
}

#[arcane]
pub(crate) fn apply_grain32(
  token: Desktop64,
  input: GrainByteBlock<'_, 32>,
) -> Result<(), Never> {
  let [first_source, second_source]: &[[u8; 16]; 2] = zerocopy::transmute_ref!(input.source);
  let [first_grain, second_grain]: &[[i8; 16]; 2] = zerocopy::transmute_ref!(input.grain);
  let [first_scaling, second_scaling]: &[[i16; 16]; 2] = zerocopy::transmute_ref!(&input.scaling);
  let Ok(first) = apply_byte_noise(
    token,
    ByteNoise {
      source: first_source,
      samples: first_grain,
      scaling: first_scaling,
      shift: input.shift,
    },
  );
  let Ok(second) = apply_byte_noise(
    token,
    ByteNoise {
      source: second_source,
      samples: second_grain,
      scaling: second_scaling,
      shift: input.shift,
    },
  );
  let packed = _mm256_permute4x64_epi64::<0xD8>(_mm256_packus_epi16(first, second));
  let minimum = _mm256_set1_epi8(input.minimum.cast_signed());
  let maximum = _mm256_set1_epi8(input.maximum.cast_signed());
  let clipped = _mm256_min_epu8(_mm256_max_epu8(packed, minimum), maximum);
  simd::_mm256_storeu_si256(input.destination, clipped);
  Ok(())
}

#[arcane]
pub(crate) fn apply_grain16(
  token: Desktop64,
  input: GrainByteBlock<'_, 16>,
) -> Result<(), Never> {
  let Ok(added) = apply_byte_noise(
    token,
    ByteNoise {
      source: input.source,
      samples: input.grain,
      scaling: &input.scaling,
      shift: input.shift,
    },
  );
  let low = _mm256_castsi256_si128(added);
  let high = _mm256_extracti128_si256::<1>(added);
  let packed = _mm_packus_epi16(low, high);
  let minimum = _mm_set1_epi8(input.minimum.cast_signed());
  let maximum = _mm_set1_epi8(input.maximum.cast_signed());
  let clipped = _mm_min_epu8(_mm_max_epu8(packed, minimum), maximum);
  simd::_mm_storeu_si128(input.destination, clipped);
  Ok(())
}

pub(crate) struct GrainWordBlock<'a> {
  pub source: &'a [u16; 16],
  pub destination: &'a mut [u16; 16],
  pub noise: [i16; 16],
  pub minimum: i16,
  pub maximum: i16,
}

#[arcane]
pub(crate) fn apply_grain_words(
  _token: Desktop64,
  input: GrainWordBlock<'_>,
) -> Result<(), Never> {
  let pixels = simd::_mm256_loadu_si256(input.source);
  let noise = simd::_mm256_loadu_si256(&input.noise);
  let result = _mm256_add_epi16(pixels, noise);
  let minimum = _mm256_set1_epi16(input.minimum);
  let maximum = _mm256_set1_epi16(input.maximum);
  let clipped = _mm256_min_epi16(_mm256_max_epi16(result, minimum), maximum);
  simd::_mm256_storeu_si256(input.destination, clipped);
  Ok(())
}
