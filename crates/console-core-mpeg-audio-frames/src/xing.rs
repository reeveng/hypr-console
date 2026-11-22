//! The tags an encoder leaves in the first frame of a stream.
//!
//! A variable rate stream cannot say how long it is from the size of one
//! frame, so an encoder writes a frame at the front that holds no audio and
//! says it instead: Xing or Info, with the count of frames and, from LAME and
//! the encoders that follow it, how many samples the encoder and the decoder
//! add at the start and the end, or VBRI from Fraunhofer. A frame that looks
//! like one is only a candidate until it has been read, and nothing in a tag
//! that is not understood is an error, since the frame is then just a frame.

use std::cmp::Ordering;

use console_core_audio_decoding_interfaces::errors::{Error, Result};
use console_core_byte_streams::{BufReader, FiniteStream, MonitorStream, ReadBytes};
use console_core_checksums::Checksum;
use console_core_checksums::crc16::Crc16AnsiLe;
use console_core_number_conversion::index;

use crate::Infallible;
use crate::header::{FrameHeader, MPEG_HEADER_LEN, Protection};

const XING_TAG_ID: [u8; 4] = *b"Xing";
const INFO_TAG_ID: [u8; 4] = *b"Info";
const VBRI_TAG_ID: [u8; 4] = *b"VBRI";

const MIN_XING_TAG_LEN: u32 = 8;
const MIN_VBRI_TAG_LEN: u32 = 26;
const VBRI_TAG_OFFSET: u32 = 36;

const LAME_EXT_LEN: u64 = 36;
const MIN_LAME_EXT_LEN: u64 = 24;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Contents {
  Tag,
  Audio,
}

pub struct LameTag {
  pub enc_delay: u32,
  pub enc_padding: u32,
}

pub struct XingInfoTag {
  pub num_frames: Option<u32>,
  pub num_bytes: Option<u32>,
  pub lame: Option<LameTag>,
}

pub struct VbriTag {
  pub num_mpeg_frames: u32,
  pub num_bytes: u32,
}

#[derive(Clone, Copy)]
enum Encoder {
  Lame,
  Lavf,
  Lavc,
  Other,
}

impl From<[u8; 9]> for Encoder {
  fn from(bytes: [u8; 9]) -> Self {
    match bytes {
      [b'L', b'A', b'M', b'E', ..] => Self::Lame,
      [b'L', b'a', b'v', b'f', ..] => Self::Lavf,
      [b'L', b'a', b'v', b'c', ..] => Self::Lavc,
      _other => Self::Other,
    }
  }
}

type Monitored<'a> = MonitorStream<BufReader<'a>, Crc16AnsiLe>;

fn info_candidate<'a>(
  buf: &'a [u8],
  header: &FrameHeader,
) -> Infallible<Option<(&'a [u8], &'a [u8])>> {
  let Ok(offset) = index(header.side_info_len.saturating_add(MPEG_HEADER_LEN));

  let Ok(shortest) = index(MIN_XING_TAG_LEN);

  Ok(match buf.split_at_checked(offset) {
    Some((head, body)) => {
      let id = body.first_chunk::<4>().copied();
      let nonzero = head.iter().skip(4).find(|byte| **byte != 0);

      match (id, body.len().cmp(&shortest), nonzero) {
        (Some(XING_TAG_ID | INFO_TAG_ID), Ordering::Equal | Ordering::Greater, None) => {
          Some((head, body))
        }
        _audio => None,
      }
    }
    None => None,
  })
}

fn vbri_candidate(buf: &[u8]) -> Infallible<Option<&[u8]>> {
  let Ok(offset) = index(VBRI_TAG_OFFSET);
  let Ok(shortest) = index(MIN_VBRI_TAG_LEN);

  Ok(match buf.split_at_checked(offset) {
    Some((head, body)) => {
      let id = body.first_chunk::<4>().copied();
      let nonzero = head.iter().skip(4).find(|byte| **byte != 0);

      match (id, body.len().cmp(&shortest), nonzero) {
        (Some(VBRI_TAG_ID), Ordering::Equal | Ordering::Greater, None) => Some(body),
        _audio => None,
      }
    }
    None => None,
  })
}

pub fn contents(buf: &[u8], header: &FrameHeader) -> Infallible<Contents> {
  let Ok(info) = read_info_tag(buf, header);
  let Ok(vbri) = read_vbri_tag(buf, header);

  Ok(match (info, vbri) {
    (Some(_), _) | (_, Some(_)) => Contents::Tag,
    (None, None) => Contents::Audio,
  })
}

