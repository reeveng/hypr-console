#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
struct VectorBounds<V> {
    lower: V,
    upper: V,
}

#[cfg(target_arch = "x86_64")]
struct RoundedProducts<V> {
    left: (V, i32),
    right: (V, i32),
    bias: i32,
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn mac_msub_shr<const SHIFT: i32>(
    _token: Desktop64,
    terms: RoundedProducts<__m256i>,
) -> Result<__m256i, Never> {
    let RoundedProducts { left: (a, c1), right: (b, c2), bias: rnd } = terms;
    let p1 = _mm256_mullo_epi32(a, _mm256_set1_epi32(c1));
    let p2 = _mm256_mullo_epi32(b, _mm256_set1_epi32(c2));
    let sum = _mm256_add_epi32(_mm256_sub_epi32(p1, p2), _mm256_set1_epi32(rnd));
    Ok(_mm256_srai_epi32::<SHIFT>(sum))
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn mac_madd_shr<const SHIFT: i32>(
    _token: Desktop64,
    terms: RoundedProducts<__m256i>,
) -> Result<__m256i, Never> {
    let RoundedProducts { left: (a, c1), right: (b, c2), bias: rnd } = terms;
    let p1 = _mm256_mullo_epi32(a, _mm256_set1_epi32(c1));
    let p2 = _mm256_mullo_epi32(b, _mm256_set1_epi32(c2));
    let sum = _mm256_add_epi32(_mm256_add_epi32(p1, p2), _mm256_set1_epi32(rnd));
    Ok(_mm256_srai_epi32::<SHIFT>(sum))
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn clip8(_token: Desktop64, v: __m256i, bounds: VectorBounds<__m256i>) -> Result<__m256i, Never> {
    let VectorBounds { lower: min_v, upper: max_v } = bounds;
    Ok(_mm256_min_epi32(_mm256_max_epi32(v, min_v), max_v))
}






#[cfg(target_arch = "x86_64")]
#[rite]
fn mac_msub_shr_v4<const SHIFT: u32>(
    _token: Server64,
    terms: RoundedProducts<__m512i>,
) -> Result<__m512i, Never> {
    let RoundedProducts { left: (a, c1), right: (b, c2), bias: rnd } = terms;
    let p1 = _mm512_mullo_epi32(a, _mm512_set1_epi32(c1));
    let p2 = _mm512_mullo_epi32(b, _mm512_set1_epi32(c2));
    let sum = _mm512_add_epi32(_mm512_sub_epi32(p1, p2), _mm512_set1_epi32(rnd));
    Ok(_mm512_srai_epi32::<SHIFT>(sum))
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn mac_madd_shr_v4<const SHIFT: u32>(
    _token: Server64,
    terms: RoundedProducts<__m512i>,
) -> Result<__m512i, Never> {
    let RoundedProducts { left: (a, c1), right: (b, c2), bias: rnd } = terms;
    let p1 = _mm512_mullo_epi32(a, _mm512_set1_epi32(c1));
    let p2 = _mm512_mullo_epi32(b, _mm512_set1_epi32(c2));
    let sum = _mm512_add_epi32(_mm512_add_epi32(p1, p2), _mm512_set1_epi32(rnd));
    Ok(_mm512_srai_epi32::<SHIFT>(sum))
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn clip16_i32(_token: Server64, v: __m512i, bounds: VectorBounds<__m512i>) -> Result<__m512i, Never> {
    let VectorBounds { lower: min_v, upper: max_v } = bounds;
    Ok(_mm512_min_epi32(_mm512_max_epi32(v, min_v), max_v))
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn dct4_1d_cols16(token: Server64, c: &mut [__m512i; 4], bounds: VectorBounds<__m512i>) -> Result<(), Never> {
    let &mut [in0, ..] = c;
    let &mut [_, in1, ..] = c;
    let &mut [_, _, in2, ..] = c;
    let &mut [_, _, _, in3, ..] = c;
    let sum02 = _mm512_add_epi32(in0, in2);
    let sub02 = _mm512_sub_epi32(in0, in2);
    let t0 = _mm512_srai_epi32::<8>(_mm512_add_epi32(
        _mm512_mullo_epi32(sum02, _mm512_set1_epi32(181)),
        _mm512_set1_epi32(128),
    ));
    let t1 = _mm512_srai_epi32::<8>(_mm512_add_epi32(
        _mm512_mullo_epi32(sub02, _mm512_set1_epi32(181)),
        _mm512_set1_epi32(128),
    ));
    let Ok(t2_shifted) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (in1, 1567), right: (in3, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 });
    let t2 = _mm512_sub_epi32(t2_shifted, in3);
    let Ok(t3_shifted) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (in1, ::core::primitive::i32::wrapping_sub(3784, 4096)), right: (in3, 1567), bias: 2048 });
    let t3 = _mm512_add_epi32(t3_shifted, in1);

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t0, t3), bounds); clipped };
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t1, t2), bounds); clipped };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t1, t2), bounds); clipped };
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t0, t3), bounds); clipped };
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn dct8_1d_cols16(token: Server64, c: &mut [__m512i; 8], bounds: VectorBounds<__m512i>) -> Result<(), Never> {
    let mut even = [match c { &mut [_explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }];
    let Ok(()) = dct4_1d_cols16(token, &mut even, bounds);
    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = match even { [_explicit_element, ..] => _explicit_element };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _explicit_element, ..] => _explicit_element };

    let &mut [_, in1, ..] = c;
    let &mut [_, _, _, in3, ..] = c;
    let &mut [_, _, _, _, _, in5, ..] = c;
    let &mut [_, _, _, _, _, _, _, in7, ..] = c;

    let t4a = _mm512_sub_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (in1, 799), right: (in7, ::core::primitive::i32::wrapping_sub(4017, 4096)), bias: 2048 }); combined },
        in7,
    );
    let Ok(t5a) = mac_msub_shr_v4::<11>(token, RoundedProducts { left: (in5, 1703), right: (in3, 1138), bias: 1024 });
    let Ok(t6a) = mac_madd_shr_v4::<11>(token, RoundedProducts { left: (in5, 1138), right: (in3, 1703), bias: 1024 });
    let t7a = _mm512_add_epi32(
        { let Ok(combined) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (in1, ::core::primitive::i32::wrapping_sub(4017, 4096)), right: (in7, 799), bias: 2048 }); combined },
        in1,
    );

    let Ok(t4) = clip16_i32(token, _mm512_add_epi32(t4a, t5a), bounds);
    let Ok(t5a_n) = clip16_i32(token, _mm512_sub_epi32(t4a, t5a), bounds);
    let Ok(t7) = clip16_i32(token, _mm512_add_epi32(t7a, t6a), bounds);
    let Ok(t6a_n) = clip16_i32(token, _mm512_sub_epi32(t7a, t6a), bounds);

    let d = _mm512_sub_epi32(t6a_n, t5a_n);
    let t5 = _mm512_srai_epi32::<8>(_mm512_add_epi32(
        _mm512_mullo_epi32(d, _mm512_set1_epi32(181)),
        _mm512_set1_epi32(128),
    ));
    let s = _mm512_add_epi32(t6a_n, t5a_n);
    let t6 = _mm512_srai_epi32::<8>(_mm512_add_epi32(
        _mm512_mullo_epi32(s, _mm512_set1_epi32(181)),
        _mm512_set1_epi32(128),
    ));

    let &mut [t0, ..] = c;
    let &mut [_, _, t1, ..] = c;
    let &mut [_, _, _, _, t2, ..] = c;
    let &mut [_, _, _, _, _, _, t3, ..] = c;

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t0, t7), bounds); clipped };
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t1, t6), bounds); clipped };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t2, t5), bounds); clipped };
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t3, t4), bounds); clipped };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t3, t4), bounds); clipped };
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t2, t5), bounds); clipped };
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t1, t6), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t0, t7), bounds); clipped };
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn dct16_1d_cols16(token: Server64, c: &mut [__m512i; 16], bounds: VectorBounds<__m512i>) -> Result<(), Never> {
    let mut even = [match c { &mut [_explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }];
    let Ok(()) = dct8_1d_cols16(token, &mut even, bounds);
    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = match even { [_explicit_element, ..] => _explicit_element };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element };

    let &mut [_, in1, ..] = c;
    let &mut [_, _, _, in3, ..] = c;
    let &mut [_, _, _, _, _, in5, ..] = c;
    let &mut [_, _, _, _, _, _, _, in7, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, in9, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, in11, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, in13, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, in15, ..] = c;

    let t8a = _mm512_sub_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (in1, 401), right: (in15, ::core::primitive::i32::wrapping_sub(4076, 4096)), bias: 2048 }); combined },
        in15,
    );
    let Ok(t9a) = mac_msub_shr_v4::<11>(token, RoundedProducts { left: (in9, 1583), right: (in7, 1299), bias: 1024 });
    let t10a = _mm512_sub_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (in5, 1931), right: (in11, ::core::primitive::i32::wrapping_sub(3612, 4096)), bias: 2048 }); combined },
        in11,
    );
    let t11a = _mm512_add_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (in13, ::core::primitive::i32::wrapping_sub(3920, 4096)), right: (in3, 1189), bias: 2048 }); combined },
        in13,
    );
    let t12a = _mm512_add_epi32(
        { let Ok(combined) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (in13, 1189), right: (in3, ::core::primitive::i32::wrapping_sub(3920, 4096)), bias: 2048 }); combined },
        in3,
    );
    let t13a = _mm512_add_epi32(
        { let Ok(combined) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (in5, ::core::primitive::i32::wrapping_sub(3612, 4096)), right: (in11, 1931), bias: 2048 }); combined },
        in5,
    );
    let Ok(t14a) = mac_madd_shr_v4::<11>(token, RoundedProducts { left: (in9, 1299), right: (in7, 1583), bias: 1024 });
    let t15a = _mm512_add_epi32(
        { let Ok(combined) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (in1, ::core::primitive::i32::wrapping_sub(4076, 4096)), right: (in15, 401), bias: 2048 }); combined },
        in1,
    );

    let Ok(t8) = clip16_i32(token, _mm512_add_epi32(t8a, t9a), bounds);
    let Ok(mut t9) = clip16_i32(token, _mm512_sub_epi32(t8a, t9a), bounds);
    let Ok(mut t10) = clip16_i32(token, _mm512_sub_epi32(t11a, t10a), bounds);
    let Ok(mut t11) = clip16_i32(token, _mm512_add_epi32(t11a, t10a), bounds);
    let Ok(mut t12) = clip16_i32(token, _mm512_add_epi32(t12a, t13a), bounds);
    let Ok(mut t13) = clip16_i32(token, _mm512_sub_epi32(t12a, t13a), bounds);
    let Ok(mut t14) = clip16_i32(token, _mm512_sub_epi32(t15a, t14a), bounds);
    let Ok(t15) = clip16_i32(token, _mm512_add_epi32(t15a, t14a), bounds);

    let t9a_2 = _mm512_sub_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (t14, 1567), right: (t9, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t9,
    );
    let t14a_2 = _mm512_add_epi32(
        { let Ok(combined) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (t14, ::core::primitive::i32::wrapping_sub(3784, 4096)), right: (t9, 1567), bias: 2048 }); combined },
        t14,
    );
    let t10a_inner = _mm512_add_epi32(
        _mm512_mullo_epi32(t13, _mm512_set1_epi32(::core::primitive::i32::wrapping_sub(3784, 4096))),
        _mm512_mullo_epi32(t10, _mm512_set1_epi32(1567)),
    );
    let t10a_2 = _mm512_sub_epi32(
        _mm512_srai_epi32::<12>(_mm512_add_epi32(
            _mm512_sub_epi32(_mm512_setzero_si512(), t10a_inner),
            _mm512_set1_epi32(2048),
        )),
        t13,
    );
    let t13a_2 = _mm512_sub_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (t13, 1567), right: (t10, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t10,
    );

    let Ok(t8a_2) = clip16_i32(token, _mm512_add_epi32(t8, t11), bounds);
    t9 = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t9a_2, t10a_2), bounds); clipped };
    t10 = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t9a_2, t10a_2), bounds); clipped };
    let Ok(t11a_2) = clip16_i32(token, _mm512_sub_epi32(t8, t11), bounds);
    let Ok(t12a_2) = clip16_i32(token, _mm512_sub_epi32(t15, t12), bounds);
    t13 = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t14a_2, t13a_2), bounds); clipped };
    t14 = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t14a_2, t13a_2), bounds); clipped };
    let Ok(t15a_2) = clip16_i32(token, _mm512_add_epi32(t15, t12), bounds);

    let d_13_10 = _mm512_sub_epi32(t13, t10);
    let t10a_new = _mm512_srai_epi32::<8>(_mm512_add_epi32(
        _mm512_mullo_epi32(d_13_10, _mm512_set1_epi32(181)),
        _mm512_set1_epi32(128),
    ));
    let s_13_10 = _mm512_add_epi32(t13, t10);
    let t13a_new = _mm512_srai_epi32::<8>(_mm512_add_epi32(
        _mm512_mullo_epi32(s_13_10, _mm512_set1_epi32(181)),
        _mm512_set1_epi32(128),
    ));
    let d_12a_11a = _mm512_sub_epi32(t12a_2, t11a_2);
    t11 = _mm512_srai_epi32::<8>(_mm512_add_epi32(
        _mm512_mullo_epi32(d_12a_11a, _mm512_set1_epi32(181)),
        _mm512_set1_epi32(128),
    ));
    let s_12a_11a = _mm512_add_epi32(t12a_2, t11a_2);
    t12 = _mm512_srai_epi32::<8>(_mm512_add_epi32(
        _mm512_mullo_epi32(s_12a_11a, _mm512_set1_epi32(181)),
        _mm512_set1_epi32(128),
    ));

    let &mut [t0, ..] = c;
    let &mut [_, _, t1, ..] = c;
    let &mut [_, _, _, _, t2, ..] = c;
    let &mut [_, _, _, _, _, _, t3, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, t4, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, t5, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, _, t6, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _, t7, ..] = c;

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t0, t15a_2), bounds); clipped };
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t1, t14), bounds); clipped };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t2, t13a_new), bounds); clipped };
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t3, t12), bounds); clipped };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t4, t11), bounds); clipped };
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t5, t10a_new), bounds); clipped };
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t6, t9), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t7, t8a_2), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t7, t8a_2), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t6, t9), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t5, t10a_new), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t4, t11), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t3, t12), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t2, t13a_new), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t1, t14), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t0, t15a_2), bounds); clipped };
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn dct32_1d_cols16(token: Server64, c: &mut [__m512i; 32], bounds: VectorBounds<__m512i>) -> Result<(), Never> {

    let [
        even0, in1, even2, in3, even4, in5, even6, in7,
        even8, in9, even10, in11, even12, in13, even14, in15,
        even16, in17, even18, in19, even20, in21, even22, in23,
        even24, in25, even26, in27, even28, in29, even30, in31,
    ] = *c;
    let mut even = [
        even0, even2, even4, even6, even8, even10, even12, even14,
        even16, even18, even20, even22, even24, even26, even28, even30,
    ];
    let Ok(()) = dct16_1d_cols16(token, &mut even, bounds);

    let t16a = _mm512_sub_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (in1, 201), right: (in31, ::core::primitive::i32::wrapping_sub(4091, 4096)), bias: 2048 }); combined },
        in31,
    );
    let t17a = _mm512_add_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (in17, ::core::primitive::i32::wrapping_sub(3035, 4096)), right: (in15, 2751), bias: 2048 }); combined },
        in17,
    );
    let t18a = _mm512_sub_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (in9, 1751), right: (in23, ::core::primitive::i32::wrapping_sub(3703, 4096)), bias: 2048 }); combined },
        in23,
    );
    let t19a = _mm512_add_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (in25, ::core::primitive::i32::wrapping_sub(3857, 4096)), right: (in7, 1380), bias: 2048 }); combined },
        in25,
    );
    let t20a = _mm512_sub_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (in5, 995), right: (in27, ::core::primitive::i32::wrapping_sub(3973, 4096)), bias: 2048 }); combined },
        in27,
    );
    let t21a = _mm512_add_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (in21, ::core::primitive::i32::wrapping_sub(3513, 4096)), right: (in11, 2106), bias: 2048 }); combined },
        in21,
    );
    let Ok(t22a) = mac_msub_shr_v4::<11>(token, RoundedProducts { left: (in13, 1220), right: (in19, 1645), bias: 1024 });
    let t23a = _mm512_add_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (in29, ::core::primitive::i32::wrapping_sub(4052, 4096)), right: (in3, 601), bias: 2048 }); combined },
        in29,
    );
    let t24a = _mm512_add_epi32(
        { let Ok(combined) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (in29, 601), right: (in3, ::core::primitive::i32::wrapping_sub(4052, 4096)), bias: 2048 }); combined },
        in3,
    );
    let Ok(t25a) = mac_madd_shr_v4::<11>(token, RoundedProducts { left: (in13, 1645), right: (in19, 1220), bias: 1024 });
    let t26a = _mm512_add_epi32(
        { let Ok(combined) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (in21, 2106), right: (in11, ::core::primitive::i32::wrapping_sub(3513, 4096)), bias: 2048 }); combined },
        in11,
    );
    let t27a = _mm512_add_epi32(
        { let Ok(combined) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (in5, ::core::primitive::i32::wrapping_sub(3973, 4096)), right: (in27, 995), bias: 2048 }); combined },
        in5,
    );
    let t28a = _mm512_add_epi32(
        { let Ok(combined) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (in25, 1380), right: (in7, ::core::primitive::i32::wrapping_sub(3857, 4096)), bias: 2048 }); combined },
        in7,
    );
    let t29a = _mm512_add_epi32(
        { let Ok(combined) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (in9, ::core::primitive::i32::wrapping_sub(3703, 4096)), right: (in23, 1751), bias: 2048 }); combined },
        in9,
    );
    let t30a = _mm512_add_epi32(
        { let Ok(combined) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (in17, 2751), right: (in15, ::core::primitive::i32::wrapping_sub(3035, 4096)), bias: 2048 }); combined },
        in15,
    );
    let t31a = _mm512_add_epi32(
        { let Ok(combined) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (in1, ::core::primitive::i32::wrapping_sub(4091, 4096)), right: (in31, 201), bias: 2048 }); combined },
        in1,
    );

    let Ok(mut t16) = clip16_i32(token, _mm512_add_epi32(t16a, t17a), bounds);
    let Ok(mut t17) = clip16_i32(token, _mm512_sub_epi32(t16a, t17a), bounds);
    let Ok(mut t18) = clip16_i32(token, _mm512_sub_epi32(t19a, t18a), bounds);
    let Ok(t19) = clip16_i32(token, _mm512_add_epi32(t19a, t18a), bounds);
    let Ok(t20) = clip16_i32(token, _mm512_add_epi32(t20a, t21a), bounds);
    let Ok(mut t21) = clip16_i32(token, _mm512_sub_epi32(t20a, t21a), bounds);
    let Ok(mut t22) = clip16_i32(token, _mm512_sub_epi32(t23a, t22a), bounds);
    let Ok(mut t23) = clip16_i32(token, _mm512_add_epi32(t23a, t22a), bounds);
    let Ok(mut t24) = clip16_i32(token, _mm512_add_epi32(t24a, t25a), bounds);
    let Ok(mut t25) = clip16_i32(token, _mm512_sub_epi32(t24a, t25a), bounds);
    let Ok(mut t26) = clip16_i32(token, _mm512_sub_epi32(t27a, t26a), bounds);
    let Ok(t27) = clip16_i32(token, _mm512_add_epi32(t27a, t26a), bounds);
    let Ok(t28) = clip16_i32(token, _mm512_add_epi32(t28a, t29a), bounds);
    let Ok(mut t29) = clip16_i32(token, _mm512_sub_epi32(t28a, t29a), bounds);
    let Ok(mut t30) = clip16_i32(token, _mm512_sub_epi32(t31a, t30a), bounds);
    let Ok(mut t31) = clip16_i32(token, _mm512_add_epi32(t31a, t30a), bounds);

    let t17a_2 = _mm512_sub_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (t30, 799), right: (t17, ::core::primitive::i32::wrapping_sub(4017, 4096)), bias: 2048 }); combined },
        t17,
    );
    let t30a_2 = _mm512_add_epi32(
        { let Ok(combined) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (t30, ::core::primitive::i32::wrapping_sub(4017, 4096)), right: (t17, 799), bias: 2048 }); combined },
        t30,
    );
    let t18a_inner = _mm512_add_epi32(
        _mm512_mullo_epi32(t29, _mm512_set1_epi32(::core::primitive::i32::wrapping_sub(4017, 4096))),
        _mm512_mullo_epi32(t18, _mm512_set1_epi32(799)),
    );
    let t18a_2 = _mm512_sub_epi32(
        _mm512_srai_epi32::<12>(_mm512_add_epi32(
            _mm512_sub_epi32(_mm512_setzero_si512(), t18a_inner),
            _mm512_set1_epi32(2048),
        )),
        t29,
    );
    let t29a_2 = _mm512_sub_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (t29, 799), right: (t18, ::core::primitive::i32::wrapping_sub(4017, 4096)), bias: 2048 }); combined },
        t18,
    );
    let Ok(t21a_2) = mac_msub_shr_v4::<11>(token, RoundedProducts { left: (t26, 1703), right: (t21, 1138), bias: 1024 });
    let Ok(t26a_2) = mac_madd_shr_v4::<11>(token, RoundedProducts { left: (t26, 1138), right: (t21, 1703), bias: 1024 });
    let t22a_inner = _mm512_add_epi32(
        _mm512_mullo_epi32(t25, _mm512_set1_epi32(1138)),
        _mm512_mullo_epi32(t22, _mm512_set1_epi32(1703)),
    );
    let t22a_2 = _mm512_srai_epi32::<11>(_mm512_add_epi32(
        _mm512_sub_epi32(_mm512_setzero_si512(), t22a_inner),
        _mm512_set1_epi32(1024),
    ));
    let Ok(t25a_2) = mac_msub_shr_v4::<11>(token, RoundedProducts { left: (t25, 1703), right: (t22, 1138), bias: 1024 });

    let Ok(t16a_2) = clip16_i32(token, _mm512_add_epi32(t16, t19), bounds);
    t17 = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t17a_2, t18a_2), bounds); clipped };
    t18 = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t17a_2, t18a_2), bounds); clipped };
    let Ok(t19a_2) = clip16_i32(token, _mm512_sub_epi32(t16, t19), bounds);
    let Ok(t20a_2) = clip16_i32(token, _mm512_sub_epi32(t23, t20), bounds);
    t21 = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t22a_2, t21a_2), bounds); clipped };
    t22 = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t22a_2, t21a_2), bounds); clipped };
    let Ok(t23a_2) = clip16_i32(token, _mm512_add_epi32(t23, t20), bounds);
    let Ok(t24a_2) = clip16_i32(token, _mm512_add_epi32(t24, t27), bounds);
    t25 = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t25a_2, t26a_2), bounds); clipped };
    t26 = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t25a_2, t26a_2), bounds); clipped };
    let Ok(t27a_2) = clip16_i32(token, _mm512_sub_epi32(t24, t27), bounds);
    let Ok(t28a_2) = clip16_i32(token, _mm512_sub_epi32(t31, t28), bounds);
    t29 = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t30a_2, t29a_2), bounds); clipped };
    t30 = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t30a_2, t29a_2), bounds); clipped };
    let Ok(t31a_2) = clip16_i32(token, _mm512_add_epi32(t31, t28), bounds);

    let t18a_3 = _mm512_sub_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (t29, 1567), right: (t18, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t18,
    );
    let t29a_3 = _mm512_add_epi32(
        { let Ok(combined) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (t29, ::core::primitive::i32::wrapping_sub(3784, 4096)), right: (t18, 1567), bias: 2048 }); combined },
        t29,
    );
    let t19_2 = _mm512_sub_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (t28a_2, 1567), right: (t19a_2, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t19a_2,
    );
    let t28_2 = _mm512_add_epi32(
        { let Ok(combined) = mac_madd_shr_v4::<12>(token, RoundedProducts { left: (t28a_2, ::core::primitive::i32::wrapping_sub(3784, 4096)), right: (t19a_2, 1567), bias: 2048 }); combined },
        t28a_2,
    );
    let t20_inner = _mm512_add_epi32(
        _mm512_mullo_epi32(t27a_2, _mm512_set1_epi32(::core::primitive::i32::wrapping_sub(3784, 4096))),
        _mm512_mullo_epi32(t20a_2, _mm512_set1_epi32(1567)),
    );
    let t20_2 = _mm512_sub_epi32(
        _mm512_srai_epi32::<12>(_mm512_add_epi32(
            _mm512_sub_epi32(_mm512_setzero_si512(), t20_inner),
            _mm512_set1_epi32(2048),
        )),
        t27a_2,
    );
    let t27_2 = _mm512_sub_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (t27a_2, 1567), right: (t20a_2, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t20a_2,
    );
    let t21a_inner = _mm512_add_epi32(
        _mm512_mullo_epi32(t26, _mm512_set1_epi32(::core::primitive::i32::wrapping_sub(3784, 4096))),
        _mm512_mullo_epi32(t21, _mm512_set1_epi32(1567)),
    );
    let t21a_3 = _mm512_sub_epi32(
        _mm512_srai_epi32::<12>(_mm512_add_epi32(
            _mm512_sub_epi32(_mm512_setzero_si512(), t21a_inner),
            _mm512_set1_epi32(2048),
        )),
        t26,
    );
    let t26a_3 = _mm512_sub_epi32(
        { let Ok(combined) = mac_msub_shr_v4::<12>(token, RoundedProducts { left: (t26, 1567), right: (t21, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t21,
    );

    t16 = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t16a_2, t23a_2), bounds); clipped };
    let Ok(t17a_3) = clip16_i32(token, _mm512_add_epi32(t17, t22), bounds);
    t18 = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t18a_3, t21a_3), bounds); clipped };
    let Ok(t19a_3) = clip16_i32(token, _mm512_add_epi32(t19_2, t20_2), bounds);
    let Ok(t20a_3) = clip16_i32(token, _mm512_sub_epi32(t19_2, t20_2), bounds);
    t21 = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t18a_3, t21a_3), bounds); clipped };
    let Ok(t22a_3) = clip16_i32(token, _mm512_sub_epi32(t17, t22), bounds);
    t23 = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t16a_2, t23a_2), bounds); clipped };
    t24 = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t31a_2, t24a_2), bounds); clipped };
    let Ok(t25a_3) = clip16_i32(token, _mm512_sub_epi32(t30, t25), bounds);
    t26 = { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t29a_3, t26a_3), bounds); clipped };
    let Ok(t27a_3) = clip16_i32(token, _mm512_sub_epi32(t28_2, t27_2), bounds);
    let Ok(t28a_3) = clip16_i32(token, _mm512_add_epi32(t28_2, t27_2), bounds);
    t29 = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t29a_3, t26a_3), bounds); clipped };
    let Ok(t30a_3) = clip16_i32(token, _mm512_add_epi32(t30, t25), bounds);
    t31 = { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t31a_2, t24a_2), bounds); clipped };

    let mul181_sum = |a: __m512i, b: __m512i| {
        let s = _mm512_add_epi32(a, b);
        _mm512_srai_epi32::<8>(_mm512_add_epi32(
            _mm512_mullo_epi32(s, _mm512_set1_epi32(181)),
            _mm512_set1_epi32(128),
        ))
    };
    let mul181_diff = |a: __m512i, b: __m512i| {
        let s = _mm512_sub_epi32(a, b);
        _mm512_srai_epi32::<8>(_mm512_add_epi32(
            _mm512_mullo_epi32(s, _mm512_set1_epi32(181)),
            _mm512_set1_epi32(128),
        ))
    };
    let t20_final = mul181_diff(t27a_3, t20a_3);
    let t27_final = mul181_sum(t27a_3, t20a_3);
    let t21a_final = mul181_diff(t26, t21);
    let t26a_final = mul181_sum(t26, t21);
    let t22_final = mul181_diff(t25a_3, t22a_3);
    let t25_final = mul181_sum(t25a_3, t22a_3);
    let t23a_3 = mul181_diff(t24, t23);
    let t24a_3 = mul181_sum(t24, t23);

    let [t0, t1, t2, t3, t4, t5, t6, t7, t8, t9, t10, t11, t12, t13, t14, t15] = even;

    *c = [
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t0, t31), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t1, t30a_3), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t2, t29), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t3, t28a_3), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t4, t27_final), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t5, t26a_final), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t6, t25_final), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t7, t24a_3), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t8, t23a_3), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t9, t22_final), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t10, t21a_final), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t11, t20_final), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t12, t19a_3), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t13, t18), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t14, t17a_3), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_add_epi32(t15, t16), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t15, t16), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t14, t17a_3), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t13, t18), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t12, t19a_3), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t11, t20_final), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t10, t21a_final), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t9, t22_final), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t8, t23a_3), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t7, t24a_3), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t6, t25_final), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t5, t26a_final), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t4, t27_final), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t3, t28a_3), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t2, t29), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t1, t30a_3), bounds); clipped },
        { let Ok(clipped) = clip16_i32(token, _mm512_sub_epi32(t0, t31), bounds); clipped },
    ];
    Ok(())
}



