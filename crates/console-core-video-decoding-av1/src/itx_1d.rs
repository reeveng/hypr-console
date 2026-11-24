#![forbid(unsafe_code)]

//! In some places, we use the pattern like this:
//!
//! ```
//! # fn f(in1: i32, in3: i32) {
//! # let t2;
//! t2 = ((in1 *  1567         - in3 * (3784 - 4096) + 2048) >> 12) - in3;
//! # }
//! ```
//!
//! even though the reference code might use something like:
//!
//! ```
//! # fn f(in1: i32, in3: i32) {
//! # let t2;
//! t2 =  (in1 *  1567         - in3 *  3784         + 2048) >> 12;
//! # }
//! ```
//!
//! The reason for this is that for 12 bits/component bitstreams (corrupt/
//! invalid ones, but they are codable nonetheless), each coefficient or
//! input can be 19(+sign) bits, and therefore if the combination of the
//! two multipliers (each 12 bits) is >= 4096, the result of the add/sub
//! after the pair of multiplies will exceed the 31+sign bit range. Signed
//! integer overflows are UB in C, and we'd like to prevent that.
//!
//! To workaround this, we invert one of the two coefficients (or, if both are
//! multiples of 2, we reduce their magnitude by one bit). It should be noted
//! that SIMD implementations do not have to follow this exact behaviour. The
//! AV1 spec clearly states that the result of the multiply/add pairs should
//! fit in 31+sign bit intermediates, and that streams violating this convention
//! are not AV1-compliant. So, as long as we don't trigger UB (which some people
//! would consider a security vulnerability), we're fine. So, SIMD can simply
//! use the faster implementation, even if that might in some cases result in
//! integer overflows, since these are not considered valid AV1 anyway, and in
//! e.g. x86 assembly, integer overflows are not considered UB, but they merely
//! wrap around.

use crate::include::common::intops::iclip;
use crate::src::error::{Rav1dError, Rav1dResult};
use crate::src::itx::CoefficientBounds;
use console_core_never::Never;
use console_core_video_interpolation::round_shift;
use console_core_video_inverse_transforms::{
  StrideError, av1, inverse_walsh_hadamard4, strided_lane_mut, transform_strided,
};
use std::num::{NonZeroU32, NonZeroUsize};

#[derive(Clone, Copy)]
enum DctInput {
  Full,
  LowHalf,
  #[cfg(target_arch = "x86_64")]
  LowOdd,
}

#[inline]
fn coefficient_offset(stride: NonZeroU32, position: u8) -> Rav1dResult<u32> {
  stride
    .get()
    .checked_mul(u32::from(position))
    .ok_or(Rav1dError::ERANGE)
}

#[inline]
fn read_coefficient(lane: &[i32], stride: NonZeroU32, position: u8) -> Rav1dResult<i32> {
  let offset = coefficient_offset(stride, position)?;
  let Ok(index) = console_core_number_conversion::index(offset);

  lane.get(index).copied().ok_or(Rav1dError::EINVAL)
}

#[inline]
fn write_coefficient(
  lane: &mut [i32],
  stride: NonZeroU32,
  position: u8,
  value: i32,
) -> Rav1dResult {
  let offset = coefficient_offset(stride, position)?;
  let Ok(index) = console_core_number_conversion::index(offset);
  let sample = lane.get_mut(index).ok_or(Rav1dError::EINVAL)?;
  *sample = value;

  Ok(())
}

#[inline]
fn merge_dct_halves<const N: usize>(
  lane: &mut [i32],
  stride: NonZeroU32,
  bounds: CoefficientBounds,
  odd: [i32; N],
) -> Rav1dResult {
  let mut even = [0; N];

  for (sample, position) in even.iter_mut().zip((0u8..).step_by(2)) {
    let checked_value = read_coefficient(lane, stride, position)?;
    *sample = checked_value;
  }

  let last =
    u8::try_from(N.saturating_mul(2).saturating_sub(1)).map_err(|_| Rav1dError::ERANGE)?;

  for ((even_sample, odd_sample), position) in
    even.into_iter().zip(odd.into_iter().rev()).zip(0u8..)
  {
    let sum = iclip(
      even_sample.wrapping_add(odd_sample),
      bounds.lower,
      bounds.upper,
    );
    let difference = iclip(
      even_sample.wrapping_sub(odd_sample),
      bounds.lower,
      bounds.upper,
    );
    write_coefficient(lane, stride, position, sum)?;
    write_coefficient(lane, stride, last.wrapping_sub(position), difference)?;
  }

  Ok(())
}

#[inline(never)]
fn inv_dct4_1d_internal_c(
  c: &mut [i32],
  stride: NonZeroU32,
  bounds: CoefficientBounds,
  input: DctInput,
) -> Rav1dResult {
  let CoefficientBounds {
    lower: min,
    upper: max,
  } = bounds;

  {
    let clip = |v| iclip(v, min, max);

    let in0 = read_coefficient(c, stride, 0)?;
    let in1 = read_coefficient(c, stride, 1)?;

    let t0;
    let t1;
    let t2;
    let t3;

    match ::core::primitive::bool::then_some(!matches!(input, DctInput::Full), ()) {
      ::core::option::Option::Some(()) => {
        t1 = i32::wrapping_shr(i32::wrapping_add(i32::wrapping_mul(in0, 181), 128), 8_u32);
        t0 = t1;
        t2 = i32::wrapping_shr(
          i32::wrapping_add(i32::wrapping_mul(in1, 1567), 2048),
          12_u32,
        );
        t3 = i32::wrapping_shr(
          i32::wrapping_add(i32::wrapping_mul(in1, 3784), 2048),
          12_u32,
        );
      }
      ::core::option::Option::None => {
        let in2 = read_coefficient(c, stride, 2)?;
        let in3 = read_coefficient(c, stride, 3)?;

        t0 = i32::wrapping_shr(
          i32::wrapping_add(i32::wrapping_mul(i32::wrapping_add(in0, in2), 181), 128),
          8_u32,
        );
        t1 = i32::wrapping_shr(
          i32::wrapping_add(i32::wrapping_mul(i32::wrapping_sub(in0, in2), 181), 128),
          8_u32,
        );
        t2 = i32::wrapping_sub(
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_sub(
                i32::wrapping_mul(in1, 1567),
                i32::wrapping_mul(in3, i32::wrapping_sub(3784, 4096)),
              ),
              2048,
            ),
            12_u32,
          ),
          in3,
        );
        t3 = i32::wrapping_add(
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_mul(in1, i32::wrapping_sub(3784, 4096)),
                i32::wrapping_mul(in3, 1567),
              ),
              2048,
            ),
            12_u32,
          ),
          in1,
        );
      }
    }

    write_coefficient(c, stride, 0, clip(i32::wrapping_add(t0, t3)))?;
    write_coefficient(c, stride, 1, clip(i32::wrapping_add(t1, t2)))?;
    write_coefficient(c, stride, 2, clip(i32::wrapping_sub(t1, t2)))?;
    write_coefficient(c, stride, 3, clip(i32::wrapping_sub(t0, t3)))?;
  };

  Ok(())
}

pub fn rav1d_inv_dct4_1d_c(
  coefficients: &mut [i32],
  stride: NonZeroU32,
  bounds: CoefficientBounds,
) -> Rav1dResult {
  let lane = checked_dct_lane::<4>(coefficients, stride)?;
  inv_dct4_1d_internal_c(lane, stride, bounds, DctInput::Full)
}

#[inline(never)]
fn inv_dct8_1d_internal_c(
  c: &mut [i32],
  stride: NonZeroU32,
  bounds: CoefficientBounds,
  input: DctInput,
) -> Rav1dResult {
  let CoefficientBounds {
    lower: min,
    upper: max,
  } = bounds;

  {
    let clip = |v| iclip(v, min, max);
    let even_stride = stride.checked_add(stride.get()).ok_or(Rav1dError::ERANGE)?;

    inv_dct4_1d_internal_c(
      c,
      even_stride,
      CoefficientBounds {
        lower: min,
        upper: max,
      },
      input,
    )?;

    let in1 = read_coefficient(c, stride, 1)?;
    let in3 = read_coefficient(c, stride, 3)?;

    let t4a;
    let mut t5a;
    let mut t6a;
    let t7a;

    match ::core::primitive::bool::then_some(!matches!(input, DctInput::Full), ()) {
      ::core::option::Option::Some(()) => {
        t4a = i32::wrapping_shr(i32::wrapping_add(i32::wrapping_mul(in1, 799), 2048), 12_u32);
        t5a = i32::wrapping_shr(
          i32::wrapping_add(i32::wrapping_mul(in3, -2276), 2048),
          12_u32,
        );
        t6a = i32::wrapping_shr(
          i32::wrapping_add(i32::wrapping_mul(in3, 3406), 2048),
          12_u32,
        );
        t7a = i32::wrapping_shr(
          i32::wrapping_add(i32::wrapping_mul(in1, 4017), 2048),
          12_u32,
        );
      }
      ::core::option::Option::None => {
        let in5 = read_coefficient(c, stride, 5)?;
        let in7 = read_coefficient(c, stride, 7)?;

        t4a = i32::wrapping_sub(
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_sub(
                i32::wrapping_mul(in1, 799),
                i32::wrapping_mul(in7, i32::wrapping_sub(4017, 4096)),
              ),
              2048,
            ),
            12_u32,
          ),
          in7,
        );
        t5a = i32::wrapping_shr(
          i32::wrapping_add(
            i32::wrapping_sub(i32::wrapping_mul(in5, 1703), i32::wrapping_mul(in3, 1138)),
            1024,
          ),
          11_u32,
        );
        t6a = i32::wrapping_shr(
          i32::wrapping_add(
            i32::wrapping_add(i32::wrapping_mul(in5, 1138), i32::wrapping_mul(in3, 1703)),
            1024,
          ),
          11_u32,
        );
        t7a = i32::wrapping_add(
          i32::wrapping_shr(
            i32::wrapping_add(
              i32::wrapping_add(
                i32::wrapping_mul(in1, i32::wrapping_sub(4017, 4096)),
                i32::wrapping_mul(in7, 799),
              ),
              2048,
            ),
            12_u32,
          ),
          in1,
        );
      }
    }

    let t4 = clip(i32::wrapping_add(t4a, t5a));
    t5a = clip(i32::wrapping_sub(t4a, t5a));
    let t7 = clip(i32::wrapping_add(t7a, t6a));
    t6a = clip(i32::wrapping_sub(t7a, t6a));

    let t5 = i32::wrapping_shr(
      i32::wrapping_add(i32::wrapping_mul(i32::wrapping_sub(t6a, t5a), 181), 128),
      8_u32,
    );
    let t6 = i32::wrapping_shr(
      i32::wrapping_add(i32::wrapping_mul(i32::wrapping_add(t6a, t5a), 181), 128),
      8_u32,
    );

    merge_dct_halves(c, stride, bounds, [t4, t5, t6, t7])?;
  };

  Ok(())
}

