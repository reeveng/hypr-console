#[cfg(target_arch = "x86_64")]
#[derive(Clone, Copy)]
struct VectorBounds<V> {
    lower: V,
    upper: V,
}

#[cfg(target_arch = "x86_64")]
#[rite]
#[must_use]
fn mac_msub_shr<const SHIFT: i32>(
    _token: Desktop64,
    a: __m256i,
    c1: i32,
    b: __m256i,
    c2: i32,
    rnd: i32,
) -> __m256i {
    let p1 = _mm256_mullo_epi32(a, _mm256_set1_epi32(c1));
    let p2 = _mm256_mullo_epi32(b, _mm256_set1_epi32(c2));
    let sum = _mm256_add_epi32(_mm256_sub_epi32(p1, p2), _mm256_set1_epi32(rnd));
    _mm256_srai_epi32::<SHIFT>(sum)
}


#[cfg(target_arch = "x86_64")]
#[rite]
#[must_use]
fn mac_madd_shr<const SHIFT: i32>(
    _token: Desktop64,
    a: __m256i,
    c1: i32,
    b: __m256i,
    c2: i32,
    rnd: i32,
) -> __m256i {
    let p1 = _mm256_mullo_epi32(a, _mm256_set1_epi32(c1));
    let p2 = _mm256_mullo_epi32(b, _mm256_set1_epi32(c2));
    let sum = _mm256_add_epi32(_mm256_add_epi32(p1, p2), _mm256_set1_epi32(rnd));
    _mm256_srai_epi32::<SHIFT>(sum)
}


#[cfg(target_arch = "x86_64")]
#[rite]
#[must_use]
fn clip8(_token: Desktop64, v: __m256i, bounds: VectorBounds<__m256i>) -> __m256i {
    let VectorBounds { lower: min_v, upper: max_v } = bounds;
    _mm256_min_epi32(_mm256_max_epi32(v, min_v), max_v)
}






#[cfg(target_arch = "x86_64")]
#[rite]
#[must_use]
fn mac_msub_shr_v4<const SHIFT: u32>(
    _token: Server64,
    a: __m512i,
    c1: i32,
    b: __m512i,
    c2: i32,
    rnd: i32,
) -> __m512i {
    let p1 = _mm512_mullo_epi32(a, _mm512_set1_epi32(c1));
    let p2 = _mm512_mullo_epi32(b, _mm512_set1_epi32(c2));
    let sum = _mm512_add_epi32(_mm512_sub_epi32(p1, p2), _mm512_set1_epi32(rnd));
    _mm512_srai_epi32::<SHIFT>(sum)
}


#[cfg(target_arch = "x86_64")]
#[rite]
#[must_use]
fn mac_madd_shr_v4<const SHIFT: u32>(
    _token: Server64,
    a: __m512i,
    c1: i32,
    b: __m512i,
    c2: i32,
    rnd: i32,
) -> __m512i {
    let p1 = _mm512_mullo_epi32(a, _mm512_set1_epi32(c1));
    let p2 = _mm512_mullo_epi32(b, _mm512_set1_epi32(c2));
    let sum = _mm512_add_epi32(_mm512_add_epi32(p1, p2), _mm512_set1_epi32(rnd));
    _mm512_srai_epi32::<SHIFT>(sum)
}


