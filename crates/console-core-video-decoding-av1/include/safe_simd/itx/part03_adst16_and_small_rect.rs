#[cfg(target_arch = "x86_64")]
enum Square16Rows {
    Scalar(Itx1dFn),
    Adst(AdstRowOrder),
    Dct,
}

#[cfg(target_arch = "x86_64")]
struct Square16Transform {
    rows: Square16Rows,
    columns: Square16Columns,
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn transform_square16_8bpc(
    token: Desktop64,
    dst: &mut [u8],
    byte_stride: u64,
    coeff: &mut [i16],
    plan: Square16Transform,
    bitdepth_max: i32,
) -> Rav1dResult {
    let coefficients = coeff.as_chunks_mut::<16>().0.first_chunk_mut::<16>()
        .ok_or(Rav1dError::EINVAL)?;
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    let row_pass = RowPass {
        coefficient_height: 16,
        scale: RectangularScale::Skip,
        rounding: 2,
        shift: 2,
        row_bounds: bounds.rows,
        column_bounds: bounds.columns,
    };
    let mut scratch = [[0; 16]; 16];

    match plan.rows {
        Square16Rows::Scalar(row) => {
            let transformed = rectangular_transform_rows(coefficients, row, RectangularScale::Skip, 2, bounds)?;
            scratch = transformed;
        }
        Square16Rows::Dct => {
            row_dct16_8bpc_block(token, coefficients.as_flattened(), 16, scratch.as_flattened_mut(), row_pass)?;
        }
        Square16Rows::Adst(order) => {
            for first_row in [0, 8] {
                simd_row_adst16_8bpc_8rows(
                    token,
                    coefficients.as_flattened(),
                    first_row,
                    order,
                    scratch.as_flattened_mut(),
                    row_pass,
                )?;
            }
        }
    }

    let residuals = scratch.as_flattened_mut().first_chunk_mut::<256>().ok_or(Rav1dError::EINVAL)?;
    transform_square16_columns(token, residuals, plan.columns, bounds.columns)?;
    let stride = byte_stride;
    add_to_dst_8bpc_avx2(token, dst, stride, residuals, SQUARE_16, bitdepth_max)?;
    *coefficients = [[0; 16]; 16];
    Ok(())
}

#[cfg_attr(
    dylint_lib = "explicit080_macro_budget",
    expect(
        explicit080_macro_budget,
        reason = "the transform dispatch table requires a distinct target-feature entry point for each row and column transform pair"
    )
)]
macro_rules! impl_square16_transform {
    ($name:ident, $rows:expr, $columns:expr) => {
        #[cfg(target_arch = "x86_64")]
        #[arcane]
        fn $name(
            token: Desktop64,
            dst: &mut [u8],
            stride: u64,
            coeff: &mut [i16],
            bitdepth_max: i32,
        ) -> Rav1dResult {
            let plan = Square16Transform { rows: $rows, columns: $columns };
            transform_square16_8bpc(token, dst, stride, coeff, plan, bitdepth_max)
        }
    };
}

