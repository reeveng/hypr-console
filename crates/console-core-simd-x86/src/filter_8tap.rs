use super::FilterTaps;
use console_core_number_conversion::{fixed_index, index};
use core::arch::x86_64::*;
use core::cmp::Ordering;
use core::convert::Infallible;

fn pairs(coefficients: &[i16; 8]) -> Result<[i32; 4], Infallible> {
  let &[a, b, c, d, e, f, g, h] = coefficients;
  let pack =
    |low: i16, high: i16| i32::from(low.cast_unsigned()) | i32::from(high).wrapping_shl(16);
  Ok([pack(a, b), pack(c, d), pack(e, f), pack(g, h)])
}

#[target_feature(enable = "avx2")]
#[inline]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "AVX2 loads and stores use raw pointers within complete tap spans and output lanes proven by the slice lengths"
  )
)]
pub(super) fn avx2<const SHIFT: i32>(
  source: &[u16],
  coefficients: &[i16; 8],
  taps: FilterTaps,
  output: &mut [u16],
) -> Result<u64, Infallible> {
  let Ok(source_samples) = fixed_index(source.len());
  let Ok(output_samples) = fixed_index(output.len());
  let available = source_samples
    .saturating_sub(taps.step.saturating_mul(7))
    .min(output_samples);
  let vector_count = available & !15;
  let Ok(packed_coefficients) = pairs(coefficients);
  let [c0, c1, c2, c3] = packed_coefficients.map(|pair| _mm256_set1_epi32(pair));
  let bias = _mm256_set1_epi32(i32::from(taps.bias));
  let limit = _mm256_set1_epi32(i32::from(taps.maximum));

  for (column, target) in (0..vector_count)
    .step_by(16)
    .zip(output.chunks_exact_mut(16))
  {
    let [a, b, c, d, e, f, g, h] = core::array::from_fn(|tap| {
      let Ok(tap_position) = fixed_index(tap);
      let Ok(offset) = index(column.wrapping_add(tap_position.wrapping_mul(taps.step)));

      // SAFETY: The tap span and sixteen samples fit at every column.
      unsafe { _mm256_loadu_si256(source.as_ptr().add(offset).cast()) }
    });
    let Ok(low_sum) = sum_avx2(
      [
        _mm256_unpacklo_epi16(a, b),
        _mm256_unpacklo_epi16(c, d),
        _mm256_unpacklo_epi16(e, f),
        _mm256_unpacklo_epi16(g, h),
      ],
      [c0, c1, c2, c3],
    );
    let Ok(high_sum) = sum_avx2(
      [
        _mm256_unpackhi_epi16(a, b),
        _mm256_unpackhi_epi16(c, d),
        _mm256_unpackhi_epi16(e, f),
        _mm256_unpackhi_epi16(g, h),
      ],
      [c0, c1, c2, c3],
    );
    let lo = _mm256_min_epi32(
      _mm256_srai_epi32::<SHIFT>(_mm256_add_epi32(low_sum, bias)),
      limit,
    );
    let hi = _mm256_min_epi32(
      _mm256_srai_epi32::<SHIFT>(_mm256_add_epi32(high_sum, bias)),
      limit,
    );

    // SAFETY: The destination holds sixteen samples.
    unsafe { _mm256_storeu_si256(target.as_mut_ptr().cast(), _mm256_packus_epi32(lo, hi)) };
  }

  let prefix = match available.wrapping_sub(vector_count).cmp(&8) {
    Ordering::Equal | Ordering::Greater => {
      let [a, b, c, d, e, f, g, h] = core::array::from_fn(|tap| {
        let Ok(tap_position) = fixed_index(tap);
        let Ok(offset) = index(vector_count.wrapping_add(tap_position.wrapping_mul(taps.step)));

        // SAFETY: At least eight samples remain in every tap.
        unsafe { _mm_loadu_si128(source.as_ptr().add(offset).cast()) }
      });
      let zip = |first, second| {
        _mm256_set_m128i(
          _mm_unpackhi_epi16(first, second),
          _mm_unpacklo_epi16(first, second),
        )
      };
      let Ok(products) = sum_avx2(
        [zip(a, b), zip(c, d), zip(e, f), zip(g, h)],
        [c0, c1, c2, c3],
      );
      let sum = _mm256_min_epi32(
        _mm256_srai_epi32::<SHIFT>(_mm256_add_epi32(products, bias)),
        limit,
      );
      let samples = _mm_packus_epi32(
        _mm256_castsi256_si128(sum),
        _mm256_extracti128_si256::<1>(sum),
      );

      let Ok(destination) = index(vector_count);

      // SAFETY: The destination has eight remaining samples.
      unsafe { _mm_storeu_si128(output.as_mut_ptr().add(destination).cast(), samples) };

      vector_count.wrapping_add(8)
    }
    Ordering::Less => vector_count,
  };

  let written = match available.wrapping_sub(prefix).cmp(&4) {
    Ordering::Equal | Ordering::Greater => {
      let [a, b, c, d, e, f, g, h] = core::array::from_fn(|tap| {
        let Ok(tap_position) = fixed_index(tap);
        let Ok(offset) = index(prefix.wrapping_add(tap_position.wrapping_mul(taps.step)));

        // SAFETY: At least four samples remain in every tap.
        unsafe { _mm_loadl_epi64(source.as_ptr().add(offset).cast()) }
      });
      let ab = _mm_madd_epi16(_mm_unpacklo_epi16(a, b), _mm256_castsi256_si128(c0));
      let cd = _mm_madd_epi16(_mm_unpacklo_epi16(c, d), _mm256_castsi256_si128(c1));
      let ef = _mm_madd_epi16(_mm_unpacklo_epi16(e, f), _mm256_castsi256_si128(c2));
      let gh = _mm_madd_epi16(_mm_unpacklo_epi16(g, h), _mm256_castsi256_si128(c3));
      let products = _mm_add_epi32(_mm_add_epi32(ab, cd), _mm_add_epi32(ef, gh));
      let sum = _mm_srai_epi32::<SHIFT>(_mm_add_epi32(products, _mm256_castsi256_si128(bias)));
      let clipped = _mm_min_epi32(sum, _mm256_castsi256_si128(limit));

      let Ok(destination) = index(prefix);

      // SAFETY: The destination has four remaining samples.
      unsafe {
        _mm_storel_epi64(
          output.as_mut_ptr().add(destination).cast(),
          _mm_packus_epi32(clipped, clipped),
        )
      };

      prefix.wrapping_add(4)
    }
    Ordering::Less => prefix,
  };

  Ok(written)
}