#[cfg(target_arch = "x86_64")]
#[rite]
#[must_use]
fn clip16_i32(_token: Server64, v: __m512i, bounds: VectorBounds<__m512i>) -> __m512i {
    let VectorBounds { lower: min_v, upper: max_v } = bounds;
    _mm512_min_epi32(_mm512_max_epi32(v, min_v), max_v)
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn dct4_1d_cols16(token: Server64, c: &mut [__m512i; 4], bounds: VectorBounds<__m512i>) {
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
    let t2_shifted = mac_msub_shr_v4::<12>(token, in1, 1567, in3, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048);
    let t2 = _mm512_sub_epi32(t2_shifted, in3);
    let t3_shifted = mac_madd_shr_v4::<12>(token, in1, ::core::primitive::i32::wrapping_sub(3784, 4096), in3, 1567, 2048);
    let t3 = _mm512_add_epi32(t3_shifted, in1);

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_add_epi32(t0, t3), bounds);
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_add_epi32(t1, t2), bounds);
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_sub_epi32(t1, t2), bounds);
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_sub_epi32(t0, t3), bounds);
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn dct8_1d_cols16(token: Server64, c: &mut [__m512i; 8], bounds: VectorBounds<__m512i>) {
    let mut even = [match c { &mut [_explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }];
    dct4_1d_cols16(token, &mut even, bounds);
    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = match even { [_explicit_element, ..] => _explicit_element };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _explicit_element, ..] => _explicit_element };

    let &mut [_, in1, ..] = c;
    let &mut [_, _, _, in3, ..] = c;
    let &mut [_, _, _, _, _, in5, ..] = c;
    let &mut [_, _, _, _, _, _, _, in7, ..] = c;

    let t4a = _mm512_sub_epi32(
        mac_msub_shr_v4::<12>(token, in1, 799, in7, ::core::primitive::i32::wrapping_sub(4017, 4096), 2048),
        in7,
    );
    let t5a = mac_msub_shr_v4::<11>(token, in5, 1703, in3, 1138, 1024);
    let t6a = mac_madd_shr_v4::<11>(token, in5, 1138, in3, 1703, 1024);
    let t7a = _mm512_add_epi32(
        mac_madd_shr_v4::<12>(token, in1, ::core::primitive::i32::wrapping_sub(4017, 4096), in7, 799, 2048),
        in1,
    );

    let t4 = clip16_i32(token, _mm512_add_epi32(t4a, t5a), bounds);
    let t5a_n = clip16_i32(token, _mm512_sub_epi32(t4a, t5a), bounds);
    let t7 = clip16_i32(token, _mm512_add_epi32(t7a, t6a), bounds);
    let t6a_n = clip16_i32(token, _mm512_sub_epi32(t7a, t6a), bounds);

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

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_add_epi32(t0, t7), bounds);
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_add_epi32(t1, t6), bounds);
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_add_epi32(t2, t5), bounds);
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_add_epi32(t3, t4), bounds);
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_sub_epi32(t3, t4), bounds);
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_sub_epi32(t2, t5), bounds);
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_sub_epi32(t1, t6), bounds);
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_sub_epi32(t0, t7), bounds);
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn dct16_1d_cols16(token: Server64, c: &mut [__m512i; 16], bounds: VectorBounds<__m512i>) {
    let mut even = [match c { &mut [_explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }];
    dct8_1d_cols16(token, &mut even, bounds);
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
        mac_msub_shr_v4::<12>(token, in1, 401, in15, ::core::primitive::i32::wrapping_sub(4076, 4096), 2048),
        in15,
    );
    let t9a = mac_msub_shr_v4::<11>(token, in9, 1583, in7, 1299, 1024);
    let t10a = _mm512_sub_epi32(
        mac_msub_shr_v4::<12>(token, in5, 1931, in11, ::core::primitive::i32::wrapping_sub(3612, 4096), 2048),
        in11,
    );
    let t11a = _mm512_add_epi32(
        mac_msub_shr_v4::<12>(token, in13, ::core::primitive::i32::wrapping_sub(3920, 4096), in3, 1189, 2048),
        in13,
    );
    let t12a = _mm512_add_epi32(
        mac_madd_shr_v4::<12>(token, in13, 1189, in3, ::core::primitive::i32::wrapping_sub(3920, 4096), 2048),
        in3,
    );
    let t13a = _mm512_add_epi32(
        mac_madd_shr_v4::<12>(token, in5, ::core::primitive::i32::wrapping_sub(3612, 4096), in11, 1931, 2048),
        in5,
    );
    let t14a = mac_madd_shr_v4::<11>(token, in9, 1299, in7, 1583, 1024);
    let t15a = _mm512_add_epi32(
        mac_madd_shr_v4::<12>(token, in1, ::core::primitive::i32::wrapping_sub(4076, 4096), in15, 401, 2048),
        in1,
    );

    let t8 = clip16_i32(token, _mm512_add_epi32(t8a, t9a), bounds);
    let mut t9 = clip16_i32(token, _mm512_sub_epi32(t8a, t9a), bounds);
    let mut t10 = clip16_i32(token, _mm512_sub_epi32(t11a, t10a), bounds);
    let mut t11 = clip16_i32(token, _mm512_add_epi32(t11a, t10a), bounds);
    let mut t12 = clip16_i32(token, _mm512_add_epi32(t12a, t13a), bounds);
    let mut t13 = clip16_i32(token, _mm512_sub_epi32(t12a, t13a), bounds);
    let mut t14 = clip16_i32(token, _mm512_sub_epi32(t15a, t14a), bounds);
    let t15 = clip16_i32(token, _mm512_add_epi32(t15a, t14a), bounds);

    let t9a_2 = _mm512_sub_epi32(
        mac_msub_shr_v4::<12>(token, t14, 1567, t9, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t9,
    );
    let t14a_2 = _mm512_add_epi32(
        mac_madd_shr_v4::<12>(token, t14, ::core::primitive::i32::wrapping_sub(3784, 4096), t9, 1567, 2048),
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
        mac_msub_shr_v4::<12>(token, t13, 1567, t10, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t10,
    );

    let t8a_2 = clip16_i32(token, _mm512_add_epi32(t8, t11), bounds);
    t9 = clip16_i32(token, _mm512_add_epi32(t9a_2, t10a_2), bounds);
    t10 = clip16_i32(token, _mm512_sub_epi32(t9a_2, t10a_2), bounds);
    let t11a_2 = clip16_i32(token, _mm512_sub_epi32(t8, t11), bounds);
    let t12a_2 = clip16_i32(token, _mm512_sub_epi32(t15, t12), bounds);
    t13 = clip16_i32(token, _mm512_sub_epi32(t14a_2, t13a_2), bounds);
    t14 = clip16_i32(token, _mm512_add_epi32(t14a_2, t13a_2), bounds);
    let t15a_2 = clip16_i32(token, _mm512_add_epi32(t15, t12), bounds);

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

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_add_epi32(t0, t15a_2), bounds);
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_add_epi32(t1, t14), bounds);
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_add_epi32(t2, t13a_new), bounds);
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_add_epi32(t3, t12), bounds);
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_add_epi32(t4, t11), bounds);
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_add_epi32(t5, t10a_new), bounds);
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_add_epi32(t6, t9), bounds);
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_add_epi32(t7, t8a_2), bounds);
    *{ let [_, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_sub_epi32(t7, t8a_2), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_sub_epi32(t6, t9), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_sub_epi32(t5, t10a_new), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_sub_epi32(t4, t11), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_sub_epi32(t3, t12), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_sub_epi32(t2, t13a_new), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_sub_epi32(t1, t14), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip16_i32(token, _mm512_sub_epi32(t0, t15a_2), bounds);
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn dct32_1d_cols16(token: Server64, c: &mut [__m512i; 32], bounds: VectorBounds<__m512i>) {

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
    dct16_1d_cols16(token, &mut even, bounds);

    let t16a = _mm512_sub_epi32(
        mac_msub_shr_v4::<12>(token, in1, 201, in31, ::core::primitive::i32::wrapping_sub(4091, 4096), 2048),
        in31,
    );
    let t17a = _mm512_add_epi32(
        mac_msub_shr_v4::<12>(token, in17, ::core::primitive::i32::wrapping_sub(3035, 4096), in15, 2751, 2048),
        in17,
    );
    let t18a = _mm512_sub_epi32(
        mac_msub_shr_v4::<12>(token, in9, 1751, in23, ::core::primitive::i32::wrapping_sub(3703, 4096), 2048),
        in23,
    );
    let t19a = _mm512_add_epi32(
        mac_msub_shr_v4::<12>(token, in25, ::core::primitive::i32::wrapping_sub(3857, 4096), in7, 1380, 2048),
        in25,
    );
    let t20a = _mm512_sub_epi32(
        mac_msub_shr_v4::<12>(token, in5, 995, in27, ::core::primitive::i32::wrapping_sub(3973, 4096), 2048),
        in27,
    );
    let t21a = _mm512_add_epi32(
        mac_msub_shr_v4::<12>(token, in21, ::core::primitive::i32::wrapping_sub(3513, 4096), in11, 2106, 2048),
        in21,
    );
    let t22a = mac_msub_shr_v4::<11>(token, in13, 1220, in19, 1645, 1024);
    let t23a = _mm512_add_epi32(
        mac_msub_shr_v4::<12>(token, in29, ::core::primitive::i32::wrapping_sub(4052, 4096), in3, 601, 2048),
        in29,
    );
    let t24a = _mm512_add_epi32(
        mac_madd_shr_v4::<12>(token, in29, 601, in3, ::core::primitive::i32::wrapping_sub(4052, 4096), 2048),
        in3,
    );
    let t25a = mac_madd_shr_v4::<11>(token, in13, 1645, in19, 1220, 1024);
    let t26a = _mm512_add_epi32(
        mac_madd_shr_v4::<12>(token, in21, 2106, in11, ::core::primitive::i32::wrapping_sub(3513, 4096), 2048),
        in11,
    );
    let t27a = _mm512_add_epi32(
        mac_madd_shr_v4::<12>(token, in5, ::core::primitive::i32::wrapping_sub(3973, 4096), in27, 995, 2048),
        in5,
    );
    let t28a = _mm512_add_epi32(
        mac_madd_shr_v4::<12>(token, in25, 1380, in7, ::core::primitive::i32::wrapping_sub(3857, 4096), 2048),
        in7,
    );
    let t29a = _mm512_add_epi32(
        mac_madd_shr_v4::<12>(token, in9, ::core::primitive::i32::wrapping_sub(3703, 4096), in23, 1751, 2048),
        in9,
    );
    let t30a = _mm512_add_epi32(
        mac_madd_shr_v4::<12>(token, in17, 2751, in15, ::core::primitive::i32::wrapping_sub(3035, 4096), 2048),
        in15,
    );
    let t31a = _mm512_add_epi32(
        mac_madd_shr_v4::<12>(token, in1, ::core::primitive::i32::wrapping_sub(4091, 4096), in31, 201, 2048),
        in1,
    );

    let mut t16 = clip16_i32(token, _mm512_add_epi32(t16a, t17a), bounds);
    let mut t17 = clip16_i32(token, _mm512_sub_epi32(t16a, t17a), bounds);
    let mut t18 = clip16_i32(token, _mm512_sub_epi32(t19a, t18a), bounds);
    let t19 = clip16_i32(token, _mm512_add_epi32(t19a, t18a), bounds);
    let t20 = clip16_i32(token, _mm512_add_epi32(t20a, t21a), bounds);
    let mut t21 = clip16_i32(token, _mm512_sub_epi32(t20a, t21a), bounds);
    let mut t22 = clip16_i32(token, _mm512_sub_epi32(t23a, t22a), bounds);
    let mut t23 = clip16_i32(token, _mm512_add_epi32(t23a, t22a), bounds);
    let mut t24 = clip16_i32(token, _mm512_add_epi32(t24a, t25a), bounds);
    let mut t25 = clip16_i32(token, _mm512_sub_epi32(t24a, t25a), bounds);
    let mut t26 = clip16_i32(token, _mm512_sub_epi32(t27a, t26a), bounds);
    let t27 = clip16_i32(token, _mm512_add_epi32(t27a, t26a), bounds);
    let t28 = clip16_i32(token, _mm512_add_epi32(t28a, t29a), bounds);
    let mut t29 = clip16_i32(token, _mm512_sub_epi32(t28a, t29a), bounds);
    let mut t30 = clip16_i32(token, _mm512_sub_epi32(t31a, t30a), bounds);
    let mut t31 = clip16_i32(token, _mm512_add_epi32(t31a, t30a), bounds);

    let t17a_2 = _mm512_sub_epi32(
        mac_msub_shr_v4::<12>(token, t30, 799, t17, ::core::primitive::i32::wrapping_sub(4017, 4096), 2048),
        t17,
    );
    let t30a_2 = _mm512_add_epi32(
        mac_madd_shr_v4::<12>(token, t30, ::core::primitive::i32::wrapping_sub(4017, 4096), t17, 799, 2048),
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
        mac_msub_shr_v4::<12>(token, t29, 799, t18, ::core::primitive::i32::wrapping_sub(4017, 4096), 2048),
        t18,
    );
    let t21a_2 = mac_msub_shr_v4::<11>(token, t26, 1703, t21, 1138, 1024);
    let t26a_2 = mac_madd_shr_v4::<11>(token, t26, 1138, t21, 1703, 1024);
    let t22a_inner = _mm512_add_epi32(
        _mm512_mullo_epi32(t25, _mm512_set1_epi32(1138)),
        _mm512_mullo_epi32(t22, _mm512_set1_epi32(1703)),
    );
    let t22a_2 = _mm512_srai_epi32::<11>(_mm512_add_epi32(
        _mm512_sub_epi32(_mm512_setzero_si512(), t22a_inner),
        _mm512_set1_epi32(1024),
    ));
    let t25a_2 = mac_msub_shr_v4::<11>(token, t25, 1703, t22, 1138, 1024);

    let t16a_2 = clip16_i32(token, _mm512_add_epi32(t16, t19), bounds);
    t17 = clip16_i32(token, _mm512_add_epi32(t17a_2, t18a_2), bounds);
    t18 = clip16_i32(token, _mm512_sub_epi32(t17a_2, t18a_2), bounds);
    let t19a_2 = clip16_i32(token, _mm512_sub_epi32(t16, t19), bounds);
    let t20a_2 = clip16_i32(token, _mm512_sub_epi32(t23, t20), bounds);
    t21 = clip16_i32(token, _mm512_sub_epi32(t22a_2, t21a_2), bounds);
    t22 = clip16_i32(token, _mm512_add_epi32(t22a_2, t21a_2), bounds);
    let t23a_2 = clip16_i32(token, _mm512_add_epi32(t23, t20), bounds);
    let t24a_2 = clip16_i32(token, _mm512_add_epi32(t24, t27), bounds);
    t25 = clip16_i32(token, _mm512_add_epi32(t25a_2, t26a_2), bounds);
    t26 = clip16_i32(token, _mm512_sub_epi32(t25a_2, t26a_2), bounds);
    let t27a_2 = clip16_i32(token, _mm512_sub_epi32(t24, t27), bounds);
    let t28a_2 = clip16_i32(token, _mm512_sub_epi32(t31, t28), bounds);
    t29 = clip16_i32(token, _mm512_sub_epi32(t30a_2, t29a_2), bounds);
    t30 = clip16_i32(token, _mm512_add_epi32(t30a_2, t29a_2), bounds);
    let t31a_2 = clip16_i32(token, _mm512_add_epi32(t31, t28), bounds);

    let t18a_3 = _mm512_sub_epi32(
        mac_msub_shr_v4::<12>(token, t29, 1567, t18, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t18,
    );
    let t29a_3 = _mm512_add_epi32(
        mac_madd_shr_v4::<12>(token, t29, ::core::primitive::i32::wrapping_sub(3784, 4096), t18, 1567, 2048),
        t29,
    );
    let t19_2 = _mm512_sub_epi32(
        mac_msub_shr_v4::<12>(token, t28a_2, 1567, t19a_2, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t19a_2,
    );
    let t28_2 = _mm512_add_epi32(
        mac_madd_shr_v4::<12>(token, t28a_2, ::core::primitive::i32::wrapping_sub(3784, 4096), t19a_2, 1567, 2048),
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
        mac_msub_shr_v4::<12>(token, t27a_2, 1567, t20a_2, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
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
        mac_msub_shr_v4::<12>(token, t26, 1567, t21, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t21,
    );

    t16 = clip16_i32(token, _mm512_add_epi32(t16a_2, t23a_2), bounds);
    let t17a_3 = clip16_i32(token, _mm512_add_epi32(t17, t22), bounds);
    t18 = clip16_i32(token, _mm512_add_epi32(t18a_3, t21a_3), bounds);
    let t19a_3 = clip16_i32(token, _mm512_add_epi32(t19_2, t20_2), bounds);
    let t20a_3 = clip16_i32(token, _mm512_sub_epi32(t19_2, t20_2), bounds);
    t21 = clip16_i32(token, _mm512_sub_epi32(t18a_3, t21a_3), bounds);
    let t22a_3 = clip16_i32(token, _mm512_sub_epi32(t17, t22), bounds);
    t23 = clip16_i32(token, _mm512_sub_epi32(t16a_2, t23a_2), bounds);
    t24 = clip16_i32(token, _mm512_sub_epi32(t31a_2, t24a_2), bounds);
    let t25a_3 = clip16_i32(token, _mm512_sub_epi32(t30, t25), bounds);
    t26 = clip16_i32(token, _mm512_sub_epi32(t29a_3, t26a_3), bounds);
    let t27a_3 = clip16_i32(token, _mm512_sub_epi32(t28_2, t27_2), bounds);
    let t28a_3 = clip16_i32(token, _mm512_add_epi32(t28_2, t27_2), bounds);
    t29 = clip16_i32(token, _mm512_add_epi32(t29a_3, t26a_3), bounds);
    let t30a_3 = clip16_i32(token, _mm512_add_epi32(t30, t25), bounds);
    t31 = clip16_i32(token, _mm512_add_epi32(t31a_2, t24a_2), bounds);

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
        clip16_i32(token, _mm512_add_epi32(t0, t31), bounds),
        clip16_i32(token, _mm512_add_epi32(t1, t30a_3), bounds),
        clip16_i32(token, _mm512_add_epi32(t2, t29), bounds),
        clip16_i32(token, _mm512_add_epi32(t3, t28a_3), bounds),
        clip16_i32(token, _mm512_add_epi32(t4, t27_final), bounds),
        clip16_i32(token, _mm512_add_epi32(t5, t26a_final), bounds),
        clip16_i32(token, _mm512_add_epi32(t6, t25_final), bounds),
        clip16_i32(token, _mm512_add_epi32(t7, t24a_3), bounds),
        clip16_i32(token, _mm512_add_epi32(t8, t23a_3), bounds),
        clip16_i32(token, _mm512_add_epi32(t9, t22_final), bounds),
        clip16_i32(token, _mm512_add_epi32(t10, t21a_final), bounds),
        clip16_i32(token, _mm512_add_epi32(t11, t20_final), bounds),
        clip16_i32(token, _mm512_add_epi32(t12, t19a_3), bounds),
        clip16_i32(token, _mm512_add_epi32(t13, t18), bounds),
        clip16_i32(token, _mm512_add_epi32(t14, t17a_3), bounds),
        clip16_i32(token, _mm512_add_epi32(t15, t16), bounds),
        clip16_i32(token, _mm512_sub_epi32(t15, t16), bounds),
        clip16_i32(token, _mm512_sub_epi32(t14, t17a_3), bounds),
        clip16_i32(token, _mm512_sub_epi32(t13, t18), bounds),
        clip16_i32(token, _mm512_sub_epi32(t12, t19a_3), bounds),
        clip16_i32(token, _mm512_sub_epi32(t11, t20_final), bounds),
        clip16_i32(token, _mm512_sub_epi32(t10, t21a_final), bounds),
        clip16_i32(token, _mm512_sub_epi32(t9, t22_final), bounds),
        clip16_i32(token, _mm512_sub_epi32(t8, t23a_3), bounds),
        clip16_i32(token, _mm512_sub_epi32(t7, t24a_3), bounds),
        clip16_i32(token, _mm512_sub_epi32(t6, t25_final), bounds),
        clip16_i32(token, _mm512_sub_epi32(t5, t26a_final), bounds),
        clip16_i32(token, _mm512_sub_epi32(t4, t27_final), bounds),
        clip16_i32(token, _mm512_sub_epi32(t3, t28a_3), bounds),
        clip16_i32(token, _mm512_sub_epi32(t2, t29), bounds),
        clip16_i32(token, _mm512_sub_epi32(t1, t30a_3), bounds),
        clip16_i32(token, _mm512_sub_epi32(t0, t31), bounds),
    ];
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

  for (output, row) in values.iter_mut().zip(scratch.chunks_exact(width)) {
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

  for (value, row) in values.into_iter().zip(scratch.chunks_exact_mut(width)) {
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
  transform: impl Fn(Desktop64, &mut [__m256i; ROWS], VectorBounds<__m256i>),
) -> Rav1dResult {
  let row_width = NonZeroU32::new(size.width).ok_or(Rav1dError::EINVAL)?;
  let active = checked_scratch(scratch, size)?;
  let Ok(stride) = console_core_number_conversion::index(row_width.get());

  for column in 0..size.width.wrapping_div(8) {
    let Ok(first) = console_core_number_conversion::index(column.wrapping_mul(8));
    let mut lanes = [_mm256_setzero_si256(); ROWS];

    for (lane, row) in lanes.iter_mut().zip(active.chunks_exact(stride)) {
      let chunk = row.get(first..).and_then(<[_]>::first_chunk::<8>);
      let values = chunk.ok_or(Rav1dError::EINVAL)?;
      *lane = loadu_256!(values);
    }

    transform(token, &mut lanes, bounds);

    for (lane, row) in lanes.into_iter().zip(active.chunks_exact_mut(stride)) {
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
    dct4_1d_cols16(token, &mut values, bounds);
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
    dct8_1d_cols16(token, &mut values, bounds);
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
    dct16_1d_cols16(token, &mut values, bounds);
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
    dct32_1d_cols16(token, &mut values, bounds);
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

  for row in active.chunks_exact_mut(row_width) {
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

  for row in active.chunks_exact_mut(row_width) {
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

  let [] = remainder else { return Err(Rav1dError::EINVAL) };

  let last_row = first_row.checked_add(16).ok_or(Rav1dError::EINVAL)?;
  let active = checked_scratch_rows(scratch, row_width, first_row..last_row)?;
  let Ok(stride) = console_core_number_conversion::index(row_width.get());

  for (columns, segment) in strips.iter().zip(0u32..) {
    let Ok(rows) = transpose_16x16_i32(_token, *columns);
    let Ok(offset) = console_core_number_conversion::index(segment.wrapping_mul(16));

    for (output, row) in active.chunks_exact_mut(stride).zip(rows) {
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
  dct8_1d_cols16(token, &mut vectors, bounds);
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
  dct16_1d_cols16(token, &mut vectors, bounds);
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
  dct32_1d_cols16(token, &mut vectors, bounds);
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
fn dct4_1d_cols8(token: Desktop64, c: &mut [__m256i; 4], bounds: VectorBounds<__m256i>) {
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

    let t2_shifted = mac_msub_shr::<12>(token, in1, 1567, in3, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048);
    let t2 = _mm256_sub_epi32(t2_shifted, in3);

    let t3_shifted = mac_madd_shr::<12>(token, in1, ::core::primitive::i32::wrapping_sub(3784, 4096), in3, 1567, 2048);
    let t3 = _mm256_add_epi32(t3_shifted, in1);

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t0, t3), bounds);
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t1, t2), bounds);
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t1, t2), bounds);
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t0, t3), bounds);
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn dct8_1d_cols8(token: Desktop64, c: &mut [__m256i; 8], bounds: VectorBounds<__m256i>) {

    let mut even = [match c { &mut [_explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }];
    dct4_1d_cols8(token, &mut even, bounds);
    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = match even { [_explicit_element, ..] => _explicit_element };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _explicit_element, ..] => _explicit_element };

    let &mut [_, in1, ..] = c;
    let &mut [_, _, _, in3, ..] = c;
    let &mut [_, _, _, _, _, in5, ..] = c;
    let &mut [_, _, _, _, _, _, _, in7, ..] = c;


    let t4a = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, in1, 799, in7, ::core::primitive::i32::wrapping_sub(4017, 4096), 2048),
        in7,
    );

    let t5a = mac_msub_shr::<11>(token, in5, 1703, in3, 1138, 1024);

    let t6a = mac_madd_shr::<11>(token, in5, 1138, in3, 1703, 1024);

    let t7a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in1, ::core::primitive::i32::wrapping_sub(4017, 4096), in7, 799, 2048),
        in1,
    );

    let t4 = clip8(token, _mm256_add_epi32(t4a, t5a), bounds);
    let t5a_n = clip8(token, _mm256_sub_epi32(t4a, t5a), bounds);
    let t7 = clip8(token, _mm256_add_epi32(t7a, t6a), bounds);
    let t6a_n = clip8(token, _mm256_sub_epi32(t7a, t6a), bounds);


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

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t0, t7), bounds);
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t1, t6), bounds);
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t2, t5), bounds);
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t3, t4), bounds);
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t3, t4), bounds);
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t2, t5), bounds);
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t1, t6), bounds);
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t0, t7), bounds);
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn adst8_1d_cols8(token: Desktop64, c: &mut [__m256i; 8], bounds: VectorBounds<__m256i>) {
    let &mut [in0, ..] = c;
    let &mut [_, in1, ..] = c;
    let &mut [_, _, in2, ..] = c;
    let &mut [_, _, _, in3, ..] = c;
    let &mut [_, _, _, _, in4, ..] = c;
    let &mut [_, _, _, _, _, in5, ..] = c;
    let &mut [_, _, _, _, _, _, in6, ..] = c;
    let &mut [_, _, _, _, _, _, _, in7, ..] = c;

    let t0a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in7, ::core::primitive::i32::wrapping_sub(4076, 4096), in0, 401, 2048),
        in7,
    );
    let t1a = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, in7, 401, in0, ::core::primitive::i32::wrapping_sub(4076, 4096), 2048),
        in0,
    );
    let t2a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in5, ::core::primitive::i32::wrapping_sub(3612, 4096), in2, 1931, 2048),
        in5,
    );
    let t3a = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, in5, 1931, in2, ::core::primitive::i32::wrapping_sub(3612, 4096), 2048),
        in2,
    );
    let t4a = mac_madd_shr::<11>(token, in3, 1299, in4, 1583, 1024);
    let t5a = mac_msub_shr::<11>(token, in3, 1583, in4, 1299, 1024);
    let t6a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in1, 1189, in6, ::core::primitive::i32::wrapping_sub(3920, 4096), 2048),
        in6,
    );
    let t7a = _mm256_add_epi32(
        mac_msub_shr::<12>(token, in1, ::core::primitive::i32::wrapping_sub(3920, 4096), in6, 1189, 2048),
        in1,
    );

    let t0 = clip8(token, _mm256_add_epi32(t0a, t4a), bounds);
    let t1 = clip8(token, _mm256_add_epi32(t1a, t5a), bounds);
    let t2 = clip8(token, _mm256_add_epi32(t2a, t6a), bounds);
    let t3 = clip8(token, _mm256_add_epi32(t3a, t7a), bounds);
    let t4 = clip8(token, _mm256_sub_epi32(t0a, t4a), bounds);
    let t5 = clip8(token, _mm256_sub_epi32(t1a, t5a), bounds);
    let t6 = clip8(token, _mm256_sub_epi32(t2a, t6a), bounds);
    let t7 = clip8(token, _mm256_sub_epi32(t3a, t7a), bounds);

    let t4a_2 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, t4, ::core::primitive::i32::wrapping_sub(3784, 4096), t5, 1567, 2048),
        t4,
    );
    let t5a_2 = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, t4, 1567, t5, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t5,
    );
    let t6a_2 = _mm256_add_epi32(
        mac_msub_shr::<12>(token, t7, ::core::primitive::i32::wrapping_sub(3784, 4096), t6, 1567, 2048),
        t7,
    );
    let t7a_2 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, t7, 1567, t6, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t6,
    );

    let zero = _mm256_setzero_si256();
    let out0 = clip8(token, _mm256_add_epi32(t0, t2), bounds);
    let out7 = _mm256_sub_epi32(zero, clip8(token, _mm256_add_epi32(t1, t3), bounds));
    let t2_final = clip8(token, _mm256_sub_epi32(t0, t2), bounds);
    let t3_final = clip8(token, _mm256_sub_epi32(t1, t3), bounds);
    let out1 = _mm256_sub_epi32(zero, clip8(token, _mm256_add_epi32(t4a_2, t6a_2), bounds));
    let out6 = clip8(token, _mm256_add_epi32(t5a_2, t7a_2), bounds);
    let t6_final = clip8(token, _mm256_sub_epi32(t4a_2, t6a_2), bounds);
    let t7_final = clip8(token, _mm256_sub_epi32(t5a_2, t7a_2), bounds);

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
}



