//! A frame header, which is where a FLAC stream says what the audio after it is.
//!
//! The header is read from a stream rather than a slice because the demuxer meets one while
//! hunting for the next frame, byte by byte, and the decoder meets one at the front of a
//! packet that is already whole. Both ask through `ReadBytes`. The header's last byte is a
//! CRC-8 over the rest, and a header that fails it is refused here, so a sync word found
//! in the middle of audio is told apart from a frame by this file alone.

use std::cmp::Ordering;

use console_core_audio_decoding_flac_and_vorbis_headers::flac::read_coded_number;
use console_core_audio_decoding_interfaces::errors::{Error, Result, decode_error};
use console_core_byte_streams::{Monitor, MonitorStream, ReadBytes};
use console_core_checksums::crc8::Crc8Ccitt;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::fitted;

use super::steps::run;

pub const MAX_FRAME_HEADER_SIZE: u32 = 16;

pub const MAX_FRAME_SIZE: u32 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockingStrategy {
  Fixed,
  Variable,
}

#[derive(Debug, Clone, Copy)]
pub enum BlockSequence {
  BySample(u64),
  ByFrame(u32),
}

#[derive(Debug, Clone, Copy)]
pub enum ChannelAssignment {
  Independent(u32),
  LeftSide,
  MidSide,
  RightSide,
}

