//! Strings.

use crate::ObjectReadMode;
use crate::crypto::DecryptionTarget;
use crate::filter::ascii_hex;
use crate::object::Object;
use crate::object::macros::object;
use crate::reader::Reader;
use crate::reader::{Readable, ReaderContext, Skippable};
use crate::trivia::is_white_space_character;
use alloc::vec::Vec;
use core::borrow::Borrow;
use core::hash::{Hash, Hasher};
use core::ops::Deref;

#[derive(Clone)]
enum StringInner<'a> {
  Borrowed(&'a [u8]),
  Owned(Vec<u8>),
}

impl AsRef<[u8]> for StringInner<'_> {
  fn as_ref(&self) -> &[u8] {
    match self {
      Self::Borrowed(data) => data,
      Self::Owned(data) => data,
    }
  }
}

#[derive(Clone)]
pub struct String<'a>(StringInner<'a>);

impl<'a> String<'a> {
  pub fn as_bytes(&self) -> core::result::Result<&[u8], console_core_never::Never> {
    Ok(self.as_ref())
  }
}

impl Deref for String<'_> {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    self.as_ref()
  }
}

impl AsRef<[u8]> for String<'_> {
  fn as_ref(&self) -> &[u8] {
    match &self.0 {
      StringInner::Borrowed(data) => data,
      StringInner::Owned(data) => data,
    }
  }
}

impl Borrow<[u8]> for String<'_> {
  fn borrow(&self) -> &[u8] {
    self.as_ref()
  }
}

impl PartialEq for String<'_> {
  fn eq(&self, other: &Self) -> bool {
    self.as_ref() == other.as_ref()
  }
}

impl Eq for String<'_> {}

impl Hash for String<'_> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.as_ref().hash(state);
  }
}

impl core::fmt::Debug for String<'_> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    <[u8] as core::fmt::Debug>::fmt(self.as_ref(), f)
  }
}

object!(String<'a>, String);

impl Skippable for String<'_> {
  fn skip(r: &mut Reader<'_>, _: ObjectReadMode) -> Option<()> {
    let Ok(peek_byte) = r.peek_byte();

    let checked_value = peek_byte?;

    match checked_value {
      b'<' => {
        let Ok(explicit_value) = skip_hex(r);
        explicit_value
      }
      b'(' => {
        let Ok(explicit_value) = skip_literal(r);
        explicit_value
      }
      _ => None,
    }
  }
}

impl<'a> Readable<'a> for String<'a> {
  fn read(r: &mut Reader<'a>, ctx: &ReaderContext<'a>) -> Option<Self> {
    let Ok(peek_byte) = r.peek_byte();
    let checked_value = peek_byte?;
    let decoded = match checked_value {
      b'<' => {
        let Ok(hex) = read_hex(r);

        let bytes = hex?;
        StringInner::Owned(bytes)
      }
      b'(' => {
        let Ok(literal) = read_literal(r);

        literal?
      }
      _ => return None,
    };

    let Ok(final_data) = decrypt_string(decoded, ctx);
    Some(Self(final_data))
  }
}

fn decrypt_string<'a>(
  decoded: StringInner<'a>,
  ctx: &ReaderContext<'_>,
) -> Result<StringInner<'a>, console_core_never::Never> {
  let Ok(xref) = ctx.xref();
  let Ok(needs_decryption) = xref.needs_decryption(ctx);
  let Ok(object_id) = ctx.obj_number();

  match (needs_decryption, object_id) {
    (crate::NeedsDecryptionAnswer::Yes, Some(id)) => {
      let Ok(decrypted) = xref.decrypt(id, decoded.as_ref(), DecryptionTarget::String);
      Ok(match decrypted {
        Some(bytes) => StringInner::Owned(bytes),
        None => decoded,
      })
    }
    (crate::NeedsDecryptionAnswer::Yes, None) | (crate::NeedsDecryptionAnswer::No, _) => {
      Ok(decoded)
    }
  }
}

impl From<Vec<u8>> for StringInner<'_> {
  fn from(value: Vec<u8>) -> Self {
    Self::Owned(value)
  }
}

