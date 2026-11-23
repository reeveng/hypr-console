//! Safe ARM NEON implementations for Inverse Transform (ITX)
//!
//! Implements the inverse transforms for AV1 decoding.
//! Transforms convert frequency-domain coefficients back to spatial-domain pixels.
//!
//! The scalar implementations provide references for the ARM NEON fixtures.

#![allow(clippy::too_many_arguments)]
#![cfg_attr(not(feature = "unchecked"), forbid(unsafe_code))]
#![cfg_attr(feature = "unchecked", deny(unsafe_code))]
#![cfg_attr(not(all(target_arch = "aarch64", test)), allow(dead_code))]

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

use archmage::autoversion;
use std::ffi::c_int;

use crate::include::common::bitdepth::AsPrimitive;
use crate::include::common::bitdepth::BitDepth8;
use crate::include::common::bitdepth::BitDepth16;
use crate::include::common::intops::iclip;
use crate::include::dav1d::picture::PicOffset;

/// WHT 4x4 transform for 8bpc
#[autoversion]
pub(crate) fn inv_txfm_add_wht_wht_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for (y, [tmp_lane_0, tmp_lane_1, tmp_lane_2, tmp_lane_3]) in
    <[_]>::iter_mut(<[_]>::as_chunks_mut::<4>(&mut tmp).0).enumerate()
  {
    let in0 = ::core::primitive::i32::wrapping_shr(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y]),
      2_u32,
    );
    let in1 = ::core::primitive::i32::wrapping_shr(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 4]),
      2_u32,
    );
    let in2 = ::core::primitive::i32::wrapping_shr(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 8]),
      2_u32,
    );
    let in3 = ::core::primitive::i32::wrapping_shr(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 12]),
      2_u32,
    );

    let t0 = in0 + in1;
    let t2 = in2 - in3;
    let t4 = ::core::primitive::i32::wrapping_shr(t0 - t2, 1_u32);
    let t3 = t4 - in3;
    let t1 = t4 - in1;

    *tmp_lane_0 = t0 - t3;
    *tmp_lane_1 = t3;
    *tmp_lane_2 = t1;
    *tmp_lane_3 = t2 + t1;
  }

  for x in 0..4 {
    let in0 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(0, 4), x)];
    let in1 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(1, 4), x)];
    let in2 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(2, 4), x)];
    let in3 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(3, 4), x)];

    let t0 = in0 + in1;
    let t2 = in2 - in3;
    let t4 = ::core::primitive::i32::wrapping_shr(t0 - t2, 1_u32);
    let t3 = t4 - in3;
    let t1 = t4 - in1;

    tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(0, 4), x)] =
      t0 - t3;
    tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(1, 4), x)] =
      t3;
    tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(2, 4), x)] =
      t1;
    tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(3, 4), x)] =
      t2 + t1;
  }

  for y in 0..4 {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
      dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
    );

    for x in 0..4 {
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let c = tmp
        [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(y, 4), x)];
      let result = iclip(d + c, 0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

/// DCT4 1D transform constants
const DCT4_C1: i32 = 2896;
const DCT4_C2: i32 = 2896;
const DCT4_C3: i32 = 1567;
const DCT4_C4: i32 = 3784;

/// DCT4 1D transform
#[inline(always)]
#[must_use]
fn dct4_1d(in0: i32, in1: i32, in2: i32, in3: i32) -> [i32; 4] {
  let t0 = in0 + in3;
  let t1 = in1 + in2;
  let t2 = in0 - in3;
  let t3 = in1 - in2;

  let s0 = t0 + t1;
  let s1 = t0 - t1;

  let s2 = ::core::primitive::i32::wrapping_shr((t2 * 1567 + t3 * 3784) + 2048, 12_u32);
  let s3 = ::core::primitive::i32::wrapping_shr((t2 * 3784 - t3 * 1567) + 2048, 12_u32);

  [s0, s2, s1, s3]
}

/// DCT 4x4 transform for 8bpc
#[autoversion]
pub(crate) fn inv_txfm_add_dct_dct_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for (y, [tmp_lane_0, tmp_lane_1, tmp_lane_2, tmp_lane_3]) in
    <[_]>::iter_mut(<[_]>::as_chunks_mut::<4>(&mut tmp).0).enumerate()
  {
    let in0 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y]);
    let in1 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 4]);
    let in2 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 8]);
    let in3 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 12]);

    let out = dct4_1d(in0, in1, in2, in3);
    *tmp_lane_0 = match out {
      [_explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_1 = match out {
      [_, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_2 = match out {
      [_, _, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_3 = match out {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    };
  }

  for x in 0..4 {
    let in0 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(0, 4), x)];
    let in1 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(1, 4), x)];
    let in2 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(2, 4), x)];
    let in3 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(3, 4), x)];

    let out = dct4_1d(in0, in1, in2, in3);

    for y in 0..4 {
      let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
        dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
      );
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );

      let c = ::core::primitive::i32::wrapping_shr(out[y] + 8, 4_u32);
      let result = iclip(d + c, 0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

/// DCT8 1D transform constants
const COS_PI_1_16: i32 = 4017;
const COS_PI_2_16: i32 = 3784;
const COS_PI_3_16: i32 = 3406;
const COS_PI_4_16: i32 = 2896;
const COS_PI_5_16: i32 = 2276;
const COS_PI_6_16: i32 = 1567;
const COS_PI_7_16: i32 = 799;

/// DCT8 1D transform
#[inline(always)]
#[must_use]
fn dct8_1d(input: &[i32; 8]) -> [i32; 8] {
  let t0 = (match input {
    &[_explicit_element, ..] => _explicit_element,
  }) + (match input {
    &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
  });
  let t7 = (match input {
    &[_explicit_element, ..] => _explicit_element,
  }) - (match input {
    &[_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
  });
  let t1 = (match input {
    &[_, _explicit_element, ..] => _explicit_element,
  }) + (match input {
    &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
  });
  let t6 = (match input {
    &[_, _explicit_element, ..] => _explicit_element,
  }) - (match input {
    &[_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
  });
  let t2 = (match input {
    &[_, _, _explicit_element, ..] => _explicit_element,
  }) + (match input {
    &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
  });
  let t5 = (match input {
    &[_, _, _explicit_element, ..] => _explicit_element,
  }) - (match input {
    &[_, _, _, _, _, _explicit_element, ..] => _explicit_element,
  });
  let t3 = (match input {
    &[_, _, _, _explicit_element, ..] => _explicit_element,
  }) + (match input {
    &[_, _, _, _, _explicit_element, ..] => _explicit_element,
  });
  let t4 = (match input {
    &[_, _, _, _explicit_element, ..] => _explicit_element,
  }) - (match input {
    &[_, _, _, _, _explicit_element, ..] => _explicit_element,
  });

  let e0 = t0 + t3;
  let e3 = t0 - t3;
  let e1 = t1 + t2;
  let e2 = t1 - t2;

  let out0 = e0 + e1;
  let out4 = e0 - e1;
  let out2 = ::core::primitive::i32::wrapping_shr((e2 * 1567 + e3 * 3784) + 2048, 12_u32);
  let out6 = ::core::primitive::i32::wrapping_shr((e3 * 1567 - e2 * 3784) + 2048, 12_u32);

  let o0 = ::core::primitive::i32::wrapping_shr((t4 * 799 + t7 * 4017) + 2048, 12_u32);
  let o7 = ::core::primitive::i32::wrapping_shr((t7 * 799 - t4 * 4017) + 2048, 12_u32);
  let o1 = ::core::primitive::i32::wrapping_shr((t5 * 2276 + t6 * 3406) + 2048, 12_u32);
  let o6 = ::core::primitive::i32::wrapping_shr((t6 * 2276 - t5 * 3406) + 2048, 12_u32);

  let out1 = o0 + o1;
  let out3 = o7 - o6;
  let out5 = o7 + o6;
  let out7 = o0 - o1;

  [out0, out1, out2, out3, out4, out5, out6, out7]
}

/// DCT 8x8 transform for 8bpc
#[autoversion]
fn inv_txfm_add_dct_dct_8x8_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 64];

  for y in 0..8 {
    let mut input = [0i32; 8];

    for (x, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
      *input_element =
        <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + x * 8]);
    }

    let out = dct8_1d(&input);

    for x in 0..8 {
      tmp[::core::primitive::usize::wrapping_add(
        ::core::primitive::usize::wrapping_mul(y, 8),
        x,
      )] = out[x];
    }
  }

  for x in 0..8 {
    let mut input = [0i32; 8];

    for (y, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
      *input_element = tmp[y.wrapping_mul(8) + x];
    }

    let out = dct8_1d(&input);

    for y in 0..8 {
      let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
        dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
      );
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let c = ::core::primitive::i32::wrapping_shr(out[y] + 32, 6_u32);
      let result = iclip(d + c, 0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  for i in 0..64 {
    coeff[i] = 0;
  }
}

/// DCT16 1D transform
#[inline(always)]
#[must_use]
fn dct16_1d(input: &[i32; 16]) -> [i32; 16] {
  let mut t = [0i32; 16];

  for (i, input_element) in <[_]>::iter(input).take(8).enumerate() {
    t[i] = *input_element + input[(::core::num::Wrapping(15) - ::core::num::Wrapping(i)).0];
    t[(::core::num::Wrapping(15) - ::core::num::Wrapping(i)).0] =
      *input_element - input[(::core::num::Wrapping(15) - ::core::num::Wrapping(i)).0];
  }

  let even_input = [
    match t {
      [_explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
  ];
  let even_out = dct8_1d(&even_input);

  let o0 = ::core::primitive::i32::wrapping_shr(
    ((match t {
      [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }) * 401
      + (match t {
        [
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _explicit_element,
          ..,
        ] => _explicit_element,
      }) * 4076)
      + 2048,
    12_u32,
  );
  let o15 = ::core::primitive::i32::wrapping_shr(
    ((match t {
      [
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _explicit_element,
        ..,
      ] => _explicit_element,
    }) * 401
      - (match t {
        [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      }) * 4076)
      + 2048,
    12_u32,
  );
  let o1 = ::core::primitive::i32::wrapping_shr(
    ((match t {
      [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }) * 1189
      + (match t {
        [
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _,
          _explicit_element,
          ..,
        ] => _explicit_element,
      }) * 3920)
      + 2048,
    12_u32,
  );
  let o14 = ::core::primitive::i32::wrapping_shr(
    ((match t {
      [
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _explicit_element,
        ..,
      ] => _explicit_element,
    }) * 1189
      - (match t {
        [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      }) * 3920)
      + 2048,
    12_u32,
  );
  let o2 = ::core::primitive::i32::wrapping_shr(
    ((match t {
      [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }) * 1931
      + (match t {
        [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      }) * 3612)
      + 2048,
    12_u32,
  );
  let o13 = ::core::primitive::i32::wrapping_shr(
    ((match t {
      [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }) * 1931
      - (match t {
        [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      }) * 3612)
      + 2048,
    12_u32,
  );
  let o3 = ::core::primitive::i32::wrapping_shr(
    ((match t {
      [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }) * 2598
      + (match t {
        [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      }) * 3166)
      + 2048,
    12_u32,
  );
  let o12 = ::core::primitive::i32::wrapping_shr(
    ((match t {
      [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    }) * 2598
      - (match t {
        [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
      }) * 3166)
      + 2048,
    12_u32,
  );

  let a0 = o0 + o1;
  let a1 = o0 - o1;
  let a2 = o2 + o3;
  let a3 = o2 - o3;
  let a4 = o12 + o13;
  let a5 = o12 - o13;
  let a6 = o14 + o15;
  let a7 = o14 - o15;

  let b1 = ::core::primitive::i32::wrapping_shr((a1 * 1567 + a6 * 3784) + 2048, 12_u32);
  let b6 = ::core::primitive::i32::wrapping_shr((a6 * 1567 - a1 * 3784) + 2048, 12_u32);
  let b3 = ::core::primitive::i32::wrapping_shr((a3 * 3784 + a4 * 1567) + 2048, 12_u32);
  let b4 = ::core::primitive::i32::wrapping_shr((a4 * 3784 - a3 * 1567) + 2048, 12_u32);

  let mut out = [0i32; 16];
  *{
    let [_explicit_part, ..] = &mut out;
    _explicit_part
  } = match even_out {
    [_explicit_element, ..] => _explicit_element,
  };
  *{
    let [_, _explicit_part, ..] = &mut out;
    _explicit_part
  } = a0 + a2;
  *{
    let [_, _, _explicit_part, ..] = &mut out;
    _explicit_part
  } = match even_out {
    [_, _explicit_element, ..] => _explicit_element,
  };
  *{
    let [_, _, _, _explicit_part, ..] = &mut out;
    _explicit_part
  } = b1 + b3;
  *{
    let [_, _, _, _, _explicit_part, ..] = &mut out;
    _explicit_part
  } = match even_out {
    [_, _, _explicit_element, ..] => _explicit_element,
  };
  *{
    let [_, _, _, _, _, _explicit_part, ..] = &mut out;
    _explicit_part
  } = a7 + a5;
  *{
    let [_, _, _, _, _, _, _explicit_part, ..] = &mut out;
    _explicit_part
  } = match even_out {
    [_, _, _, _explicit_element, ..] => _explicit_element,
  };
  *{
    let [_, _, _, _, _, _, _, _explicit_part, ..] = &mut out;
    _explicit_part
  } = b6 + b4;
  *{
    let [_, _, _, _, _, _, _, _, _explicit_part, ..] = &mut out;
    _explicit_part
  } = match even_out {
    [_, _, _, _, _explicit_element, ..] => _explicit_element,
  };
  *{
    let [_, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut out;
    _explicit_part
  } = b6 - b4;
  *{
    let [_, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut out;
    _explicit_part
  } = match even_out {
    [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
  };
  *{
    let [_, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut out;
    _explicit_part
  } = a7 - a5;
  *{
    let [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut out;
    _explicit_part
  } = match even_out {
    [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
  };
  *{
    let [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut out;
    _explicit_part
  } = b1 - b3;
  *{
    let [_, _, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_part, ..] = &mut out;
    _explicit_part
  } = match even_out {
    [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
  };
  *{
    let [
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _,
      _explicit_part,
      ..,
    ] = &mut out;
    _explicit_part
  } = a0 - a2;

  out
}

/// DCT 16x16 transform for 8bpc
#[autoversion]
fn inv_txfm_add_dct_dct_16x16_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 256];

  for y in 0..16 {
    let mut input = [0i32; 16];

    for (x, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
      *input_element =
        <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + x * 16]);
    }

    let out = dct16_1d(&input);

    for x in 0..16 {
      tmp[::core::primitive::usize::wrapping_add(
        ::core::primitive::usize::wrapping_mul(y, 16),
        x,
      )] = out[x];
    }
  }

  for x in 0..16 {
    let mut input = [0i32; 16];

    for (y, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
      *input_element = tmp[y.wrapping_mul(16) + x];
    }

    let out = dct16_1d(&input);

    for y in 0..16 {
      let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
        dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
      );
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let c = ::core::primitive::i32::wrapping_shr(out[y] + 128, 8_u32);
      let result = iclip(d + c, 0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  for i in 0..256 {
    coeff[i] = 0;
  }
}

/// Identity 4x4 transform for 8bpc
#[autoversion]
pub(crate) fn inv_txfm_add_identity_identity_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let sqrt2 = 181i32;

  for y in 0..4 {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
      dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
    );

    for x in 0..4 {
      let c = <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        coeff[::core::primitive::usize::wrapping_add(y, x * 4)],
      );

      let scaled = ::core::primitive::i32::wrapping_shr(c * sqrt2 + 64, 7_u32) * sqrt2;
      let final_val = ::core::primitive::i32::wrapping_shr(scaled + 2048, 12_u32);
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let result = iclip(d + final_val, 0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

/// Identity 8x8 transform for 8bpc
#[autoversion]
fn inv_txfm_add_identity_identity_8x8_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  for y in 0..8 {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
      dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
    );

    for x in 0..8 {
      let c = <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        coeff[::core::primitive::usize::wrapping_add(y, x * 8)],
      );

      let final_val = ::core::primitive::i32::wrapping_shr(c + 1, 1_u32);
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let result = iclip(d + final_val, 0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  for i in 0..64 {
    coeff[i] = 0;
  }
}

/// ADST4 1D transform
#[inline(always)]
#[must_use]
fn adst4_1d(in0: i32, in1: i32, in2: i32, in3: i32) -> [i32; 4] {
  const SINPI_1_9: i32 = 1321;
  const SINPI_2_9: i32 = 2482;
  const SINPI_3_9: i32 = 3344;
  const SINPI_4_9: i32 = 3803;

  let s0 = SINPI_1_9 * in0;
  let s1 = SINPI_2_9 * in0;
  let s2 = SINPI_3_9 * in1;
  let s3 = SINPI_4_9 * in2;
  let s4 = SINPI_1_9 * in2;
  let s5 = SINPI_2_9 * in3;
  let s6 = SINPI_4_9 * in3;

  let x0 = s0 + s3 + s5;
  let x1 = s1 - s4 - s6;
  let x2 = SINPI_3_9 * (in0 - in2 + in3);
  let x3 = s2;

  let s0_2 = x0 + x3;
  let s1_2 = x1 + x3;
  let s2_2 = x2;
  let s3_2 = x0 + x1 - x3;

  [
    ::core::primitive::i32::wrapping_shr(s0_2 + 2048, 12_u32),
    ::core::primitive::i32::wrapping_shr(s1_2 + 2048, 12_u32),
    ::core::primitive::i32::wrapping_shr(s2_2 + 2048, 12_u32),
    ::core::primitive::i32::wrapping_shr(s3_2 + 2048, 12_u32),
  ]
}

/// ADST 4x4 transform for 8bpc
#[autoversion]
pub(crate) fn inv_txfm_add_adst_adst_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for (y, [tmp_lane_0, tmp_lane_1, tmp_lane_2, tmp_lane_3]) in
    <[_]>::iter_mut(<[_]>::as_chunks_mut::<4>(&mut tmp).0).enumerate()
  {
    let in0 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y]);
    let in1 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 4]);
    let in2 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 8]);
    let in3 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 12]);

    let out = adst4_1d(in0, in1, in2, in3);
    *tmp_lane_0 = match out {
      [_explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_1 = match out {
      [_, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_2 = match out {
      [_, _, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_3 = match out {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    };
  }

  for x in 0..4 {
    let in0 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(0, 4), x)];
    let in1 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(1, 4), x)];
    let in2 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(2, 4), x)];
    let in3 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(3, 4), x)];

    let out = adst4_1d(in0, in1, in2, in3);

    for y in 0..4 {
      let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
        dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
      );
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let c = ::core::primitive::i32::wrapping_shr(out[y] + 8, 4_u32);
      let result = iclip(d + c, 0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

/// FlipADST4 1D transform (ADST with input flip)
#[inline(always)]
#[must_use]
fn flipadst4_1d(in0: i32, in1: i32, in2: i32, in3: i32) -> [i32; 4] {
  adst4_1d(in3, in2, in1, in0)
}

/// FlipADST 4x4 transform for 8bpc
#[autoversion]
pub(crate) fn inv_txfm_add_flipadst_flipadst_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for (y, [tmp_lane_0, tmp_lane_1, tmp_lane_2, tmp_lane_3]) in
    <[_]>::iter_mut(<[_]>::as_chunks_mut::<4>(&mut tmp).0).enumerate()
  {
    let in0 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y]);
    let in1 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 4]);
    let in2 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 8]);
    let in3 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 12]);

    let out = flipadst4_1d(in0, in1, in2, in3);
    *tmp_lane_0 = match out {
      [_explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_1 = match out {
      [_, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_2 = match out {
      [_, _, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_3 = match out {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    };
  }

  for x in 0..4 {
    let in0 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(0, 4), x)];
    let in1 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(1, 4), x)];
    let in2 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(2, 4), x)];
    let in3 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(3, 4), x)];

    let out = flipadst4_1d(in0, in1, in2, in3);

    for y in 0..4 {
      let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
        dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
      );
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let c = ::core::primitive::i32::wrapping_shr(out[y] + 8, 4_u32);
      let result = iclip(d + c, 0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

/// DCT-ADST 4x4 transform for 8bpc (DCT on rows, ADST on columns)
#[autoversion]
pub(crate) fn inv_txfm_add_dct_adst_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for (y, [tmp_lane_0, tmp_lane_1, tmp_lane_2, tmp_lane_3]) in
    <[_]>::iter_mut(<[_]>::as_chunks_mut::<4>(&mut tmp).0).enumerate()
  {
    let in0 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y]);
    let in1 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 4]);
    let in2 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 8]);
    let in3 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 12]);

    let out = dct4_1d(in0, in1, in2, in3);
    *tmp_lane_0 = match out {
      [_explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_1 = match out {
      [_, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_2 = match out {
      [_, _, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_3 = match out {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    };
  }

  for x in 0..4 {
    let in0 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(0, 4), x)];
    let in1 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(1, 4), x)];
    let in2 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(2, 4), x)];
    let in3 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(3, 4), x)];

    let out = adst4_1d(in0, in1, in2, in3);

    for y in 0..4 {
      let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
        dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
      );
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let c = ::core::primitive::i32::wrapping_shr(out[y] + 8, 4_u32);
      let result = iclip(d + c, 0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

/// ADST-DCT 4x4 transform for 8bpc (ADST on rows, DCT on columns)
#[autoversion]
pub(crate) fn inv_txfm_add_adst_dct_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for (y, [tmp_lane_0, tmp_lane_1, tmp_lane_2, tmp_lane_3]) in
    <[_]>::iter_mut(<[_]>::as_chunks_mut::<4>(&mut tmp).0).enumerate()
  {
    let in0 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y]);
    let in1 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 4]);
    let in2 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 8]);
    let in3 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 12]);

    let out = adst4_1d(in0, in1, in2, in3);
    *tmp_lane_0 = match out {
      [_explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_1 = match out {
      [_, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_2 = match out {
      [_, _, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_3 = match out {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    };
  }

  for x in 0..4 {
    let in0 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(0, 4), x)];
    let in1 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(1, 4), x)];
    let in2 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(2, 4), x)];
    let in3 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(3, 4), x)];

    let out = dct4_1d(in0, in1, in2, in3);

    for y in 0..4 {
      let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
        dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
      );
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let c = ::core::primitive::i32::wrapping_shr(out[y] + 8, 4_u32);
      let result = iclip(d + c, 0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

/// DCT-FLIPADST 4x4 transform for 8bpc
#[autoversion]
pub(crate) fn inv_txfm_add_dct_flipadst_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for (y, [tmp_lane_0, tmp_lane_1, tmp_lane_2, tmp_lane_3]) in
    <[_]>::iter_mut(<[_]>::as_chunks_mut::<4>(&mut tmp).0).enumerate()
  {
    let in0 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y]);
    let in1 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 4]);
    let in2 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 8]);
    let in3 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 12]);

    let out = dct4_1d(in0, in1, in2, in3);
    *tmp_lane_0 = match out {
      [_explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_1 = match out {
      [_, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_2 = match out {
      [_, _, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_3 = match out {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    };
  }

  for x in 0..4 {
    let in0 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(0, 4), x)];
    let in1 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(1, 4), x)];
    let in2 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(2, 4), x)];
    let in3 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(3, 4), x)];

    let out = flipadst4_1d(in0, in1, in2, in3);

    for y in 0..4 {
      let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
        dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
      );
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let c = ::core::primitive::i32::wrapping_shr(out[y] + 8, 4_u32);
      let result = iclip(d + c, 0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

/// FLIPADST-DCT 4x4 transform for 8bpc
#[autoversion]
pub(crate) fn inv_txfm_add_flipadst_dct_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for (y, [tmp_lane_0, tmp_lane_1, tmp_lane_2, tmp_lane_3]) in
    <[_]>::iter_mut(<[_]>::as_chunks_mut::<4>(&mut tmp).0).enumerate()
  {
    let in0 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y]);
    let in1 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 4]);
    let in2 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 8]);
    let in3 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 12]);

    let out = flipadst4_1d(in0, in1, in2, in3);
    *tmp_lane_0 = match out {
      [_explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_1 = match out {
      [_, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_2 = match out {
      [_, _, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_3 = match out {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    };
  }

  for x in 0..4 {
    let in0 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(0, 4), x)];
    let in1 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(1, 4), x)];
    let in2 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(2, 4), x)];
    let in3 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(3, 4), x)];

    let out = dct4_1d(in0, in1, in2, in3);

    for y in 0..4 {
      let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
        dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
      );
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let c = ::core::primitive::i32::wrapping_shr(out[y] + 8, 4_u32);
      let result = iclip(d + c, 0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

/// ADST-FLIPADST 4x4 transform for 8bpc
#[autoversion]
pub(crate) fn inv_txfm_add_adst_flipadst_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for (y, [tmp_lane_0, tmp_lane_1, tmp_lane_2, tmp_lane_3]) in
    <[_]>::iter_mut(<[_]>::as_chunks_mut::<4>(&mut tmp).0).enumerate()
  {
    let in0 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y]);
    let in1 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 4]);
    let in2 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 8]);
    let in3 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 12]);

    let out = adst4_1d(in0, in1, in2, in3);
    *tmp_lane_0 = match out {
      [_explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_1 = match out {
      [_, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_2 = match out {
      [_, _, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_3 = match out {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    };
  }

  for x in 0..4 {
    let in0 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(0, 4), x)];
    let in1 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(1, 4), x)];
    let in2 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(2, 4), x)];
    let in3 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(3, 4), x)];

    let out = flipadst4_1d(in0, in1, in2, in3);

    for y in 0..4 {
      let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
        dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
      );
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let c = ::core::primitive::i32::wrapping_shr(out[y] + 8, 4_u32);
      let result = iclip(d + c, 0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

/// FLIPADST-ADST 4x4 transform for 8bpc
#[autoversion]
pub(crate) fn inv_txfm_add_flipadst_adst_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for (y, [tmp_lane_0, tmp_lane_1, tmp_lane_2, tmp_lane_3]) in
    <[_]>::iter_mut(<[_]>::as_chunks_mut::<4>(&mut tmp).0).enumerate()
  {
    let in0 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y]);
    let in1 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 4]);
    let in2 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 8]);
    let in3 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + 12]);

    let out = flipadst4_1d(in0, in1, in2, in3);
    *tmp_lane_0 = match out {
      [_explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_1 = match out {
      [_, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_2 = match out {
      [_, _, _explicit_element, ..] => _explicit_element,
    };
    *tmp_lane_3 = match out {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    };
  }

  for x in 0..4 {
    let in0 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(0, 4), x)];
    let in1 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(1, 4), x)];
    let in2 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(2, 4), x)];
    let in3 = tmp
      [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(3, 4), x)];

    let out = adst4_1d(in0, in1, in2, in3);

    for y in 0..4 {
      let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
        dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
      );
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let c = ::core::primitive::i32::wrapping_shr(out[y] + 8, 4_u32);
      let result = iclip(d + c, 0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

/// DCT32 1D transform
#[inline(always)]
#[must_use]
fn dct32_1d(input: &[i32; 32]) -> [i32; 32] {
  let mut t = [0i32; 32];

  for (i, input_element) in <[_]>::iter(input).take(16).enumerate() {
    t[i] = *input_element + input[(::core::num::Wrapping(31) - ::core::num::Wrapping(i)).0];
    t[(::core::num::Wrapping(31) - ::core::num::Wrapping(i)).0] =
      *input_element - input[(::core::num::Wrapping(31) - ::core::num::Wrapping(i)).0];
  }

  let even_input = [
    match t {
      [_explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [_, _, _, _, _, _, _, _, _, _, _, _, _, _explicit_element, ..] => _explicit_element,
    },
    match t {
      [
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _explicit_element,
        ..,
      ] => _explicit_element,
    },
    match t {
      [
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _,
        _explicit_element,
        ..,
      ] => _explicit_element,
    },
  ];
  let even_out = dct16_1d(&even_input);

  let c1 = 4091;
  let c3 = 4076;
  let c5 = 4017;
  let c7 = 3920;
  let c9 = 3784;
  let c11 = 3612;
  let c13 = 3406;
  let c15 = 3166;

  let mut odd = [0i32; 16];

  for i in 0..8 {
    let ::core::result::Result::Ok(idx) = ::console_core_number_conversion::fixed_index(
      ::core::primitive::usize::wrapping_add(16, i),
    );
    let o0 = t[match ::console_core_number_conversion::index(idx) {
      ::core::result::Result::Ok(native_position) => native_position,
    }];
    let o1 = t[::core::primitive::usize::wrapping_sub(31, i)];
    let cos_val = match i {
      0 => c1,
      1 => c3,
      2 => c5,
      3 => c7,
      4 => c9,
      5 => c11,
      6 => c13,
      7 => c15,
      _ => c1,
    };
    let sin_val = 4096 - ::core::primitive::i32::wrapping_div(cos_val, 16);
    odd[i] = ::core::primitive::i32::wrapping_shr((o0 * cos_val + o1 * sin_val) + 2048, 12_u32);
    odd[::core::primitive::usize::wrapping_sub(15, i)] =
      ::core::primitive::i32::wrapping_shr((o1 * cos_val - o0 * sin_val) + 2048, 12_u32);
  }

  let mut out = [0i32; 32];

  for (i, ([out_lane_0, out_lane_1], odd_element)) in
    <[_]>::iter_mut(<[_]>::as_chunks_mut::<2>(&mut out).0)
      .zip(<[_]>::iter(&odd))
      .enumerate()
  {
    *out_lane_0 = even_out[i];
    *out_lane_1 = *odd_element;
  }

  out
}

/// DCT 32x32 transform for 8bpc
#[autoversion]
fn inv_txfm_add_dct_dct_32x32_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 1024];

  for y in 0..32 {
    let mut input = [0i32; 32];

    for (x, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
      *input_element =
        <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + x * 32]);
    }

    let out = dct32_1d(&input);

    for x in 0..32 {
      tmp[::core::primitive::usize::wrapping_add(
        ::core::primitive::usize::wrapping_mul(y, 32),
        x,
      )] = out[x];
    }
  }

  for x in 0..32 {
    let mut input = [0i32; 32];

    for (y, input_element) in <[_]>::iter_mut(&mut input).enumerate() {
      *input_element = tmp[y.wrapping_mul(32) + x];
    }

    let out = dct32_1d(&input);

    for y in 0..32 {
      let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
        dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
      );
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let c = ::core::primitive::i32::wrapping_shr(out[y] + 512, 10_u32);
      let result = iclip(d + c, 0, bitdepth_max);
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] =
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(result) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        };
    }
  }

  for i in 0..1024 {
    coeff[i] = 0;
  }
}