impl_square16_transform!(
    inv_txfm_add_adst_dct_16x16_8bpc_avx2_inner,
    Square16Rows::Adst(AdstRowOrder::Normal),
    Square16Columns::Dct
);
impl_square16_transform!(
    inv_txfm_add_dct_adst_16x16_8bpc_avx2_inner,
    Square16Rows::Dct,
    Square16Columns::Adst
);
impl_square16_transform!(
    inv_txfm_add_adst_adst_16x16_8bpc_avx2_inner,
    Square16Rows::Adst(AdstRowOrder::Normal),
    Square16Columns::Adst
);
impl_square16_transform!(
    inv_txfm_add_flipadst_dct_16x16_8bpc_avx2_inner,
    Square16Rows::Adst(AdstRowOrder::Reversed),
    Square16Columns::Dct
);
impl_square16_transform!(
    inv_txfm_add_dct_flipadst_16x16_8bpc_avx2_inner,
    Square16Rows::Dct,
    Square16Columns::FlipAdst
);
impl_square16_transform!(
    inv_txfm_add_flipadst_flipadst_16x16_8bpc_avx2_inner,
    Square16Rows::Adst(AdstRowOrder::Reversed),
    Square16Columns::FlipAdst
);
impl_square16_transform!(
    inv_txfm_add_adst_flipadst_16x16_8bpc_avx2_inner,
    Square16Rows::Adst(AdstRowOrder::Normal),
    Square16Columns::FlipAdst
);
impl_square16_transform!(
    inv_txfm_add_flipadst_adst_16x16_8bpc_avx2_inner,
    Square16Rows::Adst(AdstRowOrder::Reversed),
    Square16Columns::Adst
);
impl_square16_transform!(
    inv_txfm_add_identity_dct_16x16_8bpc_avx2_inner,
    Square16Rows::Scalar(identity16_1d),
    Square16Columns::Dct
);
impl_square16_transform!(
    inv_txfm_add_dct_identity_16x16_8bpc_avx2_inner,
    Square16Rows::Dct,
    Square16Columns::Identity
);
impl_square16_transform!(
    inv_txfm_add_identity_adst_16x16_8bpc_avx2_inner,
    Square16Rows::Scalar(identity16_1d),
    Square16Columns::Adst
);
impl_square16_transform!(
    inv_txfm_add_adst_identity_16x16_8bpc_avx2_inner,
    Square16Rows::Adst(AdstRowOrder::Normal),
    Square16Columns::Identity
);
impl_square16_transform!(
    inv_txfm_add_identity_flipadst_16x16_8bpc_avx2_inner,
    Square16Rows::Scalar(identity16_1d),
    Square16Columns::FlipAdst
);
impl_square16_transform!(
    inv_txfm_add_flipadst_identity_16x16_8bpc_avx2_inner,
    Square16Rows::Adst(AdstRowOrder::Reversed),
    Square16Columns::Identity
);


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_16x16_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let coefficients = coeff.first_chunk_mut::<256>().ok_or(Rav1dError::EINVAL)?;
    let Ok(mut rows) = dct16_row_pass_i16_simd(token, *coefficients);
    let rounding = _mm256_set1_epi32(2);
    let minimum = _mm256_set1_epi32(i32::from(i16::MIN));
    let maximum = _mm256_set1_epi32(i32::from(i16::MAX));

    for lanes in rows.as_chunks_mut::<8>().0 {
        let values = loadu_256!(&*lanes);
        let shifted = _mm256_srai_epi32::<2>(_mm256_add_epi32(values, rounding));
        let clipped = _mm256_max_epi32(_mm256_min_epi32(shifted, maximum), minimum);
        storeu_256!(lanes, clipped);
    }

    let Ok(residuals) = dct16_col_pass_i16(token, &rows);
    let stride = dst_stride;
    let extent = console_core_geometry::Size { width: 16, height: 16 };
    add_to_dst_8bpc_avx2(token, dst, stride, &residuals, extent, bitdepth_max)?;
    *coefficients = [0; 256];
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_16x16_16bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u16],
    byte_stride: u64,
    coeff: &mut [i32],
    bitdepth_max: i32,
) -> Rav1dResult {
    let coefficients = coeff.as_chunks_mut::<16>().0.first_chunk_mut::<16>()
        .ok_or(Rav1dError::EINVAL)?;
    let Ok(bounds) = TransformAxisBounds::wide(bitdepth_max);
    let mut rows = rectangular_transform_rows(coefficients, dct16_1d, RectangularScale::Skip, 2, bounds)?;
    let residuals = rows.as_flattened_mut().first_chunk_mut::<256>().ok_or(Rav1dError::EINVAL)?;
    transform_square16_columns(token, residuals, Square16Columns::Dct, bounds.columns)?;

    let stride = byte_stride;
    let extent = console_core_geometry::Size { width: 16, height: 16 };
    add_to_dst_16bpc_avx2(token, dst, ::core::primitive::u64::wrapping_shr(stride, 1_u32), residuals, extent, bitdepth_max)?;
    *coefficients = [[0; 16]; 16];
    Ok(())
}

#[cfg(target_arch = "x86_64")]
fn rectangular_transform_rows<
    C: Copy + Into<i32>,
    const WIDTH: console_core_api_compatibility::NativeSize,
    const HEIGHT: console_core_api_compatibility::NativeSize,
>(
    coefficients: &[[C; HEIGHT]; WIDTH],
    transform: Itx1dFn,
    scale: RectangularScale,
    row_shift: u32,
    bounds: TransformAxisBounds,
) -> Rav1dResult<[[i32; WIDTH]; HEIGHT]> {
    let input_height = u32::try_from(HEIGHT).map_err(|_| Rav1dError::ERANGE)?;
    let columns = coefficients.iter().map(|column| column.as_slice());
    let rows = RectangularRows { transform, scale, shift: row_shift };
    transform_active_rows(columns, input_height, rows, bounds)
}

