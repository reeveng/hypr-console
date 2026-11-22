//! Scalar inverse transforms over fixed-size coefficient lanes.
//!
//! AV1 and VP9 share the four-point inverse Walsh–Hadamard butterfly. Their
//! decoders choose when to scale coefficients and how to add the residuals
//! to pixels. The AV1 ADST kernels have their own fixed-point rounding and
//! clipping stages in [`av1`].
//! [`strided_lane_mut`] borrows the exact extent needed for a strided lane.
//! [`transform_strided`] checks that extent before writing any samples.
//! [`inverse_walsh_hadamard4`] transforms four prepared coefficients.

#![no_std]
#![forbid(unsafe_code)]

pub mod av1;

use console_core_never::Never;
use core::num::NonZeroUsize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrideError {
  EmptyLane,
  ExtentOverflow,
  BufferTooShort,
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the stride and checked extent are slice offsets. This preserves the nonzero native index API shared with the decoder kernels"
  )
)]
#[inline]
pub fn strided_lane_mut<const N: usize>(
  samples: &mut [i32],
  stride: NonZeroUsize,
) -> Result<&mut [i32], StrideError> {
  let last = N.checked_sub(1).ok_or(StrideError::EmptyLane)?;
  let last_offset = last
    .checked_mul(stride.get())
    .ok_or(StrideError::ExtentOverflow)?;
  samples
    .get_mut(..=last_offset)
    .ok_or(StrideError::BufferTooShort)
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the nonzero stride is the decoder's native slice index consumed by step_by after validating the lane"
  )
)]
#[inline]
pub fn transform_strided<const N: usize>(
  samples: &mut [i32],
  stride: NonZeroUsize,
  transform: impl FnOnce([i32; N]) -> Result<[i32; N], Never>,
) -> Result<(), StrideError> {
  let lane = strided_lane_mut::<N>(samples, stride)?;

  let mut input = [0; N];

  for (value, &sample) in input.iter_mut().zip(lane.iter().step_by(stride.get())) {
    *value = sample;
  }

  let Ok(output) = transform(input);

  for (sample, value) in lane.iter_mut().step_by(stride.get()).zip(output) {
    *sample = value;
  }

  Ok(())
}

#[inline]
#[must_use]
pub fn inverse_walsh_hadamard4(input: [i32; 4]) -> Result<[i32; 4], Never> {
  let [first, second, third, fourth] = input;
  let sum = first.wrapping_add(second);
  let difference = third.wrapping_sub(fourth);
  let half = sum.wrapping_sub(difference).wrapping_shr(1);
  let middle_left = half.wrapping_sub(fourth);
  let middle_right = half.wrapping_sub(second);

  Ok([
    sum.wrapping_sub(middle_left),
    middle_left,
    middle_right,
    difference.wrapping_add(middle_right),
  ])
}
