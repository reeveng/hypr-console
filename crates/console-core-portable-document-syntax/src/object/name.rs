//! Names.

use crate::ObjectReadMode;
use crate::filter::ascii_hex::decode_hex_digit;
use crate::object::Object;
use crate::object::macros::object;
use crate::reader::Reader;
use crate::reader::{Readable, ReaderContext, Skippable};
use crate::trivia::is_regular_character;
use alloc::vec::Vec;
use core::borrow::Borrow;
use core::fmt::{self, Debug, Formatter};
use core::hash::{Hash, Hasher};
use core::ops::Deref;

#[derive(Clone)]
enum NameInner<'a> {
  Borrowed(&'a [u8]),
  Owned(Vec<u8>),
}

#[derive(Clone)]
pub struct Name<'a>(NameInner<'a>);

impl<'a> Deref for Name<'a> {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    self.as_ref()
  }
}

impl AsRef<[u8]> for Name<'_> {
  fn as_ref(&self) -> &[u8] {
    match &self.0 {
      NameInner::Borrowed(data) => data,
      NameInner::Owned(data) => data,
    }
  }
}

impl Borrow<[u8]> for Name<'_> {
  fn borrow(&self) -> &[u8] {
    self.as_ref()
  }
}

impl PartialEq for Name<'_> {
  fn eq(&self, other: &Self) -> bool {
    self.as_ref() == other.as_ref()
  }
}

impl Eq for Name<'_> {}

impl Hash for Name<'_> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.as_ref().hash(state);
  }
}

impl PartialOrd for Name<'_> {
  fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for Name<'_> {
  fn cmp(&self, other: &Self) -> core::cmp::Ordering {
    self.as_ref().cmp(other.as_ref())
  }
}

impl<'a> Name<'a> {
  #[inline]
  pub fn new(data: &'a [u8]) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(!data.contains(&b'#'), ()) {
        ::core::option::Option::Some(()) => {
          let Ok(name) = Self::new_unescaped(data);

          Some(name)
        }
        ::core::option::Option::None => {
          let Ok(name) = Self::new_escaped(data);

          name
        }
      },
    )
  }

  #[inline]
  pub fn new_unescaped(data: &'a [u8]) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self(NameInner::Borrowed(data)))
  }

  #[inline]
  pub fn new_escaped(
    data: &'a [u8],
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let mut result = Vec::new();
    let Ok(mut r) = Reader::new(data);

    for _ in data {
      let Ok(next) = r.read_byte();

      match next {
        Some(b'#') => {
          let Ok(escaped) = read_escaped_byte(&mut r);

          match escaped {
            Some(value) => result.push(value),
            None => return Ok(None),
          }
        }
        Some(byte) => result.push(byte),
        None => break,
      }
    }

    Ok(Some(Self(NameInner::Owned(result))))
  }

  pub fn as_str(&self) -> core::result::Result<&str, console_core_never::Never> {
    Ok(match core::str::from_utf8(self.as_ref()) {
      Ok(text) => text,
      Err(_name_is_not_utf8) => "{non-ascii key}",
    })
  }
}

fn read_escaped_byte(r: &mut Reader<'_>) -> Result<Option<u8>, console_core_never::Never> {
  let Ok(pair) = r.read_bytes(2);
  let (high, low) = match pair {
    Some([high, low]) => (*high, *low),
    Some(_) | None => return Ok(None),
  };
  let Ok(first_digit) = decode_hex_digit(high);
  let Ok(second_digit) = decode_hex_digit(low);

  match (first_digit, second_digit) {
    (Some(first), Some(second)) => Ok(Some(first.wrapping_shl(4) | second)),
    (None, _) | (_, None) => Ok(None),
  }
}

impl Debug for Name<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match core::str::from_utf8(self.as_ref()) {
      Ok(s) => <str as Debug>::fmt(s, f),
      Err(_name_is_not_utf8) => <[u8] as Debug>::fmt(self.as_ref(), f),
    }
  }
}

object!(Name<'a>, Name);

impl Skippable for Name<'_> {
  fn skip(r: &mut Reader<'_>, _: ObjectReadMode) -> Option<()> {
    ({
      let Ok(explicit_value) = skip_name_like(r, NameSyntax::SlashPrefixed);
      explicit_value
    })
    .map(|_| ())
  }
}

impl<'a> Readable<'a> for Name<'a> {
  fn read(r: &mut Reader<'a>, _: &ReaderContext<'a>) -> Option<Self> {
    let Ok(start) = ::console_core_number_conversion::fixed_index(match r.offset() {
      ::core::result::Result::Ok(start) => start,
    });
    ({
      let Ok(explicit_value) = skip_name_like(r, NameSyntax::SlashPrefixed);
      explicit_value
    })?;
    let Ok(end) = r.fixed_offset();
    let Ok(end_position) = console_core_number_conversion::index(end);

    let Ok(range) = r.range(
      (match ::console_core_number_conversion::index(start) {
        ::core::result::Result::Ok(native_position) => native_position,
      })
      .wrapping_add(1)..end_position,
    );
    let data = range?;

    let Ok(name) = Self::new(data);

    name
  }
}

pub(crate) enum NameSyntax {
  SlashPrefixed,
  Bare,
}