#[cfg(target_arch = "x86_64")]
fn transform_active_rows<
    'a,
    C: Copy + Into<i32> + 'a,
    const WIDTH: console_core_api_compatibility::NativeSize,
    const HEIGHT: console_core_api_compatibility::NativeSize,
>(
    coefficients: impl IntoIterator<Item = &'a [C]>,
    input_height: u32,
    transform: RectangularRows,
    bounds: TransformAxisBounds,
) -> Rav1dResult<[[i32; WIDTH]; HEIGHT]> {
    use console_core_video_interpolation::round_shift;

    let mut scratch = [[0; WIDTH]; HEIGHT];

    for (column, values) in (0u32..).zip(coefficients) {
        let Ok(position) = console_core_number_conversion::index(column);

        for (row, &value) in scratch.iter_mut().zip(values) {
            let destination = row.get_mut(position).ok_or(Rav1dError::EINVAL)?;
            let coefficient = <_ as ::core::convert::Into<::core::primitive::i32>>::into(value);
            *destination = match transform.scale {
                RectangularScale::Apply => round_shift(coefficient.wrapping_mul(181), 8),
                RectangularScale::Skip => coefficient,
            };
        }
    }

    let Ok(active_rows) = console_core_number_conversion::index(input_height);

    for row in scratch.iter_mut().take(active_rows) {
        (transform.transform)(row, STRIDE_1, bounds.rows)?;

        for sample in row {
            *sample = iclip(round_shift(*sample, transform.shift), bounds.columns.lower, bounds.columns.upper);
        }
    }

    Ok(scratch)
}

