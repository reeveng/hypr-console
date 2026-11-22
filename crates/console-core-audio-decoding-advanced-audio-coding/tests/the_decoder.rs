//! Three ADTS streams decoded end to end, and the samples they come out as.
//!
//! The expected checksums were taken from the decoder as it was carried over
//! from upstream, before any of it was rewritten, so a change that moves one
//! sample is a change that fails here.

use std::error::Error;
use std::io::Cursor;

use console_core_audio_decoding_advanced_audio_coding::{AacDecoder, AdtsReader};
use console_core_audio_decoding_interfaces::audio::{AudioBuffer, AudioBufferRef, Signal};
use console_core_audio_decoding_interfaces::codecs::{Decoder, DecoderOptions};
use console_core_audio_decoding_interfaces::formats::{FormatOptions, FormatReader, Packet};
use console_core_audio_decoding_interfaces::io::{MediaSourceStream, MediaSourceStreamOptions};
use console_core_checksums::crc32;
use console_core_number_conversion::index;

type Failure = Box<dyn Error>;

struct Decoded {
  sample_rate: Option<u32>,
  packets: u32,
  bytes: Vec<u8>,
}

struct Packets {
  reader: AdtsReader,
}

impl Iterator for Packets {
  type Item = Packet;

  fn next(&mut self) -> Option<Packet> {
    match self.reader.next_packet() {
      Ok(packet) => Some(packet),
      Err(_ended) => None,
    }
  }
}

fn interleave(buffer: &AudioBuffer<f32>, bytes: &mut Vec<u8>) -> Result<(), Failure> {
  let Ok(value_1) = buffer.frames();

  for frame in 0..value_1 {
    let Ok(at) = index(frame);

    let Ok(value_2) = buffer.spec();

    let Ok(value_5) = value_2.channels.count();

    for channel in 0..value_5 {
      let Ok(value_3) = buffer.chan(channel);

      let sample = value_3.get(at).ok_or("short channel")?;

      bytes.extend(sample.to_le_bytes());
    }
  }

  Ok(())
}

fn decode(file: &[u8]) -> Result<Decoded, Failure> {
  let Ok(source) = MediaSourceStream::new(
    Box::new(Cursor::new(file.to_vec())),
    MediaSourceStreamOptions::default(),
  );
  let reader = AdtsReader::try_new(source, &FormatOptions::default())?;
  let Ok(value_4) = reader.tracks();

  let track = value_4.first().ok_or("no track")?;
  let parameters = track.codec_params.clone();
  let mut decoder = AacDecoder::try_new(&parameters, &DecoderOptions::default())?;
  let mut bytes = Vec::new();
  let mut packets = 0u32;

  for packet in (Packets { reader }) {
    let decoded = decoder.decode(&packet)?;

    match decoded {
      AudioBufferRef::F32(buffer) => interleave(&buffer, &mut bytes)?,
      AudioBufferRef::U8(_)
      | AudioBufferRef::U16(_)
      | AudioBufferRef::U24(_)
      | AudioBufferRef::U32(_)
      | AudioBufferRef::S8(_)
      | AudioBufferRef::S16(_)
      | AudioBufferRef::S24(_)
      | AudioBufferRef::S32(_)
      | AudioBufferRef::F64(_) => return Err(Failure::from("not f32")),
    }

    packets = packets.saturating_add(1);
  }

  Ok(Decoded {
    sample_rate: parameters.sample_rate,
    packets,
    bytes,
  })
}

#[test]
fn a_mono_stream_comes_out_as_it_always_did() -> Result<(), Failure> {
  let decoded = decode(include_bytes!("fixtures/mono.aac"))?;

  assert_eq!(decoded.sample_rate, Some(22_050));
  assert_eq!(decoded.packets, 12);
  assert_eq!(decoded.bytes.len(), 49_152);
  let Ok(crc) = crc32::of(&decoded.bytes);

  assert_eq!(crc, 1_104_846_278);
  Ok(())
}

#[test]
fn a_stereo_stream_comes_out_as_it_always_did() -> Result<(), Failure> {
  let decoded = decode(include_bytes!("fixtures/stereo.aac"))?;

  assert_eq!(decoded.sample_rate, Some(44_100));
  assert_eq!(decoded.packets, 23);
  assert_eq!(decoded.bytes.len(), 188_416);
  let Ok(crc) = crc32::of(&decoded.bytes);

  assert_eq!(crc, 547_898_262);
  Ok(())
}

#[test]
fn a_loud_stream_comes_out_as_it_always_did() -> Result<(), Failure> {
  let decoded = decode(include_bytes!("fixtures/loud.aac"))?;

  assert_eq!(decoded.sample_rate, Some(48_000));
  assert_eq!(decoded.packets, 20);
  assert_eq!(decoded.bytes.len(), 163_840);
  let Ok(crc) = crc32::of(&decoded.bytes);

  assert_eq!(crc, 4_101_945_000);
  Ok(())
}
