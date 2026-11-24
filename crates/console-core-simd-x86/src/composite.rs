use core::arch::x86_64::*;
use core::convert::Infallible;

#[target_feature(enable = "avx2")]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "Each source load and each destination load or store uses an array chunk holding an entire sixteen-byte vector"
  )
)]
pub(super) fn source_over_rgba16(
  [r, g, b, a]: [&[u16; 16]; 4],
  pixels: &mut [[u8; 4]; 16],
) -> Result<(), Infallible> {
  let alpha_bytes = _mm256_setr_epi64x(
    0x0706_0706_0706_0706,
    0x0f0e_0f0e_0f0e_0f0e,
    0x0706_0706_0706_0706,
    0x0f0e_0f0e_0f0e_0f0e,
  );
  let byte = _mm256_set1_epi16(255);

  let [red, green, blue, alpha] = [r, g, b, a].map(|plane| plane.as_chunks::<8>().0);

  for ((chunk, (red_chunk, green_chunk)), (blue_chunk, alpha_chunk)) in pixels
    .as_chunks_mut::<8>()
    .0
    .iter_mut()
    .zip(red.iter().zip(green))
    .zip(blue.iter().zip(alpha))
  {
    let [red_lanes, green_lanes, blue_lanes, alpha_lanes] =
      [red_chunk, green_chunk, blue_chunk, alpha_chunk].map(|lanes| {
        // SAFETY: Each source chunk holds eight u16 lanes.
        unsafe { _mm_loadu_si128(lanes.as_ptr().cast()) }
      });
    let rg_lo = _mm_unpacklo_epi16(red_lanes, green_lanes);
    let ba_lo = _mm_unpacklo_epi16(blue_lanes, alpha_lanes);
    let rg_hi = _mm_unpackhi_epi16(red_lanes, green_lanes);
    let ba_hi = _mm_unpackhi_epi16(blue_lanes, alpha_lanes);
    let src_lo = _mm256_set_m128i(
      _mm_unpackhi_epi32(rg_lo, ba_lo),
      _mm_unpacklo_epi32(rg_lo, ba_lo),
    );
    let src_hi = _mm256_set_m128i(
      _mm_unpackhi_epi32(rg_hi, ba_hi),
      _mm_unpacklo_epi32(rg_hi, ba_hi),
    );

    let [first, second] = match chunk.as_chunks_mut::<4>().0 {
      [first, second] => [first, second],
      _unexpected_shape => return Ok(()),
    };

    for (src, target) in [(src_lo, first), (src_hi, second)] {
      // SAFETY: The group holds sixteen packed bytes.
      let dst = unsafe { _mm_loadu_si128(target.as_ptr().cast()) };
      let dst_2 = _mm256_cvtepu8_epi16(dst);
      let inverse = _mm256_sub_epi16(byte, _mm256_shuffle_epi8(src, alpha_bytes));
      let scaled =
        _mm256_srli_epi16::<8>(_mm256_add_epi16(_mm256_mullo_epi16(dst_2, inverse), byte));
      let sum = _mm256_and_si256(_mm256_add_epi16(src, scaled), byte);
      let packed = _mm_packus_epi16(
        _mm256_castsi256_si128(sum),
        _mm256_extracti128_si256::<1>(sum),
      );

      // SAFETY: The destination group holds sixteen packed bytes.
      unsafe { _mm_storeu_si128(target.as_mut_ptr().cast(), packed) };
    }
  }

  Ok(())
}