pub(crate) fn skip_name_like(
  r: &mut Reader<'_>,
  syntax: NameSyntax,
) -> core::result::Result<Option<()>, console_core_never::Never> {
  match syntax {
    NameSyntax::SlashPrefixed => {
      let Ok(explicit_value) = r.forward_tag(b"/");

      match explicit_value {
        Some(value) => value,
        None => return Ok(None),
      };

      let Ok(()) = r.forward_while(|argument_0| {
        matches!(
          {
            let Ok(explicit_value_2) = is_regular_character(argument_0);
            explicit_value_2
          },
          crate::IsRegularCharacterAnswer::Yes
        )
      });
    }
    NameSyntax::Bare => {
      let Ok(explicit_value) = r.forward_while_1(|argument_0| {
        matches!(
          {
            let Ok(explicit_value) = is_regular_character(argument_0);
            explicit_value
          },
          crate::IsRegularCharacterAnswer::Yes
        )
      });

      match explicit_value {
        Some(value) => value,
        None => return Ok(None),
      };
    }
  }

  Ok(Some(()))
}

#[cfg(test)]
mod tests {
  use crate::object::Name;
  use crate::reader::{Readable, Reader, ReaderExt};

  fn read<'a, T: Readable<'a>>(data: &'a [u8]) -> Result<Option<T>, console_core_never::Never> {
    let Ok(mut reader) = Reader::new(data);

    Ok(reader.read_without_context::<T>())
  }

  #[test]
  fn name_1() {
    assert_eq!(
      read::<Name<'_>>(b"/").map(|name| name.as_deref().map(<[u8]>::to_vec)),
      Ok(Some(b"".to_vec()))
    );
  }

  #[test]
  fn name_2() {
    assert_eq!(read::<Name<'_>>(b"dfg"), Ok(None));
  }

  #[test]
  fn name_3() {
    assert_eq!(read::<Name<'_>>(b"/AB#FG"), Ok(None));
  }

  #[test]
  fn name_4() {
    assert_eq!(
      read::<Name<'_>>(b"/Name1").map(|name| name.as_deref().map(<[u8]>::to_vec)),
      Ok(Some(b"Name1".to_vec()))
    );
  }

  #[test]
  fn name_5() {
    assert_eq!(
      read::<Name<'_>>(b"/ASomewhatLongerName").map(|name| name.as_deref().map(<[u8]>::to_vec)),
      Ok(Some(b"ASomewhatLongerName".to_vec()))
    );
  }

  #[test]
  fn name_6() {
    assert_eq!(
      read::<Name<'_>>(b"/A;Name_With-Various***Characters?")
        .map(|name| name.as_deref().map(<[u8]>::to_vec)),
      Ok(Some(b"A;Name_With-Various***Characters?".to_vec()))
    );
  }

  #[test]
  fn name_7() {
    assert_eq!(
      read::<Name<'_>>(b"/1.2").map(|name| name.as_deref().map(<[u8]>::to_vec)),
      Ok(Some(b"1.2".to_vec()))
    );
  }

  #[test]
  fn name_8() {
    assert_eq!(
      read::<Name<'_>>(b"/$$").map(|name| name.as_deref().map(<[u8]>::to_vec)),
      Ok(Some(b"$$".to_vec()))
    );
  }

  #[test]
  fn name_9() {
    assert_eq!(
      read::<Name<'_>>(b"/@pattern").map(|name| name.as_deref().map(<[u8]>::to_vec)),
      Ok(Some(b"@pattern".to_vec()))
    );
  }

  #[test]
  fn name_10() {
    assert_eq!(
      read::<Name<'_>>(b"/.notdef").map(|name| name.as_deref().map(<[u8]>::to_vec)),
      Ok(Some(b".notdef".to_vec()))
    );
  }

  #[test]
  fn name_11() {
    assert_eq!(
      read::<Name<'_>>(b"/lime#20Green").map(|name| name.as_deref().map(<[u8]>::to_vec)),
      Ok(Some(b"lime Green".to_vec()))
    );
  }

  #[test]
  fn name_12() {
    assert_eq!(
      read::<Name<'_>>(b"/paired#28#29parentheses")
        .map(|name| name.as_deref().map(<[u8]>::to_vec)),
      Ok(Some(b"paired()parentheses".to_vec()))
    );
  }

  #[test]
  fn name_13() {
    assert_eq!(
      read::<Name<'_>>(b"/The_Key_of_F#23_Minor").map(|name| name.as_deref().map(<[u8]>::to_vec)),
      Ok(Some(b"The_Key_of_F#_Minor".to_vec()))
    );
  }

  #[test]
  fn name_14() {
    assert_eq!(
      read::<Name<'_>>(b"/A#42").map(|name| name.as_deref().map(<[u8]>::to_vec)),
      Ok(Some(b"AB".to_vec()))
    );
  }

  #[test]
  fn name_15() {
    assert_eq!(
      read::<Name<'_>>(b"/A#3b").map(|name| name.as_deref().map(<[u8]>::to_vec)),
      Ok(Some(b"A;".to_vec()))
    );
  }

  #[test]
  fn name_16() {
    assert_eq!(
      read::<Name<'_>>(b"/A#3B").map(|name| name.as_deref().map(<[u8]>::to_vec)),
      Ok(Some(b"A;".to_vec()))
    );
  }

  #[test]
  fn name_17() {
    assert_eq!(
      read::<Name<'_>>(b"/k1 ").map(|name| name.as_deref().map(<[u8]>::to_vec)),
      Ok(Some(b"k1".to_vec()))
    );
  }
}
