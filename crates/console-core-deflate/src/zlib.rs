//! RFC 1950 framing around a raw DEFLATE stream.
//!
//! Decoding rejects bytes between the raw stream and its checksum, and trailing bytes.

use console_core_checksums::adler32;

use crate::{DeflateError, deflate};

const HEADER: [u8; 2] = [0x78, 0x9C];

pub fn encode(bytes: &[u8]) -> Result<Vec<u8>, DeflateError> {
  let compressed = deflate(bytes)?;
  let Ok(checksum) = adler32::of(bytes);
  let mut framed = Vec::with_capacity(compressed.len().saturating_add(6));

  framed.extend_from_slice(&HEADER);
  framed.extend_from_slice(&compressed);
  framed.extend_from_slice(&checksum.to_be_bytes());

  Ok(framed)
}

pub fn decode(bytes: &[u8], limit: u32) -> Result<Vec<u8>, DeflateError> {
  let (method, flags, rest) = match bytes {
    [method, flags, rest @ ..] => (*method, *flags, rest),
    _ => return Err(DeflateError::Truncated),
  };

  match (
    method & 0x0F,
    method.wrapping_shr(4),
    u16::from_be_bytes([method, flags]).checked_rem(31),
  ) {
    (8, 0..=7, Some(0)) => {}
    _ => return Err(DeflateError::Corrupt),
  }

  match flags & 0x20 {
    0 => {}
    _ => return Err(DeflateError::UnsupportedDictionary),
  }

  let framed_length = u32::try_from(rest.len()).map_err(|_too_large| DeflateError::TooLarge)?;
  let (payload, trailer) = match rest.split_last_chunk::<4>() {
    Some(parts) => parts,
    None => return Err(DeflateError::Truncated),
  };
  let expected = u32::from_be_bytes(*trailer);
  let (output, consumed) = crate::inflate::stream(payload, limit)?;

  match ::core::primitive::bool::then_some(consumed.saturating_add(4) == framed_length, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(DeflateError::Corrupt),
  }

  let Ok(actual) = adler32::of(&output);

  match ::core::primitive::bool::then_some(actual == expected, ()) {
    ::core::option::Option::Some(()) => Ok(output),
    ::core::option::Option::None => Err(DeflateError::ChecksumMismatch),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const HELLO: &[u8] = &[
    0x78, 0x9c, 0xf3, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0, 0x05, 0x8c, 0x01, 0xf5,
  ];

  #[test]
  fn decodes_an_independent_zlib_stream() {
    assert_eq!(decode(HELLO, 5), Ok(b"Hello".to_vec()));
  }

  #[test]
  fn encoding_and_decoding_preserve_empty_and_nonempty_data()
  -> Result<(), Box<dyn std::error::Error>> {
    for bytes in [b"".as_slice(), b"hello hello hello", &[0xff; 32_768]] {
      let packed = encode(bytes)?;
      let limit = u32::try_from(bytes.len())?;

      assert_eq!(decode(&packed, limit), Ok(bytes.to_vec()));
    }

    Ok(())
  }

  #[test]
  fn rejects_corrupt_checksums_and_truncated_streams() -> Result<(), Box<dyn std::error::Error>> {
    let mut damaged = HELLO.to_vec();
    let last = damaged
      .last_mut()
      .ok_or("the fixture must have a checksum")?;
    *last ^= 1;

    assert_eq!(decode(&damaged, 5), Err(DeflateError::ChecksumMismatch));

    for (end, _byte) in HELLO.iter().enumerate() {
      let short = HELLO
        .get(..end)
        .ok_or("the cut must be inside the fixture")?;

      assert_eq!(
        decode(short, 5),
        Err(DeflateError::Truncated),
        "cut at {end}"
      );
    }

    Ok(())
  }

  #[test]
  fn respects_output_limits_and_refuses_invalid_headers_and_dictionaries() {
    assert_eq!(decode(HELLO, 4), Err(DeflateError::OutputLimitExceeded));
    assert_eq!(decode(&[0x78, 0x00], 5), Err(DeflateError::Corrupt));
    assert_eq!(
      decode(&[0x78, 0x20], 5),
      Err(DeflateError::UnsupportedDictionary)
    );
  }

  #[test]
  fn the_checksum_must_immediately_follow_the_final_deflate_block()
  -> Result<(), Box<dyn std::error::Error>> {
    let (payload, checksum) = HELLO
      .split_last_chunk::<4>()
      .ok_or("the fixture must have a checksum")?;

    for junk in [b"\0".as_slice(), &[0xde, 0xad, 0xbe, 0xef]] {
      let mut damaged = payload.to_vec();
      damaged.extend_from_slice(junk);
      damaged.extend_from_slice(checksum);
      assert_eq!(decode(&damaged, 5), Err(DeflateError::Corrupt));
      let mut trailing = HELLO.to_vec();
      trailing.extend_from_slice(junk);
      assert_eq!(decode(&trailing, 5), Err(DeflateError::Corrupt));
    }

    Ok(())
  }
}
