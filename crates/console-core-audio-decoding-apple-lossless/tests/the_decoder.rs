//! Streams built here, bit by bit, and pressed through the decoder.
//!
//! The uncompressed ones say what comes out sample by sample. The compressed
//! ones are residuals of a fixed pseudo-random stream under real headers, so
//! what they hold is a fingerprint of every sample the predictor and the
//! rice decoder produced, taken from the decoder as it was ported in.

use console_core_audio_decoding_apple_lossless::AlacDecoder;
use console_core_audio_decoding_interfaces::audio::{AudioBufferRef, Signal};
use console_core_audio_decoding_interfaces::codecs::{
  CODEC_TYPE_ALAC, CodecParameters, Decoder, DecoderOptions,
};
use console_core_audio_decoding_interfaces::errors::Error;
use console_core_audio_decoding_interfaces::formats::Packet;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

type Field = (u64, u32);

#[derive(Debug)]
enum Failed {
  Decoding(Error),
  NotThirtyTwoBit,
}

impl From<Error> for Failed {
  fn from(fault: Error) -> Self {
    Failed::Decoding(fault)
  }
}

#[derive(Clone, Copy)]
enum Coding {
  Compressed,
  Uncompressed,
}

struct Configuration {
  frame_length: u32,
  bit_depth: u8,
  channels: u8,
}

struct Compressed {
  channels: u8,
  shift_code: u8,
  mode: u64,
  order: u32,
  seed: u64,
  residual_bytes: u32,
}

fn pack(fields: &[Field]) -> Result<Vec<u8>, Never> {
  let mut bits: Vec<bool> = fields
    .iter()
    .flat_map(|&(value, width)| {
      (0..width)
        .rev()
        .map(move |bit| value.wrapping_shr(bit) & 1 == 1)
    })
    .collect();

  bits.extend([false; 7]);

  Ok(
    bits
      .as_chunks::<8>()
      .0
      .iter()
      .map(|chunk| {
        chunk
          .iter()
          .fold(0u8, |byte, &bit| byte.wrapping_shl(1) | u8::from(bit))
      })
      .collect(),
  )
}

fn signed(value: i64, width: u32) -> Result<Field, Never> {
  let bits = u64::from_be_bytes(value.to_be_bytes());

  Ok((bits & 1u64.wrapping_shl(width).wrapping_sub(1), width))
}

fn draws(seed: u64, count: u32) -> Result<Vec<u64>, Never> {
  Ok(
    (0..count)
      .scan(seed, |state, _| {
        *state = state
          .wrapping_mul(6364136223846793005)
          .wrapping_add(1442695040888963407);

        Some(state.wrapping_shr(33))
      })
      .collect(),
  )
}

fn cookie(config: Configuration) -> Result<Vec<u8>, Never> {
  let mut bytes = Vec::new();

  bytes.extend(config.frame_length.to_be_bytes());
  bytes.extend([0, config.bit_depth, 40, 10, 14, config.channels]);
  bytes.extend(255u16.to_be_bytes());
  bytes.extend(0u32.to_be_bytes());
  bytes.extend(0u32.to_be_bytes());
  bytes.extend(44100u32.to_be_bytes());

  Ok(bytes)
}

fn decoder(cookie: Vec<u8>) -> Result<AlacDecoder, Error> {
  let Ok(value_1) = CodecParameters::new();

  let mut params = value_1;

  let Ok(_configured_for_codec) = params.for_codec(CODEC_TYPE_ALAC);
  let Ok(_configured_with_extra_data) = params.with_extra_data(cookie.into_boxed_slice());

  AlacDecoder::try_new(&params, &DecoderOptions::default())
}

fn decoded(decoder: &mut AlacDecoder, bytes: Vec<u8>) -> Result<Vec<Vec<i32>>, Failed> {
  let Ok(value_2) = Packet::new_from_boxed_slice(0, 0, 0, bytes.into_boxed_slice());

  let packet = value_2;
  let outcome = decoder.decode(&packet)?;

  match outcome {
    AudioBufferRef::S32(buffer) => {
      let Ok(value_3) = buffer.spec();

      let Ok(value_5) = value_3.channels.count();

      Ok(
        (0..value_5)
          .map(|channel| {
            let Ok(value_4) = buffer.chan(channel);

            value_4.to_vec()
          })
          .collect(),
      )
    }
    AudioBufferRef::U8(_)
    | AudioBufferRef::U16(_)
    | AudioBufferRef::U24(_)
    | AudioBufferRef::U32(_)
    | AudioBufferRef::S8(_)
    | AudioBufferRef::S16(_)
    | AudioBufferRef::S24(_)
    | AudioBufferRef::F32(_)
    | AudioBufferRef::F64(_) => Err(Failed::NotThirtyTwoBit),
  }
}

