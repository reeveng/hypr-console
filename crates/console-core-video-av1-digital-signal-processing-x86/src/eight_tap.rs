//! AVX2 and AVX-512 row kernels for eight-bit motion compensation.

use archmage::intrinsics::x86_64 as memory;
use archmage::{Desktop64, Server64, arcane, rite};
use console_core_pixel_buffers::RowLayoutError;
use console_core_video_av1_digital_signal_processing::{
  MotionCompensation, MotionScratch, Prediction, RowFilters, SCRATCH_COLUMNS, VerticalInput,
  apply, copy_pixels, copy_predictors, scalar_vertical_put,
};
use console_core_video_interpolation::round_shift;
use core::arch::x86_64::*;
use core::num::Wrapping;

#[inline]
pub(super) fn try_convolve8<C: Copy + Into<i32>>(
  filter: &[C; 8],
  mut sample_at: impl FnMut(u32) -> Result<i32, RowLayoutError>,
) -> Result<i32, RowLayoutError> {
  let mut sum = 0_i32;

  for (ordinal, &coefficient) in (0_u32..8).zip(filter) {
    let sample = sample_at(ordinal)?;
    let weight = Into::<i32>::into(coefficient);
    sum = sum.wrapping_add(weight.wrapping_mul(sample));
  }

  Ok(sum)
}

#[inline]
fn pack_byte_tap_pairs(
  [a, b, c, d, e, f, g, h]: [i8; 8],
) -> Result<[i16; 4], console_core_never::Never> {
  Ok([[a, b], [c, d], [e, f], [g, h]].map(|pair| i16::from_le_bytes(pair.map(i8::cast_unsigned))))
}

#[inline]
pub(super) fn horizontal_windows<Sample>(
  input: &[Sample; 23],
) -> Result<[&[Sample; 16]; 8], RowLayoutError> {
  let mut windows = input.array_windows::<16>();
  let first = windows.next().ok_or(RowLayoutError::Bounds)?;
  let second = windows.next().ok_or(RowLayoutError::Bounds)?;
  let third = windows.next().ok_or(RowLayoutError::Bounds)?;
  let fourth = windows.next().ok_or(RowLayoutError::Bounds)?;
  let fifth = windows.next().ok_or(RowLayoutError::Bounds)?;
  let sixth = windows.next().ok_or(RowLayoutError::Bounds)?;
  let seventh = windows.next().ok_or(RowLayoutError::Bounds)?;
  let eighth = windows.next().ok_or(RowLayoutError::Bounds)?;
  Ok([first, second, third, fourth, fifth, sixth, seventh, eighth])
}

#[rite]
fn horizontal_byte_sums(
  _token: Desktop64,
  input: &[u8; 23],
  [
    coefficient_01,
    coefficient_23,
    coefficient_45,
    coefficient_67,
  ]: [__m256i; 4],
) -> Result<__m256i, RowLayoutError> {
  let [first, second, third, fourth, fifth, sixth, seventh, eighth] = horizontal_windows(input)?;
  let a = memory::_mm_loadu_si128(first);
  let b = memory::_mm_loadu_si128(second);
  let c = memory::_mm_loadu_si128(third);
  let d = memory::_mm_loadu_si128(fourth);
  let e = memory::_mm_loadu_si128(fifth);
  let f = memory::_mm_loadu_si128(sixth);
  let g = memory::_mm_loadu_si128(seventh);
  let h = memory::_mm_loadu_si128(eighth);
  let pair_01 = _mm256_set_m128i(_mm_unpackhi_epi8(a, b), _mm_unpacklo_epi8(a, b));
  let pair_23 = _mm256_set_m128i(_mm_unpackhi_epi8(c, d), _mm_unpacklo_epi8(c, d));
  let pair_45 = _mm256_set_m128i(_mm_unpackhi_epi8(e, f), _mm_unpacklo_epi8(e, f));
  let pair_67 = _mm256_set_m128i(_mm_unpackhi_epi8(g, h), _mm_unpacklo_epi8(g, h));
  let products_01 = _mm256_maddubs_epi16(pair_01, coefficient_01);
  let products_23 = _mm256_maddubs_epi16(pair_23, coefficient_23);
  let products_45 = _mm256_maddubs_epi16(pair_45, coefficient_45);
  let products_67 = _mm256_maddubs_epi16(pair_67, coefficient_67);
  let first_half = _mm256_add_epi16(products_01, products_23);
  let third_pair = _mm256_add_epi16(first_half, products_45);
  Ok(_mm256_add_epi16(third_pair, products_67))
}

