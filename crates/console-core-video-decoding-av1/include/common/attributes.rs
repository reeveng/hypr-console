#![forbid(unsafe_code)]
use std::ffi::c_int;
use std::ffi::c_uint;

#[inline]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "this C compatibility intrinsic returns the signed bit count expected at existing call sites"
  )
)]
pub fn ctz(mask: c_uint) -> c_int {
  ::core::primitive::u32::cast_signed(mask.trailing_zeros())
}

#[inline]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "this C compatibility intrinsic returns the signed bit count expected at existing call sites"
  )
)]
pub fn clz(mask: c_uint) -> c_int {
  ::core::primitive::u32::cast_signed(mask.leading_zeros())
}
