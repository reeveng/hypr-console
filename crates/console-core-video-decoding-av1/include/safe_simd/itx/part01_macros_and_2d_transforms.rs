use archmage::{Desktop64, Server64, arcane, rite};
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

use crate::include::common::bitdepth::BitDepth;
#[cfg(target_arch = "x86_64")]
use console_core_never::Never;
use crate::include::common::intops::iclip;
use crate::src::safe_simd::pixel_access::Flex;
use crate::src::safe_simd::pixel_access::{
    loadi32, loadi64, loadu_128, loadu_256, loadu_512, storei32, storei64, storeu_128, storeu_256,
    storeu_512,
};

#[cfg(all(test, target_arch = "x86_64"))]
#[rite]
fn multiply_coefficient_pairs(
    _token: Desktop64,
    pairs: __m256i,
    coefficients: [i16; 2],
) -> Result<__m256i, Never> {
    let [first, second] = coefficients;
    let [first_low, first_high] = first.to_le_bytes();
    let [second_low, second_high] = second.to_le_bytes();
    let packed = i32::from_le_bytes([first_low, first_high, second_low, second_high]);
    let weighted = _mm256_madd_epi16(pairs, _mm256_set1_epi32(packed));
    Ok(_mm256_srai_epi32::<12>(_mm256_add_epi32(weighted, _mm256_set1_epi32(2048))))
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn transpose_8x8_i32(_token: Desktop64, columns: [__m256i; 8]) -> Result<[__m256i; 8], Never> {
    let [a, b, c, d, e, f, g, h] = columns;
    let ab_low = _mm256_unpacklo_epi32(a, b);
    let ab_high = _mm256_unpackhi_epi32(a, b);
    let cd_low = _mm256_unpacklo_epi32(c, d);
    let cd_high = _mm256_unpackhi_epi32(c, d);
    let ef_low = _mm256_unpacklo_epi32(e, f);
    let ef_high = _mm256_unpackhi_epi32(e, f);
    let gh_low = _mm256_unpacklo_epi32(g, h);
    let gh_high = _mm256_unpackhi_epi32(g, h);

    let left_04 = _mm256_unpacklo_epi64(ab_low, cd_low);
    let left_15 = _mm256_unpackhi_epi64(ab_low, cd_low);
    let left_26 = _mm256_unpacklo_epi64(ab_high, cd_high);
    let left_37 = _mm256_unpackhi_epi64(ab_high, cd_high);
    let right_04 = _mm256_unpacklo_epi64(ef_low, gh_low);
    let right_15 = _mm256_unpackhi_epi64(ef_low, gh_low);
    let right_26 = _mm256_unpacklo_epi64(ef_high, gh_high);
    let right_37 = _mm256_unpackhi_epi64(ef_high, gh_high);

    Ok([
        _mm256_permute2x128_si256::<0x20>(left_04, right_04),
        _mm256_permute2x128_si256::<0x20>(left_15, right_15),
        _mm256_permute2x128_si256::<0x20>(left_26, right_26),
        _mm256_permute2x128_si256::<0x20>(left_37, right_37),
        _mm256_permute2x128_si256::<0x31>(left_04, right_04),
        _mm256_permute2x128_si256::<0x31>(left_15, right_15),
        _mm256_permute2x128_si256::<0x31>(left_26, right_26),
        _mm256_permute2x128_si256::<0x31>(left_37, right_37),
    ])
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn coefficient_columns4<C: Copy + Into<i32>>(
    _token: Desktop64,
    coefficients: &[C; 16],
) -> Result<[__m128i; 4], Never> {
    let [
        a0, a1, a2, a3,
        b0, b1, b2, b3,
        c0, c1, c2, c3,
        d0, d1, d2, d3,
    ] = coefficients.map(Into::into);

    Ok([
        _mm_setr_epi32(a0, a1, a2, a3),
        _mm_setr_epi32(b0, b1, b2, b3),
        _mm_setr_epi32(c0, c1, c2, c3),
        _mm_setr_epi32(d0, d1, d2, d3),
    ])
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn transpose_4x4_i32(_token: Desktop64, vectors: [__m128i; 4]) -> Result<[__m128i; 4], Never> {
    let [a, b, c, d] = vectors;
    let ab_low = _mm_unpacklo_epi32(a, b);
    let ab_high = _mm_unpackhi_epi32(a, b);
    let cd_low = _mm_unpacklo_epi32(c, d);
    let cd_high = _mm_unpackhi_epi32(c, d);

    Ok([
        _mm_unpacklo_epi64(ab_low, cd_low),
        _mm_unpackhi_epi64(ab_low, cd_low),
        _mm_unpacklo_epi64(ab_high, cd_high),
        _mm_unpackhi_epi64(ab_high, cd_high),
    ])
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn dct4_pair(_token: Desktop64, rows: __m256i, bounds: CoefficientBounds) -> Result<__m256i, Never> {
    let in0 = _mm256_shuffle_epi32::<0x00>(rows);
    let in1 = _mm256_shuffle_epi32::<0x55>(rows);
    let in2 = _mm256_shuffle_epi32::<0xaa>(rows);
    let in3 = _mm256_shuffle_epi32::<0xff>(rows);

    let sqrt2 = _mm256_set1_epi32(181);
    let round8 = _mm256_set1_epi32(128);
    let c1567 = _mm256_set1_epi32(1567);
    let c_minus312 = _mm256_set1_epi32(-312);
    let round12 = _mm256_set1_epi32(2048);

    let even_sum = _mm256_mullo_epi32(_mm256_add_epi32(in0, in2), sqrt2);
    let even_difference = _mm256_mullo_epi32(_mm256_sub_epi32(in0, in2), sqrt2);
    let even0 = _mm256_srai_epi32::<8>(_mm256_add_epi32(even_sum, round8));
    let even1 = _mm256_srai_epi32::<8>(_mm256_add_epi32(even_difference, round8));

    let odd_difference = _mm256_sub_epi32(
        _mm256_mullo_epi32(in1, c1567),
        _mm256_mullo_epi32(in3, c_minus312),
    );
    let odd_sum = _mm256_add_epi32(
        _mm256_mullo_epi32(in1, c_minus312),
        _mm256_mullo_epi32(in3, c1567),
    );
    let odd0 = _mm256_sub_epi32(_mm256_srai_epi32::<12>(_mm256_add_epi32(odd_difference, round12)), in3);
    let odd1 = _mm256_add_epi32(_mm256_srai_epi32::<12>(_mm256_add_epi32(odd_sum, round12)), in1);

    let minimum = _mm256_set1_epi32(bounds.lower);
    let maximum = _mm256_set1_epi32(bounds.upper);
    let clip = |value| _mm256_max_epi32(_mm256_min_epi32(value, maximum), minimum);
    let out0 = clip(_mm256_add_epi32(even0, odd1));
    let out1 = clip(_mm256_add_epi32(even1, odd0));
    let out2 = clip(_mm256_sub_epi32(even1, odd0));
    let out3 = clip(_mm256_sub_epi32(even0, odd1));

    let first = _mm256_unpacklo_epi32(out0, out1);
    let second = _mm256_unpacklo_epi32(out2, out3);
    Ok(_mm256_unpacklo_epi64(first, second))
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn dct4_rows(token: Desktop64, rows: [__m128i; 4], bounds: CoefficientBounds) -> Result<[__m128i; 4], Never> {
    let [a, b, c, d] = rows;
    let Ok(first) = dct4_pair(token, _mm256_set_m128i(b, a), bounds);
    let Ok(second) = dct4_pair(token, _mm256_set_m128i(d, c), bounds);
    Ok([
        _mm256_castsi256_si128(first),
        _mm256_extracti128_si256::<1>(first),
        _mm256_castsi256_si128(second),
        _mm256_extracti128_si256::<1>(second),
    ])
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn dct4_square(
    token: Desktop64,
    columns: [__m128i; 4],
    bounds: TransformAxisBounds,
) -> Result<[__m128i; 4], Never> {
    let Ok(rows) = transpose_4x4_i32(token, columns);
    let Ok(horizontal) = dct4_rows(token, rows, bounds.rows);
    let low = _mm_set1_epi32(bounds.columns.lower);
    let high = _mm_set1_epi32(bounds.columns.upper);
    let Ok(columns) = transpose_4x4_i32(token, horizontal);
    let clipped = columns.map(|lane| _mm_max_epi32(_mm_min_epi32(lane, high), low));
    let Ok(vertical) = dct4_rows(token, clipped, bounds.columns);
    let rounding = _mm_set1_epi32(8);
    let Ok(transformed) = transpose_4x4_i32(token, vertical);
    Ok(transformed.map(|lane| _mm_srai_epi32::<4>(_mm_add_epi32(lane, rounding))))
}

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
struct TransformAxisBounds {
    rows: CoefficientBounds,
    columns: CoefficientBounds,
}

#[cfg(target_arch = "x86_64")]
impl TransformAxisBounds {
    fn narrow(bitdepth_max: i32) -> Result<Self, Never> {
        match bitdepth_max {
            255 => {
                let bounds = CoefficientBounds { lower: i32::from(i16::MIN), upper: i32::from(i16::MAX) };
                Ok(Self { rows: bounds, columns: bounds })
            }
            _ => Self::wide(bitdepth_max),
        }
    }

    fn wide(bitdepth_max: i32) -> Result<Self, Never> {
        let row_min = (!bitdepth_max).wrapping_shl(7);
        let column_min = (!bitdepth_max).wrapping_shl(5);
        Ok(Self {
            rows: CoefficientBounds { lower: row_min, upper: !row_min },
            columns: CoefficientBounds { lower: column_min, upper: !column_min },
        })
    }
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn wht4_lanes(_token: Desktop64, input: [__m128i; 4]) -> Result<[__m128i; 4], Never> {
    let [a, b, c, d] = input;
    let first_sum = _mm_add_epi32(a, b);
    let second_difference = _mm_sub_epi32(c, d);
    let middle = _mm_srai_epi32::<1>(_mm_sub_epi32(first_sum, second_difference));
    let lower = _mm_sub_epi32(middle, d);
    let upper = _mm_sub_epi32(middle, b);
    Ok([
        _mm_sub_epi32(first_sum, lower),
        lower,
        upper,
        _mm_add_epi32(second_difference, upper),
    ])
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn wht4_square(token: Desktop64, columns: [__m128i; 4]) -> Result<[__m128i; 4], Never> {
    let scaled = columns.map(|column| _mm_srai_epi32::<2>(column));
    let Ok(horizontal) = wht4_lanes(token, scaled);
    let Ok(columns) = transpose_4x4_i32(token, horizontal);
    wht4_lanes(token, columns)
}

#[cfg(target_arch = "x86_64")]
fn write_four_pixel_rows<P, const HEIGHT: console_core_api_compatibility::NativeSize>(
    dst: &mut [P],
    pixel_stride: u64,
    residuals: [__m128i; HEIGHT],
    mut add_pixels: impl FnMut(&mut [P; 4], __m128i),
) -> Rav1dResult {
    let stride = i64::try_from(pixel_stride).map_err(|_| Rav1dError::ERANGE)?;
    let Ok(height) = console_core_number_conversion::fixed_index(HEIGHT);
    let size = console_core_geometry::Size { width: 4, height };
    let layout = console_core_pixel_buffers::RowLayout::new(size, 0, stride)
        .map_err(|_| Rav1dError::EINVAL)?;
    let rows = layout.rows_mut(dst).map_err(|_| Rav1dError::EINVAL)?;

    for (row, residual) in rows.zip(residuals) {
        let pixels = row.first_chunk_mut::<4>().ok_or(Rav1dError::EINVAL)?;
        add_pixels(pixels, residual);
    }

    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn add_four_pixels_8bpc(
    _token: Desktop64,
    pixels: &mut [u8; 4],
    residual: __m128i,
    bitdepth_max: i32,
) -> Result<(), Never> {
    let Ok(maximum) = console_core_number_conversion::wrapped::<_, i16>(bitdepth_max);
    let destination = _mm_cvtepu8_epi32(_mm_cvtsi32_si128(i32::from_le_bytes(*pixels)));
    let sum = _mm_add_epi32(destination, residual);
    let words = _mm_packs_epi32(sum, sum);
    let clipped = _mm_max_epi16(_mm_setzero_si128(), _mm_min_epi16(words, _mm_set1_epi16(maximum)));
    *pixels = _mm_cvtsi128_si32(_mm_packus_epi16(clipped, clipped)).to_le_bytes();
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn add_four_pixels_16bpc(
    _token: Desktop64,
    pixels: &mut [u16; 4],
    residual: __m128i,
    bitdepth_max: i32,
) -> Result<(), Never> {
    let [a, b, c, d] = pixels.map(i32::from);
    let destination = _mm_setr_epi32(a, b, c, d);
    let sum = _mm_add_epi32(destination, residual);
    let clipped = _mm_max_epi32(_mm_setzero_si128(), _mm_min_epi32(sum, _mm_set1_epi32(bitdepth_max)));
    let packed = _mm_packus_epi32(clipped, clipped);
    let [a_low, a_high, b_low, b_high, c_low, c_high, d_low, d_high] =
        _mm_cvtsi128_si64(packed).to_le_bytes();
    *pixels = [
        u16::from_le_bytes([a_low, a_high]),
        u16::from_le_bytes([b_low, b_high]),
        u16::from_le_bytes([c_low, c_high]),
        u16::from_le_bytes([d_low, d_high]),
    ];
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_4x4_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let block = coeff.first_chunk_mut::<16>().ok_or(Rav1dError::EINVAL)?;
    let Ok(columns) = coefficient_columns4(token, block);
    let Ok(bounds) = TransformAxisBounds::narrow(bitdepth_max);
    let Ok(residuals) = dct4_square(token, columns, bounds);
    let stride = dst_stride;
    write_four_pixel_rows(dst, stride, residuals, |pixels, values| {
        let Ok(()) = add_four_pixels_8bpc(token, pixels, values, bitdepth_max);
    })?;
    *block = [0; 16];
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_4x4_16bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u16],
    byte_stride: u64,
    coeff: &mut [i32],
    bitdepth_max: i32,
) -> Rav1dResult {
    let block = coeff.first_chunk_mut::<16>().ok_or(Rav1dError::EINVAL)?;
    let Ok(columns) = coefficient_columns4(token, block);
    let Ok(bounds) = TransformAxisBounds::wide(bitdepth_max);
    let Ok(residuals) = dct4_square(token, columns, bounds);
    let stride = byte_stride;
    write_four_pixel_rows(dst, ::core::primitive::u64::wrapping_shr(stride, 1_u32), residuals, |pixels, values| {
        let Ok(()) = add_four_pixels_16bpc(token, pixels, values, bitdepth_max);
    })?;
    *block = [0; 16];
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_wht_wht_4x4_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let block = coeff.first_chunk_mut::<16>().ok_or(Rav1dError::EINVAL)?;
    let Ok(columns) = coefficient_columns4(token, block);
    let Ok(residuals) = wht4_square(token, columns);
    let stride = dst_stride;
    write_four_pixel_rows(dst, stride, residuals, |pixels, values| {
        let Ok(()) = add_four_pixels_8bpc(token, pixels, values, bitdepth_max);
    })?;
    *block = [0; 16];
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_wht_wht_4x4_16bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u16],
    byte_stride: u64,
    coeff: &mut [i32],
    bitdepth_max: i32,
) -> Rav1dResult {
    let block = coeff.first_chunk_mut::<16>().ok_or(Rav1dError::EINVAL)?;
    let Ok(columns) = coefficient_columns4(token, block);
    let Ok(residuals) = wht4_square(token, columns);
    let stride = byte_stride;
    write_four_pixel_rows(dst, ::core::primitive::u64::wrapping_shr(stride, 1_u32), residuals, |pixels, values| {
        let Ok(()) = add_four_pixels_16bpc(token, pixels, values, bitdepth_max);
    })?;
    *block = [0; 16];
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
pub fn inv_identity_add_4x4_8bpc_avx2(
  _token: Desktop64,
  dst: &mut [u8],
  dst_stride: u64,
  coeff: &mut [i16],
  bitdepth_max: i32,
) {
    let Ok(pitch) = console_core_number_conversion::index(dst_stride);
    let mut dst_2 = dst.flex_mut();
    let mut coeff_2 = coeff.flex_mut();


    let zero = _mm_setzero_si128();
    let max_val = _mm_set1_epi16(match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(bitdepth_max) { ::core::result::Result::Ok(converted_position) => converted_position });
    let identity4 = |v: i32| -> i32 { i32::wrapping_add(v, ::core::primitive::i32::wrapping_shr(i32::wrapping_add(i32::wrapping_mul(v, 1697), 2048), 12_u32)) };

  for y in match ::console_core_number_conversion::indices::<::core::primitive::u16>(0..4) {
    ::core::result::Result::Ok(y) => y,
  } {
        let dst_off = (Wrapping(y) * Wrapping(pitch)).0;


        let d = loadi32!(&dst_2[dst_off..(Wrapping(dst_off) + Wrapping(4)).0]);
        let d16 = _mm_unpacklo_epi8(d, zero);

    let c0 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff_2[y]);
    let c1 =
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff_2[y.wrapping_add(4)]);
    let c2 =
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff_2[y.wrapping_add(8)]);
    let c3 =
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff_2[y.wrapping_add(12)]);



        let col_clip_min =  match ::core::primitive::bool::then_some(bitdepth_max == 255, ()) { ::core::option::Option::Some(()) => {
            <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN)
        }, ::core::option::Option::None => {
            ::core::primitive::i32::wrapping_shl(!bitdepth_max, 5_u32)
        } };
        let col_clip_max = !col_clip_min;
        let scale = |v: i32| -> i32 { identity4(identity4(v).clamp(col_clip_min, col_clip_max)) };

        let r0 = ::core::primitive::i32::wrapping_shr(i32::wrapping_add(scale(c0), 8), 4_u32);
        let r1 = ::core::primitive::i32::wrapping_shr(i32::wrapping_add(scale(c1), 8), 4_u32);
        let r2 = ::core::primitive::i32::wrapping_shr(i32::wrapping_add(scale(c2), 8), 4_u32);
        let r3 = ::core::primitive::i32::wrapping_shr(i32::wrapping_add(scale(c3), 8), 4_u32);


        let result = _mm_set_epi32(r3, r2, r1, r0);
        let d32 = _mm_cvtepi16_epi32(d16);
        let sum = _mm_add_epi32(d32, result);
        let sum16 = _mm_packs_epi32(sum, sum);
        let clamped = _mm_max_epi16(_mm_min_epi16(sum16, max_val), zero);
        let packed = _mm_packus_epi16(clamped, clamped);

        storei32!(&mut dst_2[dst_off..(Wrapping(dst_off) + Wrapping(4)).0], packed);
    }


    coeff_2[..16].fill(0);
}


#[cfg(target_arch = "x86_64")]
#[arcane]
pub fn inv_identity_add_8x8_8bpc_avx2(
    _token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) {
    let Ok(pitch) = console_core_number_conversion::index(dst_stride);
    let mut dst_2 = dst.flex_mut();
    let mut coeff_2 = coeff.flex_mut();
    let zero = _mm_setzero_si128();
    let max_val = _mm_set1_epi16(match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(bitdepth_max) { ::core::result::Result::Ok(converted_position) => converted_position });







    let one = _mm_set1_epi32(1);
    let eight = _mm_set1_epi32(8);

    for y in 0..8 {
        let dst_off = (Wrapping(y) * Wrapping(pitch)).0;


        let d = loadi64!(&dst_2[dst_off..(Wrapping(dst_off) + Wrapping(8)).0]);
        let d16 = _mm_unpacklo_epi8(d, zero);


        let mut coeffs = [0i16; 8];

        for (x, coeffs_element) in <[_]>::iter_mut(&mut coeffs).enumerate() {
            *coeffs_element = coeff_2[(::core::num::Wrapping(y) + ::core::num::Wrapping(::core::primitive::usize::wrapping_mul(x, 8))).0];
        }


        let c_vec = loadu_128!(<&[i16; 8]>::try_from({ let [_explicit_part @ ..] = &coeffs; let _explicit_slice: &[_] = _explicit_part; _explicit_slice }).unwrap());
        let c_lo = _mm_cvtepi16_epi32(c_vec);
        let c_hi = _mm_cvtepi16_epi32(_mm_srli_si128(c_vec, 8));


        let row_lo = _mm_slli_epi32(c_lo, 1);
        let row_hi = _mm_slli_epi32(c_hi, 1);

        let inter_lo = _mm_srai_epi32(_mm_add_epi32(row_lo, one), 1);
        let inter_hi = _mm_srai_epi32(_mm_add_epi32(row_hi, one), 1);

        let col_lo = _mm_slli_epi32(inter_lo, 1);
        let col_hi = _mm_slli_epi32(inter_hi, 1);

        let res_lo = _mm_srai_epi32(_mm_add_epi32(col_lo, eight), 4);
        let res_hi = _mm_srai_epi32(_mm_add_epi32(col_hi, eight), 4);


        let res16 = _mm_packs_epi32(res_lo, res_hi);
        let sum = _mm_add_epi16(d16, res16);
        let clamped = _mm_max_epi16(_mm_min_epi16(sum, max_val), zero);
        let packed = _mm_packus_epi16(clamped, clamped);

        storei64!(&mut dst_2[dst_off..(Wrapping(dst_off) + Wrapping(8)).0], packed);
    }


    coeff_2[..64].fill(0);
}














#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_8x8_8bpc_avx2_inner(
    _token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) {
    let Ok(pitch) = console_core_number_conversion::index(dst_stride);
    let mut dst_2 = dst.flex_mut();
    let mut coeff_2 = coeff.flex_mut();



    let _row_clip_min = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN);
    let _row_clip_max = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX);
    let col_clip_min = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN);
    let col_clip_max = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX);



    let mut tmp = [0i32; 64];


    {
        let Ok(slice) = coeff_2.as_slice();
        let coeff_arr: &[i16; 64] = slice[..64].try_into().unwrap();
        let raw = dct8_row_pass_i16_simd(_token, *coeff_arr);

        let rnd_v = _mm256_set1_epi32(1);
        let col_min_v = _mm256_set1_epi32(col_clip_min);
        let col_max_v = _mm256_set1_epi32(col_clip_max);

        for (y, tmp_row) in <[_]>::iter_mut(<[_]>::as_chunks_mut::<8>(&mut tmp).0).enumerate() {
            let v = loadu_256!(&raw[::core::primitive::usize::wrapping_mul(y, 8)..::core::primitive::usize::wrapping_mul(y, 8).wrapping_add(8)], [i32; 8]);
            let shifted = _mm256_srai_epi32::<1>(_mm256_add_epi32(v, rnd_v));
            let clipped = _mm256_max_epi32(_mm256_min_epi32(shifted, col_max_v), col_min_v);
            storeu_256!(&mut *{ let [_window @ ..] = &mut *tmp_row; let _slice: &mut [_] = _window; _slice }, [i32; 8], clipped);
        }
    }


    let col_out = dct8_col_pass_i16(_token, &tmp);


    let zero = _mm_setzero_si128();
    let max_val = _mm_set1_epi16(match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(bitdepth_max) { ::core::result::Result::Ok(converted_position) => converted_position });
    let rnd_final = _mm256_set1_epi32(8);

    for y in 0..8 {
        let dst_off = (Wrapping(y) * Wrapping(pitch)).0;


        let d = loadi64!(&dst_2[dst_off..(Wrapping(dst_off) + Wrapping(8)).0]);
        let d16 = _mm_unpacklo_epi8(d, zero);


        let c_scaled = _mm256_srai_epi32(_mm256_add_epi32(col_out[y], rnd_final), 4);


        let c_lo_scaled = _mm256_castsi256_si128(c_scaled);
        let c_hi_scaled = _mm256_extracti128_si256(c_scaled, 1);
        let c16 = _mm_packs_epi32(c_lo_scaled, c_hi_scaled);


        let sum = _mm_add_epi16(d16, c16);
        let clamped = _mm_max_epi16(_mm_min_epi16(sum, max_val), zero);
        let packed = _mm_packus_epi16(clamped, clamped);


        storei64!(&mut dst_2[dst_off..(Wrapping(dst_off) + Wrapping(8)).0], packed);
    }


    coeff_2[..64].fill(0);
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_8x8_16bpc_avx2_inner(
    _token: Desktop64,
    dst: &mut [u16],
    dst_stride: u64,
    coeff: &mut [i32],
    bitdepth_max: i32,
) {
    let mut dst_2 = dst.flex_mut();
    let mut coeff_2 = coeff.flex_mut();

    let Ok(stride_u16) = console_core_number_conversion::index(dst_stride.wrapping_div(2));


    let row_clip_min = ::core::primitive::i32::wrapping_shl(!bitdepth_max, 7_u32);
    let row_clip_max = !row_clip_min;
    let col_clip_min = ::core::primitive::i32::wrapping_shl(!bitdepth_max, 5_u32);
    let col_clip_max = !col_clip_min;



    let mut tmp = [0i32; 64];



    let rnd = 1;
    let shift = 1;

    for y in 0..8 {

        let mut scratch = [0i32; 8];

        for (x, scratch_element) in <[_]>::iter_mut(&mut scratch).enumerate() {
            *scratch_element = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(coeff_2[(::core::num::Wrapping(y) + ::core::num::Wrapping(::core::primitive::usize::wrapping_mul(x, 8))).0]) { ::core::result::Result::Ok(converted_position) => converted_position };
        }

        match dct8_1d({ let [_explicit_part @ ..] = &mut scratch; let _explicit_slice: &mut [_] = _explicit_part; _explicit_slice }, STRIDE_1, CoefficientBounds { lower: row_clip_min, upper: row_clip_max }) {
            Ok(()) => {},
            Err(_invalid_lane) => return,
        }

        for (x, scratch_element) in <[_]>::iter(&scratch).enumerate() {
            tmp[::core::primitive::usize::wrapping_mul(y, 8).wrapping_add(x)] = iclip(::core::primitive::i32::wrapping_shr(i32::wrapping_add(*scratch_element, rnd), ::core::primitive::i32::unsigned_abs(shift)), col_clip_min, col_clip_max);
        }
    }


    {
        let min_v = _mm256_set1_epi32(col_clip_min);
        let max_v = _mm256_set1_epi32(col_clip_max);
        let mut v = [_mm256_setzero_si256(); 8];

        for (v_element, tmp_row) in <[_]>::iter_mut(&mut v).zip(<[_]>::iter(<[_]>::as_chunks::<8>(&tmp).0)) {
            *v_element = loadu_256!(&*{ let [_window @ ..] = &*tmp_row; let _slice: &[_] = _window; _slice }, [i32; 8]);
        }

        dct8_1d_cols8(_token, &mut v, VectorBounds { lower: min_v, upper: max_v });

        for (v_element, tmp_row) in <[_]>::iter(&v).zip(<[_]>::iter_mut(<[_]>::as_chunks_mut::<8>(&mut tmp).0)) {
            storeu_256!(&mut *{ let [_window @ ..] = &mut *tmp_row; let _slice: &mut [_] = _window; _slice }, [i32; 8], *v_element);
        }
    }


    let zero = _mm_setzero_si128();
    let max_val = _mm_set1_epi32(bitdepth_max);
    let rnd_final = _mm_set1_epi32(8);

    for (y, tmp_row) in <[_]>::iter(<[_]>::as_chunks::<8>(&tmp).0).enumerate() {
        let dst_off = (Wrapping(y) * Wrapping(stride_u16)).0;


        let d = loadu_128!(<&[u16; 8]>::try_from(&dst_2[dst_off..(Wrapping(dst_off) + Wrapping(8)).0]).unwrap());
        let d_lo = _mm_unpacklo_epi16(d, zero);
        let d_hi = _mm_unpackhi_epi16(d, zero);


        let c_256 = loadu_256!(&*{ let [_window @ ..] = &*tmp_row; let _slice: &[_] = _window; _slice }, [i32; 8]);
        let c_lo = _mm256_castsi256_si128(c_256);
        let c_hi = _mm256_extracti128_si256(c_256, 1);


        let c_lo_scaled = _mm_srai_epi32(_mm_add_epi32(c_lo, rnd_final), 4);
        let c_hi_scaled = _mm_srai_epi32(_mm_add_epi32(c_hi, rnd_final), 4);


        let sum_lo = _mm_add_epi32(d_lo, c_lo_scaled);
        let sum_hi = _mm_add_epi32(d_hi, c_hi_scaled);


        let clamped_lo = _mm_max_epi32(_mm_min_epi32(sum_lo, max_val), zero);
        let clamped_hi = _mm_max_epi32(_mm_min_epi32(sum_hi, max_val), zero);


        let packed = _mm_packus_epi32(clamped_lo, clamped_hi);
        storeu_128!(
            <&mut [u16; 8]>::try_from(&mut dst_2[dst_off..(Wrapping(dst_off) + Wrapping(8)).0]).unwrap(),
            packed
        );
    }


    coeff_2[..64].fill(0);
}


