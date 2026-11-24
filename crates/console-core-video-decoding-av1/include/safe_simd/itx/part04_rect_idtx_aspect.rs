#[cfg(target_arch = "x86_64")]
#[inline]
fn rectangular_8bpc_views<'dst, 'coeff>(
    dst: &'dst mut [u8],
    coefficients: &'coeff mut [i16],
    stride: u64,
    extent: console_core_geometry::Size<u32>,
) -> Rav1dResult<(&'dst mut [u8], &'coeff mut [i16])> {
    let width = u64::from(extent.width);

    match stride.cmp(&width) {
        core::cmp::Ordering::Less => return Err(Rav1dError::EINVAL),
        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {}
    }

    let span = u64::from(extent.height).checked_sub(1)
        .and_then(|rows| rows.checked_mul(stride))
        .and_then(|start| start.checked_add(width))
        .ok_or(Rav1dError::EINVAL)?;
    let active = extent.width.min(32).wrapping_mul(extent.height.min(32));
    let Ok(pixels) = console_core_number_conversion::index(span);
    let Ok(samples) = console_core_number_conversion::index(active);

    let destination = dst.get_mut(..pixels).ok_or(Rav1dError::EINVAL)?;
    let input = coefficients.get_mut(..samples).ok_or(Rav1dError::EINVAL)?;
    Ok((destination, input))
}

#[cfg(target_arch = "x86_64")]
fn rectangular_8bpc_rows<const WIDTH: usize, const HEIGHT: usize>(
    coefficients: &[i16],
    transform: Itx1dFn,
    scale: RectangularScale,
    shift: u32,
) -> Rav1dResult<[[i32; WIDTH]; HEIGHT]> {
    let columns = coefficients.as_chunks::<HEIGHT>().0
        .first_chunk::<WIDTH>()
        .ok_or(Rav1dError::EINVAL)?;
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    rectangular_transform_rows(columns, transform, scale, shift, bounds)
}

