//! Data URLs read according to the Fetch Standard, directly from their text.
//! The URL parser's whitespace and fragment rules are applied while decoding,
//! avoiding an encoded copy of bytes that would immediately be decoded again.

#![no_std]

#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

#[cfg(not(feature = "alloc"))]
compile_error!("the `alloc` feature must be enabled");

use alloc::{string::String, vec::Vec};
use console_core_never::Never;
use core::fmt;

pub use console_core_bytes::base64 as forgiving_base64;
pub mod mime;

#[derive(Clone, Copy)]
enum Encoding {
  Percent,
  Base64,
}

#[derive(Clone, Copy)]
enum Query {
  Before,
  Inside,
}

enum AsciiByteMatch {
  Equal,
  Different,
}

impl From<bool> for AsciiByteMatch {
  fn from(equal: bool) -> Self {
    match bool::then_some(equal, ()) {
      Some(()) => Self::Equal,
      None => Self::Different,
    }
  }
}

pub struct DataUrl<'a> {
  mime_type: mime::Mime,
  encoding: Encoding,
  encoded_body_plus_fragment: &'a str,
}

#[derive(Debug)]
pub enum DataUrlError {
  NotADataUrl,
  NoComma,
}

impl fmt::Display for DataUrlError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::NotADataUrl => write!(to, "not a valid data url"),
      Self::NoComma => write!(
        to,
        "data url is missing comma delimiting attributes and body"
      ),
    }
  }
}

#[cfg(feature = "std")]
impl std::error::Error for DataUrlError {}

impl<'a> DataUrl<'a> {
  pub fn process(input: &'a str) -> Result<Self, DataUrlError> {
    let after_colon = parse_scheme(input)?;
    let (header, encoded_body_plus_fragment) =
      after_colon.split_once(',').ok_or(DataUrlError::NoComma)?;

    match header.split_once('#') {
      Some(_premature_fragment) => return Err(DataUrlError::NoComma),
      None => {}
    }

    let Ok((mime_type, encoding)) = parse_header(header);

    Ok(DataUrl {
      mime_type,
      encoding,
      encoded_body_plus_fragment,
    })
  }

  pub fn mime_type(&self) -> Result<&mime::Mime, Never> {
    Ok(&self.mime_type)
  }

  pub fn decode<F, E>(
    &self,
    write_bytes: F,
  ) -> Result<Option<FragmentIdentifier<'a>>, forgiving_base64::DecodeError<E>>
  where
    F: FnMut(&[u8]) -> Result<(), E>,
  {
    match self.encoding {
      Encoding::Base64 => decode_with_base64(self.encoded_body_plus_fragment, write_bytes),
      Encoding::Percent => decode_without_base64(self.encoded_body_plus_fragment, write_bytes)
        .map_err(forgiving_base64::DecodeError::WriteError),
    }
  }

  pub fn decode_to_vec(
    &self,
  ) -> Result<(Vec<u8>, Option<FragmentIdentifier<'a>>), forgiving_base64::InvalidBase64> {
    let mut body = Vec::new();
    let output = &mut body;
    let fragment = self.decode(move |bytes| {
      output.extend_from_slice(bytes);

      Ok::<(), Never>(())
    })?;

    Ok((body, fragment))
  }
}

pub struct FragmentIdentifier<'a>(&'a [u8]);

impl FragmentIdentifier<'_> {
  pub fn to_percent_encoded(&self) -> Result<String, Never> {
    let mut encoded = String::new();

    for &byte in self.0 {
      match byte {
        b'\t' | b'\n' | b'\r' => {}
        b'\0'..=b' ' | b'"' | b'<' | b'>' | b'`' | b'\x7F'..=b'\xFF' => {
          let Ok(()) = percent_encode(byte, &mut encoded);
        }
        other => encoded.push(char::from(other)),
      }
    }

    Ok(encoded)
  }
}

fn parse_scheme(input: &str) -> Result<&str, DataUrlError> {
  let trimmed = input.trim_start_matches(|ch| ch <= ' ');
  let mut bytes = trimmed.as_bytes().iter();

  for expected in b"data:" {
    let next = bytes
      .by_ref()
      .find(|byte| !matches!(byte, b'\t' | b'\n' | b'\r'));

    match next {
      Some(byte) => match AsciiByteMatch::from(byte.eq_ignore_ascii_case(expected)) {
        AsciiByteMatch::Equal => {}
        AsciiByteMatch::Different => return Err(DataUrlError::NotADataUrl),
      },
      None => return Err(DataUrlError::NotADataUrl),
    }
  }

  let rest = core::str::from_utf8(bytes.as_slice()).map_err(|_utf8| DataUrlError::NotADataUrl)?;

  Ok(rest.trim_end_matches(|ch| ch <= ' '))
}

