use console_core_audio_windows::{
  Extent, Gain, KaiserAlpha, WindowType, generate_window, vorbis_half,
};
use console_core_never::Never;
use console_core_number_conversion::index;

fn assert_complementary(window: &[f32]) -> Result<(), Never> {
  for (&left, &right) in window.iter().zip(window.iter().rev()) {
    assert!(left.is_finite() && (0.0..=1.0).contains(&left));
    assert!((left * left + right * right - 1.0).abs() < 2e-6);
  }

  assert!(window.windows(2).all(|pair| pair.first() <= pair.get(1)));

  Ok(())
}

#[test]
fn vorbis_and_celt_windows_allow_alias_cancellation() {
  for size in [32, 64, 120, 128, 256, 512, 1024, 2048, 4096] {
    let mut window = vec![0.0; size.min(4096)];

    let Ok(()) = vorbis_half(&mut window);
    let Ok(()) = assert_complementary(&window);
  }

  let Ok(()) = vorbis_half(&mut []);
}

#[test]
fn aac_windows_allow_alias_cancellation() {
  for size in [128, 1024] {
    for kind in [
      WindowType::Sine,
      WindowType::KaiserBessel(KaiserAlpha(4.0)),
      WindowType::KaiserBessel(KaiserAlpha(6.0)),
    ] {
      let Ok(length) = index(size);
      let mut window = vec![0.0; length.min(1024)];

      let Ok(()) = generate_window(kind, Gain(1.0), size, Extent::Half, &mut window);
      let Ok(()) = assert_complementary(&window);
    }
  }
}

#[test]
fn full_sine_windows_keep_the_requested_scale_and_buffer_tail()
-> Result<(), Box<dyn std::error::Error>> {
  let mut window = [42.0; 12];
  let Ok(()) = generate_window(WindowType::Sine, Gain(0.5), 8, Extent::Whole, &mut window);
  let (written, tail) = window
    .split_at_checked(8)
    .ok_or("the first eight coefficients")?;

  assert_eq!(tail, &[42.0; 4]);
  assert!(written.iter().all(|value| (0.0..=0.5).contains(value)));

  for (&left, &right) in written.iter().zip(written.iter().rev()) {
    assert!((left - right).abs() < 1e-6);
  }

  Ok(())
}

#[test]
fn a_slice_shorter_than_the_window_is_filled_as_far_as_it_goes() {
  let mut short = [42.0; 3];
  let mut whole = [0.0; 8];

  let Ok(()) = generate_window(
    WindowType::KaiserBessel(KaiserAlpha(4.0)),
    Gain(1.0),
    8,
    Extent::Half,
    &mut short,
  );
  let Ok(()) = generate_window(
    WindowType::KaiserBessel(KaiserAlpha(4.0)),
    Gain(1.0),
    8,
    Extent::Half,
    &mut whole,
  );

  assert_eq!(
    short.as_slice(),
    whole.iter().take(3).copied().collect::<Vec<f32>>()
  );
}