#[cfg(target_arch = "x86_64")]
fn add_rectangular_pixels_8bpc(
    pixels: &mut [u8],
    stride: u64,
    residuals: &[i32],
    extent: console_core_geometry::Size<u32>,
    bitdepth_max: i32,
) -> Rav1dResult {
    let rows = residual_output_rows(pixels, residuals, stride, extent)?;

    for (row, coefficients) in rows {
        for (pixel, &coefficient) in row.iter_mut().zip(coefficients) {
            let delta = console_core_video_interpolation::round_shift(coefficient, 4);
            let sample = iclip(i32::from(*pixel).wrapping_add(delta), 0, bitdepth_max);
            let Ok(color) = console_core_number_conversion::wrapped::<_, u8>(sample);
            *pixel = color;
        }
    }

    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn identity_shift_columns_8bpc<const SHIFT: i32>(
    _token: Desktop64,
    residuals: &mut [i32],
) -> Rav1dResult {
    let (lanes, remainder) = residuals.as_chunks_mut::<8>();

    match remainder {
        [] => {}
        [_, ..] => return Err(Rav1dError::EINVAL),
    }

    for lane in lanes {
        let values = loadu_256!(&*lane);
        storeu_256!(lane, _mm256_slli_epi32::<SHIFT>(values));
    }

    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn identity16_columns_8bpc(
    _token: Desktop64,
    residuals: &mut [i32],
) -> Rav1dResult {
    let (lanes, remainder) = residuals.as_chunks_mut::<8>();

    match remainder {
        [] => {}
        [_, ..] => return Err(Rav1dError::EINVAL),
    }

    let factor = _mm256_set1_epi32(1697);
    let half = _mm256_set1_epi32(1024);

    for lane in lanes {
        let values = loadu_256!(&*lane);
        let twice = _mm256_slli_epi32::<1>(values);
        let extra = _mm256_mullo_epi32(values, factor);
        let offset = _mm256_srai_epi32::<11>(_mm256_add_epi32(extra, half));
        storeu_256!(lane, _mm256_add_epi32(twice, offset));
    }

    Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_16x32_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 16, height: 32 };
    let stride = dst_stride;
    let (pixels, active) = rectangular_8bpc_views(dst, coeff, stride, extent)?;
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    let pass = RowPass {
        coefficient_height: 32,
        scale: RectangularScale::Apply,
        rounding: 1,
        shift: 1,
        row_bounds: bounds.rows,
        column_bounds: bounds.columns,
    };
    let mut scratch = [0i32; 512];
    row_dct16_8bpc_block(token, active, extent.height, &mut scratch, pass)?;
    let residuals = scratch.as_mut_slice();
    let Ok(avx512) = crate::src::cpu::summon_avx512();

    match avx512 {
        Some(wide) => {
            dct32_cols_avx512(wide, residuals, extent, bounds.columns)?;
        }
        None => {
            let clipping = VectorBounds {
                lower: _mm256_set1_epi32(bounds.columns.lower),
                upper: _mm256_set1_epi32(bounds.columns.upper),
            };
            apply_transform_columns_avx2(token, residuals, extent, clipping, |column_token, values, limits| {
                dct32_1d_cols8_i16(column_token, values, limits)
            })?;
        }
    }

    add_to_dst_8bpc_avx2(token, pixels, stride, residuals, extent, bitdepth_max)?;
    active.fill(0);
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
struct RowPass {
  coefficient_height: u32,
  scale: RectangularScale,
  rounding: i32,
  shift: i32,
  row_bounds: CoefficientBounds,
  column_bounds: CoefficientBounds,
}

#[cfg(target_arch = "x86_64")]
impl RowPass {
  fn coefficient_lane<const LANES: usize>(
    self,
    coefficients: &[i16],
    origin: console_core_geometry::Point<u32>,
  ) -> Rav1dResult<&[i16; LANES]> {
    let lanes = u32::try_from(LANES).map_err(|_| Rav1dError::EINVAL)?;
    let row_end = origin.y.checked_add(lanes).ok_or(Rav1dError::EINVAL)?;

    match row_end.cmp(&self.coefficient_height) {
      core::cmp::Ordering::Greater => return Err(Rav1dError::EINVAL),
      core::cmp::Ordering::Equal | core::cmp::Ordering::Less => {}
    }

    let start = origin.x.checked_mul(self.coefficient_height)
      .and_then(|offset| offset.checked_add(origin.y))
      .ok_or(Rav1dError::EINVAL)?;
    let Ok(first) = console_core_number_conversion::index(start);
    coefficients.get(first..).and_then(<[_]>::first_chunk::<LANES>).ok_or(Rav1dError::EINVAL)
  }
}

#[cfg(target_arch = "x86_64")]
fn checked_scratch_rows(
  scratch: &mut [i32],
  width: NonZeroU32,
  rows: core::ops::Range<u32>,
) -> Rav1dResult<&mut [i32]> {
  let start = rows.start.checked_mul(width.get()).ok_or(Rav1dError::EINVAL)?;
  let end = rows.end.checked_mul(width.get()).ok_or(Rav1dError::EINVAL)?;
  let Ok(range) = console_core_number_conversion::indices(start..end);
  scratch.get_mut(range).ok_or(Rav1dError::EINVAL)
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn load_avx2_row_lanes<const WIDTH: usize>(
  _token: Desktop64,
  coefficients: &[i16],
  first_row: u32,
  pass: RowPass,
) -> Rav1dResult<[__m256i; WIDTH]> {
  let multiplier = _mm256_set1_epi32(181);
  let bias = _mm256_set1_epi32(128);
  let mut vectors = [_mm256_setzero_si256(); WIDTH];

  for (output, column) in vectors.iter_mut().zip(0u32..) {
    let location = console_core_geometry::Point { x: column, y: first_row };
    let lane = pass.coefficient_lane::<8>(coefficients, location)?;
    let values = _mm256_cvtepi16_epi32(loadu_128!(lane));
    *output = match pass.scale {
      RectangularScale::Apply => _mm256_srai_epi32::<8>(_mm256_add_epi32(_mm256_mullo_epi32(values, multiplier), bias)),
      RectangularScale::Skip => values,
    };
  }

  Ok(vectors)
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn round_avx2_row_lanes<const WIDTH: usize>(
  _token: Desktop64,
  vectors: &mut [__m256i; WIDTH],
  pass: RowPass,
) -> Result<(), console_core_never::Never> {
  let lower = _mm256_set1_epi32(pass.column_bounds.lower);
  let upper = _mm256_set1_epi32(pass.column_bounds.upper);
  let rounding = _mm256_set1_epi32(pass.rounding);

  for vector in vectors {
    let with_bias = _mm256_add_epi32(*vector, rounding);
    let rounded = match (WIDTH, pass.shift) {
      (_, 1) => _mm256_srai_epi32::<1>(with_bias),
      (_, 2) => _mm256_srai_epi32::<2>(with_bias),
      (32, _) | (_, 0) => with_bias,
      (_, _) => _mm256_srai_epi32::<2>(with_bias),
    };
    *vector = _mm256_max_epi32(_mm256_min_epi32(rounded, upper), lower);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn store_avx2_row_lanes<const WIDTH: usize>(
  _token: Desktop64,
  scratch: &mut [i32],
  first_row: u32,
  vectors: &[__m256i; WIDTH],
) -> Rav1dResult {
  let width = u32::try_from(WIDTH).map_err(|_| Rav1dError::EINVAL)?;
  let row_width = NonZeroU32::new(width).ok_or(Rav1dError::EINVAL)?;
  let (strips, remainder) = vectors.as_chunks::<8>();

  match remainder {
      [] => {}
      [_, ..] => return Err(Rav1dError::EINVAL),
  }

  let last_row = first_row.checked_add(8).ok_or(Rav1dError::EINVAL)?;
  let active = checked_scratch_rows(scratch, row_width, first_row..last_row)?;
  let Ok(stride) = console_core_number_conversion::index(row_width.get());

  for (columns, segment) in strips.iter().zip(0u32..) {
    let Ok(rows) = transpose_8x8_i32(_token, *columns);
    let Ok(offset) = console_core_number_conversion::index(segment.wrapping_mul(8));

    for (output, row) in active.chunks_exact_mut(stride.max(1)).zip(rows) {
      let lane = output.get_mut(offset..).and_then(<[_]>::first_chunk_mut::<8>);
      let pixels = lane.ok_or(Rav1dError::EINVAL)?;
      storeu_256!(pixels, row);
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
#[inline(always)]
fn simd_row_dct8_8bpc_8rows(
  token: Desktop64,
  coefficients: &[i16],
  first_row: u32,
  scratch: &mut [i32],
  pass: RowPass,
) -> Rav1dResult {
  let mut vectors = load_avx2_row_lanes::<8>(token, coefficients, first_row, pass)?;
  let bounds = VectorBounds {
    lower: _mm256_set1_epi32(pass.row_bounds.lower),
    upper: _mm256_set1_epi32(pass.row_bounds.upper),
  };
  let Ok(()) = dct8_1d_cols8(token, &mut vectors, bounds);
  let Ok(()) = round_avx2_row_lanes(token, &mut vectors, pass);

  store_avx2_row_lanes(token, scratch, first_row, &vectors)
}

#[cfg(target_arch = "x86_64")]
#[rite]
#[inline(always)]
fn simd_row_dct16_8bpc_8rows(
  token: Desktop64,
  coefficients: &[i16],
  first_row: u32,
  scratch: &mut [i32],
  pass: RowPass,
) -> Rav1dResult {
  let mut vectors = load_avx2_row_lanes::<16>(token, coefficients, first_row, pass)?;
  let bounds = VectorBounds {
    lower: _mm256_set1_epi32(pass.row_bounds.lower),
    upper: _mm256_set1_epi32(pass.row_bounds.upper),
  };
  let Ok(()) = dct16_1d_cols8(token, &mut vectors, bounds);
  let Ok(()) = round_avx2_row_lanes(token, &mut vectors, pass);

  store_avx2_row_lanes(token, scratch, first_row, &vectors)
}

#[cfg(target_arch = "x86_64")]
#[rite]
#[inline(always)]
fn simd_row_dct32_8bpc_8rows(
  token: Desktop64,
  coefficients: &[i16],
  first_row: u32,
  scratch: &mut [i32],
  pass: RowPass,
) -> Rav1dResult {
  let mut vectors = load_avx2_row_lanes::<32>(token, coefficients, first_row, pass)?;
  let bounds = VectorBounds {
    lower: _mm256_set1_epi32(pass.row_bounds.lower),
    upper: _mm256_set1_epi32(pass.row_bounds.upper),
  };
  let Ok(()) = dct32_1d_cols8_i16(token, &mut vectors, bounds);
  let Ok(()) = round_avx2_row_lanes(token, &mut vectors, pass);

  store_avx2_row_lanes(token, scratch, first_row, &vectors)
}

#[cfg(target_arch = "x86_64")]
#[rite]
#[inline(always)]
fn simd_row_adst8_8bpc_8rows(
  token: Desktop64,
  coefficients: &[i16],
  first_row: u32,
  order: AdstRowOrder,
  scratch: &mut [i32],
  pass: RowPass,
) -> Rav1dResult {
  let mut vectors = load_avx2_row_lanes::<8>(token, coefficients, first_row, pass)?;
  let bounds = VectorBounds {
    lower: _mm256_set1_epi32(pass.row_bounds.lower),
    upper: _mm256_set1_epi32(pass.row_bounds.upper),
  };
  let Ok(()) = adst8_1d_cols8(token, &mut vectors, bounds);

  match order {
    AdstRowOrder::Reversed => vectors.reverse(),
    AdstRowOrder::Normal => {}
  }

  let Ok(()) = round_avx2_row_lanes(token, &mut vectors, pass);

  store_avx2_row_lanes(token, scratch, first_row, &vectors)
}

#[cfg(target_arch = "x86_64")]
#[rite]
#[inline(always)]
fn simd_row_adst16_8bpc_8rows(
  token: Desktop64,
  coefficients: &[i16],
  first_row: u32,
  order: AdstRowOrder,
  scratch: &mut [i32],
  pass: RowPass,
) -> Rav1dResult {
  let mut vectors = load_avx2_row_lanes::<16>(token, coefficients, first_row, pass)?;
  let bounds = VectorBounds {
    lower: _mm256_set1_epi32(pass.row_bounds.lower),
    upper: _mm256_set1_epi32(pass.row_bounds.upper),
  };
  let Ok(()) = adst16_1d_cols8(token, &mut vectors, bounds);

  match order {
    AdstRowOrder::Reversed => vectors.reverse(),
    AdstRowOrder::Normal => {}
  }

  let Ok(()) = round_avx2_row_lanes(token, &mut vectors, pass);

  store_avx2_row_lanes(token, scratch, first_row, &vectors)
}

#[cfg(target_arch = "x86_64")]
#[rite]
#[inline(always)]
fn dct8_row_build_pair(_token: Desktop64, a: __m128i, b: __m128i) -> Result<__m256i, Never> {
    let lo = _mm_unpacklo_epi16(a, b);
    let hi = _mm_unpackhi_epi16(a, b);
    Ok(_mm256_set_m128i(hi, lo))
}





#[cfg(target_arch = "x86_64")]
struct CoefficientPair {
    low: i16,
    high: i16,
}

#[cfg(target_arch = "x86_64")]
#[rite]
#[inline(always)]
fn dct8_row_coef_pack(_token: Desktop64, pair: CoefficientPair) -> Result<__m256i, Never> {
    let [low_a, low_b] = pair.low.to_le_bytes();
    let [high_a, high_b] = pair.high.to_le_bytes();
    let packed = i32::from_le_bytes([low_a, low_b, high_a, high_b]);
    Ok(_mm256_set1_epi32(packed))
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn dct8_row_pass_i16_simd(_token: Desktop64, coeff_col_major: [i16; 64]) -> Result<[i32; 64], Never> {
    let mut col_xmm = [_mm_setzero_si128(); 8];

    for (vector, lane) in col_xmm.iter_mut().zip(coeff_col_major.as_chunks::<8>().0) {
        *vector = loadu_128!(lane);
    }

    let Ok(pair_17) = dct8_row_build_pair(_token, match col_xmm { [_, _explicit_element, ..] => _explicit_element }, match col_xmm { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
    let Ok(pair_53) = dct8_row_build_pair(_token, match col_xmm { [_, _, _, _, _, _explicit_element, ..] => _explicit_element }, match col_xmm { [_, _, _, _explicit_element, ..] => _explicit_element });
    let Ok(pair_26) = dct8_row_build_pair(_token, match col_xmm { [_, _, _explicit_element, ..] => _explicit_element }, match col_xmm { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
    let Ok(pair_04) = dct8_row_build_pair(_token, match col_xmm { [_explicit_element, ..] => _explicit_element }, match col_xmm { [_, _, _, _, _explicit_element, ..] => _explicit_element });

    let pd_2048 = _mm256_set1_epi32(2048);
    let pd_128 = _mm256_set1_epi32(128);













    let t4a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_17, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 799, high: -4017 }); packed_vector }),
        pd_2048,
    ));
    let t7a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_17, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 4017, high: 799 }); packed_vector }),
        pd_2048,
    ));
    let t5a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_53, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3406, high: -2276 }); packed_vector }),
        pd_2048,
    ));
    let t6a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_53, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 2276, high: 3406 }); packed_vector }),
        pd_2048,
    ));
    let t2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 1567, high: -3784 }); packed_vector }),
        pd_2048,
    ));
    let t3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3784, high: 1567 }); packed_vector }),
        pd_2048,
    ));
    let t0 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_04, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
        pd_128,
    ));
    let t1 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_04, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
        pd_128,
    ));



    let row_min = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN);
    let row_max = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX);
    let row_min_v = _mm256_set1_epi32(row_min);
    let row_max_v = _mm256_set1_epi32(row_max);
    let clip = |v: __m256i| _mm256_max_epi32(_mm256_min_epi32(v, row_max_v), row_min_v);






    let t4 = clip(_mm256_add_epi32(t4a, t5a));
    let t5a_n = clip(_mm256_sub_epi32(t4a, t5a));
    let t7 = clip(_mm256_add_epi32(t7a, t6a));
    let t6a_n = clip(_mm256_sub_epi32(t7a, t6a));




    let c_181 = _mm256_set1_epi32(181);
    let d = _mm256_sub_epi32(t6a_n, t5a_n);
    let t5 = _mm256_srai_epi32::<8>(_mm256_add_epi32(_mm256_mullo_epi32(d, c_181), pd_128));
    let s = _mm256_add_epi32(t6a_n, t5a_n);
    let t6 = _mm256_srai_epi32::<8>(_mm256_add_epi32(_mm256_mullo_epi32(s, c_181), pd_128));






    let tmp0 = clip(_mm256_add_epi32(t0, t3));
    let tmp1 = clip(_mm256_add_epi32(t1, t2));
    let tmp2 = clip(_mm256_sub_epi32(t1, t2));
    let tmp3 = clip(_mm256_sub_epi32(t0, t3));










    let mut cols = [_mm256_setzero_si256(); 8];
    *{ let [_explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(tmp0, t7));
    *{ let [_, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(tmp1, t6));
    *{ let [_, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(tmp2, t5));
    *{ let [_, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(tmp3, t4));
    *{ let [_, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(tmp3, t4));
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(tmp2, t5));
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(tmp1, t6));
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(tmp0, t7));


    let Ok(rows) = transpose_8x8_i32(_token, cols);
    let mut out = [0i32; 64];

    for (output, vector) in out.as_chunks_mut::<8>().0.iter_mut().zip(rows) {
        storeu_256!(output, vector);
    }

    Ok(out)
}















#[cfg(target_arch = "x86_64")]
#[rite]
fn dct8_col_pass_i16(_token: Desktop64, tmp_row_major: &[i32; 64]) -> Result<[__m256i; 8], Never> {


    let mut row_xmm = [_mm_setzero_si128(); 8];

    for (row, lane) in row_xmm.iter_mut().zip(tmp_row_major.as_chunks::<8>().0) {
        let vector = loadu_256!(lane);
        let low = _mm256_castsi256_si128(vector);
        let high = _mm256_extracti128_si256::<1>(vector);
        *row = _mm_packs_epi32(low, high);
    }

    let Ok(pair_17) = dct8_row_build_pair(_token, match row_xmm { [_, _explicit_element, ..] => _explicit_element }, match row_xmm { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
    let Ok(pair_53) = dct8_row_build_pair(_token, match row_xmm { [_, _, _, _, _, _explicit_element, ..] => _explicit_element }, match row_xmm { [_, _, _, _explicit_element, ..] => _explicit_element });
    let Ok(pair_26) = dct8_row_build_pair(_token, match row_xmm { [_, _, _explicit_element, ..] => _explicit_element }, match row_xmm { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
    let Ok(pair_04) = dct8_row_build_pair(_token, match row_xmm { [_explicit_element, ..] => _explicit_element }, match row_xmm { [_, _, _, _, _explicit_element, ..] => _explicit_element });

    let pd_2048 = _mm256_set1_epi32(2048);
    let pd_128 = _mm256_set1_epi32(128);










    let t4a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_17, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 799, high: -4017 }); packed_vector }),
        pd_2048,
    ));
    let t7a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_17, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 4017, high: 799 }); packed_vector }),
        pd_2048,
    ));
    let t5a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_53, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3406, high: -2276 }); packed_vector }),
        pd_2048,
    ));
    let t6a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_53, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 2276, high: 3406 }); packed_vector }),
        pd_2048,
    ));
    let t2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 1567, high: -3784 }); packed_vector }),
        pd_2048,
    ));
    let t3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3784, high: 1567 }); packed_vector }),
        pd_2048,
    ));
    let t0 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_04, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
        pd_128,
    ));
    let t1 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_04, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
        pd_128,
    ));


    let col_min = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN);
    let col_max = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX);
    let col_min_v = _mm256_set1_epi32(col_min);
    let col_max_v = _mm256_set1_epi32(col_max);
    let clip = |v: __m256i| _mm256_max_epi32(_mm256_min_epi32(v, col_max_v), col_min_v);


    let t4 = clip(_mm256_add_epi32(t4a, t5a));
    let t5a_n = clip(_mm256_sub_epi32(t4a, t5a));
    let t7 = clip(_mm256_add_epi32(t7a, t6a));
    let t6a_n = clip(_mm256_sub_epi32(t7a, t6a));






    let t5a_n_xmm = _mm_packs_epi32(
        _mm256_castsi256_si128(t5a_n),
        _mm256_extracti128_si256(t5a_n, 1),
    );
    let t6a_n_xmm = _mm_packs_epi32(
        _mm256_castsi256_si128(t6a_n),
        _mm256_extracti128_si256(t6a_n, 1),
    );
    let Ok(pair_65) = dct8_row_build_pair(_token, t6a_n_xmm, t5a_n_xmm);
    let t5 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_65, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
        pd_128,
    ));
    let t6 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_65, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
        pd_128,
    ));



    let tmp0 = clip(_mm256_add_epi32(t0, t3));
    let tmp1 = clip(_mm256_add_epi32(t1, t2));
    let tmp2 = clip(_mm256_sub_epi32(t1, t2));
    let tmp3 = clip(_mm256_sub_epi32(t0, t3));


    let mut out = [_mm256_setzero_si256(); 8];
    *{ let [_explicit_part, ..] = &mut out; _explicit_part } = clip(_mm256_add_epi32(tmp0, t7));
    *{ let [_, _explicit_part, ..] = &mut out; _explicit_part } = clip(_mm256_add_epi32(tmp1, t6));
    *{ let [_, _, _explicit_part, ..] = &mut out; _explicit_part } = clip(_mm256_add_epi32(tmp2, t5));
    *{ let [_, _, _, _explicit_part, ..] = &mut out; _explicit_part } = clip(_mm256_add_epi32(tmp3, t4));
    *{ let [_, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = clip(_mm256_sub_epi32(tmp3, t4));
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = clip(_mm256_sub_epi32(tmp2, t5));
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = clip(_mm256_sub_epi32(tmp1, t6));
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut out; _explicit_part } = clip(_mm256_sub_epi32(tmp0, t7));

    Ok(out)
}




















