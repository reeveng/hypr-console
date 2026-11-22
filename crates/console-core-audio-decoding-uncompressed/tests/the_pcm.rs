//! Two frames of stereo, one codec at a time, decoded and read back plane by plane.
//!
//! The expected samples were read off the decoder as it stood when it came
//! out of a fork, so a rewrite that changes what a byte means is the one that
//! goes red. The two logarithmic codecs are pressed with every byte there is.

use console_core_audio_decoding_interfaces::audio::{AudioBuffer, AudioBufferRef, Channels};
use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_NULL, CODEC_TYPE_PCM_ALAW, CODEC_TYPE_PCM_F32BE, CODEC_TYPE_PCM_F32LE,
  CODEC_TYPE_PCM_F64BE, CODEC_TYPE_PCM_F64LE, CODEC_TYPE_PCM_MULAW, CODEC_TYPE_PCM_S8,
  CODEC_TYPE_PCM_S16BE, CODEC_TYPE_PCM_S16LE, CODEC_TYPE_PCM_S24BE, CODEC_TYPE_PCM_S24LE,
  CODEC_TYPE_PCM_S32BE, CODEC_TYPE_PCM_S32LE, CODEC_TYPE_PCM_U8, CODEC_TYPE_PCM_U16BE,
  CODEC_TYPE_PCM_U16LE, CODEC_TYPE_PCM_U24BE, CODEC_TYPE_PCM_U24LE, CODEC_TYPE_PCM_U32BE,
  CODEC_TYPE_PCM_U32LE, CodecParameters, CodecType, Decoder, DecoderOptions,
};
use console_core_audio_decoding_interfaces::formats::Packet;
use console_core_audio_decoding_uncompressed::PcmDecoder;
use console_core_audio_samples::sample::Sample;
use console_core_never::Never;

type Failure = Box<dyn std::error::Error>;
type Infallible<T> = std::result::Result<T, Never>;

fn shown<S: Sample + std::fmt::Debug>(buffer: &AudioBuffer<S>) -> Infallible<String> {
  let Ok(value_1) = buffer.planes();

  let planes = value_1;
  let Ok(value_2) = planes.planes();

  let rendered: Vec<String> = value_2.iter().map(|plane| format!("{plane:?}")).collect();

  Ok(rendered.join(" "))
}

fn shown_ref(decoded: AudioBufferRef<'_>) -> Infallible<String> {
  match decoded {
    AudioBufferRef::U8(buffer) => shown(&buffer),
    AudioBufferRef::U16(buffer) => shown(&buffer),
    AudioBufferRef::U24(buffer) => shown(&buffer),
    AudioBufferRef::U32(buffer) => shown(&buffer),
    AudioBufferRef::S8(buffer) => shown(&buffer),
    AudioBufferRef::S16(buffer) => shown(&buffer),
    AudioBufferRef::S24(buffer) => shown(&buffer),
    AudioBufferRef::S32(buffer) => shown(&buffer),
    AudioBufferRef::F32(buffer) => shown(&buffer),
    AudioBufferRef::F64(buffer) => shown(&buffer),
  }
}

fn parameters(codec: CodecType, coded: Option<u32>) -> Infallible<CodecParameters> {
  let Ok(value_3) = CodecParameters::new();

  let mut parameters = value_3;

  let Ok(_configured_for_codec) = parameters.for_codec(codec);
  let Ok(_configured_with_sample_rate) = parameters.with_sample_rate(44100);
  let Ok(_configured_with_channels) =
    parameters.with_channels(Channels::FRONT_LEFT | Channels::FRONT_RIGHT);
  let Ok(_configured_with_max_frames_per_packet) = parameters.with_max_frames_per_packet(2);

  match coded {
    Some(bits) => {
      let Ok(_configured_with_bits_per_coded_sample) =
        parameters.with_bits_per_coded_sample(bits);
    }
    None => {}
  }

  Ok(parameters)
}

fn decoded(codec: CodecType, coded: Option<u32>, bytes: &[u8]) -> Result<String, Failure> {
  let Ok(parameters) = parameters(codec, coded);
  let mut decoder = PcmDecoder::try_new(&parameters, &DecoderOptions::default())?;
  let Ok(value_4) = Packet::new_from_slice(0, 0, 2, bytes);

  let packet = value_4;
  let audio = decoder.decode(&packet)?;
  let Ok(rendered) = shown_ref(audio);

  Ok(rendered)
}

fn refused(parameters: &CodecParameters) -> Infallible<String> {
  Ok(
    match PcmDecoder::try_new(parameters, &DecoderOptions::default()) {
      Ok(_) => String::from("accepted"),
      Err(fault) => fault.to_string(),
    },
  )
}