#[cfg(target_arch = "x86_64")]
enum ColumnDispatch {
  Applied,
  Fallback,
}

#[cfg(target_arch = "x86_64")]
fn scratch_column_dispatch(result: Option<Rav1dResult>) -> Result<ColumnDispatch, console_core_never::Never> {
  Ok(match result {
    Some(Ok(())) => ColumnDispatch::Applied,
    Some(Err(error)) => {
      console_core_logging::debug!("Invalid AV1 column transform scratch: {error:?}");
      ColumnDispatch::Fallback
    }
    None => ColumnDispatch::Fallback,
  })
}

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
struct ScratchColumn {
  width: NonZeroU32,
  first: u32,
}

#[cfg(target_arch = "x86_64")]
fn checked_scratch(
  scratch: &mut [i32],
  size: console_core_geometry::Size<u32>,
) -> Rav1dResult<&mut [i32]> {
  let area = size.width.checked_mul(size.height).ok_or(Rav1dError::EINVAL)?;
  let Ok(end) = console_core_number_conversion::index(area);
  scratch.get_mut(..end).ok_or(Rav1dError::EINVAL)
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn gather_scratch_column<const ROWS: usize>(
  _token: Server64,
  scratch: &[i32],
  column: ScratchColumn,
) -> Rav1dResult<[__m512i; ROWS]> {
  let Ok(width) = console_core_number_conversion::index(column.width.get());
  let Ok(first) = console_core_number_conversion::index(column.first);
  let mut values = [_mm512_setzero_si512(); ROWS];

  for (output, row) in values.iter_mut().zip(scratch.chunks_exact(width.max(1))) {
    let elements = row.get(first..).and_then(<[_]>::first_chunk::<16>);
    let lane = elements.ok_or(Rav1dError::EINVAL)?;
    *output = loadu_512!(lane);
  }

  Ok(values)
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn store_scratch_column<const ROWS: usize>(
  _token: Server64,
  scratch: &mut [i32],
  column: ScratchColumn,
  values: [__m512i; ROWS],
) -> Rav1dResult {
  let Ok(width) = console_core_number_conversion::index(column.width.get());
  let Ok(first) = console_core_number_conversion::index(column.first);

  for (value, row) in values.into_iter().zip(scratch.chunks_exact_mut(width.max(1))) {
    let elements = row.get_mut(first..).and_then(<[_]>::first_chunk_mut::<16>);
    let lane = elements.ok_or(Rav1dError::EINVAL)?;
    storeu_512!(lane, value);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn apply_transform_columns_avx2<const ROWS: usize>(
  token: Desktop64,
  scratch: &mut [i32],
  size: console_core_geometry::Size<u32>,
  bounds: VectorBounds<__m256i>,
  transform: impl Fn(Desktop64, &mut [__m256i; ROWS], VectorBounds<__m256i>) -> Result<(), Never>,
) -> Rav1dResult {
  let row_width = NonZeroU32::new(size.width).ok_or(Rav1dError::EINVAL)?;
  let active = checked_scratch(scratch, size)?;
  let Ok(stride) = console_core_number_conversion::index(row_width.get());

  for column in 0..size.width.wrapping_div(8) {
    let Ok(first) = console_core_number_conversion::index(column.wrapping_mul(8));
    let mut lanes = [_mm256_setzero_si256(); ROWS];

    for (lane, row) in lanes.iter_mut().zip(active.chunks_exact(stride.max(1))) {
      let chunk = row.get(first..).and_then(<[_]>::first_chunk::<8>);
      let values = chunk.ok_or(Rav1dError::EINVAL)?;
      *lane = loadu_256!(values);
    }

    let Ok(()) = transform(token, &mut lanes, bounds);

    for (lane, row) in lanes.into_iter().zip(active.chunks_exact_mut(stride.max(1))) {
      let chunk = row.get_mut(first..).and_then(<[_]>::first_chunk_mut::<8>);
      let values = chunk.ok_or(Rav1dError::EINVAL)?;
      storeu_256!(values, lane);
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn dct4_cols_avx512(
  token: Server64,
  scratch: &mut [i32],
  size: console_core_geometry::Size<u32>,
  clipping: CoefficientBounds,
) -> Rav1dResult {
  let width = match NonZeroU32::new(size.width) {
    Some(columns) => columns,
    None => return Ok(()),
  };
  let extent = console_core_geometry::Size { width: width.get(), height: size.height.min(4) };
  let active = checked_scratch(scratch, extent)?;
  let bounds = VectorBounds {
    lower: _mm512_set1_epi32(clipping.lower),
    upper: _mm512_set1_epi32(clipping.upper),
  };

  for strip in 0..width.get().wrapping_div(16) {
    let column = ScratchColumn { width, first: strip.wrapping_mul(16) };
    let mut values = gather_scratch_column::<4>(token, active, column)?;
    let Ok(()) = dct4_1d_cols16(token, &mut values, bounds);
    store_scratch_column(token, active, column, values)?;
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn dct8_cols_avx512(
  token: Server64,
  scratch: &mut [i32],
  size: console_core_geometry::Size<u32>,
  clipping: CoefficientBounds,
) -> Rav1dResult {
  let width = match NonZeroU32::new(size.width) {
    Some(columns) => columns,
    None => return Ok(()),
  };
  let extent = console_core_geometry::Size { width: width.get(), height: size.height.min(8) };
  let active = checked_scratch(scratch, extent)?;
  let bounds = VectorBounds {
    lower: _mm512_set1_epi32(clipping.lower),
    upper: _mm512_set1_epi32(clipping.upper),
  };

  for strip in 0..width.get().wrapping_div(16) {
    let column = ScratchColumn { width, first: strip.wrapping_mul(16) };
    let mut values = gather_scratch_column::<8>(token, active, column)?;
    let Ok(()) = dct8_1d_cols16(token, &mut values, bounds);
    store_scratch_column(token, active, column, values)?;
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn dct16_cols_avx512(
  token: Server64,
  scratch: &mut [i32],
  size: console_core_geometry::Size<u32>,
  clipping: CoefficientBounds,
) -> Rav1dResult {
  let width = match NonZeroU32::new(size.width) {
    Some(columns) => columns,
    None => return Ok(()),
  };
  let extent = console_core_geometry::Size { width: width.get(), height: size.height.min(16) };
  let active = checked_scratch(scratch, extent)?;
  let bounds = VectorBounds {
    lower: _mm512_set1_epi32(clipping.lower),
    upper: _mm512_set1_epi32(clipping.upper),
  };

  for strip in 0..width.get().wrapping_div(16) {
    let column = ScratchColumn { width, first: strip.wrapping_mul(16) };
    let mut values = gather_scratch_column::<16>(token, active, column)?;
    let Ok(()) = dct16_1d_cols16(token, &mut values, bounds);
    store_scratch_column(token, active, column, values)?;
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn dct32_cols_avx512(
  token: Server64,
  scratch: &mut [i32],
  size: console_core_geometry::Size<u32>,
  clipping: CoefficientBounds,
) -> Rav1dResult {
  let width = match NonZeroU32::new(size.width) {
    Some(columns) => columns,
    None => return Ok(()),
  };
  let extent = console_core_geometry::Size { width: width.get(), height: size.height.min(32) };
  let active = checked_scratch(scratch, extent)?;
  let bounds = VectorBounds {
    lower: _mm512_set1_epi32(clipping.lower),
    upper: _mm512_set1_epi32(clipping.upper),
  };

  for strip in 0..width.get().wrapping_div(16) {
    let column = ScratchColumn { width, first: strip.wrapping_mul(16) };
    let mut values = gather_scratch_column::<32>(token, active, column)?;
    let Ok(()) = dct32_1d_cols16(token, &mut values, bounds);
    store_scratch_column(token, active, column, values)?;
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn identity_shift_cols_avx512<const SHIFT: u32>(
  _token: Server64,
  scratch: &mut [i32],
  size: console_core_geometry::Size<u32>,
) -> Rav1dResult {
  let width = match NonZeroU32::new(size.width) {
    Some(columns) => columns,
    None => return Ok(()),
  };
  let active = checked_scratch(scratch, size)?;
  let Ok(row_width) = console_core_number_conversion::index(width.get());

  for row in active.chunks_exact_mut(row_width.max(1)) {
    let (columns, _) = row.as_chunks_mut::<16>();

    for lane in columns {
      let value = loadu_512!(&*lane);
      let shifted = _mm512_slli_epi32::<SHIFT>(value);
      storeu_512!(lane, shifted);
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn identity16_cols_avx512(
  _token: Server64,
  scratch: &mut [i32],
  size: console_core_geometry::Size<u32>,
) -> Rav1dResult {
  let width = match NonZeroU32::new(size.width) {
    Some(columns) => columns,
    None => return Ok(()),
  };
  let active = checked_scratch(scratch, size)?;
  let Ok(row_width) = console_core_number_conversion::index(width.get());
  let scale = _mm512_set1_epi32(1697);
  let rounding = _mm512_set1_epi32(1024);

  for row in active.chunks_exact_mut(row_width.max(1)) {
    let (columns, _) = row.as_chunks_mut::<16>();

    for lane in columns {
      let value = loadu_512!(&*lane);
      let twice = _mm512_slli_epi32::<1>(value);
      let scaled = _mm512_mullo_epi32(value, scale);
      let shifted = _mm512_srai_epi32::<11>(_mm512_add_epi32(scaled, rounding));
      let result = _mm512_add_epi32(twice, shifted);
      storeu_512!(lane, result);
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn transpose_dword_quartets(
  _token: Server64,
  columns: [__m512i; 4],
) -> Result<[__m512i; 4], Never> {
  let [a, b, c, d] = columns;
  let ab_low = _mm512_unpacklo_epi32(a, b);
  let ab_high = _mm512_unpackhi_epi32(a, b);
  let cd_low = _mm512_unpacklo_epi32(c, d);
  let cd_high = _mm512_unpackhi_epi32(c, d);
  Ok([
    _mm512_unpacklo_epi64(ab_low, cd_low),
    _mm512_unpackhi_epi64(ab_low, cd_low),
    _mm512_unpacklo_epi64(ab_high, cd_high),
    _mm512_unpackhi_epi64(ab_high, cd_high),
  ])
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn shuffle_128_bit_lanes<const CONTROL: i32>(
  _token: Server64,
  halves: [[__m512i; 4]; 2],
) -> Result<[__m512i; 4], Never> {
  let [[a, b, c, d], [e, f, g, h]] = halves;
  Ok([
    _mm512_shuffle_i32x4::<CONTROL>(a, e),
    _mm512_shuffle_i32x4::<CONTROL>(b, f),
    _mm512_shuffle_i32x4::<CONTROL>(c, g),
    _mm512_shuffle_i32x4::<CONTROL>(d, h),
  ])
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn transpose_16x16_i32(token: Server64, columns: [__m512i; 16]) -> Result<[__m512i; 16], Never> {
  let [a, b, c, d, e, f, g, h, j, k, l, m, n, p, q, r] = columns;
  let Ok(first) = transpose_dword_quartets(token, [a, b, c, d]);
  let Ok(second) = transpose_dword_quartets(token, [e, f, g, h]);
  let Ok(third) = transpose_dword_quartets(token, [j, k, l, m]);
  let Ok(fourth) = transpose_dword_quartets(token, [n, p, q, r]);

  let Ok(left_even) = shuffle_128_bit_lanes::<0x88>(token, [first, second]);
  let Ok(left_odd) = shuffle_128_bit_lanes::<0xdd>(token, [first, second]);
  let Ok(right_even) = shuffle_128_bit_lanes::<0x88>(token, [third, fourth]);
  let Ok(right_odd) = shuffle_128_bit_lanes::<0xdd>(token, [third, fourth]);

  let Ok([r0, r1, r2, r3]) = shuffle_128_bit_lanes::<0x88>(token, [left_even, right_even]);
  let Ok([r4, r5, r6, r7]) = shuffle_128_bit_lanes::<0x88>(token, [left_odd, right_odd]);
  let Ok([r8, r9, r10, r11]) = shuffle_128_bit_lanes::<0xdd>(token, [left_even, right_even]);
  let Ok([r12, r13, r14, r15]) = shuffle_128_bit_lanes::<0xdd>(token, [left_odd, right_odd]);
  Ok([r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15])
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn load_avx512_row_lanes<const WIDTH: usize>(
  _token: Server64,
  coefficients: &[i16],
  first_row: u32,
  pass: RowPass,
) -> Rav1dResult<[__m512i; WIDTH]> {
  let multiplier = _mm512_set1_epi32(181);
  let bias = _mm512_set1_epi32(128);
  let mut vectors = [_mm512_setzero_si512(); WIDTH];

  for (output, column) in vectors.iter_mut().zip(0u32..) {
    let location = console_core_geometry::Point { x: column, y: first_row };
    let lane = pass.coefficient_lane::<16>(coefficients, location)?;
    let values = _mm512_cvtepi16_epi32(loadu_256!(lane));
    *output = match pass.scale {
      RectangularScale::Apply => _mm512_srai_epi32::<8>(_mm512_add_epi32(_mm512_mullo_epi32(values, multiplier), bias)),
      RectangularScale::Skip => values,
    };
  }

  Ok(vectors)
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn round_avx512_row_lanes<const WIDTH: usize>(
  _token: Server64,
  vectors: &mut [__m512i; WIDTH],
  pass: RowPass,
) -> Result<(), console_core_never::Never> {
  let lower = _mm512_set1_epi32(pass.column_bounds.lower);
  let upper = _mm512_set1_epi32(pass.column_bounds.upper);
  let rounding = _mm512_set1_epi32(pass.rounding);

  for vector in vectors {
    let with_bias = _mm512_add_epi32(*vector, rounding);
    let rounded = match pass.shift {
      0 => with_bias,
      1 => _mm512_srai_epi32::<1>(with_bias),
      _ => _mm512_srai_epi32::<2>(with_bias),
    };
    *vector = _mm512_max_epi32(_mm512_min_epi32(rounded, upper), lower);
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn store_avx512_row_lanes<const WIDTH: usize>(
  _token: Server64,
  scratch: &mut [i32],
  first_row: u32,
  vectors: &[__m512i; WIDTH],
) -> Rav1dResult {
  let width = u32::try_from(WIDTH).map_err(|_| Rav1dError::EINVAL)?;
  let row_width = NonZeroU32::new(width).ok_or(Rav1dError::EINVAL)?;
  let (strips, remainder) = vectors.as_chunks::<16>();

  match remainder {
      [] => {}
      [_, ..] => return Err(Rav1dError::EINVAL),
  }

  let last_row = first_row.checked_add(16).ok_or(Rav1dError::EINVAL)?;
  let active = checked_scratch_rows(scratch, row_width, first_row..last_row)?;
  let Ok(stride) = console_core_number_conversion::index(row_width.get());

  for (columns, segment) in strips.iter().zip(0u32..) {
    let Ok(rows) = transpose_16x16_i32(_token, *columns);
    let Ok(offset) = console_core_number_conversion::index(segment.wrapping_mul(16));

    for (output, row) in active.chunks_exact_mut(stride.max(1)).zip(rows) {
      let lane = output.get_mut(offset..).and_then(<[_]>::first_chunk_mut::<16>);
      let pixels = lane.ok_or(Rav1dError::EINVAL)?;
      storeu_512!(pixels, row);
    }
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn simd_row_dct8_8bpc_16rows_entry(
  token: Server64,
  coefficients: &[i16],
  first_row: u32,
  scratch: &mut [i32],
  pass: RowPass,
) -> Rav1dResult {
  let mut vectors = load_avx512_row_lanes::<8>(token, coefficients, first_row, pass)?;
  let bounds = VectorBounds {
    lower: _mm512_set1_epi32(pass.row_bounds.lower),
    upper: _mm512_set1_epi32(pass.row_bounds.upper),
  };
  let Ok(()) = dct8_1d_cols16(token, &mut vectors, bounds);
  let Ok(()) = round_avx512_row_lanes(token, &mut vectors, pass);

  let [a, b, c, d, e, f, g, h] = vectors;
  let zero = _mm512_setzero_si512();
  let padded = [a, b, c, d, e, f, g, h, zero, zero, zero, zero, zero, zero, zero, zero];
  let Ok(rows) = transpose_16x16_i32(token, padded);
  let last_row = first_row.checked_add(16).ok_or(Rav1dError::EINVAL)?;
  let active = checked_scratch_rows(scratch, STRIDE_8, first_row..last_row)?;
  let (output_rows, _) = active.as_chunks_mut::<8>();

  for (output, row) in output_rows.iter_mut().zip(rows) {
    storeu_256!(output, _mm512_castsi512_si256(row));
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
#[inline(always)]
fn row_dct8_8bpc_block(
  token: Desktop64,
  coefficients: &[i16],
  rows: u32,
  scratch: &mut [i32],
  pass: RowPass,
) -> Rav1dResult {
  let Ok(avx512) = match rows {
    16.. => crate::src::cpu::summon_avx512(),
    _ => Ok(None),
  };
  let mut processed = 0;

  match avx512 {
    Some(wide_token) => {
      let batches = rows.wrapping_div(16);

      for batch in 0..batches {
        let first_row = batch.wrapping_mul(16);
        simd_row_dct8_8bpc_16rows_entry(wide_token, coefficients, first_row, scratch, pass)?;
      }

      processed = batches.wrapping_mul(16);
    }
    None => {}
  }

  let remaining = rows.wrapping_sub(processed).wrapping_div(8);

  for batch in 0..remaining {
    let first_row = processed.wrapping_add(batch.wrapping_mul(8));
    simd_row_dct8_8bpc_8rows(token, coefficients, first_row, scratch, pass)?;
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn simd_row_dct16_8bpc_16rows_entry(
  token: Server64,
  coefficients: &[i16],
  first_row: u32,
  scratch: &mut [i32],
  pass: RowPass,
) -> Rav1dResult {
  let mut vectors = load_avx512_row_lanes::<16>(token, coefficients, first_row, pass)?;
  let bounds = VectorBounds {
    lower: _mm512_set1_epi32(pass.row_bounds.lower),
    upper: _mm512_set1_epi32(pass.row_bounds.upper),
  };
  let Ok(()) = dct16_1d_cols16(token, &mut vectors, bounds);
  let Ok(()) = round_avx512_row_lanes(token, &mut vectors, pass);

  store_avx512_row_lanes(token, scratch, first_row, &vectors)
}

#[cfg(target_arch = "x86_64")]
#[rite]
#[inline(always)]
fn row_dct16_8bpc_block(
  token: Desktop64,
  coefficients: &[i16],
  rows: u32,
  scratch: &mut [i32],
  pass: RowPass,
) -> Rav1dResult {
  let Ok(avx512) = match rows {
    16.. => crate::src::cpu::summon_avx512(),
    _ => Ok(None),
  };
  let mut processed = 0;

  match avx512 {
    Some(wide_token) => {
      let batches = rows.wrapping_div(16);

      for batch in 0..batches {
        let first_row = batch.wrapping_mul(16);
        simd_row_dct16_8bpc_16rows_entry(wide_token, coefficients, first_row, scratch, pass)?;
      }

      processed = batches.wrapping_mul(16);
    }
    None => {}
  }

  let remaining = rows.wrapping_sub(processed).wrapping_div(8);

  for batch in 0..remaining {
    let first_row = processed.wrapping_add(batch.wrapping_mul(8));
    simd_row_dct16_8bpc_8rows(token, coefficients, first_row, scratch, pass)?;
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[arcane]
fn simd_row_dct32_8bpc_16rows_entry(
  token: Server64,
  coefficients: &[i16],
  first_row: u32,
  scratch: &mut [i32],
  pass: RowPass,
) -> Rav1dResult {
  let mut vectors = load_avx512_row_lanes::<32>(token, coefficients, first_row, pass)?;
  let bounds = VectorBounds {
    lower: _mm512_set1_epi32(pass.row_bounds.lower),
    upper: _mm512_set1_epi32(pass.row_bounds.upper),
  };
  let Ok(()) = dct32_1d_cols16(token, &mut vectors, bounds);
  let Ok(()) = round_avx512_row_lanes(token, &mut vectors, pass);

  store_avx512_row_lanes(token, scratch, first_row, &vectors)
}

#[cfg(target_arch = "x86_64")]
#[rite]
#[inline(always)]
fn row_dct32_8bpc_block(
  token: Desktop64,
  coefficients: &[i16],
  rows: u32,
  scratch: &mut [i32],
  pass: RowPass,
) -> Rav1dResult {
  let Ok(avx512) = match rows {
    16.. => crate::src::cpu::summon_avx512(),
    _ => Ok(None),
  };
  let mut processed = 0;

  match avx512 {
    Some(wide_token) => {
      let batches = rows.wrapping_div(16);

      for batch in 0..batches {
        let first_row = batch.wrapping_mul(16);
        simd_row_dct32_8bpc_16rows_entry(wide_token, coefficients, first_row, scratch, pass)?;
      }

      processed = batches.wrapping_mul(16);
    }
    None => {}
  }

  let remaining = rows.wrapping_sub(processed).wrapping_div(8);

  for batch in 0..remaining {
    let first_row = processed.wrapping_add(batch.wrapping_mul(8));
    simd_row_dct32_8bpc_8rows(token, coefficients, first_row, scratch, pass)?;
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn dct4_1d_cols8(token: Desktop64, c: &mut [__m256i; 4], bounds: VectorBounds<__m256i>) -> Result<(), Never> {
    let &mut [in0, ..] = c;
    let &mut [_, in1, ..] = c;
    let &mut [_, _, in2, ..] = c;
    let &mut [_, _, _, in3, ..] = c;

    let sum02 = _mm256_add_epi32(in0, in2);
    let sub02 = _mm256_sub_epi32(in0, in2);
    let t0 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_mullo_epi32(sum02, _mm256_set1_epi32(181)),
        _mm256_set1_epi32(128),
    ));
    let t1 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_mullo_epi32(sub02, _mm256_set1_epi32(181)),
        _mm256_set1_epi32(128),
    ));

    let Ok(t2_shifted) = mac_msub_shr::<12>(token, RoundedProducts { left: (in1, 1567), right: (in3, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 });
    let t2 = _mm256_sub_epi32(t2_shifted, in3);

    let Ok(t3_shifted) = mac_madd_shr::<12>(token, RoundedProducts { left: (in1, ::core::primitive::i32::wrapping_sub(3784, 4096)), right: (in3, 1567), bias: 2048 });
    let t3 = _mm256_add_epi32(t3_shifted, in1);

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t0, t3), bounds); clipped };
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t1, t2), bounds); clipped };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t1, t2), bounds); clipped };
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t0, t3), bounds); clipped };
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn dct8_1d_cols8(token: Desktop64, c: &mut [__m256i; 8], bounds: VectorBounds<__m256i>) -> Result<(), Never> {

    let mut even = [match c { &mut [_explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }];
    let Ok(()) = dct4_1d_cols8(token, &mut even, bounds);
    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = match even { [_explicit_element, ..] => _explicit_element };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _explicit_element, ..] => _explicit_element };

    let &mut [_, in1, ..] = c;
    let &mut [_, _, _, in3, ..] = c;
    let &mut [_, _, _, _, _, in5, ..] = c;
    let &mut [_, _, _, _, _, _, _, in7, ..] = c;


    let t4a = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in1, 799), right: (in7, ::core::primitive::i32::wrapping_sub(4017, 4096)), bias: 2048 }); combined },
        in7,
    );

    let Ok(t5a) = mac_msub_shr::<11>(token, RoundedProducts { left: (in5, 1703), right: (in3, 1138), bias: 1024 });

    let Ok(t6a) = mac_madd_shr::<11>(token, RoundedProducts { left: (in5, 1138), right: (in3, 1703), bias: 1024 });

    let t7a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in1, ::core::primitive::i32::wrapping_sub(4017, 4096)), right: (in7, 799), bias: 2048 }); combined },
        in1,
    );

    let Ok(t4) = clip8(token, _mm256_add_epi32(t4a, t5a), bounds);
    let Ok(t5a_n) = clip8(token, _mm256_sub_epi32(t4a, t5a), bounds);
    let Ok(t7) = clip8(token, _mm256_add_epi32(t7a, t6a), bounds);
    let Ok(t6a_n) = clip8(token, _mm256_sub_epi32(t7a, t6a), bounds);


    let d = _mm256_sub_epi32(t6a_n, t5a_n);
    let t5 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_mullo_epi32(d, _mm256_set1_epi32(181)),
        _mm256_set1_epi32(128),
    ));

    let s = _mm256_add_epi32(t6a_n, t5a_n);
    let t6 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_mullo_epi32(s, _mm256_set1_epi32(181)),
        _mm256_set1_epi32(128),
    ));

    let &mut [t0, ..] = c;
    let &mut [_, _, t1, ..] = c;
    let &mut [_, _, _, _, t2, ..] = c;
    let &mut [_, _, _, _, _, _, t3, ..] = c;

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t0, t7), bounds); clipped };
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t1, t6), bounds); clipped };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t2, t5), bounds); clipped };
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t3, t4), bounds); clipped };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t3, t4), bounds); clipped };
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t2, t5), bounds); clipped };
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t1, t6), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t0, t7), bounds); clipped };
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn adst8_1d_cols8(token: Desktop64, c: &mut [__m256i; 8], bounds: VectorBounds<__m256i>) -> Result<(), Never> {
    let &mut [in0, ..] = c;
    let &mut [_, in1, ..] = c;
    let &mut [_, _, in2, ..] = c;
    let &mut [_, _, _, in3, ..] = c;
    let &mut [_, _, _, _, in4, ..] = c;
    let &mut [_, _, _, _, _, in5, ..] = c;
    let &mut [_, _, _, _, _, _, in6, ..] = c;
    let &mut [_, _, _, _, _, _, _, in7, ..] = c;

    let t0a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in7, ::core::primitive::i32::wrapping_sub(4076, 4096)), right: (in0, 401), bias: 2048 }); combined },
        in7,
    );
    let t1a = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in7, 401), right: (in0, ::core::primitive::i32::wrapping_sub(4076, 4096)), bias: 2048 }); combined },
        in0,
    );
    let t2a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in5, ::core::primitive::i32::wrapping_sub(3612, 4096)), right: (in2, 1931), bias: 2048 }); combined },
        in5,
    );
    let t3a = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in5, 1931), right: (in2, ::core::primitive::i32::wrapping_sub(3612, 4096)), bias: 2048 }); combined },
        in2,
    );
    let Ok(t4a) = mac_madd_shr::<11>(token, RoundedProducts { left: (in3, 1299), right: (in4, 1583), bias: 1024 });
    let Ok(t5a) = mac_msub_shr::<11>(token, RoundedProducts { left: (in3, 1583), right: (in4, 1299), bias: 1024 });
    let t6a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in1, 1189), right: (in6, ::core::primitive::i32::wrapping_sub(3920, 4096)), bias: 2048 }); combined },
        in6,
    );
    let t7a = _mm256_add_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in1, ::core::primitive::i32::wrapping_sub(3920, 4096)), right: (in6, 1189), bias: 2048 }); combined },
        in1,
    );

    let Ok(t0) = clip8(token, _mm256_add_epi32(t0a, t4a), bounds);
    let Ok(t1) = clip8(token, _mm256_add_epi32(t1a, t5a), bounds);
    let Ok(t2) = clip8(token, _mm256_add_epi32(t2a, t6a), bounds);
    let Ok(t3) = clip8(token, _mm256_add_epi32(t3a, t7a), bounds);
    let Ok(t4) = clip8(token, _mm256_sub_epi32(t0a, t4a), bounds);
    let Ok(t5) = clip8(token, _mm256_sub_epi32(t1a, t5a), bounds);
    let Ok(t6) = clip8(token, _mm256_sub_epi32(t2a, t6a), bounds);
    let Ok(t7) = clip8(token, _mm256_sub_epi32(t3a, t7a), bounds);

    let t4a_2 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (t4, ::core::primitive::i32::wrapping_sub(3784, 4096)), right: (t5, 1567), bias: 2048 }); combined },
        t4,
    );
    let t5a_2 = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t4, 1567), right: (t5, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t5,
    );
    let t6a_2 = _mm256_add_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t7, ::core::primitive::i32::wrapping_sub(3784, 4096)), right: (t6, 1567), bias: 2048 }); combined },
        t7,
    );
    let t7a_2 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (t7, 1567), right: (t6, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t6,
    );

    let zero = _mm256_setzero_si256();
    let Ok(out0) = clip8(token, _mm256_add_epi32(t0, t2), bounds);
    let out7 = _mm256_sub_epi32(zero, { let Ok(clipped) = clip8(token, _mm256_add_epi32(t1, t3), bounds); clipped });
    let Ok(t2_final) = clip8(token, _mm256_sub_epi32(t0, t2), bounds);
    let Ok(t3_final) = clip8(token, _mm256_sub_epi32(t1, t3), bounds);
    let out1 = _mm256_sub_epi32(zero, { let Ok(clipped) = clip8(token, _mm256_add_epi32(t4a_2, t6a_2), bounds); clipped });
    let Ok(out6) = clip8(token, _mm256_add_epi32(t5a_2, t7a_2), bounds);
    let Ok(t6_final) = clip8(token, _mm256_sub_epi32(t4a_2, t6a_2), bounds);
    let Ok(t7_final) = clip8(token, _mm256_sub_epi32(t5a_2, t7a_2), bounds);

    let mul181_sum = |a: __m256i, b: __m256i| {
        let s = _mm256_add_epi32(a, b);
        _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(s, _mm256_set1_epi32(181)),
            _mm256_set1_epi32(128),
        ))
    };
    let mul181_diff = |a: __m256i, b: __m256i| {
        let s = _mm256_sub_epi32(a, b);
        _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(s, _mm256_set1_epi32(181)),
            _mm256_set1_epi32(128),
        ))
    };
    let out3 = _mm256_sub_epi32(zero, mul181_sum(t2_final, t3_final));
    let out4 = mul181_diff(t2_final, t3_final);
    let out2 = mul181_sum(t6_final, t7_final);
    let out5 = _mm256_sub_epi32(zero, mul181_diff(t6_final, t7_final));

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = out0;
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = out1;
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = out2;
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out3;
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out4;
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out5;
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out6;
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out7;
    Ok(())
}



