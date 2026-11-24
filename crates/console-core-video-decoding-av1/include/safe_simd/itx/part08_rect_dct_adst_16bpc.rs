#[cfg(target_arch = "x86_64")]
struct NarrowHighTransform {
  rows: Itx1dFn,
  columns: Itx1dFn,
  scale: RectangularScale,
  row_shift: u32,
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn transform_four_wide_16bpc<const HEIGHT: usize>(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  plan: NarrowHighTransform,
  bitdepth_max: i32,
) -> Rav1dResult {
  let active = coeff.as_chunks_mut::<HEIGHT>().0
    .first_chunk_mut::<4>()
    .ok_or(Rav1dError::EINVAL)?;
  let Ok(bounds) = TransformAxisBounds::wide(bitdepth_max);
  let mut scratch = rectangular_transform_rows(active, plan.rows, plan.scale, plan.row_shift, bounds)?;
  let residuals = scratch.as_flattened_mut();
  transform_scalar_columns(residuals, STRIDE_4, plan.columns, bounds.columns)?;

  let stride = byte_stride.wrapping_div(2);
  let Ok(height) = console_core_number_conversion::wrapped::<_, u32>(HEIGHT);
  let extent = console_core_geometry::Size { width: 4, height };
  add_to_dst_16bpc_avx2(token, dst, stride, residuals, extent, bitdepth_max)?;
  active.fill([0; HEIGHT]);

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_4x8_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let plan = NarrowHighTransform {
    rows: dct4_1d,
    columns: dct8_1d,
    scale: RectangularScale::Apply,
    row_shift: 0,
  };
  transform_four_wide_16bpc::<8>(token, dst, byte_stride, coeff, plan, bitdepth_max)
}


#[cfg(target_arch = "x86_64")]
struct RectangularRows {
  transform: Itx1dFn,
  scale: RectangularScale,
  shift: u32,
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn transform_rectangular_16bpc<
  const WIDTH: console_core_api_compatibility::NativeSize,
  const HEIGHT: console_core_api_compatibility::NativeSize,
>(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  rows: RectangularRows,
  bitdepth_max: i32,
  columns: impl FnOnce(Desktop64, &mut [i32], console_core_geometry::Size<u32>, CoefficientBounds) -> Rav1dResult,
) -> Rav1dResult {
  let width = u32::try_from(WIDTH).map_err(|_| Rav1dError::EINVAL)?;
  let height = u32::try_from(HEIGHT).map_err(|_| Rav1dError::EINVAL)?;
  let extent = console_core_geometry::Size { width, height };
  let input_height = NonZeroU32::new(height.min(32)).ok_or(Rav1dError::EINVAL)?;
  let input_width = width.min(32);
  let sample_count = input_width.wrapping_mul(input_height.get());
  let Ok(count) = console_core_number_conversion::index(sample_count);
  let Ok(coefficient_pitch) = console_core_number_conversion::index(input_height.get());
  let active = coeff.get_mut(..count).ok_or(Rav1dError::EINVAL)?;
  let coefficients = active.chunks_exact(coefficient_pitch.max(1));
  let Ok(bounds) = TransformAxisBounds::wide(bitdepth_max);
  let mut scratch = transform_active_rows::<_, WIDTH, HEIGHT>(coefficients, input_height.get(), rows, bounds)?;
  let residuals = scratch.as_flattened_mut();
  columns(token, residuals, extent, bounds.columns)?;
  let pixel_stride = ::core::primitive::u64::wrapping_shr(byte_stride, 1_u32);
  let Ok(wide) = match (width, height) {
    (32.., _) | (_, 64) => crate::src::cpu::summon_avx512(),
    _ => Ok(None),
  };

  match wide {
    Some(server) => add_to_dst_16bpc_avx512(server, dst, pixel_stride, residuals, extent, bitdepth_max)?,
    None => add_to_dst_16bpc_avx2(token, dst, pixel_stride, residuals, extent, bitdepth_max)?,
  }

  active.fill(0);
  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_8x4_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: dct8_1d,
    scale: RectangularScale::Apply,
    shift: 0,
  };
  let columns = |_, residuals: &mut [i32], _, bounds| transform_scalar_columns(residuals, STRIDE_8, dct4_1d, bounds);
  let stride = byte_stride;
  transform_rectangular_16bpc::<8, 4>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_8x16_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: dct8_1d,
    scale: RectangularScale::Apply,
    shift: 1,
  };
  let columns = |desktop, residuals: &mut [i32], extent, bounds: CoefficientBounds| {
    let limits = VectorBounds {
      lower: _mm256_set1_epi32(bounds.lower),
      upper: _mm256_set1_epi32(bounds.upper),
    };
    apply_transform_columns_avx2(desktop, residuals, extent, limits, |column_token, values, clipping| {
      dct16_1d_cols8(column_token, values, clipping)
    })
  };
  let stride = byte_stride;
  transform_rectangular_16bpc::<8, 16>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_16x8_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: dct16_1d,
    scale: RectangularScale::Apply,
    shift: 1,
  };
  let columns = |desktop, residuals: &mut [i32], extent, bounds: CoefficientBounds| {
    let Ok(avx512) = crate::src::cpu::summon_avx512();
    let wide = match avx512 {
      Some(server) => Some(dct8_cols_avx512(server, residuals, extent, bounds)),
      None => None,
    };
    let Ok(dispatch) = scratch_column_dispatch(wide);

    match dispatch {
      ColumnDispatch::Applied => return Ok(()),
      ColumnDispatch::Fallback => {}
    }

    let limits = VectorBounds {
      lower: _mm256_set1_epi32(bounds.lower),
      upper: _mm256_set1_epi32(bounds.upper),
    };
    apply_transform_columns_avx2(desktop, residuals, extent, limits, |column_token, values, clipping| {
      dct8_1d_cols8(column_token, values, clipping)
    })
  };
  let stride = byte_stride;
  transform_rectangular_16bpc::<16, 8>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_4x16_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let plan = NarrowHighTransform {
    rows: dct4_1d,
    columns: dct16_1d,
    scale: RectangularScale::Skip,
    row_shift: 1,
  };
  transform_four_wide_16bpc::<16>(token, dst, byte_stride, coeff, plan, bitdepth_max)
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_16x4_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: dct16_1d,
    scale: RectangularScale::Skip,
    shift: 1,
  };
  let columns = |_, residuals: &mut [i32], _, bounds| transform_scalar_columns(residuals, STRIDE_16, dct4_1d, bounds);
  let stride = byte_stride;
  transform_rectangular_16bpc::<16, 4>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_16x32_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: dct16_1d,
    scale: RectangularScale::Apply,
    shift: 1,
  };
  let columns = |desktop, residuals: &mut [i32], extent, bounds: CoefficientBounds| {
    let Ok(avx512) = crate::src::cpu::summon_avx512();
    let wide = match avx512 {
      Some(server) => Some(dct32_cols_avx512(server, residuals, extent, bounds)),
      None => None,
    };
    let Ok(dispatch) = scratch_column_dispatch(wide);

    match dispatch {
      ColumnDispatch::Applied => return Ok(()),
      ColumnDispatch::Fallback => {}
    }

    let limits = VectorBounds {
      lower: _mm256_set1_epi32(bounds.lower),
      upper: _mm256_set1_epi32(bounds.upper),
    };
    apply_transform_columns_avx2(desktop, residuals, extent, limits, |column_token, values, clipping| {
      dct32_1d_cols8(column_token, values, clipping)
    })
  };
  let stride = byte_stride;
  transform_rectangular_16bpc::<16, 32>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_32x16_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: dct32_1d,
    scale: RectangularScale::Apply,
    shift: 1,
  };
  let columns = |desktop, residuals: &mut [i32], extent, bounds: CoefficientBounds| {
    let Ok(avx512) = crate::src::cpu::summon_avx512();
    let wide = match avx512 {
      Some(server) => Some(dct16_cols_avx512(server, residuals, extent, bounds)),
      None => None,
    };
    let Ok(dispatch) = scratch_column_dispatch(wide);

    match dispatch {
      ColumnDispatch::Applied => return Ok(()),
      ColumnDispatch::Fallback => {}
    }

    let limits = VectorBounds {
      lower: _mm256_set1_epi32(bounds.lower),
      upper: _mm256_set1_epi32(bounds.upper),
    };
    apply_transform_columns_avx2(desktop, residuals, extent, limits, |column_token, values, clipping| {
      dct16_1d_cols8(column_token, values, clipping)
    })
  };
  let stride = byte_stride;
  transform_rectangular_16bpc::<32, 16>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_8x32_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: dct8_1d,
    scale: RectangularScale::Skip,
    shift: 2,
  };
  let columns = |desktop, residuals: &mut [i32], extent, bounds: CoefficientBounds| {
    let limits = VectorBounds {
      lower: _mm256_set1_epi32(bounds.lower),
      upper: _mm256_set1_epi32(bounds.upper),
    };
    apply_transform_columns_avx2(desktop, residuals, extent, limits, |column_token, values, clipping| {
      dct32_1d_cols8(column_token, values, clipping)
    })
  };
  let stride = byte_stride;
  transform_rectangular_16bpc::<8, 32>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_32x8_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: dct32_1d,
    scale: RectangularScale::Skip,
    shift: 2,
  };
  let columns = |desktop, residuals: &mut [i32], extent, bounds: CoefficientBounds| {
    let Ok(avx512) = crate::src::cpu::summon_avx512();
    let wide = match avx512 {
      Some(server) => Some(dct8_cols_avx512(server, residuals, extent, bounds)),
      None => None,
    };
    let Ok(dispatch) = scratch_column_dispatch(wide);

    match dispatch {
      ColumnDispatch::Applied => return Ok(()),
      ColumnDispatch::Fallback => {}
    }

    let limits = VectorBounds {
      lower: _mm256_set1_epi32(bounds.lower),
      upper: _mm256_set1_epi32(bounds.upper),
    };
    apply_transform_columns_avx2(desktop, residuals, extent, limits, |column_token, values, clipping| {
      dct8_1d_cols8(column_token, values, clipping)
    })
  };
  let stride = byte_stride;
  transform_rectangular_16bpc::<32, 8>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_32x64_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: dct32_1d,
    scale: RectangularScale::Apply,
    shift: 1,
  };
  let columns = |_, residuals: &mut [i32], _, bounds| transform_scalar_columns(residuals, STRIDE_32, dct64_1d, bounds);
  let stride = byte_stride;
  transform_rectangular_16bpc::<32, 64>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_64x32_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: dct64_1d,
    scale: RectangularScale::Apply,
    shift: 1,
  };
  let columns = |desktop, residuals: &mut [i32], extent, limits: CoefficientBounds| {
    let Ok(avx512) = crate::src::cpu::summon_avx512();
    let wide = match avx512 {
      Some(server) => Some(dct32_cols_avx512(server, residuals, extent, limits)),
      None => None,
    };
    let Ok(dispatch) = scratch_column_dispatch(wide);

    match dispatch {
      ColumnDispatch::Applied => return Ok(()),
      ColumnDispatch::Fallback => {}
    }

    let clipping = VectorBounds {
      lower: _mm256_set1_epi32(limits.lower),
      upper: _mm256_set1_epi32(limits.upper),
    };
    apply_transform_columns_avx2(desktop, residuals, extent, clipping, |column_token, lanes, bounds| {
      dct32_1d_cols8(column_token, lanes, bounds)
    })
  };
  let stride = byte_stride;
  transform_rectangular_16bpc::<64, 32>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_16x64_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: dct16_1d,
    scale: RectangularScale::Skip,
    shift: 2,
  };
  let columns = |_, residuals: &mut [i32], _, bounds| transform_scalar_columns(residuals, STRIDE_16, dct64_1d, bounds);
  let stride = byte_stride;
  transform_rectangular_16bpc::<16, 64>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_64x16_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: dct64_1d,
    scale: RectangularScale::Skip,
    shift: 2,
  };
  let columns = |desktop, residuals: &mut [i32], extent, limits: CoefficientBounds| {
    let Ok(avx512) = crate::src::cpu::summon_avx512();
    let wide = match avx512 {
      Some(server) => Some(dct16_cols_avx512(server, residuals, extent, limits)),
      None => None,
    };
    let Ok(dispatch) = scratch_column_dispatch(wide);

    match dispatch {
      ColumnDispatch::Applied => return Ok(()),
      ColumnDispatch::Fallback => {}
    }

    let clipping = VectorBounds {
      lower: _mm256_set1_epi32(limits.lower),
      upper: _mm256_set1_epi32(limits.upper),
    };
    apply_transform_columns_avx2(desktop, residuals, extent, clipping, |column_token, lanes, bounds| {
      dct16_1d_cols8(column_token, lanes, bounds)
    })
  };
  let stride = byte_stride;
  transform_rectangular_16bpc::<64, 16>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}
