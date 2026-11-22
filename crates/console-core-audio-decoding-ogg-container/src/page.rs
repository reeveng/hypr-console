//! Pages as a demuxer meets them: found by scanning, checked by the CRC they carry, and cut into
//! the packets their lacing spells.
//!
//! A page that fails its CRC is not an error that ends the stream. The reader goes back to just
//! after the capture pattern it matched and scans from there, because the pattern may have been
//! four bytes of something else, and a real page may start a few bytes on.

use std::cmp::Ordering;

use console_core_audio_decoding_interfaces::errors::{Error, Result, decode_error};
use console_core_byte_streams::{MonitorStream, ReadBytes, SeekBuffered};
use console_core_checksums::Checksum;
use console_core_checksums::ogg::Crc32;
use console_core_iteration::Step;
use console_core_number_conversion::index;

use crate::stepping::{Certain, Stepped, walk};

const OGG_PAGE_MARKER: [u8; 4] = *b"OggS";

const OGG_PAGE_BODY_MAX: u64 = 255 * 255;

pub const OGG_PAGE_MAX_SIZE: u64 = 27 + 255 + OGG_PAGE_BODY_MAX;

#[derive(Copy, Clone, Default)]
pub enum PacketStart {
  #[default]
  Fresh,
  Continued,
}

#[derive(Copy, Clone, Default)]
pub enum StreamStart {
  Beginning,
  #[default]
  Later,
}

#[derive(Copy, Clone, Default)]
pub enum StreamEnd {
  Final,
  #[default]
  MorePages,
}

#[derive(Copy, Clone, Default)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "the three flags are three bits of the header-type byte, and a page may carry any combination of them"
  )
)]
pub struct PageHeader {
  pub absgp: u64,
  pub serial: u32,
  pub sequence: u32,
  pub crc: u32,
  pub n_segments: u8,
  pub packet_start: PacketStart,
  pub stream_start: StreamStart,
  pub stream_end: StreamEnd,
}

fn read_page_header(fields: [u8; 23]) -> Result<(PageHeader, [u8; 27])> {
  let [
    version,
    flags,
    g0,
    g1,
    g2,
    g3,
    g4,
    g5,
    g6,
    g7,
    s0,
    s1,
    s2,
    s3,
    q0,
    q1,
    q2,
    q3,
    c0,
    c1,
    c2,
    c3,
    n_segments,
  ] = fields;

  match version {
    0 => {}
    _unknown => return decode_error("ogg: invalid ogg version"),
  }

  match flags & 0xf8 {
    0 => {}
    _reserved => return decode_error("ogg: invalid flag bits set"),
  }

  let header = PageHeader {
    absgp: u64::from_le_bytes([g0, g1, g2, g3, g4, g5, g6, g7]),
    serial: u32::from_le_bytes([s0, s1, s2, s3]),
    sequence: u32::from_le_bytes([q0, q1, q2, q3]),
    crc: u32::from_le_bytes([c0, c1, c2, c3]),
    n_segments,
    packet_start: match flags & 0x01 {
      0 => PacketStart::Fresh,
      _set => PacketStart::Continued,
    },
    stream_start: match flags & 0x02 {
      0 => StreamStart::Later,
      _set => StreamStart::Beginning,
    },
    stream_end: match flags & 0x04 {
      0 => StreamEnd::MorePages,
      _set => StreamEnd::Final,
    },
  };

  let [m0, m1, m2, m3] = OGG_PAGE_MARKER;
  let unchecked = [
    m0, m1, m2, m3, version, flags, g0, g1, g2, g3, g4, g5, g6, g7, s0, s1, s2, s3, q0, q1, q2,
    q3, 0, 0, 0, 0, n_segments,
  ];

  Ok((header, unchecked))
}

fn sync_page<B: ReadBytes>(reader: &mut B) -> Result<()> {
  let first = reader.read_quad_bytes()?;

  walk((reader, u32::from_be_bytes(first)), sync_step)
}

fn sync_step<B: ReadBytes>((reader, marker): (&mut B, u32)) -> Stepped<(&mut B, u32), ()> {
  Ok(match marker.to_be_bytes() {
    OGG_PAGE_MARKER => Step::Halt(Ok(())),
    _searching => match reader.read_u8() {
      Ok(byte) => Step::Again((reader, marker.wrapping_shl(8) | u32::from(byte))),
      Err(fault) => Step::Halt(Err(Error::from(fault))),
    },
  })
}

pub struct PagePackets<'a> {
  lens: std::slice::Iter<'a, u16>,
  data: &'a [u8],
}

impl<'a> PagePackets<'a> {
  pub fn partial_packet(self) -> Certain<Option<&'a [u8]>> {
    let discard = self
      .lens
      .fold(0u16, |total, len| total.saturating_add(*len));
    let Ok(from) = index(discard);

    Ok(match self.data.get(from..) {
      Some([]) | None => None,
      Some(rest) => Some(rest),
    })
  }
}

impl<'a> Iterator for PagePackets<'a> {
  type Item = &'a [u8];

  fn next(&mut self) -> Option<Self::Item> {
    let len = self.lens.next()?;
    let Ok(at) = index(*len);
    let (packet, rest) = self.data.split_at_checked(at)?;

    self.data = rest;

    Some(packet)
  }
}

pub struct Page<'a> {
  pub header: PageHeader,
  packet_lens: &'a [u16],
  page_buf: &'a [u8],
}

impl Page<'_> {
  pub fn packets(&self) -> Certain<PagePackets<'_>> {
    Ok(PagePackets {
      lens: self.packet_lens.iter(),
      data: self.page_buf,
    })
  }
}