#[cfg(target_arch = "x86_64")]
#[arcane]
pub fn inv_identity_add_16x16_8bpc_avx2(
    _token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) {
    let Ok(pitch) = console_core_number_conversion::index(dst_stride);
    let mut dst_2 = dst.flex_mut();
    let mut coeff_2 = coeff.flex_mut();
    let zero = _mm256_setzero_si256();
    let max_val = _mm256_set1_epi16(match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(bitdepth_max) { ::core::result::Result::Ok(converted_position) => converted_position });







    let mut tmp = [[0i32; 16]; 16];

    for y in 0..16 {
        for x in 0..16 {
            let c = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff_2[(::core::num::Wrapping(y) + ::core::num::Wrapping(::core::primitive::usize::wrapping_mul(x, 16))).0]);
            let r = i32::wrapping_add(i32::wrapping_mul(2, c), ::core::primitive::i32::wrapping_shr(i32::wrapping_add(i32::wrapping_mul(c, 1697), 1024), 11_u32));
            tmp[y][x] = r;
        }
    }


    for y in 0..16 {
        for x in 0..16 {
            tmp[y][x] = ::core::primitive::i32::wrapping_shr(i32::wrapping_add(tmp[y][x], 2), 2_u32).clamp(<i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN), <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX));
        }
    }


    for x in 0..16 {
        for y in 0..16 {
            let c = tmp[y][x];
            let r = i32::wrapping_add(i32::wrapping_mul(2, c), ::core::primitive::i32::wrapping_shr(i32::wrapping_add(i32::wrapping_mul(c, 1697), 1024), 11_u32));
            tmp[y][x] = ::core::primitive::i32::wrapping_shr(i32::wrapping_add(r, 8), 4_u32);
        }
    }


    for (y, tmp_row) in tmp.iter().enumerate() {
        let [
            tmp_lane_0, tmp_lane_1, tmp_lane_2, tmp_lane_3,
            tmp_lane_4, tmp_lane_5, tmp_lane_6, tmp_lane_7,
            tmp_lane_8, tmp_lane_9, tmp_lane_10, tmp_lane_11,
            tmp_lane_12, tmp_lane_13, tmp_lane_14, tmp_lane_15,
        ] = tmp_row;
        let dst_off = (Wrapping(y) * Wrapping(pitch)).0;


        let d = loadu_128!(<&[u8; 16]>::try_from(&dst_2[dst_off..(Wrapping(dst_off) + Wrapping(16)).0]).unwrap());
        let d_lo = _mm256_cvtepu8_epi16(d);


        let c_vec = _mm256_set_epi16(
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_15) { ::core::result::Result::Ok(converted_position) => converted_position },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_14) { ::core::result::Result::Ok(converted_position) => converted_position },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_13) { ::core::result::Result::Ok(converted_position) => converted_position },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_12) { ::core::result::Result::Ok(converted_position) => converted_position },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_11) { ::core::result::Result::Ok(converted_position) => converted_position },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_10) { ::core::result::Result::Ok(converted_position) => converted_position },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_9) { ::core::result::Result::Ok(converted_position) => converted_position },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_8) { ::core::result::Result::Ok(converted_position) => converted_position },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_7) { ::core::result::Result::Ok(converted_position) => converted_position },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_6) { ::core::result::Result::Ok(converted_position) => converted_position },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_5) { ::core::result::Result::Ok(converted_position) => converted_position },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_4) { ::core::result::Result::Ok(converted_position) => converted_position },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_3) { ::core::result::Result::Ok(converted_position) => converted_position },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_2) { ::core::result::Result::Ok(converted_position) => converted_position },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_1) { ::core::result::Result::Ok(converted_position) => converted_position },
            match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(*tmp_lane_0) { ::core::result::Result::Ok(converted_position) => converted_position },
        );


        let sum = _mm256_add_epi16(d_lo, c_vec);
        let clamped = _mm256_max_epi16(_mm256_min_epi16(sum, max_val), zero);


        let packed = _mm256_packus_epi16(clamped, clamped);
        let packed_lo = _mm256_castsi256_si128(packed);
        let packed_hi = _mm256_extracti128_si256(packed, 1);
        let result = _mm_unpacklo_epi64(packed_lo, packed_hi);

        storeu_128!(
            <&mut [u8; 16]>::try_from(&mut dst_2[dst_off..(Wrapping(dst_off) + Wrapping(16)).0]).unwrap(),
            result
        );
    }


    coeff_2[..256].fill(0);
}
