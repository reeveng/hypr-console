//! AV1 fixed-point asymmetric discrete sine transforms for 4, 8, and 16 samples.
//!
//! The AV1 kernels preserve the prescribed output order and clipping stages.
//! Arithmetic wraps explicitly so even full-width coefficients have a defined
//! result. The flipped transform reverses the forward result at the call site.
//!
//! The arithmetic kernels come from rav1d-safe, a fork of rav1d and dav1d.
//! The 8- and 16-point transforms share the clipping butterflies and quarter
//! rotation. The input rotations and final output order are specific to each size.

use console_core_never::Never;
use console_core_video_interpolation::round_shift;
use core::ops::RangeInclusive;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipBounds {
  min: i32,
  max: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmptyClipRange;

impl ClipBounds {
  pub fn new(limits: RangeInclusive<i32>) -> Result<Self, EmptyClipRange> {
    match limits.clone().next() {
      Some(min) => Ok(Self { min, max: *limits.end() }),
      None => Err(EmptyClipRange),
    }
  }
}

#[must_use]
pub fn inverse_adst4(input: [i32; 4]) -> Result<[i32; 4], Never> {
  let [in0, in1, in2, in3] = input;
  let out0 = round_shift(
    1321_i32
      .wrapping_mul(in0)
      .wrapping_add(3803_i32.wrapping_sub(4096).wrapping_mul(in2))
      .wrapping_add(2482_i32.wrapping_sub(4096).wrapping_mul(in3))
      .wrapping_add(3344_i32.wrapping_sub(4096).wrapping_mul(in1)),
    12,
  )
  .wrapping_add(in2)
  .wrapping_add(in3)
  .wrapping_add(in1);
  let out1 = round_shift(
    2482_i32
      .wrapping_sub(4096)
      .wrapping_mul(in0)
      .wrapping_sub(1321_i32.wrapping_mul(in2))
      .wrapping_sub(3803_i32.wrapping_sub(4096).wrapping_mul(in3))
      .wrapping_add(3344_i32.wrapping_sub(4096).wrapping_mul(in1)),
    12,
  )
  .wrapping_add(in0)
  .wrapping_sub(in3)
  .wrapping_add(in1);
  let out2 = round_shift(
    209_i32.wrapping_mul(in0.wrapping_sub(in2).wrapping_add(in3)),
    8,
  );
  let out3 = round_shift(
    3803_i32
      .wrapping_sub(4096)
      .wrapping_mul(in0)
      .wrapping_add(2482_i32.wrapping_sub(4096).wrapping_mul(in2))
      .wrapping_sub(1321_i32.wrapping_mul(in3))
      .wrapping_sub(3344_i32.wrapping_sub(4096).wrapping_mul(in1)),
    12,
  )
  .wrapping_add(in0)
  .wrapping_add(in2)
  .wrapping_sub(in1);

  Ok([out0, out1, out2, out3])
}

#[must_use]
pub fn inverse_adst8(input: [i32; 8], bounds: ClipBounds) -> Result<[i32; 8], Never> {
  let Ok(rotated) = rotate_adst8(input);
  let Ok([[t0, t1, t2, t3], difference]) = combine_quarters(rotated, bounds);
  let Ok([t4a, t5a, t6a, t7a]) = rotate_quarter(difference);
  let clip = |value: i32| value.max(bounds.min).min(bounds.max);

  let out0 = clip(t0.wrapping_add(t2));
  let out7 = (clip(t1.wrapping_add(t3))).wrapping_neg();
  let left = clip(t0.wrapping_sub(t2));
  let right = clip(t1.wrapping_sub(t3));
  let out1 = (clip(t4a.wrapping_add(t6a))).wrapping_neg();
  let out6 = clip(t5a.wrapping_add(t7a));
  let t6 = clip(t4a.wrapping_sub(t6a));
  let t7 = clip(t5a.wrapping_sub(t7a));

  let out3 = (round_shift(left.wrapping_add(right).wrapping_mul(181), 8)).wrapping_neg();
  let out4 = round_shift(left.wrapping_sub(right).wrapping_mul(181), 8);
  let out2 = round_shift(t6.wrapping_add(t7).wrapping_mul(181), 8);
  let out5 = (round_shift(t6.wrapping_sub(t7).wrapping_mul(181), 8)).wrapping_neg();

  Ok([out0, out1, out2, out3, out4, out5, out6, out7])
}

#[must_use]
pub fn inverse_adst16(
  [
    in0,
    in1,
    in2,
    in3,
    in4,
    in5,
    in6,
    in7,
    in8,
    in9,
    in10,
    in11,
    in12,
    in13,
    in14,
    in15,
  ]: [i32; 16],
  bounds: ClipBounds,
) -> Result<[i32; 16], Never> {
  let Ok(first) = rotate_adst16_first([in0, in2, in4, in6, in9, in11, in13, in15]);
  let Ok(last) = rotate_adst16_last([in1, in3, in5, in7, in8, in10, in12, in14]);
  let Ok([sums, differences]) = butterflies(first, last, bounds);
  let Ok(rotated) = rotate_adst16_differences(differences);
  let Ok([[t0, t1, t2, t3], lower]) = combine_quarters(sums, bounds);
  let Ok([[t8a, t9a, t10a, t11a], upper]) = combine_quarters(rotated, bounds);
  let Ok([t4a, t5a, t6a, t7a]) = rotate_quarter(lower);
  let Ok([t12, t13, t14, t15]) = rotate_quarter(upper);
  let clip = |value: i32| value.max(bounds.min).min(bounds.max);

  let out0 = clip(t0.wrapping_add(t2));
  let out15 = (clip(t1.wrapping_add(t3))).wrapping_neg();
  let t2a = clip(t0.wrapping_sub(t2));
  let t3a = clip(t1.wrapping_sub(t3));
  let out3 = (clip(t4a.wrapping_add(t6a))).wrapping_neg();
  let out12 = clip(t5a.wrapping_add(t7a));
  let t6 = clip(t4a.wrapping_sub(t6a));
  let t7 = clip(t5a.wrapping_sub(t7a));
  let out1 = (clip(t8a.wrapping_add(t10a))).wrapping_neg();
  let out14 = clip(t9a.wrapping_add(t11a));
  let t10 = clip(t8a.wrapping_sub(t10a));
  let t11 = clip(t9a.wrapping_sub(t11a));
  let out2 = clip(t12.wrapping_add(t14));
  let out13 = (clip(t13.wrapping_add(t15))).wrapping_neg();
  let t14a = clip(t12.wrapping_sub(t14));
  let t15a = clip(t13.wrapping_sub(t15));

  let out7 = (round_shift(t2a.wrapping_add(t3a).wrapping_mul(181), 8)).wrapping_neg();
  let out8 = round_shift(t2a.wrapping_sub(t3a).wrapping_mul(181), 8);
  let out4 = round_shift(t6.wrapping_add(t7).wrapping_mul(181), 8);
  let out11 = (round_shift(t6.wrapping_sub(t7).wrapping_mul(181), 8)).wrapping_neg();
  let out6 = round_shift(t10.wrapping_add(t11).wrapping_mul(181), 8);
  let out9 = (round_shift(t10.wrapping_sub(t11).wrapping_mul(181), 8)).wrapping_neg();
  let out5 = (round_shift(t14a.wrapping_add(t15a).wrapping_mul(181), 8)).wrapping_neg();
  let out10 = round_shift(t14a.wrapping_sub(t15a).wrapping_mul(181), 8);

  Ok([
    out0, out1, out2, out3, out4, out5, out6, out7, out8, out9, out10, out11, out12, out13,
    out14, out15,
  ])
}

#[inline]
fn butterflies<const N: usize>(
  left: [i32; N],
  right: [i32; N],
  bounds: ClipBounds,
) -> Result<[[i32; N]; 2], Never> {
  let mut combined = [[0; N]; 2];
  let [sums, differences] = &mut combined;
  let clip = |value: i32| value.max(bounds.min).min(bounds.max);

  for ((sum, difference), (first, second)) in sums
    .iter_mut()
    .zip(differences)
    .zip(left.into_iter().zip(right))
  {
    *sum = clip(first.wrapping_add(second));
    *difference = clip(first.wrapping_sub(second));
  }

  Ok(combined)
}

#[inline]
fn combine_quarters(
  [a, b, c, d, e, f, g, h]: [i32; 8],
  bounds: ClipBounds,
) -> Result<[[i32; 4]; 2], Never> {
  butterflies([a, b, c, d], [e, f, g, h], bounds)
}

#[inline]
fn rotate_adst8([in0, in1, in2, in3, in4, in5, in6, in7]: [i32; 8]) -> Result<[i32; 8], Never> {
  let t0a = round_shift(
    4076_i32
      .wrapping_sub(4096)
      .wrapping_mul(in7)
      .wrapping_add(401_i32.wrapping_mul(in0)),
    12,
  )
  .wrapping_add(in7);
  let t1a = round_shift(
    401_i32
      .wrapping_mul(in7)
      .wrapping_sub(4076_i32.wrapping_sub(4096).wrapping_mul(in0)),
    12,
  )
  .wrapping_sub(in0);
  let t2a = round_shift(
    3612_i32
      .wrapping_sub(4096)
      .wrapping_mul(in5)
      .wrapping_add(1931_i32.wrapping_mul(in2)),
    12,
  )
  .wrapping_add(in5);
  let t3a = round_shift(
    1931_i32
      .wrapping_mul(in5)
      .wrapping_sub(3612_i32.wrapping_sub(4096).wrapping_mul(in2)),
    12,
  )
  .wrapping_sub(in2);
  let t4a = round_shift(
    1299_i32
      .wrapping_mul(in3)
      .wrapping_add(1583_i32.wrapping_mul(in4)),
    11,
  );
  let t5a = round_shift(
    1583_i32
      .wrapping_mul(in3)
      .wrapping_sub(1299_i32.wrapping_mul(in4)),
    11,
  );
  let t6a = round_shift(
    1189_i32
      .wrapping_mul(in1)
      .wrapping_add(3920_i32.wrapping_sub(4096).wrapping_mul(in6)),
    12,
  )
  .wrapping_add(in6);
  let t7a = round_shift(
    3920_i32
      .wrapping_sub(4096)
      .wrapping_mul(in1)
      .wrapping_sub(1189_i32.wrapping_mul(in6)),
    12,
  )
  .wrapping_add(in1);

  Ok([t0a, t1a, t2a, t3a, t4a, t5a, t6a, t7a])
}

#[inline]
fn rotate_adst16_first(
  [in0, in2, in4, in6, in9, in11, in13, in15]: [i32; 8],
) -> Result<[i32; 8], Never> {
  let t0 = round_shift(
    in15
      .wrapping_mul(4091_i32.wrapping_sub(4096))
      .wrapping_add(in0.wrapping_mul(201)),
    12,
  )
  .wrapping_add(in15);
  let t1 = round_shift(
    in15
      .wrapping_mul(201)
      .wrapping_sub(in0.wrapping_mul(4091_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_sub(in0);
  let t2 = round_shift(
    in13
      .wrapping_mul(3973_i32.wrapping_sub(4096))
      .wrapping_add(in2.wrapping_mul(995)),
    12,
  )
  .wrapping_add(in13);
  let t3 = round_shift(
    in13
      .wrapping_mul(995)
      .wrapping_sub(in2.wrapping_mul(3973_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_sub(in2);
  let t4 = round_shift(
    in11
      .wrapping_mul(3703_i32.wrapping_sub(4096))
      .wrapping_add(in4.wrapping_mul(1751)),
    12,
  )
  .wrapping_add(in11);
  let t5 = round_shift(
    in11
      .wrapping_mul(1751)
      .wrapping_sub(in4.wrapping_mul(3703_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_sub(in4);
  let t6 = round_shift(
    in9.wrapping_mul(1645).wrapping_add(in6.wrapping_mul(1220)),
    11,
  );
  let t7 = round_shift(
    in9.wrapping_mul(1220).wrapping_sub(in6.wrapping_mul(1645)),
    11,
  );

  Ok([t0, t1, t2, t3, t4, t5, t6, t7])
}

#[inline]
fn rotate_adst16_last(
  [in1, in3, in5, in7, in8, in10, in12, in14]: [i32; 8],
) -> Result<[i32; 8], Never> {
  let t8 = round_shift(
    in7
      .wrapping_mul(2751)
      .wrapping_add(in8.wrapping_mul(3035_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_add(in8);
  let t9 = round_shift(
    in7
      .wrapping_mul(3035_i32.wrapping_sub(4096))
      .wrapping_sub(in8.wrapping_mul(2751)),
    12,
  )
  .wrapping_add(in7);
  let t10 = round_shift(
    in5
      .wrapping_mul(2106)
      .wrapping_add(in10.wrapping_mul(3513_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_add(in10);
  let t11 = round_shift(
    in5
      .wrapping_mul(3513_i32.wrapping_sub(4096))
      .wrapping_sub(in10.wrapping_mul(2106)),
    12,
  )
  .wrapping_add(in5);
  let t12 = round_shift(
    in3
      .wrapping_mul(1380)
      .wrapping_add(in12.wrapping_mul(3857_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_add(in12);
  let t13 = round_shift(
    in3
      .wrapping_mul(3857_i32.wrapping_sub(4096))
      .wrapping_sub(in12.wrapping_mul(1380)),
    12,
  )
  .wrapping_add(in3);
  let t14 = round_shift(
    in1
      .wrapping_mul(601)
      .wrapping_add(in14.wrapping_mul(4052_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_add(in14);
  let t15 = round_shift(
    in1
      .wrapping_mul(4052_i32.wrapping_sub(4096))
      .wrapping_sub(in14.wrapping_mul(601)),
    12,
  )
  .wrapping_add(in1);

  Ok([t8, t9, t10, t11, t12, t13, t14, t15])
}

#[inline]
fn rotate_adst16_differences(
  [t8a, t9a, t10a, t11a, t12a, t13a, t14a, t15a]: [i32; 8],
) -> Result<[i32; 8], Never> {
  let t8 = round_shift(
    t8a
      .wrapping_mul(4017_i32.wrapping_sub(4096))
      .wrapping_add(t9a.wrapping_mul(799)),
    12,
  )
  .wrapping_add(t8a);
  let t9 = round_shift(
    t8a
      .wrapping_mul(799)
      .wrapping_sub(t9a.wrapping_mul(4017_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_sub(t9a);
  let t10 = round_shift(
    t10a
      .wrapping_mul(2276)
      .wrapping_add(t11a.wrapping_mul(3406_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_add(t11a);
  let t11 = round_shift(
    t10a
      .wrapping_mul(3406_i32.wrapping_sub(4096))
      .wrapping_sub(t11a.wrapping_mul(2276)),
    12,
  )
  .wrapping_add(t10a);
  let t12 = round_shift(
    t13a
      .wrapping_mul(4017_i32.wrapping_sub(4096))
      .wrapping_sub(t12a.wrapping_mul(799)),
    12,
  )
  .wrapping_add(t13a);
  let t13 = round_shift(
    t13a
      .wrapping_mul(799)
      .wrapping_add(t12a.wrapping_mul(4017_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_add(t12a);
  let t14 = round_shift(
    t15a
      .wrapping_mul(2276)
      .wrapping_sub(t14a.wrapping_mul(3406_i32.wrapping_sub(4096))),
    12,
  )
  .wrapping_sub(t14a);
  let t15 = round_shift(
    t15a
      .wrapping_mul(3406_i32.wrapping_sub(4096))
      .wrapping_add(t14a.wrapping_mul(2276)),
    12,
  )
  .wrapping_add(t15a);

  Ok([t8, t9, t10, t11, t12, t13, t14, t15])
}

#[inline]
fn rotate_quarter([t4, t5, t6, t7]: [i32; 4]) -> Result<[i32; 4], Never> {
  let t4a = round_shift(
    3784_i32
      .wrapping_sub(4096)
      .wrapping_mul(t4)
      .wrapping_add(1567_i32.wrapping_mul(t5)),
    12,
  )
  .wrapping_add(t4);
  let t5a = round_shift(
    1567_i32
      .wrapping_mul(t4)
      .wrapping_sub(3784_i32.wrapping_sub(4096).wrapping_mul(t5)),
    12,
  )
  .wrapping_sub(t5);
  let t6a = round_shift(
    3784_i32
      .wrapping_sub(4096)
      .wrapping_mul(t7)
      .wrapping_sub(1567_i32.wrapping_mul(t6)),
    12,
  )
  .wrapping_add(t7);
  let t7a = round_shift(
    1567_i32
      .wrapping_mul(t7)
      .wrapping_add(3784_i32.wrapping_sub(4096).wrapping_mul(t6)),
    12,
  )
  .wrapping_add(t6);

  Ok([t4a, t5a, t6a, t7a])
}