#[cfg(target_arch = "x86_64")]
#[rite]
fn flipadst8_1d_cols8(token: Desktop64, c: &mut [__m256i; 8], bounds: VectorBounds<__m256i>) -> Result<(), Never> {
    let Ok(()) = adst8_1d_cols8(token, c, bounds);
    let &mut [t0, ..] = c;
    let &mut [_, t1, ..] = c;
    let &mut [_, _, t2, ..] = c;
    let &mut [_, _, _, t3, ..] = c;
    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = match c { &mut [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = match c { &mut [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = match c { &mut [_, _, _, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match c { &mut [_, _, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = t3;
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = t2;
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = t1;
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = t0;
    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn identity8_1d_cols8(_token: Desktop64, c: &mut [__m256i; 8], _bounds: VectorBounds<__m256i>) -> Result<(), Never> {
    for c_element in <[_]>::iter_mut(c) {
        *c_element = _mm256_slli_epi32::<1>(*c_element);
    }

    Ok(())
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn dct16_1d_cols8(token: Desktop64, c: &mut [__m256i; 16], bounds: VectorBounds<__m256i>) -> Result<(), Never> {

    let mut even = [match c { &mut [_explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }];
    let Ok(()) = dct8_1d_cols8(token, &mut even, bounds);
    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = match even { [_explicit_element, ..] => _explicit_element };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element };

    let &mut [_, in1, ..] = c;
    let &mut [_, _, _, in3, ..] = c;
    let &mut [_, _, _, _, _, in5, ..] = c;
    let &mut [_, _, _, _, _, _, _, in7, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, in9, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, in11, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, in13, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, in15, ..] = c;


    let t8a = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in1, 401), right: (in15, ::core::primitive::i32::wrapping_sub(4076, 4096)), bias: 2048 }); combined },
        in15,
    );

    let Ok(t9a) = mac_msub_shr::<11>(token, RoundedProducts { left: (in9, 1583), right: (in7, 1299), bias: 1024 });

    let t10a = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in5, 1931), right: (in11, ::core::primitive::i32::wrapping_sub(3612, 4096)), bias: 2048 }); combined },
        in11,
    );

    let t11a = _mm256_add_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in13, ::core::primitive::i32::wrapping_sub(3920, 4096)), right: (in3, 1189), bias: 2048 }); combined },
        in13,
    );

    let t12a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in13, 1189), right: (in3, ::core::primitive::i32::wrapping_sub(3920, 4096)), bias: 2048 }); combined },
        in3,
    );

    let t13a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in5, ::core::primitive::i32::wrapping_sub(3612, 4096)), right: (in11, 1931), bias: 2048 }); combined },
        in5,
    );

    let Ok(t14a) = mac_madd_shr::<11>(token, RoundedProducts { left: (in9, 1299), right: (in7, 1583), bias: 1024 });

    let t15a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in1, ::core::primitive::i32::wrapping_sub(4076, 4096)), right: (in15, 401), bias: 2048 }); combined },
        in1,
    );

    let Ok(t8) = clip8(token, _mm256_add_epi32(t8a, t9a), bounds);
    let Ok(mut t9) = clip8(token, _mm256_sub_epi32(t8a, t9a), bounds);
    let Ok(mut t10) = clip8(token, _mm256_sub_epi32(t11a, t10a), bounds);
    let Ok(mut t11) = clip8(token, _mm256_add_epi32(t11a, t10a), bounds);
    let Ok(mut t12) = clip8(token, _mm256_add_epi32(t12a, t13a), bounds);
    let Ok(mut t13) = clip8(token, _mm256_sub_epi32(t12a, t13a), bounds);
    let Ok(mut t14) = clip8(token, _mm256_sub_epi32(t15a, t14a), bounds);
    let Ok(t15) = clip8(token, _mm256_add_epi32(t15a, t14a), bounds);


    let t9a_2 = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t14, 1567), right: (t9, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t9,
    );

    let t14a_2 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (t14, ::core::primitive::i32::wrapping_sub(3784, 4096)), right: (t9, 1567), bias: 2048 }); combined },
        t14,
    );

    let t10a_inner = _mm256_add_epi32(
        _mm256_mullo_epi32(t13, _mm256_set1_epi32(::core::primitive::i32::wrapping_sub(3784, 4096))),
        _mm256_mullo_epi32(t10, _mm256_set1_epi32(1567)),
    );
    let t10a_2 = _mm256_sub_epi32(
        _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(_mm256_setzero_si256(), t10a_inner),
            _mm256_set1_epi32(2048),
        )),
        t13,
    );

    let t13a_2 = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t13, 1567), right: (t10, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t10,
    );

    let Ok(t8a_2) = clip8(token, _mm256_add_epi32(t8, t11), bounds);
    t9 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t9a_2, t10a_2), bounds); clipped };
    t10 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t9a_2, t10a_2), bounds); clipped };
    let Ok(t11a_2) = clip8(token, _mm256_sub_epi32(t8, t11), bounds);
    let Ok(t12a_2) = clip8(token, _mm256_sub_epi32(t15, t12), bounds);
    t13 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t14a_2, t13a_2), bounds); clipped };
    t14 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t14a_2, t13a_2), bounds); clipped };
    let Ok(t15a_2) = clip8(token, _mm256_add_epi32(t15, t12), bounds);


    let d_13_10 = _mm256_sub_epi32(t13, t10);
    let t10a_new = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_mullo_epi32(d_13_10, _mm256_set1_epi32(181)),
        _mm256_set1_epi32(128),
    ));

    let s_13_10 = _mm256_add_epi32(t13, t10);
    let t13a_new = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_mullo_epi32(s_13_10, _mm256_set1_epi32(181)),
        _mm256_set1_epi32(128),
    ));

    let d_12a_11a = _mm256_sub_epi32(t12a_2, t11a_2);
    t11 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_mullo_epi32(d_12a_11a, _mm256_set1_epi32(181)),
        _mm256_set1_epi32(128),
    ));

    let s_12a_11a = _mm256_add_epi32(t12a_2, t11a_2);
    t12 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_mullo_epi32(s_12a_11a, _mm256_set1_epi32(181)),
        _mm256_set1_epi32(128),
    ));

    let &mut [t0, ..] = c;
    let &mut [_, _, t1, ..] = c;
    let &mut [_, _, _, _, t2, ..] = c;
    let &mut [_, _, _, _, _, _, t3, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, t4, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, t5, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, _, t6, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _, t7, ..] = c;

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t0, t15a_2), bounds); clipped };
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t1, t14), bounds); clipped };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t2, t13a_new), bounds); clipped };
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t3, t12), bounds); clipped };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t4, t11), bounds); clipped };
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t5, t10a_new), bounds); clipped };
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t6, t9), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t7, t8a_2), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t7, t8a_2), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t6, t9), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t5, t10a_new), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t4, t11), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t3, t12), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t2, t13a_new), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t1, t14), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t0, t15a_2), bounds); clipped };
    Ok(())
}