pub fn rav1d_inv_dct8_1d_c(
  coefficients: &mut [i32],
  stride: NonZeroU32,
  bounds: CoefficientBounds,
) -> Rav1dResult {
  let lane = checked_dct_lane::<8>(coefficients, stride)?;
  inv_dct8_1d_internal_c(lane, stride, bounds, DctInput::Full)
}

#[inline]
fn rotate_dct16_odd_inputs(
  c: &[i32],
  stride: NonZeroU32,
  input: DctInput,
) -> Rav1dResult<[i32; 8]> {
  let in1 = read_coefficient(c, stride, 1)?;
  let in3 = read_coefficient(c, stride, 3)?;
  let in5 = read_coefficient(c, stride, 5)?;
  let in7 = read_coefficient(c, stride, 7)?;

  let t8a;
  let t9a;
  let t10a;
  let t11a;
  let t12a;
  let t13a;
  let t14a;
  let t15a;

  match ::core::primitive::bool::then_some(!matches!(input, DctInput::Full), ()) {
    ::core::option::Option::Some(()) => {
      t8a = i32::wrapping_shr(i32::wrapping_add(i32::wrapping_mul(in1, 401), 2048), 12_u32);
      t9a = i32::wrapping_shr(
        i32::wrapping_add(i32::wrapping_mul(in7, -2598), 2048),
        12_u32,
      );
      t10a = i32::wrapping_shr(
        i32::wrapping_add(i32::wrapping_mul(in5, 1931), 2048),
        12_u32,
      );
      t11a = i32::wrapping_shr(
        i32::wrapping_add(i32::wrapping_mul(in3, -1189), 2048),
        12_u32,
      );
      t12a = i32::wrapping_shr(
        i32::wrapping_add(i32::wrapping_mul(in3, 3920), 2048),
        12_u32,
      );
      t13a = i32::wrapping_shr(
        i32::wrapping_add(i32::wrapping_mul(in5, 3612), 2048),
        12_u32,
      );
      t14a = i32::wrapping_shr(
        i32::wrapping_add(i32::wrapping_mul(in7, 3166), 2048),
        12_u32,
      );
      t15a = i32::wrapping_shr(
        i32::wrapping_add(i32::wrapping_mul(in1, 4076), 2048),
        12_u32,
      );
    }
    ::core::option::Option::None => {
      let in9 = read_coefficient(c, stride, 9)?;
      let in11 = read_coefficient(c, stride, 11)?;
      let in13 = read_coefficient(c, stride, 13)?;
      let in15 = read_coefficient(c, stride, 15)?;

      t8a = i32::wrapping_sub(
        i32::wrapping_shr(
          i32::wrapping_add(
            i32::wrapping_sub(
              i32::wrapping_mul(in1, 401),
              i32::wrapping_mul(in15, i32::wrapping_sub(4076, 4096)),
            ),
            2048,
          ),
          12_u32,
        ),
        in15,
      );
      t9a = i32::wrapping_shr(
        i32::wrapping_add(
          i32::wrapping_sub(i32::wrapping_mul(in9, 1583), i32::wrapping_mul(in7, 1299)),
          1024,
        ),
        11_u32,
      );
      t10a = i32::wrapping_sub(
        i32::wrapping_shr(
          i32::wrapping_add(
            i32::wrapping_sub(
              i32::wrapping_mul(in5, 1931),
              i32::wrapping_mul(in11, i32::wrapping_sub(3612, 4096)),
            ),
            2048,
          ),
          12_u32,
        ),
        in11,
      );
      t11a = i32::wrapping_add(
        i32::wrapping_shr(
          i32::wrapping_add(
            i32::wrapping_sub(
              i32::wrapping_mul(in13, i32::wrapping_sub(3920, 4096)),
              i32::wrapping_mul(in3, 1189),
            ),
            2048,
          ),
          12_u32,
        ),
        in13,
      );
      t12a = i32::wrapping_add(
        i32::wrapping_shr(
          i32::wrapping_add(
            i32::wrapping_add(
              i32::wrapping_mul(in13, 1189),
              i32::wrapping_mul(in3, i32::wrapping_sub(3920, 4096)),
            ),
            2048,
          ),
          12_u32,
        ),
        in3,
      );
      t13a = i32::wrapping_add(
        i32::wrapping_shr(
          i32::wrapping_add(
            i32::wrapping_add(
              i32::wrapping_mul(in5, i32::wrapping_sub(3612, 4096)),
              i32::wrapping_mul(in11, 1931),
            ),
            2048,
          ),
          12_u32,
        ),
        in5,
      );
      t14a = i32::wrapping_shr(
        i32::wrapping_add(
          i32::wrapping_add(i32::wrapping_mul(in9, 1299), i32::wrapping_mul(in7, 1583)),
          1024,
        ),
        11_u32,
      );
      t15a = i32::wrapping_add(
        i32::wrapping_shr(
          i32::wrapping_add(
            i32::wrapping_add(
              i32::wrapping_mul(in1, i32::wrapping_sub(4076, 4096)),
              i32::wrapping_mul(in15, 401),
            ),
            2048,
          ),
          12_u32,
        ),
        in1,
      );
    }
  }

  Ok([t8a, t9a, t10a, t11a, t12a, t13a, t14a, t15a])
}

#[inline]
fn rotate_dct16_middle([t9, t14, t10, t13]: [i32; 4]) -> Result<[i32; 4], Never> {
  let t9a = i32::wrapping_sub(
    i32::wrapping_shr(
      i32::wrapping_add(
        i32::wrapping_sub(
          i32::wrapping_mul(t14, 1567),
          i32::wrapping_mul(t9, i32::wrapping_sub(3784, 4096)),
        ),
        2048,
      ),
      12_u32,
    ),
    t9,
  );
  let t14a = i32::wrapping_add(
    i32::wrapping_shr(
      i32::wrapping_add(
        i32::wrapping_add(
          i32::wrapping_mul(t14, i32::wrapping_sub(3784, 4096)),
          i32::wrapping_mul(t9, 1567),
        ),
        2048,
      ),
      12_u32,
    ),
    t14,
  );
  let t10a = i32::wrapping_sub(
    i32::wrapping_shr(
      i32::wrapping_add(
        (i32::wrapping_add(
          i32::wrapping_mul(t13, i32::wrapping_sub(3784, 4096)),
          i32::wrapping_mul(t10, 1567),
        ))
        .wrapping_neg(),
        2048,
      ),
      12_u32,
    ),
    t13,
  );
  let t13a = i32::wrapping_sub(
    i32::wrapping_shr(
      i32::wrapping_add(
        i32::wrapping_sub(
          i32::wrapping_mul(t13, 1567),
          i32::wrapping_mul(t10, i32::wrapping_sub(3784, 4096)),
        ),
        2048,
      ),
      12_u32,
    ),
    t10,
  );
  Ok([t9a, t14a, t10a, t13a])
}

#[inline(never)]
fn inv_dct16_1d_internal_c(
  c: &mut [i32],
  stride: NonZeroU32,
  bounds: CoefficientBounds,
  input: DctInput,
) -> Rav1dResult {
  let CoefficientBounds {
    lower: min,
    upper: max,
  } = bounds;

  {
    let clip = |v| iclip(v, min, max);
    let even_stride = stride.checked_add(stride.get()).ok_or(Rav1dError::ERANGE)?;

    let even_input = match input {
      #[cfg(target_arch = "x86_64")]
      DctInput::LowOdd => DctInput::Full,
      DctInput::Full | DctInput::LowHalf => input,
    };
    inv_dct8_1d_internal_c(c, even_stride, bounds, even_input)?;

    let [
      mut t8a,
      mut t9a,
      mut t10a,
      mut t11a,
      mut t12a,
      mut t13a,
      mut t14a,
      mut t15a,
    ] = rotate_dct16_odd_inputs(c, stride, input)?;

    let t8 = clip(i32::wrapping_add(t8a, t9a));
    let mut t9 = clip(i32::wrapping_sub(t8a, t9a));
    let mut t10 = clip(i32::wrapping_sub(t11a, t10a));
    let mut t11 = clip(i32::wrapping_add(t11a, t10a));
    let mut t12 = clip(i32::wrapping_add(t12a, t13a));
    let mut t13 = clip(i32::wrapping_sub(t12a, t13a));
    let mut t14 = clip(i32::wrapping_sub(t15a, t14a));
    let t15 = clip(i32::wrapping_add(t15a, t14a));

    let Ok(rotated) = rotate_dct16_middle([t9, t14, t10, t13]);
    [t9a, t14a, t10a, t13a] = rotated;

    t8a = clip(i32::wrapping_add(t8, t11));
    t9 = clip(i32::wrapping_add(t9a, t10a));
    t10 = clip(i32::wrapping_sub(t9a, t10a));
    t11a = clip(i32::wrapping_sub(t8, t11));
    t12a = clip(i32::wrapping_sub(t15, t12));
    t13 = clip(i32::wrapping_sub(t14a, t13a));
    t14 = clip(i32::wrapping_add(t14a, t13a));
    t15a = clip(i32::wrapping_add(t15, t12));

    t10a = i32::wrapping_shr(
      i32::wrapping_add(i32::wrapping_mul(i32::wrapping_sub(t13, t10), 181), 128),
      8_u32,
    );
    t13a = i32::wrapping_shr(
      i32::wrapping_add(i32::wrapping_mul(i32::wrapping_add(t13, t10), 181), 128),
      8_u32,
    );
    t11 = i32::wrapping_shr(
      i32::wrapping_add(i32::wrapping_mul(i32::wrapping_sub(t12a, t11a), 181), 128),
      8_u32,
    );
    t12 = i32::wrapping_shr(
      i32::wrapping_add(i32::wrapping_mul(i32::wrapping_add(t12a, t11a), 181), 128),
      8_u32,
    );

    merge_dct_halves(
      c,
      stride,
      bounds,
      [t8a, t9, t10a, t11, t12, t13a, t14, t15a],
    )?;
  };

  Ok(())
}