fn fingerprint(planes: &[Vec<i32>]) -> Result<u64, Never> {
  Ok(
    planes
      .iter()
      .flatten()
      .flat_map(|sample| sample.to_be_bytes())
      .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
      }),
  )
}

fn element(tag: u64, shift_code: u8, coding: Coding) -> Result<Vec<Field>, Never> {
  let uncompressed = match coding {
    Coding::Compressed => 0,
    Coding::Uncompressed => 1,
  };

  Ok(vec![
    (tag, 3),
    (0, 4),
    (0, 12),
    (0, 1),
    (u64::from(shift_code), 2),
    (uncompressed, 1),
  ])
}

fn compressed(stream: Compressed) -> Result<Vec<u8>, Never> {
  let channels = u32::from(stream.channels);
  let coefficient_count = channels.wrapping_mul(stream.order);
  let tail_count = u32::from(stream.shift_code)
    .wrapping_mul(512)
    .wrapping_mul(channels);
  let Ok(tails) = index(tail_count);
  let Ok(residuals) = index(stream.residual_bytes);
  let Ok(drawn) = draws(
    stream.seed,
    coefficient_count
      .wrapping_add(tail_count)
      .wrapping_add(stream.residual_bytes),
  );
  let mut noise = drawn.into_iter();
  let coupled = u64::from(stream.channels.saturating_sub(1));
  let Ok(mut fields) = element(coupled, stream.shift_code, Coding::Compressed);

  fields.extend([(coupled.wrapping_mul(2), 8), (coupled, 8)]);

  for _ in 0..channels {
    fields.extend([
      (stream.mode, 4),
      (3, 4),
      (5, 3),
      (u64::from(stream.order), 5),
    ]);

    let Ok(taps) = index(stream.order);

    for draw in noise.by_ref().take(taps) {
      let Ok(small): Result<u32, Never> = fitted(draw.wrapping_rem(400));
      let Ok(field) = signed(i64::from(small).wrapping_sub(200), 16);

      fields.push(field);
    }
  }

  fields.extend(noise.by_ref().take(tails).map(|draw| (draw & 1, 1)));
  fields.extend(noise.by_ref().take(residuals).map(|draw| (draw & 0xff, 8)));
  fields.push((7, 3));

  pack(&fields)
}

#[test]
fn an_uncompressed_mono_frame_comes_out_shifted_up_to_32_bits() -> Result<(), Failed> {
  let Ok(mut fields) = element(0, 0, Coding::Uncompressed);

  for sample in [1i64, -2, 300, -32768, 32767, 0, 0, 0] {
    let Ok(field) = signed(sample, 16);

    fields.push(field);
  }

  fields.push((7, 3));

  let Ok(packet) = pack(&fields);
  let Ok(bytes) = cookie(Configuration {
    frame_length: 8,
    bit_depth: 16,
    channels: 1,
  });
  let mut alac = decoder(bytes)?;
  let planes = decoded(&mut alac, packet)?;

  assert_eq!(
    planes,
    vec![vec![
      65536,
      -131072,
      19660800,
      -2147483648,
      2147418112,
      0,
      0,
      0
    ]]
  );

  Ok(())
}

#[test]
fn an_uncompressed_pair_is_interleaved() -> Result<(), Failed> {
  let Ok(mut fields) = element(1, 0, Coding::Uncompressed);

  for sample in 0..4i64 {
    let Ok(left) = signed(sample, 16);
    let Ok(right) = signed(sample.wrapping_neg(), 16);

    fields.extend([left, right]);
  }

  fields.push((7, 3));

  let Ok(packet) = pack(&fields);
  let Ok(bytes) = cookie(Configuration {
    frame_length: 4,
    bit_depth: 16,
    channels: 2,
  });
  let mut alac = decoder(bytes)?;
  let planes = decoded(&mut alac, packet)?;

  assert_eq!(
    planes,
    vec![
      vec![0, 65536, 131072, 196608],
      vec![0, -65536, -131072, -196608]
    ]
  );

  Ok(())
}