#[cfg(target_arch = "x86_64")]
#[rite]
fn adst16_1d_cols8(token: Desktop64, c: &mut [__m256i; 16], bounds: VectorBounds<__m256i>) -> Result<(), Never> {
    let &mut [in0, ..] = c;
    let &mut [_, in1, ..] = c;
    let &mut [_, _, in2, ..] = c;
    let &mut [_, _, _, in3, ..] = c;
    let &mut [_, _, _, _, in4, ..] = c;
    let &mut [_, _, _, _, _, in5, ..] = c;
    let &mut [_, _, _, _, _, _, in6, ..] = c;
    let &mut [_, _, _, _, _, _, _, in7, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, in8, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, in9, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, in10, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, in11, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, _, in12, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, in13, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _, in14, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, in15, ..] = c;


    let mut t0 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in15, ::core::primitive::i32::wrapping_sub(4091, 4096)), right: (in0, 201), bias: 2048 }); combined },
        in15,
    );
    let mut t1 = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in15, 201), right: (in0, ::core::primitive::i32::wrapping_sub(4091, 4096)), bias: 2048 }); combined },
        in0,
    );
    let mut t2 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in13, ::core::primitive::i32::wrapping_sub(3973, 4096)), right: (in2, 995), bias: 2048 }); combined },
        in13,
    );
    let mut t3 = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in13, 995), right: (in2, ::core::primitive::i32::wrapping_sub(3973, 4096)), bias: 2048 }); combined },
        in2,
    );
    let mut t4 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in11, ::core::primitive::i32::wrapping_sub(3703, 4096)), right: (in4, 1751), bias: 2048 }); combined },
        in11,
    );
    let mut t5 = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in11, 1751), right: (in4, ::core::primitive::i32::wrapping_sub(3703, 4096)), bias: 2048 }); combined },
        in4,
    );
    let Ok(mut t6) = mac_madd_shr::<11>(token, RoundedProducts { left: (in9, 1645), right: (in6, 1220), bias: 1024 });
    let Ok(mut t7) = mac_msub_shr::<11>(token, RoundedProducts { left: (in9, 1220), right: (in6, 1645), bias: 1024 });
    let mut t8 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in7, 2751), right: (in8, ::core::primitive::i32::wrapping_sub(3035, 4096)), bias: 2048 }); combined },
        in8,
    );
    let mut t9 = _mm256_add_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in7, ::core::primitive::i32::wrapping_sub(3035, 4096)), right: (in8, 2751), bias: 2048 }); combined },
        in7,
    );
    let mut t10 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in5, 2106), right: (in10, ::core::primitive::i32::wrapping_sub(3513, 4096)), bias: 2048 }); combined },
        in10,
    );
    let mut t11 = _mm256_add_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in5, ::core::primitive::i32::wrapping_sub(3513, 4096)), right: (in10, 2106), bias: 2048 }); combined },
        in5,
    );
    let mut t12 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in3, 1380), right: (in12, ::core::primitive::i32::wrapping_sub(3857, 4096)), bias: 2048 }); combined },
        in12,
    );
    let mut t13 = _mm256_add_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in3, ::core::primitive::i32::wrapping_sub(3857, 4096)), right: (in12, 1380), bias: 2048 }); combined },
        in3,
    );
    let mut t14 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in1, 601), right: (in14, ::core::primitive::i32::wrapping_sub(4052, 4096)), bias: 2048 }); combined },
        in14,
    );
    let mut t15 = _mm256_add_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in1, ::core::primitive::i32::wrapping_sub(4052, 4096)), right: (in14, 601), bias: 2048 }); combined },
        in1,
    );

    let Ok(t0a) = clip8(token, _mm256_add_epi32(t0, t8), bounds);
    let Ok(t1a) = clip8(token, _mm256_add_epi32(t1, t9), bounds);
    let Ok(t2a) = clip8(token, _mm256_add_epi32(t2, t10), bounds);
    let Ok(t3a) = clip8(token, _mm256_add_epi32(t3, t11), bounds);
    let Ok(mut t4a) = clip8(token, _mm256_add_epi32(t4, t12), bounds);
    let Ok(mut t5a) = clip8(token, _mm256_add_epi32(t5, t13), bounds);
    let Ok(mut t6a) = clip8(token, _mm256_add_epi32(t6, t14), bounds);
    let Ok(mut t7a) = clip8(token, _mm256_add_epi32(t7, t15), bounds);
    let Ok(mut t8a) = clip8(token, _mm256_sub_epi32(t0, t8), bounds);
    let Ok(mut t9a) = clip8(token, _mm256_sub_epi32(t1, t9), bounds);
    let Ok(mut t10a) = clip8(token, _mm256_sub_epi32(t2, t10), bounds);
    let Ok(mut t11a) = clip8(token, _mm256_sub_epi32(t3, t11), bounds);
    let Ok(mut t12a) = clip8(token, _mm256_sub_epi32(t4, t12), bounds);
    let Ok(mut t13a) = clip8(token, _mm256_sub_epi32(t5, t13), bounds);
    let Ok(mut t14a) = clip8(token, _mm256_sub_epi32(t6, t14), bounds);
    let Ok(mut t15a) = clip8(token, _mm256_sub_epi32(t7, t15), bounds);

    t8 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (t8a, ::core::primitive::i32::wrapping_sub(4017, 4096)), right: (t9a, 799), bias: 2048 }); combined },
        t8a,
    );
    t9 = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t8a, 799), right: (t9a, ::core::primitive::i32::wrapping_sub(4017, 4096)), bias: 2048 }); combined },
        t9a,
    );
    t10 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (t10a, 2276), right: (t11a, ::core::primitive::i32::wrapping_sub(3406, 4096)), bias: 2048 }); combined },
        t11a,
    );
    t11 = _mm256_add_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t10a, ::core::primitive::i32::wrapping_sub(3406, 4096)), right: (t11a, 2276), bias: 2048 }); combined },
        t10a,
    );
    t12 = _mm256_add_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t13a, ::core::primitive::i32::wrapping_sub(4017, 4096)), right: (t12a, 799), bias: 2048 }); combined },
        t13a,
    );
    t13 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (t13a, 799), right: (t12a, ::core::primitive::i32::wrapping_sub(4017, 4096)), bias: 2048 }); combined },
        t12a,
    );
    t14 = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t15a, 2276), right: (t14a, ::core::primitive::i32::wrapping_sub(3406, 4096)), bias: 2048 }); combined },
        t14a,
    );
    t15 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (t15a, ::core::primitive::i32::wrapping_sub(3406, 4096)), right: (t14a, 2276), bias: 2048 }); combined },
        t15a,
    );

    t0 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t0a, t4a), bounds); clipped };
    t1 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t1a, t5a), bounds); clipped };
    t2 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t2a, t6a), bounds); clipped };
    t3 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t3a, t7a), bounds); clipped };
    t4 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t0a, t4a), bounds); clipped };
    t5 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t1a, t5a), bounds); clipped };
    t6 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t2a, t6a), bounds); clipped };
    t7 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t3a, t7a), bounds); clipped };
    t8a = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t8, t12), bounds); clipped };
    t9a = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t9, t13), bounds); clipped };
    t10a = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t10, t14), bounds); clipped };
    t11a = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t11, t15), bounds); clipped };
    t12a = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t8, t12), bounds); clipped };
    t13a = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t9, t13), bounds); clipped };
    t14a = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t10, t14), bounds); clipped };
    t15a = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t11, t15), bounds); clipped };

    t4a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (t4, ::core::primitive::i32::wrapping_sub(3784, 4096)), right: (t5, 1567), bias: 2048 }); combined },
        t4,
    );
    t5a = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t4, 1567), right: (t5, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t5,
    );
    t6a = _mm256_add_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t7, ::core::primitive::i32::wrapping_sub(3784, 4096)), right: (t6, 1567), bias: 2048 }); combined },
        t7,
    );
    t7a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (t7, 1567), right: (t6, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t6,
    );
    t12 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (t12a, ::core::primitive::i32::wrapping_sub(3784, 4096)), right: (t13a, 1567), bias: 2048 }); combined },
        t12a,
    );
    t13 = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t12a, 1567), right: (t13a, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t13a,
    );
    t14 = _mm256_add_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t15a, ::core::primitive::i32::wrapping_sub(3784, 4096)), right: (t14a, 1567), bias: 2048 }); combined },
        t15a,
    );
    t15 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (t15a, 1567), right: (t14a, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t14a,
    );

    let zero = _mm256_setzero_si256();

    let Ok(out0) = clip8(token, _mm256_add_epi32(t0, t2), bounds);
    let out15 = _mm256_sub_epi32(zero, { let Ok(clipped) = clip8(token, _mm256_add_epi32(t1, t3), bounds); clipped });
    let Ok(t2a_new) = clip8(token, _mm256_sub_epi32(t0, t2), bounds);
    let Ok(t3a_new) = clip8(token, _mm256_sub_epi32(t1, t3), bounds);
    let out3 = _mm256_sub_epi32(zero, { let Ok(clipped) = clip8(token, _mm256_add_epi32(t4a, t6a), bounds); clipped });
    let Ok(out12) = clip8(token, _mm256_add_epi32(t5a, t7a), bounds);
    let Ok(t6_new) = clip8(token, _mm256_sub_epi32(t4a, t6a), bounds);
    let Ok(t7_new) = clip8(token, _mm256_sub_epi32(t5a, t7a), bounds);
    let out1 = _mm256_sub_epi32(
        zero,
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t8a, t10a), bounds); clipped },
    );
    let Ok(out14) = clip8(token, _mm256_add_epi32(t9a, t11a), bounds);
    let Ok(t10_new) = clip8(token, _mm256_sub_epi32(t8a, t10a), bounds);
    let Ok(t11_new) = clip8(token, _mm256_sub_epi32(t9a, t11a), bounds);
    let Ok(out2) = clip8(token, _mm256_add_epi32(t12, t14), bounds);
    let out13 = _mm256_sub_epi32(zero, { let Ok(clipped) = clip8(token, _mm256_add_epi32(t13, t15), bounds); clipped });
    let Ok(t14_new) = clip8(token, _mm256_sub_epi32(t12, t14), bounds);
    let Ok(t15_new) = clip8(token, _mm256_sub_epi32(t13, t15), bounds);


    let mul181_sum = |a: __m256i, b: __m256i| {
        let s = _mm256_add_epi32(a, b);
        _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(s, _mm256_set1_epi32(181)),
            _mm256_set1_epi32(128),
        ))
    };
    let mul181_diff = |a: __m256i, b: __m256i| {
        let s = _mm256_sub_epi32(a, b);
        _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(s, _mm256_set1_epi32(181)),
            _mm256_set1_epi32(128),
        ))
    };

    let out7 = _mm256_sub_epi32(zero, mul181_sum(t2a_new, t3a_new));
    let out8 = mul181_diff(t2a_new, t3a_new);
    let out4 = mul181_sum(t6_new, t7_new);
    let out11 = _mm256_sub_epi32(zero, mul181_diff(t6_new, t7_new));
    let out6 = mul181_sum(t10_new, t11_new);
    let out9 = _mm256_sub_epi32(zero, mul181_diff(t10_new, t11_new));
    let out5 = _mm256_sub_epi32(zero, mul181_sum(t14_new, t15_new));
    let out10 = mul181_diff(t14_new, t15_new);

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = out0;
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = out1;
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = out2;
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out3;
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out4;
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out5;
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out6;
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out7;
    *{ let [_, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out8;
    *{ let [_, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out9;
    *{ let [_, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out10;
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out11;
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out12;
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out13;
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out14;
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = out15;
    Ok(())
}



#[cfg(target_arch = "x86_64")]
const SQUARE_16: console_core_geometry::Size<u32> = console_core_geometry::Size {
  width: 16,
  height: 16,
};

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
enum Square16Columns {
  Dct,
  Adst,
  FlipAdst,
  Identity,
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn identity16_avx2_columns(_token: Desktop64, coefficients: &mut [i32]) -> Result<(), Never> {
  let cosine = _mm256_set1_epi32(1697);
  let rounding = _mm256_set1_epi32(1024);

  for lanes in coefficients.as_chunks_mut::<8>().0 {
    let values = loadu_256!(&*lanes);
    let doubled = _mm256_slli_epi32::<1>(values);
    let weighted = _mm256_mullo_epi32(values, cosine);
    let shifted = _mm256_srai_epi32::<11>(_mm256_add_epi32(weighted, rounding));
    storeu_256!(lanes, _mm256_add_epi32(doubled, shifted));
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn identity_shift_avx2_columns<const SHIFT: i32>(
  _token: Desktop64,
  coefficients: &mut [i32],
) -> Result<(), Never> {
  for lanes in coefficients.as_chunks_mut::<8>().0 {
    let values = loadu_256!(&*lanes);
    storeu_256!(lanes, _mm256_slli_epi32::<SHIFT>(values));
  }

  Ok(())
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn transform_square16_columns(
  token: Desktop64,
  scratch: &mut [i32; 256],
  transform: Square16Columns,
  bounds: CoefficientBounds,
) -> Rav1dResult {
  let Ok(wide) = crate::src::cpu::summon_avx512();
  let wide_result = match (wide, transform) {
    (Some(server), Square16Columns::Dct) => Some(dct16_cols_avx512(server, scratch, SQUARE_16, bounds)),
    (Some(server), Square16Columns::Identity) => Some(identity16_cols_avx512(server, scratch, SQUARE_16)),
    _ => None,
  };

  match wide_result {
    Some(Ok(())) => return Ok(()),
    Some(Err(error)) => console_core_logging::debug!("AV1 column transform scratch: {error:?}"),
    None => {}
  }

  match transform {
    Square16Columns::Identity => {
      let Ok(()) = identity16_avx2_columns(token, scratch);
      Ok(())
    }
    Square16Columns::Dct | Square16Columns::Adst | Square16Columns::FlipAdst => {
      let limits = VectorBounds {
        lower: _mm256_set1_epi32(bounds.lower),
        upper: _mm256_set1_epi32(bounds.upper),
      };
      apply_transform_columns_avx2(token, scratch, SQUARE_16, limits, |desktop, lanes, clipping| {
        match transform {
          Square16Columns::Dct => dct16_1d_cols8(desktop, lanes, clipping),
          Square16Columns::Adst => adst16_1d_cols8(desktop, lanes, clipping),
          Square16Columns::FlipAdst => {
            let Ok(()) = adst16_1d_cols8(desktop, lanes, clipping);
            lanes.reverse();
            Ok(())
          }
          Square16Columns::Identity => Ok(()),
        }
      })
    }
  }
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn dct32_1d_cols8(token: Desktop64, c: &mut [__m256i; 32], bounds: VectorBounds<__m256i>) -> Result<(), Never> {

    let [
        even0, in1, even2, in3, even4, in5, even6, in7,
        even8, in9, even10, in11, even12, in13, even14, in15,
        even16, in17, even18, in19, even20, in21, even22, in23,
        even24, in25, even26, in27, even28, in29, even30, in31,
    ] = *c;
    let mut even = [
        even0, even2, even4, even6, even8, even10, even12, even14,
        even16, even18, even20, even22, even24, even26, even28, even30,
    ];
    let Ok(()) = dct16_1d_cols8(token, &mut even, bounds);

    let t16a = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in1, 201), right: (in31, ::core::primitive::i32::wrapping_sub(4091, 4096)), bias: 2048 }); combined },
        in31,
    );
    let t17a = _mm256_add_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in17, ::core::primitive::i32::wrapping_sub(3035, 4096)), right: (in15, 2751), bias: 2048 }); combined },
        in17,
    );
    let t18a = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in9, 1751), right: (in23, ::core::primitive::i32::wrapping_sub(3703, 4096)), bias: 2048 }); combined },
        in23,
    );
    let t19a = _mm256_add_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in25, ::core::primitive::i32::wrapping_sub(3857, 4096)), right: (in7, 1380), bias: 2048 }); combined },
        in25,
    );
    let t20a = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in5, 995), right: (in27, ::core::primitive::i32::wrapping_sub(3973, 4096)), bias: 2048 }); combined },
        in27,
    );
    let t21a = _mm256_add_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in21, ::core::primitive::i32::wrapping_sub(3513, 4096)), right: (in11, 2106), bias: 2048 }); combined },
        in21,
    );
    let Ok(t22a) = mac_msub_shr::<11>(token, RoundedProducts { left: (in13, 1220), right: (in19, 1645), bias: 1024 });
    let t23a = _mm256_add_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (in29, ::core::primitive::i32::wrapping_sub(4052, 4096)), right: (in3, 601), bias: 2048 }); combined },
        in29,
    );
    let t24a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in29, 601), right: (in3, ::core::primitive::i32::wrapping_sub(4052, 4096)), bias: 2048 }); combined },
        in3,
    );
    let Ok(t25a) = mac_madd_shr::<11>(token, RoundedProducts { left: (in13, 1645), right: (in19, 1220), bias: 1024 });
    let t26a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in21, 2106), right: (in11, ::core::primitive::i32::wrapping_sub(3513, 4096)), bias: 2048 }); combined },
        in11,
    );
    let t27a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in5, ::core::primitive::i32::wrapping_sub(3973, 4096)), right: (in27, 995), bias: 2048 }); combined },
        in5,
    );
    let t28a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in25, 1380), right: (in7, ::core::primitive::i32::wrapping_sub(3857, 4096)), bias: 2048 }); combined },
        in7,
    );
    let t29a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in9, ::core::primitive::i32::wrapping_sub(3703, 4096)), right: (in23, 1751), bias: 2048 }); combined },
        in9,
    );
    let t30a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in17, 2751), right: (in15, ::core::primitive::i32::wrapping_sub(3035, 4096)), bias: 2048 }); combined },
        in15,
    );
    let t31a = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (in1, ::core::primitive::i32::wrapping_sub(4091, 4096)), right: (in31, 201), bias: 2048 }); combined },
        in1,
    );

    let Ok(mut t16) = clip8(token, _mm256_add_epi32(t16a, t17a), bounds);
    let Ok(mut t17) = clip8(token, _mm256_sub_epi32(t16a, t17a), bounds);
    let Ok(mut t18) = clip8(token, _mm256_sub_epi32(t19a, t18a), bounds);
    let Ok(t19) = clip8(token, _mm256_add_epi32(t19a, t18a), bounds);
    let Ok(t20) = clip8(token, _mm256_add_epi32(t20a, t21a), bounds);
    let Ok(mut t21) = clip8(token, _mm256_sub_epi32(t20a, t21a), bounds);
    let Ok(mut t22) = clip8(token, _mm256_sub_epi32(t23a, t22a), bounds);
    let Ok(mut t23) = clip8(token, _mm256_add_epi32(t23a, t22a), bounds);
    let Ok(mut t24) = clip8(token, _mm256_add_epi32(t24a, t25a), bounds);
    let Ok(mut t25) = clip8(token, _mm256_sub_epi32(t24a, t25a), bounds);
    let Ok(mut t26) = clip8(token, _mm256_sub_epi32(t27a, t26a), bounds);
    let Ok(t27) = clip8(token, _mm256_add_epi32(t27a, t26a), bounds);
    let Ok(t28) = clip8(token, _mm256_add_epi32(t28a, t29a), bounds);
    let Ok(mut t29) = clip8(token, _mm256_sub_epi32(t28a, t29a), bounds);
    let Ok(mut t30) = clip8(token, _mm256_sub_epi32(t31a, t30a), bounds);
    let Ok(mut t31) = clip8(token, _mm256_add_epi32(t31a, t30a), bounds);

    let t17a_2 = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t30, 799), right: (t17, ::core::primitive::i32::wrapping_sub(4017, 4096)), bias: 2048 }); combined },
        t17,
    );
    let t30a_2 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (t30, ::core::primitive::i32::wrapping_sub(4017, 4096)), right: (t17, 799), bias: 2048 }); combined },
        t30,
    );

    let t18a_inner = _mm256_add_epi32(
        _mm256_mullo_epi32(t29, _mm256_set1_epi32(::core::primitive::i32::wrapping_sub(4017, 4096))),
        _mm256_mullo_epi32(t18, _mm256_set1_epi32(799)),
    );
    let t18a_2 = _mm256_sub_epi32(
        _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(_mm256_setzero_si256(), t18a_inner),
            _mm256_set1_epi32(2048),
        )),
        t29,
    );
    let t29a_2 = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t29, 799), right: (t18, ::core::primitive::i32::wrapping_sub(4017, 4096)), bias: 2048 }); combined },
        t18,
    );
    let Ok(t21a_2) = mac_msub_shr::<11>(token, RoundedProducts { left: (t26, 1703), right: (t21, 1138), bias: 1024 });
    let Ok(t26a_2) = mac_madd_shr::<11>(token, RoundedProducts { left: (t26, 1138), right: (t21, 1703), bias: 1024 });

    let t22a_inner = _mm256_add_epi32(
        _mm256_mullo_epi32(t25, _mm256_set1_epi32(1138)),
        _mm256_mullo_epi32(t22, _mm256_set1_epi32(1703)),
    );
    let t22a_2 = _mm256_srai_epi32::<11>(_mm256_add_epi32(
        _mm256_sub_epi32(_mm256_setzero_si256(), t22a_inner),
        _mm256_set1_epi32(1024),
    ));
    let Ok(t25a_2) = mac_msub_shr::<11>(token, RoundedProducts { left: (t25, 1703), right: (t22, 1138), bias: 1024 });

    let Ok(t16a_2) = clip8(token, _mm256_add_epi32(t16, t19), bounds);
    t17 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t17a_2, t18a_2), bounds); clipped };
    t18 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t17a_2, t18a_2), bounds); clipped };
    let Ok(t19a_2) = clip8(token, _mm256_sub_epi32(t16, t19), bounds);
    let Ok(t20a_2) = clip8(token, _mm256_sub_epi32(t23, t20), bounds);
    t21 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t22a_2, t21a_2), bounds); clipped };
    t22 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t22a_2, t21a_2), bounds); clipped };
    let Ok(t23a_2) = clip8(token, _mm256_add_epi32(t23, t20), bounds);
    let Ok(t24a_2) = clip8(token, _mm256_add_epi32(t24, t27), bounds);
    t25 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t25a_2, t26a_2), bounds); clipped };
    t26 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t25a_2, t26a_2), bounds); clipped };
    let Ok(t27a_2) = clip8(token, _mm256_sub_epi32(t24, t27), bounds);
    let Ok(t28a_2) = clip8(token, _mm256_sub_epi32(t31, t28), bounds);
    t29 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t30a_2, t29a_2), bounds); clipped };
    t30 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t30a_2, t29a_2), bounds); clipped };
    let Ok(t31a_2) = clip8(token, _mm256_add_epi32(t31, t28), bounds);

    let t18a_3 = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t29, 1567), right: (t18, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t18,
    );
    let t29a_3 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (t29, ::core::primitive::i32::wrapping_sub(3784, 4096)), right: (t18, 1567), bias: 2048 }); combined },
        t29,
    );
    let t19_2 = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t28a_2, 1567), right: (t19a_2, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t19a_2,
    );
    let t28_2 = _mm256_add_epi32(
        { let Ok(combined) = mac_madd_shr::<12>(token, RoundedProducts { left: (t28a_2, ::core::primitive::i32::wrapping_sub(3784, 4096)), right: (t19a_2, 1567), bias: 2048 }); combined },
        t28a_2,
    );

    let t20_inner = _mm256_add_epi32(
        _mm256_mullo_epi32(t27a_2, _mm256_set1_epi32(::core::primitive::i32::wrapping_sub(3784, 4096))),
        _mm256_mullo_epi32(t20a_2, _mm256_set1_epi32(1567)),
    );
    let t20_2 = _mm256_sub_epi32(
        _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(_mm256_setzero_si256(), t20_inner),
            _mm256_set1_epi32(2048),
        )),
        t27a_2,
    );
    let t27_2 = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t27a_2, 1567), right: (t20a_2, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t20a_2,
    );

    let t21a_inner = _mm256_add_epi32(
        _mm256_mullo_epi32(t26, _mm256_set1_epi32(::core::primitive::i32::wrapping_sub(3784, 4096))),
        _mm256_mullo_epi32(t21, _mm256_set1_epi32(1567)),
    );
    let t21a_3 = _mm256_sub_epi32(
        _mm256_srai_epi32::<12>(_mm256_add_epi32(
            _mm256_sub_epi32(_mm256_setzero_si256(), t21a_inner),
            _mm256_set1_epi32(2048),
        )),
        t26,
    );
    let t26a_3 = _mm256_sub_epi32(
        { let Ok(combined) = mac_msub_shr::<12>(token, RoundedProducts { left: (t26, 1567), right: (t21, ::core::primitive::i32::wrapping_sub(3784, 4096)), bias: 2048 }); combined },
        t21,
    );

    t16 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t16a_2, t23a_2), bounds); clipped };
    let Ok(t17a_3) = clip8(token, _mm256_add_epi32(t17, t22), bounds);
    t18 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t18a_3, t21a_3), bounds); clipped };
    let Ok(t19a_3) = clip8(token, _mm256_add_epi32(t19_2, t20_2), bounds);
    let Ok(t20a_3) = clip8(token, _mm256_sub_epi32(t19_2, t20_2), bounds);
    t21 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t18a_3, t21a_3), bounds); clipped };
    let Ok(t22a_3) = clip8(token, _mm256_sub_epi32(t17, t22), bounds);
    t23 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t16a_2, t23a_2), bounds); clipped };
    t24 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t31a_2, t24a_2), bounds); clipped };
    let Ok(t25a_3) = clip8(token, _mm256_sub_epi32(t30, t25), bounds);
    t26 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t29a_3, t26a_3), bounds); clipped };
    let Ok(t27a_3) = clip8(token, _mm256_sub_epi32(t28_2, t27_2), bounds);
    let Ok(t28a_3) = clip8(token, _mm256_add_epi32(t28_2, t27_2), bounds);
    t29 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t29a_3, t26a_3), bounds); clipped };
    let Ok(t30a_3) = clip8(token, _mm256_add_epi32(t30, t25), bounds);
    t31 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t31a_2, t24a_2), bounds); clipped };

    let mul181 = |a: __m256i, b: __m256i| -> __m256i {
        let s = _mm256_add_epi32(a, b);
        _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(s, _mm256_set1_epi32(181)),
            _mm256_set1_epi32(128),
        ))
    };
    let mul181_neg = |a: __m256i, b: __m256i| -> __m256i {
        let s = _mm256_sub_epi32(a, b);
        _mm256_srai_epi32::<8>(_mm256_add_epi32(
            _mm256_mullo_epi32(s, _mm256_set1_epi32(181)),
            _mm256_set1_epi32(128),
        ))
    };
    let t20_final = mul181_neg(t27a_3, t20a_3);
    let t27_final = mul181(t27a_3, t20a_3);
    let t21a_final = mul181_neg(t26, t21);
    let t26a_final = mul181(t26, t21);
    let t22_final = mul181_neg(t25a_3, t22a_3);
    let t25_final = mul181(t25a_3, t22a_3);
    let t23a_3 = mul181_neg(t24, t23);
    let t24a_3 = mul181(t24, t23);

    let [t0, t1, t2, t3, t4, t5, t6, t7, t8, t9, t10, t11, t12, t13, t14, t15] = even;

    *c = [
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t0, t31), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t1, t30a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t2, t29), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t3, t28a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t4, t27_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t5, t26a_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t6, t25_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t7, t24a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t8, t23a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t9, t22_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t10, t21a_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t11, t20_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t12, t19a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t13, t18), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t14, t17a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t15, t16), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t15, t16), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t14, t17a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t13, t18), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t12, t19a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t11, t20_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t10, t21a_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t9, t22_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t8, t23a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t7, t24a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t6, t25_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t5, t26a_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t4, t27_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t3, t28a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t2, t29), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t1, t30a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t0, t31), bounds); clipped },
    ];
    Ok(())
}






