/// DCT64 1D transform (simplified - uses recursive approach)
#[inline(always)]
#[must_use]
fn dct64_1d(input: &[i32; 64]) -> [i32; 64] {
  let mut t = [0i32; 64];

  for (i, input_element) in <[_]>::iter(input).take(32).enumerate() {
    t[i] = *input_element + input[(::core::num::Wrapping(63) - ::core::num::Wrapping(i)).0];
    t[(::core::num::Wrapping(63) - ::core::num::Wrapping(i)).0] =
      *input_element - input[(::core::num::Wrapping(63) - ::core::num::Wrapping(i)).0];
  }

  let even_input: [i32; 32] = core::array::from_fn(|i| t[i]);
  let even_out = dct32_1d(&even_input);

  let mut odd = [0i32; 32];

  for i in 0..16 {
    let ::core::result::Result::Ok(idx) = ::console_core_number_conversion::fixed_index(
      ::core::primitive::usize::wrapping_add(32, i),
    );
    let o0 = t[match ::console_core_number_conversion::index(idx) {
      ::core::result::Result::Ok(native_position) => native_position,
    }];
    let o1 = t[::core::primitive::usize::wrapping_sub(63, i)];

    let cos_val = ::core::primitive::i32::wrapping_sub(
      4096,
      (match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(i) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      }) * 64,
    );
    let sin_val = (match ::console_core_number_conversion::wrapped::<_, ::core::primitive::i32>(i)
    {
      ::core::result::Result::Ok(converted_position) => converted_position,
    }) * 256;
    odd[i] = ::core::primitive::i32::wrapping_shr((o0 * cos_val + o1 * sin_val) + 2048, 12_u32);
    odd[::core::primitive::usize::wrapping_sub(31, i)] =
      ::core::primitive::i32::wrapping_shr((o1 * cos_val - o0 * sin_val) + 2048, 12_u32);
  }

  let mut out = [0i32; 64];

  for (i, ([out_lane_0, out_lane_1], odd_element)) in
    <[_]>::iter_mut(<[_]>::as_chunks_mut::<2>(&mut out).0)
      .zip(<[_]>::iter(&odd))
      .enumerate()
  {
    *out_lane_0 = even_out[i];
    *out_lane_1 = *odd_element;
  }

  out
}

