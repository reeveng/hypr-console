use archmage::{Desktop64, Server64, arcane, rite};
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

use crate::include::common::bitdepth::BitDepth;
#[cfg(target_arch = "x86_64")]
use console_core_never::Never;
use crate::include::common::intops::iclip;
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
    let Ok(transformed_columns) = transpose_4x4_i32(token, horizontal);
    let clipped = transformed_columns.map(|lane| _mm_max_epi32(_mm_min_epi32(lane, high), low));
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
    let Ok(transformed_columns) = transpose_4x4_i32(token, horizontal);
    wht4_lanes(token, transformed_columns)
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
#[inline]
fn square_transform_rows<const SIDE: usize, Pixel>(
    destination: &mut [Pixel],
    pixel_stride: u64,
) -> Rav1dResult<impl Iterator<Item = &mut [Pixel; SIDE]>> {
    let Ok(side) = console_core_number_conversion::fixed_index(SIDE);

    match pixel_stride.cmp(&side) {
        core::cmp::Ordering::Less => return Err(Rav1dError::EINVAL),
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {}
    }

    let preceding_rows = side.checked_sub(1).ok_or(Rav1dError::EINVAL)?;
    let span = preceding_rows.checked_mul(pixel_stride)
        .and_then(|end| end.checked_add(side))
        .ok_or(Rav1dError::ERANGE)?;
    let Ok(length) = console_core_number_conversion::index(span);
    let pitch = console_core_number_conversion::checked_index(pixel_stride)
        .map_err(|_stride_out_of_range| Rav1dError::ERANGE)?;
    let active = destination.get_mut(..length).ok_or(Rav1dError::EINVAL)?;
    Ok(active.chunks_mut(pitch.max(1)).filter_map(<[_]>::first_chunk_mut::<SIDE>))
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_identity_add_4x4_8bpc_avx2(
    _token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let block = coeff.first_chunk_mut::<16>().ok_or(Rav1dError::EINVAL)?;
    let rows = square_transform_rows::<4, _>(dst, dst_stride)?;
    let [a0, a1, a2, a3, b0, b1, b2, b3, c0, c1, c2, c3, d0, d1, d2, d3] =
        block.map(i32::from);
    let Ok(bounds) = TransformAxisBounds::narrow(bitdepth_max);
    let zero = _mm_setzero_si128();
    let Ok(maximum) = console_core_number_conversion::wrapped::<_, i16>(bitdepth_max);
    let max_val = _mm_set1_epi16(maximum);
    let identity = |value: i32| {
        value.wrapping_add(value.wrapping_mul(1697).wrapping_add(2048).wrapping_shr(12))
    };

    for (pixels, samples) in rows.zip([
        [a0, b0, c0, d0],
        [a1, b1, c1, d1],
        [a2, b2, c2, d2],
        [a3, b3, c3, d3],
    ]) {
        let [a, b, c, d] = samples.map(|sample| {
            let row = identity(sample).clamp(bounds.columns.lower, bounds.columns.upper);
            identity(row).wrapping_add(8).wrapping_shr(4)
        });
        let residuals = _mm_setr_epi32(a, b, c, d);
        let input = _mm_unpacklo_epi8(loadi32!(&*pixels), zero);
        let destination = _mm_cvtepi16_epi32(input);
        let sum = _mm_add_epi32(destination, residuals);
        let words = _mm_packs_epi32(sum, sum);
        let clamped = _mm_max_epi16(_mm_min_epi16(words, max_val), zero);
        storei32!(&mut *pixels, _mm_packus_epi16(clamped, clamped));
    }

    block.fill(0);
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_identity_add_8x8_8bpc_avx2(
    _token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let block = coeff.first_chunk_mut::<64>().ok_or(Rav1dError::EINVAL)?;
    let (columns, _) = block.as_chunks::<8>();
    let rows = square_transform_rows::<8, _>(dst, dst_stride)?;
    let zero = _mm_setzero_si128();
    let Ok(maximum) = console_core_number_conversion::wrapped::<_, i16>(bitdepth_max);
    let max_val = _mm_set1_epi16(maximum);
    let one = _mm_set1_epi32(1);
    let eight = _mm_set1_epi32(8);

    for (y, pixels) in rows.enumerate() {
        let mut coeffs = [0i16; 8];

        for (lane, column) in coeffs.iter_mut().zip(columns) {
            let sample = column.get(y).ok_or(Rav1dError::EINVAL)?;
            *lane = *sample;
        }

        let destination = _mm_unpacklo_epi8(loadi64!(&*pixels), zero);
        let coefficients = loadu_128!(&coeffs);
        let low = _mm_cvtepi16_epi32(coefficients);
        let high = _mm_cvtepi16_epi32(_mm_srli_si128::<8>(coefficients));
        let row_low = _mm_slli_epi32::<1>(low);
        let row_high = _mm_slli_epi32::<1>(high);
        let inter_low = _mm_srai_epi32::<1>(_mm_add_epi32(row_low, one));
        let inter_high = _mm_srai_epi32::<1>(_mm_add_epi32(row_high, one));
        let col_low = _mm_slli_epi32::<1>(inter_low);
        let col_high = _mm_slli_epi32::<1>(inter_high);
        let res_low = _mm_srai_epi32::<4>(_mm_add_epi32(col_low, eight));
        let res_high = _mm_srai_epi32::<4>(_mm_add_epi32(col_high, eight));
        let residual = _mm_packs_epi32(res_low, res_high);
        let sum = _mm_add_epi16(destination, residual);
        let clamped = _mm_max_epi16(_mm_min_epi16(sum, max_val), zero);
        let packed = _mm_packus_epi16(clamped, clamped);
        storei64!(&mut *pixels, packed);
    }

    block.fill(0);
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_8x8_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let block = coeff.first_chunk_mut::<64>().ok_or(Rav1dError::EINVAL)?;
    let rows = square_transform_rows::<8, _>(dst, dst_stride)?;
    let Ok(raw) = dct8_row_pass_i16_simd(token, *block);
    let (source_rows, _) = raw.as_chunks::<8>();
    let mut tmp = [0i32; 64];
    let (horizontal_rows, _) = tmp.as_chunks_mut::<8>();
    let rounding = _mm256_set1_epi32(1);
    let minimum = _mm256_set1_epi32(i32::from(i16::MIN));
    let maximum = _mm256_set1_epi32(i32::from(i16::MAX));

    for (input, output) in source_rows.iter().zip(horizontal_rows) {
        let values = loadu_256!(input);
        let shifted = _mm256_srai_epi32::<1>(_mm256_add_epi32(values, rounding));
        let clipped = _mm256_max_epi32(_mm256_min_epi32(shifted, maximum), minimum);
        storeu_256!(output, clipped);
    }

    let Ok(residuals) = dct8_col_pass_i16(token, &tmp);
    let zero = _mm_setzero_si128();
    let Ok(max_pixel) = console_core_number_conversion::wrapped::<_, i16>(bitdepth_max);
    let ceiling = _mm_set1_epi16(max_pixel);
    let rounding_final = _mm256_set1_epi32(8);

    for (pixels, residual) in rows.zip(residuals) {
        let destination = _mm_unpacklo_epi8(loadi64!(&*pixels), zero);
        let scaled = _mm256_srai_epi32::<4>(_mm256_add_epi32(residual, rounding_final));
        let lo = _mm256_castsi256_si128(scaled);
        let hi = _mm256_extracti128_si256::<1>(scaled);
        let sum = _mm_add_epi16(destination, _mm_packs_epi32(lo, hi));
        let clamped = _mm_max_epi16(_mm_min_epi16(sum, ceiling), zero);
        let packed = _mm_packus_epi16(clamped, clamped);
        storei64!(&mut *pixels, packed);
    }

    block.fill(0);
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_8x8_16bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u16],
    byte_stride: u64,
    coeff: &mut [i32],
    bitdepth_max: i32,
) -> Rav1dResult {
    let block = coeff.first_chunk_mut::<64>().ok_or(Rav1dError::EINVAL)?;
    let (columns, _) = block.as_chunks::<8>();
    let pixel_stride = byte_stride.wrapping_div(2);
    let rows = square_transform_rows::<8, _>(dst, pixel_stride)?;
    let Ok(bounds) = TransformAxisBounds::wide(bitdepth_max);
    let mut residuals = [_mm256_setzero_si256(); 8];

    for (y, residual) in residuals.iter_mut().enumerate() {
        let mut samples = [0i32; 8];

        for (lane, column) in samples.iter_mut().zip(columns) {
            let coefficient = column.get(y).ok_or(Rav1dError::EINVAL)?;
            *lane = *coefficient;
        }

        dct8_1d(&mut samples, STRIDE_1, bounds.rows)?;
        let clipped = samples.map(|coefficient| {
            coefficient.wrapping_add(1).wrapping_shr(1)
                .clamp(bounds.columns.lower, bounds.columns.upper)
        });
        *residual = loadu_256!(&clipped);
    }

    let min_v = _mm256_set1_epi32(bounds.columns.lower);
    let max_v = _mm256_set1_epi32(bounds.columns.upper);
    let Ok(()) = dct8_1d_cols8(token, &mut residuals, VectorBounds { lower: min_v, upper: max_v });
    let zero = _mm_setzero_si128();
    let max_val = _mm_set1_epi32(bitdepth_max);
    let rounding = _mm_set1_epi32(8);

    for (pixels, residual) in rows.zip(residuals) {
        let destination = loadu_128!(&*pixels);
        let low = _mm_unpacklo_epi16(destination, zero);
        let high = _mm_unpackhi_epi16(destination, zero);
        let res_low = _mm256_castsi256_si128(residual);
        let res_high = _mm256_extracti128_si256::<1>(residual);
        let scaled_low = _mm_srai_epi32::<4>(_mm_add_epi32(res_low, rounding));
        let scaled_high = _mm_srai_epi32::<4>(_mm_add_epi32(res_high, rounding));
        let sum_low = _mm_add_epi32(low, scaled_low);
        let sum_high = _mm_add_epi32(high, scaled_high);
        let clamped_low = _mm_max_epi32(_mm_min_epi32(sum_low, max_val), zero);
        let clamped_high = _mm_max_epi32(_mm_min_epi32(sum_high, max_val), zero);
        storeu_128!(pixels, _mm_packus_epi32(clamped_low, clamped_high));
    }

    block.fill(0);
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_identity_add_16x16_8bpc_avx2(
    _token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let block = coeff.first_chunk_mut::<256>().ok_or(Rav1dError::EINVAL)?;
    let (columns, _) = block.as_chunks::<16>();
    let rows = square_transform_rows::<16, _>(dst, dst_stride)?;
    let zero = _mm256_setzero_si256();
    let Ok(maximum) = console_core_number_conversion::wrapped::<_, i16>(bitdepth_max);
    let max_val = _mm256_set1_epi16(maximum);
    let identity = |value: i32| {
        let scaled = value.wrapping_mul(1697).wrapping_add(1024).wrapping_shr(11);
        value.wrapping_mul(2).wrapping_add(scaled)
    };

    for (y, pixels) in rows.enumerate() {
        let mut residuals = [0i16; 16];

        for (lane, column) in residuals.iter_mut().zip(columns) {
            let sample = column.get(y).ok_or(Rav1dError::EINVAL)?;
            let row = identity(i32::from(*sample)).wrapping_add(2).wrapping_shr(2);
            let clipped = row.clamp(i32::from(i16::MIN), i32::from(i16::MAX));
            let residual = identity(clipped).wrapping_add(8).wrapping_shr(4);
            let Ok(value) = console_core_number_conversion::wrapped(residual);
            *lane = value;
        }

        let destination = _mm256_cvtepu8_epi16(loadu_128!(&*pixels));
        let sum = _mm256_add_epi16(destination, loadu_256!(&residuals));
        let clamped = _mm256_max_epi16(_mm256_min_epi16(sum, max_val), zero);
        let packed = _mm256_packus_epi16(clamped, clamped);
        let packed_low = _mm256_castsi256_si128(packed);
        let packed_high = _mm256_extracti128_si256::<1>(packed);
        storeu_128!(pixels, _mm_unpacklo_epi64(packed_low, packed_high));
    }

    block.fill(0);
    Ok(())
}