#[cfg(target_arch = "x86_64")]
#[rite]
fn flipadst8_1d_cols8(token: Desktop64, c: &mut [__m256i; 8], bounds: VectorBounds<__m256i>) {
    adst8_1d_cols8(token, c, bounds);
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
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn identity8_1d_cols8(_token: Desktop64, c: &mut [__m256i; 8], _bounds: VectorBounds<__m256i>) {
    for c_element in <[_]>::iter_mut(c) {
        *c_element = _mm256_slli_epi32::<1>(*c_element);
    }
}


#[cfg(target_arch = "x86_64")]
#[rite]
fn dct16_1d_cols8(token: Desktop64, c: &mut [__m256i; 16], bounds: VectorBounds<__m256i>) {

    let mut even = [match c { &mut [_explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }];
    dct8_1d_cols8(token, &mut even, bounds);
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
        mac_msub_shr::<12>(token, in1, 401, in15, ::core::primitive::i32::wrapping_sub(4076, 4096), 2048),
        in15,
    );

    let t9a = mac_msub_shr::<11>(token, in9, 1583, in7, 1299, 1024);

    let t10a = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, in5, 1931, in11, ::core::primitive::i32::wrapping_sub(3612, 4096), 2048),
        in11,
    );

    let t11a = _mm256_add_epi32(
        mac_msub_shr::<12>(token, in13, ::core::primitive::i32::wrapping_sub(3920, 4096), in3, 1189, 2048),
        in13,
    );

    let t12a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in13, 1189, in3, ::core::primitive::i32::wrapping_sub(3920, 4096), 2048),
        in3,
    );

    let t13a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in5, ::core::primitive::i32::wrapping_sub(3612, 4096), in11, 1931, 2048),
        in5,
    );

    let t14a = mac_madd_shr::<11>(token, in9, 1299, in7, 1583, 1024);

    let t15a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in1, ::core::primitive::i32::wrapping_sub(4076, 4096), in15, 401, 2048),
        in1,
    );

    let t8 = clip8(token, _mm256_add_epi32(t8a, t9a), bounds);
    let mut t9 = clip8(token, _mm256_sub_epi32(t8a, t9a), bounds);
    let mut t10 = clip8(token, _mm256_sub_epi32(t11a, t10a), bounds);
    let mut t11 = clip8(token, _mm256_add_epi32(t11a, t10a), bounds);
    let mut t12 = clip8(token, _mm256_add_epi32(t12a, t13a), bounds);
    let mut t13 = clip8(token, _mm256_sub_epi32(t12a, t13a), bounds);
    let mut t14 = clip8(token, _mm256_sub_epi32(t15a, t14a), bounds);
    let t15 = clip8(token, _mm256_add_epi32(t15a, t14a), bounds);


    let t9a_2 = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, t14, 1567, t9, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t9,
    );

    let t14a_2 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, t14, ::core::primitive::i32::wrapping_sub(3784, 4096), t9, 1567, 2048),
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
        mac_msub_shr::<12>(token, t13, 1567, t10, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t10,
    );

    let t8a_2 = clip8(token, _mm256_add_epi32(t8, t11), bounds);
    t9 = clip8(token, _mm256_add_epi32(t9a_2, t10a_2), bounds);
    t10 = clip8(token, _mm256_sub_epi32(t9a_2, t10a_2), bounds);
    let t11a_2 = clip8(token, _mm256_sub_epi32(t8, t11), bounds);
    let t12a_2 = clip8(token, _mm256_sub_epi32(t15, t12), bounds);
    t13 = clip8(token, _mm256_sub_epi32(t14a_2, t13a_2), bounds);
    t14 = clip8(token, _mm256_add_epi32(t14a_2, t13a_2), bounds);
    let t15a_2 = clip8(token, _mm256_add_epi32(t15, t12), bounds);


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

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t0, t15a_2), bounds);
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t1, t14), bounds);
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t2, t13a_new), bounds);
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t3, t12), bounds);
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t4, t11), bounds);
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t5, t10a_new), bounds);
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t6, t9), bounds);
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t7, t8a_2), bounds);
    *{ let [_, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t7, t8a_2), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t6, t9), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t5, t10a_new), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t4, t11), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t3, t12), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t2, t13a_new), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t1, t14), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t0, t15a_2), bounds);
}



