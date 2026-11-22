

#[cfg(all(
    test,
    target_arch = "x86_64",
    feature = "bitdepth_8",
    not(feature = "c-ffi")
))]
mod mixed_parity_tests {
    use super::*;
    use crate::include::common::bitdepth::BitDepth8;
    use crate::include::dav1d::picture::Rav1dPictureDataComponent;
    use crate::src::levels::{TxClass, TxfmSize};
    use crate::src::owned_recon::ReconDst;
    use crate::src::scan::dav1d_scans;
    use crate::src::tables::dav1d_tx_type_class;

    const TYPES: [TxfmType; 14] = [
        ADST_DCT,
        DCT_ADST,
        ADST_ADST,
        FLIPADST_DCT,
        DCT_FLIPADST,
        FLIPADST_FLIPADST,
        ADST_FLIPADST,
        FLIPADST_ADST,
        H_DCT,
        V_DCT,
        H_ADST,
        V_ADST,
        H_FLIPADST,
        V_FLIPADST,
    ];

    fn next(state: &mut u64) -> Result<u64, Never> {
        *state ^= state.wrapping_shr(12);
        *state ^= state.wrapping_shl(25);
        *state ^= state.wrapping_shr(27);
        Ok(state.wrapping_mul(0x2545_f491_4f6c_dd1d))
    }

    #[derive(Clone, Copy)]
    enum CoefficientPattern {
        One,
        NegativeOne,
        Maximum,
        Minimum,
        Alternating,
        SparseRandom,
        LastOnly,
        FullRandom,
    }

    const PATTERNS: [CoefficientPattern; 8] = [
        CoefficientPattern::One,
        CoefficientPattern::NegativeOne,
        CoefficientPattern::Maximum,
        CoefficientPattern::Minimum,
        CoefficientPattern::Alternating,
        CoefficientPattern::SparseRandom,
        CoefficientPattern::LastOnly,
        CoefficientPattern::FullRandom,
    ];

    const PERMUTATION_PATTERNS: [CoefficientPattern; 2] = [
        CoefficientPattern::SparseRandom,
        CoefficientPattern::FullRandom,
    ];

    struct CheckCellArguments {
        eob: u32,
        pattern: CoefficientPattern,
        stride: u32,
        offset: u32,
    }

    fn coefficient_value(
        cell: &CheckCellArguments,
        position: u32,
        state: &mut u64,
    ) -> Result<i16, Never> {
        let value = match cell.pattern {
            CoefficientPattern::One => 1,
            CoefficientPattern::NegativeOne => -1,
            CoefficientPattern::Maximum => i16::MAX,
            CoefficientPattern::Minimum => i16::MIN,
            CoefficientPattern::Alternating => match position & 1 {
                0 => i16::MIN,
                _ => i16::MAX,
            },
            CoefficientPattern::SparseRandom => {
                let Ok(random) = next(state);
                let Ok(bounded) = console_core_number_conversion::wrapped::<_, i16>(random & 4095);
                bounded.wrapping_sub(2048)
            }
            CoefficientPattern::LastOnly => match position.cmp(&cell.eob) {
                core::cmp::Ordering::Equal => 256,
                core::cmp::Ordering::Less | core::cmp::Ordering::Greater => 0,
            },
            CoefficientPattern::FullRandom => {
                let Ok(random) = next(state);
                let Ok(coefficient) = console_core_number_conversion::wrapped::<_, i16>(random);
                coefficient
            }
        };

        Ok(match (position == cell.eob, value) {
            (true, 0) => 1,
            _ => value,
        })
    }