#[cfg(target_arch = "x86_64")]
#[rite]
fn dct16_col_pass_i16(_token: Desktop64, tmp_row_major: &[i32; 256]) -> Result<[i32; 256], Never> {
    let mut result = [0i32; 256];
    let col_min = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN);
    let col_max = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX);
    let col_min_v = _mm256_set1_epi32(col_min);
    let col_max_v = _mm256_set1_epi32(col_max);
    let clip = |v: __m256i| _mm256_max_epi32(_mm256_min_epi32(v, col_max_v), col_min_v);

    let pd_2048 = _mm256_set1_epi32(2048);
    let pd_128 = _mm256_set1_epi32(128);


    let coefficient_strips = tmp_row_major.as_chunks::<8>().0;
    let residual_strips = result.as_chunks_mut::<8>().0;

    for strip in 0u8..2 {
        let Ok(first) = console_core_number_conversion::index(strip);
        let samples = coefficient_strips.iter().skip(first).step_by(2);
        let mut row_xmm = [_mm_setzero_si128(); 16];

        for (row, lane) in row_xmm.iter_mut().zip(samples) {
            let vector = loadu_256!(lane);
            let low = _mm256_castsi256_si128(vector);
            let high = _mm256_extracti128_si256::<1>(vector);
            *row = _mm_packs_epi32(low, high);
        }

        let Ok(pair_0_8) = dct8_row_build_pair(_token, match row_xmm { [_explicit_element, ..] => _explicit_element }, match row_xmm { [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
        let Ok(pair_4_12) = dct8_row_build_pair(_token, match row_xmm { [_, _, _, _, _explicit_element, ..] => _explicit_element }, match row_xmm { [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });


        let e_t0 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_0_8, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
            pd_128,
        ));

        let e_t1 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_0_8, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
            pd_128,
        ));

        let e_t2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_4_12, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 1567, high: -3784 }); packed_vector }),
            pd_2048,
        ));

        let e_t3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_4_12, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3784, high: 1567 }); packed_vector }),
            pd_2048,
        ));


        let dct4_0 = clip(_mm256_add_epi32(e_t0, e_t3));
        let dct4_1 = clip(_mm256_add_epi32(e_t1, e_t2));
        let dct4_2 = clip(_mm256_sub_epi32(e_t1, e_t2));
        let dct4_3 = clip(_mm256_sub_epi32(e_t0, e_t3));


        let Ok(pair_2_14) = dct8_row_build_pair(_token, match row_xmm { [_, _, _explicit_element, ..] => _explicit_element }, match row_xmm { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
        let Ok(pair_10_6) = dct8_row_build_pair(_token, match row_xmm { [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match row_xmm { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element });


        let t4a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_2_14, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 799, high: -4017 }); packed_vector }),
            pd_2048,
        ));

        let t7a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_2_14, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 4017, high: 799 }); packed_vector }),
            pd_2048,
        ));

        let t5a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_10_6, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3406, high: -2276 }); packed_vector }),
            pd_2048,
        ));

        let t6a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_10_6, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 2276, high: 3406 }); packed_vector }),
            pd_2048,
        ));


        let e_t4 = clip(_mm256_add_epi32(t4a, t5a));
        let e_t5a_n = clip(_mm256_sub_epi32(t4a, t5a));
        let e_t7 = clip(_mm256_add_epi32(t7a, t6a));
        let e_t6a_n = clip(_mm256_sub_epi32(t7a, t6a));


        let e_t5a_n_xmm = _mm_packs_epi32(
            _mm256_castsi256_si128(e_t5a_n),
            _mm256_extracti128_si256(e_t5a_n, 1),
        );
        let e_t6a_n_xmm = _mm_packs_epi32(
            _mm256_castsi256_si128(e_t6a_n),
            _mm256_extracti128_si256(e_t6a_n, 1),
        );
        let Ok(pair_65) = dct8_row_build_pair(_token, e_t6a_n_xmm, e_t5a_n_xmm);

        let e_t5 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_65, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
            pd_128,
        ));

        let e_t6 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_65, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
            pd_128,
        ));


        let even_0 = clip(_mm256_add_epi32(dct4_0, e_t7));
        let even_1 = clip(_mm256_add_epi32(dct4_1, e_t6));
        let even_2 = clip(_mm256_add_epi32(dct4_2, e_t5));
        let even_3 = clip(_mm256_add_epi32(dct4_3, e_t4));
        let even_4 = clip(_mm256_sub_epi32(dct4_3, e_t4));
        let even_5 = clip(_mm256_sub_epi32(dct4_2, e_t5));
        let even_6 = clip(_mm256_sub_epi32(dct4_1, e_t6));
        let even_7 = clip(_mm256_sub_epi32(dct4_0, e_t7));




        let Ok(pair_1_15) = dct8_row_build_pair(_token, match row_xmm { [_, _explicit_element, ..] => _explicit_element }, match row_xmm { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
        let Ok(pair_9_7) = dct8_row_build_pair(_token, match row_xmm { [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match row_xmm { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
        let Ok(pair_5_11) = dct8_row_build_pair(_token, match row_xmm { [_, _, _, _, _, _explicit_element, ..] => _explicit_element }, match row_xmm { [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
        let Ok(pair_13_3) = dct8_row_build_pair(_token, match row_xmm { [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match row_xmm { [_, _, _, _explicit_element, ..] => _explicit_element });


        let o_t8a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_1_15, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 401, high: -4076 }); packed_vector }),
            pd_2048,
        ));

        let o_t15a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_1_15, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 4076, high: 401 }); packed_vector }),
            pd_2048,
        ));

        let o_t9a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_9_7, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3166, high: -2598 }); packed_vector }),
            pd_2048,
        ));

        let o_t14a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_9_7, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 2598, high: 3166 }); packed_vector }),
            pd_2048,
        ));

        let o_t10a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_5_11, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 1931, high: -3612 }); packed_vector }),
            pd_2048,
        ));

        let o_t13a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_5_11, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3612, high: 1931 }); packed_vector }),
            pd_2048,
        ));

        let o_t11a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_13_3, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3920, high: -1189 }); packed_vector }),
            pd_2048,
        ));

        let o_t12a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_13_3, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 1189, high: 3920 }); packed_vector }),
            pd_2048,
        ));


        let o_t8 = clip(_mm256_add_epi32(o_t8a, o_t9a));
        let o_t9 = clip(_mm256_sub_epi32(o_t8a, o_t9a));
        let o_t10 = clip(_mm256_sub_epi32(o_t11a, o_t10a));
        let o_t11 = clip(_mm256_add_epi32(o_t11a, o_t10a));
        let o_t12 = clip(_mm256_add_epi32(o_t12a, o_t13a));
        let o_t13 = clip(_mm256_sub_epi32(o_t12a, o_t13a));
        let o_t14 = clip(_mm256_sub_epi32(o_t15a, o_t14a));
        let o_t15 = clip(_mm256_add_epi32(o_t15a, o_t14a));


        let o_t14_xmm = _mm_packs_epi32(
            _mm256_castsi256_si128(o_t14),
            _mm256_extracti128_si256(o_t14, 1),
        );
        let o_t9_xmm = _mm_packs_epi32(
            _mm256_castsi256_si128(o_t9),
            _mm256_extracti128_si256(o_t9, 1),
        );
        let Ok(pair_14_9) = dct8_row_build_pair(_token, o_t14_xmm, o_t9_xmm);


        let o_t9a_new = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_14_9, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 1567, high: -3784 }); packed_vector }),
            pd_2048,
        ));

        let o_t14a_new = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_14_9, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3784, high: 1567 }); packed_vector }),
            pd_2048,
        ));

        let o_t13_xmm = _mm_packs_epi32(
            _mm256_castsi256_si128(o_t13),
            _mm256_extracti128_si256(o_t13, 1),
        );
        let o_t10_xmm = _mm_packs_epi32(
            _mm256_castsi256_si128(o_t10),
            _mm256_extracti128_si256(o_t10, 1),
        );
        let Ok(pair_13_10) = dct8_row_build_pair(_token, o_t13_xmm, o_t10_xmm);


        let o_t10a_new = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_13_10, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: -3784, high: -1567 }); packed_vector }),
            pd_2048,
        ));

        let o_t13a_new = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_13_10, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 1567, high: -3784 }); packed_vector }),
            pd_2048,
        ));


        let o_t8a_f = clip(_mm256_add_epi32(o_t8, o_t11));
        let o_t9_f = clip(_mm256_add_epi32(o_t9a_new, o_t10a_new));
        let o_t10_f = clip(_mm256_sub_epi32(o_t9a_new, o_t10a_new));
        let o_t11a_f = clip(_mm256_sub_epi32(o_t8, o_t11));
        let o_t12a_f = clip(_mm256_sub_epi32(o_t15, o_t12));
        let o_t13_f = clip(_mm256_sub_epi32(o_t14a_new, o_t13a_new));
        let o_t14_f = clip(_mm256_add_epi32(o_t14a_new, o_t13a_new));
        let o_t15a_f = clip(_mm256_add_epi32(o_t15, o_t12));


        let o_t13_f_xmm = _mm_packs_epi32(
            _mm256_castsi256_si128(o_t13_f),
            _mm256_extracti128_si256(o_t13_f, 1),
        );
        let o_t10_f_xmm = _mm_packs_epi32(
            _mm256_castsi256_si128(o_t10_f),
            _mm256_extracti128_si256(o_t10_f, 1),
        );
        let Ok(pair_13f_10f) = dct8_row_build_pair(_token, o_t13_f_xmm, o_t10_f_xmm);


        let o_t10a_f = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_13f_10f, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
            pd_128,
        ));

        let o_t13a_f = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_13f_10f, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
            pd_128,
        ));

        let o_t12a_f_xmm = _mm_packs_epi32(
            _mm256_castsi256_si128(o_t12a_f),
            _mm256_extracti128_si256(o_t12a_f, 1),
        );
        let o_t11a_f_xmm = _mm_packs_epi32(
            _mm256_castsi256_si128(o_t11a_f),
            _mm256_extracti128_si256(o_t11a_f, 1),
        );
        let Ok(pair_12a_11a) = dct8_row_build_pair(_token, o_t12a_f_xmm, o_t11a_f_xmm);


        let o_t11_f = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_12a_11a, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
            pd_128,
        ));

        let o_t12_f = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_12a_11a, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
            pd_128,
        ));











        let odd = [
            o_t15a_f, o_t14_f, o_t13a_f, o_t12_f, o_t11_f, o_t10a_f, o_t9_f, o_t8a_f,
        ];
        let even = [
            even_0, even_1, even_2, even_3, even_4, even_5, even_6, even_7,
        ];

        let mut cols = [[_mm256_setzero_si256(); 8]; 2];
        let [first_half, second_half] = &mut cols;

        for ((left, right), (a, b)) in first_half.iter_mut()
            .zip(second_half.iter_mut().rev()).zip(even.into_iter().zip(odd))
        {
            *left = clip(_mm256_add_epi32(a, b));
            *right = clip(_mm256_sub_epi32(a, b));
        }



        let destination = residual_strips.iter_mut().skip(first).step_by(2);

        for (lane, &vector) in destination.zip(cols.as_flattened()) {
            storeu_256!(lane, vector);
        }
    }

    Ok(result)
}













