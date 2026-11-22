#[cfg(target_arch = "x86_64")]
#[inline]
fn square_identity_residuals<const SIDE: usize>(
  coefficients: &[i32],
  scale: impl Fn(i32) -> i32,
) -> Rav1dResult<[[i32; SIDE]; SIDE]> {
  let (columns, tail) = coefficients.as_chunks::<SIDE>();
  match bool::then_some(columns.len() == SIDE && tail.is_empty(), ()) {
    Some(()) => {}
    None => return Err(Rav1dError::EINVAL),
  }

  let mut samples = [[0; SIDE]; SIDE];
  for (column, source) in (0u32..).zip(columns) {
    let Ok(position) = console_core_number_conversion::index(column);

    for (row, &value) in samples.iter_mut().zip(source) {
      let sample = row.get_mut(position).ok_or(Rav1dError::EINVAL)?;
      *sample = scale(value);
    }
  }

  Ok(samples)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_identity_add_4x4_16bpc_avx2(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  use console_core_video_interpolation::round_shift;

  let coefficients = coeff.first_chunk_mut::<16>().ok_or(Rav1dError::EINVAL)?;
  let residuals = square_identity_residuals::<4>(coefficients, |value| {
    let identity = |value: i32| value.wrapping_add(round_shift(value.wrapping_mul(1697), 12));
    identity(identity(value))
  })?;
  let stride = byte_stride.wrapping_div(2);
  let extent = console_core_geometry::Size { width: 4, height: 4 };
  add_to_dst_16bpc_avx2(token, dst, stride, residuals.as_flattened(), extent, bitdepth_max)?;
  coefficients.fill(0);
  Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_identity_add_8x8_16bpc_avx2(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  use console_core_video_interpolation::round_shift;

  let coefficients = coeff.first_chunk_mut::<64>().ok_or(Rav1dError::EINVAL)?;
  let lower = (!bitdepth_max).wrapping_shl(5);
  let bounds = CoefficientBounds { lower, upper: !lower };
  let residuals = square_identity_residuals::<8>(coefficients, |value| {
    let row = value.wrapping_mul(2);
    round_shift(row, 1).clamp(bounds.lower, bounds.upper).wrapping_mul(2)
  })?;
  let stride = byte_stride.wrapping_div(2);
  let extent = console_core_geometry::Size { width: 8, height: 8 };
  add_to_dst_16bpc_avx2(token, dst, stride, residuals.as_flattened(), extent, bitdepth_max)?;
  coefficients.fill(0);
  Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_identity_add_16x16_16bpc_avx2(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  use console_core_video_interpolation::round_shift;

  let coefficients = coeff.first_chunk_mut::<256>().ok_or(Rav1dError::EINVAL)?;
  let lower = (!bitdepth_max).wrapping_shl(5);
  let bounds = CoefficientBounds { lower, upper: !lower };
  let residuals = square_identity_residuals::<16>(coefficients, |value| {
    let identity = |coefficient: i32| {
      coefficient.wrapping_mul(2).wrapping_add(round_shift(coefficient.wrapping_mul(1697), 11))
    };
    let column_input = round_shift(identity(value), 2).clamp(bounds.lower, bounds.upper);
    identity(column_input)
  })?;
  let stride = byte_stride.wrapping_div(2);
  let extent = console_core_geometry::Size { width: 16, height: 16 };
  add_to_dst_16bpc_avx2(token, dst, stride, residuals.as_flattened(), extent, bitdepth_max)?;
  coefficients.fill(0);
  Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_4x8_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let plan = NarrowHighTransform {
    rows: identity4_1d,
    columns: identity8_1d,
    scale: RectangularScale::Apply,
    row_shift: 0,
  };
  transform_four_wide_16bpc::<8>(token, dst, byte_stride, coeff, plan, bitdepth_max)
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_8x4_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: identity8_1d,
    scale: RectangularScale::Apply,
    shift: 0,
  };
  let columns = |_, residuals: &mut [i32], _, bounds| transform_scalar_columns(residuals, STRIDE_8, identity4_1d, bounds);
  let stride = byte_stride;
  transform_rectangular_16bpc::<8, 4>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_8x16_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: identity8_1d,
    scale: RectangularScale::Apply,
    shift: 1,
  };
  let columns = |desktop, residuals: &mut [i32], _, _| {
    let Ok(()) = identity16_avx2_columns(desktop, residuals);
    Ok(())
  };
  let stride = byte_stride;
  transform_rectangular_16bpc::<8, 16>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_16x8_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: identity16_1d,
    scale: RectangularScale::Apply,
    shift: 1,
  };
  let columns = |desktop, residuals: &mut [i32], extent, _| {
    let Ok(avx512) = crate::src::cpu::summon_avx512();
    let wide = avx512.map(|server| identity_shift_cols_avx512::<1>(server, residuals, extent));
    let Ok(dispatch) = scratch_column_dispatch(wide);

    match dispatch {
      ColumnDispatch::Applied => return Ok(()),
      ColumnDispatch::Fallback => {}
    }

    let Ok(()) = identity_shift_avx2_columns::<1>(desktop, residuals);
    Ok(())
  };
  let stride = byte_stride;
  transform_rectangular_16bpc::<16, 8>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_4x16_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let plan = NarrowHighTransform {
    rows: identity4_1d,
    columns: identity16_1d,
    scale: RectangularScale::Skip,
    row_shift: 1,
  };
  transform_four_wide_16bpc::<16>(token, dst, byte_stride, coeff, plan, bitdepth_max)
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_16x4_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: identity16_1d,
    scale: RectangularScale::Skip,
    shift: 1,
  };
  let columns = |_, residuals: &mut [i32], _, bounds| transform_scalar_columns(residuals, STRIDE_16, identity4_1d, bounds);
  let stride = byte_stride;
  transform_rectangular_16bpc::<16, 4>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_16x32_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: identity16_1d,
    scale: RectangularScale::Apply,
    shift: 1,
  };
  let columns = |desktop, residuals: &mut [i32], extent, _| {
    let Ok(avx512) = crate::src::cpu::summon_avx512();
    let wide = avx512.map(|server| identity_shift_cols_avx512::<2>(server, residuals, extent));
    let Ok(dispatch) = scratch_column_dispatch(wide);

    match dispatch {
      ColumnDispatch::Applied => return Ok(()),
      ColumnDispatch::Fallback => {}
    }

    let Ok(()) = identity_shift_avx2_columns::<2>(desktop, residuals);
    Ok(())
  };
  let stride = byte_stride;
  transform_rectangular_16bpc::<16, 32>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_32x16_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: identity32_1d,
    scale: RectangularScale::Apply,
    shift: 1,
  };
  let columns = |desktop, residuals: &mut [i32], extent, _| {
    let Ok(avx512) = crate::src::cpu::summon_avx512();
    let wide = avx512.map(|server| identity16_cols_avx512(server, residuals, extent));
    let Ok(dispatch) = scratch_column_dispatch(wide);

    match dispatch {
      ColumnDispatch::Applied => return Ok(()),
      ColumnDispatch::Fallback => {}
    }

    let Ok(()) = identity16_avx2_columns(desktop, residuals);
    Ok(())
  };
  let stride = byte_stride;
  transform_rectangular_16bpc::<32, 16>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_8x32_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: identity8_1d,
    scale: RectangularScale::Skip,
    shift: 2,
  };
  let columns = |desktop, residuals: &mut [i32], _, _| {
    let Ok(()) = identity_shift_avx2_columns::<2>(desktop, residuals);
    Ok(())
  };
  let stride = byte_stride;
  transform_rectangular_16bpc::<8, 32>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_32x8_16bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [u16],
  byte_stride: u64,
  coeff: &mut [i32],
  bitdepth_max: i32,
) -> Rav1dResult {
  let rows = RectangularRows {
    transform: identity32_1d,
    scale: RectangularScale::Skip,
    shift: 2,
  };
  let columns = |desktop, residuals: &mut [i32], extent, _| {
    let Ok(avx512) = crate::src::cpu::summon_avx512();
    let wide = avx512.map(|server| identity_shift_cols_avx512::<1>(server, residuals, extent));
    let Ok(dispatch) = scratch_column_dispatch(wide);

    match dispatch {
      ColumnDispatch::Applied => return Ok(()),
      ColumnDispatch::Fallback => {}
    }

    let Ok(()) = identity_shift_avx2_columns::<1>(desktop, residuals);
    Ok(())
  };
  let stride = byte_stride;
  transform_rectangular_16bpc::<32, 8>(token, dst, stride, coeff, rows, bitdepth_max, columns)
}