    fn mixed_coefficients(
        tx: TxfmSize,
        kind: TxfmType,
        extent: console_core_geometry::Size<u32>,
        cell: &CheckCellArguments,
        state: &mut u64,
    ) -> Rav1dResult<Vec<i16>> {
        let count = extent.width.wrapping_mul(extent.height);
        let mut input: Vec<_> = (0..count).map(|_| 0i16).chain([0x1234; 16]).collect();
        let Ok(shape) = tx.ordinal();
        let Ok(shape_index) = console_core_number_conversion::index(shape);
        let Ok(kind_index) = console_core_number_conversion::index(kind);
        let scan = ::core::result::Result::<&_, _>::copied(dav1d_scans.get(shape_index).ok_or(Rav1dError::EINVAL))?;
        let class = ::core::result::Result::<&_, _>::copied(dav1d_tx_type_class.get(kind_index).ok_or(Rav1dError::EINVAL))?;

        for ordinal in 0..=cell.eob {
            let position = match class {
                TxClass::TwoD => {
                    let Ok(scan_index) = console_core_number_conversion::index(ordinal);
                    let sample = scan.get(scan_index).ok_or(Rav1dError::EINVAL)?;
                    let Ok(coefficient) = sample.get();
                    u32::from(coefficient)
                }
                TxClass::H => ordinal,
                TxClass::V => {
                    let column = ordinal & extent.width.wrapping_sub(1);
                    let row = ordinal.wrapping_shr(extent.width.trailing_zeros());
                    column.wrapping_mul(extent.height).wrapping_add(row)
                }
            };
            let Ok(index) = console_core_number_conversion::index(position);
            let sample = input.get_mut(index).ok_or(Rav1dError::EINVAL)?;
            let Ok(coefficient) = coefficient_value(cell, ordinal, state);
            *sample = coefficient;
        }

        Ok(input)
    }

    fn mixed_pixels(
        extent: console_core_geometry::Size<u32>,
        stride: u32,
        state: &mut u64,
    ) -> Result<Vec<u8>, Never> {
        let rows = extent.height.wrapping_add(2);
        let count = stride.wrapping_mul(rows).next_multiple_of(64);
        let mut pixels = Vec::new();

        for ordinal in 0..count {
            let color = match ordinal & 3 {
                0 => 0,
                1 => 128,
                2 => 255,
                _ => {
                    let Ok(random) = next(state);
                    let Ok(sample) = console_core_number_conversion::wrapped::<_, u8>(random);
                    sample
                }
            };
            pixels.push(color);
        }

        Ok(pixels)
    }

    enum FixtureKernel<'a> {
        Dispatch(&'a TransformDispatch),
        Scalar,
    }

    fn run_mixed_transform(
        tx: TxfmSize,
        kind: TxfmType,
        cell: &CheckCellArguments,
        kernel: FixtureKernel<'_>,
        pixels: &[u8],
        input: &[i16],
    ) -> Rav1dResult<(Vec<u8>, Vec<i16>)> {
        let Ok(mut source) = crate::src::safe_simd::aligned_plane(pixels);
        let mut coefficients = input.to_vec();
        let Ok(stride) = console_core_number_conversion::index(cell.stride);
        let Ok(offset) = console_core_number_conversion::index(cell.offset);
        let Ok(eob) = console_core_number_conversion::wrapped::<_, i32>(cell.eob);
        let picture = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut source, stride);
        let mut dst = ReconDst::Pic(picture.with_offset::<BitDepth8>() + offset.cast_signed());
        let depth = BitDepth8::new(());

        match kernel {
            FixtureKernel::Dispatch(expected) => {
                let Ok(observed) = itxfm_add_dispatch::<BitDepth8>(tx, kind, &mut dst, &mut coefficients, eob, depth);
                assert_eq!(observed, *expected, "dispatch liveness, type={kind}");
            }
            FixtureKernel::Scalar => {
                let Ok(shape) = tx.ordinal();
                crate::src::itx::itxfm_add_scalar_fallback::<BitDepth8>(
                    shape, kind, &mut dst, &mut coefficients, eob, depth,
                )?;
            }
        }