#[cfg(target_arch = "x86_64")]
#[arcane]
fn dct16_row_pass_i16_simd(_token: Desktop64, coeff_col_major: [i16; 256]) -> Result<[i32; 256], Never> {
    let mut out = [0i32; 256];

    let row_min = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN);
    let row_max = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX);
    let row_min_v = _mm256_set1_epi32(row_min);
    let row_max_v = _mm256_set1_epi32(row_max);
    let clip = |v: __m256i| _mm256_max_epi32(_mm256_min_epi32(v, row_max_v), row_min_v);

    let pd_2048 = _mm256_set1_epi32(2048);
    let pd_128 = _mm256_set1_epi32(128);
    let c_181 = _mm256_set1_epi32(181);

    let coefficient_strips = coeff_col_major.as_chunks::<8>().0;
    let row_groups = out.as_chunks_mut::<128>().0;

    for (batch, output) in (0u8..).zip(row_groups) {
        let Ok(first) = console_core_number_conversion::index(batch);
        let input = coefficient_strips.iter().skip(first).step_by(2);
        let mut col_xmm = [_mm_setzero_si128(); 16];
        let mut nz = _mm_setzero_si128();

        for (destination, lane) in col_xmm.iter_mut().zip(input) {
            let vector = loadu_128!(lane);
            *destination = vector;
            nz = _mm_or_si128(nz, vector);
        }

         match ::core::primitive::bool::then_some(_mm_testz_si128(nz, nz) != 0, ()) { ::core::option::Option::Some(()) => {
            continue;
        }, ::core::option::Option::None => {} }














        let Ok(pair_0_8) = dct8_row_build_pair(_token, match col_xmm { [_explicit_element, ..] => _explicit_element }, match col_xmm { [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
        let Ok(pair_4_12) = dct8_row_build_pair(_token, match col_xmm { [_, _, _, _, _explicit_element, ..] => _explicit_element }, match col_xmm { [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });


        let e_t0 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_0_8, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
            pd_128,
        ));

        let e_t1 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_0_8, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
            pd_128,
        ));

        let e_t2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_4_12, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 1567, high: -3784 }); packed_vector }),
            pd_2048,
        ));

        let e_t3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_4_12, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3784, high: 1567 }); packed_vector }),
            pd_2048,
        ));


        let dct4_0 = clip(_mm256_add_epi32(e_t0, e_t3));
        let dct4_1 = clip(_mm256_add_epi32(e_t1, e_t2));
        let dct4_2 = clip(_mm256_sub_epi32(e_t1, e_t2));
        let dct4_3 = clip(_mm256_sub_epi32(e_t0, e_t3));


        let Ok(pair_2_14) = dct8_row_build_pair(_token, match col_xmm { [_, _, _explicit_element, ..] => _explicit_element }, match col_xmm { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
        let Ok(pair_10_6) = dct8_row_build_pair(_token, match col_xmm { [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match col_xmm { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element });


        let t4a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_2_14, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 799, high: -4017 }); packed_vector }),
            pd_2048,
        ));

        let t7a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_2_14, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 4017, high: 799 }); packed_vector }),
            pd_2048,
        ));

        let t5a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_10_6, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3406, high: -2276 }); packed_vector }),
            pd_2048,
        ));

        let t6a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_10_6, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 2276, high: 3406 }); packed_vector }),
            pd_2048,
        ));


        let t4 = clip(_mm256_add_epi32(t4a, t5a));
        let t5a_n = clip(_mm256_sub_epi32(t4a, t5a));
        let t7 = clip(_mm256_add_epi32(t7a, t6a));
        let t6a_n = clip(_mm256_sub_epi32(t7a, t6a));


        let d_65 = _mm256_sub_epi32(t6a_n, t5a_n);
        let t5 = _mm256_srai_epi32::<8>(_mm256_add_epi32(_mm256_mullo_epi32(d_65, c_181), pd_128));
        let s_65 = _mm256_add_epi32(t6a_n, t5a_n);
        let t6 = _mm256_srai_epi32::<8>(_mm256_add_epi32(_mm256_mullo_epi32(s_65, c_181), pd_128));


        let even = [
            clip(_mm256_add_epi32(dct4_0, t7)),
            clip(_mm256_add_epi32(dct4_1, t6)),
            clip(_mm256_add_epi32(dct4_2, t5)),
            clip(_mm256_add_epi32(dct4_3, t4)),
            clip(_mm256_sub_epi32(dct4_3, t4)),
            clip(_mm256_sub_epi32(dct4_2, t5)),
            clip(_mm256_sub_epi32(dct4_1, t6)),
            clip(_mm256_sub_epi32(dct4_0, t7)),
        ];




        let Ok(pair_1_15) = dct8_row_build_pair(_token, match col_xmm { [_, _explicit_element, ..] => _explicit_element }, match col_xmm { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
        let Ok(pair_9_7) = dct8_row_build_pair(_token, match col_xmm { [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match col_xmm { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
        let Ok(pair_5_11) = dct8_row_build_pair(_token, match col_xmm { [_, _, _, _, _, _explicit_element, ..] => _explicit_element }, match col_xmm { [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
        let Ok(pair_13_3) = dct8_row_build_pair(_token, match col_xmm { [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match col_xmm { [_, _, _, _explicit_element, ..] => _explicit_element });


        let o_t8a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_1_15, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 401, high: -4076 }); packed_vector }),
            pd_2048,
        ));

        let o_t15a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_1_15, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 4076, high: 401 }); packed_vector }),
            pd_2048,
        ));

        let o_t9a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_9_7, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3166, high: -2598 }); packed_vector }),
            pd_2048,
        ));

        let o_t14a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_9_7, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 2598, high: 3166 }); packed_vector }),
            pd_2048,
        ));

        let o_t10a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_5_11, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 1931, high: -3612 }); packed_vector }),
            pd_2048,
        ));

        let o_t13a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_5_11, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3612, high: 1931 }); packed_vector }),
            pd_2048,
        ));

        let o_t11a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_13_3, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3920, high: -1189 }); packed_vector }),
            pd_2048,
        ));

        let o_t12a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_13_3, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 1189, high: 3920 }); packed_vector }),
            pd_2048,
        ));


        let o_t8 = clip(_mm256_add_epi32(o_t8a, o_t9a));
        let mut o_t9 = clip(_mm256_sub_epi32(o_t8a, o_t9a));
        let mut o_t10 = clip(_mm256_sub_epi32(o_t11a, o_t10a));
        let o_t11 = clip(_mm256_add_epi32(o_t11a, o_t10a));
        let o_t12 = clip(_mm256_add_epi32(o_t12a, o_t13a));
        let mut o_t13 = clip(_mm256_sub_epi32(o_t12a, o_t13a));
        let mut o_t14 = clip(_mm256_sub_epi32(o_t15a, o_t14a));
        let o_t15 = clip(_mm256_add_epi32(o_t15a, o_t14a));


        let c_1567 = _mm256_set1_epi32(1567);
        let c_3784 = _mm256_set1_epi32(3784);


        let o_t9a_new = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_mullo_epi32(o_t14, c_1567),
                _mm256_mullo_epi32(o_t9, c_3784),
            ),
            pd_2048,
        ));

        let o_t14a_new = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_add_epi32(
                _mm256_mullo_epi32(o_t14, c_3784),
                _mm256_mullo_epi32(o_t9, c_1567),
            ),
            pd_2048,
        ));

        let o_t10a_new = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_setzero_si256(),
                _mm256_add_epi32(
                    _mm256_mullo_epi32(o_t13, c_3784),
                    _mm256_mullo_epi32(o_t10, c_1567),
                ),
            ),
            pd_2048,
        ));

        let o_t13a_new = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_mullo_epi32(o_t13, c_1567),
                _mm256_mullo_epi32(o_t10, c_3784),
            ),
            pd_2048,
        ));


        let o_t8a_f = clip(_mm256_add_epi32(o_t8, o_t11));
        o_t9 = clip(_mm256_add_epi32(o_t9a_new, o_t10a_new));
        o_t10 = clip(_mm256_sub_epi32(o_t9a_new, o_t10a_new));
        let o_t11a_f = clip(_mm256_sub_epi32(o_t8, o_t11));
        let o_t12a_f = clip(_mm256_sub_epi32(o_t15, o_t12));
        o_t13 = clip(_mm256_sub_epi32(o_t14a_new, o_t13a_new));
        o_t14 = clip(_mm256_add_epi32(o_t14a_new, o_t13a_new));
        let o_t15a_f = clip(_mm256_add_epi32(o_t15, o_t12));



        let d2 = _mm256_sub_epi32(o_t13, o_t10);
        let o_t10a_f =
            _mm256_srai_epi32::<8>(_mm256_add_epi32(_mm256_mullo_epi32(d2, c_181), pd_128));

        let s2 = _mm256_add_epi32(o_t13, o_t10);
        let o_t13a_f =
            _mm256_srai_epi32::<8>(_mm256_add_epi32(_mm256_mullo_epi32(s2, c_181), pd_128));

        let d3 = _mm256_sub_epi32(o_t12a_f, o_t11a_f);
        let o_t11_f =
            _mm256_srai_epi32::<8>(_mm256_add_epi32(_mm256_mullo_epi32(d3, c_181), pd_128));

        let s3 = _mm256_add_epi32(o_t12a_f, o_t11a_f);
        let o_t12_f =
            _mm256_srai_epi32::<8>(_mm256_add_epi32(_mm256_mullo_epi32(s3, c_181), pd_128));











        let odd = [
            o_t15a_f, o_t14, o_t13a_f, o_t12_f, o_t11_f, o_t10a_f, o_t9, o_t8a_f,
        ];

        let mut cols = [[_mm256_setzero_si256(); 8]; 2];
        let [first_half, second_half] = &mut cols;

        for ((left, right), (a, b)) in first_half.iter_mut()
            .zip(second_half.iter_mut().rev()).zip(even.into_iter().zip(odd))
        {
            *left = clip(_mm256_add_epi32(a, b));
            *right = clip(_mm256_sub_epi32(a, b));
        }



        let destination = output.as_chunks_mut::<8>().0;

        for (ordinal, columns) in (0u8..).zip(cols.iter()) {
            let Ok(strip) = console_core_number_conversion::index(ordinal);
            let Ok(rows) = transpose_8x8_i32(_token, *columns);
            let lanes = destination.iter_mut().skip(strip).step_by(2);

            for (lane, vector) in lanes.zip(rows) {
                storeu_256!(lane, vector);
            }
        }
    }

    Ok(out)
}















