//! A stream cut from the real encoder is decoded to the end, and what comes
//! out is compared with a digest taken before the decoder was rewritten.
//!
//! The streams in `streams/` were made by libvorbis from a tone with clicks in
//! it, so both block sizes are used and the windows between them are crossed.
//! A digest covers every sample of every channel in the order they were
//! decoded, so a sample that moved by one bit changes it.

use std::io::Cursor;

use console_core_audio_decoding_interfaces::audio::{AudioBuffer, AudioBufferRef, Signal};
use console_core_audio_decoding_interfaces::codecs::{Decoder, DecoderOptions};
use console_core_audio_decoding_interfaces::errors::Error;
use console_core_audio_decoding_interfaces::formats::{FormatOptions, FormatReader, Packet};
use console_core_audio_decoding_interfaces::io::{MediaSourceStream, MediaSourceStreamOptions};
use console_core_audio_decoding_ogg_container::OggReader;
use console_core_audio_decoding_vorbis::VorbisDecoder;

type Failure = Box<dyn std::error::Error>;

use console_core_checksums::fnv1a::{self, OFFSET64 as OFFSET};
use console_core_never::Never;

#[derive(Debug, PartialEq, Eq)]
struct Decoded {
  packets: u64,
  frames: u64,
  digest: u64,
}

fn fold(digest: u64, word: u32) -> Result<u64, Never> {
  fnv1a::update64(digest, &word.to_le_bytes())
}

struct Packets {
  reader: OggReader,
}

impl Iterator for Packets {
  type Item = Result<Packet, Error>;

  fn next(&mut self) -> Option<Self::Item> {
    match self.reader.next_packet() {
      Ok(packet) => Some(Ok(packet)),
      Err(Error::IoError(_ended)) => None,
      Err(other) => Some(Err(other)),
    }
  }
}

fn decode(stream: &[u8]) -> Result<Decoded, Failure> {
  let Ok(source) = MediaSourceStream::new(
    Box::new(Cursor::new(stream.to_vec())),
    MediaSourceStreamOptions::default(),
  );
  let reader = OggReader::try_new(source, &FormatOptions::default())?;
  let Ok(value_1) = reader.default_track();

  let track = value_1.ok_or("no track")?;
  let params = track.codec_params.clone();
  let mut decoder = VorbisDecoder::try_new(&params, &DecoderOptions::default())?;
  let mut held = Decoded {
    packets: 0,
    frames: 0,
    digest: OFFSET,
  };
  let packets = Packets { reader };

  for read in packets {
    let packet = read?;
    let decoded = decoder.decode(&packet)?;
    let mut samples: AudioBuffer<f32> = decoded.make_equivalent()?;

    AudioBufferRef::convert(&decoded, &mut samples)?;

    held.packets = held.packets.saturating_add(1);
    let Ok(value_2) = samples.frames();

    held.frames = held.frames.saturating_add(u64::from(value_2));

    let Ok(planes) = samples.planes();
    let Ok(channels) = planes.planes();

    for sample in channels.iter().flat_map(|plane| plane.iter()) {
      let Ok(next) = fold(held.digest, sample.to_bits());

      held.digest = next;
    }
  }

  Ok(held)
}

fn expect(stream: &[u8], expected: Decoded) -> Result<(), Failure> {
  let decoded = decode(stream)?;

  assert_eq!(decoded, expected);

  Ok(())
}

#[test]
fn a_mono_stream_is_decoded_to_the_same_samples() -> Result<(), Failure> {
  expect(
    include_bytes!("streams/mono.ogg"),
    Decoded {
      packets: 36,
      frames: 8960,
      digest: 9394002791397998368,
    },
  )
}

#[test]
fn a_stereo_stream_is_decoded_to_the_same_samples() -> Result<(), Failure> {
  expect(
    include_bytes!("streams/stereo.ogg"),
    Decoded {
      packets: 33,
      frames: 12800,
      digest: 7706977776626769605,
    },
  )
}

#[test]
fn three_channels_are_decoded_to_the_same_samples() -> Result<(), Failure> {
  expect(
    include_bytes!("streams/ch3.ogg"),
    Decoded {
      packets: 9,
      frames: 2048,
      digest: 15711917676650053952,
    },
  )
}

#[test]
fn four_channels_are_decoded_to_the_same_samples() -> Result<(), Failure> {
  expect(
    include_bytes!("streams/ch4.ogg"),
    Decoded {
      packets: 9,
      frames: 2048,
      digest: 16959435506721908482,
    },
  )
}

#[test]
fn five_channels_are_decoded_to_the_same_samples() -> Result<(), Failure> {
  expect(
    include_bytes!("streams/ch5.ogg"),
    Decoded {
      packets: 9,
      frames: 2048,
      digest: 10977907068243650460,
    },
  )
}

#[test]
fn six_channels_are_decoded_to_the_same_samples() -> Result<(), Failure> {
  expect(
    include_bytes!("streams/ch6.ogg"),
    Decoded {
      packets: 9,
      frames: 2048,
      digest: 3908444420688888789,
    },
  )
}

#[test]
fn seven_channels_are_decoded_to_the_same_samples() -> Result<(), Failure> {
  expect(
    include_bytes!("streams/ch7.ogg"),
    Decoded {
      packets: 9,
      frames: 2048,
      digest: 8018207651830449725,
    },
  )
}

#[test]
fn eight_channels_are_decoded_to_the_same_samples() -> Result<(), Failure> {
  expect(
    include_bytes!("streams/ch8.ogg"),
    Decoded {
      packets: 9,
      frames: 2048,
      digest: 5423138802972413920,
    },
  )
}

#[test]
fn a_stream_without_its_headers_is_refused() -> Result<(), Failure> {
  let Ok(params) = console_core_audio_decoding_interfaces::codecs::CodecParameters::new();

  let refused = VorbisDecoder::try_new(&params, &DecoderOptions::default());

  assert!(matches!(refused, Err(_missing_headers)));

  Ok(())
}