#[rite]
fn h_filter_8tap_8bpc_avx2_inner(
  token: Desktop64,
  dst: &mut [i16],
  src: &[u8],
  width: u32,
  filter: &[i8; 8],
  sh: u8,
) -> Result<(), RowLayoutError> {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let Ok([pair_01, pair_23, pair_45, pair_67]) = pack_byte_tap_pairs(*filter);
  let coeff_01 = _mm256_set1_epi16(pair_01);
  let coeff_23 = _mm256_set1_epi16(pair_23);
  let coeff_45 = _mm256_set1_epi16(pair_45);
  let coeff_67 = _mm256_set1_epi16(pair_67);

  let rnd = _mm256_set1_epi16(::core::primitive::i16::wrapping_shr(
    i16::wrapping_shl(1i16, u32::from(sh)),
    1_u32,
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let window = src_2
      .get(column_index..)
      .and_then(<[u8]>::first_chunk::<23>)
      .ok_or(RowLayoutError::Bounds)?;
    let sum = horizontal_byte_sums(token, window, [coeff_01, coeff_23, coeff_45, coeff_67])?;

    let shift_count = _mm_cvtsi32_si128(
      <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
    );
    let result = _mm256_sra_epi16(_mm256_add_epi16(sum, rnd), shift_count);

    let target = dst_2
      .get_mut(column_index..)
      .and_then(<[i16]>::first_chunk_mut::<16>)
      .ok_or(RowLayoutError::Bounds)?;
    memory::_mm256_storeu_si256(target, result);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value =
        <[_]>::get(&*src_2, column_index.wrapping_add(tap)).ok_or(RowLayoutError::Bounds)?;
      Ok(i32::from(*checked_value))
    })?;

    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(RowLayoutError::Bounds)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(round_shift(
      sum,
      u32::from(sh),
    )) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[rite]
fn h_filter_8tap_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [i16],
  src: &[u8],
  width: u32,
  filter: &[i8; 8],
  sh: u8,
) -> Result<(), RowLayoutError> {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let Ok([pair_01, pair_23, pair_45, pair_67]) = pack_byte_tap_pairs(*filter);
  let coeff_01 = _mm512_set1_epi16(pair_01);
  let coeff_23 = _mm512_set1_epi16(pair_23);
  let coeff_45 = _mm512_set1_epi16(pair_45);
  let coeff_67 = _mm512_set1_epi16(pair_67);

  let rnd = _mm512_set1_epi16(::core::primitive::i16::wrapping_shr(
    i16::wrapping_shl(1i16, u32::from(sh)),
    1_u32,
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(32));

  for col in (0..vector_columns).step_by(32) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s0 = memory::_mm256_loadu_si256(checked_value_2);
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(33)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s1 = memory::_mm256_loadu_si256(checked_value_3);
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(2)).0..(Wrapping(column_index) + Wrapping(34)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s2 = memory::_mm256_loadu_si256(checked_value_4);
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(3)).0..(Wrapping(column_index) + Wrapping(35)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s3 = memory::_mm256_loadu_si256(checked_value_5);
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(4)).0..(Wrapping(column_index) + Wrapping(36)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s4 = memory::_mm256_loadu_si256(checked_value_6);
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(5)).0..(Wrapping(column_index) + Wrapping(37)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s5 = memory::_mm256_loadu_si256(checked_value_7);
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(6)).0..(Wrapping(column_index) + Wrapping(38)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s6 = memory::_mm256_loadu_si256(checked_value_8);
    let checked_value_9 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(7)).0..(Wrapping(column_index) + Wrapping(39)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s7 = memory::_mm256_loadu_si256(checked_value_9);

    let lo01 = _mm256_unpacklo_epi8(s0, s1);
    let hi01 = _mm256_unpackhi_epi8(s0, s1);
    let p01 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo01), hi01);

    let lo23 = _mm256_unpacklo_epi8(s2, s3);
    let hi23 = _mm256_unpackhi_epi8(s2, s3);
    let p23 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo23), hi23);

    let lo45 = _mm256_unpacklo_epi8(s4, s5);
    let hi45 = _mm256_unpackhi_epi8(s4, s5);
    let p45 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo45), hi45);

    let lo67 = _mm256_unpacklo_epi8(s6, s7);
    let hi67 = _mm256_unpackhi_epi8(s6, s7);
    let p67 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo67), hi67);

    let ma01 = _mm512_maddubs_epi16(p01, coeff_01);
    let ma23 = _mm512_maddubs_epi16(p23, coeff_23);
    let ma45 = _mm512_maddubs_epi16(p45, coeff_45);
    let ma67 = _mm512_maddubs_epi16(p67, coeff_67);

    let sum = _mm512_add_epi16(_mm512_add_epi16(ma01, ma23), _mm512_add_epi16(ma45, ma67));

    let shift_count = _mm_cvtsi32_si128(
      <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
    );
    let result = _mm512_sra_epi16(_mm512_add_epi16(sum, rnd), shift_count);

    let result_2 = _mm512_shuffle_i64x2::<0b11_01_10_00>(result, result);

    let checked_value_10 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    memory::_mm512_storeu_si512(checked_value_10, result_2);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value =
        <[_]>::get(&*src_2, column_index.wrapping_add(tap)).ok_or(RowLayoutError::Bounds)?;
      Ok(i32::from(*checked_value))
    })?;

    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(RowLayoutError::Bounds)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(round_shift(
      sum,
      u32::from(sh),
    )) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[rite]