#[cfg(target_arch = "x86_64")]
#[rite]
#[inline(always)]
#[doc = "Packs interleaved i32 inputs into i16 lanes with saturation in every build profile."]
fn i32_to_i16_pair(_token: Desktop64, a: __m256i, b: __m256i) -> Result<__m256i, Never> {
    let a_i16 = _mm_packs_epi32(_mm256_castsi256_si128(a), _mm256_extracti128_si256(a, 1));
    let b_i16 = _mm_packs_epi32(_mm256_castsi256_si128(b), _mm256_extracti128_si256(b, 1));
    Ok(_mm256_set_m128i(
        _mm_unpackhi_epi16(a_i16, b_i16),
        _mm_unpacklo_epi16(a_i16, b_i16),
    ))
}



#[cfg(target_arch = "x86_64")]
#[rite]
fn dct4_1d_cols8_i16(token: Desktop64, c: &mut [__m256i; 4], bounds: VectorBounds<__m256i>) -> Result<(), Never> {
    let pd_2048 = _mm256_set1_epi32(2048);
    let pd_128 = _mm256_set1_epi32(128);




    let Ok(pair_02) = i32_to_i16_pair(token, match c { &mut [_explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _explicit_element, ..] => _explicit_element });
    let t0 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_02, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
        pd_128,
    ));
    let t1 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_02, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
        pd_128,
    ));



    let Ok(pair_13) = i32_to_i16_pair(token, match c { &mut [_, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _explicit_element, ..] => _explicit_element });
    let t2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1567, high: -3784 }); packed_vector }),
        pd_2048,
    ));
    let t3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 3784, high: 1567 }); packed_vector }),
        pd_2048,
    ));

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t0, t3), bounds); clipped };
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t1, t2), bounds); clipped };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t1, t2), bounds); clipped };
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t0, t3), bounds); clipped };
    Ok(())
}