pub struct PageReader {
  header: PageHeader,
  packet_lens: Vec<u16>,
  page_buf: Vec<u8>,
}

impl PageReader {
  pub fn try_new<B>(reader: &mut B) -> Result<Self>
  where
    B: ReadBytes + SeekBuffered,
  {
    let mut page_reader = PageReader {
      header: PageHeader::default(),
      packet_lens: Vec::new(),
      page_buf: Vec::new(),
    };

    page_reader.try_next_page(reader)?;

    Ok(page_reader)
  }

  pub fn try_next_page<B>(&mut self, reader: &mut B) -> Result<()>
  where
    B: ReadBytes + SeekBuffered,
  {
    sync_page(reader)?;

    let sync_pos = reader.pos();
    let mut fields = [0u8; 23];

    reader.read_buf_exact(&mut fields)?;

    let (header, unchecked) = read_page_header(fields)?;
    let Ok(mut crc32) = Crc32::new(0);
    let Ok(()) = crc32.process_buf_bytes(&unchecked);
    let Ok(mut crc32_reader) = MonitorStream::new(reader, crc32);

    self.packet_lens.clear();

    let body_len = self.read_segment_table(&mut crc32_reader, header.n_segments)?;

    self.read_page_body(&mut crc32_reader, body_len)?;

    let Ok(monitor) = crc32_reader.monitor();
    let Ok(calculated_crc) = monitor.crc();

    match header.crc.cmp(&calculated_crc) {
      Ordering::Equal => {
        self.header = header;

        Ok(())
      }
      Ordering::Less | Ordering::Greater => {
        self.packet_lens.clear();
        self.page_buf.clear();

        let Ok(inner) = crc32_reader.into_inner();
        let _position = inner.seek_buffered(sync_pos);

        decode_error("ogg: crc mismatch")
      }
    }
  }

  pub fn next_page<B>(&mut self, reader: &mut B) -> Result<()>
  where
    B: ReadBytes + SeekBuffered,
  {
    walk((self, reader), next_page_step)
  }

  pub fn next_page_for_serial<B>(&mut self, reader: &mut B, serial: u32) -> Result<()>
  where
    B: ReadBytes + SeekBuffered,
  {
    walk((self, reader, serial), next_page_for_serial_step)
  }

  pub fn first_packet(&self) -> Certain<Option<&[u8]>> {
    match self.packet_lens.first() {
      Some(len) => {
        let Ok(end) = index(*len);

        Ok(self.page_buf.get(..end))
      }
      None => Ok(None),
    }
  }

  pub fn header(&self) -> Certain<PageHeader> {
    Ok(self.header)
  }

  pub fn page(&self) -> Certain<Page<'_>> {
    Ok(Page {
      header: self.header,
      packet_lens: &self.packet_lens,
      page_buf: &self.page_buf,
    })
  }

  fn read_segment_table<B: ReadBytes>(&mut self, reader: &mut B, count: u8) -> Result<u32> {
    let mut body_len = 0u32;
    let mut packet_len = 0u16;

    for _ in 0..count {
      let seg_len = reader.read_byte()?;

      body_len = body_len.saturating_add(u32::from(seg_len));
      packet_len = packet_len.saturating_add(u16::from(seg_len));

      match seg_len {
        255 => {}
        _ending => {
          self.packet_lens.push(packet_len);
          packet_len = 0;
        }
      }
    }

    Ok(body_len)
  }

  fn read_page_body<B: ReadBytes>(&mut self, reader: &mut B, len: u32) -> Result<()> {
    let declared = u64::from(len);

    match declared.cmp(&OGG_PAGE_BODY_MAX) {
      Ordering::Less | Ordering::Equal => {}
      Ordering::Greater => return decode_error("ogg: page body exceeds the segment table limit"),
    }

    self.page_buf.clear();
    self.page_buf.extend((0..declared).map(|_| 0));

    reader.read_buf_exact(&mut self.page_buf)?;

    Ok(())
  }
}

fn next_page_step<'a, B>(
  (pages, reader): (&'a mut PageReader, &'a mut B),
) -> Stepped<(&'a mut PageReader, &'a mut B), ()>
where
  B: ReadBytes + SeekBuffered,
{
  Ok(match pages.try_next_page(reader) {
    Ok(()) => Step::Halt(Ok(())),
    Err(Error::IoError(fault)) => Step::Halt(Err(Error::IoError(fault))),
    Err(
      Error::DecodeError(_)
      | Error::SeekError(_)
      | Error::Unsupported(_)
      | Error::LimitError(_)
      | Error::ResetRequired,
    ) => Step::Again((pages, reader)),
  })
}

fn next_page_for_serial_step<'a, B>(
  (pages, reader, serial): (&'a mut PageReader, &'a mut B, u32),
) -> Stepped<(&'a mut PageReader, &'a mut B, u32), ()>
where
  B: ReadBytes + SeekBuffered,
{
  Ok(match pages.try_next_page(reader) {
    Ok(()) => match (pages.header.serial.cmp(&serial), pages.header.packet_start) {
      (Ordering::Equal, PacketStart::Fresh) => Step::Halt(Ok(())),
      (Ordering::Less | Ordering::Greater, _) | (Ordering::Equal, PacketStart::Continued) => {
        Step::Again((pages, reader, serial))
      }
    },
    Err(Error::IoError(fault)) => Step::Halt(Err(Error::IoError(fault))),
    Err(
      Error::DecodeError(_)
      | Error::SeekError(_)
      | Error::Unsupported(_)
      | Error::LimitError(_)
      | Error::ResetRequired,
    ) => Step::Again((pages, reader, serial)),
  })
}