fn v_filter_8tap_8bpc_avx2_inner(
  _token: Desktop64,
  dst: &mut [u8],
  mid: &[[i16; SCRATCH_COLUMNS]],
  width: u32,
  filter: &[i8; 8],
  sh: u8,
  max: i32,
) -> Result<(), RowLayoutError> {
  let dst_2 = &mut *dst;

  let rnd = _mm256_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1i32, u32::from(sh)),
    1_u32,
  ));
  let zero = _mm256_setzero_si256();
  let _max_vec = _mm256_set1_epi16(
    match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(max) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    },
  );

  let c0 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, 0).ok_or(RowLayoutError::Bounds)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m0 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &(*{
        let checked_value_37 = <[_]>::get(&*mid, 1).ok_or(RowLayoutError::Bounds)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m1 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &(*{
        let checked_value_38 = <[_]>::get(&*mid, 2).ok_or(RowLayoutError::Bounds)?;
        checked_value_38
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m2 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &(*{
        let checked_value_39 = <[_]>::get(&*mid, 3).ok_or(RowLayoutError::Bounds)?;
        checked_value_39
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m3 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &(*{
        let checked_value_40 = <[_]>::get(&*mid, 4).ok_or(RowLayoutError::Bounds)?;
        checked_value_40
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m4 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &(*{
        let checked_value_41 = <[_]>::get(&*mid, 5).ok_or(RowLayoutError::Bounds)?;
        checked_value_41
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m5 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &(*{
        let checked_value_42 = <[_]>::get(&*mid, 6).ok_or(RowLayoutError::Bounds)?;
        checked_value_42
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m6 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_8));
    let checked_value_10 = <[_]>::get(
      &(*{
        let checked_value_43 = <[_]>::get(&*mid, 7).ok_or(RowLayoutError::Bounds)?;
        checked_value_43
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m7 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_10));

    let mut sum = _mm256_mullo_epi32(m0, c0);
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m1, c1));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m2, c2));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m3, c3));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m4, c4));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m5, c5));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m6, c6));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m7, c7));

    let shift_count = _mm_cvtsi32_si128(
      <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
    );
    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);

    let clamped = _mm256_min_epi32(_mm256_max_epi32(shifted, zero), _mm256_set1_epi32(max));

    let packed16 = _mm256_packs_epi32(clamped, clamped);
    let packed16_2 = _mm256_permute4x64_epi64(packed16, 0b11011000);
    let packed8 = _mm256_packus_epi16(packed16_2, packed16_2);

    let result_64 = _mm256_extract_epi64(packed8, 0);

    {
      let explicit_byte_copy_destination = <[_]>::get_mut(&mut *dst_2, column_index..)
        .and_then(<[_]>::first_chunk_mut::<8>)
        .ok_or(RowLayoutError::Bounds)?;
      *explicit_byte_copy_destination = result_64.to_le_bytes();
    };
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value_2 = <[_]>::get(
        &(*{
          let checked_value_9 = <[_]>::get(&*mid, tap).ok_or(RowLayoutError::Bounds)?;
          checked_value_9
        }),
        column_index,
      )
      .ok_or(RowLayoutError::Bounds)?;
      Ok(i32::from(*checked_value_2))
    })?;

    let output_sample = round_shift(sum, u32::from(sh)).clamp(0, max);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(RowLayoutError::Bounds)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(output_sample)
    {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[rite]
fn v_filter_8tap_8bpc_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  mid: &[[i16; SCRATCH_COLUMNS]],
  width: u32,
  filter: &[i8; 8],
  sh: u8,
  max: i32,
) -> Result<(), RowLayoutError> {
  let dst_2 = &mut *dst;

  let rnd = _mm512_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1i32, u32::from(sh)),
    1_u32,
  ));
  let zero = _mm512_setzero_si512();
  let max_v = _mm512_set1_epi32(max);

  let c0 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, 0).ok_or(RowLayoutError::Bounds)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m0 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &(*{
        let checked_value_37 = <[_]>::get(&*mid, 1).ok_or(RowLayoutError::Bounds)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m1 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &(*{
        let checked_value_38 = <[_]>::get(&*mid, 2).ok_or(RowLayoutError::Bounds)?;
        checked_value_38
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m2 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &(*{
        let checked_value_39 = <[_]>::get(&*mid, 3).ok_or(RowLayoutError::Bounds)?;
        checked_value_39
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m3 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &(*{
        let checked_value_40 = <[_]>::get(&*mid, 4).ok_or(RowLayoutError::Bounds)?;
        checked_value_40
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m4 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &(*{
        let checked_value_41 = <[_]>::get(&*mid, 5).ok_or(RowLayoutError::Bounds)?;
        checked_value_41
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m5 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &(*{
        let checked_value_42 = <[_]>::get(&*mid, 6).ok_or(RowLayoutError::Bounds)?;
        checked_value_42
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m6 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_8));
    let checked_value_10 = <[_]>::get(
      &(*{
        let checked_value_43 = <[_]>::get(&*mid, 7).ok_or(RowLayoutError::Bounds)?;
        checked_value_43
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m7 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_10));

    let mut sum = _mm512_mullo_epi32(m0, c0);
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m1, c1));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m2, c2));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m3, c3));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m4, c4));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m5, c5));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m6, c6));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m7, c7));

    let shift_count = _mm_cvtsi32_si128(
      <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
    );
    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let clamped = _mm512_min_epi32(_mm512_max_epi32(shifted, zero), max_v);

    let packed = _mm512_cvtusepi32_epi8(clamped);

    let checked_value_11 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&mut [u8; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    memory::_mm_storeu_si128(checked_value_11, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value_2 = <[_]>::get(
        &(*{
          let checked_value_9 = <[_]>::get(&*mid, tap).ok_or(RowLayoutError::Bounds)?;
          checked_value_9
        }),
        column_index,
      )
      .ok_or(RowLayoutError::Bounds)?;
      Ok(i32::from(*checked_value_2))
    })?;

    let output_sample = round_shift(sum, u32::from(sh)).clamp(0, max);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(RowLayoutError::Bounds)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(output_sample)
    {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[rite]
fn h_filter_8tap_8bpc_put_avx2_inner(
  token: Desktop64,
  dst: &mut [u8],
  src: &[u8],
  width: u32,
  filter: &[i8; 8],
) -> Result<(), RowLayoutError> {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let Ok([pair_01, pair_23, pair_45, pair_67]) = pack_byte_tap_pairs(*filter);
  let coeff_01 = _mm256_set1_epi16(pair_01);
  let coeff_23 = _mm256_set1_epi16(pair_23);
  let coeff_45 = _mm256_set1_epi16(pair_45);
  let coeff_67 = _mm256_set1_epi16(pair_67);

  let rnd = _mm256_set1_epi16(34);
  let zero = _mm256_setzero_si256();

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let window = src_2
      .get(column_index..)
      .and_then(<[u8]>::first_chunk::<23>)
      .ok_or(RowLayoutError::Bounds)?;
    let sum = horizontal_byte_sums(token, window, [coeff_01, coeff_23, coeff_45, coeff_67])?;

    let shift_count = _mm_cvtsi32_si128(6);
    let shifted = _mm256_sra_epi16(_mm256_add_epi16(sum, rnd), shift_count);
    let clamped = _mm256_max_epi16(_mm256_min_epi16(shifted, _mm256_set1_epi16(255)), zero);

    let packed = _mm256_packus_epi16(clamped, clamped);
    let packed_2 = _mm256_permute4x64_epi64(packed, 0b11011000);

    let target = dst_2
      .get_mut(column_index..)
      .and_then(<[u8]>::first_chunk_mut::<16>)
      .ok_or(RowLayoutError::Bounds)?;
    memory::_mm_storeu_si128(target, _mm256_castsi256_si128(packed_2));
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value =
        <[_]>::get(&*src_2, column_index.wrapping_add(tap)).ok_or(RowLayoutError::Bounds)?;
      Ok(i32::from(*checked_value))
    })?;

    let output_sample =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(sum, 34), 6_u32).clamp(0, 255);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(RowLayoutError::Bounds)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(output_sample)
    {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[rite]
fn h_filter_8tap_8bpc_put_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  src: &[u8],
  width: u32,
  filter: &[i8; 8],
) -> Result<(), RowLayoutError> {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let Ok([pair_01, pair_23, pair_45, pair_67]) = pack_byte_tap_pairs(*filter);
  let coeff_01 = _mm512_set1_epi16(pair_01);
  let coeff_23 = _mm512_set1_epi16(pair_23);
  let coeff_45 = _mm512_set1_epi16(pair_45);
  let coeff_67 = _mm512_set1_epi16(pair_67);

  let rnd = _mm512_set1_epi16(34);
  let zero = _mm512_setzero_si512();
  let max_v = _mm512_set1_epi16(255);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(32));

  for col in (0..vector_columns).step_by(32) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s0 = memory::_mm256_loadu_si256(checked_value_2);
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(1)).0..(Wrapping(column_index) + Wrapping(33)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s1 = memory::_mm256_loadu_si256(checked_value_3);
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(2)).0..(Wrapping(column_index) + Wrapping(34)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s2 = memory::_mm256_loadu_si256(checked_value_4);
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(3)).0..(Wrapping(column_index) + Wrapping(35)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s3 = memory::_mm256_loadu_si256(checked_value_5);
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(4)).0..(Wrapping(column_index) + Wrapping(36)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s4 = memory::_mm256_loadu_si256(checked_value_6);
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(5)).0..(Wrapping(column_index) + Wrapping(37)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s5 = memory::_mm256_loadu_si256(checked_value_7);
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(6)).0..(Wrapping(column_index) + Wrapping(38)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s6 = memory::_mm256_loadu_si256(checked_value_8);
    let checked_value_9 = <[_]>::get(
      &*src_2,
      (Wrapping(column_index) + Wrapping(7)).0..(Wrapping(column_index) + Wrapping(39)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let s7 = memory::_mm256_loadu_si256(checked_value_9);

    let lo01 = _mm256_unpacklo_epi8(s0, s1);
    let hi01 = _mm256_unpackhi_epi8(s0, s1);
    let p01 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo01), hi01);

    let lo23 = _mm256_unpacklo_epi8(s2, s3);
    let hi23 = _mm256_unpackhi_epi8(s2, s3);
    let p23 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo23), hi23);

    let lo45 = _mm256_unpacklo_epi8(s4, s5);
    let hi45 = _mm256_unpackhi_epi8(s4, s5);
    let p45 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo45), hi45);

    let lo67 = _mm256_unpacklo_epi8(s6, s7);
    let hi67 = _mm256_unpackhi_epi8(s6, s7);
    let p67 = _mm512_inserti64x4::<1>(_mm512_castsi256_si512(lo67), hi67);

    let ma01 = _mm512_maddubs_epi16(p01, coeff_01);
    let ma23 = _mm512_maddubs_epi16(p23, coeff_23);
    let ma45 = _mm512_maddubs_epi16(p45, coeff_45);
    let ma67 = _mm512_maddubs_epi16(p67, coeff_67);

    let sum = _mm512_add_epi16(_mm512_add_epi16(ma01, ma23), _mm512_add_epi16(ma45, ma67));

    let shift_count = _mm_cvtsi32_si128(6);
    let shifted = _mm512_sra_epi16(_mm512_add_epi16(sum, rnd), shift_count);
    let clamped = _mm512_max_epi16(_mm512_min_epi16(shifted, max_v), zero);

    let clamped_2 = _mm512_shuffle_i64x2::<0b11_01_10_00>(clamped, clamped);
    let packed = _mm512_cvtusepi16_epi8(clamped_2);

    let checked_value_10 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(32)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&mut [u8; 32]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    memory::_mm256_storeu_si256(checked_value_10, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value =
        <[_]>::get(&*src_2, column_index.wrapping_add(tap)).ok_or(RowLayoutError::Bounds)?;
      Ok(i32::from(*checked_value))
    })?;

    let output_sample =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(sum, 34), 6_u32).clamp(0, 255);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(RowLayoutError::Bounds)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(output_sample)
    {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[rite]
fn v_filter_8tap_8bpc_direct_avx2_inner(
  _token: Desktop64,
  dst: &mut [u8],
  src: &[u8],
  src_stride: i64,
  width: u32,
  filter: &[i8; 8],
) -> Result<(), RowLayoutError> {
  let dst_2 = &mut *dst;
  let src_2 = &*src;
  let pitch = u64::try_from(src_stride).map_err(|_| RowLayoutError::Bounds)?;
  let Ok(stride) = console_core_number_conversion::index(pitch);
  let c0 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let rnd = _mm256_set1_epi32(32);
  let zero = _mm256_setzero_si256();
  let max = _mm256_set1_epi32(255);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)?;
    let p0 = _mm256_cvtepu8_epi32(memory::_mm_loadu_si64(
      <&[u8; 8]>::try_from(checked_value_2).map_err(|_| RowLayoutError::Bounds)?,
    ));
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping(stride) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping(stride) + Wrapping(column_index)).0) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)?;
    let p1 = _mm256_cvtepu8_epi32(memory::_mm_loadu_si64(
      <&[u8; 8]>::try_from(checked_value_3).map_err(|_| RowLayoutError::Bounds)?,
    ));
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(2) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(2) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(RowLayoutError::Bounds)?;
    let p2 = _mm256_cvtepu8_epi32(memory::_mm_loadu_si64(
      <&[u8; 8]>::try_from(checked_value_4).map_err(|_| RowLayoutError::Bounds)?,
    ));
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(3) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(3) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(RowLayoutError::Bounds)?;
    let p3 = _mm256_cvtepu8_epi32(memory::_mm_loadu_si64(
      <&[u8; 8]>::try_from(checked_value_5).map_err(|_| RowLayoutError::Bounds)?,
    ));
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(4) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(4) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(RowLayoutError::Bounds)?;
    let p4 = _mm256_cvtepu8_epi32(memory::_mm_loadu_si64(
      <&[u8; 8]>::try_from(checked_value_6).map_err(|_| RowLayoutError::Bounds)?,
    ));
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(5) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(5) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(RowLayoutError::Bounds)?;
    let p5 = _mm256_cvtepu8_epi32(memory::_mm_loadu_si64(
      <&[u8; 8]>::try_from(checked_value_7).map_err(|_| RowLayoutError::Bounds)?,
    ));
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(6) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(6) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(RowLayoutError::Bounds)?;
    let p6 = _mm256_cvtepu8_epi32(memory::_mm_loadu_si64(
      <&[u8; 8]>::try_from(checked_value_8).map_err(|_| RowLayoutError::Bounds)?,
    ));
    let checked_value_9 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(7) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(7) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(8))
        .0,
    )
    .ok_or(RowLayoutError::Bounds)?;
    let p7 = _mm256_cvtepu8_epi32(memory::_mm_loadu_si64(
      <&[u8; 8]>::try_from(checked_value_9).map_err(|_| RowLayoutError::Bounds)?,
    ));

    let mut sum = _mm256_mullo_epi32(p0, c0);
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p1, c1));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p2, c2));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p3, c3));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p4, c4));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p5, c5));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p6, c6));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(p7, c7));

    let shift_count = _mm_cvtsi32_si128(6);
    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);
    let clamped = _mm256_min_epi32(_mm256_max_epi32(shifted, zero), max);

    let packed16 = _mm256_packs_epi32(clamped, clamped);
    let packed16_2 = _mm256_permute4x64_epi64(packed16, 0b11011000);
    let packed8 = _mm256_packus_epi16(packed16_2, packed16_2);

    let result_64 = _mm256_extract_epi64(packed8, 0);

    {
      let explicit_byte_copy_destination = <[_]>::get_mut(&mut *dst_2, column_index..)
        .and_then(<[_]>::first_chunk_mut::<8>)
        .ok_or(RowLayoutError::Bounds)?;
      *explicit_byte_copy_destination = result_64.to_le_bytes();
    };
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let ::core::result::Result::Ok(offset) =
        ::console_core_number_conversion::fixed_index(tap.wrapping_mul(stride));
      let checked_value = <[_]>::get(
        &*src_2,
        column_index.wrapping_add(match ::console_core_number_conversion::index(offset) {
          ::core::result::Result::Ok(native_position) => native_position,
        }),
      )
      .ok_or(RowLayoutError::Bounds)?;
      Ok(i32::from(*checked_value))
    })?;

    let output_sample =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(sum, 32), 6_u32).clamp(0, 255);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(RowLayoutError::Bounds)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(output_sample)
    {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[rite]
fn v_filter_8tap_8bpc_direct_avx512_inner(
  _token: Server64,
  dst: &mut [u8],
  src: &[u8],
  src_stride: i64,
  width: u32,
  filter: &[i8; 8],
) -> Result<(), RowLayoutError> {
  let dst_2 = &mut *dst;
  let src_2 = &*src;

  let c0 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let rnd = _mm512_set1_epi32(32);
  let zero = _mm512_setzero_si512();
  let max = _mm512_set1_epi32(255);

  let pitch = u64::try_from(src_stride).map_err(|_| RowLayoutError::Bounds)?;
  let Ok(stride) = console_core_number_conversion::index(pitch);

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &*src_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let p0 = _mm512_cvtepu8_epi32(memory::_mm_loadu_si128(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &*src_2,
      (Wrapping(stride) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping(stride) + Wrapping(column_index)).0) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let p1 = _mm512_cvtepu8_epi32(memory::_mm_loadu_si128(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(2) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(2) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let p2 = _mm512_cvtepu8_epi32(memory::_mm_loadu_si128(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(3) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(3) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let p3 = _mm512_cvtepu8_epi32(memory::_mm_loadu_si128(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(4) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(4) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let p4 = _mm512_cvtepu8_epi32(memory::_mm_loadu_si128(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(5) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(5) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let p5 = _mm512_cvtepu8_epi32(memory::_mm_loadu_si128(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(6) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(6) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let p6 = _mm512_cvtepu8_epi32(memory::_mm_loadu_si128(checked_value_8));
    let checked_value_9 = <[_]>::get(
      &*src_2,
      (Wrapping((Wrapping(7) * Wrapping(stride)).0) + Wrapping(column_index)).0
        ..(Wrapping((Wrapping((Wrapping(7) * Wrapping(stride)).0) + Wrapping(column_index)).0)
          + Wrapping(16))
        .0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[u8; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let p7 = _mm512_cvtepu8_epi32(memory::_mm_loadu_si128(checked_value_9));

    let mut sum = _mm512_mullo_epi32(p0, c0);
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p1, c1));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p2, c2));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p3, c3));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p4, c4));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p5, c5));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p6, c6));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(p7, c7));

    let shift_count = _mm_cvtsi32_si128(6);
    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);
    let clamped = _mm512_min_epi32(_mm512_max_epi32(shifted, zero), max);

    let packed = _mm512_cvtusepi32_epi8(clamped);

    let checked_value_10 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&mut [u8; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    memory::_mm_storeu_si128(checked_value_10, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let ::core::result::Result::Ok(offset) =
        ::console_core_number_conversion::fixed_index(tap.wrapping_mul(stride));
      let checked_value = <[_]>::get(
        &*src_2,
        column_index.wrapping_add(match ::console_core_number_conversion::index(offset) {
          ::core::result::Result::Ok(native_position) => native_position,
        }),
      )
      .ok_or(RowLayoutError::Bounds)?;
      Ok(i32::from(*checked_value))
    })?;

    let output_sample =
      ::core::primitive::i32::wrapping_shr(i32::wrapping_add(sum, 32), 6_u32).clamp(0, 255);
    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(RowLayoutError::Bounds)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(output_sample)
    {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[rite]
fn v_filter_8tap_to_i16_avx2_inner(
  _token: Desktop64,
  mid: &[[i16; SCRATCH_COLUMNS]],
  dst: &mut [i16],
  width: u32,
  filter: &[i8; 8],
  sh: u8,
) -> Result<(), RowLayoutError> {
  let dst_2 = &mut *dst;
  let rnd = _mm256_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1i32, u32::from(sh)),
    1_u32,
  ));

  let c0 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm256_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(8));

  for col in (0..vector_columns).step_by(8) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, 0).ok_or(RowLayoutError::Bounds)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m0 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &(*{
        let checked_value_37 = <[_]>::get(&*mid, 1).ok_or(RowLayoutError::Bounds)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m1 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &(*{
        let checked_value_38 = <[_]>::get(&*mid, 2).ok_or(RowLayoutError::Bounds)?;
        checked_value_38
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m2 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &(*{
        let checked_value_39 = <[_]>::get(&*mid, 3).ok_or(RowLayoutError::Bounds)?;
        checked_value_39
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m3 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &(*{
        let checked_value_40 = <[_]>::get(&*mid, 4).ok_or(RowLayoutError::Bounds)?;
        checked_value_40
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m4 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &(*{
        let checked_value_41 = <[_]>::get(&*mid, 5).ok_or(RowLayoutError::Bounds)?;
        checked_value_41
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m5 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &(*{
        let checked_value_42 = <[_]>::get(&*mid, 6).ok_or(RowLayoutError::Bounds)?;
        checked_value_42
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m6 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_8));
    let checked_value_10 = <[_]>::get(
      &(*{
        let checked_value_43 = <[_]>::get(&*mid, 7).ok_or(RowLayoutError::Bounds)?;
        checked_value_43
      }),
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m7 = _mm256_cvtepi16_epi32(memory::_mm_loadu_si128(checked_value_10));

    let mut sum = _mm256_mullo_epi32(m0, c0);
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m1, c1));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m2, c2));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m3, c3));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m4, c4));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m5, c5));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m6, c6));
    sum = _mm256_add_epi32(sum, _mm256_mullo_epi32(m7, c7));

    let shift_count = _mm_cvtsi32_si128(
      <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
    );
    let shifted = _mm256_sra_epi32(_mm256_add_epi32(sum, rnd), shift_count);

    let packed = _mm256_packs_epi32(shifted, shifted);
    let packed_2 = _mm256_permute4x64_epi64(packed, 0b11011000);

    let checked_value_11 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(8)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 8]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    memory::_mm_storeu_si128(checked_value_11, _mm256_castsi256_si128(packed_2));
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value_2 = <[_]>::get(
        &(*{
          let checked_value_9 = <[_]>::get(&*mid, tap).ok_or(RowLayoutError::Bounds)?;
          checked_value_9
        }),
        column_index,
      )
      .ok_or(RowLayoutError::Bounds)?;
      Ok(i32::from(*checked_value_2))
    })?;

    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(RowLayoutError::Bounds)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(round_shift(
      sum,
      u32::from(sh),
    )) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[rite]