#[cfg(target_arch = "x86_64")]
#[rite]
fn dct8_1d_cols8_i16(token: Desktop64, c: &mut [__m256i; 8], bounds: VectorBounds<__m256i>) -> Result<(), Never> {

    let mut even = [match c { &mut [_explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }];
    let Ok(()) = dct4_1d_cols8_i16(token, &mut even, bounds);
    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = match even { [_explicit_element, ..] => _explicit_element };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _explicit_element, ..] => _explicit_element };

    let pd_2048 = _mm256_set1_epi32(2048);
    let pd_128 = _mm256_set1_epi32(128);




    let Ok(pair_17) = i32_to_i16_pair(token, match c { &mut [_, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
    let t4a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_17, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 799, high: -4017 }); packed_vector }),
        pd_2048,
    ));
    let t7a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_17, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 4017, high: 799 }); packed_vector }),
        pd_2048,
    ));



    let Ok(pair_53) = i32_to_i16_pair(token, match c { &mut [_, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _explicit_element, ..] => _explicit_element });
    let t5a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_53, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 3406, high: -2276 }); packed_vector }),
        pd_2048,
    ));
    let t6a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_53, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 2276, high: 3406 }); packed_vector }),
        pd_2048,
    ));

    let Ok(t4) = clip8(token, _mm256_add_epi32(t4a, t5a), bounds);
    let Ok(t5a_n) = clip8(token, _mm256_sub_epi32(t4a, t5a), bounds);
    let Ok(t7) = clip8(token, _mm256_add_epi32(t7a, t6a), bounds);
    let Ok(t6a_n) = clip8(token, _mm256_sub_epi32(t7a, t6a), bounds);




    let Ok(pair_65) = i32_to_i16_pair(token, t6a_n, t5a_n);
    let t5 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_65, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
        pd_128,
    ));
    let t6 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_65, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
        pd_128,
    ));

    let &mut [t0, ..] = c;
    let &mut [_, _, t1, ..] = c;
    let &mut [_, _, _, _, t2, ..] = c;
    let &mut [_, _, _, _, _, _, t3, ..] = c;

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t0, t7), bounds); clipped };
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t1, t6), bounds); clipped };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t2, t5), bounds); clipped };
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t3, t4), bounds); clipped };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t3, t4), bounds); clipped };
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t2, t5), bounds); clipped };
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t1, t6), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t0, t7), bounds); clipped };
    Ok(())
}



