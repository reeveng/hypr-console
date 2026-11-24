#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::*;

    #[test]
    fn avx2_high_bit_depth_residuals_match_scalar_on_padded_rows() -> Rav1dResult {
        let Ok(_lock) = crate::src::safe_simd::token_test_lock();
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let token = match avx2 {
            Some(token) => token,
            None => return Ok(()),
        };

        check_avx2_high_bit_depth_residuals(token)
    }

    #[arcane]
    fn check_avx2_high_bit_depth_residuals(token: Desktop64) -> Rav1dResult {
        let size = console_core_geometry::Size { width: 16, height: 3 };
        let residuals: [i32; 48] = core::array::from_fn(|position| {
            let Ok(coefficient) = console_core_number_conversion::wrapped::<_, i32>(position);
            coefficient.wrapping_mul(17_089).wrapping_rem(131_071).wrapping_sub(65_535)
        });
        let (input_rows, _) = residuals.as_chunks::<16>();

        for bitdepth_max in [1023, 4095] {
            let mut expected = [400u16; 60];
            let mut actual = expected;

            for (row, input) in expected.chunks_exact_mut(20).zip(input_rows) {
                let active = row.first_chunk_mut::<16>().ok_or(Rav1dError::EINVAL)?;

                for (pixel, residual) in active.iter_mut().zip(input) {
                    let delta = residual.wrapping_add(8).wrapping_shr(4);
                    let value = i32::from(*pixel).wrapping_add(delta).clamp(0, bitdepth_max);
                    let Ok(clipped) = console_core_number_conversion::wrapped::<_, u16>(value);
                    *pixel = clipped;
                }
            }

            add_to_dst_16bpc_avx2(token, &mut actual, 20, &residuals, size, bitdepth_max)?;
            assert_eq!(actual, expected);
        }

        Ok(())
    }

    #[test]
    fn avx512_residual_output_rejects_truncated_planes_before_writing() -> Rav1dResult {
        let Ok(_lock) = crate::src::safe_simd::token_test_lock();
        let Ok(wide) = crate::src::cpu::summon_avx512();
        let token = match wide {
            Some(token) => token,
            None => return Ok(()),
        };
        let size = console_core_geometry::Size { width: 16, height: 2 };
        let mut short_pixels = [128u8; 35];
        let residuals = [32i32; 32];
        let result = add_to_dst_8bpc_avx512(token, &mut short_pixels, 20, &residuals, size);
        assert_eq!(result, Err(Rav1dError::EINVAL));
        assert_eq!(short_pixels, [128; 35]);

        let mut pixels = [128u8; 36];
        let short_residuals = [32i32; 31];
        let result_2 = add_to_dst_8bpc_avx512(token, &mut pixels, 20, &short_residuals, size);
        assert_eq!(result_2, Err(Rav1dError::EINVAL));
        assert_eq!(pixels, [128; 36]);
        add_to_dst_8bpc_avx512(token, &mut pixels, 20, &residuals, size)?;
        let (left_row, rest) = pixels.split_first_chunk::<16>().ok_or(Rav1dError::EINVAL)?;
        let (padding, right_row) = rest.split_first_chunk::<4>().ok_or(Rav1dError::EINVAL)?;
        assert_eq!(*left_row, [130; 16]);
        assert_eq!(*padding, [128; 4]);
        assert_eq!(right_row, [130; 16]);

        let high_size = console_core_geometry::Size { width: 8, height: 2 };
        let mut high_pixels = [512u16; 19];
        let high_residuals = [32i32; 16];
        let result_3 = add_to_dst_16bpc_avx512(token, &mut high_pixels, 12, &high_residuals, high_size, 1023);
        assert_eq!(result_3, Err(Rav1dError::EINVAL));
        assert_eq!(high_pixels, [512; 19]);

        Ok(())
    }

    #[test]
  fn rectangular_transform_rejects_short_views_before_writing() {
    let Ok(avx2) = crate::src::cpu::summon_avx2();

    let token = match avx2 {
      Some(matched_token) => matched_token,
      ::core::option::Option::None => return,
    };

    let mut short_destination = [128; 63];
        let mut coefficients = [16; 64];
        assert_eq!(
            inv_txfm_add_dct_dct_4x16_8bpc_avx2_inner(
                token, &mut short_destination, 4, &mut coefficients, 255,
            ),
            Err(Rav1dError::EINVAL),
        );
        assert_eq!(short_destination, [128; 63]);
        assert_eq!(coefficients, [16; 64]);

        let mut destination = [128; 64];
        let mut short_coefficients = [16; 63];
        assert_eq!(
            inv_txfm_add_dct_dct_4x16_8bpc_avx2_inner(
                token, &mut destination, 4, &mut short_coefficients, 255,
            ),
            Err(Rav1dError::EINVAL),
        );
        assert_eq!(destination, [128; 64]);
        assert_eq!(short_coefficients, [16; 63]);
    }

    #[test]
    fn test_wht4_basic() -> Rav1dResult {
        let _lock = archmage::testing::lock_token_testing();
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let token = match avx2 {
            Some(token) => token,
            None => return Ok(()),
        };
        let mut coeff = [0i16; 16];
        let [dc, ..] = &mut coeff;
        *dc = 64;
        let mut dst = [128u8; 16];

        inv_txfm_add_wht_wht_4x4_8bpc_avx2_inner(token, &mut dst, 4, &mut coeff, 255)?;
        assert!(dst.iter().all(|&pixel| pixel >= 128));
        assert!(coeff.iter().all(|&coefficient| coefficient == 0));
        Ok(())
    }

    #[test]
    fn test_wht4_token_permutations() {
        use archmage::testing::{CompileTimePolicy, for_each_token_permutation};

        let Ok(_lock) = crate::src::safe_simd::token_test_lock();

        let reference = {
            let Ok(avx2) = crate::src::cpu::summon_avx2();
            let token = match avx2 { Some (token) => token, None => {
                eprintln!("Skipping: AVX2 not available");
                return;
            } };
            let mut coeff = [0i16; 16];
            *{ let [_explicit_part, ..] = &mut coeff; _explicit_part } = 64;
            let mut dst = [128u8; 16];
            assert_eq!(inv_txfm_add_wht_wht_4x4_8bpc_avx2_inner(token, &mut dst, 4, &mut coeff, 255), Ok(()));
            dst
        };

        let report = for_each_token_permutation(CompileTimePolicy::WarnStderr, |perm| {
            let Ok(avx2) = crate::src::cpu::summon_avx2();

            match avx2 { Some(token) => {
                let mut coeff = [0i16; 16];
                *{ let [_explicit_part, ..] = &mut coeff; _explicit_part } = 64;
                let mut dst = [128u8; 16];
                assert_eq!(inv_txfm_add_wht_wht_4x4_8bpc_avx2_inner(token, &mut dst, 4, &mut coeff, 255), Ok(()));
                assert_eq!(dst, reference, "WHT output mismatch at: {perm}");
                assert!(
                    coeff.iter().all(|&c| c == 0),
                    "coeffs not zeroed at: {perm}"
                );
            }, None => {} }

        });
        eprintln!("WHT permutations: {}", report.permutations_run);
        assert!(report.permutations_run >= 1);
    }














    #[arcane]
    fn itx_mul2x_pack_probe(
        token: Desktop64,
        a: [i16; 16],
        b: [i16; 16],
        coefficients: [i16; 2],
    ) -> Result<[i32; 16], Never> {
        let va = loadu_256!(&a, [i16; 16]);
        let vb = loadu_256!(&b, [i16; 16]);
        let lo = _mm256_unpacklo_epi16(va, vb);
        let hi = _mm256_unpackhi_epi16(va, vb);

        let Ok(r_lo) = multiply_coefficient_pairs(token, lo, coefficients);
        let Ok(r_hi) = multiply_coefficient_pairs(token, hi, coefficients);
        let mut out_lo = [0i32; 8];
        let mut out_hi = [0i32; 8];
        storeu_256!(&mut out_lo, [i32; 8], r_lo);
        storeu_256!(&mut out_hi, [i32; 8], r_hi);

        let [a0, a1, a2, a3, a4, a5, a6, a7] = out_lo;
        let [b0, b1, b2, b3, b4, b5, b6, b7] = out_hi;
        Ok([a0, a1, a2, a3, b0, b1, b2, b3, a4, a5, a6, a7, b4, b5, b6, b7])
    }

    fn next_test_state(state: &mut u64) -> Result<u64, Never> {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        Ok(*state)
    }

    #[test]
    fn test_itx_mul2x_pack_matches_scalar() -> Rav1dResult {
        use console_core_number_conversion::{index, wrapped};

        let _lock = archmage::testing::lock_token_testing();
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let token = match avx2 {
            Some(token) => token,
            None => return Ok(()),
        };
        let coefficient_pairs = [
            [799, 4017], [3406, 2276], [1567, 3784], [2896, 2896],
            [-2896, 2896], [401, 4076], [3166, 2598], [1931, 3612],
            [3920, 1189], [-3784, 1567], [i16::MIN, 1], [1, i16::MAX],
            [i16::MAX, i16::MIN],
        ];
        let mut state = 0xdead_beef_cafe_babe;

        for trial in 0..64_u32 {
            let mut a = [0i16; 16];
            let mut b = [0i16; 16];

            for (left, right) in a.iter_mut().zip(&mut b) {
                let Ok(state_a) = next_test_state(&mut state);
                let Ok(state_b) = next_test_state(&mut state);
                let Ok(sample_a) = wrapped(state_a.wrapping_shr(32));
                let Ok(sample_b) = wrapped(state_b.wrapping_shr(32));
                *left = sample_a;
                *right = sample_b;
            }

            match trial.wrapping_rem(8) {
                0 => {
                    let Ok(position_a) = index(trial.wrapping_rem(16));
                    let Ok(position_b) = index(trial.wrapping_add(1).wrapping_rem(16));
                    let left = a.get_mut(position_a).ok_or(Rav1dError::EINVAL)?;
                    let right = b.get_mut(position_b).ok_or(Rav1dError::EINVAL)?;
                    *left = i16::MAX;
                    *right = i16::MIN;
                }
                _remainder => {}
            }

            for coefficients @ [c1, c2] in coefficient_pairs {
                let Ok(actual) = itx_mul2x_pack_probe(token, a, b, coefficients);

                for ((left, right), packed) in a.into_iter().zip(b).zip(actual) {
                    let product_a = i32::from(left).wrapping_mul(i32::from(c1));
                    let product_b = i32::from(right).wrapping_mul(i32::from(c2));
                    let expected = product_a
                        .wrapping_add(product_b)
                        .wrapping_add(2048)
                        .wrapping_shr(12);
                    assert_eq!(packed, expected, "trial={trial} a={left} b={right} c1={c1} c2={c2}");
                }
            }
        }

        Ok(())
    }

    #[arcane]
    fn transpose_8x8_probe(token: Desktop64, input_col_major: [i32; 64]) -> Result<[i32; 64], Never> {
        let mut cols = [_mm256_setzero_si256(); 8];
        let (input_cols, _) = input_col_major.as_chunks::<8>();

        for (col, input) in cols.iter_mut().zip(input_cols) {
            *col = loadu_256!(input, [i32; 8]);
        }

        let Ok(rows) = transpose_8x8_i32(token, cols);
        let mut output = [0i32; 64];
        let (output_rows, _) = output.as_chunks_mut::<8>();

        for (row, vector) in output_rows.iter_mut().zip(rows) {
            storeu_256!(row, [i32; 8], vector);
        }

        Ok(output)
    }

    #[test]
    fn test_transpose_8x8_i32_roundtrip() -> Rav1dResult {
        use console_core_number_conversion::{index, wrapped};

        let _lock = archmage::testing::lock_token_testing();
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let token = match avx2 {
            Some(token) => token,
            None => return Ok(()),
        };
        let ordered = core::array::from_fn(|position| {
            let Ok(offset) = wrapped::<_, i32>(position);
            let x = offset.wrapping_div(8);
            let y = offset.wrapping_rem(8);
            x.wrapping_shl(24) | y.wrapping_shl(8) | 0x55
        });
        let mut state = 0xface_feed_dead_beef;
        let mut sampled = [0; 64];

        for value in &mut sampled {
            let Ok(random) = next_test_state(&mut state);
            let Ok(sample) = wrapped(random);
            *value = sample;
        }

        for input in [ordered, sampled] {
            let Ok(actual) = transpose_8x8_probe(token, input);
            let (source_columns, _) = input.as_chunks::<8>();
            let (transposed_rows, _) = actual.as_chunks::<8>();

            for (column, x) in source_columns.iter().zip(0..8_u32) {
                let Ok(offset) = index(x);

                for (&expected, row) in column.iter().zip(transposed_rows) {
                    let pixel = row.get(offset).ok_or(Rav1dError::EINVAL)?;
                    assert_eq!(*pixel, expected, "transpose mismatch at column {x}");
                }
            }
        }

        Ok(())
    }

    fn scalar_dct_rows<const WIDTH: usize, const AREA: usize>(
        coefficients: &[i16; AREA],
        bounds: CoefficientBounds,
        transform: Itx1dFn,
    ) -> Rav1dResult<[i32; AREA]> {
        let mut output = [0; AREA];
        let (columns, _) = coefficients.as_chunks::<WIDTH>();
        let (rows, _) = output.as_chunks_mut::<WIDTH>();

        for (y, row) in rows.iter_mut().enumerate() {
            for (pixel, column) in row.iter_mut().zip(columns) {
                let coefficient = column.get(y).ok_or(Rav1dError::EINVAL)?;
                *pixel = i32::from(*coefficient);
            }

            transform(row, STRIDE_1, bounds)?;
        }

        Ok(output)
    }

    pub(super) fn seeded_i16_block<const N: usize>(seed: u64) -> core::result::Result<[i16; N], console_core_never::Never> { Ok({
        let mut state = seed.wrapping_mul(6364136223846793005);
        let mut out = [0i16; N];

        for v in out.iter_mut() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);

            *v = { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(u64::wrapping_shr(state, 32_u32)); numeric_value };
        }

        out
    }) }




    #[test]
    fn test_dct_scalar_reference_helpers_are_deterministic() -> Rav1dResult {
        let Ok(row_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(row_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);

        let in8: [i16; 64] = { let Ok(explicit_value) = seeded_i16_block(0xdeadbeef); explicit_value };
        let a8 = scalar_dct_rows::<8, _>(
                &in8,
                CoefficientBounds { lower: row_min, upper: row_max },
                crate::src::itx_1d::rav1d_inv_dct8_1d_c,
            )?;
        let b8 = scalar_dct_rows::<8, _>(
                &in8,
                CoefficientBounds { lower: row_min, upper: row_max },
                crate::src::itx_1d::rav1d_inv_dct8_1d_c,
            )?;
        assert_eq!(a8, b8, "dct8 scalar reference is non-deterministic");

        let in16: [i16; 256] = { let Ok(explicit_value) = seeded_i16_block(0xc0ffee); explicit_value };
        let a16 = scalar_dct_rows::<16, _>(
                &in16,
                CoefficientBounds { lower: row_min, upper: row_max },
                crate::src::itx_1d::rav1d_inv_dct16_1d_c,
            )?;
        let b16 = scalar_dct_rows::<16, _>(
                &in16,
                CoefficientBounds { lower: row_min, upper: row_max },
                crate::src::itx_1d::rav1d_inv_dct16_1d_c,
            )?;
        assert_eq!(a16, b16, "dct16 scalar reference is non-deterministic");

        let in32: [i16; 1024] = { let Ok(explicit_value) = seeded_i16_block(0xfeedface); explicit_value };
        let a32 = scalar_dct_rows::<32, _>(
                &in32,
                CoefficientBounds { lower: row_min, upper: row_max },
                crate::src::itx_1d::rav1d_inv_dct32_1d_c,
            )?;
        let b32 = scalar_dct_rows::<32, _>(
                &in32,
                CoefficientBounds { lower: row_min, upper: row_max },
                crate::src::itx_1d::rav1d_inv_dct32_1d_c,
            )?;
        assert_eq!(a32, b32, "dct32 scalar reference is non-deterministic");
        Ok(())
    }











    #[test]
    fn test_dct8_row_pass_i16_simd_matches_scalar() -> Rav1dResult {
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let token = match avx2 { Some (token) => token, None => {
            eprintln!("Skipping: AVX2 not available");
            return Ok(());
        } };
        let Ok(row_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(row_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);
        let seeds: &[u64] = &[
            0xdeadbeef,
            0xc0ffee,
            0xfeedface,
            0xbaadf00d,
            0x12345678,
            0xa5a5a5a5,
            0x5a5a5a5a,
            0xffff_ffff_ffff_ffff,
        ];

        for &seed in seeds {
            let input: [i16; 64] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };
            let scalar_out = scalar_dct_rows::<8, _>(
                &input,
                CoefficientBounds { lower: row_min, upper: row_max },
                crate::src::itx_1d::rav1d_inv_dct8_1d_c,
            )?;
            let Ok(simd_out) = dct8_row_pass_i16_simd(token, input);

            assert_eq!(simd_out, scalar_out, "seed {seed:#x}");
        }

        Ok(())
    }











    #[test]
    fn test_dct8_col_pass_i16_matches_i32_col_pass() -> Rav1dResult {
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let token = match avx2 { Some (token) => token, None => {
            eprintln!("Skipping: AVX2 not available");
            return Ok(());
        } };


        let seeds: &[u64] = &[
            0xdeadbeef,
            0xc0ffee,
            0xfeedface,
            0xbaadf00d,
            0x12345678,
            0xa5a5a5a5,
            0x5a5a5a5a,
            0xffff_ffff_ffff_ffff,
            0x0,
            0x7fff_7fff_7fff_7fff,
            0x8000_8000_8000_8000,
        ];
        let Ok(row_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(row_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);
        let Ok(col_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(col_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);

        for &seed in seeds {
            let input: [i16; 64] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };



            let row_out = scalar_dct_rows::<8, _>(
                &input,
                CoefficientBounds { lower: row_min, upper: row_max },
                crate::src::itx_1d::rav1d_inv_dct8_1d_c,
            )?;

            let mut scalar_tmp = [0i32; 64];

            for (i, scalar_tmp_element) in <[_]>::iter_mut(&mut scalar_tmp).enumerate() {
                *scalar_tmp_element = i32::wrapping_shr(i32::wrapping_add(*(match <[_]>::get(&row_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), 1), 1_u32).clamp(col_min, col_max);
            }

            let mut scalar_col_out = [0i32; 64];

            for x in 0..8 {
                let mut col = [0i32; 8];

                for (y, col_element) in <[_]>::iter_mut(&mut col).enumerate() {
                    *col_element = *(match <[_]>::get(&scalar_tmp, y.wrapping_mul(8).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) });
                }

                assert_eq!(crate::src::itx_1d::rav1d_inv_dct8_1d_c(
                    &mut col,
                    STRIDE_1,
                    CoefficientBounds { lower: col_min, upper: col_max },
                ), Ok(()));

                for (y, col_element) in <[_]>::iter(&col).enumerate() {
                    *(match <[_]>::get_mut(&mut scalar_col_out, y.wrapping_mul(8).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = *col_element;
                }
            }


            let mut dst_simd = [128u8; 64];
            let mut coeff_simd = input;
            inv_txfm_add_dct_dct_8x8_8bpc_avx2_inner(
                token,
                &mut dst_simd,
                8,
                &mut coeff_simd,
                255,
            )?;


            let mut dst_scalar = [128u8; 64];

            for y in match ::console_core_number_conversion::indices::<::core::primitive::u16>(0..8) { ::core::result::Result::Ok(y) => y } {
                for x in 0..8 {
                    let c = *(match <[_]>::get(&scalar_col_out, y.wrapping_mul(8).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) });
                    let scaled = i32::wrapping_shr(i32::wrapping_add(c, 8), 4_u32);
                    let p = i32::wrapping_add({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(*(match <[_]>::get(&dst_scalar, y.wrapping_mul(8).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })); numeric_value }, scaled).clamp(0, 255);
                    *(match <[_]>::get_mut(&mut dst_scalar, y.wrapping_mul(8).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(p); numeric_value };
                }
            }


            assert_eq!(dst_simd, dst_scalar, "seed {seed:#x}");
        }

        Ok(())
    }







    #[cfg(target_arch = "x86_64")]
  #[arcane]
  #[must_use]
  fn test_dct16_col_i16_pipeline(_token: Desktop64, input: [i16; 256]) -> Rav1dResult<[i32; 256]>{
        let col_min = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN);
        let col_max = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX);


        let Ok(simd_row_out) = dct16_row_pass_i16_simd(_token, input);


        let mut simd_tmp = [0i32; 256];
        let rnd_v = _mm256_set1_epi32(2);
        let col_min_v = _mm256_set1_epi32(col_min);
        let col_max_v = _mm256_set1_epi32(col_max);

        let (input_chunks, _) = simd_row_out.as_chunks::<8>();
        let (output_chunks, _) = simd_tmp.as_chunks_mut::<8>();

        for (source, output) in input_chunks.iter().zip(output_chunks) {
            let values = loadu_256!(source);
            let shifted = _mm256_srai_epi32::<2>(_mm256_add_epi32(values, rnd_v));
            let clamped = _mm256_max_epi32(_mm256_min_epi32(shifted, col_max_v), col_min_v);
            storeu_256!(output, clamped);
        }

        Ok({ let Ok(column_values) = dct16_col_pass_i16(_token, &simd_tmp); column_values })
    }







    #[test]
    fn test_dct16_col_pass_i16_matches_existing() -> Rav1dResult {
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let token = match avx2 { Some (token) => token, None => {
            eprintln!("Skipping: AVX2 not available");
            return Ok(());
        } };
        let seeds: &[u64] = &[
            0xdeadbeef,
            0xc0ffee,
            0xfeedface,
            0xbaadf00d,
            0x12345678,
            0xa5a5a5a5,
            0x5a5a5a5a,
            0xffff_ffff_ffff_ffff,
            0x0,
            0x7fff_7fff_7fff_7fff,
            0x8000_8000_8000_8000,
        ];
        let Ok(row_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(row_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);
        let Ok(col_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(col_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);

        for &seed in seeds {
            let input: [i16; 256] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };



            let row_out = scalar_dct_rows::<16, _>(
                &input,
                CoefficientBounds { lower: row_min, upper: row_max },
                crate::src::itx_1d::rav1d_inv_dct16_1d_c,
            )?;

            let mut scalar_tmp = [0i32; 256];

            for (i, scalar_tmp_element) in <[_]>::iter_mut(&mut scalar_tmp).enumerate() {
                *scalar_tmp_element = i32::wrapping_shr(i32::wrapping_add(*(match <[_]>::get(&row_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), 2), 2_u32).clamp(col_min, col_max);
            }

            let mut scalar_col_out = [0i32; 256];

            for x in 0..16 {
                let mut col = [0i32; 16];

                for (y, col_element) in <[_]>::iter_mut(&mut col).enumerate() {
                    *col_element = *(match <[_]>::get(&scalar_tmp, y.wrapping_mul(16).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) });
                }

                assert_eq!(crate::src::itx_1d::rav1d_inv_dct16_1d_c(
                    &mut col,
                    STRIDE_1,
                    CoefficientBounds { lower: col_min, upper: col_max },
                ), Ok(()));

                for (y, col_element) in <[_]>::iter(&col).enumerate() {
                    *(match <[_]>::get_mut(&mut scalar_col_out, y.wrapping_mul(16).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = *col_element;
                }
            }


            let simd_col_out = test_dct16_col_i16_pipeline(token, input)?;


            let mut dst_scalar = [128u8; 256];
            let mut dst_simd = [128u8; 256];

            for y in match ::console_core_number_conversion::indices::<::core::primitive::u16>(0..16) { ::core::result::Result::Ok(y) => y } {
                for x in 0..16 {
                    let sc = *(match <[_]>::get(&scalar_col_out, y.wrapping_mul(16).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) });
                    let scaled = i32::wrapping_shr(i32::wrapping_add(sc, 8), 4_u32);
                    let p = i32::wrapping_add(128i32, scaled).clamp(0, 255);
                    *(match <[_]>::get_mut(&mut dst_scalar, y.wrapping_mul(16).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(p); numeric_value };

                    let si = *(match <[_]>::get(&simd_col_out, y.wrapping_mul(16).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) });
                    let scaled_2 = i32::wrapping_shr(i32::wrapping_add(si, 8), 4_u32);
                    let p_2 = i32::wrapping_add(128i32, scaled_2).clamp(0, 255);
                    *(match <[_]>::get_mut(&mut dst_simd, y.wrapping_mul(16).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(p_2); numeric_value };
                }
            }

            assert_eq!(dst_simd, dst_scalar, "seed {seed:#x}");
        }

        Ok(())
    }




    #[cfg(target_arch = "x86_64")]
    #[arcane]
    #[must_use]
    fn test_dct16_col_direct_compare(
        _token: Desktop64,
        tmp: &[i32; 256],
    ) -> Rav1dResult<([i32; 256], [i32; 256])> {
        let col_min = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN);
        let col_max = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX);


        let Ok(pmaddwd_out) = dct16_col_pass_i16(_token, tmp);


        let mut mullo_tmp = *tmp;
        let bounds = CoefficientBounds { lower: col_min, upper: col_max };
        transform_square16_columns(_token, &mut mullo_tmp, Square16Columns::Dct, bounds)?;

        Ok((pmaddwd_out, mullo_tmp))
    }

    #[test]
    fn test_dct16_col_pass_i16_vs_mullo_direct() -> Rav1dResult {
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let token = match avx2 { Some (token) => token, None => {
            eprintln!("Skipping: AVX2 not available");
            return Ok(());
        } };

        let seeds: &[u64] = &[
            0xdeadbeef,
            0xc0ffee,
            0xfeedface,
            0xbaadf00d,
            0x12345678,
            0xa5a5a5a5,
            0x5a5a5a5a,
            0xffff_ffff_ffff_ffff,
            0x0,
            0x7fff_7fff_7fff_7fff,
            0x8000_8000_8000_8000,

            0x0102030405060708,
            0xFEDCBA9876543210,
            0x1111111111111111,
            0x9999999999999999,
            0xAAAABBBBCCCCDDDD,
        ];
        let Ok(row_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(row_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);
        let Ok(col_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(col_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);

        for &seed in seeds {
            let input: [i16; 256] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };


            let row_out = scalar_dct_rows::<16, _>(
                &input,
                CoefficientBounds { lower: row_min, upper: row_max },
                crate::src::itx_1d::rav1d_inv_dct16_1d_c,
            )?;
            let mut tmp = [0i32; 256];

            for (i, tmp_element) in <[_]>::iter_mut(&mut tmp).enumerate() {
                *tmp_element = i32::wrapping_shr(i32::wrapping_add(*(match <[_]>::get(&row_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), 2), 2_u32).clamp(col_min, col_max);
            }


            let (pmaddwd_out, mullo_out) = test_dct16_col_direct_compare(token, &tmp)?;

            assert_eq!(pmaddwd_out, mullo_out, "seed {seed:#x}");
        }

        Ok(())
    }







    #[test]
    fn test_dct16_row_pass_i16_simd_matches_scalar() -> Rav1dResult {
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let token = match avx2 { Some (token) => token, None => {
            eprintln!("Skipping: AVX2 not available");
            return Ok(());
        } };
        let Ok(row_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(row_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);
        let seeds: &[u64] = &[
            0xdeadbeef, 0xc0ffee, 0xfeedface, 0xbaadf00d, 0x12345678, 0xaabbccdd, 0x11223344,
            0x55667788,
        ];

        for &seed in seeds {
            let input: [i16; 256] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };
            let scalar_out = scalar_dct_rows::<16, _>(
                &input,
                CoefficientBounds { lower: row_min, upper: row_max },
                crate::src::itx_1d::rav1d_inv_dct16_1d_c,
            )?;
            let Ok(simd_out) = dct16_row_pass_i16_simd(token, input);

            assert_eq!(simd_out, scalar_out, "seed {seed:#x}");
        }



        for nonzero_batches in [1_u64, 2] {
            let Ok(full) = seeded_i16_block::<256>(0x9e37_79b9 ^ nonzero_batches);
            let mut input = [0i16; 256];
            let Ok(active_lanes) = console_core_number_conversion::index(nonzero_batches.wrapping_mul(8));

            for (destination, source) in input.chunks_mut(16).zip(full.chunks(16)) {
                for (pixel, sample) in destination.iter_mut().zip(source).take(active_lanes) {
                    *pixel = *sample;
                }
            }

            let scalar_out = scalar_dct_rows::<16, _>(
                &input,
                CoefficientBounds { lower: row_min, upper: row_max },
                crate::src::itx_1d::rav1d_inv_dct16_1d_c,
            )?;
            let Ok(simd_out) = dct16_row_pass_i16_simd(token, input);
            assert_eq!(
                simd_out, scalar_out,
                "dct16_row_pass_i16_simd sparse ({nonzero_batches} batches) diverged from scalar"
            );
        }

    Ok(())
    }







    #[test]
    fn test_dct32_row_pass_i16_simd_matches_scalar() -> Rav1dResult {
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let token = match avx2 { Some (token) => token, None => {
            eprintln!("Skipping: AVX2 not available");
            return Ok(());
        } };
        let Ok(row_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(row_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);
        let seeds: &[u64] = &[
            0xdeadbeef,
            0xc0ffee,
            0xfeedface,
            0xbaadf00d,
            0x12345678,
            0xa5a5a5a5,
            0x5a5a5a5a,
            0xffff_ffff_ffff_ffff,
        ];

        for &seed in seeds {
            let input: [i16; 1024] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };
            let scalar_out = scalar_dct_rows::<32, _>(
                &input,
                CoefficientBounds { lower: row_min, upper: row_max },
                crate::src::itx_1d::rav1d_inv_dct32_1d_c,
            )?;
            let Ok(simd_out) = dct32_row_pass_i16_simd(token, input);

            assert_eq!(simd_out, scalar_out, "seed {seed:#x}");
        }

        Ok(())
    }





    #[test]
    fn test_dct32_row_pass_i16_simd_sparse_batches() -> Rav1dResult {
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let token = match avx2 { Some (token) => token, None => {
            eprintln!("Skipping: AVX2 not available");
            return Ok(());
        } };
        let Ok(row_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(row_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);

        for nonzero_batches in [1_u64, 2, 3, 4] {
            let Ok(full) = seeded_i16_block::<1024>(0x1234_5678 ^ nonzero_batches);
            let mut input = [0i16; 1024];
            let Ok(active_lanes) = console_core_number_conversion::index(nonzero_batches.wrapping_mul(8));

            for (destination, source) in input.chunks_mut(32).zip(full.chunks(32)) {
                for (pixel, sample) in destination.iter_mut().zip(source).take(active_lanes) {
                    *pixel = *sample;
                }
            }

            let scalar_out = scalar_dct_rows::<32, _>(
                &input,
                CoefficientBounds { lower: row_min, upper: row_max },
                crate::src::itx_1d::rav1d_inv_dct32_1d_c,
            )?;
            let Ok(simd_out) = dct32_row_pass_i16_simd(token, input);
            assert_eq!(
                simd_out, scalar_out,
                "dct32_row_pass_i16_simd sparse ({nonzero_batches} batches) diverged from scalar"
            );
        }

    Ok(())
    }














    #[test]
    fn test_simd_row_dct8_16rows_matches_scalar() -> Rav1dResult {
        let Ok(avx512) = crate::src::cpu::summon_avx512();
        let token = match avx512 { Some (token) => token, None => {
            eprintln!("Skipping: AVX-512 not available");
            return Ok(());
        } };
        let Ok(row_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(row_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);
        let seeds: &[u64] = &[
            0xdeadbeef,
            0xc0ffee,
            0xfeedface,
            0xbaadf00d,
            0x12345678,
            0xa5a5a5a5,
            0x5a5a5a5a,
            0xffff_ffff_ffff_ffff,
        ];

        for &seed in seeds {

            let input: [i16; 128] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };

            let mut scalar = [0i32; 128];

            let (input_columns, _) = input.as_chunks::<16>();
            let (output_rows, _) = scalar.as_chunks_mut::<8>();

            for (y, destination) in output_rows.iter_mut().enumerate() {
                let mut row = [0i32; 8];

                for (lane, column) in row.iter_mut().zip(input_columns) {
                    let coefficient = column.get(y).ok_or(Rav1dError::EINVAL)?;
                    *lane = i32::from(*coefficient);
                }

                crate::src::itx_1d::rav1d_inv_dct8_1d_c(
                    &mut row,
                    STRIDE_1,
                    CoefficientBounds { lower: row_min, upper: row_max },
                )?;
                *destination = row;
            }

            let mut simd = [0i32; 128];
            let clipping = CoefficientBounds { lower: row_min, upper: row_max };
            simd_row_dct8_8bpc_16rows_entry(
                token,
                &input,
                0,
                &mut simd,
                RowPass {
                    coefficient_height: 16,
                    scale: RectangularScale::Skip,
                    rounding: 0,
                    shift: 0,
                    row_bounds: clipping,
                    column_bounds: clipping,
                },
            )?;

            assert_eq!(simd, scalar, "seed {seed:#x}");
        }

        Ok(())
    }


    #[test]
    fn test_simd_row_dct16_16rows_matches_scalar() -> Rav1dResult {
        let Ok(avx512) = crate::src::cpu::summon_avx512();
        let token = match avx512 { Some (token) => token, None => {
            eprintln!("Skipping: AVX-512 not available");
            return Ok(());
        } };
        let Ok(row_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(row_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);
        let seeds: &[u64] = &[
            0xdeadbeef,
            0xc0ffee,
            0xfeedface,
            0xbaadf00d,
            0x12345678,
            0xa5a5a5a5,
            0x5a5a5a5a,
            0xffff_ffff_ffff_ffff,
        ];

        for &seed in seeds {
            let input: [i16; 256] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };
            let scalar = scalar_dct_rows::<16, _>(
                &input,
                CoefficientBounds { lower: row_min, upper: row_max },
                crate::src::itx_1d::rav1d_inv_dct16_1d_c,
            )?;
            let mut simd = [0i32; 256];
            let clipping = CoefficientBounds { lower: row_min, upper: row_max };
            simd_row_dct16_8bpc_16rows_entry(
                token,
                &input,
                0,
                &mut simd,
                RowPass {
                    coefficient_height: 16,
                    scale: RectangularScale::Skip,
                    rounding: 0,
                    shift: 0,
                    row_bounds: clipping,
                    column_bounds: clipping,
                },
            )?;

            assert_eq!(simd, scalar, "seed {seed:#x}");
        }

        Ok(())
    }



    #[test]
    fn test_simd_row_dct32_16rows_matches_scalar() -> Rav1dResult {
        let Ok(avx512) = crate::src::cpu::summon_avx512();
        let token = match avx512 { Some (token) => token, None => {
            eprintln!("Skipping: AVX-512 not available");
            return Ok(());
        } };
        let Ok(row_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(row_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);
        let seeds: &[u64] = &[
            0xdeadbeef,
            0xc0ffee,
            0xfeedface,
            0xbaadf00d,
            0x12345678,
            0xa5a5a5a5,
            0x5a5a5a5a,
            0xffff_ffff_ffff_ffff,
        ];

        for &seed in seeds {

            let input: [i16; 512] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };
            let mut scalar = [0i32; 512];

            let (input_columns, _) = input.as_chunks::<16>();
            let (output_rows, _) = scalar.as_chunks_mut::<32>();

            for (y, destination) in output_rows.iter_mut().enumerate() {
                let mut row = [0i32; 32];

                for (lane, column) in row.iter_mut().zip(input_columns) {
                    let coefficient = column.get(y).ok_or(Rav1dError::EINVAL)?;
                    *lane = i32::from(*coefficient);
                }

                crate::src::itx_1d::rav1d_inv_dct32_1d_c(
                    &mut row,
                    STRIDE_1,
                    CoefficientBounds { lower: row_min, upper: row_max },
                )?;
                *destination = row;
            }

            let mut simd = [0i32; 512];
            let clipping = CoefficientBounds { lower: row_min, upper: row_max };
            simd_row_dct32_8bpc_16rows_entry(
                token,
                &input,
                0,
                &mut simd,
                RowPass {
                    coefficient_height: 16,
                    scale: RectangularScale::Skip,
                    rounding: 0,
                    shift: 0,
                    row_bounds: clipping,
                    column_bounds: clipping,
                },
            )?;

            assert_eq!(simd, scalar, "seed {seed:#x}");
        }

        Ok(())
    }





    #[test]
    fn test_dct8_cols_avx512_matches_scalar() -> Rav1dResult {
        let Ok(avx512) = crate::src::cpu::summon_avx512();
        let token = match avx512 { Some (token) => token, None => {
            eprintln!("Skipping: AVX-512 not available");
            return Ok(());
        } };
        let Ok(col_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(col_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);
        let seeds: &[u64] = &[
            0xdeadbeef,
            0xc0ffee,
            0xfeedface,
            0xbaadf00d,
            0x12345678,
            0xa5a5a5a5,
            0x5a5a5a5a,
            0xffff_ffff_ffff_ffff,
        ];

        let bounds = CoefficientBounds { lower: col_min, upper: col_max };

        for width in [16_u32, 32] {
            let size = console_core_geometry::Size { width, height: 8 };
            let count = u64::from(size.width).wrapping_mul(u64::from(size.height));
            let Ok(native_width) = console_core_number_conversion::index(width);
            let row_layout = console_core_pixel_buffers::RowLayout::new(
                console_core_geometry::Size { width: u64::from(width), height: 8 },
                0,
                i64::from(width),
            ).map_err(|_| Rav1dError::EINVAL)?;

            for &seed in seeds {
                let input: Vec<i32> = (0..count)
                    .map(|position| {
                        let sample = seed
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add(position)
                            .wrapping_mul(1442695040888963407);
                        let Ok(narrowed) = console_core_number_conversion::wrapped::<_, i16>(sample.wrapping_shr(33));
                        i32::from(narrowed)
                    })
                    .collect();
                let mut scalar = input.clone();

                for x in 0..native_width {
                    let mut column = [0i32; 8];

                    let source_rows = row_layout.rows(&input).map_err(|_| Rav1dError::EINVAL)?;

                    for (coefficient, row) in column.iter_mut().zip(source_rows) {
                        let value = row.get(x).ok_or(Rav1dError::EINVAL)?;
                        *coefficient = *value;
                    }

                    crate::src::itx_1d::rav1d_inv_dct8_1d_c(&mut column, STRIDE_1, bounds)?;

                    let destination_rows = row_layout.rows_mut(&mut scalar).map_err(|_| Rav1dError::EINVAL)?;

                    for (coefficient, row) in column.into_iter().zip(destination_rows) {
                        let value = row.get_mut(x).ok_or(Rav1dError::EINVAL)?;
                        *value = coefficient;
                    }
                }

                let mut simd = input.clone();
                dct8_cols_avx512(token, &mut simd, size, bounds)?;
                assert_eq!(simd, scalar, "seed {seed:#x}, width {width}");
            }
        }

        Ok(())
    }




    #[test]
    fn test_dct4_cols_avx512_matches_scalar() -> Rav1dResult {
        let Ok(avx512) = crate::src::cpu::summon_avx512();
        let token = match avx512 { Some (token) => token, None => {
            eprintln!("Skipping: AVX-512 not available");
            return Ok(());
        } };
        let Ok(col_min) = console_core_number_conversion::wrapped::<_, i32>(i16::MIN);
        let Ok(col_max) = console_core_number_conversion::wrapped::<_, i32>(i16::MAX);
        let seeds: &[u64] = &[
            0xdeadbeef,
            0xc0ffee,
            0xfeedface,
            0xbaadf00d,
            0x12345678,
            0xa5a5a5a5,
            0x5a5a5a5a,
            0xffff_ffff_ffff_ffff,
        ];
        let size = console_core_geometry::Size { width: 16, height: 4 };
        let bounds = CoefficientBounds { lower: col_min, upper: col_max };

        for &seed in seeds {
            let mut input = [0i32; 64];

            for (position, coefficient) in (0_u64..).zip(&mut input) {
                let sample = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(position)
                    .wrapping_mul(1442695040888963407);
                let Ok(narrowed) = console_core_number_conversion::wrapped::<_, i16>(sample.wrapping_shr(49));
                *coefficient = i32::from(narrowed);
            }

            let mut scalar = input;
            let (input_rows, _) = input.as_chunks::<16>();
            let (scalar_rows, _) = scalar.as_chunks_mut::<16>();

            for x in 0..16 {
                let mut column = [0i32; 4];

                for (coefficient, row) in column.iter_mut().zip(input_rows) {
                    let value = row.get(x).ok_or(Rav1dError::EINVAL)?;
                    *coefficient = *value;
                }

                crate::src::itx_1d::rav1d_inv_dct4_1d_c(&mut column, STRIDE_1, bounds)?;

                for (coefficient, row) in column.into_iter().zip(&mut *scalar_rows) {
                    let value = row.get_mut(x).ok_or(Rav1dError::EINVAL)?;
                    *value = coefficient;
                }
            }

            let mut simd = input;
            dct4_cols_avx512(token, &mut simd, size, bounds)?;
            assert_eq!(simd, scalar, "seed {seed:#x}");
        }

        Ok(())
    }






    #[test]
    fn test_identity_cols_avx512_matches_scalar() -> Rav1dResult {
        let Ok(avx512) = crate::src::cpu::summon_avx512();
        let token = match avx512 { Some (token) => token, None => {
            eprintln!("Skipping: AVX-512 not available");
            return Ok(());
        } };
        let seeds: &[u64] = &[
            0xdeadbeef,
            0xc0ffee,
            0xfeedface,
            0xbaadf00d,
            0x12345678,
            0xa5a5a5a5,
            0x5a5a5a5a,
            0xffff_ffff_ffff_ffff,
        ];

        #[derive(Clone, Copy, Debug)]
        enum ColumnScale {
            Double,
            Quadruple,
            Identity16,
        }

        let cases = [
            (16, 8, ColumnScale::Double),
            (32, 8, ColumnScale::Double),
            (16, 32, ColumnScale::Quadruple),
            (32, 16, ColumnScale::Identity16),
            (16, 16, ColumnScale::Identity16),
        ];

        for (width, height, scale) in cases {
            let size = console_core_geometry::Size { width, height };
            let count = u64::from(width).wrapping_mul(u64::from(height));

            for &seed in seeds {
                let input: Vec<i32> = (0..count)
                    .map(|position| {
                        let sample = seed
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add(position)
                            .wrapping_mul(1442695040888963407);
                        let Ok(value) = console_core_number_conversion::wrapped::<_, i32>(sample.wrapping_shr(40));
                        (value & 0xffff).wrapping_sub(0x8000)
                    })
                    .collect();
                let mut scalar = input.clone();

                for value in &mut scalar {
                    *value = match scale {
                        ColumnScale::Double => value.wrapping_mul(2),
                        ColumnScale::Quadruple => value.wrapping_mul(4),
                        ColumnScale::Identity16 => {
                            let rounded = value.wrapping_mul(1697).wrapping_add(1024).wrapping_shr(11);
                            value.wrapping_mul(2).wrapping_add(rounded)
                        }
                    };
                }

                let mut simd = input;

                let result = match scale {
                    ColumnScale::Double => identity_shift_cols_avx512::<1>(token, &mut simd, size),
                    ColumnScale::Quadruple => identity_shift_cols_avx512::<2>(token, &mut simd, size),
                    ColumnScale::Identity16 => identity16_cols_avx512(token, &mut simd, size),
                };
                result?;
                assert_eq!(simd, scalar, "seed {seed:#x}, width {width}, rows {height}, scale {scale:?}");
            }
        }

        Ok(())
    }
}
