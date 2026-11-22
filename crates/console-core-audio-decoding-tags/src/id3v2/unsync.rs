//! Synchsafe integers, and the unsynchronisation scheme that keeps a tag from
//! looking like an MPEG sync word: a zero byte is written after every 0xff,
//! and read back out here before anything else looks at the bytes.

use console_core_audio_decoding_interfaces::errors::Error;
use console_core_byte_streams::ReadBytes;
use console_core_never::Never;

pub(super) fn read_syncsafe<B: ReadBytes>(reader: &mut B, bit_width: u32) -> Result<u32, Error> {
  let mut result = 0u32;
  let mut remaining = bit_width;

  for _ in 0..bit_width.div_ceil(7) {
    let taken = remaining.min(7);
    let byte = reader.read_u8()?;
    let mask = 1u32.wrapping_shl(taken).wrapping_sub(1);

    remaining = remaining.saturating_sub(taken);
    result |= (u32::from(byte) & mask).wrapping_shl(remaining);
  }

  Ok(result)
}

pub(super) fn decode_unsynchronisation(raw: &[u8]) -> Result<Vec<u8>, Never> {
  let mut decoded = Vec::with_capacity(raw.len());
  let mut previous = None;

  for byte in raw {
    match (previous, byte) {
      (Some(0xff), 0x00) => {}
      _ => decoded.push(*byte),
    }

    previous = Some(*byte);
  }

  Ok(decoded)
}

#[cfg(test)]
mod tests {
  use console_core_audio_decoding_interfaces::errors::Error;
  use console_core_byte_streams::BufReader;

  use super::{decode_unsynchronisation, read_syncsafe};

  #[test]
  fn a_synchsafe_integer_keeps_seven_bits_of_each_byte() -> Result<(), Error> {
    let cases: &[(&[u8], u32, u32)] = &[
      (&[3, 4, 80, 1, 15], 32, 101_875_743),
      (&[16, 16, 16, 16, 16], 32, 541_098_240),
      (&[3, 4, 80, 1], 28, 6_367_233),
      (&[3, 4, 80, 1], 0, 0),
    ];

    for (bytes, width, expected) in cases {
      let Ok(mut reader) = BufReader::new(bytes);
      let read = read_syncsafe(&mut reader, *width)?;

      assert_eq!(read, *expected);
    }

    Ok(())
  }

  #[test]
  fn a_zero_after_an_ff_is_dropped_and_nothing_else_is() {
    let Ok(decoded) =
      decode_unsynchronisation(&[0xff, 0x00, 0x00, 0xff, 0xff, 0x00, 0x01, 0xff, 0x00]);

    assert_eq!(decoded, [0xff, 0x00, 0xff, 0xff, 0x01, 0xff]);
  }
}