#[inline(always)]
fn identity4_1d_arm(c: &mut [i32], stride: usize) {
  for i in 0..4 {
    let v = c[i * stride];
    c[i * stride] = ::core::primitive::i32::wrapping_shr(v * 181 + 128, 8_u32) + v;
  }
}

#[autoversion]
pub(crate) fn inv_txfm_add_dct_identity_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for y in 0..4 {
    let c0 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y]);
    let c1 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(
      coeff[::core::primitive::usize::wrapping_add(y, 4)],
    );
    let c2 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(
      coeff[::core::primitive::usize::wrapping_add(y, 8)],
    );
    let c3 = <i32 as ::core::convert::From<::core::primitive::i16>>::from(
      coeff[::core::primitive::usize::wrapping_add(y, 12)],
    );
    let out = dct4_1d(c0, c1, c2, c3);

    for x in 0..4 {
      tmp[::core::primitive::usize::wrapping_add(
        ::core::primitive::usize::wrapping_mul(y, 4),
        x,
      )] = out[x];
    }
  }

  for x in 0..4 {
    identity4_1d_arm(&mut tmp[x..], 4);
  }

  for y in 0..4 {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
      dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
    );

    for x in 0..4 {
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let val = ::core::primitive::i32::wrapping_shr(
        tmp[::core::primitive::usize::wrapping_mul(y, 4) + x] + 8,
        4_u32,
      );
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        iclip(d + val, 0, bitdepth_max),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

#[autoversion]
pub(crate) fn inv_txfm_add_identity_dct_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for y in 0..4 {
    let mut row = [0i32; 4];

    for (x, row_element) in <[_]>::iter_mut(&mut row).enumerate() {
      *row_element =
        <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + x * 4]);
    }

    identity4_1d_arm(&mut row, 1);

    for (x, row_element) in <[_]>::iter(&row).enumerate() {
      tmp[::core::primitive::usize::wrapping_mul(y, 4) + x] = *row_element;
    }
  }

  for x in 0..4 {
    let out = dct4_1d(
      tmp
        [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(0, 4), x)],
      tmp
        [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(1, 4), x)],
      tmp
        [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(2, 4), x)],
      tmp
        [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(3, 4), x)],
    );

    for y in 0..4 {
      tmp[::core::primitive::usize::wrapping_add(
        ::core::primitive::usize::wrapping_mul(y, 4),
        x,
      )] = out[y];
    }
  }

  for y in 0..4 {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
      dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
    );

    for x in 0..4 {
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let val = ::core::primitive::i32::wrapping_shr(
        tmp[::core::primitive::usize::wrapping_mul(y, 4) + x] + 8,
        4_u32,
      );
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        iclip(d + val, 0, bitdepth_max),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

#[autoversion]
pub(crate) fn inv_txfm_add_adst_identity_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for y in 0..4 {
    let out = adst4_1d(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y]),
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        coeff[::core::primitive::usize::wrapping_add(y, 4)],
      ),
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        coeff[::core::primitive::usize::wrapping_add(y, 8)],
      ),
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        coeff[::core::primitive::usize::wrapping_add(y, 12)],
      ),
    );

    for x in 0..4 {
      tmp[::core::primitive::usize::wrapping_add(
        ::core::primitive::usize::wrapping_mul(y, 4),
        x,
      )] = out[x];
    }
  }

  for x in 0..4 {
    identity4_1d_arm(&mut tmp[x..], 4);
  }

  for y in 0..4 {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
      dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
    );

    for x in 0..4 {
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let val = ::core::primitive::i32::wrapping_shr(
        tmp[::core::primitive::usize::wrapping_mul(y, 4) + x] + 8,
        4_u32,
      );
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        iclip(d + val, 0, bitdepth_max),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

#[autoversion]
pub(crate) fn inv_txfm_add_identity_adst_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for y in 0..4 {
    let mut row = [0i32; 4];

    for (x, row_element) in <[_]>::iter_mut(&mut row).enumerate() {
      *row_element =
        <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + x * 4]);
    }

    identity4_1d_arm(&mut row, 1);

    for (x, row_element) in <[_]>::iter(&row).enumerate() {
      tmp[::core::primitive::usize::wrapping_mul(y, 4) + x] = *row_element;
    }
  }

  for x in 0..4 {
    let out = adst4_1d(
      tmp
        [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(0, 4), x)],
      tmp
        [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(1, 4), x)],
      tmp
        [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(2, 4), x)],
      tmp
        [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(3, 4), x)],
    );

    for y in 0..4 {
      tmp[::core::primitive::usize::wrapping_add(
        ::core::primitive::usize::wrapping_mul(y, 4),
        x,
      )] = out[y];
    }
  }

  for y in 0..4 {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
      dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
    );

    for x in 0..4 {
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let val = ::core::primitive::i32::wrapping_shr(
        tmp[::core::primitive::usize::wrapping_mul(y, 4) + x] + 8,
        4_u32,
      );
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        iclip(d + val, 0, bitdepth_max),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

#[autoversion]
pub(crate) fn inv_txfm_add_flipadst_identity_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for y in 0..4 {
    let out = flipadst4_1d(
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y]),
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        coeff[::core::primitive::usize::wrapping_add(y, 4)],
      ),
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        coeff[::core::primitive::usize::wrapping_add(y, 8)],
      ),
      <i32 as ::core::convert::From<::core::primitive::i16>>::from(
        coeff[::core::primitive::usize::wrapping_add(y, 12)],
      ),
    );

    for x in 0..4 {
      tmp[::core::primitive::usize::wrapping_add(
        ::core::primitive::usize::wrapping_mul(y, 4),
        x,
      )] = out[x];
    }
  }

  for x in 0..4 {
    identity4_1d_arm(&mut tmp[x..], 4);
  }

  for y in 0..4 {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
      dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
    );

    for x in 0..4 {
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let val = ::core::primitive::i32::wrapping_shr(
        tmp[::core::primitive::usize::wrapping_mul(y, 4) + x] + 8,
        4_u32,
      );
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        iclip(d + val, 0, bitdepth_max),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

#[autoversion]
pub(crate) fn inv_txfm_add_identity_flipadst_4x4_8bpc_inner(
  dst: &mut [u8],
  dst_base: usize,
  dst_stride: isize,
  coeff: &mut [i16],
  _eob: i32,
  bitdepth_max: i32,
) {
  let mut tmp = [0i32; 16];

  for y in 0..4 {
    let mut row = [0i32; 4];

    for (x, row_element) in <[_]>::iter_mut(&mut row).enumerate() {
      *row_element =
        <i32 as ::core::convert::From<::core::primitive::i16>>::from(coeff[y + x * 4]);
    }

    identity4_1d_arm(&mut row, 1);

    for (x, row_element) in <[_]>::iter(&row).enumerate() {
      tmp[::core::primitive::usize::wrapping_mul(y, 4) + x] = *row_element;
    }
  }

  for x in 0..4 {
    let out = flipadst4_1d(
      tmp
        [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(0, 4), x)],
      tmp
        [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(1, 4), x)],
      tmp
        [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(2, 4), x)],
      tmp
        [::core::primitive::usize::wrapping_add(::core::primitive::usize::wrapping_mul(3, 4), x)],
    );

    for y in 0..4 {
      tmp[::core::primitive::usize::wrapping_add(
        ::core::primitive::usize::wrapping_mul(y, 4),
        x,
      )] = out[y];
    }
  }

  for y in 0..4 {
    let ::core::result::Result::Ok(row_off) = ::console_core_number_conversion::fixed_index(
      dst_base.wrapping_add_signed(::core::primitive::usize::cast_signed(y) * dst_stride),
    );

    for x in 0..4 {
      let d = <i32 as ::core::convert::From<::core::primitive::u8>>::from(
        dst[(match ::console_core_number_conversion::index(row_off) {
          ::core::result::Result::Ok(native_position) => native_position,
        }) + x],
      );
      let val = ::core::primitive::i32::wrapping_shr(
        tmp[::core::primitive::usize::wrapping_mul(y, 4) + x] + 8,
        4_u32,
      );
      dst[(match ::console_core_number_conversion::index(row_off) {
        ::core::result::Result::Ok(native_position) => native_position,
      }) + x] = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
        iclip(d + val, 0, bitdepth_max),
      ) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    }
  }

  for i in 0..16 {
    coeff[i] = 0;
  }
}