struct Case {
  name: &'static str,
  codec: CodecType,
  coded: Option<u32>,
  bytes: &'static [u8],
  planes: &'static str,
}

const CASES: &[Case] = &[
  Case {
    name: "s32le",
    codec: CODEC_TYPE_PCM_S32LE,
    coded: Some(32),
    bytes: &[
      1, 2, 3, 0x84, 0xff, 0xff, 0xff, 0x7f, 0, 0, 0, 1, 0x78, 0x56, 0x34, 0x12,
    ],
    planes: "[-2080177663, 16777216] [2147483647, 305419896]",
  },
  Case {
    name: "s32be",
    codec: CODEC_TYPE_PCM_S32BE,
    coded: Some(32),
    bytes: &[
      0x84, 3, 2, 1, 0x7f, 0xff, 0xff, 0xff, 1, 0, 0, 0, 0x12, 0x34, 0x56, 0x78,
    ],
    planes: "[-2080177663, 16777216] [2147483647, 305419896]",
  },
  Case {
    name: "s32le coded 24",
    codec: CODEC_TYPE_PCM_S32LE,
    coded: Some(24),
    bytes: &[
      1, 2, 3, 0x04, 0xff, 0xff, 0x7f, 0x00, 0, 0, 0, 1, 0x78, 0x56, 0x34, 0x12,
    ],
    planes: "[50462976, 0] [2147483392, 878082048]",
  },
  Case {
    name: "s24le",
    codec: CODEC_TYPE_PCM_S24LE,
    coded: Some(24),
    bytes: &[1, 2, 0x83, 0xff, 0xff, 0x7f, 0, 0, 1, 0x56, 0x34, 0x92],
    planes: "[i24(-8191487), i24(65536)] [i24(8388607), i24(-7195562)]",
  },
  Case {
    name: "s24be",
    codec: CODEC_TYPE_PCM_S24BE,
    coded: Some(24),
    bytes: &[0x83, 2, 1, 0x7f, 0xff, 0xff, 1, 0, 0, 0x92, 0x34, 0x56],
    planes: "[i24(-8191487), i24(65536)] [i24(8388607), i24(-7195562)]",
  },
  Case {
    name: "s24le coded 20",
    codec: CODEC_TYPE_PCM_S24LE,
    coded: Some(20),
    bytes: &[1, 2, 0x03, 0xff, 0xff, 0x07, 0, 0, 1, 0x56, 0x34, 0x02],
    planes: "[i24(3153936), i24(1048576)] [i24(8388592), i24(2311520)]",
  },
  Case {
    name: "s16le",
    codec: CODEC_TYPE_PCM_S16LE,
    coded: Some(16),
    bytes: &[1, 0x80, 0xff, 0x7f, 0, 1, 0x34, 0x12],
    planes: "[-32767, 256] [32767, 4660]",
  },
  Case {
    name: "s16be",
    codec: CODEC_TYPE_PCM_S16BE,
    coded: Some(16),
    bytes: &[0x80, 1, 0x7f, 0xff, 1, 0, 0x12, 0x34],
    planes: "[-32767, 256] [32767, 4660]",
  },
  Case {
    name: "s16le coded 12",
    codec: CODEC_TYPE_PCM_S16LE,
    coded: Some(12),
    bytes: &[1, 0xf8, 0xff, 0x07, 0, 1, 0x34, 0x02],
    planes: "[-32752, 4096] [32752, 9024]",
  },
  Case {
    name: "s8",
    codec: CODEC_TYPE_PCM_S8,
    coded: Some(8),
    bytes: &[0x80, 0x7f, 1, 0xff],
    planes: "[-128, 1] [127, -1]",
  },
  Case {
    name: "s8 coded 4",
    codec: CODEC_TYPE_PCM_S8,
    coded: Some(4),
    bytes: &[0x08, 0x07, 1, 0xff],
    planes: "[-128, 16] [112, -16]",
  },
  Case {
    name: "u32le",
    codec: CODEC_TYPE_PCM_U32LE,
    coded: Some(32),
    bytes: &[
      1, 2, 3, 0x84, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0, 0x78, 0x56, 0x34, 0x12,
    ],
    planes: "[2214789633, 0] [4294967295, 305419896]",
  },
  Case {
    name: "u32be",
    codec: CODEC_TYPE_PCM_U32BE,
    coded: Some(32),
    bytes: &[
      0x84, 3, 2, 1, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 1, 0x12, 0x34, 0x56, 0x78,
    ],
    planes: "[2214789633, 1] [4294967295, 305419896]",
  },
  Case {
    name: "u24le",
    codec: CODEC_TYPE_PCM_U24LE,
    coded: Some(24),
    bytes: &[1, 2, 0x83, 0xff, 0xff, 0xff, 0, 0, 0, 0x56, 0x34, 0x92],
    planes: "[u24(8585729), u24(0)] [u24(16777215), u24(9581654)]",
  },
  Case {
    name: "u24be",
    codec: CODEC_TYPE_PCM_U24BE,
    coded: Some(24),
    bytes: &[0x83, 2, 1, 0xff, 0xff, 0xff, 0, 0, 1, 0x92, 0x34, 0x56],
    planes: "[u24(8585729), u24(1)] [u24(16777215), u24(9581654)]",
  },
  Case {
    name: "u24le coded 20",
    codec: CODEC_TYPE_PCM_U24LE,
    coded: Some(20),
    bytes: &[1, 2, 0x03, 0xff, 0xff, 0x0f, 0, 0, 0, 0x56, 0x34, 0x02],
    planes: "[u24(3153936), u24(0)] [u24(16777200), u24(2311520)]",
  },
  Case {
    name: "u16le",
    codec: CODEC_TYPE_PCM_U16LE,
    coded: Some(16),
    bytes: &[1, 0x80, 0xff, 0xff, 0, 0, 0x34, 0x12],
    planes: "[32769, 0] [65535, 4660]",
  },
  Case {
    name: "u16be",
    codec: CODEC_TYPE_PCM_U16BE,
    coded: Some(16),
    bytes: &[0x80, 1, 0xff, 0xff, 0, 1, 0x12, 0x34],
    planes: "[32769, 1] [65535, 4660]",
  },
  Case {
    name: "u16le coded 12",
    codec: CODEC_TYPE_PCM_U16LE,
    coded: Some(12),
    bytes: &[1, 0x08, 0xff, 0x0f, 0, 0, 0x34, 0x02],
    planes: "[32784, 0] [65520, 9024]",
  },
  Case {
    name: "u8",
    codec: CODEC_TYPE_PCM_U8,
    coded: Some(8),
    bytes: &[0x80, 0xff, 0, 1],
    planes: "[128, 0] [255, 1]",
  },
  Case {
    name: "u8 coded 4",
    codec: CODEC_TYPE_PCM_U8,
    coded: Some(4),
    bytes: &[0x08, 0x0f, 0, 1],
    planes: "[128, 0] [240, 16]",
  },
  Case {
    name: "f32le",
    codec: CODEC_TYPE_PCM_F32LE,
    coded: None,
    bytes: &[
      0, 0, 0x80, 0x3f, 0, 0, 0, 0xbf, 0, 0, 0, 0, 0, 0, 0x40, 0x40,
    ],
    planes: "[1.0, 0.0] [-0.5, 3.0]",
  },
  Case {
    name: "f32be",
    codec: CODEC_TYPE_PCM_F32BE,
    coded: Some(32),
    bytes: &[
      0x3f, 0x80, 0, 0, 0xbf, 0, 0, 0, 0, 0, 0, 0, 0x40, 0x40, 0, 0,
    ],
    planes: "[1.0, 0.0] [-0.5, 3.0]",
  },
  Case {
    name: "f64le",
    codec: CODEC_TYPE_PCM_F64LE,
    coded: None,
    bytes: &[
      0, 0, 0, 0, 0, 0, 0xf0, 0x3f, 0, 0, 0, 0, 0, 0, 0xe0, 0xbf, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0x08, 0x40,
    ],
    planes: "[1.0, 0.0] [-0.5, 3.0]",
  },
  Case {
    name: "f64be",
    codec: CODEC_TYPE_PCM_F64BE,
    coded: None,
    bytes: &[
      0x3f, 0xf0, 0, 0, 0, 0, 0, 0, 0xbf, 0xe0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x40,
      0x08, 0, 0, 0, 0, 0, 0,
    ],
    planes: "[1.0, 0.0] [-0.5, 3.0]",
  },
  Case {
    name: "alaw",
    codec: CODEC_TYPE_PCM_ALAW,
    coded: None,
    bytes: &[0, 0x55, 0xd5, 0xff],
    planes: "[-5504, 8] [-8, 848]",
  },
  Case {
    name: "mulaw",
    codec: CODEC_TYPE_PCM_MULAW,
    coded: Some(8),
    bytes: &[0, 0x7f, 0x80, 0xff],
    planes: "[-32124, 32124] [0, 0]",
  },
];