#[cfg(target_arch = "x86_64")]
#[rite]
fn adst16_1d_cols8(token: Desktop64, c: &mut [__m256i; 16], bounds: VectorBounds<__m256i>) {
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
        mac_madd_shr::<12>(token, in15, ::core::primitive::i32::wrapping_sub(4091, 4096), in0, 201, 2048),
        in15,
    );
    let mut t1 = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, in15, 201, in0, ::core::primitive::i32::wrapping_sub(4091, 4096), 2048),
        in0,
    );
    let mut t2 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in13, ::core::primitive::i32::wrapping_sub(3973, 4096), in2, 995, 2048),
        in13,
    );
    let mut t3 = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, in13, 995, in2, ::core::primitive::i32::wrapping_sub(3973, 4096), 2048),
        in2,
    );
    let mut t4 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in11, ::core::primitive::i32::wrapping_sub(3703, 4096), in4, 1751, 2048),
        in11,
    );
    let mut t5 = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, in11, 1751, in4, ::core::primitive::i32::wrapping_sub(3703, 4096), 2048),
        in4,
    );
    let mut t6 = mac_madd_shr::<11>(token, in9, 1645, in6, 1220, 1024);
    let mut t7 = mac_msub_shr::<11>(token, in9, 1220, in6, 1645, 1024);
    let mut t8 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in7, 2751, in8, ::core::primitive::i32::wrapping_sub(3035, 4096), 2048),
        in8,
    );
    let mut t9 = _mm256_add_epi32(
        mac_msub_shr::<12>(token, in7, ::core::primitive::i32::wrapping_sub(3035, 4096), in8, 2751, 2048),
        in7,
    );
    let mut t10 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in5, 2106, in10, ::core::primitive::i32::wrapping_sub(3513, 4096), 2048),
        in10,
    );
    let mut t11 = _mm256_add_epi32(
        mac_msub_shr::<12>(token, in5, ::core::primitive::i32::wrapping_sub(3513, 4096), in10, 2106, 2048),
        in5,
    );
    let mut t12 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in3, 1380, in12, ::core::primitive::i32::wrapping_sub(3857, 4096), 2048),
        in12,
    );
    let mut t13 = _mm256_add_epi32(
        mac_msub_shr::<12>(token, in3, ::core::primitive::i32::wrapping_sub(3857, 4096), in12, 1380, 2048),
        in3,
    );
    let mut t14 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in1, 601, in14, ::core::primitive::i32::wrapping_sub(4052, 4096), 2048),
        in14,
    );
    let mut t15 = _mm256_add_epi32(
        mac_msub_shr::<12>(token, in1, ::core::primitive::i32::wrapping_sub(4052, 4096), in14, 601, 2048),
        in1,
    );

    let t0a = clip8(token, _mm256_add_epi32(t0, t8), bounds);
    let t1a = clip8(token, _mm256_add_epi32(t1, t9), bounds);
    let t2a = clip8(token, _mm256_add_epi32(t2, t10), bounds);
    let t3a = clip8(token, _mm256_add_epi32(t3, t11), bounds);
    let mut t4a = clip8(token, _mm256_add_epi32(t4, t12), bounds);
    let mut t5a = clip8(token, _mm256_add_epi32(t5, t13), bounds);
    let mut t6a = clip8(token, _mm256_add_epi32(t6, t14), bounds);
    let mut t7a = clip8(token, _mm256_add_epi32(t7, t15), bounds);
    let mut t8a = clip8(token, _mm256_sub_epi32(t0, t8), bounds);
    let mut t9a = clip8(token, _mm256_sub_epi32(t1, t9), bounds);
    let mut t10a = clip8(token, _mm256_sub_epi32(t2, t10), bounds);
    let mut t11a = clip8(token, _mm256_sub_epi32(t3, t11), bounds);
    let mut t12a = clip8(token, _mm256_sub_epi32(t4, t12), bounds);
    let mut t13a = clip8(token, _mm256_sub_epi32(t5, t13), bounds);
    let mut t14a = clip8(token, _mm256_sub_epi32(t6, t14), bounds);
    let mut t15a = clip8(token, _mm256_sub_epi32(t7, t15), bounds);

    t8 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, t8a, ::core::primitive::i32::wrapping_sub(4017, 4096), t9a, 799, 2048),
        t8a,
    );
    t9 = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, t8a, 799, t9a, ::core::primitive::i32::wrapping_sub(4017, 4096), 2048),
        t9a,
    );
    t10 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, t10a, 2276, t11a, ::core::primitive::i32::wrapping_sub(3406, 4096), 2048),
        t11a,
    );
    t11 = _mm256_add_epi32(
        mac_msub_shr::<12>(token, t10a, ::core::primitive::i32::wrapping_sub(3406, 4096), t11a, 2276, 2048),
        t10a,
    );
    t12 = _mm256_add_epi32(
        mac_msub_shr::<12>(token, t13a, ::core::primitive::i32::wrapping_sub(4017, 4096), t12a, 799, 2048),
        t13a,
    );
    t13 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, t13a, 799, t12a, ::core::primitive::i32::wrapping_sub(4017, 4096), 2048),
        t12a,
    );
    t14 = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, t15a, 2276, t14a, ::core::primitive::i32::wrapping_sub(3406, 4096), 2048),
        t14a,
    );
    t15 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, t15a, ::core::primitive::i32::wrapping_sub(3406, 4096), t14a, 2276, 2048),
        t15a,
    );

    t0 = clip8(token, _mm256_add_epi32(t0a, t4a), bounds);
    t1 = clip8(token, _mm256_add_epi32(t1a, t5a), bounds);
    t2 = clip8(token, _mm256_add_epi32(t2a, t6a), bounds);
    t3 = clip8(token, _mm256_add_epi32(t3a, t7a), bounds);
    t4 = clip8(token, _mm256_sub_epi32(t0a, t4a), bounds);
    t5 = clip8(token, _mm256_sub_epi32(t1a, t5a), bounds);
    t6 = clip8(token, _mm256_sub_epi32(t2a, t6a), bounds);
    t7 = clip8(token, _mm256_sub_epi32(t3a, t7a), bounds);
    t8a = clip8(token, _mm256_add_epi32(t8, t12), bounds);
    t9a = clip8(token, _mm256_add_epi32(t9, t13), bounds);
    t10a = clip8(token, _mm256_add_epi32(t10, t14), bounds);
    t11a = clip8(token, _mm256_add_epi32(t11, t15), bounds);
    t12a = clip8(token, _mm256_sub_epi32(t8, t12), bounds);
    t13a = clip8(token, _mm256_sub_epi32(t9, t13), bounds);
    t14a = clip8(token, _mm256_sub_epi32(t10, t14), bounds);
    t15a = clip8(token, _mm256_sub_epi32(t11, t15), bounds);

    t4a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, t4, ::core::primitive::i32::wrapping_sub(3784, 4096), t5, 1567, 2048),
        t4,
    );
    t5a = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, t4, 1567, t5, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t5,
    );
    t6a = _mm256_add_epi32(
        mac_msub_shr::<12>(token, t7, ::core::primitive::i32::wrapping_sub(3784, 4096), t6, 1567, 2048),
        t7,
    );
    t7a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, t7, 1567, t6, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t6,
    );
    t12 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, t12a, ::core::primitive::i32::wrapping_sub(3784, 4096), t13a, 1567, 2048),
        t12a,
    );
    t13 = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, t12a, 1567, t13a, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t13a,
    );
    t14 = _mm256_add_epi32(
        mac_msub_shr::<12>(token, t15a, ::core::primitive::i32::wrapping_sub(3784, 4096), t14a, 1567, 2048),
        t15a,
    );
    t15 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, t15a, 1567, t14a, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t14a,
    );

    let zero = _mm256_setzero_si256();

    let out0 = clip8(token, _mm256_add_epi32(t0, t2), bounds);
    let out15 = _mm256_sub_epi32(zero, clip8(token, _mm256_add_epi32(t1, t3), bounds));
    let t2a_new = clip8(token, _mm256_sub_epi32(t0, t2), bounds);
    let t3a_new = clip8(token, _mm256_sub_epi32(t1, t3), bounds);
    let out3 = _mm256_sub_epi32(zero, clip8(token, _mm256_add_epi32(t4a, t6a), bounds));
    let out12 = clip8(token, _mm256_add_epi32(t5a, t7a), bounds);
    let t6_new = clip8(token, _mm256_sub_epi32(t4a, t6a), bounds);
    let t7_new = clip8(token, _mm256_sub_epi32(t5a, t7a), bounds);
    let out1 = _mm256_sub_epi32(
        zero,
        clip8(token, _mm256_add_epi32(t8a, t10a), bounds),
    );
    let out14 = clip8(token, _mm256_add_epi32(t9a, t11a), bounds);
    let t10_new = clip8(token, _mm256_sub_epi32(t8a, t10a), bounds);
    let t11_new = clip8(token, _mm256_sub_epi32(t9a, t11a), bounds);
    let out2 = clip8(token, _mm256_add_epi32(t12, t14), bounds);
    let out13 = _mm256_sub_epi32(zero, clip8(token, _mm256_add_epi32(t13, t15), bounds));
    let t14_new = clip8(token, _mm256_sub_epi32(t12, t14), bounds);
    let t15_new = clip8(token, _mm256_sub_epi32(t13, t15), bounds);


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
            adst16_1d_cols8(desktop, lanes, clipping);
            lanes.reverse();
          }
          Square16Columns::Identity => {}
        }
      })
    }
  }
}

