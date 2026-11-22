
#[cfg(target_arch = "x86_64")]
mod packed6 {
    use super::*;

    #[cfg(test)]
    std::thread_local! {
        static CALLS: std::cell::Cell<[u64; 2]> = const { std::cell::Cell::new([0; 2]) };
    }

    #[cfg(test)]
    pub(super) fn calls() -> core::result::Result<[u64; 2], console_core_never::Never> { Ok({
        CALLS.get()
    }) }

    #[rite]
    fn compute(_token: Desktop64, taps: [__m128i; 6], levels: [u8; 3]) -> Result<[__m128i; 4], console_core_never::Never> {
        let [p2_v, p1_v, p0_v, q0_v, q1_v, q2_v] = taps;
        let [e, i, h] = levels.map(i16::from);
        let i_v = _mm_set1_epi16(i);
        let e_v = _mm_set1_epi16(e);
        let h_v = _mm_set1_epi16(h);
        let f_v = _mm_set1_epi16(1);

        let abs = |a: __m128i, b: __m128i| _mm_abs_epi16(_mm_sub_epi16(a, b));

        let abs_p1p0 = abs(p1_v, p0_v);
        let abs_q1q0 = abs(q1_v, q0_v);
        let abs_p0q0 = abs(p0_v, q0_v);
        let abs_p1q1 = abs(p1_v, q1_v);
        let abs_p2p1 = abs(p2_v, p1_v);
        let abs_q2q1 = abs(q2_v, q1_v);

        let not_gt = |a: __m128i, b: __m128i| -> __m128i {
            _mm_andnot_si128(_mm_cmpgt_epi16(a, b), _mm_set1_epi16(-1))
        };

        let m_p1p0 = not_gt(abs_p1p0, i_v);
        let m_q1q0 = not_gt(abs_q1q0, i_v);
        let val_ee = _mm_add_epi16(_mm_slli_epi16::<1>(abs_p0q0), _mm_srli_epi16::<1>(abs_p1q1));
        let m_val = not_gt(val_ee, e_v);
        let m_p2p1 = not_gt(abs_p2p1, i_v);
        let m_q2q1 = not_gt(abs_q2q1, i_v);
        let fm_mask = _mm_and_si128(
            _mm_and_si128(_mm_and_si128(m_p1p0, m_q1q0), m_val),
            _mm_and_si128(m_p2p1, m_q2q1),
        );

        let abs_p2p0 = abs(p2_v, p0_v);
        let abs_q2q0 = abs(q2_v, q0_v);
        let flat_mask = _mm_and_si128(
            _mm_and_si128(not_gt(abs_p2p0, f_v), not_gt(abs_p1p0, f_v)),
            _mm_and_si128(not_gt(abs_q1q0, f_v), not_gt(abs_q2q0, f_v)),
        );

        let p2_3 = _mm_add_epi16(_mm_slli_epi16::<1>(p2_v), p2_v);
        let c4 = _mm_set1_epi16(4);
        let dbl = |v: __m128i| _mm_slli_epi16::<1>(v);

        let out_m2 = _mm_srai_epi16::<3>(_mm_add_epi16(
            _mm_add_epi16(
                _mm_add_epi16(p2_3, dbl(p1_v)),
                _mm_add_epi16(dbl(p0_v), q0_v),
            ),
            c4,
        ));
        let out_m1 = _mm_srai_epi16::<3>(_mm_add_epi16(
            _mm_add_epi16(
                _mm_add_epi16(p2_v, dbl(p1_v)),
                _mm_add_epi16(_mm_add_epi16(dbl(p0_v), dbl(q0_v)), q1_v),
            ),
            c4,
        ));
        let out_0 = _mm_srai_epi16::<3>(_mm_add_epi16(
            _mm_add_epi16(
                _mm_add_epi16(p1_v, dbl(p0_v)),
                _mm_add_epi16(_mm_add_epi16(dbl(q0_v), dbl(q1_v)), q2_v),
            ),
            c4,
        ));
        let q2_3 = _mm_add_epi16(_mm_slli_epi16::<1>(q2_v), q2_v);
        let out_1 = _mm_srai_epi16::<3>(_mm_add_epi16(
            _mm_add_epi16(
                _mm_add_epi16(p0_v, dbl(q0_v)),
                _mm_add_epi16(dbl(q1_v), q2_3),
            ),
            c4,
        ));

        let neg128 = _mm_set1_epi16(-128);
        let pos127 = _mm_set1_epi16(127);
        let iclip = |v: __m128i| _mm_min_epi16(_mm_max_epi16(v, neg128), pos127);

        let diff_q0p0 = _mm_sub_epi16(q0_v, p0_v);
        let three_d = _mm_add_epi16(_mm_slli_epi16::<1>(diff_q0p0), diff_q0p0);
        let diff_p1q1 = _mm_sub_epi16(p1_v, q1_v);

        let hev_mask = _mm_or_si128(
            _mm_cmpgt_epi16(abs_p1p0, h_v),
            _mm_cmpgt_epi16(abs_q1q0, h_v),
        );

        let f_hev = iclip(_mm_add_epi16(three_d, iclip(diff_p1q1)));
        let f_no = iclip(three_d);

        let c4i = _mm_set1_epi16(4);
        let c3i = _mm_set1_epi16(3);
        let one = _mm_set1_epi16(1);

        let f1_hev = _mm_srai_epi16::<3>(_mm_min_epi16(_mm_add_epi16(f_hev, c4i), pos127));
        let f2_hev = _mm_srai_epi16::<3>(_mm_min_epi16(_mm_add_epi16(f_hev, c3i), pos127));
        let f1_no = _mm_srai_epi16::<3>(_mm_min_epi16(_mm_add_epi16(f_no, c4i), pos127));
        let f2_no = _mm_srai_epi16::<3>(_mm_min_epi16(_mm_add_epi16(f_no, c3i), pos127));
        let f_extra = _mm_srai_epi16::<1>(_mm_add_epi16(f1_no, one));

        let p0_hev = _mm_add_epi16(p0_v, f2_hev);
        let q0_hev = _mm_sub_epi16(q0_v, f1_hev);
        let p0_no = _mm_add_epi16(p0_v, f2_no);
        let q0_no = _mm_sub_epi16(q0_v, f1_no);
        let p1_no = _mm_add_epi16(p1_v, f_extra);
        let q1_no = _mm_sub_epi16(q1_v, f_extra);

        let blendv = |a: __m128i, b: __m128i, mask: __m128i| -> __m128i {
            _mm_or_si128(_mm_andnot_si128(mask, a), _mm_and_si128(mask, b))
        };

        let narrow_p1 = blendv(p1_no, p1_v, hev_mask);
        let narrow_p0 = blendv(p0_no, p0_hev, hev_mask);
        let narrow_q0 = blendv(q0_no, q0_hev, hev_mask);
        let narrow_q1 = blendv(q1_no, q1_v, hev_mask);

        let out_m2_sel = blendv(narrow_p1, out_m2, flat_mask);
        let out_m1_sel = blendv(narrow_p0, out_m1, flat_mask);
        let out_0_sel = blendv(narrow_q0, out_0, flat_mask);
        let out_1_sel = blendv(narrow_q1, out_1, flat_mask);

        let final_p1 = blendv(p1_v, out_m2_sel, fm_mask);
        let final_p0 = blendv(p0_v, out_m1_sel, fm_mask);
        let final_q0 = blendv(q0_v, out_0_sel, fm_mask);
        let final_q1 = blendv(q1_v, out_1_sel, fm_mask);
        Ok([final_p1, final_p0, final_q0, final_q1])
    }