#[cfg(target_arch = "x86_64")]
fn transform_scalar_columns(
    scratch: &mut [i32],
    width: NonZeroU32,
    transform: Itx1dFn,
    bounds: CoefficientBounds,
) -> Rav1dResult {
    for column in 0..width.get() {
        let Ok(position) = console_core_number_conversion::index(column);
        let input = scratch.get_mut(position..).ok_or(Rav1dError::EINVAL)?;
        transform(input, width, bounds)?;
    }

    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn add_eight_pixels_8bpc(
    _token: Desktop64,
    pixels: &mut [u8; 8],
    residuals: &[i32; 8],
    maximum: i16,
) -> Result<(), Never> {
    let input = loadu_256!(residuals);
    let rounding = _mm256_set1_epi32(8);
    let scaled = _mm256_srai_epi32::<4>(_mm256_add_epi32(input, rounding));
    let low = _mm256_castsi256_si128(scaled);
    let high = _mm256_extracti128_si256::<1>(scaled);
    let residual = _mm_packs_epi32(low, high);
    let destination = _mm_cvtepu8_epi16(_mm_cvtsi64_si128(i64::from_le_bytes(*pixels)));
    let sum = _mm_add_epi16(destination, residual);
    let clipped = _mm_max_epi16(_mm_setzero_si128(), _mm_min_epi16(sum, _mm_set1_epi16(maximum)));
    *pixels = _mm_cvtsi128_si64(_mm_packus_epi16(clipped, clipped)).to_le_bytes();
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_4x8_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let coefficients = coeff.as_chunks_mut::<8>().0.first_chunk_mut::<4>()
        .ok_or(Rav1dError::EINVAL)?;
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    let mut scratch = rectangular_transform_rows(coefficients, dct4_1d, RectangularScale::Apply, 0, bounds)?;
    transform_scalar_columns(scratch.as_flattened_mut(), STRIDE_4, dct8_1d, bounds.columns)?;

    let rounding = _mm_set1_epi32(8);
    let residuals = scratch.map(|row| _mm_srai_epi32::<4>(_mm_add_epi32(loadu_128!(&row), rounding)));
    let stride = dst_stride;
    write_four_pixel_rows(dst, stride, residuals, |pixels, values| {
        let Ok(()) = add_four_pixels_8bpc(token, pixels, values, bitdepth_max);
    })?;
    *coefficients = [[0; 8]; 4];
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_8x4_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let coefficients = coeff.as_chunks_mut::<4>().0.first_chunk_mut::<8>()
        .ok_or(Rav1dError::EINVAL)?;
    let Ok(scalar_bounds) = TransformAxisBounds::narrow(255);
    let mut scratch = rectangular_transform_rows(coefficients, dct8_1d, RectangularScale::Apply, 0, scalar_bounds)?;
    let extent = console_core_geometry::Size { width: 8, height: 4 };
    let bounds = VectorBounds {
        lower: _mm256_set1_epi32(scalar_bounds.columns.lower),
        upper: _mm256_set1_epi32(scalar_bounds.columns.upper),
    };
    apply_transform_columns_avx2(token, scratch.as_flattened_mut(), extent, bounds, |column_token, values, clipping| dct4_1d_cols8(column_token, values, clipping))?;

    let stride = dst_stride;
    let rows = residual_output_rows(dst, scratch.as_flattened(), stride, extent)?;
    let Ok(maximum) = console_core_number_conversion::wrapped::<_, i16>(bitdepth_max);

    for (pixels, values) in rows {
        let block = pixels.first_chunk_mut::<8>().ok_or(Rav1dError::EINVAL)?;
        let residuals = values.first_chunk::<8>().ok_or(Rav1dError::EINVAL)?;
        let Ok(()) = add_eight_pixels_8bpc(token, block, residuals, maximum);
    }

    *coefficients = [[0; 4]; 8];
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_8x16_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 8, height: 16 };
    let (pixels, coefficients) = rectangular_8bpc_views(dst, coeff, dst_stride, extent)?;
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    let mut scratch = [0i32; 128];
    let row_pass = RowPass {
        coefficient_height: 16,
        scale: RectangularScale::Apply,
        rounding: 1,
        shift: 1,
        row_bounds: bounds.rows,
        column_bounds: bounds.columns,
    };
    row_dct8_8bpc_block(token, coefficients, 16, &mut scratch, row_pass)?;

    let vector_bounds = VectorBounds {
        lower: _mm256_set1_epi32(bounds.columns.lower),
        upper: _mm256_set1_epi32(bounds.columns.upper),
    };
    apply_transform_columns_avx2(token, &mut scratch, extent, vector_bounds, |column_token, values, clipping| {
        dct16_1d_cols8(column_token, values, clipping)
    })?;

    let rows = residual_output_rows(pixels, &scratch, dst_stride, extent)?;
    let Ok(maximum) = console_core_number_conversion::wrapped::<_, i16>(bitdepth_max);

    for (output, residual) in rows {
        let pixel_row = output.first_chunk_mut::<8>().ok_or(Rav1dError::EINVAL)?;
        let residual_row = residual.first_chunk::<8>().ok_or(Rav1dError::EINVAL)?;
        let Ok(()) = add_eight_pixels_8bpc(token, pixel_row, residual_row, maximum);
    }

    coefficients.fill(0);
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_16x8_8bpc_avx2_inner(
    _token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 16, height: 8 };
    let native_stride = dst_stride;
    let (pixel_view, coefficient_view) = rectangular_8bpc_views(dst, coeff, native_stride, extent)?;

    let row_clip_min = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN);
    let row_clip_max = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX);
    let col_clip_min = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN);
    let col_clip_max = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX);
    let mut tmp = [0i32; 128];


    {
        simd_row_dct16_8bpc_8rows(
          _token,
          coefficient_view,
          0,
          &mut tmp,
          RowPass {
            coefficient_height: 8,
            scale: RectangularScale::Apply,
            rounding: 1,
            shift: 1,
            row_bounds: CoefficientBounds { lower: row_clip_min, upper: row_clip_max },
            column_bounds: CoefficientBounds { lower: col_clip_min, upper: col_clip_max },
          },
        )?;
    }



    let Ok(avx512) = crate::src::cpu::summon_avx512();

    let wide_result = match avx512 {
        Some(server) => Some(dct8_cols_avx512(server, &mut tmp, extent, CoefficientBounds { lower: col_clip_min, upper: col_clip_max })),
        None => None,
    };
    let Ok(column_dispatch) = scratch_column_dispatch(wide_result);

    match column_dispatch { ColumnDispatch::Applied => {}, ColumnDispatch::Fallback => {
        let min_v = _mm256_set1_epi32(col_clip_min);
        let max_v = _mm256_set1_epi32(col_clip_max);

        let columns = console_core_geometry::Size { width: 16, height: 8 };
        apply_transform_columns_avx2(
          _token,
          &mut tmp,
          columns,
          VectorBounds { lower: min_v, upper: max_v },
          |token, values, bounds| dct8_1d_cols8(token, values, bounds),
        )?;
    } }


    add_to_dst_8bpc_avx2(_token, pixel_view, native_stride, &tmp, extent, bitdepth_max)?;
    coefficient_view.fill(0);

    Ok(())
}