#[cfg(target_arch = "x86_64")]
#[rite]
fn dct32_1d_cols8(token: Desktop64, c: &mut [__m256i; 32], bounds: VectorBounds<__m256i>) {

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
    dct16_1d_cols8(token, &mut even, bounds);

    let t16a = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, in1, 201, in31, ::core::primitive::i32::wrapping_sub(4091, 4096), 2048),
        in31,
    );
    let t17a = _mm256_add_epi32(
        mac_msub_shr::<12>(token, in17, ::core::primitive::i32::wrapping_sub(3035, 4096), in15, 2751, 2048),
        in17,
    );
    let t18a = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, in9, 1751, in23, ::core::primitive::i32::wrapping_sub(3703, 4096), 2048),
        in23,
    );
    let t19a = _mm256_add_epi32(
        mac_msub_shr::<12>(token, in25, ::core::primitive::i32::wrapping_sub(3857, 4096), in7, 1380, 2048),
        in25,
    );
    let t20a = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, in5, 995, in27, ::core::primitive::i32::wrapping_sub(3973, 4096), 2048),
        in27,
    );
    let t21a = _mm256_add_epi32(
        mac_msub_shr::<12>(token, in21, ::core::primitive::i32::wrapping_sub(3513, 4096), in11, 2106, 2048),
        in21,
    );
    let t22a = mac_msub_shr::<11>(token, in13, 1220, in19, 1645, 1024);
    let t23a = _mm256_add_epi32(
        mac_msub_shr::<12>(token, in29, ::core::primitive::i32::wrapping_sub(4052, 4096), in3, 601, 2048),
        in29,
    );
    let t24a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in29, 601, in3, ::core::primitive::i32::wrapping_sub(4052, 4096), 2048),
        in3,
    );
    let t25a = mac_madd_shr::<11>(token, in13, 1645, in19, 1220, 1024);
    let t26a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in21, 2106, in11, ::core::primitive::i32::wrapping_sub(3513, 4096), 2048),
        in11,
    );
    let t27a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in5, ::core::primitive::i32::wrapping_sub(3973, 4096), in27, 995, 2048),
        in5,
    );
    let t28a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in25, 1380, in7, ::core::primitive::i32::wrapping_sub(3857, 4096), 2048),
        in7,
    );
    let t29a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in9, ::core::primitive::i32::wrapping_sub(3703, 4096), in23, 1751, 2048),
        in9,
    );
    let t30a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in17, 2751, in15, ::core::primitive::i32::wrapping_sub(3035, 4096), 2048),
        in15,
    );
    let t31a = _mm256_add_epi32(
        mac_madd_shr::<12>(token, in1, ::core::primitive::i32::wrapping_sub(4091, 4096), in31, 201, 2048),
        in1,
    );

    let mut t16 = clip8(token, _mm256_add_epi32(t16a, t17a), bounds);
    let mut t17 = clip8(token, _mm256_sub_epi32(t16a, t17a), bounds);
    let mut t18 = clip8(token, _mm256_sub_epi32(t19a, t18a), bounds);
    let t19 = clip8(token, _mm256_add_epi32(t19a, t18a), bounds);
    let t20 = clip8(token, _mm256_add_epi32(t20a, t21a), bounds);
    let mut t21 = clip8(token, _mm256_sub_epi32(t20a, t21a), bounds);
    let mut t22 = clip8(token, _mm256_sub_epi32(t23a, t22a), bounds);
    let mut t23 = clip8(token, _mm256_add_epi32(t23a, t22a), bounds);
    let mut t24 = clip8(token, _mm256_add_epi32(t24a, t25a), bounds);
    let mut t25 = clip8(token, _mm256_sub_epi32(t24a, t25a), bounds);
    let mut t26 = clip8(token, _mm256_sub_epi32(t27a, t26a), bounds);
    let t27 = clip8(token, _mm256_add_epi32(t27a, t26a), bounds);
    let t28 = clip8(token, _mm256_add_epi32(t28a, t29a), bounds);
    let mut t29 = clip8(token, _mm256_sub_epi32(t28a, t29a), bounds);
    let mut t30 = clip8(token, _mm256_sub_epi32(t31a, t30a), bounds);
    let mut t31 = clip8(token, _mm256_add_epi32(t31a, t30a), bounds);

    let t17a_2 = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, t30, 799, t17, ::core::primitive::i32::wrapping_sub(4017, 4096), 2048),
        t17,
    );
    let t30a_2 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, t30, ::core::primitive::i32::wrapping_sub(4017, 4096), t17, 799, 2048),
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
        mac_msub_shr::<12>(token, t29, 799, t18, ::core::primitive::i32::wrapping_sub(4017, 4096), 2048),
        t18,
    );
    let t21a_2 = mac_msub_shr::<11>(token, t26, 1703, t21, 1138, 1024);
    let t26a_2 = mac_madd_shr::<11>(token, t26, 1138, t21, 1703, 1024);

    let t22a_inner = _mm256_add_epi32(
        _mm256_mullo_epi32(t25, _mm256_set1_epi32(1138)),
        _mm256_mullo_epi32(t22, _mm256_set1_epi32(1703)),
    );
    let t22a_2 = _mm256_srai_epi32::<11>(_mm256_add_epi32(
        _mm256_sub_epi32(_mm256_setzero_si256(), t22a_inner),
        _mm256_set1_epi32(1024),
    ));
    let t25a_2 = mac_msub_shr::<11>(token, t25, 1703, t22, 1138, 1024);

    let t16a_2 = clip8(token, _mm256_add_epi32(t16, t19), bounds);
    t17 = clip8(token, _mm256_add_epi32(t17a_2, t18a_2), bounds);
    t18 = clip8(token, _mm256_sub_epi32(t17a_2, t18a_2), bounds);
    let t19a_2 = clip8(token, _mm256_sub_epi32(t16, t19), bounds);
    let t20a_2 = clip8(token, _mm256_sub_epi32(t23, t20), bounds);
    t21 = clip8(token, _mm256_sub_epi32(t22a_2, t21a_2), bounds);
    t22 = clip8(token, _mm256_add_epi32(t22a_2, t21a_2), bounds);
    let t23a_2 = clip8(token, _mm256_add_epi32(t23, t20), bounds);
    let t24a_2 = clip8(token, _mm256_add_epi32(t24, t27), bounds);
    t25 = clip8(token, _mm256_add_epi32(t25a_2, t26a_2), bounds);
    t26 = clip8(token, _mm256_sub_epi32(t25a_2, t26a_2), bounds);
    let t27a_2 = clip8(token, _mm256_sub_epi32(t24, t27), bounds);
    let t28a_2 = clip8(token, _mm256_sub_epi32(t31, t28), bounds);
    t29 = clip8(token, _mm256_sub_epi32(t30a_2, t29a_2), bounds);
    t30 = clip8(token, _mm256_add_epi32(t30a_2, t29a_2), bounds);
    let t31a_2 = clip8(token, _mm256_add_epi32(t31, t28), bounds);

    let t18a_3 = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, t29, 1567, t18, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t18,
    );
    let t29a_3 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, t29, ::core::primitive::i32::wrapping_sub(3784, 4096), t18, 1567, 2048),
        t29,
    );
    let t19_2 = _mm256_sub_epi32(
        mac_msub_shr::<12>(token, t28a_2, 1567, t19a_2, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t19a_2,
    );
    let t28_2 = _mm256_add_epi32(
        mac_madd_shr::<12>(token, t28a_2, ::core::primitive::i32::wrapping_sub(3784, 4096), t19a_2, 1567, 2048),
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
        mac_msub_shr::<12>(token, t27a_2, 1567, t20a_2, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
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
        mac_msub_shr::<12>(token, t26, 1567, t21, ::core::primitive::i32::wrapping_sub(3784, 4096), 2048),
        t21,
    );

    t16 = clip8(token, _mm256_add_epi32(t16a_2, t23a_2), bounds);
    let t17a_3 = clip8(token, _mm256_add_epi32(t17, t22), bounds);
    t18 = clip8(token, _mm256_add_epi32(t18a_3, t21a_3), bounds);
    let t19a_3 = clip8(token, _mm256_add_epi32(t19_2, t20_2), bounds);
    let t20a_3 = clip8(token, _mm256_sub_epi32(t19_2, t20_2), bounds);
    t21 = clip8(token, _mm256_sub_epi32(t18a_3, t21a_3), bounds);
    let t22a_3 = clip8(token, _mm256_sub_epi32(t17, t22), bounds);
    t23 = clip8(token, _mm256_sub_epi32(t16a_2, t23a_2), bounds);
    t24 = clip8(token, _mm256_sub_epi32(t31a_2, t24a_2), bounds);
    let t25a_3 = clip8(token, _mm256_sub_epi32(t30, t25), bounds);
    t26 = clip8(token, _mm256_sub_epi32(t29a_3, t26a_3), bounds);
    let t27a_3 = clip8(token, _mm256_sub_epi32(t28_2, t27_2), bounds);
    let t28a_3 = clip8(token, _mm256_add_epi32(t28_2, t27_2), bounds);
    t29 = clip8(token, _mm256_add_epi32(t29a_3, t26a_3), bounds);
    let t30a_3 = clip8(token, _mm256_add_epi32(t30, t25), bounds);
    t31 = clip8(token, _mm256_add_epi32(t31a_2, t24a_2), bounds);

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
        clip8(token, _mm256_add_epi32(t0, t31), bounds),
        clip8(token, _mm256_add_epi32(t1, t30a_3), bounds),
        clip8(token, _mm256_add_epi32(t2, t29), bounds),
        clip8(token, _mm256_add_epi32(t3, t28a_3), bounds),
        clip8(token, _mm256_add_epi32(t4, t27_final), bounds),
        clip8(token, _mm256_add_epi32(t5, t26a_final), bounds),
        clip8(token, _mm256_add_epi32(t6, t25_final), bounds),
        clip8(token, _mm256_add_epi32(t7, t24a_3), bounds),
        clip8(token, _mm256_add_epi32(t8, t23a_3), bounds),
        clip8(token, _mm256_add_epi32(t9, t22_final), bounds),
        clip8(token, _mm256_add_epi32(t10, t21a_final), bounds),
        clip8(token, _mm256_add_epi32(t11, t20_final), bounds),
        clip8(token, _mm256_add_epi32(t12, t19a_3), bounds),
        clip8(token, _mm256_add_epi32(t13, t18), bounds),
        clip8(token, _mm256_add_epi32(t14, t17a_3), bounds),
        clip8(token, _mm256_add_epi32(t15, t16), bounds),
        clip8(token, _mm256_sub_epi32(t15, t16), bounds),
        clip8(token, _mm256_sub_epi32(t14, t17a_3), bounds),
        clip8(token, _mm256_sub_epi32(t13, t18), bounds),
        clip8(token, _mm256_sub_epi32(t12, t19a_3), bounds),
        clip8(token, _mm256_sub_epi32(t11, t20_final), bounds),
        clip8(token, _mm256_sub_epi32(t10, t21a_final), bounds),
        clip8(token, _mm256_sub_epi32(t9, t22_final), bounds),
        clip8(token, _mm256_sub_epi32(t8, t23a_3), bounds),
        clip8(token, _mm256_sub_epi32(t7, t24a_3), bounds),
        clip8(token, _mm256_sub_epi32(t6, t25_final), bounds),
        clip8(token, _mm256_sub_epi32(t5, t26a_final), bounds),
        clip8(token, _mm256_sub_epi32(t4, t27_final), bounds),
        clip8(token, _mm256_sub_epi32(t3, t28a_3), bounds),
        clip8(token, _mm256_sub_epi32(t2, t29), bounds),
        clip8(token, _mm256_sub_epi32(t1, t30a_3), bounds),
        clip8(token, _mm256_sub_epi32(t0, t31), bounds),
    ];
}






