fn parse_header(header: &str) -> Result<(mime::Mime, Encoding), Never> {
  let trimmed = header.trim_matches([' ', '\t', '\n', '\r']);
  let Ok(without_suffix) = remove_base64_suffix(trimmed);
  let (mime_type, encoding) = match without_suffix {
    Some(prefix) => (prefix, Encoding::Base64),
    None => (trimmed, Encoding::Percent),
  };
  let mut text = String::new();

  match mime_type.as_bytes().first() {
    Some(b';') => text.push_str("text/plain"),
    Some(_) | None => {}
  }

  let mut query = Query::Before;

  for byte in mime_type.bytes() {
    match byte {
      b'\t' | b'\n' | b'\r' => {}
      b'\0'..=b'\x1F' | b'\x7F'..=b'\xFF' => {
        let Ok(()) = percent_encode(byte, &mut text);
      }
      b' ' | b'"' | b'<' | b'>' => match query {
        Query::Before => text.push(char::from(byte)),
        Query::Inside => {
          let Ok(()) = percent_encode(byte, &mut text);
        }
      },
      b'?' => {
        query = Query::Inside;
        text.push('?');
      }
      other => text.push(char::from(other)),
    }
  }

  let parsed = match text.parse() {
    Ok(parsed_mime) => parsed_mime,
    Err(_invalid_mime) => {
      let Ok(default) = mime::Mime::default_for_data_url();

      default
    }
  };

  Ok((parsed, encoding))
}

fn remove_base64_suffix(text: &str) -> Result<Option<&str>, Never> {
  let mut bytes = text.as_bytes().iter();

  for expected in b"46esab" {
    let next = bytes
      .by_ref()
      .rev()
      .find(|byte| !matches!(byte, b'\t' | b'\n' | b'\r'));

    match next {
      Some(byte) => match AsciiByteMatch::from(byte.eq_ignore_ascii_case(expected)) {
        AsciiByteMatch::Equal => {}
        AsciiByteMatch::Different => return Ok(None),
      },
      None => return Ok(None),
    }
  }

  let separator = bytes
    .by_ref()
    .rev()
    .find(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | b'\r'));

  match separator {
    Some(b';') => Ok(match core::str::from_utf8(bytes.as_slice()) {
      Ok(prefix) => Some(prefix),
      Err(_invalid_text) => None,
    }),
    Some(_) | None => Ok(None),
  }
}

fn percent_encode(byte: u8, text: &mut String) -> Result<(), Never> {
  let Ok([high, low]) =
    console_core_bytes::hex::encode_byte(byte, console_core_bytes::hex::Case::Upper);

  text.push('%');
  text.push(char::from(high));
  text.push(char::from(low));

  Ok(())
}

enum Part<'a> {
  End,
  Fragment(&'a [u8]),
  Chunk(&'a [u8], &'a [u8]),
  Decoded(u8, &'a [u8]),
  Skipped(&'a [u8]),
}

fn next_part(remaining: &[u8]) -> Result<Part<'_>, Never> {
  let (byte, rest) = match remaining.split_first() {
    Some(split) => split,
    None => return Ok(Part::End),
  };

  match byte {
    b'\t' | b'\n' | b'\r' => Ok(Part::Skipped(rest)),
    b'#' => Ok(Part::Fragment(rest)),
    b'%' => {
      let Ok(escaped) = console_core_percent_decoding::escaped_byte(rest);

      Ok(match (escaped, rest.split_first_chunk::<2>()) {
        (Some(decoded), Some((_digits, after))) => Part::Decoded(decoded, after),
        (None, _) | (Some(_), None) => {
          let Ok(plain) = plain_chunk(remaining);

          plain
        }
      })
    }
    _ordinary => plain_chunk(remaining),
  }
}

fn plain_chunk(remaining: &[u8]) -> Result<Part<'_>, Never> {
  let (_first, rest) = match remaining.split_first() {
    Some(split) => split,
    None => return Ok(Part::End),
  };
  let tail = match rest
    .split(|byte| matches!(byte, b'%' | b'#' | b'\t' | b'\n' | b'\r'))
    .next()
  {
    Some(tail) => tail,
    None => return Ok(Part::End),
  };

  Ok(
    match remaining.split_at_checked(tail.len().saturating_add(1)) {
      Some((chunk, after)) => Part::Chunk(chunk, after),
      None => Part::End,
    },
  )
}

fn decode_without_base64<F, E>(
  input: &str,
  mut write_bytes: F,
) -> Result<Option<FragmentIdentifier<'_>>, E>
where
  F: FnMut(&[u8]) -> Result<(), E>,
{
  let mut remaining = input.as_bytes();

  for _step in input.bytes() {
    let Ok(part) = next_part(remaining);

    match part {
      Part::End => {
        write_bytes(&[])?;

        return Ok(None);
      }
      Part::Fragment(fragment) => return Ok(Some(FragmentIdentifier(fragment))),
      Part::Skipped(after) => remaining = after,
      Part::Decoded(byte, after) => {
        write_bytes(&[byte])?;

        remaining = after;
      }
      Part::Chunk(chunk, after) => {
        write_bytes(chunk)?;

        match after {
          [] => return Ok(None),
          [_, ..] => remaining = after,
        }
      }
    }
  }

  write_bytes(&[])?;

  Ok(None)
}

fn decode_with_base64<F, E>(
  input: &str,
  write_bytes: F,
) -> Result<Option<FragmentIdentifier<'_>>, forgiving_base64::DecodeError<E>>
where
  F: FnMut(&[u8]) -> Result<(), E>,
{
  let Ok(mut decoder) = forgiving_base64::Decoder::new(write_bytes);
  let output = &mut decoder;
  let fragment = decode_without_base64(input, move |bytes| output.feed(bytes))?;

  decoder.finish()?;

  Ok(fragment)
}
