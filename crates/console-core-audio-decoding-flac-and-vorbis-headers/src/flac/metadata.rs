//! FLAC's metadata blocks: the header every block opens with, the stream information that
//! says how to decode the rest, and the blocks a demuxer keeps (seek points, cues, the
//! application's own bytes).

use std::ascii;
use std::cmp::Ordering;

use console_core_audio_decoding_interfaces::audio::Channels;
use console_core_audio_decoding_interfaces::errors::{Result, decode_error};
use console_core_audio_decoding_interfaces::formats::{Cue, CuePoint, util::SeekIndex};
use console_core_audio_decoding_interfaces::meta::{StandardTagKey, Tag, Value};
use console_core_byte_streams::*;

#[derive(PartialEq, Eq)]
pub enum MetadataBlockType {
  StreamInfo,
  Padding,
  Application,
  SeekTable,
  VorbisComment,
  Cuesheet,
  Picture,
  Unknown(u8),
}

fn flac_channels_to_channels(channels: u32) -> Result<Channels> {
  let layout = match channels {
    1 => Channels::FRONT_LEFT,
    2 => Channels::FRONT_LEFT | Channels::FRONT_RIGHT,
    3 => Channels::FRONT_LEFT | Channels::FRONT_RIGHT | Channels::FRONT_CENTRE,
    4 => {
      Channels::FRONT_LEFT | Channels::FRONT_RIGHT | Channels::REAR_LEFT | Channels::REAR_RIGHT
    }
    5 => {
      Channels::FRONT_LEFT
        | Channels::FRONT_RIGHT
        | Channels::FRONT_CENTRE
        | Channels::REAR_LEFT
        | Channels::REAR_RIGHT
    }
    6 => {
      Channels::FRONT_LEFT
        | Channels::FRONT_RIGHT
        | Channels::FRONT_CENTRE
        | Channels::LFE1
        | Channels::REAR_LEFT
        | Channels::REAR_RIGHT
    }
    7 => {
      Channels::FRONT_LEFT
        | Channels::FRONT_RIGHT
        | Channels::FRONT_CENTRE
        | Channels::LFE1
        | Channels::REAR_CENTRE
        | Channels::SIDE_LEFT
        | Channels::SIDE_RIGHT
    }
    8 => {
      Channels::FRONT_LEFT
        | Channels::FRONT_RIGHT
        | Channels::FRONT_CENTRE
        | Channels::LFE1
        | Channels::REAR_LEFT
        | Channels::REAR_RIGHT
        | Channels::SIDE_LEFT
        | Channels::SIDE_RIGHT
    }
    _ => return decode_error("flac: stream channels are out of bounds"),
  };

  Ok(layout)
}

#[derive(Debug, Default)]
pub struct StreamInfo {
  pub block_len_min: u16,
  pub block_len_max: u16,
  pub frame_byte_len_min: u32,
  pub frame_byte_len_max: u32,
  pub sample_rate: u32,
  pub channels: Channels,
  pub bits_per_sample: u32,
  pub n_samples: Option<u64>,
  pub md5: Option<[u8; 16]>,
}

impl StreamInfo {
  pub fn read<B: ReadBytes>(reader: &mut B) -> Result<StreamInfo> {
    let block_len_min = reader.read_be_u16()?;
    let block_len_max = reader.read_be_u16()?;

    match (block_len_min, block_len_max) {
      (0..=15, _) | (_, 0..=15) => {
        return decode_error("flac: minimum block length is 16 samples");
      }
      _ => {}
    }

    match block_len_max.cmp(&block_len_min) {
      Ordering::Less => {
        return decode_error("flac: maximum block length is less than the minimum block length");
      }
      Ordering::Equal | Ordering::Greater => {}
    }

    let frame_byte_len_min = reader.read_be_u24()?;
    let frame_byte_len_max = reader.read_be_u24()?;

    match (frame_byte_len_min, frame_byte_len_max) {
      (0, _) | (_, 0) => {}
      (least, most) => match most.cmp(&least) {
        Ordering::Less => {
          return decode_error(
            "flac: maximum frame length is less than the minimum frame length",
          );
        }
        Ordering::Equal | Ordering::Greater => {}
      },
    }

    let Ok(mut br) = BitStreamLtr::new(reader);

    let sample_rate = br.read_bits_leq32(20)?;

    match sample_rate {
      1..=655_350 => {}
      _ => return decode_error("flac: stream sample rate out of bounds"),
    }

    let channels_enc = br.read_bits_leq32(3)?;

    let channels = flac_channels_to_channels(channels_enc.saturating_add(1))?;

    let bits_enc = br.read_bits_leq32(5)?;
    let bits_per_sample = bits_enc.saturating_add(1);

    match bits_per_sample {
      4..=32 => {}
      _ => return decode_error("flac: stream bits per sample are out of bounds"),
    }

    let total_samples = br.read_bits_leq64(36)?;

    let n_samples = match total_samples {
      0 => None,
      samples => Some(samples),
    };

    let mut md5 = [0; 16];
    reader.read_buf_exact(&mut md5)?;

    let checksum = match md5 {
      [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] => None,
      present => Some(present),
    };

    Ok(StreamInfo {
      block_len_min,
      block_len_max,
      frame_byte_len_min,
      frame_byte_len_max,
      sample_rate,
      channels,
      bits_per_sample,
      n_samples,
      md5: checksum,
    })
  }