use crate::include::common::bitdepth::BPC;
use crate::include::common::bitdepth::BitDepth;
use crate::src::levels::ADST_ADST;
use crate::src::levels::ADST_DCT;
use crate::src::levels::ADST_FLIPADST;
use crate::src::levels::DCT_ADST;
use crate::src::levels::DCT_DCT;
use crate::src::levels::DCT_FLIPADST;
use crate::src::levels::FLIPADST_ADST;
use crate::src::levels::FLIPADST_DCT;
use crate::src::levels::FLIPADST_FLIPADST;
use crate::src::levels::H_ADST;
use crate::src::levels::H_DCT;
use crate::src::levels::H_FLIPADST;
use crate::src::levels::IDTX;
use crate::src::levels::TxfmSize;
use crate::src::levels::TxfmType;
use crate::src::levels::V_ADST;
use crate::src::levels::V_DCT;
use crate::src::levels::V_FLIPADST;
use crate::src::levels::WHT_WHT;
use crate::src::strided::Strided as _;

/// Safe dispatch entry point for ITX SIMD on ARM.
pub fn itxfm_add_dispatch<BD: BitDepth>(
  tx_size: usize,
  tx_type: usize,
  dst: &mut crate::src::owned_recon::ReconDst<'_>,
  coeff: &mut [BD::Coef],
  eob: i32,
  bd: BD,
) -> bool {
  match {
    ::core::primitive::bool::then_some(
      crate::src::ablate::is_off(crate::src::ablate::Family::Itx),
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return false;
    }
    ::core::option::Option::None => {}
  }

  {
    #[cfg(target_arch = "aarch64")]
    {
      use crate::include::common::bitdepth::BPC;
      use crate::src::levels::{self, TxfmSize};
      use crate::src::strided::Strided as _;
      use zerocopy::IntoBytes;

      use archmage::SimdToken;

      let token = {
        let optional_token = archmage::Arm64::summon();

        match optional_token {
          Some(matched_token) => matched_token,
          ::core::option::Option::None => return false,
        }
      };

      let txfm = match TxfmSize::from_repr(tx_size) {
        Some(t) => t,
        None => return false,
      };
      let Ok(extent) = txfm.extent();
      let Ok(w) = console_core_number_conversion::index(extent.width);
      let Ok(h) = console_core_number_conversion::index(extent.height);
      let samples = u64::from(extent.width).wrapping_mul(u64::from(extent.height));
      let Ok(()) = crate::src::ablate::note(crate::src::ablate::Family::Itx, samples);

      match {
        ::core::primitive::bool::then_some(
          BD::BPC == BPC::BPC16 && super::itx_arm_hbd::hbd_supported(w, h),
          (),
        )
      } {
        ::core::option::Option::Some(()) => {
          use super::itx_arm_hbd::Kind;

          let (second, first) =
            match match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
              tx_type,
            ) {
              ::core::result::Result::Ok(converted_position) => converted_position,
            } {
              levels::IDTX => (Kind::Identity, Kind::Identity),
              levels::DCT_DCT => (Kind::Dct, Kind::Dct),
              levels::ADST_DCT => (Kind::Adst, Kind::Dct),
              levels::FLIPADST_DCT => (Kind::FlipAdst, Kind::Dct),
              levels::H_DCT => (Kind::Identity, Kind::Dct),
              levels::DCT_ADST => (Kind::Dct, Kind::Adst),
              levels::ADST_ADST => (Kind::Adst, Kind::Adst),
              levels::FLIPADST_ADST => (Kind::FlipAdst, Kind::Adst),
              levels::DCT_FLIPADST => (Kind::Dct, Kind::FlipAdst),
              levels::ADST_FLIPADST => (Kind::Adst, Kind::FlipAdst),
              levels::FLIPADST_FLIPADST => (Kind::FlipAdst, Kind::FlipAdst),
              levels::V_DCT => (Kind::Dct, Kind::Identity),
              levels::H_ADST => (Kind::Identity, Kind::Adst),
              levels::H_FLIPADST => (Kind::Identity, Kind::FlipAdst),
              levels::V_ADST => (Kind::Adst, Kind::Identity),
              levels::V_FLIPADST => (Kind::FlipAdst, Kind::Identity),

              _ => return false,
            };
          let shift: u32 = match (w, h) {
            (4, 4) | (4, 8) | (8, 4) => 0,
            (4, 16) | (8, 8) | (8, 16) | (16, 4) | (16, 8) => 1,
            (16, 16) => 2,
            _ => return false,
          };
          let has_dc_only =
            (match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(tx_type)
            {
              ::core::result::Result::Ok(converted_position) => converted_position,
            }) == levels::DCT_DCT;
          let bd_c = bd.into_c();

          match {
            ::core::primitive::bool::then_some(
              core::mem::size_of::<BD::Pixel>() != core::mem::size_of::<u16>()
                || core::mem::align_of::<BD::Pixel>() < core::mem::align_of::<u16>(),
              (),
            )
          } {
            ::core::option::Option::Some(()) => {
              return false;
            }
            ::core::option::Option::None => {}
          }

          let coeff_i32: &mut [i32] =
            match zerocopy::FromBytes::mut_from_bytes(coeff.as_mut_bytes()) {
              Ok(samples) => samples,
              Err(problem) => {
                console_core_logging::debug!(
                  "AV1 NEON coefficient layout cannot be used: {problem:?}"
                );
                return false;
              }
            };

          match { ::core::primitive::bool::then_some(eob < has_dc_only as i32, ()) } {
            ::core::option::Option::Some(()) => {
              let dc = super::itx_arm_hbd::hbd_dc_value(w, h, shift, coeff_i32);
              let Ok(()) = dst.for_rows_mut::<BD, _>(w, h, |_y, row| {
                let px: &mut [u16] = match zerocopy::FromBytes::mut_from_bytes(row.as_mut_bytes())
                {
                  Ok(samples) => samples,
                  Err(problem) => {
                    console_core_logging::error!(
                      "AV1 NEON row violates its verified pixel layout: {problem:?}"
                    );
                    return;
                  }
                };
                super::itx_arm_hbd::add_row_dc_hbd_neon(token, px, w, dc, bd_c);
              });
              return true;
            }
            ::core::option::Option::None => {}
          }

          macro_rules! hbd_run {
            ($n:expr) => {{
              let mut tmp = [0i32; $n];
              super::itx_arm_hbd::inv_txfm_hbd_neon(
                token, w, h, first, second, shift, coeff_i32, bd_c, &mut tmp,
              );
              let Ok(()) = dst.for_rows_mut::<BD, _>(w, h, |y, row| {
                let px: &mut [u16] = match zerocopy::FromBytes::mut_from_bytes(row.as_mut_bytes())
                {
                  Ok(samples) => samples,
                  Err(problem) => {
                    console_core_logging::error!(
                      "AV1 NEON row violates its verified pixel layout: {problem:?}"
                    );
                    return;
                  }
                };
                super::itx_arm_hbd::add_row_hbd_neon(token, px, &tmp[y * w..y * w + w], w, bd_c);
              });
            }};
          }

          match w * h {
            16 => hbd_run!(16),
            32 => hbd_run!(32),
            64 => hbd_run!(64),
            128 => hbd_run!(128),
            256 => hbd_run!(256),

            n => unreachable!("itx_arm_hbd: no scratch arm for {w}x{h} (area {n})"),
          }

          return true;
        }
        ::core::option::Option::None => {}
      }

      match { ::core::primitive::bool::then_some(w == 4 && h == 4 && BD::BPC == BPC::BPC8, ()) } {
        ::core::option::Option::Some(()) => {
          let Ok(mut block) = dst.block_mut::<BD>(w, h);
          let Ok(byte_stride_i) = block.byte_stride();
          let Ok(base) = block.base();
          let Ok(dst_u8) = block.as_mut_bytes();
          let coeff_i16: &mut [i16] = zerocopy::FromBytes::mut_from_bytes(coeff.as_mut_bytes())
            .expect("coeff alignment/size mismatch for i16 reinterpretation");

          use super::itx_arm_neon_4x4::{Transform4, transform_4x4_neon};
          let tx_t = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
            tx_type,
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };

          match tx_t {
            levels::WHT_WHT => {
              let Ok(wht_base) = console_core_number_conversion::fixed_index(base);
              let Ok(wht_pitch) = console_core_number_conversion::fixed_offset(byte_stride_i);
              let output = super::itx_arm_neon_wht::WhtOutput {
                base: wht_base,
                pitch: wht_pitch,
              };

              match super::itx_arm_neon_wht::inv_txfm_add_wht_wht_4x4_8bpc_neon_inner(
                token, dst_u8, output, coeff_i16,
              ) {
                Ok(()) => {}
                Err(problem) => {
                  console_core_logging::debug!("Invalid AV1 WHT output window: {problem:?}");
                  return false;
                }
              }
            }
            kind => {
              let Ok(first) = console_core_number_conversion::fixed_index(base);
              let Ok(pitch) = console_core_number_conversion::fixed_offset(byte_stride_i);
              let request = Transform4 {
                pixels: dst_u8,
                origin: (first, pitch),
                coefficients: coeff_i16,
                eob,
              };
              match transform_4x4_neon(token, request, kind) {
                Ok(()) => {}
                Err(_invalid_block) => return false,
              }
            }
          }

          return true;
        }
        ::core::option::Option::None => {}
      }

      match { ::core::primitive::bool::then_some(w == 8 && h == 8 && BD::BPC == BPC::BPC8, ()) } {
        ::core::option::Option::Some(()) => {
          let bd_c = bd.into_c();

          let Ok(mut block) = dst.block_mut::<BD>(w, h);
          let Ok(byte_stride_i) = block.byte_stride();
          let Ok(base) = block.base();
          let Ok(dst_u8) = block.as_mut_bytes();
          let coeff_i16: &mut [i16] = zerocopy::FromBytes::mut_from_bytes(coeff.as_mut_bytes())
            .expect("coeff alignment/size mismatch for i16 reinterpretation");

          use super::itx_arm_neon_8x8::*;
          let tx_t = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
            tx_type,
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };

          match tx_t {
            levels::DCT_DCT => inv_txfm_add_dct_dct_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::IDTX => inv_txfm_add_identity_identity_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::ADST_ADST => inv_txfm_add_adst_adst_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::ADST_DCT => inv_txfm_add_dct_adst_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::DCT_ADST => inv_txfm_add_adst_dct_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::FLIPADST_DCT => inv_txfm_add_dct_flipadst_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::DCT_FLIPADST => inv_txfm_add_flipadst_dct_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::FLIPADST_FLIPADST => inv_txfm_add_flipadst_flipadst_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::ADST_FLIPADST => inv_txfm_add_flipadst_adst_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::FLIPADST_ADST => inv_txfm_add_adst_flipadst_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::H_DCT => inv_txfm_add_dct_identity_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::V_DCT => inv_txfm_add_identity_dct_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::H_ADST => inv_txfm_add_adst_identity_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::V_ADST => inv_txfm_add_identity_adst_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::H_FLIPADST => inv_txfm_add_flipadst_identity_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::V_FLIPADST => inv_txfm_add_identity_flipadst_8x8_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            _ => return false,
          }

          return true;
        }
        ::core::option::Option::None => {}
      }

      match { ::core::primitive::bool::then_some(w == 16 && h == 16 && BD::BPC == BPC::BPC8, ()) }
      {
        ::core::option::Option::Some(()) => {
          let bd_c = bd.into_c();

          let Ok(mut block) = dst.block_mut::<BD>(w, h);
          let Ok(byte_stride_i) = block.byte_stride();
          let Ok(base) = block.base();
          let Ok(dst_u8) = block.as_mut_bytes();
          let coeff_i16: &mut [i16] = zerocopy::FromBytes::mut_from_bytes(coeff.as_mut_bytes())
            .expect("coeff alignment/size mismatch for i16 reinterpretation");

          use super::itx_arm_neon_16x16::*;
          let tx_t = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
            tx_type,
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };

          match tx_t {
            levels::DCT_DCT => inv_txfm_add_dct_dct_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::IDTX => inv_txfm_add_identity_identity_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::ADST_ADST => inv_txfm_add_adst_adst_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::ADST_DCT => inv_txfm_add_dct_adst_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::DCT_ADST => inv_txfm_add_adst_dct_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::FLIPADST_DCT => inv_txfm_add_dct_flipadst_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::DCT_FLIPADST => inv_txfm_add_flipadst_dct_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::FLIPADST_FLIPADST => inv_txfm_add_flipadst_flipadst_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::ADST_FLIPADST => inv_txfm_add_flipadst_adst_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::FLIPADST_ADST => inv_txfm_add_adst_flipadst_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::H_DCT => inv_txfm_add_dct_identity_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::V_DCT => inv_txfm_add_identity_dct_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::H_ADST => inv_txfm_add_adst_identity_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::V_ADST => inv_txfm_add_identity_adst_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::H_FLIPADST => inv_txfm_add_flipadst_identity_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::V_FLIPADST => inv_txfm_add_identity_flipadst_16x16_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            _ => return false,
          }

          return true;
        }
        ::core::option::Option::None => {}
      }

      match { ::core::primitive::bool::then_some(w == 32 && h == 32 && BD::BPC == BPC::BPC8, ()) }
      {
        ::core::option::Option::Some(()) => {
          let bd_c = bd.into_c();

          let Ok(mut block) = dst.block_mut::<BD>(w, h);
          let Ok(byte_stride_i) = block.byte_stride();
          let Ok(base) = block.base();
          let Ok(dst_u8) = block.as_mut_bytes();
          let coeff_i16: &mut [i16] = zerocopy::FromBytes::mut_from_bytes(coeff.as_mut_bytes())
            .expect("coeff alignment/size mismatch for i16 reinterpretation");

          use super::itx_arm_neon_32::*;
          let tx_t = match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u8>(
            tx_type,
          ) {
            ::core::result::Result::Ok(converted_position) => converted_position,
          };

          match tx_t {
            levels::DCT_DCT => inv_txfm_add_dct_dct_32x32_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            levels::IDTX => inv_txfm_add_identity_identity_32x32_8bpc_neon_inner(
              token,
              dst_u8,
              base,
              byte_stride_i,
              coeff_i16,
              eob,
              bd_c,
            ),
            _ => return false,
          }

          return true;
        }
        ::core::option::Option::None => {}
      }

      match (BD::BPC, extent.width, extent.height) {
        (BPC::BPC8, 8, 32)
        | (BPC::BPC8, 32, 8)
        | (BPC::BPC8, 16, 32 | 64)
        | (BPC::BPC8, 32 | 64, 16)
        | (BPC::BPC8, 32, 64)
        | (BPC::BPC8, 64, 32 | 64) => {
          use super::itx_arm_neon_64::{
            LargeNeonBlock, LargeNeonOutput, dct_64_axis_neon, identity_64_axis_neon,
            identity_rect_neon,
          };
          use super::itx_arm_neon_large_rect::dct_large_rect_neon;

          let Ok(type_code) = console_core_number_conversion::wrapped::<_, u8>(tx_type);
          let kernel: fn(archmage::Arm64, LargeNeonBlock<'_>) -> crate::src::error::Rav1dResult =
            match (extent.width, extent.height, type_code) {
              (64, _, levels::DCT_DCT) | (_, 64, levels::DCT_DCT) => dct_64_axis_neon,
              (64, _, levels::IDTX) | (_, 64, levels::IDTX) => identity_64_axis_neon,
              (_, _, levels::DCT_DCT) => dct_large_rect_neon,
              (_, _, levels::IDTX) => identity_rect_neon,
              _unsupported => return false,
            };
          let Ok(mut block) = dst.block_mut::<BD>(w, h);
          let Ok(first) = console_core_number_conversion::fixed_index(match block.base() {
            Ok(native) => native,
          });
          let Ok(pitch) =
            console_core_number_conversion::fixed_offset(match block.byte_stride() {
              Ok(native) => native,
            });
          let Ok(pixels) = block.as_mut_bytes();

          let coefficients = match zerocopy::FromBytes::mut_from_bytes(coeff.as_mut_bytes()) {
            Ok(samples) => samples,
            Err(_layout) => return false,
          };
          let transform_size = console_core_geometry::Size {
            width: u64::from(extent.width),
            height: u64::from(extent.height),
          };

          let output = match LargeNeonOutput::new(pixels, transform_size, (first, pitch)) {
            Ok(block_output) => block_output,
            Err(_layout) => return false,
          };

          return matches!(
            kernel(
              token,
              LargeNeonBlock {
                output,
                coefficients,
                eob
              }
            ),
            Ok(())
          );
        }
        _other => {}
      }

      return false;
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
      let _ = (tx_size, tx_type, &dst, coeff, eob, &bd);
      return false;
    }
  }
}