#[test]
fn a_surround_frame_puts_each_element_on_its_own_channels() -> Result<(), Failed> {
  let mut fields = Vec::new();

  for (position, (tag, count)) in [(0u64, 1i64), (1, 2), (1, 2), (3, 1)]
    .into_iter()
    .enumerate()
  {
    let Ok(header) = element(tag, 0, Coding::Uncompressed);
    let Ok(base): Result<i64, Never> = fitted(position);

    fields.extend(header);

    for frame in 0..4i64 {
      for part in 0..count {
        let Ok(field) = signed(
          100i64
            .wrapping_mul(base)
            .wrapping_add(10i64.wrapping_mul(part))
            .wrapping_add(frame),
          16,
        );

        fields.push(field);
      }
    }
  }

  fields.push((7, 3));

  let Ok(packet) = pack(&fields);
  let Ok(bytes) = cookie(Configuration {
    frame_length: 4,
    bit_depth: 16,
    channels: 6,
  });
  let mut alac = decoder(bytes)?;
  let planes = decoded(&mut alac, packet)?;
  let firsts: Vec<Option<i32>> = planes
    .iter()
    .map(|plane| plane.first().map(|sample| sample.wrapping_shr(16)))
    .collect();
  let Ok(print) = fingerprint(&planes);

  assert_eq!(firsts, [100, 110, 0, 300, 200, 210].map(Some));
  assert_eq!(print, 4511243244292145073);

  Ok(())
}

#[test]
fn a_compressed_pair_decodes_the_same() -> Result<(), Failed> {
  let Ok(packet) = compressed(Compressed {
    channels: 2,
    shift_code: 0,
    mode: 0,
    order: 4,
    seed: 1,
    residual_bytes: 400,
  });
  let Ok(bytes) = cookie(Configuration {
    frame_length: 64,
    bit_depth: 16,
    channels: 2,
  });
  let mut alac = decoder(bytes)?;
  let planes = decoded(&mut alac, packet)?;
  let Ok(print) = fingerprint(&planes);

  assert_eq!(print, 9101092596303180304);

  Ok(())
}

#[test]
fn a_compressed_mono_frame_with_the_first_order_pass_decodes_the_same() -> Result<(), Failed> {
  let Ok(packet) = compressed(Compressed {
    channels: 1,
    shift_code: 0,
    mode: 15,
    order: 8,
    seed: 2,
    residual_bytes: 400,
  });
  let Ok(bytes) = cookie(Configuration {
    frame_length: 64,
    bit_depth: 16,
    channels: 1,
  });
  let mut alac = decoder(bytes)?;
  let planes = decoded(&mut alac, packet)?;
  let Ok(print) = fingerprint(&planes);

  assert_eq!(print, 13993647373690170407);

  Ok(())
}

#[test]
fn a_compressed_pair_with_tail_bits_decodes_the_same() -> Result<(), Failed> {
  let Ok(packet) = compressed(Compressed {
    channels: 2,
    shift_code: 1,
    mode: 0,
    order: 6,
    seed: 3,
    residual_bytes: 400,
  });
  let Ok(bytes) = cookie(Configuration {
    frame_length: 64,
    bit_depth: 24,
    channels: 2,
  });
  let mut alac = decoder(bytes)?;
  let planes = decoded(&mut alac, packet)?;
  let Ok(print) = fingerprint(&planes);

  assert_eq!(print, 7918388653430381513);

  Ok(())
}

#[test]
fn a_cookie_of_the_wrong_size_is_refused() {
  assert!(matches!(decoder(vec![0; 25]), Err(Error::Unsupported(_))));
}

#[test]
fn a_cookie_from_a_later_version_is_refused() {
  let Ok(mut bytes) = cookie(Configuration {
    frame_length: 64,
    bit_depth: 16,
    channels: 2,
  });

  for version in bytes.iter_mut().skip(4).take(1) {
    *version = 1;
  }

  assert!(matches!(decoder(bytes), Err(Error::Unsupported(_))));
}

#[test]
fn a_cookie_promising_an_enormous_frame_is_refused() {
  let Ok(bytes) = cookie(Configuration {
    frame_length: u32::MAX,
    bit_depth: 16,
    channels: 2,
  });

  assert!(matches!(decoder(bytes), Err(Error::DecodeError(_))));
}

#[test]
fn a_packet_that_stops_short_is_an_error() -> Result<(), Failed> {
  let Ok(bytes) = cookie(Configuration {
    frame_length: 64,
    bit_depth: 16,
    channels: 2,
  });
  let mut alac = decoder(bytes)?;
  let Ok(mut packet) = compressed(Compressed {
    channels: 2,
    shift_code: 0,
    mode: 0,
    order: 4,
    seed: 1,
    residual_bytes: 400,
  });

  packet.truncate(20);

  assert!(matches!(
    decoded(&mut alac, packet),
    Err(Failed::Decoding(Error::IoError(_)))
  ));

  Ok(())
}