#[cfg(target_arch = "x86_64")]
#[rite]
#[inline(always)]
#[must_use]
fn i32_to_i16_pair(_token: Desktop64, a: __m256i, b: __m256i) -> __m256i {




    #[cfg(debug_assertions)]
    {
        let mut buf = [0i32; 16];
        let lo: &mut [i32; 8] = { let [_explicit_part @ .., _, _, _, _, _, _, _, _] = &mut buf; let _explicit_slice: &mut [_] = _explicit_part; _explicit_slice }.try_into().unwrap();
        storeu_256!(lo, [i32; 8], a);
        let hi: &mut [i32; 8] = { let [_, _, _, _, _, _, _, _, _explicit_part @ ..] = &mut buf; let _explicit_slice: &mut [_] = _explicit_part; _explicit_slice }.try_into().unwrap();
        storeu_256!(hi, [i32; 8], b);

        for &v in buf.iter() {
            debug_assert!(
                v >= <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MIN) && v <= <i32 as ::core::convert::From<::core::primitive::i16>>::from(i16::MAX),
                "i32_to_i16_pair: lane {v} outside i16 range — _mm_packs_epi32 would \
                 silently saturate and corrupt output. Caller must clip before packing."
            );
        }
    }

    let a_i16 = _mm_packs_epi32(_mm256_castsi256_si128(a), _mm256_extracti128_si256(a, 1));
    let b_i16 = _mm_packs_epi32(_mm256_castsi256_si128(b), _mm256_extracti128_si256(b, 1));
    _mm256_set_m128i(
        _mm_unpackhi_epi16(a_i16, b_i16),
        _mm_unpacklo_epi16(a_i16, b_i16),
    )
}



#[cfg(target_arch = "x86_64")]
#[rite]
fn dct4_1d_cols8_i16(token: Desktop64, c: &mut [__m256i; 4], bounds: VectorBounds<__m256i>) {
    let pd_2048 = _mm256_set1_epi32(2048);
    let pd_128 = _mm256_set1_epi32(128);




    let pair_02 = i32_to_i16_pair(token, match c { &mut [_explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _explicit_element, ..] => _explicit_element });
    let t0 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_02, dct8_row_coef_pack(token, 181, 181)),
        pd_128,
    ));
    let t1 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_02, dct8_row_coef_pack(token, 181, -181)),
        pd_128,
    ));



    let pair_13 = i32_to_i16_pair(token, match c { &mut [_, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _explicit_element, ..] => _explicit_element });
    let t2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13, dct8_row_coef_pack(token, 1567, -3784)),
        pd_2048,
    ));
    let t3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13, dct8_row_coef_pack(token, 3784, 1567)),
        pd_2048,
    ));

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t0, t3), bounds);
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t1, t2), bounds);
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t1, t2), bounds);
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t0, t3), bounds);
}



#[cfg(target_arch = "x86_64")]
#[rite]
fn dct8_1d_cols8_i16(token: Desktop64, c: &mut [__m256i; 8], bounds: VectorBounds<__m256i>) {

    let mut even = [match c { &mut [_explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }];
    dct4_1d_cols8_i16(token, &mut even, bounds);
    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = match even { [_explicit_element, ..] => _explicit_element };
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _explicit_element, ..] => _explicit_element };
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = match even { [_, _, _, _explicit_element, ..] => _explicit_element };

    let pd_2048 = _mm256_set1_epi32(2048);
    let pd_128 = _mm256_set1_epi32(128);




    let pair_17 = i32_to_i16_pair(token, match c { &mut [_, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
    let t4a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_17, dct8_row_coef_pack(token, 799, -4017)),
        pd_2048,
    ));
    let t7a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_17, dct8_row_coef_pack(token, 4017, 799)),
        pd_2048,
    ));



    let pair_53 = i32_to_i16_pair(token, match c { &mut [_, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _explicit_element, ..] => _explicit_element });
    let t5a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_53, dct8_row_coef_pack(token, 3406, -2276)),
        pd_2048,
    ));
    let t6a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_53, dct8_row_coef_pack(token, 2276, 3406)),
        pd_2048,
    ));

    let t4 = clip8(token, _mm256_add_epi32(t4a, t5a), bounds);
    let t5a_n = clip8(token, _mm256_sub_epi32(t4a, t5a), bounds);
    let t7 = clip8(token, _mm256_add_epi32(t7a, t6a), bounds);
    let t6a_n = clip8(token, _mm256_sub_epi32(t7a, t6a), bounds);




    let pair_65 = i32_to_i16_pair(token, t6a_n, t5a_n);
    let t5 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_65, dct8_row_coef_pack(token, 181, -181)),
        pd_128,
    ));
    let t6 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_65, dct8_row_coef_pack(token, 181, 181)),
        pd_128,
    ));

    let &mut [t0, ..] = c;
    let &mut [_, _, t1, ..] = c;
    let &mut [_, _, _, _, t2, ..] = c;
    let &mut [_, _, _, _, _, _, t3, ..] = c;

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t0, t7), bounds);
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t1, t6), bounds);
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t2, t5), bounds);
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t3, t4), bounds);
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t3, t4), bounds);
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t2, t5), bounds);
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t1, t6), bounds);
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t0, t7), bounds);
}