  pub const BLOCK_SIZE: u64 = 34;
}

pub fn read_seek_table_block<B: ReadBytes>(
  reader: &mut B,
  block_length: u32,
  table: &mut SeekIndex,
) -> Result<()> {
  let count = block_length.div_euclid(18);

  for _ in 0..count {
    let sample = reader.read_be_u64()?;

    match sample {
      0xffff_ffff_ffff_ffff => reader.ignore_bytes(10)?,
      first_sample => {
        let ts = reader.read_be_u64()?;
        let in_frame = reader.read_be_u16()?;
        let Ok(()) = table.insert(first_sample, ts, u32::from(in_frame));
      }
    }
  }

  Ok(())
}

fn printable_ascii_to_string(bytes: &[u8], fault: &'static str) -> Result<String> {
  let mut result = String::with_capacity(bytes.len());

  for c in bytes {
    match c {
      0x00 => break,
      0x20..=0x7e => result.push(char::from(*c)),
      _ => return decode_error(fault),
    }
  }

  Ok(result)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Disc {
  CompactDisc,
  Other,
}

const SECTOR: u64 = 588;

pub fn read_cuesheet_block<B: ReadBytes>(reader: &mut B, cues: &mut Vec<Cue>) -> Result<()> {
  let mut catalog_number_buf = [0u8; 128];
  reader.read_buf_exact(&mut catalog_number_buf)?;

  printable_ascii_to_string(
    &catalog_number_buf,
    "flac: cuesheet catalog number contains invalid characters",
  )?;

  let n_lead_in_samples = reader.read_be_u64()?;

  let kind = reader.read_u8()?;

  let disc = match kind & 0x80 {
    0x80 => Disc::CompactDisc,
    _ => Disc::Other,
  };

  match (disc, n_lead_in_samples) {
    (Disc::Other, 1..) => {
      return decode_error("flac: cuesheet lead-in samples should be zero if not CD-DA");
    }
    _ => {}
  }

  for _ in 0..129 {
    let reserved = reader.read_be_u16()?;

    match reserved {
      0 => {}
      _ => return decode_error("flac: cuesheet reserved bits should be zero"),
    }
  }

  let n_tracks = reader.read_u8()?;

  match (disc, n_tracks) {
    (_, 0) => return decode_error("flac: cuesheet must have at-least one track"),
    (Disc::CompactDisc, 101..) => {
      return decode_error("flac: cuesheets for CD-DA must not have more than 100 tracks");
    }
    _ => {}
  }

  for _ in 0..n_tracks {
    read_cuesheet_track(reader, disc, cues)?;
  }

  Ok(())
}

fn read_cuesheet_track<B: ReadBytes>(
  reader: &mut B,
  disc: Disc,
  cues: &mut Vec<Cue>,
) -> Result<()> {
  let n_offset_samples = reader.read_be_u64()?;

  match (disc, n_offset_samples.rem_euclid(SECTOR)) {
    (Disc::CompactDisc, 1..) => {
      return decode_error(
        "flac: cuesheet track sample offset is not a multiple of 588 for CD-DA",
      );
    }
    _ => {}
  }

  let number_byte = reader.read_u8()?;
  let number = u32::from(number_byte);

  match (disc, number) {
    (_, 0) => return decode_error("flac: cuesheet track number of 0 not allowed"),
    (Disc::CompactDisc, 100..=169 | 171..) => {
      return decode_error(
        "flac: cuesheet track numbers greater than 99 are not allowed for CD-DA",
      );
    }
    _ => {}
  }

  let mut isrc_buf = [0u8; 12];
  reader.read_buf_exact(&mut isrc_buf)?;

  let isrc = printable_ascii_to_string(
    &isrc_buf,
    "flac: cuesheet track ISRC contains invalid characters",
  )?;

  let flags = reader.read_be_u16()?;

  match flags & 0x3fff {
    0 => {}
    _ => return decode_error("flac: cuesheet track reserved bits should be zero"),
  }

  for _ in 0..3 {
    let reserved = reader.read_be_u32()?;

    match reserved {
      0 => {}
      _ => return decode_error("flac: cuesheet track reserved bits should be zero"),
    }
  }

  let n_indicies = reader.read_u8()?;

  match (disc, n_indicies) {
    (Disc::CompactDisc, 101..) => {
      return decode_error("flac: cuesheet track indicies cannot exceed 100 for CD-DA");
    }
    _ => {}
  }

  let mut cue = Cue {
    index: number,
    start_ts: n_offset_samples,
    tags: Vec::new(),
    points: Vec::new(),
  };

  let Ok(value_1) = Tag::new(Some(StandardTagKey::IdentIsrc), "ISRC", Value::from(isrc));

  cue.tags.push(value_1);

  for _ in 0..n_indicies {
    let point = read_cuesheet_track_index(reader, disc)?;
    cue.points.push(point);
  }

  cues.push(cue);

  Ok(())
}

fn read_cuesheet_track_index<B: ReadBytes>(reader: &mut B, disc: Disc) -> Result<CuePoint> {
  let n_offset_samples = reader.read_be_u64()?;
  let idx_point_enc = reader.read_be_u32()?;

  match (disc, n_offset_samples.rem_euclid(SECTOR)) {
    (Disc::CompactDisc, 1..) => {
      return decode_error(
        "flac: cuesheet track index point sample offset is not a multiple of 588 for CD-DA",
      );
    }
    _ => {}
  }

  match idx_point_enc & 0x00ff_ffff {
    0 => {}
    _ => return decode_error("flac: cuesheet track index reserved bits should be 0"),
  }

  Ok(CuePoint {
    start_offset_ts: n_offset_samples,
    tags: Vec::new(),
  })
}

#[derive(Clone, Debug)]
pub struct ApplicationData {
  pub ident: String,
  pub data: Box<[u8]>,
}

pub fn read_application_block<B: ReadBytes>(
  reader: &mut B,
  block_length: u32,
) -> Result<ApplicationData> {
  let ident_buf = reader.read_quad_bytes()?;
  let ident: String = ident_buf
    .iter()
    .copied()
    .flat_map(ascii::escape_default)
    .map(char::from)
    .collect();

  let data_len = match block_length.checked_sub(4) {
    Some(remaining) => remaining,
    None => return decode_error("flac: application block is shorter than its identifier"),
  };

  let data = reader.read_boxed_slice_exact(u64::from(data_len))?;
  Ok(ApplicationData { ident, data })
}

pub use console_core_audio_decoding_tags::flac::read_comment_block;
pub use console_core_audio_decoding_tags::flac::read_picture_block;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataBlockBoundary {
  MoreBlocks,
  LastBlock,
}

pub struct MetadataBlockHeader {
  pub boundary: MetadataBlockBoundary,
  pub block_type: MetadataBlockType,
  pub block_len: u32,
}

impl MetadataBlockHeader {
  pub fn read<B: ReadBytes>(reader: &mut B) -> Result<MetadataBlockHeader> {
    let header_enc = reader.read_u8()?;

    let boundary = match header_enc & 0x80 {
      0x80 => MetadataBlockBoundary::LastBlock,
      _clear => MetadataBlockBoundary::MoreBlocks,
    };

    let block_type_id = header_enc & 0x7f;

    let block_type = match block_type_id {
      0 => MetadataBlockType::StreamInfo,
      1 => MetadataBlockType::Padding,
      2 => MetadataBlockType::Application,
      3 => MetadataBlockType::SeekTable,
      4 => MetadataBlockType::VorbisComment,
      5 => MetadataBlockType::Cuesheet,
      6 => MetadataBlockType::Picture,
      _ => MetadataBlockType::Unknown(block_type_id),
    };

    let block_len = reader.read_be_u24()?;

    Ok(MetadataBlockHeader {
      boundary,
      block_type,
      block_len,
    })
  }
}