/// Benchmark comparing autoversioned (LLVM auto-vectorized) scalar fallbacks
/// against hand-written NEON intrinsic implementations.
///
/// Run on aarch64: `cargo test --release -- --ignored bench_autoversion_vs_neon --nocapture`
#[cfg(all(test, target_arch = "aarch64"))]
mod bench_autoversion_vs_neon {
  use archmage::SimdToken as _;
  use std::time::Instant;

  #[must_use]
  fn bench_fn(name: &str, iters: u32, mut f: impl FnMut()) -> u64 {
    for _ in 0..100 {
      f();
    }

    let mut times = Vec::with_capacity(
      match ::console_core_number_conversion::wrapped_index(iters) {
        ::core::result::Result::Ok(converted_position) => converted_position,
      },
    );

    for _ in 0..iters {
      let start = Instant::now();
      f();
      times.push(
        match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u64>(
          start.elapsed().as_nanos(),
        ) {
          ::core::result::Result::Ok(converted_position) => converted_position,
        },
      );
    }

    times.sort();
    let median = times[times.len().wrapping_div(2)];
    let mean: u64 = times.iter().sum::<u64>()
      / match ::console_core_number_conversion::wrapped::<_, ::core::primitive::u64>(times.len())
      {
        ::core::result::Result::Ok(converted_position) => converted_position,
      };
    println!("  {name}: median={median}ns mean={mean}ns");
    median
  }