#[cfg(target_arch = "x86_64")]
#[rite]
fn dct16_1d_cols8_i16(token: Desktop64, c: &mut [__m256i; 16], bounds: VectorBounds<__m256i>) -> Result<(), Never> {

    let mut even = [match c { &mut [_explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }];
    let Ok(()) = dct8_1d_cols8_i16(token, &mut even, bounds);
    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = match even { [_explicit_element, ..] => _explicit_element };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element };

    let pd_2048 = _mm256_set1_epi32(2048);
    let pd_128 = _mm256_set1_epi32(128);






    let Ok(pair_1_15) = i32_to_i16_pair(token, match c { &mut [_, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
    let t8a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_1_15, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 401, high: -4076 }); packed_vector }),
        pd_2048,
    ));
    let t15a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_1_15, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 4076, high: 401 }); packed_vector }),
        pd_2048,
    ));



    let Ok(pair_9_7) = i32_to_i16_pair(token, match c { &mut [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
    let t9a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_9_7, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 3166, high: -2598 }); packed_vector }),
        pd_2048,
    ));
    let t14a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_9_7, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 2598, high: 3166 }); packed_vector }),
        pd_2048,
    ));



    let Ok(pair_5_11) = i32_to_i16_pair(token, match c { &mut [_, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
    let t10a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_5_11, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1931, high: -3612 }); packed_vector }),
        pd_2048,
    ));
    let t13a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_5_11, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 3612, high: 1931 }); packed_vector }),
        pd_2048,
    ));



    let Ok(pair_13_3) = i32_to_i16_pair(token, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _explicit_element, ..] => _explicit_element });
    let t11a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_3, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 3920, high: -1189 }); packed_vector }),
        pd_2048,
    ));
    let t12a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_3, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1189, high: 3920 }); packed_vector }),
        pd_2048,
    ));

    let Ok(t8) = clip8(token, _mm256_add_epi32(t8a, t9a), bounds);
    let Ok(mut t9) = clip8(token, _mm256_sub_epi32(t8a, t9a), bounds);
    let Ok(mut t10) = clip8(token, _mm256_sub_epi32(t11a, t10a), bounds);
    let Ok(mut t11) = clip8(token, _mm256_add_epi32(t11a, t10a), bounds);
    let Ok(mut t12) = clip8(token, _mm256_add_epi32(t12a, t13a), bounds);
    let Ok(mut t13) = clip8(token, _mm256_sub_epi32(t12a, t13a), bounds);
    let Ok(mut t14) = clip8(token, _mm256_sub_epi32(t15a, t14a), bounds);
    let Ok(t15) = clip8(token, _mm256_add_epi32(t15a, t14a), bounds);




    let Ok(pair_14_9) = i32_to_i16_pair(token, t14, t9);
    let t9a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_14_9, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1567, high: -3784 }); packed_vector }),
        pd_2048,
    ));
    let t14a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_14_9, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 3784, high: 1567 }); packed_vector }),
        pd_2048,
    ));
























    let Ok(pair_13_10) = i32_to_i16_pair(token, t13, t10);
    let t10a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_10, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: -3784, high: -1567 }); packed_vector }),
        pd_2048,
    ));







    let t13a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_10, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1567, high: -3784 }); packed_vector }),
        pd_2048,
    ));

    let Ok(t8a_2) = clip8(token, _mm256_add_epi32(t8, t11), bounds);
    t9 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t9a_2, t10a_2), bounds); clipped };
    t10 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t9a_2, t10a_2), bounds); clipped };
    let Ok(t11a_2) = clip8(token, _mm256_sub_epi32(t8, t11), bounds);
    let Ok(t12a_2) = clip8(token, _mm256_sub_epi32(t15, t12), bounds);
    t13 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t14a_2, t13a_2), bounds); clipped };
    t14 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t14a_2, t13a_2), bounds); clipped };
    let Ok(t15a_2) = clip8(token, _mm256_add_epi32(t15, t12), bounds);




    let Ok(pair_13_10_f) = i32_to_i16_pair(token, t13, t10);
    let t10a_new = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_10_f, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
        pd_128,
    ));
    let t13a_new = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_10_f, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
        pd_128,
    ));
    let Ok(pair_12a_11a) = i32_to_i16_pair(token, t12a_2, t11a_2);
    t11 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_12a_11a, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
        pd_128,
    ));
    t12 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_12a_11a, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
        pd_128,
    ));

    let &mut [t0, ..] = c;
    let &mut [_, _, t1, ..] = c;
    let &mut [_, _, _, _, t2, ..] = c;
    let &mut [_, _, _, _, _, _, t3, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, t4, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, t5, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, _, t6, ..] = c;
    let &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _, t7, ..] = c;

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t0, t15a_2), bounds); clipped };
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t1, t14), bounds); clipped };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t2, t13a_new), bounds); clipped };
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t3, t12), bounds); clipped };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t4, t11), bounds); clipped };
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t5, t10a_new), bounds); clipped };
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t6, t9), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t7, t8a_2), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t7, t8a_2), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t6, t9), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t5, t10a_new), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t4, t11), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t3, t12), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t2, t13a_new), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t1, t14), bounds); clipped };
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t0, t15a_2), bounds); clipped };
    Ok(())
}










