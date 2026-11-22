//! A byte reader.

use core::ops::Range;

#[derive(Clone, Debug)]
pub struct Reader<'a> {
  pub data: &'a [u8],

  pub offset: ::console_core_api_compatibility::NativeSize,
}

impl<'a> Reader<'a> {
  #[inline]
  pub fn new(data: &'a [u8]) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self { data, offset: 0 })
  }

  #[inline]
  pub fn new_with(
    data: &'a [u8],
    offset: ::console_core_api_compatibility::NativeSize,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self { data, offset })
  }

  #[inline]
  pub fn at_end(&self) -> core::result::Result<crate::End, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.offset >= self.data.len(), ()) {
        ::core::option::Option::Some(()) => crate::End::Reached,
        ::core::option::Option::None => crate::End::Pending,
      },
    )
  }

  #[inline]
  pub fn jump_to_end(&mut self) -> core::result::Result<(), console_core_never::Never> {
    self.offset = self.data.len();

    Ok(())
  }

  #[inline]
  pub fn jump(
    &mut self,
    offset: ::console_core_api_compatibility::NativeSize,
  ) -> core::result::Result<(), console_core_never::Never> {
    self.offset = offset;

    Ok(())
  }

  #[inline]
  pub fn tail(&mut self) -> core::result::Result<Option<&'a [u8]>, console_core_never::Never> {
    Ok(self.data.get(self.offset..))
  }

  #[inline]
  #[allow(
    clippy::len_without_is_empty,
    reason = "is_empty answers an Emptiness, which is not the bool clippy looks for"
  )]
  pub fn len(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    Ok(self.data.len())
  }

  #[inline]
  pub fn is_empty(&self) -> core::result::Result<crate::Emptiness, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.data.is_empty(), ()) {
        ::core::option::Option::Some(()) => crate::Emptiness::Empty,
        ::core::option::Option::None => crate::Emptiness::NonEmpty,
      },
    )
  }

  #[inline]
  pub fn range(
    &self,
    range: Range<::console_core_api_compatibility::NativeSize>,
  ) -> core::result::Result<Option<&'a [u8]>, console_core_never::Never> {
    Ok(self.data.get(range))
  }

  #[inline]
  pub fn offset(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    Ok(self.offset)
  }

  pub(crate) fn fixed_offset(&self) -> Result<u64, console_core_never::Never> {
    console_core_number_conversion::fixed_index(self.offset)
  }

  #[inline]
  pub fn read_bytes(
    &mut self,
    len: ::console_core_api_compatibility::NativeSize,
  ) -> core::result::Result<Option<&'a [u8]>, console_core_never::Never> {
    let Ok(explicit_value) = self.peek_bytes(len);
    let v = match explicit_value {
      Some(value) => value,
      None => return Ok(None),
    };

    let ::core::result::Result::Ok(()) =
      ::console_core_number_conversion::wrapping_add_index(len, &mut self.offset);

    Ok(Some(v))
  }

  #[inline]
  pub fn read_byte(&mut self) -> core::result::Result<Option<u8>, console_core_never::Never> {
    let Ok(explicit_value) = self.peek_byte();
    let v = match explicit_value {
      Some(value) => value,
      None => return Ok(None),
    };

    let ::core::result::Result::Ok(()) =
      ::console_core_number_conversion::wrapping_add_index(1, &mut self.offset);

    Ok(Some(v))
  }

  #[inline]
  pub fn skip_bytes(
    &mut self,
    len: ::console_core_api_compatibility::NativeSize,
  ) -> core::result::Result<Option<()>, console_core_never::Never> {
    let Ok(bytes) = self.read_bytes(len);

    Ok(bytes.map(|_| {}))
  }

  #[inline]
  pub fn peek_bytes(
    &self,
    len: ::console_core_api_compatibility::NativeSize,
  ) -> core::result::Result<Option<&'a [u8]>, console_core_never::Never> {
    Ok(
      self
        .offset
        .checked_add(len)
        .and_then(|end| self.data.get(self.offset..end)),
    )
  }

  #[inline]
  pub fn peek_byte(&self) -> core::result::Result<Option<u8>, console_core_never::Never> {
    Ok(self.data.get(self.offset).copied())
  }

  #[inline]
  pub fn eat(
    &mut self,
    f: impl Fn(u8) -> bool,
  ) -> core::result::Result<Option<u8>, console_core_never::Never> {
    let Ok(explicit_value) = self.peek_byte();
    let val = match explicit_value {
      Some(value) => value,
      None => return Ok(None),
    };

    Ok(match ::core::primitive::bool::then_some(f(val), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.forward();
        Some(val)
      }
      ::core::option::Option::None => None,
    })
  }

  #[inline]
  pub fn forward(&mut self) -> core::result::Result<(), console_core_never::Never> {
    let ::core::result::Result::Ok(()) =
      ::console_core_number_conversion::wrapping_add_index(1, &mut self.offset);

    Ok(())
  }

  #[inline]
  pub fn forward_if(
    &mut self,
    f: impl Fn(u8) -> bool,
  ) -> core::result::Result<Option<()>, console_core_never::Never> {
    let Ok(peek_byte) = self.peek_byte();

    Ok(
      match ::core::primitive::bool::then_some(
        f(match peek_byte {
          Some(value) => value,
          None => return Ok(None),
        }),
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let Ok(()) = self.forward();

          Some(())
        }
        ::core::option::Option::None => None,
      },
    )
  }

  #[inline]
  pub fn forward_while_1(
    &mut self,
    f: impl Fn(u8) -> bool,
  ) -> core::result::Result<Option<()>, console_core_never::Never> {
    let Ok(explicit_value) = self.eat(&f);

    match explicit_value {
      Some(value) => value,
      None => return Ok(None),
    };

    let Ok(()) = self.forward_while(f);

    Ok(Some(()))
  }

  #[inline]
  pub fn forward_tag(
    &mut self,
    tag: &[u8],
  ) -> core::result::Result<Option<()>, console_core_never::Never> {
    let Ok(explicit_value) = self.peek_tag(tag);

    match explicit_value {
      Some(value) => value,
      None => return Ok(None),
    };

    {
      let ::core::result::Result::Ok(numeric_amount) =
        ::console_core_number_conversion::fixed_index(tag.len());
      let ::core::result::Result::Ok(()) = ::console_core_number_conversion::wrapping_add_index(
        match ::console_core_number_conversion::index(numeric_amount) {
          ::core::result::Result::Ok(native_position) => native_position,
        },
        &mut self.offset,
      );
    };

    Ok(Some(()))
  }

  #[inline]
  pub fn forward_while(
    &mut self,
    f: impl Fn(u8) -> bool,
  ) -> core::result::Result<(), console_core_never::Never> {
    let segment = match self.data.get(self.offset..) {
      Some(bytes) => bytes,
      None => return Ok(()),
    };

    for &byte in segment {
      match f(byte).then_some(()) {
        Some(()) => {
          let Ok(()) = self.forward();
        }
        None => break,
      }
    }

    Ok(())
  }

  #[inline]
  pub fn peek_tag(
    &self,
    tag: &[u8],
  ) -> core::result::Result<Option<()>, console_core_never::Never> {
    let mut cloned = self.clone();

    for b in tag.iter().copied() {
      let Ok(peek_byte) = cloned.peek_byte();

      match ::core::primitive::bool::then_some(peek_byte == Some(b), ()) {
        ::core::option::Option::Some(()) => {
          let Ok(()) = cloned.forward();
        }
        ::core::option::Option::None => {
          return Ok(None);
        }
      }
    }

    Ok(Some(()))
  }

  #[inline]
  pub fn read_u16(&mut self) -> core::result::Result<Option<u16>, console_core_never::Never> {
    let Ok(read) = self.read_bytes(2);

    Ok(match read {
      Some([b0, b1]) => Some(u16::from_be_bytes([*b0, *b1])),
      Some(_) | None => None,
    })
  }

  #[inline]
  pub fn read_u32(&mut self) -> core::result::Result<Option<u32>, console_core_never::Never> {
    let Ok(read) = self.read_bytes(4);

    Ok(match read {
      Some([b0, b1, b2, b3]) => Some(u32::from_be_bytes([*b0, *b1, *b2, *b3])),
      Some(_) | None => None,
    })
  }

  #[inline]
  pub fn read_u64(&mut self) -> core::result::Result<Option<u64>, console_core_never::Never> {
    let Ok(read) = self.read_bytes(8);

    Ok(match read {
      Some([b0, b1, b2, b3, b4, b5, b6, b7]) => {
        Some(u64::from_be_bytes([*b0, *b1, *b2, *b3, *b4, *b5, *b6, *b7]))
      }
      Some(_) | None => None,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::Reader;

  #[test]
  fn peek_bytes_rejects_overflowing_len() {
    let Ok(reader) = Reader::new(b"abc");

    match console_core_number_conversion::index(u64::MAX) {
      Ok(everything) => assert_eq!(reader.peek_bytes(everything), Ok(None)),
    }
  }
}