#[cfg(target_arch = "x86_64")]
#[rite]
fn dct16_1d_cols8_i16(token: Desktop64, c: &mut [__m256i; 16], bounds: VectorBounds<__m256i>) {

    let mut even = [match c { &mut [_explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }];
    dct8_1d_cols8_i16(token, &mut even, bounds);
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






    let pair_1_15 = i32_to_i16_pair(token, match c { &mut [_, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
    let t8a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_1_15, dct8_row_coef_pack(token, 401, -4076)),
        pd_2048,
    ));
    let t15a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_1_15, dct8_row_coef_pack(token, 4076, 401)),
        pd_2048,
    ));



    let pair_9_7 = i32_to_i16_pair(token, match c { &mut [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
    let t9a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_9_7, dct8_row_coef_pack(token, 3166, -2598)),
        pd_2048,
    ));
    let t14a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_9_7, dct8_row_coef_pack(token, 2598, 3166)),
        pd_2048,
    ));



    let pair_5_11 = i32_to_i16_pair(token, match c { &mut [_, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element });
    let t10a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_5_11, dct8_row_coef_pack(token, 1931, -3612)),
        pd_2048,
    ));
    let t13a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_5_11, dct8_row_coef_pack(token, 3612, 1931)),
        pd_2048,
    ));



    let pair_13_3 = i32_to_i16_pair(token, match c { &mut [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element }, match c { &mut [_, _, _, _explicit_element, ..] => _explicit_element });
    let t11a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_3, dct8_row_coef_pack(token, 3920, -1189)),
        pd_2048,
    ));
    let t12a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_3, dct8_row_coef_pack(token, 1189, 3920)),
        pd_2048,
    ));

    let t8 = clip8(token, _mm256_add_epi32(t8a, t9a), bounds);
    let mut t9 = clip8(token, _mm256_sub_epi32(t8a, t9a), bounds);
    let mut t10 = clip8(token, _mm256_sub_epi32(t11a, t10a), bounds);
    let mut t11 = clip8(token, _mm256_add_epi32(t11a, t10a), bounds);
    let mut t12 = clip8(token, _mm256_add_epi32(t12a, t13a), bounds);
    let mut t13 = clip8(token, _mm256_sub_epi32(t12a, t13a), bounds);
    let mut t14 = clip8(token, _mm256_sub_epi32(t15a, t14a), bounds);
    let t15 = clip8(token, _mm256_add_epi32(t15a, t14a), bounds);




    let pair_14_9 = i32_to_i16_pair(token, t14, t9);
    let t9a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_14_9, dct8_row_coef_pack(token, 1567, -3784)),
        pd_2048,
    ));
    let t14a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_14_9, dct8_row_coef_pack(token, 3784, 1567)),
        pd_2048,
    ));
























    let pair_13_10 = i32_to_i16_pair(token, t13, t10);
    let t10a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_10, dct8_row_coef_pack(token, -3784, -1567)),
        pd_2048,
    ));







    let t13a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_10, dct8_row_coef_pack(token, 1567, -3784)),
        pd_2048,
    ));

    let t8a_2 = clip8(token, _mm256_add_epi32(t8, t11), bounds);
    t9 = clip8(token, _mm256_add_epi32(t9a_2, t10a_2), bounds);
    t10 = clip8(token, _mm256_sub_epi32(t9a_2, t10a_2), bounds);
    let t11a_2 = clip8(token, _mm256_sub_epi32(t8, t11), bounds);
    let t12a_2 = clip8(token, _mm256_sub_epi32(t15, t12), bounds);
    t13 = clip8(token, _mm256_sub_epi32(t14a_2, t13a_2), bounds);
    t14 = clip8(token, _mm256_add_epi32(t14a_2, t13a_2), bounds);
    let t15a_2 = clip8(token, _mm256_add_epi32(t15, t12), bounds);




    let pair_13_10_f = i32_to_i16_pair(token, t13, t10);
    let t10a_new = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_10_f, dct8_row_coef_pack(token, 181, -181)),
        pd_128,
    ));
    let t13a_new = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_10_f, dct8_row_coef_pack(token, 181, 181)),
        pd_128,
    ));
    let pair_12a_11a = i32_to_i16_pair(token, t12a_2, t11a_2);
    t11 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_12a_11a, dct8_row_coef_pack(token, 181, -181)),
        pd_128,
    ));
    t12 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_12a_11a, dct8_row_coef_pack(token, 181, 181)),
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

    *{ let [_explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t0, t15a_2), bounds);
    *{ let [_, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t1, t14), bounds);
    *{ let [_, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t2, t13a_new), bounds);
    *{ let [_, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t3, t12), bounds);
    *{ let [_, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t4, t11), bounds);
    *{ let [_, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t5, t10a_new), bounds);
    *{ let [_, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t6, t9), bounds);
    *{ let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_add_epi32(t7, t8a_2), bounds);
    *{ let [_, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t7, t8a_2), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t6, t9), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t5, t10a_new), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t4, t11), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t3, t12), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t2, t13a_new), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t1, t14), bounds);
    *{ let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut *c; _explicit_part } = clip8(token, _mm256_sub_epi32(t0, t15a_2), bounds);
}