fn v_filter_8tap_to_i16_avx512_inner(
  _token: Server64,
  mid: &[[i16; SCRATCH_COLUMNS]],
  dst: &mut [i16],
  width: u32,
  filter: &[i8; 8],
  sh: u8,
) -> Result<(), RowLayoutError> {
  let dst_2 = &mut *dst;
  let rnd = _mm512_set1_epi32(::core::primitive::i32::wrapping_shr(
    i32::wrapping_shl(1i32, u32::from(sh)),
    1_u32,
  ));

  let c0 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[element, ..] => element,
    },
  ));
  let c1 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, element, ..] => element,
    },
  ));
  let c2 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, element, ..] => element,
    },
  ));
  let c3 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, element, ..] => element,
    },
  ));
  let c4 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c5 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c6 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));
  let c7 = _mm512_set1_epi32(<i32 as ::core::convert::From<::core::primitive::i8>>::from(
    match filter {
      &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ));

  let vector_columns = width.wrapping_sub(width.wrapping_rem(16));

  for col in (0..vector_columns).step_by(16) {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let checked_value_2 = <[_]>::get(
      &(*{
        let checked_value_36 = <[_]>::get(&*mid, 0).ok_or(RowLayoutError::Bounds)?;
        checked_value_36
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m0 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_2));
    let checked_value_3 = <[_]>::get(
      &(*{
        let checked_value_37 = <[_]>::get(&*mid, 1).ok_or(RowLayoutError::Bounds)?;
        checked_value_37
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m1 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_3));
    let checked_value_4 = <[_]>::get(
      &(*{
        let checked_value_38 = <[_]>::get(&*mid, 2).ok_or(RowLayoutError::Bounds)?;
        checked_value_38
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m2 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_4));
    let checked_value_5 = <[_]>::get(
      &(*{
        let checked_value_39 = <[_]>::get(&*mid, 3).ok_or(RowLayoutError::Bounds)?;
        checked_value_39
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m3 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_5));
    let checked_value_6 = <[_]>::get(
      &(*{
        let checked_value_40 = <[_]>::get(&*mid, 4).ok_or(RowLayoutError::Bounds)?;
        checked_value_40
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m4 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_6));
    let checked_value_7 = <[_]>::get(
      &(*{
        let checked_value_41 = <[_]>::get(&*mid, 5).ok_or(RowLayoutError::Bounds)?;
        checked_value_41
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m5 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_7));
    let checked_value_8 = <[_]>::get(
      &(*{
        let checked_value_42 = <[_]>::get(&*mid, 6).ok_or(RowLayoutError::Bounds)?;
        checked_value_42
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m6 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_8));
    let checked_value_10 = <[_]>::get(
      &(*{
        let checked_value_43 = <[_]>::get(&*mid, 7).ok_or(RowLayoutError::Bounds)?;
        checked_value_43
      }),
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&[i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    let m7 = _mm512_cvtepi16_epi32(memory::_mm256_loadu_si256(checked_value_10));

    let mut sum = _mm512_mullo_epi32(m0, c0);
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m1, c1));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m2, c2));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m3, c3));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m4, c4));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m5, c5));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m6, c6));
    sum = _mm512_add_epi32(sum, _mm512_mullo_epi32(m7, c7));

    let shift_count = _mm_cvtsi32_si128(
      <i32 as ::core::convert::From<::core::primitive::u8>>::from(sh),
    );
    let shifted = _mm512_sra_epi32(_mm512_add_epi32(sum, rnd), shift_count);

    let packed = _mm512_cvtsepi32_epi16(shifted);

    let checked_value_11 = <[_]>::get_mut(
      &mut *dst_2,
      column_index..(Wrapping(column_index) + Wrapping(16)).0,
    )
    .ok_or(RowLayoutError::Bounds)
    .and_then(|_explicit_array_source| {
      <&mut [i16; 16]>::try_from(_explicit_array_source).map_err(|_| RowLayoutError::Bounds)
    })?;
    memory::_mm256_storeu_si256(checked_value_11, packed);
  }

  for col in vector_columns..width {
    let Ok(column_index) = console_core_number_conversion::index(col);

    let sum = try_convolve8(filter, |ordinal| {
      let Ok(tap) = console_core_number_conversion::index(ordinal);
      let checked_value_2 = <[_]>::get(
        &(*{
          let checked_value_9 = <[_]>::get(&*mid, tap).ok_or(RowLayoutError::Bounds)?;
          checked_value_9
        }),
        column_index,
      )
      .ok_or(RowLayoutError::Bounds)?;
      Ok(i32::from(*checked_value_2))
    })?;

    *{
      let checked_value_18 =
        <[_]>::get_mut(&mut *dst_2, column_index).ok_or(RowLayoutError::Bounds)?;
      checked_value_18
    } = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i16>(round_shift(
      sum,
      u32::from(sh),
    )) {
      ::core::result::Result::Ok(converted_position) => converted_position,
    };
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "both x86 tiers assemble the same row operations with different feature tokens"
  )
)]
macro_rules! motion_compensation {
  ($install:ident, $put:ident, $prep:ident, $put_kernel:ident, $prep_kernel:ident, $token:ident;
    $buffered:ident, $horizontal:ident, $vertical:ident, $separable:ident, $prepare:ident) => {
    /// Installs this tier when the processor can execute its kernels.
    pub fn $install(table: &mut MotionCompensation) {
      use archmage::SimdToken as _;

      match <$token>::summon() {
        Some(_) => {
          *table = MotionCompensation {
            put: $put,
            prep: $prep,
          }
        }
        None => {}
      }
    }

    fn $put(
      prediction: Prediction<'_, u8>,
      scratch: &mut MotionScratch,
    ) -> Result<(), RowLayoutError> {
      use archmage::SimdToken as _;

      let token = match <$token>::summon() {
        Some(token) => token,
        None => return (MotionCompensation::default().put)(prediction, scratch),
      };
      $put_kernel(token, prediction, scratch)
    }

    #[arcane]
    fn $put_kernel(
      token: $token,
      prediction: Prediction<'_, u8>,
      scratch: &mut MotionScratch,
    ) -> Result<(), RowLayoutError> {
      use console_core_number_conversion::wrapped;
      apply(
        prediction,
        scratch,
        RowFilters {
          copy: copy_pixels,
          horizontal: |target: &mut [u8], input: &[u8], taps: &[i8; 8]| {
            let Ok(width) = wrapped(target.len());
            $horizontal(token, target, input, width, taps)
          },
          intermediate: |target: &mut [i16], input: &[u8], taps: &[i8; 8]| {
            let Ok(width) = wrapped(target.len());
            $buffered(token, target, input, width, taps, 2)
          },
          vertical: |target: &mut [u8],
                     input: VerticalInput<'_>,
                     taps: &[i8; 8],
                     _: &mut [[i16; SCRATCH_COLUMNS]; 8]| {
            let Ok(width) = wrapped(target.len());
            match input.forward_span()? {
              Some((pixels, pitch)) => $vertical(token, target, pixels, pitch, width, taps),
              None => scalar_vertical_put(target, input, taps),
            }
          },
          separable: |target: &mut [u8], input: &[[i16; SCRATCH_COLUMNS]], taps: &[i8; 8]| {
            let Ok(width) = wrapped(target.len());
            $separable(token, target, input, width, taps, 10, 255)
          },
        },
      )
    }

    fn $prep(
      prediction: Prediction<'_, i16>,
      scratch: &mut MotionScratch,
    ) -> Result<(), RowLayoutError> {
      use archmage::SimdToken as _;

      let token = match <$token>::summon() {
        Some(token) => token,
        None => return (MotionCompensation::default().prep)(prediction, scratch),
      };
      $prep_kernel(token, prediction, scratch)
    }

    #[arcane]
    fn $prep_kernel(
      token: $token,
      prediction: Prediction<'_, i16>,
      scratch: &mut MotionScratch,
    ) -> Result<(), RowLayoutError> {
      use console_core_number_conversion::wrapped;
      let horizontal = |target: &mut [i16], input: &[u8], taps: &[i8; 8]| {
        let Ok(width) = wrapped(target.len());
        $buffered(token, target, input, width, taps, 2)
      };
      apply(
        prediction,
        scratch,
        RowFilters {
          copy: copy_predictors,
          horizontal,
          intermediate: horizontal,
          vertical: |target: &mut [i16],
                     input: VerticalInput<'_>,
                     taps: &[i8; 8],
                     filtered: &mut [[i16; SCRATCH_COLUMNS]; 8]| {
            for (line, source) in filtered.iter_mut().zip(input.rows()?) {
              copy_predictors(line, source)?;
            }
            let Ok(width) = wrapped(target.len());
            $prepare(token, filtered, target, width, taps, 6)
          },
          separable: |target: &mut [i16], input: &[[i16; SCRATCH_COLUMNS]], taps: &[i8; 8]| {
            let Ok(width) = wrapped(target.len());
            $prepare(token, input, target, width, taps, 6)
          },
        },
      )
    }
  };
}

motion_compensation!(
  install_avx2, put_avx2, prep_avx2, put_avx2_kernel, prep_avx2_kernel, Desktop64;
  h_filter_8tap_8bpc_avx2_inner, h_filter_8tap_8bpc_put_avx2_inner,
  v_filter_8tap_8bpc_direct_avx2_inner, v_filter_8tap_8bpc_avx2_inner,
  v_filter_8tap_to_i16_avx2_inner
);
motion_compensation!(
  install_avx512, put_avx512, prep_avx512, put_avx512_kernel, prep_avx512_kernel, Server64;
  h_filter_8tap_8bpc_avx512_inner, h_filter_8tap_8bpc_put_avx512_inner,
  v_filter_8tap_8bpc_direct_avx512_inner, v_filter_8tap_8bpc_avx512_inner,
  v_filter_8tap_to_i16_avx512_inner
);