fn optional<T>(
  reader: &mut Monitored<'_>,
  masked: u32,
  read: fn(&mut Monitored<'_>) -> std::io::Result<T>,
) -> Result<Option<T>> {
  match masked {
    0 => Ok(None),
    _ => read(reader).map(Some).map_err(Error::from),
  }
}

fn tolerated<T>(read: Result<Option<T>>) -> Infallible<Option<T>> {
  Ok(match read {
    Ok(read_2) => read_2,
    Err(
      Error::IoError(_)
      | Error::DecodeError(_)
      | Error::SeekError(_)
      | Error::Unsupported(_)
      | Error::LimitError(_)
      | Error::ResetRequired,
    ) => None,
  })
}

fn tag_crc(
  reader: &mut Monitored<'_>,
  header: &FrameHeader,
  encoder: Encoder,
) -> Result<Option<u16>> {
  let Ok(inner) = reader.inner();
  let required = LAME_EXT_LEN.saturating_sub(MIN_LAME_EXT_LEN);

  match inner.bytes_available().cmp(&required) {
    Ordering::Less => Ok(None),
    Ordering::Equal | Ordering::Greater => {
      let mut skipped = [0u8; 10];

      reader.read_buf_exact(&mut skipped)?;

      match (header.protection, encoder) {
        (Protection::Crc, _) | (Protection::None, Encoder::Lame) => {
          let Ok(inner_2) = reader.inner_mut();

          inner_2.read_be_u16().map(Some).map_err(Error::from)
        }
        (Protection::None, Encoder::Lavf | Encoder::Lavc | Encoder::Other) => Ok(None),
      }
    }
  }
}

fn read_lame(reader: &mut Monitored<'_>, header: &FrameHeader) -> Result<Option<LameTag>> {
  let Ok(inner) = reader.inner();

  match inner.bytes_available().cmp(&MIN_LAME_EXT_LEN) {
    Ordering::Less => Ok(None),
    Ordering::Equal | Ordering::Greater => {
      let mut encoder = [0u8; 9];
      let mut skipped = [0u8; 12];

      reader.read_buf_exact(&mut encoder)?;
      reader.read_buf_exact(&mut skipped)?;

      let trim = reader.read_be_u24()?;

      let encoder_kind = Encoder::from(encoder);
      let (enc_delay, enc_padding) = match encoder_kind {
        Encoder::Lame | Encoder::Lavf | Encoder::Lavc => (
          529u32.saturating_add(trim.wrapping_shr(12)),
          (trim & 0xfff).saturating_sub(529),
        ),
        Encoder::Other => (0, 0),
      };

      let expected = tag_crc(reader, header, encoder_kind)?;
      let Ok(monitor) = reader.monitor();
      let Ok(actual) = monitor.crc();

      Ok(match expected.map(|checksum| checksum.cmp(&actual)) {
        Some(Ordering::Equal) | None => Some(LameTag {
          enc_delay,
          enc_padding,
        }),
        Some(Ordering::Less | Ordering::Greater) => None,
      })
    }
  }
}

fn parse_info_tag(head: &[u8], body: &[u8], header: &FrameHeader) -> Result<XingInfoTag> {
  let Ok(mut crc16) = Crc16AnsiLe::new(0);
  let Ok(()) = crc16.process_buf_bytes(head);

  let Ok(body_reader) = BufReader::new(body);
  let Ok(mut reader) = MonitorStream::new(body_reader, crc16);

  let mut id = [0u8; 4];

  reader.read_buf_exact(&mut id)?;

  let flags = reader.read_be_u32()?;

  let num_frames = optional(&mut reader, flags & 0x1, |reader_2| reader_2.read_be_u32())?;
  let num_bytes = optional(&mut reader, flags & 0x2, |reader_3| reader_3.read_be_u32())?;
  let _toc = optional(&mut reader, flags & 0x4, |reader_4| {
    reader_4.read_boxed_slice_exact(100)
  })?;
  let _quality = optional(&mut reader, flags & 0x8, |reader_5| reader_5.read_be_u32())?;

  let lame = read_lame(&mut reader, header)?;

  Ok(XingInfoTag {
    num_frames,
    num_bytes,
    lame,
  })
}

pub fn read_info_tag(buf: &[u8], header: &FrameHeader) -> Infallible<Option<XingInfoTag>> {
  let Ok(candidate) = info_candidate(buf, header);

  match candidate {
    Some((head, body)) => tolerated(parse_info_tag(head, body, header).map(Some)),
    None => Ok(None),
  }
}

fn parse_vbri_tag(body: &[u8]) -> Result<Option<VbriTag>> {
  let Ok(mut reader) = BufReader::new(body);

  let id = reader.read_quad_bytes()?;
  let version = reader.read_be_u16()?;

  match (id, version) {
    (VBRI_TAG_ID, 1) => {
      let _delay = reader.read_be_u16()?;
      let _quality = reader.read_be_u16()?;
      let num_bytes = reader.read_be_u32()?;
      let num_mpeg_frames = reader.read_be_u32()?;

      Ok(Some(VbriTag {
        num_mpeg_frames,
        num_bytes,
      }))
    }
    (_, _) => Ok(None),
  }
}

pub fn read_vbri_tag(buf: &[u8], _header: &FrameHeader) -> Infallible<Option<VbriTag>> {
  let Ok(candidate) = vbri_candidate(buf);

  match candidate {
    Some(body) => tolerated(parse_vbri_tag(body)),
    None => Ok(None),
  }
}
