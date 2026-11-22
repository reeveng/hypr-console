//! The four ways an ID3v2 text field is encoded, and how far each one reads:
//! to a null of its own width, or to the end of what is left when the writer
//! forgot the null, which a good many files did.

use std::io;

use console_core_audio_decoding_interfaces::errors::{Error, decode_error};
use console_core_byte_streams::{BufReader, FiniteStream, ReadBytes};
use console_core_never::Never;
use encoding_rs::UTF_16BE;

use crate::utf8_with_replacement;

const NULL_PAIR: [u8; 2] = [0, 0];

#[derive(Clone, Copy)]
pub(super) enum Encoding {
  Iso8859_1,
  Utf16Bom,
  Utf16Be,
  Utf8,
}

pub(super) fn read_encoding(
  reader: &mut BufReader<'_>,
  invalid: &'static str,
) -> Result<Encoding, Error> {
  let byte = reader.read_byte()?;

  match byte {
    0 => Ok(Encoding::Iso8859_1),
    1 => Ok(Encoding::Utf16Bom),
    2 => Ok(Encoding::Utf16Be),
    3 => Ok(Encoding::Utf8),
    _ => decode_error(invalid),
  }
}

pub(super) fn scan_text(reader: &mut BufReader<'_>, encoding: Encoding) -> io::Result<String> {
  let length = reader.bytes_available();

  let scanned = match encoding {
    Encoding::Iso8859_1 | Encoding::Utf8 => reader.scan_bytes_aligned_ref(&[0x00], 1, length),
    Encoding::Utf16Bom | Encoding::Utf16Be => {
      reader.scan_bytes_aligned_ref(&NULL_PAIR, 2, length)
    }
  };

  let bytes = scanned?;
  let Ok(text) = decode_text(encoding, bytes);

  Ok(text)
}

fn decode_text(encoding: Encoding, data: &[u8]) -> Result<String, Never> {
  Ok(match encoding {
    Encoding::Iso8859_1 => data
      .iter()
      .filter(|byte| **byte > 0x1f)
      .map(|byte| char::from(*byte))
      .collect(),
    Encoding::Utf8 => {
      let Ok(text) = utf8_with_replacement(data);

      text.trim_end_matches('\0').to_string()
    }
    Encoding::Utf16Bom | Encoding::Utf16Be => {
      let kept = (0..data.len().wrapping_div(2)).fold(data, |rest, _| {
        match rest.strip_suffix(&NULL_PAIR) {
          Some(shorter) => shorter,
          None => rest,
        }
      });
      let (text, _encoding, _malformed) = UTF_16BE.decode(kept);

      text.into_owned()
    }
  })
}
