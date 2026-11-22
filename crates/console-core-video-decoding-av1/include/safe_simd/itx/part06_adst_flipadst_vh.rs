#[cfg(target_arch = "x86_64")]
type ScalarItx4 = fn([i32; 4], CoefficientBounds) -> Result<[i32; 4], console_core_never::Never>;

#[cfg(target_arch = "x86_64")]
struct ScalarItx4Passes {
  rows: ScalarItx4,
  columns: ScalarItx4,
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn adst4_1d_scalar(
  input: [i32; 4],
  bounds: CoefficientBounds,
) -> Result<[i32; 4], console_core_never::Never> {
  let Ok(output) = console_core_video_inverse_transforms::av1::inverse_adst4(input);
  Ok(output.map(|sample| sample.clamp(bounds.lower, bounds.upper)))
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn flipadst4_1d_scalar(
  input: [i32; 4],
  bounds: CoefficientBounds,
) -> Result<[i32; 4], console_core_never::Never> {
  let Ok([a, b, c, d]) = adst4_1d_scalar(input, bounds);
  Ok([d, c, b, a])
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn dct4_1d_scalar(
  [in0, in1, in2, in3]: [i32; 4],
  bounds: CoefficientBounds,
) -> Result<[i32; 4], console_core_never::Never> {
  let even_sum = in0.wrapping_add(in2).wrapping_mul(181).wrapping_add(128).wrapping_shr(8);
  let even_difference = in0.wrapping_sub(in2).wrapping_mul(181).wrapping_add(128).wrapping_shr(8);
  let odd_difference = in1.wrapping_mul(1567)
    .wrapping_sub(in3.wrapping_mul(-312))
    .wrapping_add(2048)
    .wrapping_shr(12)
    .wrapping_sub(in3);
  let odd_sum = in1.wrapping_mul(-312)
    .wrapping_add(in3.wrapping_mul(1567))
    .wrapping_add(2048)
    .wrapping_shr(12)
    .wrapping_add(in1);
  let result = [
    even_sum.wrapping_add(odd_sum),
    even_difference.wrapping_add(odd_difference),
    even_difference.wrapping_sub(odd_difference),
    even_sum.wrapping_sub(odd_sum),
  ];
  Ok(result.map(|sample| sample.clamp(bounds.lower, bounds.upper)))
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn identity4_1d_scalar(
  input: [i32; 4],
  _bounds: CoefficientBounds,
) -> Result<[i32; 4], console_core_never::Never> {
  Ok(input.map(|value| {
    let scaled = value.wrapping_mul(1697).wrapping_add(2048).wrapping_shr(12);
    value.wrapping_add(scaled)
  }))
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn transpose_4x4_samples(
  [[a, b, c, d], [e, f, g, h], [i, j, k, l], [m, n, o, p]]: [[i32; 4]; 4],
) -> Result<[[i32; 4]; 4], console_core_never::Never> {
  Ok([[a, e, i, m], [b, f, j, n], [c, g, k, o], [d, h, l, p]])
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn add_scalar_residuals_8bpc(pixels: &mut [u8], residuals: &[i32]) -> Result<(), console_core_never::Never> {
  for (pixel, residual) in pixels.iter_mut().zip(residuals) {
    let delta = residual.wrapping_add(8).wrapping_shr(4);
    let value = i32::from(*pixel).wrapping_add(delta).clamp(0, 255);
    let Ok(clamped) = console_core_number_conversion::wrapped::<_, u8>(value);
    *pixel = clamped;
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn inv_txfm_add_4x4_8bpc(
  dst: &mut [u8],
  dst_base: u64,
  dst_stride: i64,
  coeff: &mut [i16],
  passes: ScalarItx4Passes,
) -> Rav1dResult {
  use console_core_geometry::Size;
  use console_core_pixel_buffers::RowLayout;

  let coefficients = coeff.first_chunk_mut::<16>().ok_or(Rav1dError::EINVAL)?;
  let layout = RowLayout::new(Size { width: 4, height: 4 }, dst_base, dst_stride)
    .map_err(|_| Rav1dError::EINVAL)?;
  let dst_rows = layout.rows_mut(dst).map_err(|_| Rav1dError::EINVAL)?;

  let [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p] = coefficients.map(i32::from);
  let Ok(input) = transpose_4x4_samples([[a, b, c, d], [e, f, g, h], [i, j, k, l], [m, n, o, p]]);
  let bounds = CoefficientBounds { lower: i32::from(i16::MIN), upper: i32::from(i16::MAX) };
  let rows = input.map(|row| {
    let Ok(samples) = (passes.rows)(row, bounds);
    samples.map(|sample| sample.clamp(bounds.lower, bounds.upper))
  });
  let Ok(column_input) = transpose_4x4_samples(rows);
  let columns = column_input.map(|column| {
    let Ok(samples) = (passes.columns)(column, bounds);
    samples
  });
  let Ok(output) = transpose_4x4_samples(columns);

  for (row, residuals) in dst_rows.zip(output) {
    let Ok(()) = add_scalar_residuals_8bpc(row, &residuals);
  }

  coefficients.fill(0);
  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn mixed8_row_dct(token: Desktop64, coeff: &[i16], tmp: &mut [i32; 64]) -> Rav1dResult {
    const MIN: i32 = ::console_core_number_conversion::const_wrapped_integer!(signed ::core::primitive::i16 => ::core::primitive::i32, i16::MIN);
    const MAX: i32 = ::console_core_number_conversion::const_wrapped_integer!(signed ::core::primitive::i16 => ::core::primitive::i32, i16::MAX);
    simd_row_dct8_8bpc_8rows(
      token,
      coeff,
      0,
      tmp,
      RowPass {
        coefficient_height: 8,
        scale: RectangularScale::Skip,
        rounding: 1,
        shift: 1,
        row_bounds: CoefficientBounds { lower: MIN, upper: MAX },
        column_bounds: CoefficientBounds { lower: MIN, upper: MAX },
      },
    )?;

    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn mixed8_row_adst<const FLIPPED: bool>(token: Desktop64, coeff: &[i16], tmp: &mut [i32; 64]) -> Rav1dResult {
    const MIN: i32 = ::console_core_number_conversion::const_wrapped_integer!(signed ::core::primitive::i16 => ::core::primitive::i32, i16::MIN);
    const MAX: i32 = ::console_core_number_conversion::const_wrapped_integer!(signed ::core::primitive::i16 => ::core::primitive::i32, i16::MAX);
    let row_order = match ::core::primitive::bool::then_some(FLIPPED, ()) {
        ::core::option::Option::Some(()) => AdstRowOrder::Reversed,
        ::core::option::Option::None => AdstRowOrder::Normal,
    };
    simd_row_adst8_8bpc_8rows(
      token,
      coeff,
      0,
      row_order,
      tmp,
      RowPass {
        coefficient_height: 8,
        scale: RectangularScale::Skip,
        rounding: 1,
        shift: 1,
        row_bounds: CoefficientBounds { lower: MIN, upper: MAX },
        column_bounds: CoefficientBounds { lower: MIN, upper: MAX },
      },
    )?;

    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn mixed8_row_identity(_token: Desktop64, coeff: &[i16], tmp: &mut [i32; 64]) -> Rav1dResult {
    let samples = coeff.first_chunk::<64>().ok_or(Rav1dError::EINVAL)?;
    let (columns, _) = samples.as_chunks::<8>();
    let mut cols = [_mm256_setzero_si256(); 8];

    for (column, lane) in cols.iter_mut().zip(columns) {
        *column = _mm256_cvtepi16_epi32(loadu_128!(lane));
    }

    let Ok(rows) = transpose_8x8_i32(_token, cols);

    for (tmp_row, row) in tmp.as_chunks_mut::<8>().0.iter_mut().zip(rows) {
        storeu_256!(tmp_row, row);
    }

    Ok(())
}

#[cfg(target_arch = "x86_64")]
enum Mixed8Pass {
  Dct,
  Adst,
  FlipAdst,
  Identity,
}

#[cfg(target_arch = "x86_64")]
struct Mixed8Passes {
  rows: Mixed8Pass,
  columns: Mixed8Pass,
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_mixed_8x8_8bpc(
  token: Desktop64,
  dst: &mut [u8],
  stride: u64,
  coeff: &mut [i16],
  passes: Mixed8Passes,
) -> Rav1dResult {
  let coefficients = coeff.first_chunk_mut::<64>().ok_or(Rav1dError::EINVAL)?;
  let mut tmp = [0i32; 64];

  match passes.rows {
    Mixed8Pass::Dct => mixed8_row_dct(token, coefficients, &mut tmp),
    Mixed8Pass::Adst => mixed8_row_adst::<false>(token, coefficients, &mut tmp),
    Mixed8Pass::FlipAdst => mixed8_row_adst::<true>(token, coefficients, &mut tmp),
    Mixed8Pass::Identity => mixed8_row_identity(token, coefficients, &mut tmp),
  }?;

  let bounds = VectorBounds {
    lower: _mm256_set1_epi32(i32::from(i16::MIN)),
    upper: _mm256_set1_epi32(i32::from(i16::MAX)),
  };
  let (output_rows, _) = tmp.as_chunks_mut::<8>();
  let mut vectors = [_mm256_setzero_si256(); 8];

  for (vector, row) in vectors.iter_mut().zip(output_rows.iter()) {
    *vector = loadu_256!(row);
  }

  match passes.columns {
    Mixed8Pass::Dct => dct8_1d_cols8(token, &mut vectors, bounds),
    Mixed8Pass::Adst => adst8_1d_cols8(token, &mut vectors, bounds),
    Mixed8Pass::FlipAdst => flipadst8_1d_cols8(token, &mut vectors, bounds),
    Mixed8Pass::Identity => identity8_1d_cols8(token, &mut vectors, bounds),
  }

  for (row, vector) in output_rows.iter_mut().zip(vectors) {
    storeu_256!(row, vector);
  }

  let extent = console_core_geometry::Size { width: 8, height: 8 };
  let rows = residual_output_rows(dst, &tmp, stride, extent).map_err(|_| Rav1dError::EINVAL)?;

  for (pixels, residuals) in rows {
    let Ok(()) = add_scalar_residuals_8bpc(pixels, residuals);
  }

  coefficients.fill(0);
  Ok(())
}