    #[arcane]
    pub(super) fn apply<const H: bool>(
        token: Desktop64,
        pixels: &mut [u8],
        window: PackedWindow,
        levels: [u8; 3],
    ) -> Rav1dResult {
        #[cfg(test)]
        CALLS.with(|counter| {
            let [vertical, horizontal] = counter.get();
            counter.set(match H.then_some(()) {
                Some(()) => [vertical, horizontal.wrapping_add(1)],
                None => [vertical.wrapping_add(1), horizontal],
            });
        });
        let minimum_stride = match H.then_some(()) {
            Some(()) => 6,
            None => 8,
        };

        match (window.stride.unsigned_abs() >= minimum_stride).then_some(()) {
            Some(()) => {}
            None => return Err(Rav1dError::EINVAL),
        }

        match H.then_some(()) {
            Some(()) => apply_horizontal(token, pixels, window, levels),
            None => apply_vertical(token, pixels, window, levels),
        }
    }

    #[rite]
    fn apply_horizontal(token: Desktop64, pixels: &mut [u8], window: PackedWindow, levels: [u8; 3]) -> Rav1dResult {
        let zero = _mm_setzero_si128();
        let mut rows = [zero; 8];

        for (vector, row) in rows.iter_mut().zip(0i64..) {
            let start = window.position(PackedPosition { rows: row, columns: -3 })?;
            let &[a, b, c, d, e, f] = packed_pixels(pixels, start)?;
            let bytes = [a, b, c, d, e, f, 0, 0];
            *vector = _mm_cvtepu8_epi16(_mm_cvtsi64_si128(i64::from_le_bytes(bytes)));
        }

        let Ok([a, b, c, d, e, f, _, _]) = transpose_packed_rows(token, rows);
        let Ok([a_2, b_2, c_2, d_2]) = compute(token, [a, b, c, d, e, f], levels);
        let Ok(filtered) = transpose_packed_rows(token, [a_2, b_2, c_2, d_2, zero, zero, zero, zero]);

        for (vector, row) in filtered.into_iter().zip(0i64..) {
            let start = window.position(PackedPosition { rows: row, columns: -2 })?;
            let output = packed_pixels_mut(pixels, start)?;
            *output = _mm_cvtsi128_si32(_mm_packus_epi16(vector, zero)).to_le_bytes();
        }

        Ok(())
    }

    #[rite]
    fn apply_vertical(token: Desktop64, pixels: &mut [u8], window: PackedWindow, levels: [u8; 3]) -> Rav1dResult {
        let zero = _mm_setzero_si128();
        let mut taps = [zero; 6];

        for (vector, row) in taps.iter_mut().zip(-3i64..) {
            let start = window.position(PackedPosition { rows: row, columns: 0 })?;
            let bytes = packed_pixels(pixels, start)?;
            *vector = _mm_cvtepu8_epi16(_mm_cvtsi64_si128(i64::from_le_bytes(*bytes)));
        }

        let Ok(filtered) = compute(token, taps, levels);

        for (vector, row) in filtered.into_iter().zip(-2i64..) {
            let start = window.position(PackedPosition { rows: row, columns: 0 })?;
            let output = packed_pixels_mut(pixels, start)?;
            *output = _mm_cvtsi128_si64(_mm_packus_epi16(vector, zero)).to_le_bytes();
        }

        Ok(())
    }
}
