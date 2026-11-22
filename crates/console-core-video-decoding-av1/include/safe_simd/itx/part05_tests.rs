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
    fn test_wht4_basic() {

         match ::core::primitive::bool::then_some(!is_x86_feature_detected!("avx2"), ()) { ::core::option::Option::Some(()) => {
            return;
        }, ::core::option::Option::None => {} }

        let mut coeff = [0i16; 16];
        *{ let [_explicit_part, ..] = &mut coeff; _explicit_part } = 64;

        let mut dst = [128u8; 16];
        let stride = 4u64;

        let token = ({ let Ok(explicit_value) = crate::src::cpu::summon_avx2(); explicit_value }).expect("AVX2 required");
        assert_eq!(inv_txfm_add_wht_wht_4x4_8bpc_avx2_inner(token, &mut dst, stride, &mut coeff, 255), Ok(()));


        assert!(dst.iter().all(|&p| p >= 128));
        assert!(coeff.iter().all(|&c| c == 0));
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
    #[must_use]
    fn itx_mul2x_pack_probe(
        _token: Desktop64,
        a: [i16; 16],
        b: [i16; 16],
        coef_a: i16,
        coef_b: i16,
    ) -> Rav1dResult<[i32; 16]> {

        let arr_a: &[i16; 16] = &a;
        let arr_b: &[i16; 16] = &b;
        let va = loadu_256!(arr_a, [i16; 16]);
        let vb = loadu_256!(arr_b, [i16; 16]);



        let lo = _mm256_unpacklo_epi16(va, vb);
        let hi = _mm256_unpackhi_epi16(va, vb);

        let Ok(r_lo) = multiply_coefficient_pairs(_token, lo, [coef_a, coef_b]);
        let Ok(r_hi) = multiply_coefficient_pairs(_token, hi, [coef_a, coef_b]);

        let mut out_lo = [0i32; 8];
        let mut out_hi = [0i32; 8];
        let arr_lo: &mut [i32; 8] = &mut out_lo;
        let arr_hi: &mut [i32; 8] = &mut out_hi;
        storeu_256!(arr_lo, [i32; 8], r_lo);
        storeu_256!(arr_hi, [i32; 8], r_hi);






        let mut out = [0i32; 16];
        (*(match <[_]>::get_mut(&mut out, 0..4) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })).copy_from_slice({ let [_explicit_part @ .., _, _, _, _] = &out_lo; let _explicit_slice: &[_] = _explicit_part; _explicit_slice });
        (*(match <[_]>::get_mut(&mut out, 4..8) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })).copy_from_slice({ let [_explicit_part @ .., _, _, _, _] = &out_hi; let _explicit_slice: &[_] = _explicit_part; _explicit_slice });
        (*(match <[_]>::get_mut(&mut out, 8..12) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })).copy_from_slice({ let [_, _, _, _, _explicit_part @ ..] = &out_lo; let _explicit_slice: &[_] = _explicit_part; _explicit_slice });
        (*(match <[_]>::get_mut(&mut out, 12..16) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })).copy_from_slice({ let [_, _, _, _, _explicit_part @ ..] = &out_hi; let _explicit_slice: &[_] = _explicit_part; _explicit_slice });
        Ok(out)
    }








    #[test]
    fn test_itx_mul2x_pack_matches_scalar() -> Rav1dResult {
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let token = match avx2 { Some (token) => token, None => {
            eprintln!("Skipping: AVX2 not available");
            return Ok(());
        } };




        let coef_pairs: &[(i16, i16)] = &[
            (799, 4017),
            (3406, 2276),
            (1567, 3784),
            (2896, 2896),
            (-2896, 2896),
            (401, 4076),
            (3166, 2598),
            (1931, 3612),
            (3920, 1189),
            (-3784, 1567),
            (i16::MIN, 1),
            (1, i16::MAX),
            (i16::MAX, i16::MIN),
        ];


        let mut state: u64 = 0xdead_beef_cafe_babe;
        let mut next_i16 = || -> i16 {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);

            { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(u64::wrapping_shr(state, 32_u32)); numeric_value }); numeric_value }
        };

        const N_TRIALS: usize = 64;
        let mut mismatches = 0u32;

        for trial in 0..N_TRIALS {


            let mut a = [0i16; 16];
            let mut b = [0i16; 16];

            for (a_element, b_element) in <[_]>::iter_mut(&mut a).zip(<[_]>::iter_mut(&mut b)) {
                *a_element = next_i16();
                *b_element = next_i16();
            }

             match ::core::primitive::bool::then_some(trial.wrapping_rem(8) == 0, ()) { ::core::option::Option::Some(()) => {
                *(match <[_]>::get_mut(&mut a, trial.wrapping_rem(16)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = i16::MAX;
                *(match <[_]>::get_mut(&mut b, trial.wrapping_add(1).wrapping_rem(16)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = i16::MIN;
            }, ::core::option::Option::None => {} }

            for &(c1, c2) in coef_pairs {
                let simd_out = itx_mul2x_pack_probe(token, a, b, c1, c2)?;

                for i in 0..16 {

                    let scalar =
                        i32::wrapping_shr(i32::wrapping_add(i32::wrapping_add(i32::wrapping_mul({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(*(match <[_]>::get(&a, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(c1); numeric_value }), i32::wrapping_mul({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(*(match <[_]>::get(&b, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })); numeric_value }, { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(c2); numeric_value })), 2048), 12_u32);

                     match ::core::primitive::bool::then_some(scalar != *(match <[_]>::get(&simd_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), ()) { ::core::option::Option::Some(()) => {
                         match ::core::primitive::bool::then_some(mismatches < 8, ()) { ::core::option::Option::Some(()) => {
                            eprintln!(
                                "MISMATCH trial={trial} i={i} c1={c1} c2={c2} a={} b={} simd={} scalar={}",
                                *(match <[_]>::get(&a, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), *(match <[_]>::get(&b, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), *(match <[_]>::get(&simd_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), scalar
                            );
                        }, ::core::option::Option::None => {} }

                        { let numeric_amount = 1; let numeric_target = &mut (mismatches); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
                    }, ::core::option::Option::None => {} }
                }
            }
        }

        assert_eq!(
            mismatches, 0,
            "Packed coefficient multiplication diverged from scalar C reference (a*c1 + b*c2 + 2048) >> 12"
        );
        Ok(())
    }








    #[arcane]
  #[must_use]
  fn transpose_8x8_probe(_token: Desktop64, input_col_major: [i32; 64]) -> Rav1dResult<[i32; 64]>{
        let mut cols = [_mm256_setzero_si256(); 8];

    for x in match ::console_core_number_conversion::indices::<::core::primitive::u16>(0..8) {
      ::core::result::Result::Ok(x) => x,
    } {
      let arr: &[i32; 8] = (&*(match <[_]>::get(
        &input_col_major,
        x.wrapping_mul(8)..x.wrapping_mul(8).wrapping_add(8),
      ) {
        Some(indexed_value) => indexed_value,
        None => return Err(Rav1dError::EINVAL),
      }))
        .try_into()
        .unwrap();
            *(match <[_]>::get_mut(&mut cols, x) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = loadu_256!(arr, [i32; 8]);
        }

        let Ok(rows) = transpose_8x8_i32(_token, cols);
        let mut out = [0i32; 64];

    for y in match ::console_core_number_conversion::indices::<::core::primitive::u16>(0..8) {
      ::core::result::Result::Ok(y) => y,
    } {
      let arr: &mut [i32; 8] = (&mut *(match <[_]>::get_mut(
        &mut out,
        y.wrapping_mul(8)..y.wrapping_mul(8).wrapping_add(8),
      ) {
        Some(indexed_value) => indexed_value,
        None => return Err(Rav1dError::EINVAL),
      }))
        .try_into()
        .unwrap();
            storeu_256!(arr, [i32; 8], *(match <[_]>::get(&rows, y) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }));
        }

        Ok(out)
    }



    #[test]
    fn test_transpose_8x8_i32_roundtrip() -> Rav1dResult {
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let token = match avx2 { Some (token) => token, None => {
            eprintln!("Skipping: AVX2 not available");
            return Ok(());
        } };


        let mut col_major = [0i32; 64];

        for x in 0..8 {
            for y in 0..8 {



                *(match <[_]>::get_mut(&mut col_major, usize::wrapping_mul(x, 8).wrapping_add(y)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = i32::wrapping_shl({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(x); numeric_value }, 24_u32) | i32::wrapping_shl({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(y); numeric_value }, 8_u32) | 0x55;
            }
        }

        let row_major = transpose_8x8_probe(token, col_major)?;

        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(
                    *(match <[_]>::get(&row_major, usize::wrapping_mul(y, 8).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                    *(match <[_]>::get(&col_major, usize::wrapping_mul(x, 8).wrapping_add(y)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                    "transpose mismatch at (x={x}, y={y})"
                );
            }
        }

        let mut state: u64 = 0xface_feed_dead_beef;
        let mut rand = || -> i32 {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);

            { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(state); numeric_value }
        };
        let mut col_major2 = [0i32; 64];

        for v in col_major2.iter_mut() {
            *v = rand();
        }

        let row_major2 = transpose_8x8_probe(token, col_major2)?;

        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(
                    *(match <[_]>::get(&row_major2, usize::wrapping_mul(y, 8).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                    *(match <[_]>::get(&col_major2, usize::wrapping_mul(x, 8).wrapping_add(y)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                    "transpose (random) mismatch at (x={x}, y={y})"
                );
            }
        }

    Ok(())
    }



















    pub(super) fn run_scalar_dct8_per_row(
        coeff_col_major: &[i16; 64],
        row_min: i32,
        row_max: i32,
    ) -> Rav1dResult<[i32; 64]> { Ok({
            let mut tmp_row_major = [0i32; 64];

        for y in 0..8 {
            let mut row = [0i32; 8];

            for (x, row_element) in <[_]>::iter_mut(&mut row).enumerate() {
                *row_element = { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(*(match <[_]>::get(&*coeff_col_major, (::core::num::Wrapping(y) + ::core::num::Wrapping(x.wrapping_mul(8))).0) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })); numeric_value };
            }

            assert_eq!(crate::src::itx_1d::rav1d_inv_dct8_1d_c(
                &mut row,
                STRIDE_1,
                CoefficientBounds { lower: row_min, upper: row_max },
            ), Ok(()));

            for (x, row_element) in <[_]>::iter(&row).enumerate() {
                *(match <[_]>::get_mut(&mut tmp_row_major, usize::wrapping_mul(y, 8).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = *row_element;
            }
        }

        tmp_row_major
    }) }



    pub(super) fn run_scalar_dct16_per_row(
        coeff_col_major: &[i16; 256],
        row_min: i32,
        row_max: i32,
    ) -> Rav1dResult<[i32; 256]> { Ok({
            let mut tmp_row_major = [0i32; 256];

        for y in 0..16 {
            let mut row = [0i32; 16];

            for (x, row_element) in <[_]>::iter_mut(&mut row).enumerate() {
                *row_element = { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(*(match <[_]>::get(&*coeff_col_major, (::core::num::Wrapping(y) + ::core::num::Wrapping(x.wrapping_mul(16))).0) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })); numeric_value };
            }

            assert_eq!(crate::src::itx_1d::rav1d_inv_dct16_1d_c(
                &mut row,
                STRIDE_1,
                CoefficientBounds { lower: row_min, upper: row_max },
            ), Ok(()));

            for (x, row_element) in <[_]>::iter(&row).enumerate() {
                *(match <[_]>::get_mut(&mut tmp_row_major, usize::wrapping_mul(y, 16).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = *row_element;
            }
        }

        tmp_row_major
    }) }



    pub(super) fn run_scalar_dct32_per_row(
        coeff_col_major: &[i16; 1024],
        row_min: i32,
        row_max: i32,
    ) -> Rav1dResult<[i32; 1024]> { Ok({
            let mut tmp_row_major = [0i32; 1024];

        for y in 0..32 {
            let mut row = [0i32; 32];

            for (x, row_element) in <[_]>::iter_mut(&mut row).enumerate() {
                *row_element = { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(*(match <[_]>::get(&*coeff_col_major, (::core::num::Wrapping(y) + ::core::num::Wrapping(x.wrapping_mul(32))).0) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })); numeric_value };
            }

            assert_eq!(crate::src::itx_1d::rav1d_inv_dct32_1d_c(
                &mut row,
                STRIDE_1,
                CoefficientBounds { lower: row_min, upper: row_max },
            ), Ok(()));

            for (x, row_element) in <[_]>::iter(&row).enumerate() {
                *(match <[_]>::get_mut(&mut tmp_row_major, usize::wrapping_mul(y, 32).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = *row_element;
            }
        }

        tmp_row_major
    }) }


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
        let a8 = run_scalar_dct8_per_row(&in8, row_min, row_max)?;
        let b8 = run_scalar_dct8_per_row(&in8, row_min, row_max)?;
        assert_eq!(a8, b8, "dct8 scalar reference is non-deterministic");

        let in16: [i16; 256] = { let Ok(explicit_value) = seeded_i16_block(0xc0ffee); explicit_value };
        let a16 = run_scalar_dct16_per_row(&in16, row_min, row_max)?;
        let b16 = run_scalar_dct16_per_row(&in16, row_min, row_max)?;
        assert_eq!(a16, b16, "dct16 scalar reference is non-deterministic");

        let in32: [i16; 1024] = { let Ok(explicit_value) = seeded_i16_block(0xfeedface); explicit_value };
        let a32 = run_scalar_dct32_per_row(&in32, row_min, row_max)?;
        let b32 = run_scalar_dct32_per_row(&in32, row_min, row_max)?;
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
        let mut total_mismatches = 0u32;

        for &seed in seeds {
            let input: [i16; 64] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };
            let scalar_out = run_scalar_dct8_per_row(&input, row_min, row_max)?;
            let simd_out = dct8_row_pass_i16_simd(token, input);

             match ::core::primitive::bool::then_some(simd_out != scalar_out, ()) { ::core::option::Option::Some(()) => {
                let mut mism = 0u32;

                for i in 0..64 {
                     match ::core::primitive::bool::then_some(*(match <[_]>::get(&simd_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&scalar_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), ()) { ::core::option::Option::Some(()) => {
                         match ::core::primitive::bool::then_some(mism < 8, ()) { ::core::option::Option::Some(()) => {
                            eprintln!(
                                "seed={:#x} idx={} row={} col={} scalar={} simd={} diff={}",
                                seed,
                                i,
                                ::core::primitive::usize::wrapping_div(i, 8),
                                ::core::primitive::usize::wrapping_rem(i, 8),
                                *(match <[_]>::get(&scalar_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                                *(match <[_]>::get(&simd_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                                (*(match <[_]>::get(&simd_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })).wrapping_sub(*(match <[_]>::get(&scalar_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }))
                            );
                        }, ::core::option::Option::None => {} }

                        { let numeric_amount = 1; let numeric_target = &mut (mism); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
                    }, ::core::option::Option::None => {} }
                }

                eprintln!("seed={seed:#x}: {mism} mismatches");

                { let numeric_amount = mism; let numeric_target = &mut (total_mismatches); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
            }, ::core::option::Option::None => {} }
        }

        assert_eq!(
            total_mismatches, 0,
            "dct8_row_pass_i16_simd diverged from scalar reference"
        );
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
        let mut total_mism = 0u32;

        for &seed in seeds {
            let input: [i16; 64] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };



            let row_out = run_scalar_dct8_per_row(&input, row_min, row_max)?;

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


            let mut dst_simd = [128u8; usize::wrapping_mul(8, 8)];
            let mut coeff_simd = input;
            inv_txfm_add_dct_dct_8x8_8bpc_avx2_inner(
                token,
                &mut dst_simd,
                8,
                &mut coeff_simd,
                255,
            );


            let mut dst_scalar = [128u8; usize::wrapping_mul(8, 8)];

            for y in match ::console_core_number_conversion::indices::<::core::primitive::u16>(0..8) { ::core::result::Result::Ok(y) => y } {
                for x in 0..8 {
                    let c = *(match <[_]>::get(&scalar_col_out, y.wrapping_mul(8).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) });
                    let scaled = i32::wrapping_shr(i32::wrapping_add(c, 8), 4_u32);
                    let p = i32::wrapping_add({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(*(match <[_]>::get(&dst_scalar, y.wrapping_mul(8).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })); numeric_value }, scaled).clamp(0, 255);
                    *(match <[_]>::get_mut(&mut dst_scalar, y.wrapping_mul(8).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u8>(p); numeric_value };
                }
            }


             match ::core::primitive::bool::then_some(dst_simd != dst_scalar, ()) { ::core::option::Option::Some(()) => {
                let mut mism = 0u32;

                for i in 0..64 {
                     match ::core::primitive::bool::then_some(*(match <[_]>::get(&dst_simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&dst_scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), ()) { ::core::option::Option::Some(()) => {
                         match ::core::primitive::bool::then_some(mism < 8, ()) { ::core::option::Option::Some(()) => {
                            eprintln!(
                                "seed={:#x} idx={} row={} col={} scalar={} simd={}",
                                seed,
                                i,
                                ::core::primitive::usize::wrapping_div(i, 8),
                                ::core::primitive::usize::wrapping_rem(i, 8),
                                *(match <[_]>::get(&dst_scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                                *(match <[_]>::get(&dst_simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                            );
                        }, ::core::option::Option::None => {} }

                        { let numeric_amount = 1; let numeric_target = &mut (mism); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
                    }, ::core::option::Option::None => {} }
                }

                eprintln!("seed={seed:#x}: {mism} dst mismatches");

                { let numeric_amount = mism; let numeric_target = &mut (total_mism); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
            }, ::core::option::Option::None => {} }
        }

        assert_eq!(
            total_mism, 0,
            "dct8_col_pass_i16 full pipeline diverged from scalar reference"
        );
        Ok(())
    }







    #[cfg(target_arch = "x86_64")]
  #[arcane]
  #[must_use]
  fn test_dct16_col_i16_pipeline(_token: Desktop64, input: [i16; 256]) -> Rav1dResult<[i32; 256]>{
        let col_min = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN);
        let col_max = <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX);


        let simd_row_out = dct16_row_pass_i16_simd(_token, input);


        let mut simd_tmp = [0i32; 256];
        let rnd_v = _mm256_set1_epi32(2);
        let col_min_v = _mm256_set1_epi32(col_min);
        let col_max_v = _mm256_set1_epi32(col_max);

        for y in 0..16 {
            for chunk in 0..2u32 {
                let ::core::result::Result::Ok(b) = ::console_core_number_conversion::fixed_index({ let ::core::result::Result::Ok(explicit_position) = ::console_core_number_conversion::wrapped_index::<::core::primitive::u32>(::core::primitive::u32::wrapping_mul(chunk, 8)); explicit_position });
                let off = ::core::primitive::usize::wrapping_mul(y, 16).wrapping_add(match ::console_core_number_conversion::index(b) { ::core::result::Result::Ok(native_position) => native_position });
                let v = loadu_256!(&*(match <[_]>::get(&simd_row_out, off..off.wrapping_add(8)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), [i32; 8]);
                let shifted = _mm256_srai_epi32::<2>(_mm256_add_epi32(v, rnd_v));
                let clamped = _mm256_max_epi32(_mm256_min_epi32(shifted, col_max_v), col_min_v);
                storeu_256!(&mut *(match <[_]>::get_mut(&mut simd_tmp, off..off.wrapping_add(8)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), [i32; 8], clamped);
            }
        }


        Ok(dct16_col_pass_i16(_token, &simd_tmp))
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
        let mut total_mism = 0u32;

        for &seed in seeds {
            let input: [i16; 256] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };



            let row_out = run_scalar_dct16_per_row(&input, row_min, row_max)?;

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


            let mut dst_scalar = [128u8; usize::wrapping_mul(16, 16)];
            let mut dst_simd = [128u8; usize::wrapping_mul(16, 16)];

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

             match ::core::primitive::bool::then_some(dst_simd != dst_scalar, ()) { ::core::option::Option::Some(()) => {
                let mut mism = 0u32;

                for i in 0..256 {
                     match ::core::primitive::bool::then_some(*(match <[_]>::get(&dst_simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&dst_scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), ()) { ::core::option::Option::Some(()) => {
                         match ::core::primitive::bool::then_some(mism < 8, ()) { ::core::option::Option::Some(()) => {
                            eprintln!(
                                "seed={:#x} idx={} row={} col={} scalar={} simd={}",
                                seed,
                                i,
                                ::core::primitive::usize::wrapping_div(i, 16),
                                ::core::primitive::usize::wrapping_rem(i, 16),
                                *(match <[_]>::get(&dst_scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                                *(match <[_]>::get(&dst_simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                            );
                        }, ::core::option::Option::None => {} }

                        { let numeric_amount = 1; let numeric_target = &mut (mism); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
                    }, ::core::option::Option::None => {} }
                }

                eprintln!("seed={seed:#x}: {mism} dst mismatches");

                { let numeric_amount = mism; let numeric_target = &mut (total_mism); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
            }, ::core::option::Option::None => {} }
        }

        assert_eq!(
            total_mism, 0,
            "dct16_col_pass_i16 full pipeline diverged from scalar reference"
        );
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


        let pmaddwd_out = dct16_col_pass_i16(_token, tmp);


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
        let mut total_mism = 0u32;

        for &seed in seeds {
            let input: [i16; 256] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };


            let row_out = run_scalar_dct16_per_row(&input, row_min, row_max)?;
            let mut tmp = [0i32; 256];

            for (i, tmp_element) in <[_]>::iter_mut(&mut tmp).enumerate() {
                *tmp_element = i32::wrapping_shr(i32::wrapping_add(*(match <[_]>::get(&row_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), 2), 2_u32).clamp(col_min, col_max);
            }


            let (pmaddwd_out, mullo_out) = test_dct16_col_direct_compare(token, &tmp)?;

             match ::core::primitive::bool::then_some(pmaddwd_out != mullo_out, ()) { ::core::option::Option::Some(()) => {
                let mut mism = 0u32;

                for i in 0..256 {
                     match ::core::primitive::bool::then_some(*(match <[_]>::get(&pmaddwd_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&mullo_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), ()) { ::core::option::Option::Some(()) => {
                         match ::core::primitive::bool::then_some(mism < 8, ()) { ::core::option::Option::Some(()) => {
                            eprintln!(
                                "seed={:#x} idx={} row={} col={} mullo={} pmaddwd={}",
                                seed,
                                i,
                                ::core::primitive::usize::wrapping_div(i, 16),
                                ::core::primitive::usize::wrapping_rem(i, 16),
                                *(match <[_]>::get(&mullo_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                                *(match <[_]>::get(&pmaddwd_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                            );
                        }, ::core::option::Option::None => {} }

                        { let numeric_amount = 1; let numeric_target = &mut (mism); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
                    }, ::core::option::Option::None => {} }
                }

                eprintln!("seed={seed:#x}: {mism} col-pass mismatches");

                { let numeric_amount = mism; let numeric_target = &mut (total_mism); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
            }, ::core::option::Option::None => {} }
        }

        assert_eq!(
            total_mism, 0,
            "The packed DCT column pass diverged from the direct multiplication path"
        );
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
        let mut total_mismatches = 0u32;

        for &seed in seeds {
            let input: [i16; 256] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };
            let scalar_out = run_scalar_dct16_per_row(&input, row_min, row_max)?;
            let simd_out = dct16_row_pass_i16_simd(token, input);

             match ::core::primitive::bool::then_some(simd_out != scalar_out, ()) { ::core::option::Option::Some(()) => {
                let mut mism = 0u32;

                for i in 0..256 {
                     match ::core::primitive::bool::then_some(*(match <[_]>::get(&simd_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&scalar_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), ()) { ::core::option::Option::Some(()) => {
                         match ::core::primitive::bool::then_some(mism < 16, ()) { ::core::option::Option::Some(()) => {
                            eprintln!(
                                "seed={:#x} idx={} row={} col={} scalar={} simd={} diff={}",
                                seed,
                                i,
                                ::core::primitive::usize::wrapping_div(i, 16),
                                ::core::primitive::usize::wrapping_rem(i, 16),
                                *(match <[_]>::get(&scalar_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                                *(match <[_]>::get(&simd_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                                (*(match <[_]>::get(&simd_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })).wrapping_sub(*(match <[_]>::get(&scalar_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }))
                            );
                        }, ::core::option::Option::None => {} }

                        { let numeric_amount = 1; let numeric_target = &mut (mism); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
                    }, ::core::option::Option::None => {} }
                }

                eprintln!("seed={seed:#x}: {mism} mismatches");

                { let numeric_amount = mism; let numeric_target = &mut (total_mismatches); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
            }, ::core::option::Option::None => {} }
        }

        assert_eq!(
            total_mismatches, 0,
            "dct16_row_pass_i16_simd diverged from scalar reference"
        );


        for nonzero_batches in [1usize, 2] {
            let full: [i16; 256] = { let Ok(explicit_value) = seeded_i16_block(0x9e37_79b9 ^ ({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u64>(nonzero_batches); numeric_value })); explicit_value };
            let mut input = [0i16; 256];

            for (i, (input_element, full_element)) in <[_]>::iter_mut(&mut input).zip(<[_]>::iter(&full)).enumerate() {
                 match ::core::primitive::bool::then_some(i.wrapping_rem(16) < usize::wrapping_mul(nonzero_batches, 8), ()) { ::core::option::Option::Some(()) => {
                    *input_element = *full_element;
                }, ::core::option::Option::None => {} }
            }

            let scalar_out = run_scalar_dct16_per_row(&input, row_min, row_max)?;
            let simd_out = dct16_row_pass_i16_simd(token, input);
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
        let mut total_mismatches = 0u32;

        for &seed in seeds {
            let input: [i16; 1024] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };
            let scalar_out = run_scalar_dct32_per_row(&input, row_min, row_max)?;
            let simd_out = dct32_row_pass_i16_simd(token, input);

             match ::core::primitive::bool::then_some(simd_out != scalar_out, ()) { ::core::option::Option::Some(()) => {
                let mut mism = 0u32;

                for i in 0..1024 {
                     match ::core::primitive::bool::then_some(*(match <[_]>::get(&simd_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&scalar_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), ()) { ::core::option::Option::Some(()) => {
                         match ::core::primitive::bool::then_some(mism < 8, ()) { ::core::option::Option::Some(()) => {
                            eprintln!(
                                "seed={:#x} idx={} row={} col={} scalar={} simd={} diff={}",
                                seed,
                                i,
                                ::core::primitive::usize::wrapping_div(i, 32),
                                ::core::primitive::usize::wrapping_rem(i, 32),
                                *(match <[_]>::get(&scalar_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                                *(match <[_]>::get(&simd_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                                (*(match <[_]>::get(&simd_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })).wrapping_sub(*(match <[_]>::get(&scalar_out, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }))
                            );
                        }, ::core::option::Option::None => {} }

                        { let numeric_amount = 1; let numeric_target = &mut (mism); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
                    }, ::core::option::Option::None => {} }
                }

                eprintln!("seed={seed:#x}: {mism} mismatches");

                { let numeric_amount = mism; let numeric_target = &mut (total_mismatches); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
            }, ::core::option::Option::None => {} }
        }

        assert_eq!(
            total_mismatches, 0,
            "dct32_row_pass_i16_simd diverged from scalar reference"
        );
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

        for nonzero_batches in [1usize, 2, 3, 4] {
            let full: [i16; 1024] = { let Ok(explicit_value) = seeded_i16_block(0x1234_5678 ^ ({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u64>(nonzero_batches); numeric_value })); explicit_value };
            let mut input = [0i16; 1024];

            for (i, (input_element, full_element)) in <[_]>::iter_mut(&mut input).zip(<[_]>::iter(&full)).enumerate() {
                 match ::core::primitive::bool::then_some(i.wrapping_rem(32) < usize::wrapping_mul(nonzero_batches, 8), ()) { ::core::option::Option::Some(()) => {
                    *input_element = *full_element;
                }, ::core::option::Option::None => {} }
            }

            let scalar_out = run_scalar_dct32_per_row(&input, row_min, row_max)?;
            let simd_out = dct32_row_pass_i16_simd(token, input);
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
        let mut total = 0u32;

        for &seed in seeds {

            let input: [i16; 128] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };

            let mut scalar = [0i32; 128];

            for y in 0..16 {
                let mut row = [0i32; 8];

                for (x, row_element) in <[_]>::iter_mut(&mut row).enumerate() {
                    *row_element = { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(*(match <[_]>::get(&input, (::core::num::Wrapping(y) + ::core::num::Wrapping(x.wrapping_mul(16))).0) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })); numeric_value };
                }

                assert_eq!(crate::src::itx_1d::rav1d_inv_dct8_1d_c(
                    &mut row,
                    STRIDE_1,
                    CoefficientBounds { lower: row_min, upper: row_max },
                ), Ok(()));

                for (x, row_element) in <[_]>::iter(&row).enumerate() {
                    *(match <[_]>::get_mut(&mut scalar, usize::wrapping_mul(y, 8).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = *row_element;
                }
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

             match ::core::primitive::bool::then_some(simd != scalar, ()) { ::core::option::Option::Some(()) => {
                for i in 0..128 {
                     match ::core::primitive::bool::then_some(*(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) && total < 8, ()) { ::core::option::Option::Some(()) => {
                        eprintln!(
                            "dct8-16r seed={seed:#x} idx={i} row={} col={} scalar={} simd={}",
                            ::core::primitive::usize::wrapping_div(i, 8),
                            ::core::primitive::usize::wrapping_rem(i, 8),
                            *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                            *(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })
                        );
                    }, ::core::option::Option::None => {} }

                     match ::core::primitive::bool::then_some(*(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), ()) { ::core::option::Option::Some(()) => {
                        { let numeric_amount = 1; let numeric_target = &mut (total); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
                    }, ::core::option::Option::None => {} }
                }
            }, ::core::option::Option::None => {} }
        }

        assert_eq!(total, 0, "simd_row_dct8_8bpc_16rows diverged from scalar");
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
        let mut total = 0u32;

        for &seed in seeds {
            let input: [i16; 256] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };
            let scalar = run_scalar_dct16_per_row(&input, row_min, row_max)?;
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

             match ::core::primitive::bool::then_some(simd != scalar, ()) { ::core::option::Option::Some(()) => {
                for i in 0..256 {
                     match ::core::primitive::bool::then_some(*(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) && total < 8, ()) { ::core::option::Option::Some(()) => {
                        eprintln!(
                            "dct16-16r seed={seed:#x} idx={i} row={} col={} scalar={} simd={}",
                            ::core::primitive::usize::wrapping_div(i, 16),
                            ::core::primitive::usize::wrapping_rem(i, 16),
                            *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                            *(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })
                        );
                    }, ::core::option::Option::None => {} }

                     match ::core::primitive::bool::then_some(*(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), ()) { ::core::option::Option::Some(()) => {
                        { let numeric_amount = 1; let numeric_target = &mut (total); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
                    }, ::core::option::Option::None => {} }
                }
            }, ::core::option::Option::None => {} }
        }

        assert_eq!(total, 0, "simd_row_dct16_8bpc_16rows diverged from scalar");
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
        let mut total = 0u32;

        for &seed in seeds {

            let input: [i16; 512] = { let Ok(explicit_value) = seeded_i16_block(seed); explicit_value };
            let mut scalar = [0i32; 512];

            for y in 0..16 {
                let mut row = [0i32; 32];

                for (x, row_element) in <[_]>::iter_mut(&mut row).enumerate() {
                    *row_element = { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(*(match <[_]>::get(&input, (::core::num::Wrapping(y) + ::core::num::Wrapping(x.wrapping_mul(16))).0) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })); numeric_value };
                }

                assert_eq!(crate::src::itx_1d::rav1d_inv_dct32_1d_c(
                    &mut row,
                    STRIDE_1,
                    CoefficientBounds { lower: row_min, upper: row_max },
                ), Ok(()));

                for (x, row_element) in <[_]>::iter(&row).enumerate() {
                    *(match <[_]>::get_mut(&mut scalar, usize::wrapping_mul(y, 32).wrapping_add(x)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = *row_element;
                }
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

             match ::core::primitive::bool::then_some(simd != scalar, ()) { ::core::option::Option::Some(()) => {
                for i in 0..512 {
                     match ::core::primitive::bool::then_some(*(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) && total < 8, ()) { ::core::option::Option::Some(()) => {
                        eprintln!(
                            "dct32-16r seed={seed:#x} idx={i} row={} col={} scalar={} simd={}",
                            ::core::primitive::usize::wrapping_div(i, 32),
                            ::core::primitive::usize::wrapping_rem(i, 32),
                            *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                            *(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })
                        );
                    }, ::core::option::Option::None => {} }

                     match ::core::primitive::bool::then_some(*(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), ()) { ::core::option::Option::Some(()) => {
                        { let numeric_amount = 1; let numeric_target = &mut (total); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
                    }, ::core::option::Option::None => {} }
                }
            }, ::core::option::Option::None => {} }
        }

        assert_eq!(total, 0, "simd_row_dct32_8bpc_16rows diverged from scalar");
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
        let mut total = 0u32;

        for &total_w in &[16usize, 32usize] {
            for &seed in seeds {


                let n = usize::wrapping_mul(total_w, 8);
                let raw: Vec<i16> = (0..n)
                    .map(|i| {
                        let s = seed
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u64>(i); numeric_value })
                            .wrapping_mul(1442695040888963407);

                        { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(u64::wrapping_shr(s, 33_u32)); numeric_value }
                    })
                    .collect();
                let mut input = vec![0i32; n];

                for i in 0..n {
                    *(match <[_]>::get_mut(&mut input, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(*(match <[_]>::get(&raw, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })); numeric_value };
                }


                let mut scalar = input.clone();

                for cx in 0..total_w {
                    let mut col = [0i32; 8];

                    for (r, col_element) in <[_]>::iter_mut(&mut col).enumerate() {
                        *col_element = *(match <[_]>::get(&input, r.wrapping_mul(total_w).wrapping_add(cx)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) });
                    }

                    assert_eq!(crate::src::itx_1d::rav1d_inv_dct8_1d_c(
                        &mut col,
                        STRIDE_1,
                        CoefficientBounds { lower: col_min, upper: col_max },
                    ), Ok(()));

                    for (r, col_element) in <[_]>::iter(&col).enumerate() {
                        *(match <[_]>::get_mut(&mut scalar, r.wrapping_mul(total_w).wrapping_add(cx)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = *col_element;
                    }
                }

                let mut simd = input.clone();
                dct8_cols_avx512(token, &mut simd, console_core_geometry::Size { width: { let checked_value = u32::try_from(total_w).map_err(|_| Rav1dError::EINVAL)?; checked_value }, height: 8 }, CoefficientBounds { lower: col_min, upper: col_max })?;

                 match ::core::primitive::bool::then_some(simd != scalar, ()) { ::core::option::Option::Some(()) => {
                    for i in 0..n {
                         match ::core::primitive::bool::then_some(*(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) && total < 8, ()) { ::core::option::Option::Some(()) => {
                            eprintln!(
                                "dct8-cols512 w={total_w} seed={seed:#x} idx={i} row={} col={} scalar={} simd={}",
                                i / total_w,
                                i % total_w,
                                *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                                *(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })
                            );
                        }, ::core::option::Option::None => {} }

                         match ::core::primitive::bool::then_some(*(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), ()) { ::core::option::Option::Some(()) => {
                            { let numeric_amount = 1; let numeric_target = &mut (total); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
                        }, ::core::option::Option::None => {} }
                    }
                }, ::core::option::Option::None => {} }
            }
        }

        assert_eq!(total, 0, "dct8_cols_avx512 diverged from scalar");
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
        let total_w = 16usize;
        let n_rows = 4usize;
        let mut total = 0u32;

        for &seed in seeds {
            let n = total_w.wrapping_mul(n_rows);
            let input: Vec<i32> = (0..n)
                .map(|i| {
                    let s = seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u64>(i); numeric_value })
                        .wrapping_mul(1442695040888963407);

                    { let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i16>(u64::wrapping_shr(s, 49_u32)); numeric_value }); numeric_value }
                })
                .collect();

            let mut scalar = input.clone();

            for cx in 0..total_w {
                let mut col = [0i32; 4];

                for r in 0..n_rows {
                    *(match <[_]>::get_mut(&mut col, r) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = *(match <[_]>::get(&input, r.wrapping_mul(total_w).wrapping_add(cx)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) });
                }

                assert_eq!(crate::src::itx_1d::rav1d_inv_dct4_1d_c(
                    &mut col,
                    STRIDE_1,
                    CoefficientBounds { lower: col_min, upper: col_max },
                ), Ok(()));

                for r in 0..n_rows {
                    *(match <[_]>::get_mut(&mut scalar, r.wrapping_mul(total_w).wrapping_add(cx)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = *(match <[_]>::get(&col, r) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) });
                }
            }

            let mut simd = input.clone();
            dct4_cols_avx512(token, &mut simd, console_core_geometry::Size { width: { let checked_value = u32::try_from(total_w).map_err(|_| Rav1dError::EINVAL)?; checked_value }, height: { let checked_value_2 = u32::try_from(n_rows).map_err(|_| Rav1dError::EINVAL)?; checked_value_2 } }, CoefficientBounds { lower: col_min, upper: col_max })?;

             match ::core::primitive::bool::then_some(simd != scalar, ()) { ::core::option::Option::Some(()) => {
                for i in 0..n {
                     match ::core::primitive::bool::then_some(*(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) && total < 8, ()) { ::core::option::Option::Some(()) => {
                        eprintln!(
                            "dct4-cols512 seed={seed:#x} idx={i} row={} col={} scalar={} simd={}",
                            i.wrapping_div(total_w),
                            i.wrapping_rem(total_w),
                            *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }),
                            *(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })
                        );
                    }, ::core::option::Option::None => {} }

                     match ::core::primitive::bool::then_some(*(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), ()) { ::core::option::Option::Some(()) => {
                        { let numeric_amount = 1; let numeric_target = &mut (total); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
                    }, ::core::option::Option::None => {} }
                }
            }, ::core::option::Option::None => {} }
        }

        assert_eq!(total, 0, "dct4_cols_avx512 diverged from scalar");
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

        let cases: &[(usize, usize, u8)] = &[
            (16, 8, 1),
            (32, 8, 1),
            (16, 32, 2),
            (32, 16, 16),
            (16, 16, 16),
        ];
        let mut total = 0u32;

        for &(total_w, n_rows, kind) in cases {
            for &seed in seeds {
                let n = usize::wrapping_mul(total_w, n_rows);

                let input: Vec<i32> = (0..n)
                    .map(|i| {
                        let s = seed
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u64>(i); numeric_value })
                            .wrapping_mul(1442695040888963407);
                        i32::wrapping_sub(({ let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(u64::wrapping_shr(s, 40_u32)); numeric_value }) & 0xffff, 0x8000)
                    })
                    .collect();


                let mut scalar = input.clone();

                for cx in 0..total_w {
                    for r in 0..n_rows {
                        let v = *(match <[_]>::get(&input, usize::wrapping_mul(r, total_w).wrapping_add(cx)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) });
                        *(match <[_]>::get_mut(&mut scalar, usize::wrapping_mul(r, total_w).wrapping_add(cx)) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) = match kind {
                            1 => i32::wrapping_mul(v, 2),
                            2 => i32::wrapping_mul(v, 4),
                            _ => i32::wrapping_add(i32::wrapping_mul(2, v), i32::wrapping_shr(i32::wrapping_add(i32::wrapping_mul(v, 1697), 1024), 11_u32)),
                        };
                    }
                }

                let mut simd = input.clone();

                match kind {
                    1 => identity_shift_cols_avx512::<1>(token, &mut simd, console_core_geometry::Size { width: { let checked_value = u32::try_from(total_w).map_err(|_| Rav1dError::EINVAL)?; checked_value }, height: { let checked_value_2 = u32::try_from(n_rows).map_err(|_| Rav1dError::EINVAL)?; checked_value_2 } })?,
                    2 => identity_shift_cols_avx512::<2>(token, &mut simd, console_core_geometry::Size { width: { let checked_value_3 = u32::try_from(total_w).map_err(|_| Rav1dError::EINVAL)?; checked_value_3 }, height: { let checked_value_4 = u32::try_from(n_rows).map_err(|_| Rav1dError::EINVAL)?; checked_value_4 } })?,
                    _ => identity16_cols_avx512(token, &mut simd, console_core_geometry::Size { width: { let checked_value_5 = u32::try_from(total_w).map_err(|_| Rav1dError::EINVAL)?; checked_value_5 }, height: { let checked_value_6 = u32::try_from(n_rows).map_err(|_| Rav1dError::EINVAL)?; checked_value_6 } })?,
                }

                 match ::core::primitive::bool::then_some(simd != scalar, ()) { ::core::option::Option::Some(()) => {
                    for i in 0..n {
                         match ::core::primitive::bool::then_some(*(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) && total < 8, ()) { ::core::option::Option::Some(()) => {
                            eprintln!(
                                "idtx512 kind={kind} w={total_w} rows={n_rows} seed={seed:#x} idx={i} scalar={} simd={}",
                                *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), *(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) })
                            );
                        }, ::core::option::Option::None => {} }

                         match ::core::primitive::bool::then_some(*(match <[_]>::get(&simd, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }) != *(match <[_]>::get(&scalar, i) { Some(indexed_value) => indexed_value, None => return Err(Rav1dError::EINVAL) }), ()) { ::core::option::Option::Some(()) => {
                            { let numeric_amount = 1; let numeric_target = &mut (total); *numeric_target = u32::wrapping_add(*numeric_target, numeric_amount); };
                        }, ::core::option::Option::None => {} }
                    }
                }, ::core::option::Option::None => {} }
            }
        }

        assert_eq!(total, 0, "identity AVX-512 column pass diverged from scalar");
        Ok(())
    }
}