#[target_feature(enable = "avx2")]
#[inline]
#[must_use]
fn sum_avx2(
  [a, b, c, d]: [__m256i; 4],
  [w0, w1, w2, w3]: [__m256i; 4],
) -> Result<__m256i, Infallible> {
  let lo = _mm256_add_epi32(_mm256_madd_epi16(a, w0), _mm256_madd_epi16(b, w1));
  let hi = _mm256_add_epi32(_mm256_madd_epi16(c, w2), _mm256_madd_epi16(d, w3));
  Ok(_mm256_add_epi32(lo, hi))
}

#[target_feature(enable = "avx2,avx512f,avx512bw")]
#[inline]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "AVX-512 reads full tap spans and stores complete output chunks proven by the slice lengths"
  )
)]
pub(super) fn avx512<const SHIFT: i32>(
  source: &[u16],
  coefficients: &[i16; 8],
  taps: FilterTaps,
  output: &mut [u16],
) -> Result<u64, Infallible> {
  let Ok(source_samples) = fixed_index(source.len());
  let Ok(output_samples) = fixed_index(output.len());
  let available = source_samples
    .saturating_sub(taps.step.saturating_mul(7))
    .min(output_samples);
  let count = available & !31;
  let Ok(packed_coefficients) = pairs(coefficients);
  let [c0, c1, c2, c3] = packed_coefficients.map(|pair| _mm512_set1_epi32(pair));
  let bias = _mm512_set1_epi32(i32::from(taps.bias));
  let limit = _mm512_set1_epi32(i32::from(taps.maximum));
  let shift = _mm_cvtsi32_si128(SHIFT);

  for (column, target) in (0..count).step_by(32).zip(output.chunks_exact_mut(32)) {
    let [a, b, c, d, e, f, g, h] = core::array::from_fn(|tap| {
      let Ok(tap_position) = fixed_index(tap);
      let Ok(offset) = index(column.wrapping_add(tap_position.wrapping_mul(taps.step)));

      // SAFETY: The tap span and thirty-two samples fit at every column.
      unsafe { _mm512_loadu_si512(source.as_ptr().add(offset).cast()) }
    });
    let Ok(low_products) = sum_avx512(
      [
        _mm512_unpacklo_epi16(a, b),
        _mm512_unpacklo_epi16(c, d),
        _mm512_unpacklo_epi16(e, f),
        _mm512_unpacklo_epi16(g, h),
      ],
      [c0, c1, c2, c3],
    );
    let Ok(high_products) = sum_avx512(
      [
        _mm512_unpackhi_epi16(a, b),
        _mm512_unpackhi_epi16(c, d),
        _mm512_unpackhi_epi16(e, f),
        _mm512_unpackhi_epi16(g, h),
      ],
      [c0, c1, c2, c3],
    );
    let lo = _mm512_min_epi32(
      _mm512_sra_epi32(_mm512_add_epi32(low_products, bias), shift),
      limit,
    );
    let hi = _mm512_min_epi32(
      _mm512_sra_epi32(_mm512_add_epi32(high_products, bias), shift),
      limit,
    );
    let samples = _mm512_packus_epi32(lo, hi);

    // SAFETY: The destination holds thirty-two samples.
    unsafe { _mm512_storeu_si512(target.as_mut_ptr().cast(), samples) };
  }

  let Ok(tail) = index(count);

  match (
    available.wrapping_sub(count),
    source.get(tail..),
    output.get_mut(tail..),
  ) {
    (4.., Some(readable), Some(target)) => {
      let Ok(remainder) = avx2::<SHIFT>(readable, coefficients, taps, target);
      Ok(count.wrapping_add(remainder))
    }
    _ => Ok(count),
  }
}

#[target_feature(enable = "avx512f,avx512bw")]
#[inline]
#[must_use]
fn sum_avx512(
  [a, b, c, d]: [__m512i; 4],
  [w0, w1, w2, w3]: [__m512i; 4],
) -> Result<__m512i, Infallible> {
  let lo = _mm512_add_epi32(_mm512_madd_epi16(a, w0), _mm512_madd_epi16(b, w1));
  let hi = _mm512_add_epi32(_mm512_madd_epi16(c, w2), _mm512_madd_epi16(d, w3));
  Ok(_mm512_add_epi32(lo, hi))
}