#[test]
fn every_codec_reads_two_frames_of_stereo() -> Result<(), Failure> {
  let mut mismatches = Vec::new();

  for case in CASES {
    let actual = decoded(case.codec, case.coded, case.bytes)?;

    match actual.as_str().cmp(case.planes) {
      std::cmp::Ordering::Equal => {}
      std::cmp::Ordering::Less | std::cmp::Ordering::Greater => {
        mismatches.push(format!("{}: {actual:?}", case.name));
      }
    }
  }

  assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));

  Ok(())
}

#[test]
fn a_short_packet_is_a_fault_and_leaves_nothing_behind() -> Result<(), Failure> {
  let Ok(parameters) = parameters(CODEC_TYPE_PCM_S16LE, Some(16));
  let mut decoder = PcmDecoder::try_new(&parameters, &DecoderOptions::default())?;
  let Ok(value_5) = Packet::new_from_slice(0, 0, 2, &[1, 0, 2, 0, 3, 0, 4, 0]);

  let whole = value_5;
  let Ok(value_6) = Packet::new_from_slice(0, 0, 2, &[1, 0, 2, 0, 3]);

  let short = value_6;

  let first_audio = decoder.decode(&whole)?;
  let Ok(first) = shown_ref(first_audio);
  assert_eq!(first, "[1, 3] [2, 4]");

  let refusal = decoder.decode(&short).map(|_| ());
  assert_eq!(
    refusal.map_err(|fault| fault.to_string()),
    Err(String::from("buffer underrun"))
  );

  let Ok(value_7) = decoder.last_decoded();

  let Ok(left) = shown_ref(value_7);
  assert_eq!(left, "[] []");

  let second_audio = decoder.decode(&whole)?;
  let Ok(again) = shown_ref(second_audio);
  assert_eq!(again, "[1, 3] [2, 4]");

  Ok(())
}

