#[cfg(target_arch = "x86_64")]
fn residual_output_rows<'pixels, 'coefficients, Pixel>(
  destination: &'pixels mut [Pixel],
  residuals: &'coefficients [i32],
  stride: u64,
  size: console_core_geometry::Size<u32>,
) -> Rav1dResult<impl Iterator<Item = (&'pixels mut [Pixel], &'coefficients [i32])> + use<'pixels, 'coefficients, Pixel>> {
  use console_core_pixel_buffers::{RowLayout, RowLayoutError};

  let stride_2 = i64::try_from(stride).map_err(|_| Rav1dError::ERANGE)?;
  let Ok(extent) = size.map(u64::from);
  let layout = RowLayout::new(extent, 0, stride_2).map_err(|problem| match problem {
    RowLayoutError::Bounds => Rav1dError::ERANGE,
    RowLayoutError::Empty | RowLayoutError::Stride => Rav1dError::EINVAL,
  })?;
  let residual_layout = RowLayout::new(extent, 0, i64::from(size.width)).map_err(|_| Rav1dError::EINVAL)?;
  let pixels = layout.rows_mut(destination).map_err(|_| Rav1dError::EINVAL)?;
  let coefficients = residual_layout.rows(residuals).map_err(|_| Rav1dError::EINVAL)?;

  Ok(pixels.zip(coefficients))
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn add_16_residuals_8bpc(
  _token: Desktop64,
  pixels: &mut [u8; 16],
  chunks: &[[i32; 8]; 2],
  bitdepth_max: i16,
) -> Result<(), console_core_never::Never> {
  let [low, high] = chunks;
  let input = _mm256_cvtepu8_epi16(loadu_128!(&*pixels));
  let rnd = _mm256_set1_epi32(8);
  let lo = _mm256_srai_epi32::<4>(_mm256_add_epi32(loadu_256!(low), rnd));
  let hi = _mm256_srai_epi32::<4>(_mm256_add_epi32(loadu_256!(high), rnd));
  let packed = _mm256_packs_epi32(lo, hi);
  let ordered = _mm256_permute4x64_epi64::<0b11_01_10_00>(packed);
  let sum = _mm256_add_epi16(input, ordered);
  let maximum = _mm256_set1_epi16(bitdepth_max);
  let clamped = _mm256_max_epi16(_mm256_min_epi16(sum, maximum), _mm256_setzero_si256());
  let packed_pixels = _mm256_packus_epi16(clamped, clamped);
  let ordered_pixels = _mm256_permute4x64_epi64::<0b11_01_10_00>(packed_pixels);
  storeu_128!(pixels, _mm256_castsi256_si128(ordered_pixels));

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn add_to_dst_8bpc_avx2(
  token: Desktop64,
  pixels: &mut [u8],
  stride: u64,
  residuals: &[i32],
  size: console_core_geometry::Size<u32>,
  bitdepth_max: i32,
) -> Rav1dResult {
  match size.width.wrapping_rem(16) {
    0 => {}
    _ => return Err(Rav1dError::EINVAL),
  }

  let rows = residual_output_rows(pixels, residuals, stride, size)?;
  let Ok(maximum) = console_core_number_conversion::wrapped::<_, i16>(bitdepth_max);

  for (row, input) in rows {
    let (pixels, _) = row.as_chunks_mut::<16>();
    let (coefficients, _) = input.as_chunks::<8>();
    let (vectors, _) = coefficients.as_chunks::<2>();

    for (block, residual) in pixels.iter_mut().zip(vectors) {
      let Ok(()) = add_16_residuals_8bpc(token, block, residual, maximum);
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn add_to_dst_8bpc_avx512(
  token: Server64,
  pixels: &mut [u8],
  stride: u64,
  residuals: &[i32],
  size: console_core_geometry::Size<u32>,
) -> Rav1dResult {
  match size.width.wrapping_rem(16) {
    0 => {}
    _ => return Err(Rav1dError::EINVAL),
  }

  let rows = residual_output_rows(pixels, residuals, stride, size)?;
  let zero = _mm512_setzero_si512();
  let maximum = _mm512_set1_epi16(255);
  let rnd = _mm512_set1_epi32(8);

  for (row, input) in rows {
    let (blocks, remainder) = row.as_chunks_mut::<32>();
    let (coefficients, tail) = input.as_chunks::<32>();

    for (pixels, residuals) in blocks.iter_mut().zip(coefficients) {
      let (chunks, _) = residuals.as_chunks::<16>();

      let [low, high] = chunks else { return Err(Rav1dError::EINVAL) };

      let input_pixels = _mm512_cvtepu8_epi16(loadu_256!(&*pixels));
      let low_scaled = _mm512_srai_epi32::<4>(_mm512_add_epi32(loadu_512!(low), rnd));
      let high_scaled = _mm512_srai_epi32::<4>(_mm512_add_epi32(loadu_512!(high), rnd));
      let low_words = _mm512_cvtsepi32_epi16(low_scaled);
      let high_words = _mm512_cvtsepi32_epi16(high_scaled);
      let residual_words = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(low_words), high_words);
      let sum = _mm512_add_epi16(input_pixels, residual_words);
      let clipped = _mm512_max_epi16(_mm512_min_epi16(sum, maximum), zero);
      let packed = _mm512_cvtusepi16_epi8(clipped);
      storeu_256!(pixels, packed);
    }

    match (remainder.first_chunk_mut::<16>(), tail.as_chunks::<8>().0.first_chunk::<2>()) {
      (Some(pixels), Some(coefficients)) => {
        let desktop = token.v3();
        let Ok(()) = add_16_residuals_8bpc(desktop, pixels, coefficients, 255);
      }
      (_, _) => {}
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn add_4_residuals_16bpc(
  token: Desktop64,
  pixels: &mut [u16; 4],
  residuals: &[i32; 4],
  bitdepth_max: i32,
) -> Result<(), console_core_never::Never> {
  let values = loadu_128!(residuals);
  let scaled = _mm_srai_epi32::<4>(_mm_add_epi32(values, _mm_set1_epi32(8)));
  add_four_pixels_16bpc(token, pixels, scaled, bitdepth_max)
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn add_8_residuals_16bpc(
  _token: Desktop64,
  pixels: &mut [u16; 8],
  residuals: &[i32; 8],
  bitdepth_max: i32,
) -> Result<(), console_core_never::Never> {
  let wide_pixels = _mm256_cvtepu16_epi32(loadu_128!(&*pixels));
  let values = loadu_256!(residuals);
  let rounding = _mm256_set1_epi32(8);
  let scaled = _mm256_srai_epi32::<4>(_mm256_add_epi32(values, rounding));
  let sum = _mm256_add_epi32(wide_pixels, scaled);
  let lower = _mm256_setzero_si256();
  let upper = _mm256_set1_epi32(bitdepth_max);
  let clipped = _mm256_max_epi32(_mm256_min_epi32(sum, upper), lower);
  let low = _mm256_castsi256_si128(clipped);
  let high = _mm256_extracti128_si256(clipped, 1);
  storeu_128!(pixels, _mm_packus_epi32(low, high));

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn add_to_dst_16bpc_avx2(
  token: Desktop64,
  pixels: &mut [u16],
  stride: u64,
  residuals: &[i32],
  size: console_core_geometry::Size<u32>,
  bitdepth_max: i32,
) -> Rav1dResult {
  match (size.width, size.width.wrapping_rem(8)) {
    (4, _) | (_, 0) => {}
    _ => return Err(Rav1dError::EINVAL),
  }

  let rows = residual_output_rows(pixels, residuals, stride, size)?;

  match size.width {
    4 => {
      for (row, samples) in rows {
        let pixels = row.first_chunk_mut::<4>().ok_or(Rav1dError::EINVAL)?;
        let residuals_2 = samples.first_chunk::<4>().ok_or(Rav1dError::EINVAL)?;
        let Ok(()) = add_4_residuals_16bpc(token, pixels, residuals_2, bitdepth_max);
      }

      return Ok(());
    }
    _ => {}
  }

  for (row, input) in rows {
    let (pixels, _) = row.as_chunks_mut::<8>();
    let (coefficients, _) = input.as_chunks::<8>();

    for (block, residual) in pixels.iter_mut().zip(coefficients) {
      let Ok(()) = add_8_residuals_16bpc(token, block, residual, bitdepth_max);
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn add_to_dst_16bpc_avx512(
  token: Server64,
  pixels: &mut [u16],
  stride: u64,
  residuals: &[i32],
  size: console_core_geometry::Size<u32>,
  bitdepth_max: i32,
) -> Rav1dResult {
  match size.width.wrapping_rem(8) {
    0 => {}
    _ => return Err(Rav1dError::EINVAL),
  }

  let rows = residual_output_rows(pixels, residuals, stride, size)?;
  let zero = _mm512_setzero_si512();
  let maximum = _mm512_set1_epi32(bitdepth_max);
  let rnd = _mm512_set1_epi32(8);

  for (row, input) in rows {
    let (blocks, remainder) = row.as_chunks_mut::<16>();
    let (coefficients, tail) = input.as_chunks::<16>();

    for (pixels, residuals) in blocks.iter_mut().zip(coefficients) {
      let pixels_wide = _mm512_cvtepu16_epi32(loadu_256!(&*pixels));
      let values = loadu_512!(residuals);
      let scaled = _mm512_srai_epi32::<4>(_mm512_add_epi32(values, rnd));
      let sum = _mm512_add_epi32(pixels_wide, scaled);
      let clamped = _mm512_max_epi32(_mm512_min_epi32(sum, maximum), zero);
      storeu_256!(pixels, _mm512_cvtusepi32_epi16(clamped));
    }

    match (remainder.first_chunk_mut::<8>(), tail.first_chunk::<8>()) {
      (Some(pixels), Some(residuals)) => {
        let desktop = token.v3();
        let Ok(()) = add_8_residuals_16bpc(desktop, pixels, residuals, bitdepth_max);
      }
      (_, _) => {}
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn write_large_residuals_8bpc(
    token: Desktop64,
    dst: &mut [u8],
    stride: u64,
    residuals: &[i32],
    size: console_core_geometry::Size<u32>,
    bitdepth_max: i32,
) -> Rav1dResult {
    let Ok(avx512) = crate::src::cpu::summon_avx512();

    match avx512 {
        Some(wide) => add_to_dst_8bpc_avx512(wide, dst, stride, residuals, size),
        None => add_to_dst_8bpc_avx2(token, dst, stride, residuals, size, bitdepth_max),
    }
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn write_large_residuals_16bpc(
    token: Desktop64,
    dst: &mut [u16],
    stride: u64,
    residuals: &[i32],
    size: console_core_geometry::Size<u32>,
    bitdepth_max: i32,
) -> Rav1dResult {
    let Ok(avx512) = crate::src::cpu::summon_avx512();

    match avx512 {
        Some(wide) => add_to_dst_16bpc_avx512(wide, dst, stride, residuals, size, bitdepth_max),
        None => add_to_dst_16bpc_avx2(token, dst, stride, residuals, size, bitdepth_max),
    }
}

#[cfg(target_arch = "x86_64")]
fn square_dct64_residuals<C: Copy + Into<i32>>(
    coefficients: &[[C; 32]; 32],
    bounds: TransformAxisBounds,
) -> Rav1dResult<[[i32; 64]; 64]> {
    let input = coefficients.iter().map(|column| column.as_slice());
    let plan = RectangularRows {
        transform: dct64_1d,
        scale: RectangularScale::Skip,
        shift: 2,
    };
    let mut rows = transform_active_rows(input, 32, plan, bounds)?;
    transform_scalar_columns(rows.as_flattened_mut(), STRIDE_64, dct64_1d, bounds.columns)?;
    Ok(rows)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_32x32_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let coefficients = coeff.first_chunk_mut::<1024>().ok_or(Rav1dError::EINVAL)?;
    let mut residuals = dct32_row_pass_i16_simd(token, *coefficients);
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    let bias = _mm256_set1_epi32(2);
    let minimum = _mm256_set1_epi32(bounds.columns.lower);
    let maximum = _mm256_set1_epi32(bounds.columns.upper);

    for lane in residuals.as_chunks_mut::<8>().0 {
        let values = loadu_256!(&*lane);
        let rounded = _mm256_srai_epi32::<2>(_mm256_add_epi32(values, bias));
        storeu_256!(lane, _mm256_max_epi32(_mm256_min_epi32(rounded, maximum), minimum));
    }

    dct32x32_cols_simd(token, &mut residuals, bounds.columns)?;
    let stride = dst_stride;
    let size = console_core_geometry::Size { width: 32, height: 32 };
    write_large_residuals_8bpc(token, dst, stride, &residuals, size, bitdepth_max)?;
    coefficients.fill(0);
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_32x32_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let coefficients = coeff.as_chunks_mut::<32>().0.first_chunk_mut::<32>()
        .ok_or(Rav1dError::EINVAL)?;
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    let mut rows = rectangular_transform_rows(
        coefficients, identity32_1d, RectangularScale::Skip, 2, bounds,
    )?;
    let residuals = rows.as_flattened_mut();
    transform_scalar_columns(residuals, STRIDE_32, identity32_1d, bounds.columns)?;
    let stride = dst_stride;
    let size = console_core_geometry::Size { width: 32, height: 32 };
    write_large_residuals_8bpc(token, dst, stride, residuals, size, bitdepth_max)?;
    coefficients.fill([0; 32]);
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_64x64_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let coefficients = coeff.as_chunks_mut::<32>().0.first_chunk_mut::<32>()
        .ok_or(Rav1dError::EINVAL)?;
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    let rows = square_dct64_residuals(coefficients, bounds)?;
    let residuals = rows.as_flattened();
    let stride = dst_stride;
    let size = console_core_geometry::Size { width: 64, height: 64 };
    write_large_residuals_8bpc(token, dst, stride, residuals, size, bitdepth_max)?;
    coefficients.fill([0; 32]);
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_32x32_16bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u16],
    dst_stride: u64,
    coeff: &mut [i32],
    bitdepth_max: i32,
) -> Rav1dResult {
    let coefficients = coeff.as_chunks_mut::<32>().0.first_chunk_mut::<32>()
        .ok_or(Rav1dError::EINVAL)?;
    let Ok(bounds) = TransformAxisBounds::wide(bitdepth_max);
    let mut rows = rectangular_transform_rows(
        coefficients, identity32_1d, RectangularScale::Skip, 2, bounds,
    )?;
    let residuals = rows.as_flattened_mut();
    transform_scalar_columns(residuals, STRIDE_32, identity32_1d, bounds.columns)?;
    let stride = dst_stride.wrapping_div(2);
    let size = console_core_geometry::Size { width: 32, height: 32 };
    write_large_residuals_16bpc(token, dst, stride, residuals, size, bitdepth_max)?;
    coefficients.fill([0; 32]);
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_64x64_16bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u16],
    dst_stride: u64,
    coeff: &mut [i32],
    bitdepth_max: i32,
) -> Rav1dResult {
    let coefficients = coeff.as_chunks_mut::<32>().0.first_chunk_mut::<32>()
        .ok_or(Rav1dError::EINVAL)?;
    let Ok(bounds) = TransformAxisBounds::wide(bitdepth_max);
    let rows = square_dct64_residuals(coefficients, bounds)?;
    let residuals = rows.as_flattened();
    let stride = dst_stride.wrapping_div(2);
    let size = console_core_geometry::Size { width: 64, height: 64 };
    write_large_residuals_16bpc(token, dst, stride, residuals, size, bitdepth_max)?;
    coefficients.fill([0; 32]);
    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_32x32_16bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u16],
    dst_stride: u64,
    coeff: &mut [i32],
    bitdepth_max: i32,
) -> Rav1dResult {
    let coefficients = coeff.as_chunks_mut::<32>().0.first_chunk_mut::<32>()
        .ok_or(Rav1dError::EINVAL)?;
    let Ok(bounds) = TransformAxisBounds::wide(bitdepth_max);
    let mut rows = rectangular_transform_rows(
        coefficients, dct32_1d, RectangularScale::Skip, 2, bounds,
    )?;
    let residuals = rows.as_flattened_mut();
    dct32x32_cols_simd(token, residuals, bounds.columns)?;
    let stride = dst_stride.wrapping_div(2);
    let size = console_core_geometry::Size { width: 32, height: 32 };
    write_large_residuals_16bpc(token, dst, stride, residuals, size, bitdepth_max)?;
    coefficients.fill([0; 32]);
    Ok(())
}
