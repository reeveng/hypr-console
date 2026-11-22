use console_core_audio_decoding_interfaces::audio::{AudioBuffer, Channels, Signal, SignalSpec};
use console_core_audio_decoding_interfaces::errors::Error;
use console_core_never::Never;

fn stereo() -> Result<SignalSpec, Never> {
  let Ok(value_1) = SignalSpec::new(44_100, Channels::FRONT_LEFT | Channels::FRONT_RIGHT);

  Ok(value_1)
}

fn filled() -> Result<AudioBuffer<i16>, Error> {
  let Ok(spec) = stereo();
  let mut buffer = AudioBuffer::<i16>::new(4, spec)?;

  buffer.render_reserved(Some(3))?;
  let Ok(value_2) = buffer.chan_mut(0);

  assert_eq!(value_2.len(), 3);

  for (slot, sample) in value_2.iter_mut().zip([1, 2, 3]) {
    *slot = sample;
  }

  let Ok(value_3) = buffer.chan_mut(1);

  assert_eq!(value_3.len(), 3);

  for (slot, sample) in value_3.iter_mut().zip([10, 20, 30]) {
    *slot = sample;
  }

  Ok(buffer)
}

#[test]
fn a_plane_is_what_was_rendered_and_a_missing_plane_is_empty() -> Result<(), Error> {
  let buffer = filled()?;

  let Ok(value_4) = buffer.frames();

  assert_eq!(value_4, 3);
  let Ok(value_5) = buffer.capacity();

  assert_eq!(value_5, 4);
  let Ok(value_6) = buffer.chan(0);

  assert_eq!(value_6, &[1, 2, 3]);
  let Ok(value_7) = buffer.chan(1);

  assert_eq!(value_7, &[10, 20, 30]);
  let Ok(value_8) = buffer.chan(2);

  assert!(value_8.is_empty());

  Ok(())
}

#[test]
fn two_planes_are_lent_in_the_order_they_were_asked() -> Result<(), Error> {
  let mut buffer = filled()?;
  let Ok(value_9) = buffer.chan_pair_mut(1, 0);

  let (second, first) = value_9;

  assert_eq!(second, &[10, 20, 30]);
  assert_eq!(first, &[1, 2, 3]);

  let Ok(value_10) = buffer.chan_pair_mut(1, 1);

  let (same, other) = value_10;

  assert!(same.is_empty() && other.is_empty());

  Ok(())
}

#[test]
fn trimming_and_shifting_drop_frames_from_either_end() -> Result<(), Error> {
  let mut buffer = filled()?;

  let Ok(()) = buffer.trim(1, 1);

  let Ok(value_11) = buffer.chan(0);

  assert_eq!(value_11, &[2]);
  let Ok(value_12) = buffer.chan(1);

  assert_eq!(value_12, &[20]);

  let Ok(()) = buffer.shift(5);

  let Ok(value_13) = buffer.frames();

  assert_eq!(value_13, 0);

  Ok(())
}

#[test]
fn rendering_past_the_capacity_is_a_limit_and_not_a_panic() -> Result<(), Error> {
  let mut buffer = filled()?;

  assert!(matches!(
    buffer.render_reserved(Some(2)),
    Err(Error::LimitError(_))
  ));
  assert!(matches!(buffer.render_silence(Some(1)), Ok(())));
  let Ok(value_14) = buffer.chan(0);

  assert_eq!(value_14, &[1, 2, 3, 0]);

  Ok(())
}

#[test]
fn fill_hands_each_frame_index_to_the_writer() -> Result<(), Error> {
  let Ok(spec) = stereo();
  let mut buffer = AudioBuffer::<i16>::new(3, spec)?;

  buffer.fill(|planes, frame| {
    let Ok(value_15) = planes.planes();

    value_15
      .iter_mut()
      .for_each(|plane| plane.iter_mut().for_each(|sample| *sample = 7));

    match ::core::primitive::bool::then_some(frame < 3, ()) {
      ::core::option::Option::Some(()) => Ok(()),
      ::core::option::Option::None => Err(Error::DecodeError("past the end")),
    }
  })?;

  let Ok(value_16) = buffer.chan(0);

  assert_eq!(value_16, &[7, 7, 7]);

  Ok(())
}

#[test]
fn converting_keeps_the_frames_and_scales_the_samples() -> Result<(), Error> {
  let buffer = filled()?;
  let Ok(spec) = stereo();
  let mut wide = AudioBuffer::<i32>::new(4, spec)?;

  buffer.convert(&mut wide)?;

  let Ok(value_17) = wide.frames();

  assert_eq!(value_17, 3);
  let Ok(value_18) = wide.chan(0);

  assert_eq!(value_18, &[65_536, 131_072, 196_608]);

  Ok(())
}