  macro_rules! bench_transform {
    ($test_name:ident, $w:expr, $h:expr,
         autoversioned: $av_fn:path,
         neon: $neon_fn:expr,
         $coeff_count:expr) => {
      #[test]
      #[ignore]
      fn $test_name() {
        let Ok(_lock) = crate::src::safe_simd::token_test_lock();
        let token = archmage::Arm64::summon().expect("NEON required");

        let stride: isize = ($w + 16) as isize;
        let dst_size = ($h as usize) * (stride as usize) + $w;
        let mut dst_av = vec![128u8; dst_size];
        let mut dst_neon = vec![128u8; dst_size];
        let base = 0usize;

        let mut coeff_template = vec![0i16; $coeff_count];
        for (i, c) in coeff_template.iter_mut().enumerate() {
          *c = (((i * 37 + 13) % 512) as i16) - 256;
        }
        coeff_template[0] = 1000;

        let iters = 100_000u32;
        println!("\n=== {} ({}x{}) ===", stringify!($test_name), $w, $h);

        let mut coeff = coeff_template.clone();
        let av_ns = bench_fn("autoversioned", iters, || {
          dst_av.fill(128);
          coeff.copy_from_slice(&coeff_template);
          $av_fn(
            &mut dst_av,
            base,
            stride,
            &mut coeff,
            $coeff_count as i32 - 1,
            255,
          );
        });

        let neon_ns = bench_fn("hand-written NEON", iters, || {
          dst_neon.fill(128);
          coeff.copy_from_slice(&coeff_template);
          ($neon_fn)(
            token,
            &mut dst_neon,
            base,
            stride,
            &mut coeff,
            $coeff_count as i32 - 1,
            255,
          );
        });

        dst_av.fill(128);
        dst_neon.fill(128);
        let mut ca = coeff_template.clone();
        let mut cb = coeff_template.clone();
        $av_fn(
          &mut dst_av,
          base,
          stride,
          &mut ca,
          $coeff_count as i32 - 1,
          255,
        );
        ($neon_fn)(
          token,
          &mut dst_neon,
          base,
          stride,
          &mut cb,
          $coeff_count as i32 - 1,
          255,
        );
        assert_eq!(dst_av, dst_neon, "Output mismatch!");

        let ratio = av_ns as f64 / neon_ns as f64;
        println!("  ratio: {ratio:.2}x (autoversioned / NEON, <1 = autoversioned faster)");
      }
    };
  }