#[test]
fn every_byte_of_a_law_is_one_sample() -> Result<(), Failure> {
  let everything: Vec<u8> = (0..=255u8).collect();
  let tables = [
    (CODEC_TYPE_PCM_ALAW, include_str!("laws/a_law.txt")),
    (CODEC_TYPE_PCM_MULAW, include_str!("laws/mu_law.txt")),
  ];

  for (codec, table) in tables {
    let Ok(value_8) = CodecParameters::new();

    let mut parameters = value_8;

    let Ok(_configured_for_codec) = parameters.for_codec(codec);
    let Ok(_configured_with_sample_rate) = parameters.with_sample_rate(8000);
    let Ok(_configured_with_channels) = parameters.with_channels(Channels::FRONT_LEFT);
    let Ok(_configured_with_max_frames_per_packet) = parameters.with_max_frames_per_packet(256);

    let mut decoder = PcmDecoder::try_new(&parameters, &DecoderOptions::default())?;
    let Ok(value_9) = Packet::new_from_slice(0, 0, 256, &everything);

    let packet = value_9;
    let audio = decoder.decode(&packet)?;
    let Ok(rendered) = shown_ref(audio);

    assert_eq!(rendered, table.trim());
  }

  Ok(())
}

#[test]
fn what_the_decoder_will_not_take_it_says() {
  let attempts = [
    (CODEC_TYPE_NULL, None),
    (CODEC_TYPE_PCM_S16LE, None),
    (CODEC_TYPE_PCM_S16LE, Some(0)),
    (CODEC_TYPE_PCM_S16LE, Some(17)),
    (CODEC_TYPE_PCM_F32LE, Some(33)),
    (CODEC_TYPE_PCM_ALAW, Some(17)),
    (CODEC_TYPE_PCM_F32LE, Some(16)),
  ];
  let faults: Vec<String> = attempts
    .iter()
    .map(|(codec, coded)| {
      let Ok(parameters) = parameters(*codec, *coded);
      let Ok(fault) = refused(&parameters);

      fault
    })
    .collect();

  assert_eq!(
    faults.join("\n"),
    "unsupported feature: pcm: invalid codec type\nunsupported feature: pcm: unknown bits per (coded) sample\nunsupported feature: pcm: unknown bits per (coded) sample\nmalformed stream: pcm: coded bits per sample is greater than the sample format\nmalformed stream: pcm: coded bits per sample is greater than the sample format\nmalformed stream: pcm: coded bits per sample is greater than the sample format\naccepted"
  );
}

