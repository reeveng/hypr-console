use console_core_audio_decoding_interfaces::audio::{
  AsAudioBufferRef, AudioBuffer, Channels, SampleBuffer, Signal, SignalSpec,
};
use console_core_audio_decoding_interfaces::errors::Error;
use console_core_audio_samples::i24;

#[test]
fn extracted_sample_types_work_in_decoder_buffers() -> Result<(), Error> {
  let Ok(value_1) = SignalSpec::new(48_000, Channels::FRONT_LEFT | Channels::FRONT_RIGHT);

  let spec = value_1;
  let mut decoded = AudioBuffer::<i24>::new(2, spec)?;
  decoded.render_reserved(Some(2))?;
  let Ok(value_2) = decoded.chan_mut(0);

  assert_eq!(value_2.len(), 2);

  for (slot, sample) in value_2.iter_mut().zip([i24::from(-8_388_608), i24::from(0)]) {
    *slot = sample;
  }

  let Ok(value_3) = decoded.chan_mut(1);

  assert_eq!(value_3.len(), 2);

  for (slot, sample) in value_3.iter_mut().zip([i24::from(4_194_304), i24::from(8_388_607)]) {
    *slot = sample;
  }

  let mut playback = SampleBuffer::<f32>::new(2, spec)?;
  let Ok(value_4) = decoded.as_audio_buffer_ref();

  playback.copy_interleaved_ref(value_4)?;
  let Ok(value_5) = playback.samples();

  assert_eq!(value_5, &[-1.0, 0.5, 0.0, 8_388_607.0 / 8_388_608.0]);
  Ok(())
}
