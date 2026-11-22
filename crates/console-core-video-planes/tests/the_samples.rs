//! A plane gives back the samples it was made from, at the width it holds them.

use console_core_video_planes::{Emptiness, Plane};

#[test]
fn rawvideo_bytes_preserve_sample_width_and_little_endian_order() {
  let byte_samples = Plane::U8(vec![0, 128, 255]);
  let deep_samples = Plane::U16(vec![0, 128, 255, 1023, 4095]);

  assert_eq!(byte_samples.to_le_bytes(), Ok(vec![0, 128, 255]));
  assert_eq!(
    deep_samples.to_le_bytes(),
    Ok(vec![0, 0, 128, 0, 255, 0, 255, 3, 255, 15])
  );
  assert_eq!(deep_samples.sample(3), Ok(Some(1023)));
  assert_eq!(byte_samples.sample(2), Ok(Some(255)));
}

#[test]
fn a_place_past_the_end_is_no_sample() {
  let byte_samples = Plane::U8(vec![7, 8]);
  let deep_samples = Plane::U16(vec![900]);

  assert_eq!(byte_samples.sample(2), Ok(None));
  assert_eq!(deep_samples.sample(1), Ok(None));
  assert_eq!(byte_samples.sample_count(), Ok(2));
  assert_eq!(deep_samples.sample_count(), Ok(1));
}

#[test]
fn empty_planes_keep_their_storage_type() {
  let bytes = Plane::U8(Vec::new());
  let deep = Plane::U16(Vec::new());
  let non_empty = Plane::U8(vec![1]);

  assert_eq!(bytes.emptiness(), Ok(Emptiness::Empty));
  assert_eq!(deep.emptiness(), Ok(Emptiness::Empty));
  assert_eq!(non_empty.emptiness(), Ok(Emptiness::NonEmpty));
  assert_eq!(bytes.as_u8(), Ok(Some([].as_slice())));
  assert_eq!(bytes.as_u16(), Ok(None));
  assert_eq!(deep.as_u8(), Ok(None));
  assert_eq!(deep.as_u16(), Ok(Some([].as_slice())));
}