#[test]
fn the_parameters_a_stream_must_give() {
  let Ok(value_10) = CodecParameters::new();

  let mut without_frames = value_10;
  let Ok(_configured_for_codec) = without_frames.for_codec(CODEC_TYPE_PCM_S16LE);
  let Ok(_configured_with_sample_rate) = without_frames.with_sample_rate(44100);
  let Ok(_configured_with_channels) = without_frames.with_channels(Channels::FRONT_LEFT);
  let Ok(_configured_with_bits_per_sample) = without_frames.with_bits_per_sample(16);

  let Ok(value_11) = CodecParameters::new();

  let mut without_rate = value_11;
  let Ok(_configured_for_codec) = without_rate.for_codec(CODEC_TYPE_PCM_S16LE);
  let Ok(_configured_with_channels) = without_rate.with_channels(Channels::FRONT_LEFT);
  let Ok(_configured_with_max_frames_per_packet) = without_rate.with_max_frames_per_packet(2);
  let Ok(_configured_with_bits_per_sample) = without_rate.with_bits_per_sample(16);

  let Ok(value_12) = CodecParameters::new();

  let mut without_channels = value_12;
  let Ok(_configured_for_codec) = without_channels.for_codec(CODEC_TYPE_PCM_S16LE);
  let Ok(_configured_with_sample_rate) = without_channels.with_sample_rate(44100);
  let Ok(_configured_with_max_frames_per_packet) = without_channels.with_max_frames_per_packet(2);
  let Ok(_configured_with_bits_per_sample) = without_channels.with_bits_per_sample(16);

  let Ok(value_13) = CodecParameters::new();

  let mut no_channels = value_13;
  let Ok(_configured_for_codec) = no_channels.for_codec(CODEC_TYPE_PCM_S16LE);
  let Ok(_configured_with_sample_rate) = no_channels.with_sample_rate(44100);
  let Ok(value_14) = Channels::empty();

  let Ok(_configured_with_channels) = no_channels.with_channels(value_14);
  let Ok(_configured_with_max_frames_per_packet) = no_channels.with_max_frames_per_packet(2);
  let Ok(_configured_with_bits_per_sample) = no_channels.with_bits_per_sample(16);

  let Ok(value_15) = CodecParameters::new();

  let mut by_sample_width = value_15;
  let Ok(_configured_for_codec) = by_sample_width.for_codec(CODEC_TYPE_PCM_S16LE);
  let Ok(_configured_with_sample_rate) = by_sample_width.with_sample_rate(44100);
  let Ok(_configured_with_channels) = by_sample_width.with_channels(Channels::FRONT_LEFT);
  let Ok(_configured_with_max_frames_per_packet) = by_sample_width.with_max_frames_per_packet(2);
  let Ok(_configured_with_bits_per_sample) = by_sample_width.with_bits_per_sample(16);

  let faults: Vec<String> = [
    &without_frames,
    &without_rate,
    &without_channels,
    &no_channels,
    &by_sample_width,
  ]
  .into_iter()
  .map(|parameters| {
    let Ok(fault) = refused(parameters);

    fault
  })
  .collect();

  assert_eq!(
    faults.join("\n"),
    "unsupported feature: pcm: maximum frames per packet is required\nunsupported feature: pcm: sample rate is required\nunsupported feature: pcm: channels or channel_layout is required\nunsupported feature: pcm: number of channels cannot be 0\naccepted"
  );
}

#[test]
fn the_decoder_lists_what_it_decodes() {
  let Ok(supported_codecs) = PcmDecoder::supported_codecs();
  let names: Vec<&str> = supported_codecs
    .iter()
    .map(|descriptor| descriptor.short_name)
    .collect();

  assert_eq!(
    names.join(" "),
    "pcm_s32le pcm_s32be pcm_s24le pcm_s24be pcm_s16le pcm_s16be pcm_s8 pcm_u32le pcm_u32be pcm_u24le pcm_u24be pcm_u16le pcm_u16be pcm_u8 pcm_f32le pcm_f32be pcm_f64le pcm_f64be pcm_alaw pcm_mulaw"
  );
}

#[test]
fn a_decoder_made_for_the_registry_is_the_same_decoder() -> Result<(), Failure> {
  let Ok(descriptors) = PcmDecoder::supported_codecs();
  let descriptor = descriptors
    .iter()
    .find(|descriptor| descriptor.codec == CODEC_TYPE_PCM_S16LE)
    .ok_or("no s16le")?;
  let Ok(parameters) = parameters(CODEC_TYPE_PCM_S16LE, Some(16));
  let mut made = (descriptor.inst_func)(&parameters, &DecoderOptions::default())?;
  let Ok(value_16) = Packet::new_from_slice(0, 0, 2, &[1, 0, 2, 0, 3, 0, 4, 0]);

  let packet = value_16;
  let audio = made.decode(&packet)?;
  let Ok(rendered) = shown_ref(audio);

  assert_eq!(rendered, "[1, 3] [2, 4]");

  Ok(())
}