#[cfg(target_arch = "x86_64")]
#[rite]
fn dct32_1d_cols8_i16(token: Desktop64, c: &mut [__m256i; 32], bounds: VectorBounds<__m256i>) -> Result<(), Never> {

    let [
        even0, in1, even2, in3, even4, in5, even6, in7,
        even8, in9, even10, in11, even12, in13, even14, in15,
        even16, in17, even18, in19, even20, in21, even22, in23,
        even24, in25, even26, in27, even28, in29, even30, in31,
    ] = *c;
    let mut even = [
        even0, even2, even4, even6, even8, even10, even12, even14,
        even16, even18, even20, even22, even24, even26, even28, even30,
    ];
    let Ok(()) = dct16_1d_cols8_i16(token, &mut even, bounds);

    let pd_2048 = _mm256_set1_epi32(2048);
    let pd_1024 = _mm256_set1_epi32(1024);
    let pd_128 = _mm256_set1_epi32(128);

    let Ok(pair_1_31) = i32_to_i16_pair(token, in1, in31);
    let t16a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_1_31, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 201, high: -4091 }); packed_vector }),
        pd_2048,
    ));
    let t31a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_1_31, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 4091, high: 201 }); packed_vector }),
        pd_2048,
    ));

    let Ok(pair_17_15) = i32_to_i16_pair(token, in17, in15);
    let t17a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_17_15, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 3035, high: -2751 }); packed_vector }),
        pd_2048,
    ));
    let t30a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_17_15, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 2751, high: 3035 }); packed_vector }),
        pd_2048,
    ));

    let Ok(pair_9_23) = i32_to_i16_pair(token, in9, in23);
    let t18a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_9_23, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1751, high: -3703 }); packed_vector }),
        pd_2048,
    ));
    let t29a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_9_23, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 3703, high: 1751 }); packed_vector }),
        pd_2048,
    ));

    let Ok(pair_25_7) = i32_to_i16_pair(token, in25, in7);
    let t19a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_25_7, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 3857, high: -1380 }); packed_vector }),
        pd_2048,
    ));
    let t28a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_25_7, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1380, high: 3857 }); packed_vector }),
        pd_2048,
    ));

    let Ok(pair_5_27) = i32_to_i16_pair(token, in5, in27);
    let t20a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_5_27, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 995, high: -3973 }); packed_vector }),
        pd_2048,
    ));
    let t27a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_5_27, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 3973, high: 995 }); packed_vector }),
        pd_2048,
    ));

    let Ok(pair_21_11) = i32_to_i16_pair(token, in21, in11);
    let t21a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_21_11, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 3513, high: -2106 }); packed_vector }),
        pd_2048,
    ));
    let t26a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_21_11, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 2106, high: 3513 }); packed_vector }),
        pd_2048,
    ));

    let Ok(pair_13_19) = i32_to_i16_pair(token, in13, in19);
    let t22a = _mm256_srai_epi32::<11>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_19, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1220, high: -1645 }); packed_vector }),
        pd_1024,
    ));
    let t25a = _mm256_srai_epi32::<11>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_19, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1645, high: 1220 }); packed_vector }),
        pd_1024,
    ));

    let Ok(pair_29_3) = i32_to_i16_pair(token, in29, in3);
    let t23a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_29_3, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 4052, high: -601 }); packed_vector }),
        pd_2048,
    ));
    let t24a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_29_3, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 601, high: 4052 }); packed_vector }),
        pd_2048,
    ));

    let Ok(mut t16) = clip8(token, _mm256_add_epi32(t16a, t17a), bounds);
    let Ok(mut t17) = clip8(token, _mm256_sub_epi32(t16a, t17a), bounds);
    let Ok(mut t18) = clip8(token, _mm256_sub_epi32(t19a, t18a), bounds);
    let Ok(t19) = clip8(token, _mm256_add_epi32(t19a, t18a), bounds);
    let Ok(t20) = clip8(token, _mm256_add_epi32(t20a, t21a), bounds);
    let Ok(mut t21) = clip8(token, _mm256_sub_epi32(t20a, t21a), bounds);
    let Ok(mut t22) = clip8(token, _mm256_sub_epi32(t23a, t22a), bounds);
    let Ok(mut t23) = clip8(token, _mm256_add_epi32(t23a, t22a), bounds);
    let Ok(mut t24) = clip8(token, _mm256_add_epi32(t24a, t25a), bounds);
    let Ok(mut t25) = clip8(token, _mm256_sub_epi32(t24a, t25a), bounds);
    let Ok(mut t26) = clip8(token, _mm256_sub_epi32(t27a, t26a), bounds);
    let Ok(t27) = clip8(token, _mm256_add_epi32(t27a, t26a), bounds);
    let Ok(t28) = clip8(token, _mm256_add_epi32(t28a, t29a), bounds);
    let Ok(mut t29) = clip8(token, _mm256_sub_epi32(t28a, t29a), bounds);
    let Ok(mut t30) = clip8(token, _mm256_sub_epi32(t31a, t30a), bounds);
    let Ok(mut t31) = clip8(token, _mm256_add_epi32(t31a, t30a), bounds);

    let Ok(pair_30_17) = i32_to_i16_pair(token, t30, t17);
    let t17a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_30_17, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 799, high: -4017 }); packed_vector }),
        pd_2048,
    ));
    let t30a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_30_17, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 4017, high: 799 }); packed_vector }),
        pd_2048,
    ));

    let Ok(pair_29_18) = i32_to_i16_pair(token, t29, t18);
    let t18a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_29_18, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: -4017, high: -799 }); packed_vector }),
        pd_2048,
    ));
    let t29a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_29_18, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 799, high: -4017 }); packed_vector }),
        pd_2048,
    ));

    let Ok(pair_26_21) = i32_to_i16_pair(token, t26, t21);
    let t21a_2 = _mm256_srai_epi32::<11>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26_21, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1703, high: -1138 }); packed_vector }),
        pd_1024,
    ));
    let t26a_2 = _mm256_srai_epi32::<11>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26_21, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1138, high: 1703 }); packed_vector }),
        pd_1024,
    ));

    let Ok(pair_25_22) = i32_to_i16_pair(token, t25, t22);
    let t22a_2 = _mm256_srai_epi32::<11>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_25_22, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: -1138, high: -1703 }); packed_vector }),
        pd_1024,
    ));
    let t25a_2 = _mm256_srai_epi32::<11>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_25_22, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1703, high: -1138 }); packed_vector }),
        pd_1024,
    ));

    let Ok(t16a_2) = clip8(token, _mm256_add_epi32(t16, t19), bounds);
    t17 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t17a_2, t18a_2), bounds); clipped };
    t18 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t17a_2, t18a_2), bounds); clipped };
    let Ok(t19a_2) = clip8(token, _mm256_sub_epi32(t16, t19), bounds);
    let Ok(t20a_2) = clip8(token, _mm256_sub_epi32(t23, t20), bounds);
    t21 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t22a_2, t21a_2), bounds); clipped };
    t22 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t22a_2, t21a_2), bounds); clipped };
    let Ok(t23a_2) = clip8(token, _mm256_add_epi32(t23, t20), bounds);
    let Ok(t24a_2) = clip8(token, _mm256_add_epi32(t24, t27), bounds);
    t25 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t25a_2, t26a_2), bounds); clipped };
    t26 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t25a_2, t26a_2), bounds); clipped };
    let Ok(t27a_2) = clip8(token, _mm256_sub_epi32(t24, t27), bounds);
    let Ok(t28a_2) = clip8(token, _mm256_sub_epi32(t31, t28), bounds);
    t29 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t30a_2, t29a_2), bounds); clipped };
    t30 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t30a_2, t29a_2), bounds); clipped };
    let Ok(t31a_2) = clip8(token, _mm256_add_epi32(t31, t28), bounds);

    let Ok(pair_29_18_2) = i32_to_i16_pair(token, t29, t18);
    let t18a_3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_29_18_2, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1567, high: -3784 }); packed_vector }),
        pd_2048,
    ));
    let t29a_3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_29_18_2, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 3784, high: 1567 }); packed_vector }),
        pd_2048,
    ));

    let Ok(pair_28a_19a) = i32_to_i16_pair(token, t28a_2, t19a_2);
    let t19_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_28a_19a, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1567, high: -3784 }); packed_vector }),
        pd_2048,
    ));
    let t28_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_28a_19a, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 3784, high: 1567 }); packed_vector }),
        pd_2048,
    ));

    let Ok(pair_27a_20a) = i32_to_i16_pair(token, t27a_2, t20a_2);
    let t20_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_27a_20a, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: -3784, high: -1567 }); packed_vector }),
        pd_2048,
    ));
    let t27_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_27a_20a, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1567, high: -3784 }); packed_vector }),
        pd_2048,
    ));

    let Ok(pair_26_21_2) = i32_to_i16_pair(token, t26, t21);
    let t21a_3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26_21_2, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: -3784, high: -1567 }); packed_vector }),
        pd_2048,
    ));
    let t26a_3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26_21_2, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 1567, high: -3784 }); packed_vector }),
        pd_2048,
    ));

    t16 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t16a_2, t23a_2), bounds); clipped };
    let Ok(t17a_3) = clip8(token, _mm256_add_epi32(t17, t22), bounds);
    t18 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t18a_3, t21a_3), bounds); clipped };
    let Ok(t19a_3) = clip8(token, _mm256_add_epi32(t19_2, t20_2), bounds);
    let Ok(t20a_3) = clip8(token, _mm256_sub_epi32(t19_2, t20_2), bounds);
    t21 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t18a_3, t21a_3), bounds); clipped };
    let Ok(t22a_3) = clip8(token, _mm256_sub_epi32(t17, t22), bounds);
    t23 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t16a_2, t23a_2), bounds); clipped };
    t24 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t31a_2, t24a_2), bounds); clipped };
    let Ok(t25a_3) = clip8(token, _mm256_sub_epi32(t30, t25), bounds);
    t26 = { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t29a_3, t26a_3), bounds); clipped };
    let Ok(t27a_3) = clip8(token, _mm256_sub_epi32(t28_2, t27_2), bounds);
    let Ok(t28a_3) = clip8(token, _mm256_add_epi32(t28_2, t27_2), bounds);
    t29 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t29a_3, t26a_3), bounds); clipped };
    let Ok(t30a_3) = clip8(token, _mm256_add_epi32(t30, t25), bounds);
    t31 = { let Ok(clipped) = clip8(token, _mm256_add_epi32(t31a_2, t24a_2), bounds); clipped };

    let Ok(pair_27a_20a_2) = i32_to_i16_pair(token, t27a_3, t20a_3);
    let t20_final = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_27a_20a_2, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
        pd_128,
    ));
    let t27_final = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_27a_20a_2, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
        pd_128,
    ));

    let Ok(pair_26_21_3) = i32_to_i16_pair(token, t26, t21);
    let t21a_final = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26_21_3, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
        pd_128,
    ));
    let t26a_final = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26_21_3, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
        pd_128,
    ));

    let Ok(pair_25a_22a) = i32_to_i16_pair(token, t25a_3, t22a_3);
    let t22_final = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_25a_22a, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
        pd_128,
    ));
    let t25_final = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_25a_22a, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
        pd_128,
    ));

    let Ok(pair_24_23) = i32_to_i16_pair(token, t24, t23);
    let t23a_3 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_24_23, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: -181 }); packed_vector }),
        pd_128,
    ));
    let t24a_3 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_24_23, { let Ok(packed_vector) = dct8_row_coef_pack(token, CoefficientPair { low: 181, high: 181 }); packed_vector }),
        pd_128,
    ));

    let [t0, t1, t2, t3, t4, t5, t6, t7, t8, t9, t10, t11, t12, t13, t14, t15] = even;

    *c = [
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t0, t31), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t1, t30a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t2, t29), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t3, t28a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t4, t27_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t5, t26a_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t6, t25_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t7, t24a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t8, t23a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t9, t22_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t10, t21a_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t11, t20_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t12, t19a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t13, t18), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t14, t17a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_add_epi32(t15, t16), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t15, t16), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t14, t17a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t13, t18), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t12, t19a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t11, t20_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t10, t21a_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t9, t22_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t8, t23a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t7, t24a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t6, t25_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t5, t26a_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t4, t27_final), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t3, t28a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t2, t29), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t1, t30a_3), bounds); clipped },
        { let Ok(clipped) = clip8(token, _mm256_sub_epi32(t0, t31), bounds); clipped },
    ];
    Ok(())
}




#[cfg(target_arch = "x86_64")]
#[rite]
fn dct32x32_cols_simd(
    token: Desktop64,
    residuals: &mut [i32],
    clipping: CoefficientBounds,
) -> Rav1dResult {
    let size = console_core_geometry::Size { width: 32, height: 32 };
    let Ok(avx512) = crate::src::cpu::summon_avx512();

    match avx512 {
        Some(wide) => match dct32_cols_avx512(wide, residuals, size, clipping) {
            Ok(()) => return Ok(()),
            Err(error) => console_core_logging::debug!("AV1 column transform scratch: {error:?}"),
        },
        None => {}
    }

    let narrow = clipping.lower == i32::from(i16::MIN) && clipping.upper == i32::from(i16::MAX);
    let bounds = VectorBounds {
        lower: _mm256_set1_epi32(clipping.lower),
        upper: _mm256_set1_epi32(clipping.upper),
    };
    apply_transform_columns_avx2(token, residuals, size, bounds, |active_token, values, limits| {
        match narrow.then_some(()) {
            Some(()) => dct32_1d_cols8_i16(active_token, values, limits),
            None => dct32_1d_cols8(active_token, values, limits),
        }
    })
}
