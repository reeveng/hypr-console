//! The identification and comment packets, followed by the final page's sample clock.

use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::time::Duration;

use console_core_number_conversion::{fitted, index};
use console_core_ogg_pages::{Packets, Page, PageError, PageHeader};

use crate::Contents;
use crate::bytes::{Cursor, length, read_into};
use crate::fault::{Malformed, ParseError};

#[derive(Clone, Copy)]
enum Codec {
  Opus,
  Vorbis,
}

fn final_page<R: Read + Seek>(reader: &mut R) -> Result<Option<PageHeader>, ParseError> {
  let end_of_source = reader.seek(SeekFrom::End(0))?;
  let mut end = end_of_source;
  let mut window = [0u8; 65_536];
  let Ok(width) = length(&window);
  let step = width.saturating_sub(4).max(1);

  'windows: for _window in 0..end_of_source.wrapping_div(step).saturating_add(1) {
    let start = end.saturating_sub(width);
    let Ok(size) = index(end.saturating_sub(start));
    let bytes = window.get_mut(..size).ok_or(Malformed::Short)?;

    reader.seek(SeekFrom::Start(start))?;
    read_into(reader, bytes)?;

    'captures: for (offset, capture) in bytes.windows(4).enumerate().rev() {
      match capture {
        b"OggS" => {}
        _not_ogg => continue 'captures,
      }

      let Ok(offset_2) = fitted::<_, u64>(offset);

      reader.seek(SeekFrom::Start(start.saturating_add(offset_2)))?;

      match Page::read(reader) {
        Ok(page) => return Ok(Some(page.header)),
        Err(PageError::Io(fault)) => {
          match bool::then_some(fault.kind() == ErrorKind::UnexpectedEof, ()) {
            Some(()) => continue 'captures,
            None => return Err(ParseError::Read(fault)),
          }
        }
        Err(
          PageError::MissingCapturePattern
          | PageError::UnknownVersion
          | PageError::NoSegments
          | PageError::TooFewPackets
          | PageError::NoSuchPacket,
        ) => continue 'captures,
      }
    }

    match start {
      0 => break 'windows,
      _earlier => end = start.saturating_add(3),
    }
  }

  Ok(None)
}

fn read<R: Read + Seek>(reader: &mut R, codec: Codec) -> Result<Contents, ParseError> {
  reader.seek(SeekFrom::Start(0))?;

  let first = PageHeader::read(reader)?;

  reader.seek(SeekFrom::Start(0))?;

  let packets = Packets::read_count(reader, 2)?;
  let Ok(head) = packets.get(0);
  let Ok(comments) = packets.get(1);
  let head_2 = head.ok_or(Malformed::Signature)?;
  let comments_2 = comments.ok_or(Malformed::Signature)?;
  let (rate, skip, comments_5) = match codec {
    Codec::Opus => {
      let fields = head_2
        .strip_prefix(b"OpusHead")
        .ok_or(Malformed::Signature)?;
      let comments_3 = comments_2
        .strip_prefix(b"OpusTags")
        .ok_or(Malformed::Signature)?;
      let Ok(mut fields_2) = Cursor::new(fields);
      let _version = fields_2.byte()?;
      let channels = fields_2.byte()?;
      let skip = fields_2.u16_le()?;

      match channels {
        0 => return Err(ParseError::Parse(Malformed::Properties)),
        _channels => {}
      }

      (48_000u64, u64::from(skip), comments_3)
    }
    Codec::Vorbis => {
      let fields = head_2
        .strip_prefix(b"\x01vorbis")
        .ok_or(Malformed::Signature)?;
      let comments_4 = comments_2
        .strip_prefix(b"\x03vorbis")
        .ok_or(Malformed::Signature)?;
      let Ok(mut fields_3) = Cursor::new(fields);
      let _version = fields_3.u32_le()?;
      let _channels = fields_3.byte()?;
      let rate = fields_3.u32_le()?;

      (u64::from(rate), 0, comments_4)
    }
  };

  let tag = crate::vorbis_comments::read(comments_5)?;
  let value_1 = final_page(reader)?;

  let duration = match value_1 {
    Some(last) => {
      let samples = last
        .granule_position
        .saturating_sub(first.granule_position)
        .saturating_sub(skip);

      match (samples, rate) {
        (0, _) | (_, 0) => Ok(Duration::ZERO),
        (_, _) => crate::duration::rounded(crate::duration::Interval {
          units: samples,
          per_second: rate,
        }),
      }
    }
    None => Ok(Duration::ZERO),
  };
  let duration_2 = duration?;

  Ok(Contents {
    duration: duration_2,
    tags: vec![tag],
  })
}

pub(crate) fn read_opus<R: Read + Seek>(reader: &mut R) -> Result<Contents, ParseError> {
  read(reader, Codec::Opus)
}

pub(crate) fn read_vorbis<R: Read + Seek>(reader: &mut R) -> Result<Contents, ParseError> {
  read(reader, Codec::Vorbis)
}
