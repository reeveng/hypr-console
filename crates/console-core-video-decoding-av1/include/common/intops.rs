#![forbid(unsafe_code)]
use crate::include::common::attributes::clz;
use crate::include::common::bitdepth::{AsPrimitive, ToPrimitive};
use core::cmp::Ordering;
use std::ffi::{c_int, c_uint};

#[doc = "Clamp to ordered bounds whose conversion into the input type preserves their value."]
#[inline]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the scalar codec kernels compose this clamp directly in their fixed-point expressions"
  )
)]
pub fn clip<T, U>(value: T, min: U, max: U) -> U
where
  T: Copy + Ord + TryInto<U> + ToPrimitive<U>,
  U: Copy + Ord + Into<T>,
{
  match value.cmp(&<U as Into<T>>::into(min)) {
    Ordering::Less => min,
    Ordering::Equal | Ordering::Greater => match value.cmp(&<U as Into<T>>::into(max)) {
      Ordering::Greater => max,
      Ordering::Less | Ordering::Equal => value.as_(),
    },
  }
}

#[inline]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "this saturation primitive supplies a pixel directly to the existing scalar and SIMD kernels"
  )
)]
pub fn clip_u8<T>(value: T) -> u8
where
  T: Copy + Ord + From<u8> + TryInto<u8> + ToPrimitive<u8>,
{
  clip(value, u8::MIN, u8::MAX)
}

#[inline]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the signed clamp is part of the existing fixed-point kernel expressions"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  expect(
    explicit024_no_swappable_params,
    reason = "the codec uses the established dav1d clamp order of value, lower bound, and upper bound"
  )
)]
pub fn iclip(value: c_int, min: c_int, max: c_int) -> c_int {
  clip(value, min, max)
}

#[inline]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the signed magnitude is used directly in the established fixed-point expressions"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  expect(
    explicit024_no_swappable_params,
    reason = "the codec uses the established dav1d sign helper order of magnitude followed by sign source"
  )
)]
pub fn apply_sign(value: c_int, sign: c_int) -> c_int {
  match sign {
    i32::MIN..=-1 => value.wrapping_neg(),
    0.. => value,
  }
}

#[inline]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the signed magnitude is used directly in the established fixed-point expressions"
  )
)]
pub fn apply_sign64(value: c_int, sign: i64) -> c_int {
  match sign {
    i64::MIN..=-1 => value.wrapping_neg(),
    0.. => value,
  }
}

#[inline]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the signed exponent including the zero sentinel is used directly in codec shift expressions"
  )
)]
pub fn ulog2(value: c_uint) -> c_int {
  31_i32.wrapping_sub(clz(value))
}

#[inline]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the entropy readers use the recentered value directly in the decoded range arithmetic"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  expect(
    explicit024_no_swappable_params,
    reason = "the entropy readers follow dav1d's inverse recenter order of reference followed by value"
  )
)]
pub fn inv_recenter(reference: c_uint, value: c_uint) -> c_uint {
  match (value.cmp(&reference.wrapping_shl(1)), value & 1) {
    (Ordering::Greater, _) => value,
    (_, 0) => value.wrapping_shr(1).wrapping_add(reference),
    _ => reference.wrapping_sub(value.wrapping_add(1).wrapping_shr(1)),
  }
}
