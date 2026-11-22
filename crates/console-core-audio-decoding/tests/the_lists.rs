use std::io::Cursor;

use console_core_audio_decoding::core::codecs::{
  CODEC_TYPE_AAC, CODEC_TYPE_ALAC, CODEC_TYPE_FLAC, CODEC_TYPE_MP1, CODEC_TYPE_MP2,
  CODEC_TYPE_MP3, CODEC_TYPE_PCM_S16LE, CODEC_TYPE_VORBIS, CodecType, DecoderOptions,
};
use console_core_audio_decoding::core::formats::FormatOptions;
use console_core_audio_decoding::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use console_core_audio_decoding::core::meta::MetadataOptions;
use console_core_audio_decoding::core::probe::Hint;
use console_core_audio_decoding::{codec_registry, format_probe};
use console_core_never::Never;

type Failure = Box<dyn std::error::Error>;

const REGISTERED: [CodecType; 6] = [
  CODEC_TYPE_AAC,
  CODEC_TYPE_ALAC,
  CODEC_TYPE_FLAC,
  CODEC_TYPE_MP3,
  CODEC_TYPE_PCM_S16LE,
  CODEC_TYPE_VORBIS,
];

const NEVER_ASKED_FOR: [CodecType; 2] = [CODEC_TYPE_MP1, CODEC_TYPE_MP2];

const SAMPLE_RATE: u32 = 8000;

const SAMPLES: [i16; 4] = [0, 1000, -1000, 0];

fn wave() -> Result<Vec<u8>, Failure> {
  let data: Vec<u8> = SAMPLES
    .iter()
    .flat_map(|sample| sample.to_le_bytes())
    .collect();
  let data_length = u32::try_from(data.len())?;
  let mut file = b"RIFF".to_vec();

  file.extend(data_length.saturating_add(36).to_le_bytes());
  file.extend(b"WAVEfmt ");
  file.extend(16u32.to_le_bytes());
  file.extend(1u16.to_le_bytes());
  file.extend(1u16.to_le_bytes());
  file.extend(SAMPLE_RATE.to_le_bytes());
  file.extend(SAMPLE_RATE.saturating_mul(2).to_le_bytes());
  file.extend(2u16.to_le_bytes());
  file.extend(16u16.to_le_bytes());
  file.extend(b"data");
  file.extend(data_length.to_le_bytes());
  file.extend(data);

  Ok(file)
}

#[test]
fn every_codec_the_player_opens_is_registered() -> Result<(), Never> {
  let Ok(registry) = codec_registry();
  let held: Vec<CodecType> = REGISTERED
    .into_iter()
    .filter(|codec| {
      let Ok(descriptor) = registry.get_codec(*codec);

      descriptor.is_some()
    })
    .collect();

  assert_eq!(held, REGISTERED);

  Ok(())
}

#[test]
fn the_mpeg_layers_nobody_asked_for_are_not_registered() -> Result<(), Never> {
  let Ok(registry) = codec_registry();
  let held: Vec<CodecType> = NEVER_ASKED_FOR
    .into_iter()
    .filter(|codec| {
      let Ok(descriptor) = registry.get_codec(*codec);

      descriptor.is_some()
    })
    .collect();

  assert_eq!(held, []);

  Ok(())
}

#[test]
fn a_wave_file_is_recognized_and_its_track_gets_a_decoder() -> Result<(), Failure> {
  let Ok(probe) = format_probe();
  let Ok(registry) = codec_registry();
  let file = wave()?;
  let Ok(source) = MediaSourceStream::new(
    Box::new(Cursor::new(file)),
    MediaSourceStreamOptions::default(),
  );
  let Ok(hint) = Hint::new();
  let probed = probe.format(
    &hint,
    source,
    &FormatOptions::default(),
    &MetadataOptions::default(),
  )?;
  let Ok(tracks) = probed.format.tracks();
  let codecs: Vec<CodecType> = tracks
    .iter()
    .map(|track| track.codec_params.codec)
    .collect();

  assert_eq!(codecs, [CODEC_TYPE_PCM_S16LE]);

  let mut decoders: Vec<CodecType> = Vec::new();

  for track in tracks.iter() {
    let decoder = registry.make(&track.codec_params, &DecoderOptions::default())?;
    let Ok(params) = decoder.codec_params();

    decoders.push(params.codec);
  }

  assert_eq!(decoders, [CODEC_TYPE_PCM_S16LE]);

  Ok(())
}