fn skip_hex(r: &mut Reader<'_>) -> core::result::Result<Option<()>, console_core_never::Never> {
  let Ok(explicit_value) = r.forward_tag(b"<");

  match explicit_value {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(()) = r.forward_while(|byte| {
    let Ok(whitespace) = is_white_space_character(byte);
    byte.is_ascii_hexdigit() || whitespace == crate::IsWhiteSpaceCharacterAnswer::Yes
  });

  let Ok(explicit_value_4) = r.forward_tag(b">");

  match explicit_value_4 {
    Some(value) => value,
    None => return Ok(None),
  };

  Ok(Some(()))
}

fn read_hex(
  r: &mut Reader<'_>,
) -> core::result::Result<Option<Vec<u8>>, console_core_never::Never> {
  let Ok(start) = ::console_core_number_conversion::fixed_index(match r.offset() {
    ::core::result::Result::Ok(start) => start,
  });
  let Ok(explicit_value) = skip_hex(r);

  match explicit_value {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(end) = ::console_core_number_conversion::fixed_index(match r.offset() {
    ::core::result::Result::Ok(end) => end,
  });

  let Ok(explicit_value_3) = r.range(
    (match ::console_core_number_conversion::index(start) {
      ::core::result::Result::Ok(native_position) => native_position,
    })
    .wrapping_add(1)..(match ::console_core_number_conversion::index(end) {
      ::core::result::Result::Ok(native_position) => native_position,
    })
    .wrapping_sub(1),
  );
  let raw = match explicit_value_3 {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(explicit_value_4) = ascii_hex::decode_into(raw);
  let decoded = match explicit_value_4 {
    Some(value) => value,
    None => return Ok(None),
  };

  Ok(Some(decoded))
}

fn skip_literal(
  r: &mut Reader<'_>,
) -> core::result::Result<Option<()>, console_core_never::Never> {
  let Ok(explicit_value) = r.forward_tag(b"(");

  match explicit_value {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(tail) = r.tail();
  let remaining = match tail {
    Some(bytes) => bytes,
    None => return Ok(None),
  };
  let mut depth = 1_u64;

  for _ in remaining {
    let Ok(next) = r.read_byte();

    match (next, depth) {
      (Some(b'('), _) => depth = depth.wrapping_add(1),
      (Some(b')'), 1) => return Ok(Some(())),
      (Some(b')'), _) => depth = depth.wrapping_sub(1),
      (Some(b'\\'), _) => {
        let Ok(escaped) = r.read_byte();

        match escaped {
          Some(_escaped_byte) => {}
          None => return Ok(None),
        }
      }
      (Some(_), _) => {}
      (None, _) => return Ok(None),
    }
  }

  Ok(None)
}

fn read_literal<'a>(
  r: &mut Reader<'a>,
) -> core::result::Result<Option<StringInner<'a>>, console_core_never::Never> {
  let Ok(start) = ::console_core_number_conversion::fixed_index(match r.offset() {
    ::core::result::Result::Ok(start) => start,
  });
  let Ok(explicit_value) = skip_literal(r);

  match explicit_value {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(end) = ::console_core_number_conversion::fixed_index(match r.offset() {
    ::core::result::Result::Ok(end) => end,
  });

  let Ok(explicit_value_3) = r.range(
    (match ::console_core_number_conversion::index(start) {
      ::core::result::Result::Ok(native_position) => native_position,
    })
    .wrapping_add(1)..(match ::console_core_number_conversion::index(end) {
      ::core::result::Result::Ok(native_position) => native_position,
    })
    .wrapping_sub(1),
  );
  let data = match explicit_value_3 {
    Some(value) => value,
    None => return Ok(None),
  };

  match ::core::primitive::bool::then_some(
    !data.iter().any(|b| matches!(b, b'\\' | b'\n' | b'\r')),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(Some(StringInner::Borrowed(data)));
    }
    ::core::option::Option::None => {}
  }

  let mut result = Vec::new();

  match console_core_postscript_tokens::decoding::literal(data, &mut result) {
    Ok(()) => {}
    Err(error) => {
      warn!("invalid escape in PDF literal string {:?}", error);
      return Ok(None);
    }
  }

  Ok(Some(StringInner::Owned(result)))
}

#[cfg(test)]
mod tests {
  use alloc::{vec, vec::Vec};

  use crate::object::String;
  use crate::reader::Reader;
  use crate::reader::ReaderExt;

  fn bytes(data: &[u8]) -> Result<Option<Vec<u8>>, console_core_never::Never> {
    let Ok(mut reader) = Reader::new(data);
    let read = reader.read_without_context::<String<'_>>();

    Ok(read.map(|string| {
      let Ok(held) = string.as_bytes();
      held.to_vec()
    }))
  }

  #[test]
  fn hex_string_empty() {
    assert_eq!(bytes(b"<>"), Ok(Some(b"".to_vec())));
  }

  #[test]
  fn hex_string_1() {
    assert_eq!(bytes(b"<00010203>"), Ok(Some(vec![0x00, 0x01, 0x02, 0x03])));
  }

  #[test]
  fn hex_string_2() {
    assert_eq!(
      bytes(b"<000102034>"),
      Ok(Some(vec![0x00, 0x01, 0x02, 0x03, 0x40]))
    );
  }

  #[test]
  fn hex_string_trailing_1() {
    assert_eq!(
      bytes(b"<000102034>dfgfg4"),
      Ok(Some(vec![0x00, 0x01, 0x02, 0x03, 0x40]))
    );
  }

  #[test]
  fn hex_string_trailing_2() {
    assert_eq!(bytes(b"<1  3 4>dfgfg4"), Ok(Some(vec![0x13, 0x40])));
  }

  #[test]
  fn hex_string_trailing_3() {
    assert_eq!(bytes(b"<1>dfgfg4"), Ok(Some(vec![0x10])));
  }

  #[test]
  fn hex_string_invalid_1() {
    assert_eq!(bytes(b"<"), Ok(None));
  }

  #[test]
  fn hex_string_invalid_2() {
    assert_eq!(bytes(b"34AD"), Ok(None));
  }

  #[test]
  fn literal_string_empty() {
    assert_eq!(bytes(b"()"), Ok(Some(b"".to_vec())));
  }

  #[test]
  fn literal_string_1() {
    assert_eq!(bytes(b"(Hi there.)"), Ok(Some(b"Hi there.".to_vec())));
  }

  #[test]
  fn literal_string_2() {
    assert!(matches!(bytes(b"(Hi \\777)"), Ok(Some(_))));
  }

  #[test]
  fn literal_string_3() {
    assert_eq!(bytes(b"(Hi ) there.)"), Ok(Some(b"Hi ".to_vec())));
  }

  #[test]
  fn literal_string_4() {
    assert_eq!(
      bytes(b"(Hi (()) there)"),
      Ok(Some(b"Hi (()) there".to_vec()))
    );
  }

  #[test]
  fn literal_string_5() {
    assert_eq!(bytes(b"(Hi \\()"), Ok(Some(b"Hi (".to_vec())));
  }

  #[test]
  fn literal_string_6() {
    assert_eq!(bytes(b"(Hi \\\nthere)"), Ok(Some(b"Hi there".to_vec())));
  }

  #[test]
  fn literal_string_7() {
    assert_eq!(bytes(b"(Hi \\05354)"), Ok(Some(b"Hi +54".to_vec())));
  }

  #[test]
  fn literal_string_8() {
    assert_eq!(bytes(b"(\\3)"), Ok(Some(b"\x03".to_vec())));
  }

  #[test]
  fn literal_string_9() {
    assert_eq!(bytes(b"(\\36)"), Ok(Some(b"\x1e".to_vec())));
  }

  #[test]
  fn literal_string_10() {
    assert_eq!(bytes(b"(\\36ab)"), Ok(Some(b"\x1eab".to_vec())));
  }

  #[test]
  fn literal_string_11() {
    assert_eq!(bytes(b"(\\00Y)"), Ok(Some(b"\0Y".to_vec())));
  }

  #[test]
  fn literal_string_12() {
    assert_eq!(bytes(b"(\\0Y)"), Ok(Some(b"\0Y".to_vec())));
  }

  #[test]
  fn literal_string_trailing() {
    assert_eq!(bytes(b"(Hi there.)abcde"), Ok(Some(b"Hi there.".to_vec())));
  }

  #[test]
  fn literal_string_invalid() {
    assert_eq!(bytes(b"(Hi \\778)"), Ok(Some(b"Hi \x3F8".to_vec())));
  }

  #[test]
  fn string_1() {
    assert_eq!(bytes(b"(Hi there.)"), Ok(Some(b"Hi there.".to_vec())));
  }

  #[test]
  fn string_2() {
    assert_eq!(bytes(b"<00010203>"), Ok(Some(vec![0x00, 0x01, 0x02, 0x03])));
  }
}