pub struct FrameHeader {
  pub block_sequence: BlockSequence,
  pub block_num_samples: u16,
  pub channel_assignment: ChannelAssignment,
  pub bits_per_sample: Option<u32>,
  pub sample_rate: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plausible {
  Header,
  Noise,
}

impl From<bool> for Plausible {
  fn from(header: bool) -> Self {
    match bool::then_some(header, ()) {
      Some(()) => Self::Header,
      None => Self::Noise,
    }
  }
}

const SYNC_MASK: u16 = 0xfffc;

const SYNC_CODE: u16 = 0xfff8;

pub fn starts_with_sync(bytes: &[u8]) -> std::result::Result<Plausible, Never> {
  Ok(match bytes.first_chunk::<2>() {
    Some(pair) => Plausible::from(u16::from_be_bytes(*pair) & SYNC_MASK == SYNC_CODE),
    None => Plausible::Noise,
  })
}

fn find_sync<B: ReadBytes>(
  state: (&mut B, u16),
) -> std::result::Result<Step<(&mut B, u16), Result<u16>>, Never> {
  let (reader, sync) = state;

  Ok(match reader.read_u8() {
    Err(fault) => Step::Halt(Err(Error::from(fault))),
    Ok(byte) => {
      let next = sync.wrapping_shl(8) | u16::from(byte);

      match next & SYNC_MASK {
        SYNC_CODE => Step::Halt(Ok(next)),
        _seeking => Step::Again((reader, next)),
      }
    }
  })
}

pub fn sync_frame<B: ReadBytes>(reader: &mut B) -> Result<u16> {
  run((reader, 0u16), find_sync)
}

fn block_sequence<B: ReadBytes>(
  strategy: BlockingStrategy,
  reader: &mut B,
) -> Result<BlockSequence> {
  match strategy {
    BlockingStrategy::Fixed => {
      let decoded = read_coded_number(reader)?;

      let frame = match decoded {
        Some(frame) => frame,
        None => return decode_error("flac: frame sequence number is not valid"),
      };

      match frame {
        0..=0x7fff_ffff => {
          let Ok(held) = fitted::<u64, u32>(frame);

          Ok(BlockSequence::ByFrame(held))
        }
        _outside => decode_error("flac: frame sequence number exceeds 31-bits"),
      }
    }
    BlockingStrategy::Variable => {
      let decoded = read_coded_number(reader)?;

      let sample = match decoded {
        Some(sample) => sample,
        None => return decode_error("flac: sample sequence number is not valid"),
      };

      match sample {
        0..=0x000f_ffff_ffff => Ok(BlockSequence::BySample(sample)),
        _outside => decode_error("flac: sample sequence number exceeds 36-bits"),
      }
    }
  }
}

fn block_size<B: ReadBytes>(encoded: u32, reader: &mut B) -> Result<u16> {
  Ok(match encoded {
    0x1 => 192,
    0x2 => 576,
    0x3 => 1152,
    0x4 => 2304,
    0x5 => 4608,
    0x6 => {
      let size = reader.read_u8()?;

      u16::from(size).saturating_add(1)
    }
    0x7 => {
      let size = reader.read_be_u16()?;

      match size {
        0xffff => return decode_error("flac: block size not allowed to be greater than 65535"),
        size_2 => size_2.saturating_add(1),
      }
    }
    0x8 => 256,
    0x9 => 512,
    0xa => 1024,
    0xb => 2048,
    0xc => 4096,
    0xd => 8192,
    0xe => 16384,
    0xf => 32768,
    _ => return decode_error("flac: block size set to reserved value"),
  })
}

fn sample_rate<B: ReadBytes>(encoded: u32, reader: &mut B) -> Result<Option<u32>> {
  let rate = match encoded {
    0x0 => return Ok(None),
    0x1 => 88_200,
    0x2 => 176_400,
    0x3 => 192_000,
    0x4 => 8_000,
    0x5 => 16_000,
    0x6 => 22_050,
    0x7 => 24_000,
    0x8 => 32_000,
    0x9 => 44_100,
    0xa => 48_000,
    0xb => 96_000,
    0xc => {
      let kilohertz = reader.read_u8()?;

      u32::from(kilohertz).saturating_mul(1000)
    }
    0xd => {
      let hertz = reader.read_be_u16()?;

      u32::from(hertz)
    }
    0xe => {
      let tens = reader.read_be_u16()?;

      u32::from(tens).saturating_mul(10)
    }
    _ => return decode_error("flac: sample rate set to reserved value"),
  };

  match rate {
    1..=655_350 => Ok(Some(rate)),
    _ => decode_error("flac: sample rate out of bounds"),
  }
}

fn bits_per_sample(encoded: u32) -> Result<Option<u32>> {
  Ok(match encoded {
    0x0 => None,
    0x1 => Some(8),
    0x2 => Some(12),
    0x4 => Some(16),
    0x5 => Some(20),
    0x6 => Some(24),
    _ => return decode_error("flac: bits per sample set to reserved value"),
  })
}

fn channel_assignment(encoded: u32) -> Result<ChannelAssignment> {
  match encoded {
    0x0..=0x7 => Ok(ChannelAssignment::Independent(encoded.saturating_add(1))),
    0x8 => Ok(ChannelAssignment::LeftSide),
    0x9 => Ok(ChannelAssignment::RightSide),
    0xa => Ok(ChannelAssignment::MidSide),
    _ => decode_error("flac: channel assignment set to reserved value"),
  }
}

pub fn read_frame_header<B: ReadBytes>(reader: &mut B, sync: u16) -> Result<FrameHeader> {
  let Ok(mut crc8) = Crc8Ccitt::new(0);
  let Ok(()) = crc8.process_buf_bytes(&sync.to_be_bytes());

  let Ok(mut reader_crc8) = MonitorStream::new(reader, crc8);

  let strategy = match sync & 0x1 {
    0 => BlockingStrategy::Fixed,
    _ => BlockingStrategy::Variable,
  };

  let desc = reader_crc8.read_be_u16()?;

  match desc & 0x0001 {
    0 => (),
    _ => return decode_error("flac: frame header reserved bit is not set to mandatory value"),
  }

  let block_sequence = block_sequence(strategy, &mut reader_crc8)?;
  let block_num_samples = block_size(
    u32::from((desc & 0xf000).wrapping_shr(12)),
    &mut reader_crc8,
  )?;
  let sample_rate = sample_rate(u32::from((desc & 0x0f00).wrapping_shr(8)), &mut reader_crc8)?;
  let bits_per_sample = bits_per_sample(u32::from((desc & 0x000e).wrapping_shr(1)))?;
  let channel_assignment = channel_assignment(u32::from((desc & 0x00f0).wrapping_shr(4)))?;

  let Ok(monitor) = reader_crc8.monitor();
  let Ok(crc8_computed) = monitor.crc();
  let Ok(rest) = reader_crc8.into_inner();
  let crc8_expected = rest.read_u8()?;

  match crc8_expected.cmp(&crc8_computed) {
    Ordering::Equal => Ok(FrameHeader {
      block_sequence,
      block_num_samples,
      channel_assignment,
      bits_per_sample,
      sample_rate,
    }),
    Ordering::Less | Ordering::Greater => {
      decode_error("flac: computed frame header CRC does not match expected CRC")
    }
  }
}

pub fn looks_like_frame_header(buf: &[u8]) -> std::result::Result<Plausible, Never> {
  let (first, second, size, assignment) = match buf.first_chunk::<6>() {
    Some(&[first, second, size, assignment, ..]) => (first, second, size, assignment),
    None => return Ok(Plausible::Noise),
  };

  let plausible = first == 0xff
    && second & 0xfc == 0xf8
    && size & 0xf0 != 0x00
    && size & 0x0f != 0x0f
    && assignment & 0xf0 < 0xb0
    && assignment & 0x0e != 0x06
    && assignment & 0x0e != 0x0e
    && assignment & 0x01 == 0;

  Ok(Plausible::from(plausible))
}