#[cfg(target_arch = "x86_64")]
#[arcane]
fn dct32_row_pass_i16_simd(_token: Desktop64, coeff_col_major: [i16; 1024]) -> Result<[i32; 1024], Never> {
    let mut out = [0i32; 1024];

    let coefficient_strips = coeff_col_major.as_chunks::<8>().0;
    let row_groups = out.as_chunks_mut::<256>().0;

    for (batch, output) in (0u8..).zip(row_groups) {
        let Ok(first) = console_core_number_conversion::index(batch);
        let input = coefficient_strips.iter().skip(first).step_by(4);
        let mut cx = [_mm_setzero_si128(); 32];
        let mut nz = _mm_setzero_si128();

        for (destination, lane) in cx.iter_mut().zip(input) {
            let vector = loadu_128!(lane);
            *destination = vector;
            nz = _mm_or_si128(nz, vector);
        }

         match ::core::primitive::bool::then_some(_mm_testz_si128(nz, nz) != 0, ()) { ::core::option::Option::Some(()) => {
            continue;
        }, ::core::option::Option::None => {} }

        let pd_2048 = _mm256_set1_epi32(2048);
        let pd_128 = _mm256_set1_epi32(128);
        let row_min_v = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN));
        let row_max_v = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX));
        let clip = |v: __m256i| _mm256_max_epi32(_mm256_min_epi32(v, row_max_v), row_min_v);
        let c_181 = _mm256_set1_epi32(181);


        let Ok(pair_04) = dct8_row_build_pair(_token, match cx { [_explicit_element, ..] => _explicit_element }, match cx { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
        let Ok(pair_13) = dct8_row_build_pair(_token, match cx { [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match cx { [.., _explicit_element, _, _, _, _, _, _, _] => _explicit_element });

        let dct4_t0 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_04, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
            pd_128,
        ));
        let dct4_t1 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_04, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
            pd_128,
        ));
        let dct4_t2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_13, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 1567, high: -3784 }); packed_vector }),
            pd_2048,
        ));
        let dct4_t3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_13, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3784, high: 1567 }); packed_vector }),
            pd_2048,
        ));

        let dct4_o0 = clip(_mm256_add_epi32(dct4_t0, dct4_t3));
        let dct4_o1 = clip(_mm256_add_epi32(dct4_t1, dct4_t2));
        let dct4_o2 = clip(_mm256_sub_epi32(dct4_t1, dct4_t2));
        let dct4_o3 = clip(_mm256_sub_epi32(dct4_t0, dct4_t3));


        let Ok(pair_8_17) = dct8_row_build_pair(_token, match cx { [_, _, _, _, _explicit_element, ..] => _explicit_element }, match cx { [.., _explicit_element, _, _, _] => _explicit_element });
        let Ok(pair_8_53) = dct8_row_build_pair(_token, match cx { [.., _explicit_element, _, _, _, _, _, _, _, _, _, _, _] => _explicit_element }, match cx { [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });

        let t4a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_8_17, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 799, high: -4017 }); packed_vector }),
            pd_2048,
        ));
        let t7a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_8_17, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 4017, high: 799 }); packed_vector }),
            pd_2048,
        ));
        let t5a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_8_53, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3406, high: -2276 }); packed_vector }),
            pd_2048,
        ));
        let t6a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_8_53, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 2276, high: 3406 }); packed_vector }),
            pd_2048,
        ));

        let t4 = clip(_mm256_add_epi32(t4a, t5a));
        let t5a_n = clip(_mm256_sub_epi32(t4a, t5a));
        let t7 = clip(_mm256_add_epi32(t7a, t6a));
        let t6a_n = clip(_mm256_sub_epi32(t7a, t6a));

        let d_56 = _mm256_sub_epi32(t6a_n, t5a_n);
        let t5 = _mm256_srai_epi32::<8>(_mm256_add_epi32(_mm256_mullo_epi32(d_56, c_181), pd_128));
        let s_56 = _mm256_add_epi32(t6a_n, t5a_n);
        let t6 = _mm256_srai_epi32::<8>(_mm256_add_epi32(_mm256_mullo_epi32(s_56, c_181), pd_128));

        let dct8_o0 = clip(_mm256_add_epi32(dct4_o0, t7));
        let dct8_o1 = clip(_mm256_add_epi32(dct4_o1, t6));
        let dct8_o2 = clip(_mm256_add_epi32(dct4_o2, t5));
        let dct8_o3 = clip(_mm256_add_epi32(dct4_o3, t4));
        let dct8_o4 = clip(_mm256_sub_epi32(dct4_o3, t4));
        let dct8_o5 = clip(_mm256_sub_epi32(dct4_o2, t5));
        let dct8_o6 = clip(_mm256_sub_epi32(dct4_o1, t6));
        let dct8_o7 = clip(_mm256_sub_epi32(dct4_o0, t7));


        let Ok(pair_16_1_15) = dct8_row_build_pair(_token, match cx { [_, _, _explicit_element, ..] => _explicit_element }, match cx { [.., _explicit_element, _] => _explicit_element });
        let Ok(pair_16_9_7) = dct8_row_build_pair(_token, match cx { [.., _explicit_element, _, _, _, _, _, _, _, _, _, _, _, _, _] => _explicit_element }, match cx { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
        let Ok(pair_16_5_11) = dct8_row_build_pair(_token, match cx { [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match cx { [.., _explicit_element, _, _, _, _, _, _, _, _, _] => _explicit_element });
        let Ok(pair_16_13_3) = dct8_row_build_pair(_token, match cx { [.., _explicit_element, _, _, _, _, _] => _explicit_element }, match cx { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element });

        let t8a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_16_1_15, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 401, high: -4076 }); packed_vector }),
            pd_2048,
        ));
        let t15a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_16_1_15, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 4076, high: 401 }); packed_vector }),
            pd_2048,
        ));

        let t9a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_16_9_7, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3166, high: -2598 }); packed_vector }),
            pd_2048,
        ));
        let t14a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_16_9_7, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 2598, high: 3166 }); packed_vector }),
            pd_2048,
        ));
        let t10a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_16_5_11, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 1931, high: -3612 }); packed_vector }),
            pd_2048,
        ));
        let t13a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_16_5_11, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3612, high: 1931 }); packed_vector }),
            pd_2048,
        ));
        let t11a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_16_13_3, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3920, high: -1189 }); packed_vector }),
            pd_2048,
        ));
        let t12a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_16_13_3, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 1189, high: 3920 }); packed_vector }),
            pd_2048,
        ));


        let t8 = clip(_mm256_add_epi32(t8a, t9a));
        let mut t9 = clip(_mm256_sub_epi32(t8a, t9a));
        let mut t10 = clip(_mm256_sub_epi32(t11a, t10a));
        let t11 = clip(_mm256_add_epi32(t11a, t10a));
        let t12 = clip(_mm256_add_epi32(t12a, t13a));
        let mut t13 = clip(_mm256_sub_epi32(t12a, t13a));
        let mut t14 = clip(_mm256_sub_epi32(t15a, t14a));
        let t15 = clip(_mm256_add_epi32(t15a, t14a));


        let c1567 = _mm256_set1_epi32(1567);
        let c3784 = _mm256_set1_epi32(3784);
        let t9a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_mullo_epi32(t14, c1567),
                _mm256_mullo_epi32(t9, c3784),
            ),
            pd_2048,
        ));
        let t14a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_add_epi32(
                _mm256_mullo_epi32(t14, c3784),
                _mm256_mullo_epi32(t9, c1567),
            ),
            pd_2048,
        ));
        let t10a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_setzero_si256(),
                _mm256_add_epi32(
                    _mm256_mullo_epi32(t13, c3784),
                    _mm256_mullo_epi32(t10, c1567),
                ),
            ),
            pd_2048,
        ));
        let t13a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_mullo_epi32(t13, c1567),
                _mm256_mullo_epi32(t10, c3784),
            ),
            pd_2048,
        ));


        let t8a_2 = clip(_mm256_add_epi32(t8, t11));
        t9 = clip(_mm256_add_epi32(t9a_2, t10a_2));
        t10 = clip(_mm256_sub_epi32(t9a_2, t10a_2));
        let t11a_2 = clip(_mm256_sub_epi32(t8, t11));
        let t12a_2 = clip(_mm256_sub_epi32(t15, t12));
        t13 = clip(_mm256_sub_epi32(t14a_2, t13a_2));
        t14 = clip(_mm256_add_epi32(t14a_2, t13a_2));
        let t15a_2 = clip(_mm256_add_epi32(t15, t12));


        let t10a_3 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(_mm256_sub_epi32(t13, t10), c_181),
            pd_128,
        ));
        let t13a_3 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(_mm256_add_epi32(t13, t10), c_181),
            pd_128,
        ));
        let t11_2 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(_mm256_sub_epi32(t12a_2, t11a_2), c_181),
            pd_128,
        ));
        let t12_2 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(_mm256_add_epi32(t12a_2, t11a_2), c_181),
            pd_128,
        ));


        let dct16_o = [
            clip(_mm256_add_epi32(dct8_o0, t15a_2)),
            clip(_mm256_add_epi32(dct8_o1, t14)),
            clip(_mm256_add_epi32(dct8_o2, t13a_3)),
            clip(_mm256_add_epi32(dct8_o3, t12_2)),
            clip(_mm256_add_epi32(dct8_o4, t11_2)),
            clip(_mm256_add_epi32(dct8_o5, t10a_3)),
            clip(_mm256_add_epi32(dct8_o6, t9)),
            clip(_mm256_add_epi32(dct8_o7, t8a_2)),
            clip(_mm256_sub_epi32(dct8_o7, t8a_2)),
            clip(_mm256_sub_epi32(dct8_o6, t9)),
            clip(_mm256_sub_epi32(dct8_o5, t10a_3)),
            clip(_mm256_sub_epi32(dct8_o4, t11_2)),
            clip(_mm256_sub_epi32(dct8_o3, t12_2)),
            clip(_mm256_sub_epi32(dct8_o2, t13a_3)),
            clip(_mm256_sub_epi32(dct8_o1, t14)),
            clip(_mm256_sub_epi32(dct8_o0, t15a_2)),
        ];


        let Ok(pair_32_1_31) = dct8_row_build_pair(_token, match cx { [_, _explicit_element, ..] => _explicit_element }, match cx { [.., _explicit_element] => _explicit_element });
        let Ok(pair_32_17_15) = dct8_row_build_pair(_token, match cx { [.., _explicit_element, _, _, _, _, _, _, _, _, _, _, _, _, _, _] => _explicit_element }, match cx { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
        let Ok(pair_32_9_23) = dct8_row_build_pair(_token, match cx { [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match cx { [.., _explicit_element, _, _, _, _, _, _, _, _] => _explicit_element });
        let Ok(pair_32_25_7) = dct8_row_build_pair(_token, match cx { [.., _explicit_element, _, _, _, _, _, _] => _explicit_element }, match cx { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
        let Ok(pair_32_5_27) = dct8_row_build_pair(_token, match cx { [_, _, _, _, _, _explicit_element, ..] => _explicit_element }, match cx { [.., _explicit_element, _, _, _, _] => _explicit_element });
        let Ok(pair_32_21_11) = dct8_row_build_pair(_token, match cx { [.., _explicit_element, _, _, _, _, _, _, _, _, _, _] => _explicit_element }, match cx { [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
        let Ok(pair_32_13_19) = dct8_row_build_pair(_token, match cx { [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match cx { [.., _explicit_element, _, _, _, _, _, _, _, _, _, _, _, _] => _explicit_element });
        let Ok(pair_32_29_3) = dct8_row_build_pair(_token, match cx { [.., _explicit_element, _, _] => _explicit_element }, match cx { [_, _, _, _explicit_element, ..] => _explicit_element });


        let t16a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_1_31, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 201, high: -4091 }); packed_vector }),
            pd_2048,
        ));
        let t31a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_1_31, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 4091, high: 201 }); packed_vector }),
            pd_2048,
        ));
        let t17a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_17_15, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3035, high: -2751 }); packed_vector }),
            pd_2048,
        ));
        let t30a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_17_15, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 2751, high: 3035 }); packed_vector }),
            pd_2048,
        ));
        let t18a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_9_23, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 1751, high: -3703 }); packed_vector }),
            pd_2048,
        ));
        let t29a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_9_23, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3703, high: 1751 }); packed_vector }),
            pd_2048,
        ));
        let t19a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_25_7, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3857, high: -1380 }); packed_vector }),
            pd_2048,
        ));
        let t28a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_25_7, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 1380, high: 3857 }); packed_vector }),
            pd_2048,
        ));
        let t20a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_5_27, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 995, high: -3973 }); packed_vector }),
            pd_2048,
        ));
        let t27a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_5_27, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3973, high: 995 }); packed_vector }),
            pd_2048,
        ));
        let t21a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_21_11, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3513, high: -2106 }); packed_vector }),
            pd_2048,
        ));
        let t26a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_21_11, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 2106, high: 3513 }); packed_vector }),
            pd_2048,
        ));

        let t22a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_13_19, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 2440, high: -3290 }); packed_vector }),
            pd_2048,
        ));
        let t25a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_13_19, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 3290, high: 2440 }); packed_vector }),
            pd_2048,
        ));
        let t23a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_29_3, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 4052, high: -601 }); packed_vector }),
            pd_2048,
        ));
        let t24a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_madd_epi16(pair_32_29_3, { let Ok(packed_vector) = dct8_row_coef_pack(_token, CoefficientPair { low: 601, high: 4052 }); packed_vector }),
            pd_2048,
        ));


        let mut t16 = clip(_mm256_add_epi32(t16a, t17a));
        let mut t17 = clip(_mm256_sub_epi32(t16a, t17a));
        let mut t18 = clip(_mm256_sub_epi32(t19a, t18a));
        let t19 = clip(_mm256_add_epi32(t19a, t18a));
        let t20 = clip(_mm256_add_epi32(t20a, t21a));
        let mut t21 = clip(_mm256_sub_epi32(t20a, t21a));
        let mut t22 = clip(_mm256_sub_epi32(t23a, t22a));
        let mut t23 = clip(_mm256_add_epi32(t23a, t22a));
        let mut t24 = clip(_mm256_add_epi32(t24a, t25a));
        let mut t25 = clip(_mm256_sub_epi32(t24a, t25a));
        let mut t26 = clip(_mm256_sub_epi32(t27a, t26a));
        let t27 = clip(_mm256_add_epi32(t27a, t26a));
        let t28 = clip(_mm256_add_epi32(t28a, t29a));
        let mut t29 = clip(_mm256_sub_epi32(t28a, t29a));
        let mut t30 = clip(_mm256_sub_epi32(t31a, t30a));
        let mut t31 = clip(_mm256_add_epi32(t31a, t30a));


        let c799 = _mm256_set1_epi32(799);
        let c4017 = _mm256_set1_epi32(4017);
        let c1703 = _mm256_set1_epi32(1703);
        let c1138 = _mm256_set1_epi32(1138);
        let pd_1024 = _mm256_set1_epi32(1024);
        let t17a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_mullo_epi32(t30, c799),
                _mm256_mullo_epi32(t17, c4017),
            ),
            pd_2048,
        ));
        let t30a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_add_epi32(
                _mm256_mullo_epi32(t30, c4017),
                _mm256_mullo_epi32(t17, c799),
            ),
            pd_2048,
        ));
        let t18a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_setzero_si256(),
                _mm256_add_epi32(
                    _mm256_mullo_epi32(t29, c4017),
                    _mm256_mullo_epi32(t18, c799),
                ),
            ),
            pd_2048,
        ));
        let t29a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_mullo_epi32(t29, c799),
                _mm256_mullo_epi32(t18, c4017),
            ),
            pd_2048,
        ));
        let t21a_2 = _mm256_srai_epi32::<11>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_mullo_epi32(t26, c1703),
                _mm256_mullo_epi32(t21, c1138),
            ),
            pd_1024,
        ));
        let t26a_2 = _mm256_srai_epi32::<11>(_mm256_add_epi32(
            _mm256_add_epi32(
                _mm256_mullo_epi32(t26, c1138),
                _mm256_mullo_epi32(t21, c1703),
            ),
            pd_1024,
        ));
        let t22a_2 = _mm256_srai_epi32::<11>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_setzero_si256(),
                _mm256_add_epi32(
                    _mm256_mullo_epi32(t25, c1138),
                    _mm256_mullo_epi32(t22, c1703),
                ),
            ),
            pd_1024,
        ));
        let t25a_2 = _mm256_srai_epi32::<11>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_mullo_epi32(t25, c1703),
                _mm256_mullo_epi32(t22, c1138),
            ),
            pd_1024,
        ));


        let t16a_2 = clip(_mm256_add_epi32(t16, t19));
        t17 = clip(_mm256_add_epi32(t17a_2, t18a_2));
        t18 = clip(_mm256_sub_epi32(t17a_2, t18a_2));
        let t19a_2 = clip(_mm256_sub_epi32(t16, t19));
        let t20a_2 = clip(_mm256_sub_epi32(t23, t20));
        t21 = clip(_mm256_sub_epi32(t22a_2, t21a_2));
        t22 = clip(_mm256_add_epi32(t22a_2, t21a_2));
        let t23a_2 = clip(_mm256_add_epi32(t23, t20));
        let t24a_2 = clip(_mm256_add_epi32(t24, t27));
        t25 = clip(_mm256_add_epi32(t25a_2, t26a_2));
        t26 = clip(_mm256_sub_epi32(t25a_2, t26a_2));
        let t27a_2 = clip(_mm256_sub_epi32(t24, t27));
        let t28a_2 = clip(_mm256_sub_epi32(t31, t28));
        t29 = clip(_mm256_sub_epi32(t30a_2, t29a_2));
        t30 = clip(_mm256_add_epi32(t30a_2, t29a_2));
        let t31a_2 = clip(_mm256_add_epi32(t31, t28));


        let t18a_3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_mullo_epi32(t29, c1567),
                _mm256_mullo_epi32(t18, c3784),
            ),
            pd_2048,
        ));
        let t29a_3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_add_epi32(
                _mm256_mullo_epi32(t29, c3784),
                _mm256_mullo_epi32(t18, c1567),
            ),
            pd_2048,
        ));
        let t19_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_mullo_epi32(t28a_2, c1567),
                _mm256_mullo_epi32(t19a_2, c3784),
            ),
            pd_2048,
        ));
        let t28_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_add_epi32(
                _mm256_mullo_epi32(t28a_2, c3784),
                _mm256_mullo_epi32(t19a_2, c1567),
            ),
            pd_2048,
        ));
        let t20_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_setzero_si256(),
                _mm256_add_epi32(
                    _mm256_mullo_epi32(t27a_2, c3784),
                    _mm256_mullo_epi32(t20a_2, c1567),
                ),
            ),
            pd_2048,
        ));
        let t27_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_mullo_epi32(t27a_2, c1567),
                _mm256_mullo_epi32(t20a_2, c3784),
            ),
            pd_2048,
        ));
        let t21a_3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_setzero_si256(),
                _mm256_add_epi32(
                    _mm256_mullo_epi32(t26, c3784),
                    _mm256_mullo_epi32(t21, c1567),
                ),
            ),
            pd_2048,
        ));
        let t26a_3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(
                _mm256_mullo_epi32(t26, c1567),
                _mm256_mullo_epi32(t21, c3784),
            ),
            pd_2048,
        ));


        t16 = clip(_mm256_add_epi32(t16a_2, t23a_2));
        let t17a_3 = clip(_mm256_add_epi32(t17, t22));
        t18 = clip(_mm256_add_epi32(t18a_3, t21a_3));
        let t19a_3 = clip(_mm256_add_epi32(t19_2, t20_2));
        let t20a_3 = clip(_mm256_sub_epi32(t19_2, t20_2));
        t21 = clip(_mm256_sub_epi32(t18a_3, t21a_3));
        let t22a_3 = clip(_mm256_sub_epi32(t17, t22));
        t23 = clip(_mm256_sub_epi32(t16a_2, t23a_2));
        t24 = clip(_mm256_sub_epi32(t31a_2, t24a_2));
        let t25a_3 = clip(_mm256_sub_epi32(t30, t25));
        t26 = clip(_mm256_sub_epi32(t29a_3, t26a_3));
        let t27a_3 = clip(_mm256_sub_epi32(t28_2, t27_2));
        let t28a_3 = clip(_mm256_add_epi32(t28_2, t27_2));
        t29 = clip(_mm256_add_epi32(t29a_3, t26a_3));
        let t30a_3 = clip(_mm256_add_epi32(t30, t25));
        t31 = clip(_mm256_add_epi32(t31a_2, t24a_2));


        let t20_f = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(_mm256_sub_epi32(t27a_3, t20a_3), c_181),
            pd_128,
        ));
        let t27_f = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(_mm256_add_epi32(t27a_3, t20a_3), c_181),
            pd_128,
        ));
        let t21a_f = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(_mm256_sub_epi32(t26, t21), c_181),
            pd_128,
        ));
        let t26a_f = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(_mm256_add_epi32(t26, t21), c_181),
            pd_128,
        ));
        let t22_f = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(_mm256_sub_epi32(t25a_3, t22a_3), c_181),
            pd_128,
        ));
        let t25_f = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(_mm256_add_epi32(t25a_3, t22a_3), c_181),
            pd_128,
        ));
        let t23a_f = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(_mm256_sub_epi32(t24, t23), c_181),
            pd_128,
        ));
        let t24a_f = _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(_mm256_add_epi32(t24, t23), c_181),
            pd_128,
        ));


        let mut cols = [_mm256_setzero_si256(); 32];
        *{ let [_explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_explicit_element, ..] => _explicit_element }, t31));
        *{ let [_, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_, _explicit_element, ..] => _explicit_element }, t30a_3));
        *{ let [_, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_, _, _explicit_element, ..] => _explicit_element }, t29));
        *{ let [_, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_, _, _, _explicit_element, ..] => _explicit_element }, t28a_3));
        *{ let [_, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_, _, _, _, _explicit_element, ..] => _explicit_element }, t27_f));
        *{ let [_, _, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_, _, _, _, _, _explicit_element, ..] => _explicit_element }, t26a_f));
        *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t25_f));
        *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t24a_f));
        *{ let [_, _, _, _, _, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t23a_f));
        *{ let [_, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t22_f));
        *{ let [_, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t21a_f));
        *{ let [_, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t20_f));
        *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t19a_3));
        *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t18));
        *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t17a_3));
        *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_add_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t16));
        *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t16));
        *{ let [.., _explicit_part, _, _, _, _, _, _, _, _, _, _, _, _, _, _] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t17a_3));
        *{ let [.., _explicit_part, _, _, _, _, _, _, _, _, _, _, _, _, _] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t18));
        *{ let [.., _explicit_part, _, _, _, _, _, _, _, _, _, _, _, _] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t19a_3));
        *{ let [.., _explicit_part, _, _, _, _, _, _, _, _, _, _, _] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t20_f));
        *{ let [.., _explicit_part, _, _, _, _, _, _, _, _, _, _] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t21a_f));
        *{ let [.., _explicit_part, _, _, _, _, _, _, _, _, _] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t22_f));
        *{ let [.., _explicit_part, _, _, _, _, _, _, _, _] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t23a_f));
        *{ let [.., _explicit_part, _, _, _, _, _, _, _] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t24a_f));
        *{ let [.., _explicit_part, _, _, _, _, _, _] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, t25_f));
        *{ let [.., _explicit_part, _, _, _, _, _] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_, _, _, _, _, _explicit_element, ..] => _explicit_element }, t26a_f));
        *{ let [.., _explicit_part, _, _, _, _] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_, _, _, _, _explicit_element, ..] => _explicit_element }, t27_f));
        *{ let [.., _explicit_part, _, _, _] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_, _, _, _explicit_element, ..] => _explicit_element }, t28a_3));
        *{ let [.., _explicit_part, _, _] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_, _, _explicit_element, ..] => _explicit_element }, t29));
        *{ let [.., _explicit_part, _] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_, _explicit_element, ..] => _explicit_element }, t30a_3));
        *{ let [.., _explicit_part] = &mut cols; _explicit_part } = clip(_mm256_sub_epi32(match dct16_o { [_explicit_element, ..] => _explicit_element }, t31));


        let destination = output.as_chunks_mut::<8>().0;

        for (ordinal, columns) in (0u8..).zip(cols.as_chunks::<8>().0) {
            let Ok(strip) = console_core_number_conversion::index(ordinal);
            let Ok(rows) = transpose_8x8_i32(_token, *columns);
            let lanes = destination.iter_mut().skip(strip).step_by(4);

            for (lane, vector) in lanes.zip(rows) {
                storeu_256!(lane, vector);
            }
        }
    }

    Ok(out)
}