        let mut result = pixels.to_vec();
        picture.copy_pixels_to::<BitDepth8>(&mut result);
        Ok((result, coefficients))
    }

    fn assert_padding(
        extent: console_core_geometry::Size<u32>,
        cell: &CheckCellArguments,
        output: &[u8],
        input: &[u8],
    ) -> Result<(), Never> {
        for (ordinal, (&decoded, &original)) in (0u32..).zip(output.iter().zip(input)) {
            let position = ordinal.checked_sub(cell.offset);
            let inside = position.is_some_and(|relative| {
                relative.wrapping_div(cell.stride) < extent.height
                    && relative.wrapping_rem(cell.stride) < extent.width
            });

            match ::core::primitive::bool::then_some(inside, ()) {
                ::core::option::Option::Some(()) => {}
                ::core::option::Option::None => assert_eq!(decoded, original, "write outside block at {ordinal}"),
            }
        }

        Ok(())
    }

    fn check_cell(
        tx: TxfmSize,
        kind: TxfmType,
        cell: CheckCellArguments,
        expected_dispatch: &TransformDispatch,
    ) -> Rav1dResult {
        let Ok(extent) = tx.extent();
        let mut state = 0xdeca_fbad_f00d_u64 ^ u64::from(kind) ^ u64::from(cell.eob).wrapping_shl(8);
        let input = mixed_coefficients(tx, kind, extent, &cell, &mut state)?;
        let Ok(pixels) = mixed_pixels(extent, cell.stride, &mut state);
        let (decoded, coefficients) = run_mixed_transform(
            tx, kind, &cell, FixtureKernel::Dispatch(expected_dispatch), &pixels, &input,
        )?;

        match expected_dispatch {
            TransformDispatch::Fallback => {
                assert_eq!((decoded, coefficients), (pixels, input), "declined dispatch wrote data");
            }
            TransformDispatch::Applied => {
                let (expected_pixels, expected_coefficients) = run_mixed_transform(
                    tx, kind, &cell, FixtureKernel::Scalar, &pixels, &input,
                )?;
                assert_eq!(decoded, expected_pixels, "pixels: {tx:?}, type={kind}, eob={}", cell.eob);
                assert_eq!(coefficients, expected_coefficients, "coefficient clearing: type={kind}");

                let count = extent.width.wrapping_mul(extent.height);
                let Ok(active_count) = console_core_number_conversion::index(count);
                let (active, tail) = coefficients.split_at_checked(active_count).ok_or(Rav1dError::EINVAL)?;
                let sentinel = input.get(active_count..).ok_or(Rav1dError::EINVAL)?;
                assert!(active.iter().all(|&value| value == 0));
                assert_eq!(tail, sentinel);

                let Ok(()) = assert_padding(extent, &cell, &decoded, &pixels);
            }
        }

        Ok(())
    }

    fn sweep(tx: TxfmSize, eobs: &[u32], expected_cells: u32) -> Rav1dResult {
        let Ok(_lock) = crate::src::safe_simd::token_test_lock();
        let Ok(avx2) = crate::src::cpu::summon_avx2();

        match avx2 {
            None => return Ok(()),
            Some(_) => {}
        }

        let Ok(extent) = tx.extent();
        let mut cells = 0_u32;

        for kind in TYPES {
            for &eob in eobs {
                for pattern in PATTERNS {
                    for stride in [extent.width.wrapping_mul(2), extent.width.wrapping_mul(4)] {
                        for offset in [0, stride.wrapping_add(3)] {
                            let cell = CheckCellArguments { eob, pattern, stride, offset };
                            check_cell(tx, kind, cell, &TransformDispatch::Applied)?;
                            cells = cells.saturating_add(1);
                        }
                    }
                }
            }
        }

        assert_eq!(cells, expected_cells);
        Ok(())
    }

    fn check_permutation(tx: TxfmSize, dispatch: &TransformDispatch) -> Rav1dResult<u32> {
        let Ok(extent) = tx.extent();
        let last = extent.width.wrapping_mul(extent.height).wrapping_sub(1);
        let mut cells = 0_u32;

        for kind in TYPES {
            for eob in [0, 31, last] {
                for pattern in PERMUTATION_PATTERNS {
                    let cell = CheckCellArguments { eob, pattern, stride: 32, offset: 35 };
                    check_cell(tx, kind, cell, dispatch)?;
                    cells = cells.saturating_add(1);
                }
            }
        }

        Ok(cells)
    }

    fn token_sweep(tx: TxfmSize) -> Rav1dResult {
        use archmage::testing::{CompileTimePolicy, for_each_token_permutation};
        use core::cell::Cell;

        let Ok(_lock) = crate::src::safe_simd::token_test_lock();
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let live_cells = Cell::new(0_u32);
        let declined_cells = Cell::new(0_u32);
        let error = Cell::new(None);
        let report = for_each_token_permutation(CompileTimePolicy::WarnStderr, |_| {
            let Ok(token) = crate::src::cpu::summon_avx2();
            let (dispatch, count) = match token {
                Some(_) => (TransformDispatch::Applied, &live_cells),
                None => (TransformDispatch::Fallback, &declined_cells),
            };

            match check_permutation(tx, &dispatch) {
                Ok(cells) => count.set(count.get().saturating_add(cells)),
                Err(problem) => error.set(Some(problem)),
            }
        });
        assert!(report.permutations_run >= 2);
        assert!(avx2.is_none() || live_cells.get() > 0, "SIMD dispatch was never exercised");
        assert!(declined_cells.get() > 0, "disabled dispatch was never exercised");

        match error.get() {
            Some(problem) => Err(problem),
            None => Ok(()),
        }
    }

    #[test]
    fn test_mixed16_dispatch_matches_scalar() -> Rav1dResult {
        sweep(
            TxfmSize::S16x16,
            &[0, 1, 7, 8, 15, 16, 31, 32, 63, 64, 127, 128, 255],
            5824,
        )
    }

    #[test]
    fn test_mixed16_dispatch_cpu_permutations() -> Rav1dResult {
        token_sweep(TxfmSize::S16x16)
    }

    #[test]
    fn test_mixed4_dispatch_matches_scalar() -> Rav1dResult {
        sweep(TxfmSize::S4x4, &[0, 1, 3, 4, 7, 8, 15], 3136)
    }

    #[test]
    fn test_mixed4_rejects_short_views_before_writing() {
        let mut pixels = [128; 32];
        let mut coeff = [16; 15];
        assert_eq!(
            inv_txfm_add_4x4_8bpc(&mut pixels, 0, 8, &mut coeff, ScalarItx4Passes { rows: adst4_1d_scalar, columns: dct4_1d_scalar }),
            Err(Rav1dError::EINVAL),
        );
        assert_eq!(pixels, [128; 32]);
        assert_eq!(coeff, [16; 15]);

        let mut pixels_2 = [128; 27];
        let mut coeff_2 = [16; 16];
        assert_eq!(
            inv_txfm_add_4x4_8bpc(&mut pixels_2, 0, 8, &mut coeff_2, ScalarItx4Passes { rows: adst4_1d_scalar, columns: dct4_1d_scalar }),
            Err(Rav1dError::EINVAL),
        );
        assert_eq!(pixels_2, [128; 27]);
        assert_eq!(coeff_2, [16; 16]);
    }

    #[test]
    fn test_mixed8_dispatch_matches_scalar() -> Rav1dResult {
        sweep(
            TxfmSize::S8x8,
            &[0, 1, 3, 4, 7, 8, 15, 16, 31, 32, 63],
            4928,
        )
    }

    #[test]
    fn test_mixed8_dispatch_cpu_permutations() -> Rav1dResult {
        token_sweep(TxfmSize::S8x8)
    }

    #[test]
    fn low_depth_transforms_match_scalar_on_padded_rows() -> Rav1dResult {
        let Ok(_lock) = crate::src::safe_simd::token_test_lock();
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let token = match avx2 {
            Some(token) => token,
            None => return Ok(()),
        };
        let kernels: [(TxfmSize, TxfmType, CheckedKernel<u8, i16>); 20] = [
            (TxfmSize::R4x8, DCT_DCT, inv_txfm_add_dct_dct_4x8_8bpc_avx2_inner),
            (TxfmSize::R8x4, DCT_DCT, inv_txfm_add_dct_dct_8x4_8bpc_avx2_inner),
            (TxfmSize::S16x16, DCT_DCT, inv_txfm_add_dct_dct_16x16_8bpc_avx2_inner),
            (TxfmSize::R4x16, DCT_DCT, inv_txfm_add_dct_dct_4x16_8bpc_avx2_inner),
            (TxfmSize::R16x4, DCT_DCT, inv_txfm_add_dct_dct_16x4_8bpc_avx2_inner),
            (TxfmSize::R8x32, DCT_DCT, inv_txfm_add_dct_dct_8x32_8bpc_avx2_inner),
            (TxfmSize::R16x32, IDTX, inv_txfm_add_identity_identity_16x32_8bpc_avx2_inner),
            (TxfmSize::R32x16, IDTX, inv_txfm_add_identity_identity_32x16_8bpc_avx2_inner),
            (TxfmSize::R8x32, IDTX, inv_txfm_add_identity_identity_8x32_8bpc_avx2_inner),
            (TxfmSize::R32x8, IDTX, inv_txfm_add_identity_identity_32x8_8bpc_avx2_inner),
            (TxfmSize::R16x32, DCT_DCT, inv_txfm_add_dct_dct_16x32_8bpc_avx2_inner),
            (TxfmSize::R32x16, DCT_DCT, inv_txfm_add_dct_dct_32x16_8bpc_avx2_inner),
            (TxfmSize::R32x8, DCT_DCT, inv_txfm_add_dct_dct_32x8_8bpc_avx2_inner),
            (TxfmSize::R32x64, DCT_DCT, inv_txfm_add_dct_dct_32x64_8bpc_avx2_inner),
            (TxfmSize::R64x32, DCT_DCT, inv_txfm_add_dct_dct_64x32_8bpc_avx2_inner),
            (TxfmSize::R16x64, DCT_DCT, inv_txfm_add_dct_dct_16x64_8bpc_avx2_inner),
            (TxfmSize::R64x16, DCT_DCT, inv_txfm_add_dct_dct_64x16_8bpc_avx2_inner),
            (TxfmSize::S32x32, DCT_DCT, inv_txfm_add_dct_dct_32x32_8bpc_avx2_inner),
            (TxfmSize::S32x32, IDTX, inv_txfm_add_identity_identity_32x32_8bpc_avx2_inner),
            (TxfmSize::S64x64, DCT_DCT, inv_txfm_add_dct_dct_64x64_8bpc_avx2_inner),
        ];

        for (size, kind, kernel) in kernels {
            let Ok(shape) = size.ordinal();
            let Ok(extent) = size.extent();
            let active = extent.width.min(32).wrapping_mul(extent.height.min(32)).wrapping_sub(1);
            let Ok(eob) = console_core_number_conversion::wrapped::<_, i32>(active);
            let mut pixels = [0u8; 5120];

            for (ordinal, value) in (0u16..).zip(&mut pixels) {
                let Ok(pixel) = console_core_number_conversion::wrapped::<_, u8>(ordinal.wrapping_mul(97));
                *value = pixel;
            }

            let mut reference = pixels;
            let mut coefficients = [0i16; 1024];

            for (ordinal, value) in (0i16..).zip(&mut coefficients) {
                *value = ordinal.wrapping_mul(47).wrapping_rem(133).wrapping_sub(66);
            }

            let mut scalar_coefficients = coefficients;
            let incomplete_len = extent.height.wrapping_sub(1).wrapping_mul(80)
                .wrapping_add(extent.width).wrapping_sub(1);
            let Ok(end) = console_core_number_conversion::index(incomplete_len);
            let incomplete = pixels.get_mut(..end).ok_or(Rav1dError::EINVAL)?;
            assert_eq!(
                kernel(token, incomplete, 80, &mut coefficients, 255),
                Err(Rav1dError::EINVAL)
            );
            assert_eq!(pixels, reference);
            assert_eq!(coefficients, scalar_coefficients);

            let picture = Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(&mut reference, 80);
            let mut scalar = ReconDst::Pic(picture.with_offset::<BitDepth8>());
            crate::src::itx::itxfm_add_scalar_fallback::<BitDepth8>(
                shape, kind, &mut scalar, &mut scalar_coefficients, eob, BitDepth8::new(()),
            )?;
            kernel(token, &mut pixels, 80, &mut coefficients, 255)?;

            assert_eq!(pixels.as_slice(), &*picture.slice::<BitDepth8, _>(..), "{size:?} kind={kind}");
            assert_eq!(coefficients, scalar_coefficients);
        }

        Ok(())
    }

    #[cfg(feature = "bitdepth_16")]
    #[test]
    fn high_depth_transforms_match_scalar_on_padded_rows() -> Rav1dResult {
        use crate::include::common::bitdepth::BitDepth16;

        let Ok(_lock) = crate::src::safe_simd::token_test_lock();
        let Ok(avx2) = crate::src::cpu::summon_avx2();
        let token = match avx2 {
            Some(token) => token,
            None => return Ok(()),
        };
        let kernels: [(TxfmSize, TxfmType, CheckedKernel<u16, i32>); 31] = [
            (TxfmSize::S4x4, IDTX, inv_identity_add_4x4_16bpc_avx2),
            (TxfmSize::S8x8, IDTX, inv_identity_add_8x8_16bpc_avx2),
            (TxfmSize::S16x16, IDTX, inv_identity_add_16x16_16bpc_avx2),
            (TxfmSize::S16x16, DCT_DCT, inv_txfm_add_dct_dct_16x16_16bpc_avx2_inner),
            (TxfmSize::R4x8, IDTX, inv_txfm_add_identity_identity_4x8_16bpc_avx2_inner),
            (TxfmSize::R4x16, IDTX, inv_txfm_add_identity_identity_4x16_16bpc_avx2_inner),
            (TxfmSize::R4x8, DCT_DCT, inv_txfm_add_dct_dct_4x8_16bpc_avx2_inner),
            (TxfmSize::R4x16, DCT_DCT, inv_txfm_add_dct_dct_4x16_16bpc_avx2_inner),
            (TxfmSize::R8x4, DCT_DCT, inv_txfm_add_dct_dct_8x4_16bpc_avx2_inner),
            (TxfmSize::R8x16, DCT_DCT, inv_txfm_add_dct_dct_8x16_16bpc_avx2_inner),
            (TxfmSize::R16x8, DCT_DCT, inv_txfm_add_dct_dct_16x8_16bpc_avx2_inner),
            (TxfmSize::R16x4, DCT_DCT, inv_txfm_add_dct_dct_16x4_16bpc_avx2_inner),
            (TxfmSize::R16x32, DCT_DCT, inv_txfm_add_dct_dct_16x32_16bpc_avx2_inner),
            (TxfmSize::R8x32, DCT_DCT, inv_txfm_add_dct_dct_8x32_16bpc_avx2_inner),
            (TxfmSize::R32x16, DCT_DCT, inv_txfm_add_dct_dct_32x16_16bpc_avx2_inner),
            (TxfmSize::R32x8, DCT_DCT, inv_txfm_add_dct_dct_32x8_16bpc_avx2_inner),
            (TxfmSize::R8x4, IDTX, inv_txfm_add_identity_identity_8x4_16bpc_avx2_inner),
            (TxfmSize::R8x16, IDTX, inv_txfm_add_identity_identity_8x16_16bpc_avx2_inner),
            (TxfmSize::R16x8, IDTX, inv_txfm_add_identity_identity_16x8_16bpc_avx2_inner),
            (TxfmSize::R16x4, IDTX, inv_txfm_add_identity_identity_16x4_16bpc_avx2_inner),
            (TxfmSize::R16x32, IDTX, inv_txfm_add_identity_identity_16x32_16bpc_avx2_inner),
            (TxfmSize::R32x16, IDTX, inv_txfm_add_identity_identity_32x16_16bpc_avx2_inner),
            (TxfmSize::R8x32, IDTX, inv_txfm_add_identity_identity_8x32_16bpc_avx2_inner),
            (TxfmSize::R32x8, IDTX, inv_txfm_add_identity_identity_32x8_16bpc_avx2_inner),
            (TxfmSize::R32x64, DCT_DCT, inv_txfm_add_dct_dct_32x64_16bpc_avx2_inner),
            (TxfmSize::R64x32, DCT_DCT, inv_txfm_add_dct_dct_64x32_16bpc_avx2_inner),
            (TxfmSize::R16x64, DCT_DCT, inv_txfm_add_dct_dct_16x64_16bpc_avx2_inner),
            (TxfmSize::R64x16, DCT_DCT, inv_txfm_add_dct_dct_64x16_16bpc_avx2_inner),
            (TxfmSize::S32x32, DCT_DCT, inv_txfm_add_dct_dct_32x32_16bpc_avx2_inner),
            (TxfmSize::S32x32, IDTX, inv_txfm_add_identity_identity_32x32_16bpc_avx2_inner),
            (TxfmSize::S64x64, DCT_DCT, inv_txfm_add_dct_dct_64x64_16bpc_avx2_inner),
        ];

        for (size, kind, kernel) in kernels {
            let Ok(shape) = size.ordinal();
            let Ok(extent) = size.extent();
            let last = extent.width.min(32).wrapping_mul(extent.height.min(32)).wrapping_sub(1);
            let Ok(eob) = console_core_number_conversion::wrapped::<_, i32>(last);

            for maximum in [1023, 4095] {
                let mut pixels = [0u16; 5120];

                for (ordinal, value) in (0u32..).zip(pixels.iter_mut()) {
                    let sample = ordinal.wrapping_mul(97).wrapping_rem(1024);
                    let Ok(color) = console_core_number_conversion::wrapped::<_, u16>(sample);
                    *value = color;
                }

                let mut original = pixels;
                let mut coefficients = [0i32; 1024];

                for (ordinal, value) in (0i32..).zip(coefficients.iter_mut()) {
                    *value = ordinal.wrapping_mul(47).wrapping_rem(133).wrapping_sub(66);
                }

                let mut scalar_coefficients = coefficients;
                let picture = Rav1dPictureDataComponent::wrap_buf::<BitDepth16>(&mut original, 80);
                let mut scalar = ReconDst::Pic(picture.with_offset::<BitDepth16>());
                let depth = BitDepth16::from_c(maximum);

                crate::src::itx::itxfm_add_scalar_fallback::<BitDepth16>(
                    shape, kind, &mut scalar, &mut scalar_coefficients, eob, depth,
                )?;
                kernel(token, &mut pixels, 160, &mut coefficients, maximum)?;

                assert_eq!(pixels.as_slice(), &*picture.slice::<BitDepth16, _>(..));
                assert_eq!(coefficients, scalar_coefficients);
            }
        }

        Ok(())
    }

    #[test]
    fn test_mixed8_api_rejects_invalid_views_before_writing() {
        let Ok(_lock) = crate::src::safe_simd::token_test_lock();
        let Ok(avx2) = crate::src::cpu::summon_avx2();

        let token = match avx2 {
            Some(matched_token) => matched_token,
            ::core::option::Option::None => return,
        };


        for (dst_len, stride, coeff_len) in [
            (0, 8, 64),
            (63, 8, 64),
            (64, 8, 63),
            (64, 8, 0),
            (126, 17, 64),
            (128, u64::MAX, 64),
            (128, u64::MAX.wrapping_div(7), 64),
        ] {
            let mut pixels = [128; 128];
            let mut coeff = [64; 64];
            let result = inv_txfm_add_mixed_8x8_8bpc(
                token,
                &mut pixels[..dst_len],
                stride,
                &mut coeff[..coeff_len],
                Mixed8Passes { rows: Mixed8Pass::Adst, columns: Mixed8Pass::Adst },
            );
            assert_eq!(
                result,
                Err(Rav1dError::EINVAL),
                "invalid view accepted: {dst_len}, {stride}, {coeff_len}"
            );
            assert_eq!(pixels, [128; 128], "rejected call changed pixels");
            assert_eq!(coeff, [64; 64], "rejected call changed coefficients");
        }

        let mut pixels = [128; 64];
        let mut coeff = [0; 64];
        *{ let [_explicit_part, ..] = &mut coeff; _explicit_part } = 1024;
        assert_eq!(
            inv_txfm_add_mixed_8x8_8bpc(token, &mut pixels, 8, &mut coeff, Mixed8Passes { rows: Mixed8Pass::Adst, columns: Mixed8Pass::Adst }),
            Ok(()),
        );
        assert!(pixels.iter().any(|&p| p != 128));
        assert_eq!(coeff, [0; 64]);
    }
}
