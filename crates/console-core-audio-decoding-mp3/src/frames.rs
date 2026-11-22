//! Reading whole frames out of a stream that may have junk between them.
//!
//! A frame header is eleven set bits and a handful of fields, which random
//! data produces often enough that finding one is not finding a frame. A frame
//! is believed when the bytes after it are the header of one like it, and
//! until then the stream is walked on from the second byte of what was
//! rejected, so that the same junk is not synced to twice.

use console_core_audio_decoding_interfaces::errors::decode_error;
use console_core_audio_decoding_interfaces::errors::{Error, Result};
use console_core_audio_decoding_interfaces::io::{
  MediaSource, MediaSourceStream, ReadBytes, SeekBuffered,
};
use console_core_iteration::{Endless, Step, iterate};
use console_core_mpeg_audio_frames::header::{
  MPEG_HEADER_LEN, parse_candidate, parse_frame_header, synced,
};
use console_core_number_conversion::{Float, index, toward_zero_u64};

use crate::common::{FrameHeader, Infallible, Protection, Version, finished};
use crate::header::{MAX_MPEG_FRAME_SIZE, read_frame_header_word_no_sync, sync_frame};

const MAX_FRAMES: u32 = 16;
const MAX_LEN: u64 = 16 * 1024;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Following {
  Similar,
  Foreign,
}

#[derive(Copy, Clone, Default)]
struct Tally {
  len: u64,
  frames: u32,
}

fn parsed(reader: &mut MediaSourceStream) -> Result<Option<(FrameHeader, u32)>> {
  let word = sync_frame(reader)?;
  let candidate = parse_candidate(word)?;

  Ok(candidate.map(|header| (header, word)))
}

pub fn read_mpeg_frame(reader: &mut MediaSourceStream) -> Result<(FrameHeader, Vec<u8>)> {
  let found = iterate(&mut *reader, |reader_2| {
    Ok(match parsed(reader_2) {
      Err(fault) => Step::Halt(Err(fault)),
      Ok(None) => Step::Again(reader_2),
      Ok(Some(found)) => Step::Halt(Ok(found)),
    })
  });

  let (header, word) = finished(found)?;

  match ::core::primitive::bool::then_some(u64::from(header.frame_size).saturating_add(u64::from(MPEG_HEADER_LEN))
    <= MAX_MPEG_FRAME_SIZE, ())
  {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return decode_error("mpa: frame exceeds the maximum MPEG audio packet size"),
  }

  let Ok(body_len) = index(header.frame_size);
  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(
      explicit088_no_arriving_length,
      reason = "the complete MPEG audio packet, including its header, was checked against MAX_MPEG_FRAME_SIZE before allocating its body"
    )
  )]
  let mut body = vec![0u8; body_len];

  reader.read_buf_exact(&mut body)?;

  let mut packet = word.to_be_bytes().to_vec();

  packet.extend(body);

  Ok((header, packet))
}

fn following(header: &FrameHeader, word: u32) -> Result<Following> {
  let Ok(found) = synced(word);

  let candidate = match found {
    Some(word_2) => parse_candidate(word_2)?,
    None => None,
  };

  Ok(match candidate {
    Some(candidate_2) => {
      let similar = header.version == candidate_2.version
        && header.layer == candidate_2.layer
        && header.sample_rate == candidate_2.sample_rate
        && header.channels == candidate_2.channels;

      match ::core::primitive::bool::then_some(similar, ()) {
        ::core::option::Option::Some(()) => Following::Similar,
        ::core::option::Option::None => Following::Foreign,
      }
    }
    None => Following::Foreign,
  })
}

fn strict_attempt(reader: &mut MediaSourceStream) -> Result<Option<(FrameHeader, Vec<u8>)>> {
  let (header, packet) = read_mpeg_frame(reader)?;
  let pos = reader.pos();

  let verdict = match read_frame_header_word_no_sync(reader) {
    Ok(word) => following(&header, word),
    Err(_ended) => Ok(Following::Similar),
  };

  let verdict_2 = verdict?;

  match verdict_2 {
    Following::Foreign => {
      let rewind = header
        .frame_size
        .saturating_add(MPEG_HEADER_LEN)
        .saturating_add(MPEG_HEADER_LEN)
        .saturating_sub(1);
      let Ok(()) = reader.seek_buffered_rev(u64::from(rewind));

      Ok(None)
    }
    Following::Similar => {
      reader.seek_buffered(pos);

      Ok(Some((header, packet)))
    }
  }
}

pub fn read_mpeg_frame_strict(reader: &mut MediaSourceStream) -> Result<(FrameHeader, Vec<u8>)> {
  let accepted = iterate(&mut *reader, |reader_2| {
    Ok(match strict_attempt(reader_2) {
      Err(fault) => Step::Halt(Err(fault)),
      Ok(None) => Step::Again(reader_2),
      Ok(Some(frame)) => Step::Halt(Ok(frame)),
    })
  });

  finished(accepted)
}

pub fn read_main_data_begin<B: ReadBytes>(reader: &mut B, header: &FrameHeader) -> Result<u16> {
  let crc = match header.protection {
    Protection::Crc => reader.read_be_u16().map(|_crc| ()),
    Protection::None => Ok(()),
  };

  crc?;

  let begin = match header.version {
    Version::Mpeg1 => reader.read_be_u16().map(|word| word.wrapping_shr(7)),
    Version::Mpeg2 | Version::Mpeg2p5 => reader.read_u8().map(u16::from),
  };

  begin.map_err(Error::from)
}

fn tabulated(reader: &mut MediaSourceStream) -> Result<u64> {
  let word = reader.read_be_u32()?;
  let header = parse_frame_header(word)?;

  reader.ignore_bytes(u64::from(header.frame_size))?;

  Ok(u64::from(header.frame_size.saturating_add(MPEG_HEADER_LEN)))
}

pub fn estimate_num_mpeg_frames(reader: &mut MediaSourceStream) -> Infallible<Option<u64>> {
  let start_pos = reader.pos();

  let total_len = reader.byte_len().map(|len| len.saturating_sub(start_pos));

  let counted = iterate((&mut *reader, Tally::default()), |(reader_2, tally)| {
    Ok(match tabulated(reader_2) {
      Err(_unreadable) => Step::Halt(None),
      Ok(frame_len) => {
        let tally_2 = Tally {
          len: tally.len.saturating_add(frame_len),
          frames: tally.frames.saturating_add(1),
        };

        match ::core::primitive::bool::then_some(tally_2.frames > MAX_FRAMES || tally_2.len > MAX_LEN, ()) {
          ::core::option::Option::Some(()) => Step::Halt(Some(tally_2)),
          ::core::option::Option::None => Step::Again((reader_2, tally_2)),
        }
      }
    })
  });

  let back = reader.pos().saturating_sub(start_pos);
  let Ok(()) = reader.seek_buffered_rev(back);

  Ok(match (counted, total_len) {
    (Ok(Some(tally)), Some(total_len_2)) => {
      let Ok(len) = tally.len.float();
      let average = len / f64::from(tally.frames);
      let Ok(total) = total_len_2.float();
      let Ok(estimate) = toward_zero_u64(total / average);

      Some(estimate)
    }
    (Ok(None) | Err(Endless), _) | (Ok(Some(_)), None) => None,
  })
}
