//! The EBML elements and Matroska lacing a caller inspecting raw elements reads
//! with, and the CRC-32 a master element may begin with.
//!
//! Variable integers, element values and the walk over a master element's
//! children are `console-core-extensible-binary-meta-language`'s, and block
//! lacing is `console-core-matroska-blocks`'s; what is here hands their answers
//! back at the widths this reader holds a width and an offset at. Writing an
//! element went with the writer, to `forks/console-core-video-decoding-matroska-container-writer`.

use std::cmp::Ordering;

use console_core_extensible_binary_meta_language as shared;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

pub use console_core_extensible_binary_meta_language::{Elem, Size, VintBits};
pub use console_core_matroska_blocks::{Lacing, LacingError};

const CRC_LEN: u64 = 4;

pub struct Children<'a>(shared::Children<'a>);

impl<'a> Children<'a> {
  pub fn new(data: &'a [u8]) -> Result<Self, Never> {
    Ok(Self(shared::Children::from(data)))
  }
}

impl<'a> Iterator for Children<'a> {
  type Item = Elem<'a>;

  fn next(&mut self) -> Option<Elem<'a>> {
    self.0.next()
  }
}

pub fn read_vint(data: &[u8]) -> Result<Option<(u64, u64, VintBits)>, Never> {
  let Ok(read) = shared::read_vint(data);

  Ok(read.map(|(value, width, bits)| (value, u64::from(width), bits)))
}

pub fn read_size(data: &[u8]) -> Result<Option<(Size, u64)>, Never> {
  let Ok(read) = shared::read_size(data);

  Ok(read.map(|(size, width)| (size, u64::from(width))))
}

pub fn read_id(data: &[u8]) -> Result<Option<(u32, u64)>, Never> {
  let Ok(read) = shared::read_id(data);

  Ok(read.map(|(id, width)| (id, u64::from(width))))
}

pub fn read_svint(data: &[u8]) -> Result<Option<(i64, u64)>, Never> {
  let Ok(read) = shared::read_svint(data);

  Ok(read.map(|(value, width)| (value, u64::from(width))))
}

pub fn uint(data: &[u8]) -> Result<u64, Never> {
  shared::uint(data)
}

pub fn int(data: &[u8]) -> Result<i64, Never> {
  shared::int(data)
}

pub fn float(data: &[u8]) -> Result<f64, Never> {
  shared::float(data)
}

pub fn string(data: &[u8]) -> Result<String, Never> {
  shared::string(data)
}

pub fn parse_lacing(lacing: Lacing, data: &[u8]) -> Result<Vec<(u64, u64)>, LacingError> {
  let frames = console_core_matroska_blocks::parse_lacing(lacing, data)?;

  Ok(
    frames
      .iter()
      .map(|frame| (frame.offset, frame.len))
      .collect(),
  )
}

pub fn crc32(data: &[u8]) -> Result<u32, Never> {
  console_core_checksums::crc32::of(data)
}

pub fn crc32_update(crc: u32, data: &[u8]) -> Result<u32, Never> {
  console_core_checksums::crc32::update(crc, data)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrcMatch {
  Matches,
  Mismatch,
}

pub fn verify_crc(master_data: &[u8]) -> Result<Option<CrcMatch>, Never> {
  let Ok(mut children) = Children::new(master_data);
  let stored = match children.next() {
    Some(Elem {
      id: crate::ids::CRC32,
      offset,
      header_len,
      data: [a, b, c, d],
      ..
    }) => Some((
      u32::from_le_bytes([*a, *b, *c, *d]),
      offset.saturating_add(u64::from(header_len)),
    )),
    Some(_) | None => None,
  };

  Ok(stored.and_then(|(stored_2, data_start)| {
    let Ok(after) = index(data_start.saturating_add(CRC_LEN));
    let rest = master_data.get(after..)?;
    let Ok(computed) = crc32(rest);

    Some(match computed.cmp(&stored_2) {
      Ordering::Equal => CrcMatch::Matches,
      Ordering::Less | Ordering::Greater => CrcMatch::Mismatch,
    })
  }))
}

pub(crate) fn width(data: &[u8]) -> Result<u64, Never> {
  fitted::<_, u64>(data.len())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn crc() {
    let payload = [0x42u8, 0x86, 0x81, 0x01];
    let Ok(checksum) = crc32(&payload);
    let mut master = vec![0xBF, 0x84];

    master.extend_from_slice(&checksum.to_le_bytes());
    master.extend_from_slice(&payload);

    assert_eq!(crc32(b"123456789"), Ok(0xCBF4_3926));
    assert_eq!(verify_crc(&master), Ok(Some(CrcMatch::Matches)));

    match master.get_mut(7) {
      Some(byte) => *byte ^= 1,
      None => {}
    }

    assert_eq!(verify_crc(&master), Ok(Some(CrcMatch::Mismatch)));
    assert_eq!(verify_crc(&payload), Ok(None));
  }
}