  bench_transform!(bench_dct_dct_4x4, 4, 4,
      autoversioned: super::inv_txfm_add_dct_dct_4x4_8bpc_inner,
      neon: match super::super::itx_arm_neon_4x4::benchmark::<{ crate::src::levels::DCT_DCT }>() {
        Ok(callback) => callback,
      },
      16
  );
  bench_transform!(bench_identity_4x4, 4, 4,
      autoversioned: super::inv_txfm_add_identity_identity_4x4_8bpc_inner,
      neon: match super::super::itx_arm_neon_4x4::benchmark::<{ crate::src::levels::IDTX }>() {
        Ok(callback) => callback,
      },
      16
  );
  bench_transform!(bench_adst_adst_4x4, 4, 4,
      autoversioned: super::inv_txfm_add_adst_adst_4x4_8bpc_inner,
      neon: match super::super::itx_arm_neon_4x4::benchmark::<{ crate::src::levels::ADST_ADST }>() {
        Ok(callback) => callback,
      },
      16
  );
  bench_transform!(bench_wht_wht_4x4, 4, 4,
      autoversioned: super::inv_txfm_add_wht_wht_4x4_8bpc_inner,
      neon: |token, dst, base, stride, coeff, _, _| {
        use super::super::itx_arm_neon_wht::{WhtOutput, inv_txfm_add_wht_wht_4x4_8bpc_neon_inner};
        let Ok(offset) = console_core_number_conversion::fixed_index(base);
        let Ok(pitch) = console_core_number_conversion::fixed_offset(stride);
        let output = WhtOutput { base: offset, pitch };
        assert_eq!(inv_txfm_add_wht_wht_4x4_8bpc_neon_inner(token, dst, output, coeff), Ok(()));
      },
      16
  );
  bench_transform!(bench_dct_dct_8x8, 8, 8,
      autoversioned: super::inv_txfm_add_dct_dct_8x8_8bpc_inner,
      neon: super::super::itx_arm_neon_8x8::inv_txfm_add_dct_dct_8x8_8bpc_neon_inner,
      64
  );
  bench_transform!(bench_identity_8x8, 8, 8,
      autoversioned: super::inv_txfm_add_identity_identity_8x8_8bpc_inner,
      neon: super::super::itx_arm_neon_8x8::inv_txfm_add_identity_identity_8x8_8bpc_neon_inner,
      64
  );
  bench_transform!(bench_dct_dct_16x16, 16, 16,
      autoversioned: super::inv_txfm_add_dct_dct_16x16_8bpc_inner,
      neon: super::super::itx_arm_neon_16x16::inv_txfm_add_dct_dct_16x16_8bpc_neon_inner,
      256
  );
  bench_transform!(bench_dct_dct_32x32, 32, 32,
      autoversioned: super::inv_txfm_add_dct_dct_32x32_8bpc_inner,
      neon: super::super::itx_arm_neon_32::inv_txfm_add_dct_dct_32x32_8bpc_neon_inner,
      1024
  );
}