pub fn rav1d_inv_dct16_1d_c(
  coefficients: &mut [i32],
  stride: NonZeroU32,
  bounds: CoefficientBounds,
) -> Rav1dResult {
  let lane = checked_dct_lane::<16>(coefficients, stride)?;
  inv_dct16_1d_internal_c(lane, stride, bounds, DctInput::Full)
}

#[inline]
fn rotate_dct32_odd_inputs(
  c: &[i32],
  stride: NonZeroU32,
  input: DctInput,
) -> Rav1dResult<[i32; 16]> {
  let in1 = read_coefficient(c, stride, 1)?;
  let in3 = read_coefficient(c, stride, 3)?;
  let in5 = read_coefficient(c, stride, 5)?;
  let in7 = read_coefficient(c, stride, 7)?;
  let in9 = read_coefficient(c, stride, 9)?;
  let in11 = read_coefficient(c, stride, 11)?;
  let in13 = read_coefficient(c, stride, 13)?;
  let in15 = read_coefficient(c, stride, 15)?;

  let t16a;
  let t17a;
  let t18a;
  let t19a;
  let t20a;
  let t21a;
  let t22a;
  let t23a;
  let t24a;
  let t25a;
  let t26a;
  let t27a;
  let t28a;
  let t29a;
  let t30a;
  let t31a;

  match ::core::primitive::bool::then_some(!matches!(input, DctInput::Full), ()) {
    ::core::option::Option::Some(()) => {
      t16a = round_shift(in1.wrapping_mul(201), 12);
      t17a = round_shift(in15.wrapping_mul(-2751), 12);
      t18a = round_shift(in9.wrapping_mul(1751), 12);
      t19a = round_shift(in7.wrapping_mul(-1380), 12);
      t20a = round_shift(in5.wrapping_mul(995), 12);
      t21a = round_shift(in11.wrapping_mul(-2106), 12);
      t22a = round_shift(in13.wrapping_mul(2440), 12);
      t23a = round_shift(in3.wrapping_mul(-601), 12);
      t24a = round_shift(in3.wrapping_mul(4052), 12);
      t25a = round_shift(in13.wrapping_mul(3290), 12);
      t26a = round_shift(in11.wrapping_mul(3513), 12);
      t27a = round_shift(in5.wrapping_mul(3973), 12);
      t28a = round_shift(in7.wrapping_mul(3857), 12);
      t29a = round_shift(in9.wrapping_mul(3703), 12);
      t30a = round_shift(in15.wrapping_mul(3035), 12);
      t31a = round_shift(in1.wrapping_mul(4091), 12);
    }
    ::core::option::Option::None => {
      let in17 = read_coefficient(c, stride, 17)?;
      let in19 = read_coefficient(c, stride, 19)?;
      let in21 = read_coefficient(c, stride, 21)?;
      let in23 = read_coefficient(c, stride, 23)?;
      let in25 = read_coefficient(c, stride, 25)?;
      let in27 = read_coefficient(c, stride, 27)?;
      let in29 = read_coefficient(c, stride, 29)?;
      let in31 = read_coefficient(c, stride, 31)?;

      t16a = i32::wrapping_sub(
        round_shift(
          in1
            .wrapping_mul(201)
            .wrapping_sub(in31.wrapping_mul(4091_i32.wrapping_sub(4096))),
          12,
        ),
        in31,
      );
      t17a = i32::wrapping_add(
        round_shift(
          in17
            .wrapping_mul(3035_i32.wrapping_sub(4096))
            .wrapping_sub(in15.wrapping_mul(2751)),
          12,
        ),
        in17,
      );
      t18a = i32::wrapping_sub(
        round_shift(
          in9
            .wrapping_mul(1751)
            .wrapping_sub(in23.wrapping_mul(3703_i32.wrapping_sub(4096))),
          12,
        ),
        in23,
      );
      t19a = i32::wrapping_add(
        round_shift(
          in25
            .wrapping_mul(3857_i32.wrapping_sub(4096))
            .wrapping_sub(in7.wrapping_mul(1380)),
          12,
        ),
        in25,
      );
      t20a = i32::wrapping_sub(
        round_shift(
          in5
            .wrapping_mul(995)
            .wrapping_sub(in27.wrapping_mul(3973_i32.wrapping_sub(4096))),
          12,
        ),
        in27,
      );
      t21a = i32::wrapping_add(
        round_shift(
          in21
            .wrapping_mul(3513_i32.wrapping_sub(4096))
            .wrapping_sub(in11.wrapping_mul(2106)),
          12,
        ),
        in21,
      );
      t22a = round_shift(
        in13
          .wrapping_mul(1220)
          .wrapping_sub(in19.wrapping_mul(1645)),
        11,
      );
      t23a = i32::wrapping_add(
        round_shift(
          in29
            .wrapping_mul(4052_i32.wrapping_sub(4096))
            .wrapping_sub(in3.wrapping_mul(601)),
          12,
        ),
        in29,
      );
      t24a = i32::wrapping_add(
        round_shift(
          in29
            .wrapping_mul(601)
            .wrapping_add(in3.wrapping_mul(4052_i32.wrapping_sub(4096))),
          12,
        ),
        in3,
      );
      t25a = round_shift(
        in13
          .wrapping_mul(1645)
          .wrapping_add(in19.wrapping_mul(1220)),
        11,
      );
      t26a = i32::wrapping_add(
        round_shift(
          in21
            .wrapping_mul(2106)
            .wrapping_add(in11.wrapping_mul(3513_i32.wrapping_sub(4096))),
          12,
        ),
        in11,
      );
      t27a = i32::wrapping_add(
        round_shift(
          in5
            .wrapping_mul(3973_i32.wrapping_sub(4096))
            .wrapping_add(in27.wrapping_mul(995)),
          12,
        ),
        in5,
      );
      t28a = i32::wrapping_add(
        round_shift(
          in25
            .wrapping_mul(1380)
            .wrapping_add(in7.wrapping_mul(3857_i32.wrapping_sub(4096))),
          12,
        ),
        in7,
      );
      t29a = i32::wrapping_add(
        round_shift(
          in9
            .wrapping_mul(3703_i32.wrapping_sub(4096))
            .wrapping_add(in23.wrapping_mul(1751)),
          12,
        ),
        in9,
      );
      t30a = i32::wrapping_add(
        round_shift(
          in17
            .wrapping_mul(2751)
            .wrapping_add(in15.wrapping_mul(3035_i32.wrapping_sub(4096))),
          12,
        ),
        in15,
      );
      t31a = i32::wrapping_add(
        round_shift(
          in1
            .wrapping_mul(4091_i32.wrapping_sub(4096))
            .wrapping_add(in31.wrapping_mul(201)),
          12,
        ),
        in1,
      );
    }
  }

  Ok([
    t16a, t17a, t18a, t19a, t20a, t21a, t22a, t23a, t24a, t25a, t26a, t27a, t28a, t29a, t30a,
    t31a,
  ])
}

#[inline]
fn butterfly_dct32_odd_pairs(
  [
    t16a,
    t17a,
    t18a,
    t19a,
    t20a,
    t21a,
    t22a,
    t23a,
    t24a,
    t25a,
    t26a,
    t27a,
    t28a,
    t29a,
    t30a,
    t31a,
  ]: [i32; 16],
  bounds: CoefficientBounds,
) -> Result<[i32; 16], Never> {
  let clip = |value| iclip(value, bounds.lower, bounds.upper);

  let t16 = clip(i32::wrapping_add(t16a, t17a));
  let t17 = clip(i32::wrapping_sub(t16a, t17a));
  let t18 = clip(i32::wrapping_sub(t19a, t18a));
  let t19 = clip(i32::wrapping_add(t19a, t18a));
  let t20 = clip(i32::wrapping_add(t20a, t21a));
  let t21 = clip(i32::wrapping_sub(t20a, t21a));
  let t22 = clip(i32::wrapping_sub(t23a, t22a));
  let t23 = clip(i32::wrapping_add(t23a, t22a));
  let t24 = clip(i32::wrapping_add(t24a, t25a));
  let t25 = clip(i32::wrapping_sub(t24a, t25a));
  let t26 = clip(i32::wrapping_sub(t27a, t26a));
  let t27 = clip(i32::wrapping_add(t27a, t26a));
  let t28 = clip(i32::wrapping_add(t28a, t29a));
  let t29 = clip(i32::wrapping_sub(t28a, t29a));
  let t30 = clip(i32::wrapping_sub(t31a, t30a));
  let t31 = clip(i32::wrapping_add(t31a, t30a));

  Ok([
    t16, t17, t18, t19, t20, t21, t22, t23, t24, t25, t26, t27, t28, t29, t30, t31,
  ])
}