#[cfg(target_arch = "x86_64")]
#[rite]
fn dct32_1d_cols8_i16(token: Desktop64, c: &mut [__m256i; 32], bounds: VectorBounds<__m256i>) {

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
    dct16_1d_cols8_i16(token, &mut even, bounds);

    let pd_2048 = _mm256_set1_epi32(2048);
    let pd_1024 = _mm256_set1_epi32(1024);
    let pd_128 = _mm256_set1_epi32(128);

    let pair_1_31 = i32_to_i16_pair(token, in1, in31);
    let t16a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_1_31, dct8_row_coef_pack(token, 201, -4091)),
        pd_2048,
    ));
    let t31a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_1_31, dct8_row_coef_pack(token, 4091, 201)),
        pd_2048,
    ));

    let pair_17_15 = i32_to_i16_pair(token, in17, in15);
    let t17a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_17_15, dct8_row_coef_pack(token, 3035, -2751)),
        pd_2048,
    ));
    let t30a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_17_15, dct8_row_coef_pack(token, 2751, 3035)),
        pd_2048,
    ));

    let pair_9_23 = i32_to_i16_pair(token, in9, in23);
    let t18a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_9_23, dct8_row_coef_pack(token, 1751, -3703)),
        pd_2048,
    ));
    let t29a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_9_23, dct8_row_coef_pack(token, 3703, 1751)),
        pd_2048,
    ));

    let pair_25_7 = i32_to_i16_pair(token, in25, in7);
    let t19a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_25_7, dct8_row_coef_pack(token, 3857, -1380)),
        pd_2048,
    ));
    let t28a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_25_7, dct8_row_coef_pack(token, 1380, 3857)),
        pd_2048,
    ));

    let pair_5_27 = i32_to_i16_pair(token, in5, in27);
    let t20a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_5_27, dct8_row_coef_pack(token, 995, -3973)),
        pd_2048,
    ));
    let t27a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_5_27, dct8_row_coef_pack(token, 3973, 995)),
        pd_2048,
    ));

    let pair_21_11 = i32_to_i16_pair(token, in21, in11);
    let t21a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_21_11, dct8_row_coef_pack(token, 3513, -2106)),
        pd_2048,
    ));
    let t26a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_21_11, dct8_row_coef_pack(token, 2106, 3513)),
        pd_2048,
    ));

    let pair_13_19 = i32_to_i16_pair(token, in13, in19);
    let t22a = _mm256_srai_epi32::<11>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_19, dct8_row_coef_pack(token, 1220, -1645)),
        pd_1024,
    ));
    let t25a = _mm256_srai_epi32::<11>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_13_19, dct8_row_coef_pack(token, 1645, 1220)),
        pd_1024,
    ));

    let pair_29_3 = i32_to_i16_pair(token, in29, in3);
    let t23a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_29_3, dct8_row_coef_pack(token, 4052, -601)),
        pd_2048,
    ));
    let t24a = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_29_3, dct8_row_coef_pack(token, 601, 4052)),
        pd_2048,
    ));

    let mut t16 = clip8(token, _mm256_add_epi32(t16a, t17a), bounds);
    let mut t17 = clip8(token, _mm256_sub_epi32(t16a, t17a), bounds);
    let mut t18 = clip8(token, _mm256_sub_epi32(t19a, t18a), bounds);
    let t19 = clip8(token, _mm256_add_epi32(t19a, t18a), bounds);
    let t20 = clip8(token, _mm256_add_epi32(t20a, t21a), bounds);
    let mut t21 = clip8(token, _mm256_sub_epi32(t20a, t21a), bounds);
    let mut t22 = clip8(token, _mm256_sub_epi32(t23a, t22a), bounds);
    let mut t23 = clip8(token, _mm256_add_epi32(t23a, t22a), bounds);
    let mut t24 = clip8(token, _mm256_add_epi32(t24a, t25a), bounds);
    let mut t25 = clip8(token, _mm256_sub_epi32(t24a, t25a), bounds);
    let mut t26 = clip8(token, _mm256_sub_epi32(t27a, t26a), bounds);
    let t27 = clip8(token, _mm256_add_epi32(t27a, t26a), bounds);
    let t28 = clip8(token, _mm256_add_epi32(t28a, t29a), bounds);
    let mut t29 = clip8(token, _mm256_sub_epi32(t28a, t29a), bounds);
    let mut t30 = clip8(token, _mm256_sub_epi32(t31a, t30a), bounds);
    let mut t31 = clip8(token, _mm256_add_epi32(t31a, t30a), bounds);

    let pair_30_17 = i32_to_i16_pair(token, t30, t17);
    let t17a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_30_17, dct8_row_coef_pack(token, 799, -4017)),
        pd_2048,
    ));
    let t30a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_30_17, dct8_row_coef_pack(token, 4017, 799)),
        pd_2048,
    ));

    let pair_29_18 = i32_to_i16_pair(token, t29, t18);
    let t18a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_29_18, dct8_row_coef_pack(token, -4017, -799)),
        pd_2048,
    ));
    let t29a_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_29_18, dct8_row_coef_pack(token, 799, -4017)),
        pd_2048,
    ));

    let pair_26_21 = i32_to_i16_pair(token, t26, t21);
    let t21a_2 = _mm256_srai_epi32::<11>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26_21, dct8_row_coef_pack(token, 1703, -1138)),
        pd_1024,
    ));
    let t26a_2 = _mm256_srai_epi32::<11>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26_21, dct8_row_coef_pack(token, 1138, 1703)),
        pd_1024,
    ));

    let pair_25_22 = i32_to_i16_pair(token, t25, t22);
    let t22a_2 = _mm256_srai_epi32::<11>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_25_22, dct8_row_coef_pack(token, -1138, -1703)),
        pd_1024,
    ));
    let t25a_2 = _mm256_srai_epi32::<11>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_25_22, dct8_row_coef_pack(token, 1703, -1138)),
        pd_1024,
    ));

    let t16a_2 = clip8(token, _mm256_add_epi32(t16, t19), bounds);
    t17 = clip8(token, _mm256_add_epi32(t17a_2, t18a_2), bounds);
    t18 = clip8(token, _mm256_sub_epi32(t17a_2, t18a_2), bounds);
    let t19a_2 = clip8(token, _mm256_sub_epi32(t16, t19), bounds);
    let t20a_2 = clip8(token, _mm256_sub_epi32(t23, t20), bounds);
    t21 = clip8(token, _mm256_sub_epi32(t22a_2, t21a_2), bounds);
    t22 = clip8(token, _mm256_add_epi32(t22a_2, t21a_2), bounds);
    let t23a_2 = clip8(token, _mm256_add_epi32(t23, t20), bounds);
    let t24a_2 = clip8(token, _mm256_add_epi32(t24, t27), bounds);
    t25 = clip8(token, _mm256_add_epi32(t25a_2, t26a_2), bounds);
    t26 = clip8(token, _mm256_sub_epi32(t25a_2, t26a_2), bounds);
    let t27a_2 = clip8(token, _mm256_sub_epi32(t24, t27), bounds);
    let t28a_2 = clip8(token, _mm256_sub_epi32(t31, t28), bounds);
    t29 = clip8(token, _mm256_sub_epi32(t30a_2, t29a_2), bounds);
    t30 = clip8(token, _mm256_add_epi32(t30a_2, t29a_2), bounds);
    let t31a_2 = clip8(token, _mm256_add_epi32(t31, t28), bounds);

    let pair_29_18_2 = i32_to_i16_pair(token, t29, t18);
    let t18a_3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_29_18_2, dct8_row_coef_pack(token, 1567, -3784)),
        pd_2048,
    ));
    let t29a_3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_29_18_2, dct8_row_coef_pack(token, 3784, 1567)),
        pd_2048,
    ));

    let pair_28a_19a = i32_to_i16_pair(token, t28a_2, t19a_2);
    let t19_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_28a_19a, dct8_row_coef_pack(token, 1567, -3784)),
        pd_2048,
    ));
    let t28_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_28a_19a, dct8_row_coef_pack(token, 3784, 1567)),
        pd_2048,
    ));

    let pair_27a_20a = i32_to_i16_pair(token, t27a_2, t20a_2);
    let t20_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_27a_20a, dct8_row_coef_pack(token, -3784, -1567)),
        pd_2048,
    ));
    let t27_2 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_27a_20a, dct8_row_coef_pack(token, 1567, -3784)),
        pd_2048,
    ));

    let pair_26_21_2 = i32_to_i16_pair(token, t26, t21);
    let t21a_3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26_21_2, dct8_row_coef_pack(token, -3784, -1567)),
        pd_2048,
    ));
    let t26a_3 = _mm256_srai_epi32::<12>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26_21_2, dct8_row_coef_pack(token, 1567, -3784)),
        pd_2048,
    ));

    t16 = clip8(token, _mm256_add_epi32(t16a_2, t23a_2), bounds);
    let t17a_3 = clip8(token, _mm256_add_epi32(t17, t22), bounds);
    t18 = clip8(token, _mm256_add_epi32(t18a_3, t21a_3), bounds);
    let t19a_3 = clip8(token, _mm256_add_epi32(t19_2, t20_2), bounds);
    let t20a_3 = clip8(token, _mm256_sub_epi32(t19_2, t20_2), bounds);
    t21 = clip8(token, _mm256_sub_epi32(t18a_3, t21a_3), bounds);
    let t22a_3 = clip8(token, _mm256_sub_epi32(t17, t22), bounds);
    t23 = clip8(token, _mm256_sub_epi32(t16a_2, t23a_2), bounds);
    t24 = clip8(token, _mm256_sub_epi32(t31a_2, t24a_2), bounds);
    let t25a_3 = clip8(token, _mm256_sub_epi32(t30, t25), bounds);
    t26 = clip8(token, _mm256_sub_epi32(t29a_3, t26a_3), bounds);
    let t27a_3 = clip8(token, _mm256_sub_epi32(t28_2, t27_2), bounds);
    let t28a_3 = clip8(token, _mm256_add_epi32(t28_2, t27_2), bounds);
    t29 = clip8(token, _mm256_add_epi32(t29a_3, t26a_3), bounds);
    let t30a_3 = clip8(token, _mm256_add_epi32(t30, t25), bounds);
    t31 = clip8(token, _mm256_add_epi32(t31a_2, t24a_2), bounds);

    let pair_27a_20a_2 = i32_to_i16_pair(token, t27a_3, t20a_3);
    let t20_final = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_27a_20a_2, dct8_row_coef_pack(token, 181, -181)),
        pd_128,
    ));
    let t27_final = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_27a_20a_2, dct8_row_coef_pack(token, 181, 181)),
        pd_128,
    ));

    let pair_26_21_3 = i32_to_i16_pair(token, t26, t21);
    let t21a_final = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26_21_3, dct8_row_coef_pack(token, 181, -181)),
        pd_128,
    ));
    let t26a_final = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_26_21_3, dct8_row_coef_pack(token, 181, 181)),
        pd_128,
    ));

    let pair_25a_22a = i32_to_i16_pair(token, t25a_3, t22a_3);
    let t22_final = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_25a_22a, dct8_row_coef_pack(token, 181, -181)),
        pd_128,
    ));
    let t25_final = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_25a_22a, dct8_row_coef_pack(token, 181, 181)),
        pd_128,
    ));

    let pair_24_23 = i32_to_i16_pair(token, t24, t23);
    let t23a_3 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_24_23, dct8_row_coef_pack(token, 181, -181)),
        pd_128,
    ));
    let t24a_3 = _mm256_srai_epi32::<8>(_mm256_add_epi32(
        _mm256_madd_epi16(pair_24_23, dct8_row_coef_pack(token, 181, 181)),
        pd_128,
    ));

    let [t0, t1, t2, t3, t4, t5, t6, t7, t8, t9, t10, t11, t12, t13, t14, t15] = even;

    *c = [
        clip8(token, _mm256_add_epi32(t0, t31), bounds),
        clip8(token, _mm256_add_epi32(t1, t30a_3), bounds),
        clip8(token, _mm256_add_epi32(t2, t29), bounds),
        clip8(token, _mm256_add_epi32(t3, t28a_3), bounds),
        clip8(token, _mm256_add_epi32(t4, t27_final), bounds),
        clip8(token, _mm256_add_epi32(t5, t26a_final), bounds),
        clip8(token, _mm256_add_epi32(t6, t25_final), bounds),
        clip8(token, _mm256_add_epi32(t7, t24a_3), bounds),
        clip8(token, _mm256_add_epi32(t8, t23a_3), bounds),
        clip8(token, _mm256_add_epi32(t9, t22_final), bounds),
        clip8(token, _mm256_add_epi32(t10, t21a_final), bounds),
        clip8(token, _mm256_add_epi32(t11, t20_final), bounds),
        clip8(token, _mm256_add_epi32(t12, t19a_3), bounds),
        clip8(token, _mm256_add_epi32(t13, t18), bounds),
        clip8(token, _mm256_add_epi32(t14, t17a_3), bounds),
        clip8(token, _mm256_add_epi32(t15, t16), bounds),
        clip8(token, _mm256_sub_epi32(t15, t16), bounds),
        clip8(token, _mm256_sub_epi32(t14, t17a_3), bounds),
        clip8(token, _mm256_sub_epi32(t13, t18), bounds),
        clip8(token, _mm256_sub_epi32(t12, t19a_3), bounds),
        clip8(token, _mm256_sub_epi32(t11, t20_final), bounds),
        clip8(token, _mm256_sub_epi32(t10, t21a_final), bounds),
        clip8(token, _mm256_sub_epi32(t9, t22_final), bounds),
        clip8(token, _mm256_sub_epi32(t8, t23a_3), bounds),
        clip8(token, _mm256_sub_epi32(t7, t24a_3), bounds),
        clip8(token, _mm256_sub_epi32(t6, t25_final), bounds),
        clip8(token, _mm256_sub_epi32(t5, t26a_final), bounds),
        clip8(token, _mm256_sub_epi32(t4, t27_final), bounds),
        clip8(token, _mm256_sub_epi32(t3, t28a_3), bounds),
        clip8(token, _mm256_sub_epi32(t2, t29), bounds),
        clip8(token, _mm256_sub_epi32(t1, t30a_3), bounds),
        clip8(token, _mm256_sub_epi32(t0, t31), bounds),
    ];
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
        match narrow {
            true => dct32_1d_cols8_i16(active_token, values, limits),
            false => dct32_1d_cols8(active_token, values, limits),
        }
    })
}