#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_32x16_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 32, height: 16 };
    let stride = dst_stride;
    let (pixels, active) = rectangular_8bpc_views(dst, coeff, stride, extent)?;
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    let pass = RowPass {
        coefficient_height: 16,
        scale: RectangularScale::Apply,
        rounding: 1,
        shift: 1,
        row_bounds: bounds.rows,
        column_bounds: bounds.columns,
    };
    let mut scratch = [0i32; 512];
    row_dct32_8bpc_block(token, active, extent.height, &mut scratch, pass)?;
    let residuals = scratch.as_mut_slice();
    let Ok(avx512) = crate::src::cpu::summon_avx512();

    match avx512 {
        Some(wide) => {
            dct16_cols_avx512(wide, residuals, extent, bounds.columns)?;
            add_to_dst_8bpc_avx512(wide, pixels, stride, residuals, extent)?;
        }
        None => {
            let clipping = VectorBounds {
                lower: _mm256_set1_epi32(bounds.columns.lower),
                upper: _mm256_set1_epi32(bounds.columns.upper),
            };
            apply_transform_columns_avx2(token, residuals, extent, clipping, |column_token, values, limits| {
                dct16_1d_cols8(column_token, values, limits)
            })?;
            add_to_dst_8bpc_avx2(token, pixels, stride, residuals, extent, bitdepth_max)?;
        }
    }

    active.fill(0);
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_16x32_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 16, height: 32 };
    let stride = dst_stride;
    let (pixels, active) = rectangular_8bpc_views(dst, coeff, stride, extent)?;
    let mut rows = rectangular_8bpc_rows::<16, 32>(active, identity16_1d, RectangularScale::Apply, 1)?;
    let residuals = rows.as_flattened_mut();
    let Ok(avx512) = crate::src::cpu::summon_avx512();

    match avx512 {
        Some(wide) => identity_shift_cols_avx512::<2>(wide, residuals, extent)?,
        None => identity_shift_columns_8bpc::<2>(token, residuals)?,
    }

    add_to_dst_8bpc_avx2(token, pixels, stride, residuals, extent, bitdepth_max)?;
    active.fill(0);
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_32x16_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 32, height: 16 };
    let stride = dst_stride;
    let (pixels, active) = rectangular_8bpc_views(dst, coeff, stride, extent)?;
    let mut rows = rectangular_8bpc_rows::<32, 16>(active, identity32_1d, RectangularScale::Apply, 1)?;
    let residuals = rows.as_flattened_mut();
    let Ok(avx512) = crate::src::cpu::summon_avx512();

    match avx512 {
        Some(wide) => {
            identity16_cols_avx512(wide, residuals, extent)?;
            add_to_dst_8bpc_avx512(wide, pixels, stride, residuals, extent)?;
        }
        None => {
            identity16_columns_8bpc(token, residuals)?;
            add_rectangular_pixels_8bpc(pixels, stride, residuals, extent, bitdepth_max)?;
        }
    }

    active.fill(0);
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_32x64_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 32, height: 64 };
    let stride = dst_stride;
    let (pixels, active) = rectangular_8bpc_views(dst, coeff, stride, extent)?;
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    let pass = RowPass {
        coefficient_height: 32,
        scale: RectangularScale::Apply,
        rounding: 1,
        shift: 1,
        row_bounds: bounds.rows,
        column_bounds: bounds.columns,
    };
    let mut scratch = [0i32; 2048];

    for first_row in [0u32, 8, 16, 24] {
        simd_row_dct32_8bpc_8rows(token, active, first_row, &mut scratch, pass)?;
    }

    let residuals = scratch.as_mut_slice();
    transform_scalar_columns(residuals, STRIDE_32, dct64_1d, bounds.columns)?;
    let Ok(avx512) = crate::src::cpu::summon_avx512();

    match avx512 {
        Some(wide) => add_to_dst_8bpc_avx512(wide, pixels, stride, residuals, extent)?,
        None => add_to_dst_8bpc_avx2(token, pixels, stride, residuals, extent, bitdepth_max)?,
    }

    active.fill(0);
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_64x32_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 64, height: 32 };
    let stride = dst_stride;
    let (pixels, active) = rectangular_8bpc_views(dst, coeff, stride, extent)?;
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    let input = active.as_chunks::<32>().0.iter().map(|column| column.as_slice());
    let pass = RectangularRows {
        transform: dct64_1d,
        scale: RectangularScale::Apply,
        shift: 1,
    };
    let mut rows = transform_active_rows::<_, 64, 32>(input, extent.height, pass, bounds)?;
    let residuals = rows.as_flattened_mut();
    let Ok(avx512) = crate::src::cpu::summon_avx512();

    match avx512 {
        Some(wide) => {
            dct32_cols_avx512(wide, residuals, extent, bounds.columns)?;
            add_to_dst_8bpc_avx512(wide, pixels, stride, residuals, extent)?;
        }
        None => {
            let clipping = VectorBounds {
                lower: _mm256_set1_epi32(bounds.columns.lower),
                upper: _mm256_set1_epi32(bounds.columns.upper),
            };
            apply_transform_columns_avx2(token, residuals, extent, clipping, |column_token, values, limits| {
                dct32_1d_cols8_i16(column_token, values, limits)
            })?;
            add_to_dst_8bpc_avx2(token, pixels, stride, residuals, extent, bitdepth_max)?;
        }
    }

    active.fill(0);
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_4x16_8bpc_avx2_inner(
    _token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 4, height: 16 };
    let stride = dst_stride;
    let (pixels, active) = rectangular_8bpc_views(dst, coeff, stride, extent)?;
    let mut rows = rectangular_8bpc_rows::<4, 16>(active, dct4_1d, RectangularScale::Skip, 1)?;
    let residuals = rows.as_flattened_mut();
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    transform_scalar_columns(residuals, STRIDE_4, dct16_1d, bounds.columns)?;
    add_rectangular_pixels_8bpc(pixels, stride, residuals, extent, bitdepth_max)?;
    active.fill(0);
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_16x4_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 16, height: 4 };
    let stride = dst_stride;
    let (pixels, active) = rectangular_8bpc_views(dst, coeff, stride, extent)?;
    let mut rows = rectangular_8bpc_rows::<16, 4>(active, dct16_1d, RectangularScale::Skip, 1)?;
    let residuals = rows.as_flattened_mut();
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    let Ok(avx512) = crate::src::cpu::summon_avx512();

    match avx512 {
        Some(wide) => dct4_cols_avx512(wide, residuals, extent, bounds.columns)?,
        None => {
            let lanes = VectorBounds {
                lower: _mm256_set1_epi32(bounds.columns.lower),
                upper: _mm256_set1_epi32(bounds.columns.upper),
            };
            apply_transform_columns_avx2(token, residuals, extent, lanes, |column_token, values, column_bounds| {
                dct4_1d_cols8(column_token, values, column_bounds)
            })?;
        }
    }

    add_to_dst_8bpc_avx2(token, pixels, stride, residuals, extent, bitdepth_max)?;
    active.fill(0);
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_8x32_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 8, height: 32 };
    let stride = dst_stride;
    let (pixels, active) = rectangular_8bpc_views(dst, coeff, stride, extent)?;
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    let pass = RowPass {
        coefficient_height: 32,
        scale: RectangularScale::Skip,
        rounding: 2,
        shift: 2,
        row_bounds: bounds.rows,
        column_bounds: bounds.columns,
    };
    let mut residuals = [0i32; 8 * 32];

    for first_row in [0u32, 8, 16, 24] {
        simd_row_dct8_8bpc_8rows(token, active, first_row, &mut residuals, pass)?;
    }

    let lanes = VectorBounds {
        lower: _mm256_set1_epi32(bounds.columns.lower),
        upper: _mm256_set1_epi32(bounds.columns.upper),
    };
    apply_transform_columns_avx2(token, &mut residuals, extent, lanes, |column_token, values, column_bounds| {
        dct32_1d_cols8_i16(column_token, values, column_bounds)
    })?;
    add_rectangular_pixels_8bpc(pixels, stride, &residuals, extent, bitdepth_max)?;
    active.fill(0);
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_32x8_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 32, height: 8 };
    let stride = dst_stride;
    let (pixels, active) = rectangular_8bpc_views(dst, coeff, stride, extent)?;
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    let pass = RowPass {
        coefficient_height: 8,
        scale: RectangularScale::Skip,
        rounding: 2,
        shift: 2,
        row_bounds: bounds.rows,
        column_bounds: bounds.columns,
    };
    let mut scratch = [0i32; 256];
    simd_row_dct32_8bpc_8rows(token, active, 0, &mut scratch, pass)?;
    let residuals = scratch.as_mut_slice();
    let Ok(avx512) = crate::src::cpu::summon_avx512();

    match avx512 {
        Some(wide) => {
            dct8_cols_avx512(wide, residuals, extent, bounds.columns)?;
            add_to_dst_8bpc_avx512(wide, pixels, stride, residuals, extent)?;
        }
        None => {
            let clipping = VectorBounds {
                lower: _mm256_set1_epi32(bounds.columns.lower),
                upper: _mm256_set1_epi32(bounds.columns.upper),
            };
            apply_transform_columns_avx2(token, residuals, extent, clipping, |column_token, values, limits| {
                dct8_1d_cols8(column_token, values, limits)
            })?;
            add_to_dst_8bpc_avx2(token, pixels, stride, residuals, extent, bitdepth_max)?;
        }
    }

    active.fill(0);
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_8x32_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 8, height: 32 };
    let stride = dst_stride;
    let (pixels, active) = rectangular_8bpc_views(dst, coeff, stride, extent)?;
    let mut rows = rectangular_8bpc_rows::<8, 32>(active, identity8_1d, RectangularScale::Skip, 2)?;
    let residuals = rows.as_flattened_mut();
    identity_shift_columns_8bpc::<2>(token, residuals)?;
    add_rectangular_pixels_8bpc(pixels, stride, residuals, extent, bitdepth_max)?;
    active.fill(0);
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_identity_identity_32x8_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 32, height: 8 };
    let stride = dst_stride;
    let (pixels, active) = rectangular_8bpc_views(dst, coeff, stride, extent)?;
    let mut rows = rectangular_8bpc_rows::<32, 8>(active, identity32_1d, RectangularScale::Skip, 2)?;
    let residuals = rows.as_flattened_mut();
    let Ok(avx512) = crate::src::cpu::summon_avx512();

    match avx512 {
        Some(wide) => {
            identity_shift_cols_avx512::<1>(wide, residuals, extent)?;
            add_to_dst_8bpc_avx512(wide, pixels, stride, residuals, extent)?;
        }
        None => {
            identity_shift_columns_8bpc::<1>(token, residuals)?;
            add_rectangular_pixels_8bpc(pixels, stride, residuals, extent, bitdepth_max)?;
        }
    }

    active.fill(0);
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_16x64_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 16, height: 64 };
    let stride = dst_stride;
    let (pixels, active) = rectangular_8bpc_views(dst, coeff, stride, extent)?;
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    let pass = RowPass {
        coefficient_height: 32,
        scale: RectangularScale::Skip,
        rounding: 2,
        shift: 2,
        row_bounds: bounds.rows,
        column_bounds: bounds.columns,
    };
    let mut scratch = [0i32; 1024];

    for first_row in [0u32, 8, 16, 24] {
        simd_row_dct16_8bpc_8rows(token, active, first_row, &mut scratch, pass)?;
    }

    let residuals = scratch.as_mut_slice();
    transform_scalar_columns(residuals, STRIDE_16, dct64_1d, bounds.columns)?;
    add_to_dst_8bpc_avx2(token, pixels, stride, residuals, extent, bitdepth_max)?;
    active.fill(0);
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[arcane]
fn inv_txfm_add_dct_dct_64x16_8bpc_avx2_inner(
    token: Desktop64,
    dst: &mut [u8],
    dst_stride: u64,
    coeff: &mut [i16],
    bitdepth_max: i32,
) -> Rav1dResult {
    let extent = console_core_geometry::Size { width: 64, height: 16 };
    let stride = dst_stride;
    let (pixels, active) = rectangular_8bpc_views(dst, coeff, stride, extent)?;
    let Ok(bounds) = TransformAxisBounds::narrow(255);
    let input = active.as_chunks::<16>().0.iter().map(|column| column.as_slice());
    let pass = RectangularRows {
        transform: dct64_1d,
        scale: RectangularScale::Skip,
        shift: 2,
    };
    let mut rows = transform_active_rows::<_, 64, 16>(input, extent.height, pass, bounds)?;
    let residuals = rows.as_flattened_mut();
    let Ok(avx512) = crate::src::cpu::summon_avx512();

    match avx512 {
        Some(wide) => {
            dct16_cols_avx512(wide, residuals, extent, bounds.columns)?;
            add_to_dst_8bpc_avx512(wide, pixels, stride, residuals, extent)?;
        }
        None => {
            let clipping = VectorBounds {
                lower: _mm256_set1_epi32(bounds.columns.lower),
                upper: _mm256_set1_epi32(bounds.columns.upper),
            };
            apply_transform_columns_avx2(token, residuals, extent, clipping, |column_token, values, limits| {
                dct16_1d_cols8(column_token, values, limits)
            })?;
            add_to_dst_8bpc_avx2(token, pixels, stride, residuals, extent, bitdepth_max)?;
        }
    }

    active.fill(0);
    Ok(())
}