#[inline]
fn rotate_dct32_odd_pairs(
  [
    t16,
    t17,
    t18,
    t19,
    t20,
    t21,
    t22,
    t23,
    t24,
    t25,
    t26,
    t27,
    t28,
    t29,
    t30,
    t31,
  ]: [i32; 16],
) -> Result<[i32; 16], Never> {
  let t17a = round_shift(
    t30
      .wrapping_mul(799)
      .wrapping_sub(t17.wrapping_mul(4017_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_sub(t17);
  let t30a = round_shift(
    t30
      .wrapping_mul(4017_i32.wrapping_sub(4096))
      .wrapping_add(t17.wrapping_mul(799)),
    12,
  )
  .wrapping_add(t30);
  let t18a = round_shift(
    (i32::wrapping_add(
      i32::wrapping_mul(t29, i32::wrapping_sub(4017, 4096)),
      i32::wrapping_mul(t18, 799),
    ))
    .wrapping_neg(),
    12,
  )
  .wrapping_sub(t29);
  let t29a = round_shift(
    t29
      .wrapping_mul(799)
      .wrapping_sub(t18.wrapping_mul(4017_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_sub(t18);
  let t21a = round_shift(
    t26.wrapping_mul(1703).wrapping_sub(t21.wrapping_mul(1138)),
    11,
  );
  let t26a = round_shift(
    t26.wrapping_mul(1138).wrapping_add(t21.wrapping_mul(1703)),
    11,
  );
  let t22a = round_shift(
    (i32::wrapping_add(i32::wrapping_mul(t25, 1138), i32::wrapping_mul(t22, 1703)))
      .wrapping_neg(),
    11,
  );
  let t25a = round_shift(
    t25.wrapping_mul(1703).wrapping_sub(t22.wrapping_mul(1138)),
    11,
  );

  Ok([
    t16, t17a, t18a, t19, t20, t21a, t22a, t23, t24, t25a, t26a, t27, t28, t29a, t30a, t31,
  ])
}

#[inline]
fn butterfly_dct32_odd_quarters(
  [
    t16,
    t17a,
    t18a,
    t19,
    t20,
    t21a,
    t22a,
    t23,
    t24,
    t25a,
    t26a,
    t27,
    t28,
    t29a,
    t30a,
    t31,
  ]: [i32; 16],
  bounds: CoefficientBounds,
) -> Result<[i32; 16], Never> {
  let clip = |value| iclip(value, bounds.lower, bounds.upper);

  let t16a = clip(i32::wrapping_add(t16, t19));
  let t17 = clip(i32::wrapping_add(t17a, t18a));
  let t18 = clip(i32::wrapping_sub(t17a, t18a));
  let t19a = clip(i32::wrapping_sub(t16, t19));
  let t20a = clip(i32::wrapping_sub(t23, t20));
  let t21 = clip(i32::wrapping_sub(t22a, t21a));
  let t22 = clip(i32::wrapping_add(t22a, t21a));
  let t23a = clip(i32::wrapping_add(t23, t20));
  let t24a = clip(i32::wrapping_add(t24, t27));
  let t25 = clip(i32::wrapping_add(t25a, t26a));
  let t26 = clip(i32::wrapping_sub(t25a, t26a));
  let t27a = clip(i32::wrapping_sub(t24, t27));
  let t28a = clip(i32::wrapping_sub(t31, t28));
  let t29 = clip(i32::wrapping_sub(t30a, t29a));
  let t30 = clip(i32::wrapping_add(t30a, t29a));
  let t31a = clip(i32::wrapping_add(t31, t28));

  Ok([
    t16a, t17, t18, t19a, t20a, t21, t22, t23a, t24a, t25, t26, t27a, t28a, t29, t30, t31a,
  ])
}

#[inline]
fn rotate_dct32_odd_quarters(
  [
    t16a,
    t17,
    t18,
    t19a,
    t20a,
    t21,
    t22,
    t23a,
    t24a,
    t25,
    t26,
    t27a,
    t28a,
    t29,
    t30,
    t31a,
  ]: [i32; 16],
) -> Result<[i32; 16], Never> {
  let t18a = round_shift(
    t29
      .wrapping_mul(1567)
      .wrapping_sub(t18.wrapping_mul(3784_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_sub(t18);
  let t29a = round_shift(
    t29
      .wrapping_mul(3784_i32.wrapping_sub(4096))
      .wrapping_add(t18.wrapping_mul(1567)),
    12,
  )
  .wrapping_add(t29);
  let t19 = round_shift(
    t28a
      .wrapping_mul(1567)
      .wrapping_sub(t19a.wrapping_mul(3784_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_sub(t19a);
  let t28 = round_shift(
    t28a
      .wrapping_mul(3784_i32.wrapping_sub(4096))
      .wrapping_add(t19a.wrapping_mul(1567)),
    12,
  )
  .wrapping_add(t28a);
  let t20 = round_shift(
    (i32::wrapping_add(
      i32::wrapping_mul(t27a, i32::wrapping_sub(3784, 4096)),
      i32::wrapping_mul(t20a, 1567),
    ))
    .wrapping_neg(),
    12,
  )
  .wrapping_sub(t27a);
  let t27 = round_shift(
    t27a
      .wrapping_mul(1567)
      .wrapping_sub(t20a.wrapping_mul(3784_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_sub(t20a);
  let t21a = round_shift(
    (i32::wrapping_add(
      i32::wrapping_mul(t26, i32::wrapping_sub(3784, 4096)),
      i32::wrapping_mul(t21, 1567),
    ))
    .wrapping_neg(),
    12,
  )
  .wrapping_sub(t26);
  let t26a = round_shift(
    t26
      .wrapping_mul(1567)
      .wrapping_sub(t21.wrapping_mul(3784_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_sub(t21);

  Ok([
    t16a, t17, t18a, t19, t20, t21a, t22, t23a, t24a, t25, t26a, t27, t28, t29a, t30, t31a,
  ])
}

#[inline]
fn butterfly_dct32_odd_halves(
  [
    t16a,
    t17,
    t18a,
    t19,
    t20,
    t21a,
    t22,
    t23a,
    t24a,
    t25,
    t26a,
    t27,
    t28,
    t29a,
    t30,
    t31a,
  ]: [i32; 16],
  bounds: CoefficientBounds,
) -> Result<[i32; 16], Never> {
  let clip = |value| iclip(value, bounds.lower, bounds.upper);

  let t16 = clip(i32::wrapping_add(t16a, t23a));
  let t17a = clip(i32::wrapping_add(t17, t22));
  let t18 = clip(i32::wrapping_add(t18a, t21a));
  let t19a = clip(i32::wrapping_add(t19, t20));
  let t20a = clip(i32::wrapping_sub(t19, t20));
  let t21 = clip(i32::wrapping_sub(t18a, t21a));
  let t22a = clip(i32::wrapping_sub(t17, t22));
  let t23 = clip(i32::wrapping_sub(t16a, t23a));
  let t24 = clip(i32::wrapping_sub(t31a, t24a));
  let t25a = clip(i32::wrapping_sub(t30, t25));
  let t26 = clip(i32::wrapping_sub(t29a, t26a));
  let t27a = clip(i32::wrapping_sub(t28, t27));
  let t28a = clip(i32::wrapping_add(t28, t27));
  let t29 = clip(i32::wrapping_add(t29a, t26a));
  let t30a = clip(i32::wrapping_add(t30, t25));
  let t31 = clip(i32::wrapping_add(t31a, t24a));

  Ok([
    t16, t17a, t18, t19a, t20a, t21, t22a, t23, t24, t25a, t26, t27a, t28a, t29, t30a, t31,
  ])
}

#[inline]
fn rotate_dct32_odd_center(
  [
    t16,
    t17a,
    t18,
    t19a,
    t20a,
    t21,
    t22a,
    t23,
    t24,
    t25a,
    t26,
    t27a,
    t28a,
    t29,
    t30a,
    t31,
  ]: [i32; 16],
) -> Result<[i32; 16], Never> {
  let t20 = round_shift(t27a.wrapping_sub(t20a).wrapping_mul(181), 8);
  let t27 = round_shift(t27a.wrapping_add(t20a).wrapping_mul(181), 8);
  let t21a = round_shift(t26.wrapping_sub(t21).wrapping_mul(181), 8);
  let t26a = round_shift(t26.wrapping_add(t21).wrapping_mul(181), 8);
  let t22 = round_shift(t25a.wrapping_sub(t22a).wrapping_mul(181), 8);
  let t25 = round_shift(t25a.wrapping_add(t22a).wrapping_mul(181), 8);
  let t23a = round_shift(t24.wrapping_sub(t23).wrapping_mul(181), 8);
  let t24a = round_shift(t24.wrapping_add(t23).wrapping_mul(181), 8);

  Ok([
    t16, t17a, t18, t19a, t20, t21a, t22, t23a, t24a, t25, t26a, t27, t28a, t29, t30a, t31,
  ])
}

#[inline(never)]
fn inv_dct32_1d_internal_c(
  c: &mut [i32],
  stride: NonZeroU32,
  bounds: CoefficientBounds,
  input: DctInput,
) -> Rav1dResult {
  {
    let even_stride = stride.checked_add(stride.get()).ok_or(Rav1dError::ERANGE)?;

    inv_dct16_1d_internal_c(c, even_stride, bounds, input)?;

    let odd_inputs = rotate_dct32_odd_inputs(c, stride, input)?;
    let Ok(pairs_mixed) = butterfly_dct32_odd_pairs(odd_inputs, bounds);
    let Ok(pairs_rotated) = rotate_dct32_odd_pairs(pairs_mixed);
    let Ok(quarters_mixed) = butterfly_dct32_odd_quarters(pairs_rotated, bounds);
    let Ok(quarters_rotated) = rotate_dct32_odd_quarters(quarters_mixed);
    let Ok(halves_mixed) = butterfly_dct32_odd_halves(quarters_rotated, bounds);
    let Ok(center_rotated) = rotate_dct32_odd_center(halves_mixed);

    merge_dct_halves(c, stride, bounds, center_rotated)?;
  };

  Ok(())
}

pub fn rav1d_inv_dct32_1d_c(
  coefficients: &mut [i32],
  stride: NonZeroU32,
  bounds: CoefficientBounds,
) -> Rav1dResult {
  let lane = checked_dct_lane::<32>(coefficients, stride)?;
  inv_dct32_1d_internal_c(lane, stride, bounds, DctInput::Full)
}

pub fn rav1d_inv_dct64_1d_c(
  coefficients: &mut [i32],
  stride: NonZeroU32,
  bounds: CoefficientBounds,
) -> Rav1dResult {
  let lane = checked_dct_lane::<64>(coefficients, stride)?;
  inv_dct64_1d(lane, stride, bounds, DctInput::LowHalf)
}

#[cfg(target_arch = "x86_64")]
#[inline]
pub(super) fn inv_dct64_1d_x86(
  coefficients: &mut [i32],
  stride: NonZeroU32,
  bounds: CoefficientBounds,
) -> Rav1dResult {
  let lane = checked_dct_lane::<64>(coefficients, stride)?;
  inv_dct64_1d(lane, stride, bounds, DctInput::LowOdd)
}

#[inline]
fn rotate_dct64_odd_inputs(c: &[i32], stride: NonZeroU32) -> Rav1dResult<[i32; 32]> {
  let in1 = read_coefficient(c, stride, 1)?;
  let in3 = read_coefficient(c, stride, 3)?;
  let in5 = read_coefficient(c, stride, 5)?;
  let in7 = read_coefficient(c, stride, 7)?;
  let in9 = read_coefficient(c, stride, 9)?;
  let in11 = read_coefficient(c, stride, 11)?;
  let in13 = read_coefficient(c, stride, 13)?;
  let in15 = read_coefficient(c, stride, 15)?;
  let in17 = read_coefficient(c, stride, 17)?;
  let in19 = read_coefficient(c, stride, 19)?;
  let in21 = read_coefficient(c, stride, 21)?;
  let in23 = read_coefficient(c, stride, 23)?;
  let in25 = read_coefficient(c, stride, 25)?;
  let in27 = read_coefficient(c, stride, 27)?;
  let in29 = read_coefficient(c, stride, 29)?;
  let in31 = read_coefficient(c, stride, 31)?;

  let t32a = round_shift(in1.wrapping_mul(101), 12);
  let t33a = round_shift(in31.wrapping_mul(-2824), 12);
  let t34a = round_shift(in17.wrapping_mul(1660), 12);
  let t35a = round_shift(in15.wrapping_mul(-1474), 12);
  let t36a = round_shift(in9.wrapping_mul(897), 12);
  let t37a = round_shift(in23.wrapping_mul(-2191), 12);
  let t38a = round_shift(in25.wrapping_mul(2359), 12);
  let t39a = round_shift(in7.wrapping_mul(-700), 12);
  let t40a = round_shift(in5.wrapping_mul(501), 12);
  let t41a = round_shift(in27.wrapping_mul(-2520), 12);
  let t42a = round_shift(in21.wrapping_mul(2019), 12);
  let t43a = round_shift(in11.wrapping_mul(-1092), 12);
  let t44a = round_shift(in13.wrapping_mul(1285), 12);
  let t45a = round_shift(in19.wrapping_mul(-1842), 12);
  let t46a = round_shift(in29.wrapping_mul(2675), 12);
  let t47a = round_shift(in3.wrapping_mul(-301), 12);
  let t48a = round_shift(in3.wrapping_mul(4085), 12);
  let t49a = round_shift(in29.wrapping_mul(3102), 12);
  let t50a = round_shift(in19.wrapping_mul(3659), 12);
  let t51a = round_shift(in13.wrapping_mul(3889), 12);
  let t52a = round_shift(in11.wrapping_mul(3948), 12);
  let t53a = round_shift(in21.wrapping_mul(3564), 12);
  let t54a = round_shift(in27.wrapping_mul(3229), 12);
  let t55a = round_shift(in5.wrapping_mul(4065), 12);
  let t56a = round_shift(in7.wrapping_mul(4036), 12);
  let t57a = round_shift(in25.wrapping_mul(3349), 12);
  let t58a = round_shift(in23.wrapping_mul(3461), 12);
  let t59a = round_shift(in9.wrapping_mul(3996), 12);
  let t60a = round_shift(in15.wrapping_mul(3822), 12);
  let t61a = round_shift(in17.wrapping_mul(3745), 12);
  let t62a = round_shift(in31.wrapping_mul(2967), 12);
  let t63a = round_shift(in1.wrapping_mul(4095), 12);

  Ok([
    t32a, t33a, t34a, t35a, t36a, t37a, t38a, t39a, t40a, t41a, t42a, t43a, t44a, t45a, t46a,
    t47a, t48a, t49a, t50a, t51a, t52a, t53a, t54a, t55a, t56a, t57a, t58a, t59a, t60a, t61a,
    t62a, t63a,
  ])
}

#[inline]
fn butterfly_dct64_odd_pairs(
  [
    t32a,
    t33a,
    t34a,
    t35a,
    t36a,
    t37a,
    t38a,
    t39a,
    t40a,
    t41a,
    t42a,
    t43a,
    t44a,
    t45a,
    t46a,
    t47a,
    t48a,
    t49a,
    t50a,
    t51a,
    t52a,
    t53a,
    t54a,
    t55a,
    t56a,
    t57a,
    t58a,
    t59a,
    t60a,
    t61a,
    t62a,
    t63a,
  ]: [i32; 32],
  bounds: CoefficientBounds,
) -> Result<[i32; 32], Never> {
  let clip = |value| iclip(value, bounds.lower, bounds.upper);

  let t32 = clip(i32::wrapping_add(t32a, t33a));
  let t33 = clip(i32::wrapping_sub(t32a, t33a));
  let t34 = clip(i32::wrapping_sub(t35a, t34a));
  let t35 = clip(i32::wrapping_add(t35a, t34a));
  let t36 = clip(i32::wrapping_add(t36a, t37a));
  let t37 = clip(i32::wrapping_sub(t36a, t37a));
  let t38 = clip(i32::wrapping_sub(t39a, t38a));
  let t39 = clip(i32::wrapping_add(t39a, t38a));
  let t40 = clip(i32::wrapping_add(t40a, t41a));
  let t41 = clip(i32::wrapping_sub(t40a, t41a));
  let t42 = clip(i32::wrapping_sub(t43a, t42a));
  let t43 = clip(i32::wrapping_add(t43a, t42a));
  let t44 = clip(i32::wrapping_add(t44a, t45a));
  let t45 = clip(i32::wrapping_sub(t44a, t45a));
  let t46 = clip(i32::wrapping_sub(t47a, t46a));
  let t47 = clip(i32::wrapping_add(t47a, t46a));
  let t48 = clip(i32::wrapping_add(t48a, t49a));
  let t49 = clip(i32::wrapping_sub(t48a, t49a));
  let t50 = clip(i32::wrapping_sub(t51a, t50a));
  let t51 = clip(i32::wrapping_add(t51a, t50a));
  let t52 = clip(i32::wrapping_add(t52a, t53a));
  let t53 = clip(i32::wrapping_sub(t52a, t53a));
  let t54 = clip(i32::wrapping_sub(t55a, t54a));
  let t55 = clip(i32::wrapping_add(t55a, t54a));
  let t56 = clip(i32::wrapping_add(t56a, t57a));
  let t57 = clip(i32::wrapping_sub(t56a, t57a));
  let t58 = clip(i32::wrapping_sub(t59a, t58a));
  let t59 = clip(i32::wrapping_add(t59a, t58a));
  let t60 = clip(i32::wrapping_add(t60a, t61a));
  let t61 = clip(i32::wrapping_sub(t60a, t61a));
  let t62 = clip(i32::wrapping_sub(t63a, t62a));
  let t63 = clip(i32::wrapping_add(t63a, t62a));

  Ok([
    t32, t33, t34, t35, t36, t37, t38, t39, t40, t41, t42, t43, t44, t45, t46, t47, t48, t49,
    t50, t51, t52, t53, t54, t55, t56, t57, t58, t59, t60, t61, t62, t63,
  ])
}

struct Dct64ReducedAngle {
  cosine: i32,
  sine: i32,
}

impl Dct64ReducedAngle {
  fn forward(&self, [first, second]: [i32; 2]) -> Result<[i32; 2], Never> {
    let reduced = 4096i32.wrapping_sub(self.cosine);
    let inverse = self.cosine.wrapping_sub(4096);
    let low = round_shift(
      first
        .wrapping_mul(reduced)
        .wrapping_add(second.wrapping_mul(self.sine)),
      12,
    )
    .wrapping_sub(first);
    let high = round_shift(
      first
        .wrapping_mul(self.sine)
        .wrapping_add(second.wrapping_mul(inverse)),
      12,
    )
    .wrapping_add(second);
    Ok([low, high])
  }

  fn reverse(&self, [first, second]: [i32; 2]) -> Result<[i32; 2], Never> {
    let reduced = 4096i32.wrapping_sub(self.cosine);
    let low = round_shift(
      first
        .wrapping_mul(self.sine.wrapping_neg())
        .wrapping_add(second.wrapping_mul(reduced)),
      12,
    )
    .wrapping_sub(second);
    let high = round_shift(
      first
        .wrapping_mul(reduced)
        .wrapping_add(second.wrapping_mul(self.sine)),
      12,
    )
    .wrapping_sub(first);
    Ok([low, high])
  }
}

#[inline]
fn rotate_dct64_odd_pairs(
  [
    t32,
    t33,
    t34,
    t35,
    t36,
    t37,
    t38,
    t39,
    t40,
    t41,
    t42,
    t43,
    t44,
    t45,
    t46,
    t47,
    t48,
    t49,
    t50,
    t51,
    t52,
    t53,
    t54,
    t55,
    t56,
    t57,
    t58,
    t59,
    t60,
    t61,
    t62,
    t63,
  ]: [i32; 32],
) -> Result<[i32; 32], Never> {
  let first = Dct64ReducedAngle {
    cosine: 4076,
    sine: 401,
  };
  let Ok([t33a, t62a]) = first.forward([t33, t62]);
  let Ok([t34a, t61a]) = first.reverse([t34, t61]);
  let t37a = round_shift(
    t37.wrapping_mul(-1299).wrapping_add(t58.wrapping_mul(1583)),
    11,
  );
  let t38a = round_shift(
    t38
      .wrapping_mul(-1583)
      .wrapping_add(t57.wrapping_mul(-1299)),
    11,
  );
  let middle = Dct64ReducedAngle {
    cosine: 3612,
    sine: 1931,
  };
  let Ok([t41a, t54a]) = middle.forward([t41, t54]);
  let Ok([t42a, t53a]) = middle.reverse([t42, t53]);
  let t45a = round_shift(
    t45
      .wrapping_mul(-1189)
      .wrapping_add(t50.wrapping_mul(3920_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_add(t50);
  let center = Dct64ReducedAngle {
    cosine: 3920,
    sine: -1189,
  };
  let Ok([t46a, t49a]) = center.forward([t46, t49]);
  let t50a = round_shift(
    t45
      .wrapping_mul(3920_i32.wrapping_sub(4096))
      .wrapping_add(t50.wrapping_mul(1189)),
    12,
  )
  .wrapping_add(t45);
  let t57a = round_shift(
    t38.wrapping_mul(-1299).wrapping_add(t57.wrapping_mul(1583)),
    11,
  );
  let t58a = round_shift(
    t37.wrapping_mul(1583).wrapping_add(t58.wrapping_mul(1299)),
    11,
  );

  Ok([
    t32, t33a, t34a, t35, t36, t37a, t38a, t39, t40, t41a, t42a, t43, t44, t45a, t46a, t47, t48,
    t49a, t50a, t51, t52, t53a, t54a, t55, t56, t57a, t58a, t59, t60, t61a, t62a, t63,
  ])
}

#[inline]
fn butterfly_dct64_odd_quarters(
  [
    t32,
    t33a,
    t34a,
    t35,
    t36,
    t37a,
    t38a,
    t39,
    t40,
    t41a,
    t42a,
    t43,
    t44,
    t45a,
    t46a,
    t47,
    t48,
    t49a,
    t50a,
    t51,
    t52,
    t53a,
    t54a,
    t55,
    t56,
    t57a,
    t58a,
    t59,
    t60,
    t61a,
    t62a,
    t63,
  ]: [i32; 32],
  bounds: CoefficientBounds,
) -> Result<[i32; 32], Never> {
  let clip = |value| iclip(value, bounds.lower, bounds.upper);

  let t32a = clip(i32::wrapping_add(t32, t35));
  let t33 = clip(i32::wrapping_add(t33a, t34a));
  let t34 = clip(i32::wrapping_sub(t33a, t34a));
  let t35a = clip(i32::wrapping_sub(t32, t35));
  let t36a = clip(i32::wrapping_sub(t39, t36));
  let t37 = clip(i32::wrapping_sub(t38a, t37a));
  let t38 = clip(i32::wrapping_add(t38a, t37a));
  let t39a = clip(i32::wrapping_add(t39, t36));
  let t40a = clip(i32::wrapping_add(t40, t43));
  let t41 = clip(i32::wrapping_add(t41a, t42a));
  let t42 = clip(i32::wrapping_sub(t41a, t42a));
  let t43a = clip(i32::wrapping_sub(t40, t43));
  let t44a = clip(i32::wrapping_sub(t47, t44));
  let t45 = clip(i32::wrapping_sub(t46a, t45a));
  let t46 = clip(i32::wrapping_add(t46a, t45a));
  let t47a = clip(i32::wrapping_add(t47, t44));
  let t48a = clip(i32::wrapping_add(t48, t51));
  let t49 = clip(i32::wrapping_add(t49a, t50a));
  let t50 = clip(i32::wrapping_sub(t49a, t50a));
  let t51a = clip(i32::wrapping_sub(t48, t51));
  let t52a = clip(i32::wrapping_sub(t55, t52));
  let t53 = clip(i32::wrapping_sub(t54a, t53a));
  let t54 = clip(i32::wrapping_add(t54a, t53a));
  let t55a = clip(i32::wrapping_add(t55, t52));
  let t56a = clip(i32::wrapping_add(t56, t59));
  let t57 = clip(i32::wrapping_add(t57a, t58a));
  let t58 = clip(i32::wrapping_sub(t57a, t58a));
  let t59a = clip(i32::wrapping_sub(t56, t59));
  let t60a = clip(i32::wrapping_sub(t63, t60));
  let t61 = clip(i32::wrapping_sub(t62a, t61a));
  let t62 = clip(i32::wrapping_add(t62a, t61a));
  let t63a = clip(i32::wrapping_add(t63, t60));

  Ok([
    t32a, t33, t34, t35a, t36a, t37, t38, t39a, t40a, t41, t42, t43a, t44a, t45, t46, t47a, t48a,
    t49, t50, t51a, t52a, t53, t54, t55a, t56a, t57, t58, t59a, t60a, t61, t62, t63a,
  ])
}

#[inline]
fn rotate_dct64_odd_quarters(
  [
    t32a,
    t33,
    t34,
    t35a,
    t36a,
    t37,
    t38,
    t39a,
    t40a,
    t41,
    t42,
    t43a,
    t44a,
    t45,
    t46,
    t47a,
    t48a,
    t49,
    t50,
    t51a,
    t52a,
    t53,
    t54,
    t55a,
    t56a,
    t57,
    t58,
    t59a,
    t60a,
    t61,
    t62,
    t63a,
  ]: [i32; 32],
) -> Result<[i32; 32], Never> {
  let angle = Dct64ReducedAngle {
    cosine: 4017,
    sine: 799,
  };
  let Ok([t34a, t61a]) = angle.forward([t34, t61]);
  let Ok([t35, t60]) = angle.forward([t35a, t60a]);
  let Ok([t36, t59]) = angle.reverse([t36a, t59a]);
  let Ok([t37a, t58a]) = angle.reverse([t37, t58]);
  let t42a = round_shift(
    t42.wrapping_mul(-1138).wrapping_add(t53.wrapping_mul(1703)),
    11,
  );
  let t43 = round_shift(
    t43a
      .wrapping_mul(-1138)
      .wrapping_add(t52a.wrapping_mul(1703)),
    11,
  );
  let t44 = round_shift(
    t44a
      .wrapping_mul(-1703)
      .wrapping_add(t51a.wrapping_mul(-1138)),
    11,
  );
  let t45a = round_shift(
    t45
      .wrapping_mul(-1703)
      .wrapping_add(t50.wrapping_mul(-1138)),
    11,
  );
  let t50a = round_shift(
    t45.wrapping_mul(-1138).wrapping_add(t50.wrapping_mul(1703)),
    11,
  );
  let t51 = round_shift(
    t44a
      .wrapping_mul(-1138)
      .wrapping_add(t51a.wrapping_mul(1703)),
    11,
  );
  let t52 = round_shift(
    t43a
      .wrapping_mul(1703)
      .wrapping_add(t52a.wrapping_mul(1138)),
    11,
  );
  let t53a = round_shift(
    t42.wrapping_mul(1703).wrapping_add(t53.wrapping_mul(1138)),
    11,
  );

  Ok([
    t32a, t33, t34a, t35, t36, t37a, t38, t39a, t40a, t41, t42a, t43, t44, t45a, t46, t47a, t48a,
    t49, t50a, t51, t52, t53a, t54, t55a, t56a, t57, t58a, t59, t60, t61a, t62, t63a,
  ])
}

#[inline]
fn butterfly_dct64_odd_eighths(
  [
    t32a,
    t33,
    t34a,
    t35,
    t36,
    t37a,
    t38,
    t39a,
    t40a,
    t41,
    t42a,
    t43,
    t44,
    t45a,
    t46,
    t47a,
    t48a,
    t49,
    t50a,
    t51,
    t52,
    t53a,
    t54,
    t55a,
    t56a,
    t57,
    t58a,
    t59,
    t60,
    t61a,
    t62,
    t63a,
  ]: [i32; 32],
  bounds: CoefficientBounds,
) -> Result<[i32; 32], Never> {
  let clip = |value| iclip(value, bounds.lower, bounds.upper);

  let t32 = clip(i32::wrapping_add(t32a, t39a));
  let t33a = clip(i32::wrapping_add(t33, t38));
  let t34 = clip(i32::wrapping_add(t34a, t37a));
  let t35a = clip(i32::wrapping_add(t35, t36));
  let t36a = clip(i32::wrapping_sub(t35, t36));
  let t37 = clip(i32::wrapping_sub(t34a, t37a));
  let t38a = clip(i32::wrapping_sub(t33, t38));
  let t39 = clip(i32::wrapping_sub(t32a, t39a));
  let t40 = clip(i32::wrapping_sub(t47a, t40a));
  let t41a = clip(i32::wrapping_sub(t46, t41));
  let t42 = clip(i32::wrapping_sub(t45a, t42a));
  let t43a = clip(i32::wrapping_sub(t44, t43));
  let t44a = clip(i32::wrapping_add(t44, t43));
  let t45 = clip(i32::wrapping_add(t45a, t42a));
  let t46a = clip(i32::wrapping_add(t46, t41));
  let t47 = clip(i32::wrapping_add(t47a, t40a));
  let t48 = clip(i32::wrapping_add(t48a, t55a));
  let t49a = clip(i32::wrapping_add(t49, t54));
  let t50 = clip(i32::wrapping_add(t50a, t53a));
  let t51a = clip(i32::wrapping_add(t51, t52));
  let t52a = clip(i32::wrapping_sub(t51, t52));
  let t53 = clip(i32::wrapping_sub(t50a, t53a));
  let t54a = clip(i32::wrapping_sub(t49, t54));
  let t55 = clip(i32::wrapping_sub(t48a, t55a));
  let t56 = clip(i32::wrapping_sub(t63a, t56a));
  let t57a = clip(i32::wrapping_sub(t62, t57));
  let t58 = clip(i32::wrapping_sub(t61a, t58a));
  let t59a = clip(i32::wrapping_sub(t60, t59));
  let t60a = clip(i32::wrapping_add(t60, t59));
  let t61 = clip(i32::wrapping_add(t61a, t58a));
  let t62a = clip(i32::wrapping_add(t62, t57));
  let t63 = clip(i32::wrapping_add(t63a, t56a));

  Ok([
    t32, t33a, t34, t35a, t36a, t37, t38a, t39, t40, t41a, t42, t43a, t44a, t45, t46a, t47, t48,
    t49a, t50, t51a, t52a, t53, t54a, t55, t56, t57a, t58, t59a, t60a, t61, t62a, t63,
  ])
}

#[derive(Clone, Copy)]
enum EighthRotation {
  Forward,
  Reverse,
}

#[inline]
fn rotate_odd_eighth_pair(
  [first, second]: [i32; 2],
  rotation: EighthRotation,
) -> Result<[i32; 2], Never> {
  let cosine_correction = 312;
  let sine = 1567;
  let aligned = round_shift(
    first
      .wrapping_mul(cosine_correction)
      .wrapping_add(second.wrapping_mul(sine)),
    12,
  )
  .wrapping_sub(first);
  let cross = first
    .wrapping_mul(sine)
    .wrapping_sub(second.wrapping_mul(cosine_correction));

  let rotated = match rotation {
    EighthRotation::Forward => round_shift(cross, 12).wrapping_add(second),
    EighthRotation::Reverse => round_shift(cross.wrapping_neg(), 12).wrapping_sub(second),
  };

  Ok([aligned, rotated])
}

#[inline]
fn rotate_dct64_odd_eighths(
  [
    t32,
    t33a,
    t34,
    t35a,
    t36a,
    t37,
    t38a,
    t39,
    t40,
    t41a,
    t42,
    t43a,
    t44a,
    t45,
    t46a,
    t47,
    t48,
    t49a,
    t50,
    t51a,
    t52a,
    t53,
    t54a,
    t55,
    t56,
    t57a,
    t58,
    t59a,
    t60a,
    t61,
    t62a,
    t63,
  ]: [i32; 32],
) -> Result<[i32; 32], Never> {
  let Ok([t36, t59]) = rotate_odd_eighth_pair([t36a, t59a], EighthRotation::Forward);
  let Ok([t37a, t58a]) = rotate_odd_eighth_pair([t37, t58], EighthRotation::Forward);
  let Ok([t38, t57]) = rotate_odd_eighth_pair([t38a, t57a], EighthRotation::Forward);
  let Ok([t39a, t56a]) = rotate_odd_eighth_pair([t39, t56], EighthRotation::Forward);
  let Ok([t55a, t40a]) = rotate_odd_eighth_pair([t40, t55], EighthRotation::Reverse);
  let Ok([t54, t41]) = rotate_odd_eighth_pair([t41a, t54a], EighthRotation::Reverse);
  let Ok([t53a, t42a]) = rotate_odd_eighth_pair([t42, t53], EighthRotation::Reverse);
  let Ok([t52, t43]) = rotate_odd_eighth_pair([t43a, t52a], EighthRotation::Reverse);

  Ok([
    t32, t33a, t34, t35a, t36, t37a, t38, t39a, t40a, t41, t42a, t43, t44a, t45, t46a, t47, t48,
    t49a, t50, t51a, t52, t53a, t54, t55a, t56a, t57, t58a, t59, t60a, t61, t62a, t63,
  ])
}

#[inline]
fn butterfly_dct64_odd_halves(
  [
    t32,
    t33a,
    t34,
    t35a,
    t36,
    t37a,
    t38,
    t39a,
    t40a,
    t41,
    t42a,
    t43,
    t44a,
    t45,
    t46a,
    t47,
    t48,
    t49a,
    t50,
    t51a,
    t52,
    t53a,
    t54,
    t55a,
    t56a,
    t57,
    t58a,
    t59,
    t60a,
    t61,
    t62a,
    t63,
  ]: [i32; 32],
  bounds: CoefficientBounds,
) -> Result<[i32; 32], Never> {
  let clip = |value| iclip(value, bounds.lower, bounds.upper);

  let t32a = clip(i32::wrapping_add(t32, t47));
  let t33 = clip(i32::wrapping_add(t33a, t46a));
  let t34a = clip(i32::wrapping_add(t34, t45));
  let t35 = clip(i32::wrapping_add(t35a, t44a));
  let t36a = clip(i32::wrapping_add(t36, t43));
  let t37 = clip(i32::wrapping_add(t37a, t42a));
  let t38a = clip(i32::wrapping_add(t38, t41));
  let t39 = clip(i32::wrapping_add(t39a, t40a));
  let t40 = clip(i32::wrapping_sub(t39a, t40a));
  let t41a = clip(i32::wrapping_sub(t38, t41));
  let t42 = clip(i32::wrapping_sub(t37a, t42a));
  let t43a = clip(i32::wrapping_sub(t36, t43));
  let t44 = clip(i32::wrapping_sub(t35a, t44a));
  let t45a = clip(i32::wrapping_sub(t34, t45));
  let t46 = clip(i32::wrapping_sub(t33a, t46a));
  let t47a = clip(i32::wrapping_sub(t32, t47));
  let t48a = clip(i32::wrapping_sub(t63, t48));
  let t49 = clip(i32::wrapping_sub(t62a, t49a));
  let t50a = clip(i32::wrapping_sub(t61, t50));
  let t51 = clip(i32::wrapping_sub(t60a, t51a));
  let t52a = clip(i32::wrapping_sub(t59, t52));
  let t53 = clip(i32::wrapping_sub(t58a, t53a));
  let t54a = clip(i32::wrapping_sub(t57, t54));
  let t55 = clip(i32::wrapping_sub(t56a, t55a));
  let t56 = clip(i32::wrapping_add(t56a, t55a));
  let t57a = clip(i32::wrapping_add(t57, t54));
  let t58 = clip(i32::wrapping_add(t58a, t53a));
  let t59a = clip(i32::wrapping_add(t59, t52));
  let t60 = clip(i32::wrapping_add(t60a, t51a));
  let t61a = clip(i32::wrapping_add(t61, t50));
  let t62 = clip(i32::wrapping_add(t62a, t49a));
  let t63a = clip(i32::wrapping_add(t63, t48));

  Ok([
    t32a, t33, t34a, t35, t36a, t37, t38a, t39, t40, t41a, t42, t43a, t44, t45a, t46, t47a, t48a,
    t49, t50a, t51, t52a, t53, t54a, t55, t56, t57a, t58, t59a, t60, t61a, t62, t63a,
  ])
}

#[inline]
fn rotate_dct64_odd_center(
  [
    t32a,
    t33,
    t34a,
    t35,
    t36a,
    t37,
    t38a,
    t39,
    t40,
    t41a,
    t42,
    t43a,
    t44,
    t45a,
    t46,
    t47a,
    t48a,
    t49,
    t50a,
    t51,
    t52a,
    t53,
    t54a,
    t55,
    t56,
    t57a,
    t58,
    t59a,
    t60,
    t61a,
    t62,
    t63a,
  ]: [i32; 32],
) -> Result<[i32; 32], Never> {
  let t40a = round_shift(t55.wrapping_sub(t40).wrapping_mul(181), 8);
  let t41 = round_shift(t54a.wrapping_sub(t41a).wrapping_mul(181), 8);
  let t42a = round_shift(t53.wrapping_sub(t42).wrapping_mul(181), 8);
  let t43 = round_shift(t52a.wrapping_sub(t43a).wrapping_mul(181), 8);
  let t44a = round_shift(t51.wrapping_sub(t44).wrapping_mul(181), 8);
  let t45 = round_shift(t50a.wrapping_sub(t45a).wrapping_mul(181), 8);
  let t46a = round_shift(t49.wrapping_sub(t46).wrapping_mul(181), 8);
  let t47 = round_shift(t48a.wrapping_sub(t47a).wrapping_mul(181), 8);
  let t48 = round_shift(t47a.wrapping_add(t48a).wrapping_mul(181), 8);
  let t49a = round_shift(t46.wrapping_add(t49).wrapping_mul(181), 8);
  let t50 = round_shift(t45a.wrapping_add(t50a).wrapping_mul(181), 8);
  let t51a = round_shift(t44.wrapping_add(t51).wrapping_mul(181), 8);
  let t52 = round_shift(t43a.wrapping_add(t52a).wrapping_mul(181), 8);
  let t53a = round_shift(t42.wrapping_add(t53).wrapping_mul(181), 8);
  let t54 = round_shift(t41a.wrapping_add(t54a).wrapping_mul(181), 8);
  let t55a = round_shift(t40.wrapping_add(t55).wrapping_mul(181), 8);

  Ok([
    t32a, t33, t34a, t35, t36a, t37, t38a, t39, t40a, t41, t42a, t43, t44a, t45, t46a, t47, t48,
    t49a, t50, t51a, t52, t53a, t54, t55a, t56, t57a, t58, t59a, t60, t61a, t62, t63a,
  ])
}

fn inv_dct64_1d(
  c: &mut [i32],
  stride: NonZeroU32,
  bounds: CoefficientBounds,
  input: DctInput,
) -> Rav1dResult {
  {
    let even_stride = stride.checked_add(stride.get()).ok_or(Rav1dError::ERANGE)?;

    inv_dct32_1d_internal_c(c, even_stride, bounds, input)?;

    let odd_inputs = rotate_dct64_odd_inputs(c, stride)?;
    let Ok(pairs_mixed) = butterfly_dct64_odd_pairs(odd_inputs, bounds);
    let Ok(pairs_rotated) = rotate_dct64_odd_pairs(pairs_mixed);
    let Ok(quarters_mixed) = butterfly_dct64_odd_quarters(pairs_rotated, bounds);
    let Ok(quarters_rotated) = rotate_dct64_odd_quarters(quarters_mixed);
    let Ok(eighths_mixed) = butterfly_dct64_odd_eighths(quarters_rotated, bounds);
    let Ok(eighths_rotated) = rotate_dct64_odd_eighths(eighths_mixed);
    let Ok(halves_mixed) = butterfly_dct64_odd_halves(eighths_rotated, bounds);
    let Ok(center_rotated) = rotate_dct64_odd_center(halves_mixed);

    merge_dct_halves(c, stride, bounds, center_rotated)?;
  };

  Ok(())
}

impl From<StrideError> for Rav1dError {
  fn from(error: StrideError) -> Self {
    match error {
      StrideError::ExtentOverflow => Self::ERANGE,
      StrideError::EmptyLane | StrideError::BufferTooShort => Self::EINVAL,
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "The shared inverse-transform helper uses a Rust array extent and a native nonzero slice step"
  )
)]
fn native_lane_stride<const N: usize>(stride: NonZeroU32) -> Rav1dResult<NonZeroUsize> {
  let last = u32::try_from(N.saturating_sub(1)).map_err(|_overflow| Rav1dError::ERANGE)?;
  let _last_offset = stride.get().checked_mul(last).ok_or(Rav1dError::ERANGE)?;
  let native = console_core_number_conversion::checked_index(stride.get())
    .map_err(|_overflow| Rav1dError::ERANGE)?;

  NonZeroUsize::new(native).ok_or(Rav1dError::ERANGE)
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "The shared inverse-transform helper takes a native nonzero step at this slice boundary"
  )
)]
fn checked_dct_lane<const N: usize>(
  coefficients: &mut [i32],
  stride: NonZeroU32,
) -> Rav1dResult<&mut [i32]> {
  let step = native_lane_stride::<N>(stride)?;

  strided_lane_mut::<N>(coefficients, step).map_err(Rav1dError::from)
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "The shared inverse-transform helper takes a native nonzero step at this slice boundary"
  )
)]
fn apply_strided<const N: usize>(
  coefficients: &mut [i32],
  stride: NonZeroU32,
  kernel: impl FnOnce([i32; N]) -> Result<[i32; N], Never>,
) -> Rav1dResult {
  let step = native_lane_stride::<N>(stride)?;

  transform_strided(coefficients, step, kernel).map_err(Rav1dError::from)
}

fn adst_output<const N: usize, const REVERSED: bool>(
  mut samples: [i32; N],
) -> Result<[i32; N], Never> {
  match ::core::primitive::bool::then_some(REVERSED, ()) {
    ::core::option::Option::Some(()) => samples.reverse(),
    ::core::option::Option::None => {}
  }

  Ok(samples)
}

pub(super) fn inverse_adst4<const REVERSED: bool>(
  coefficients: &mut [i32],
  stride: NonZeroU32,
  _bounds: CoefficientBounds,
) -> Rav1dResult {
  apply_strided(coefficients, stride, |input| {
    av1::inverse_adst4(input).and_then(adst_output::<4, REVERSED>)
  })
}

pub(super) fn inverse_adst8<const REVERSED: bool>(
  coefficients: &mut [i32],
  stride: NonZeroU32,
  bounds: CoefficientBounds,
) -> Rav1dResult {
  let CoefficientBounds {
    lower: min,
    upper: max,
  } = bounds;
  let bounds_2 = av1::ClipBounds::new(min..=max).map_err(|_| Rav1dError::EINVAL)?;
  apply_strided(coefficients, stride, |input| {
    av1::inverse_adst8(input, bounds_2).and_then(adst_output::<8, REVERSED>)
  })
}

pub(super) fn inverse_adst16<const REVERSED: bool>(
  coefficients: &mut [i32],
  stride: NonZeroU32,
  bounds: CoefficientBounds,
) -> Rav1dResult {
  let CoefficientBounds {
    lower: min,
    upper: max,
  } = bounds;
  let bounds_2 = av1::ClipBounds::new(min..=max).map_err(|_| Rav1dError::EINVAL)?;
  apply_strided(coefficients, stride, |input| {
    av1::inverse_adst16(input, bounds_2).and_then(adst_output::<16, REVERSED>)
  })
}

pub fn rav1d_inv_identity4_1d_c(
  coefficients: &mut [i32],
  stride: NonZeroU32,
  _bounds: CoefficientBounds,
) -> Rav1dResult {
  apply_strided::<4>(coefficients, stride, |lane| {
    Ok(lane.map(|value| {
      value.wrapping_add(value.wrapping_mul(1697).wrapping_add(2048).wrapping_shr(12))
    }))
  })
}

pub fn rav1d_inv_identity8_1d_c(
  coefficients: &mut [i32],
  stride: NonZeroU32,
  _bounds: CoefficientBounds,
) -> Rav1dResult {
  apply_strided::<8>(coefficients, stride, |lane| {
    Ok(lane.map(|value| value.wrapping_mul(2)))
  })
}

pub fn rav1d_inv_identity16_1d_c(
  coefficients: &mut [i32],
  stride: NonZeroU32,
  _bounds: CoefficientBounds,
) -> Rav1dResult {
  apply_strided::<16>(coefficients, stride, |lane| {
    Ok(lane.map(|value| {
      value
        .wrapping_mul(2)
        .wrapping_add(value.wrapping_mul(1697).wrapping_add(1024).wrapping_shr(11))
    }))
  })
}

pub fn rav1d_inv_identity32_1d_c(
  coefficients: &mut [i32],
  stride: NonZeroU32,
  _bounds: CoefficientBounds,
) -> Rav1dResult {
  apply_strided::<32>(coefficients, stride, |lane| {
    Ok(lane.map(|value| value.wrapping_mul(4)))
  })
}

pub fn rav1d_inv_wht4_1d_c(coefficients: &mut [i32], stride: NonZeroU32) -> Rav1dResult {
  apply_strided(coefficients, stride, inverse_walsh_hadamard4)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn dct_and_identity_reject_bad_lanes_before_writing() {
    let transforms: [crate::src::itx::Itx1dFn; 9] = [
      rav1d_inv_dct4_1d_c,
      rav1d_inv_dct8_1d_c,
      rav1d_inv_dct16_1d_c,
      rav1d_inv_dct32_1d_c,
      rav1d_inv_dct64_1d_c,
      rav1d_inv_identity4_1d_c,
      rav1d_inv_identity8_1d_c,
      rav1d_inv_identity16_1d_c,
      rav1d_inv_identity32_1d_c,
    ];

    for transform in transforms {
      let mut coefficients = [1, 2, 3];
      let before = coefficients;

      assert_eq!(
        transform(
          &mut coefficients,
          NonZeroU32::MIN,
          CoefficientBounds {
            lower: -32768,
            upper: 32767
          }
        ),
        Err(Rav1dError::EINVAL),
      );
      assert_eq!(coefficients, before);

      assert_eq!(
        transform(
          &mut coefficients,
          NonZeroU32::MAX,
          CoefficientBounds {
            lower: -32768,
            upper: 32767
          }
        ),
        Err(Rav1dError::ERANGE),
      );
      assert_eq!(coefficients, before);
    }
  }

  #[test]
  fn wht_rejects_a_short_or_overflowing_lane_before_writing()
  -> ::core::result::Result<(), &'static str> {
    let mut coefficients = [1, 2, 3, 4, 5, 6];
    let before = coefficients;
    let stride = match NonZeroU32::new(2) {
      ::core::option::Option::Some(read_value) => read_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(::core::concat!(
          ::core::file!(),
          ":",
          ::core::line!(),
          ": expected Some, found None"
        ));
      }
    };
    assert_eq!(
      rav1d_inv_wht4_1d_c(&mut coefficients, stride),
      Err(Rav1dError::EINVAL)
    );
    assert_eq!(
      rav1d_inv_wht4_1d_c(&mut coefficients, NonZeroU32::MAX),
      Err(Rav1dError::ERANGE),
    );
    assert_eq!(coefficients, before);
    ::core::result::Result::Ok(())
  }

  #[test]
  fn wht_accepts_the_exact_extent_and_leaves_other_lanes_alone()
  -> ::core::result::Result<(), &'static str> {
    let mut contiguous = [4, 8, 12, 16];
    let mut strided = [4, -1, 8, -1, 12, -1, 16];
    let stride = match NonZeroU32::new(2) {
      ::core::option::Option::Some(read_value) => read_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(::core::concat!(
          ::core::file!(),
          ":",
          ::core::line!(),
          ": expected Some, found None"
        ));
      }
    };
    assert_eq!(
      rav1d_inv_wht4_1d_c(&mut contiguous, NonZeroU32::MIN),
      Ok(())
    );
    assert_eq!(rav1d_inv_wht4_1d_c(&mut strided, stride), Ok(()));
    let [first, second, third, fourth] = contiguous;
    assert_eq!(strided, [first, -1, second, -1, third, -1, fourth]);
    ::core::result::Result::Ok(())
  }
}
