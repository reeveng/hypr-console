use console_core_video_inverse_transforms::{
  StrideError, av1, inverse_walsh_hadamard4, transform_strided,
};
use core::cell::Cell;
use core::num::NonZeroUsize;

#[test]
fn av1_adst_keeps_the_fixed_point_rounding_and_intermediate_clipping()
-> Result<(), av1::EmptyClipRange> {
  let bounds = av1::ClipBounds::new(-128..=127)?;

  let Ok(four) = av1::inverse_adst4([256, -113, 79, -42]);
  let Ok(eight) = av1::inverse_adst8([256, -113, 79, -42, 209, 151, -220, 46], bounds);
  let Ok(sixteen) = av1::inverse_adst16(
    [
      256, -113, 79, -42, 209, 151, -220, 46, 130, 55, -218, -511, 823, 1023, -219, 19,
    ],
    bounds,
  );
  assert_eq!(four, [38, 76, 110, 391]);
  assert_eq!(eight, [54, 98, -118, -29, 151, 4, 127, 128]);
  assert_eq!(
    sixteen,
    [
      126, 128, -49, 49, -48, 59, -165, 1, 180, -16, 122, -117, 117, 106, 91, 1
    ]
  );
  let reversed = core::ops::RangeInclusive::new(127, -128);
  assert_eq!(av1::ClipBounds::new(reversed), Err(av1::EmptyClipRange));

  let mut exhausted = 0..=0;
  assert_eq!(exhausted.next(), Some(0));
  assert_eq!(av1::ClipBounds::new(exhausted), Err(av1::EmptyClipRange));

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the fixture uses the same nonzero slice stride type as the decoder's transform API"
  )
)]
#[test]
fn walsh_hadamard_preserves_the_samples_between_lane_positions() {
  let mut samples = [4, -1, 8, -1, 12, -1, 16, -1];
  let stride = NonZeroUsize::MIN.saturating_add(1);
  assert_eq!(
    transform_strided(&mut samples, stride, inverse_walsh_hadamard4),
    Ok(())
  );
  assert_eq!(samples, [20, -1, -8, -1, 0, -1, -4, -1]);
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the fixture uses the platform's maximum nonzero slice stride to force checked offset overflow"
  )
)]
#[test]
fn an_invalid_lane_leaves_the_buffer_unmodified() {
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  enum TransformCall {
    Skipped,
    Called,
  }

  let before = [1, 2, 3, 4, 5, 6];
  let mut samples = before;
  let call = Cell::new(TransformCall::Skipped);
  let transform = |input| {
    call.set(TransformCall::Called);
    inverse_walsh_hadamard4(input)
  };
  assert_eq!(
    transform_strided(&mut samples, NonZeroUsize::MIN.saturating_add(1), transform),
    Err(StrideError::BufferTooShort),
  );
  assert_eq!(
    transform_strided(&mut samples, NonZeroUsize::MAX, transform),
    Err(StrideError::ExtentOverflow),
  );
  assert_eq!(
    transform_strided::<0>(&mut samples, NonZeroUsize::MIN, Ok),
    Err(StrideError::EmptyLane),
  );
  assert_eq!(call.get(), TransformCall::Skipped);
  assert_eq!(samples, before);
}
