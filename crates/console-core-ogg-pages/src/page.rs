//! One page: the header, read in the order the format writes it, and the
//! content its lacing says follows.
//!
//! A header is read a part at a time, the capture pattern before the version
//! and the version before the rest, so a reader that is not at a page says so
//! as the fork did rather than as a read that ran short; the editor looking for
//! the last page in a file tells a false match from a broken file that way.
//! Writing a page out keeps the header-type byte as it was read, reserved bits
//! included, so renumbering a page changes its number and its CRC and nothing
//! else.

use std::io::{Read, Seek};

use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::{PageError, fill, read_content};

const CAPTURE_PATTERN: [u8; 4] = *b"OggS";

const VERSION: u8 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageHeader {
  pub start: u64,
  pub granule_position: u64,
  pub stream_serial: u32,
  pub sequence_number: u32,
  pub(crate) kind: u8,
  pub(crate) segments: Vec<u8>,
  pub(crate) checksum: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
  pub header: PageHeader,
  pub(crate) content: Vec<u8>,
}

impl PageHeader {
  pub fn read<R: Read + Seek>(data: &mut R) -> Result<PageHeader, PageError> {
    let start = data.stream_position()?;
    let mut pattern = [0u8; 4];

    fill(data, &mut pattern)?;

    match pattern {
      CAPTURE_PATTERN => {}
      _other => return Err(PageError::MissingCapturePattern),
    }

    let mut version = [0u8; 1];

    fill(data, &mut version)?;

    match version {
      [VERSION] => {}
      [_other] => return Err(PageError::UnknownVersion),
    }

    let mut fields = [0u8; 22];

    fill(data, &mut fields)?;

    let [
      kind,
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
      n0,
      n1,
      n2,
      n3,
      c0,
      c1,
      c2,
      c3,
      count,
    ] = fields;

    match count {
      0 => return Err(PageError::NoSegments),
      _listed => {}
    }

    let segments = read_content(data, u64::from(count))?;

    Ok(PageHeader {
      start,
      granule_position: u64::from_le_bytes([g0, g1, g2, g3, g4, g5, g6, g7]),
      stream_serial: u32::from_le_bytes([s0, s1, s2, s3]),
      sequence_number: u32::from_le_bytes([n0, n1, n2, n3]),
      kind,
      segments,
      checksum: u32::from_le_bytes([c0, c1, c2, c3]),
    })
  }

  pub(crate) fn content_size(&self) -> Result<u64, Never> {
    Ok(self.segments.iter().fold(0u64, |total, segment| {
      total.saturating_add(u64::from(*segment))
    }))
  }
}

impl Page {
  pub fn read<R: Read + Seek>(data: &mut R) -> Result<Page, PageError> {
    let header = PageHeader::read(data)?;
    let Ok(size) = header.content_size();
    let content = read_content(data, size)?;

    Ok(Page { header, content })
  }

  pub fn bytes(&self) -> Result<Vec<u8>, Never> {
    let Ok(count) = fitted::<_, u8>(self.header.segments.len());
    let mut bytes = Vec::new();

    bytes.extend(CAPTURE_PATTERN);
    bytes.extend([VERSION, self.header.kind]);
    bytes.extend(self.header.granule_position.to_le_bytes());
    bytes.extend(self.header.stream_serial.to_le_bytes());
    bytes.extend(self.header.sequence_number.to_le_bytes());
    bytes.extend(self.header.checksum.to_le_bytes());
    bytes.extend([count]);
    bytes.extend_from_slice(&self.header.segments);
    bytes.extend_from_slice(&self.content);

    Ok(bytes)
  }

  pub fn update_checksum(&mut self) -> Result<(), Never> {
    self.header.checksum = 0;

    let Ok(unchecked) = self.bytes();
    let Ok(checksum) = console_core_checksums::ogg::of(&unchecked);

    self.header.checksum = checksum;

    Ok(())
  }
}
