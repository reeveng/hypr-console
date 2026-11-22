//! Find and read MPEG audio headers from the decoder's byte stream.

use console_core_audio_decoding_interfaces::errors::{Error, Result};
use console_core_byte_streams::ReadBytes;
use console_core_iteration::{Step, iterate};
use console_core_mpeg_audio_frames::header::{FrameHeader, parse_frame_header, synced};

use crate::common::finished;

pub const MAX_MPEG_FRAME_SIZE: u64 = 2881;

pub fn sync_frame<B: ReadBytes>(reader: &mut B) -> Result<u32> {
  let found = iterate((reader, 0u32), |(reader_2, sync)| {
    Ok(match reader_2.read_u8() {
      Err(fault) => Step::Halt(Err(Error::from(fault))),
      Ok(byte) => {
        let word = sync.wrapping_shl(8) | u32::from(byte);
        let Ok(found) = synced(word);

        match found {
          Some(word_2) => Step::Halt(Ok(word_2)),
          None => Step::Again((reader_2, word)),
        }
      }
    })
  });

  finished(found)
}

pub fn read_frame_header<B: ReadBytes>(reader: &mut B) -> Result<FrameHeader> {
  let word = sync_frame(reader)?;

  parse_frame_header(word)
}

pub fn read_frame_header_word_no_sync<B: ReadBytes>(reader: &mut B) -> Result<u32> {
  reader.read_be_u32().map_err(Error::from)
}
