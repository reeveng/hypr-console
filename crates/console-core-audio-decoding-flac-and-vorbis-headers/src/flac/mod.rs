//! FLAC's metadata and frame-header fields shared by decoders and containers.

use console_core_byte_streams::ReadBytes;

pub mod metadata;

pub fn read_coded_number<B: ReadBytes>(src: &mut B) -> std::io::Result<Option<u64>> {
  let first = src.read_u8()?;
  let mut state = u64::from(first);

  let mask: u8 = match first {
    0x00..=0x7f => return Ok(Some(state)),
    0xc0..=0xdf => 0x1f,
    0xe0..=0xef => 0x0f,
    0xf0..=0xf7 => 0x07,
    0xf8..=0xfb => 0x03,
    0xfc..=0xfd => 0x01,
    0xfe => 0x00,
    0x80..=0xbf | 0xff => return Ok(None),
  };

  state &= u64::from(mask);

  for _ in 2..mask.leading_zeros() {
    let byte = src.read_u8()?;

    state = state.wrapping_shl(6) | u64::from(byte & 0x3f);
  }

  Ok(Some(state))
}
