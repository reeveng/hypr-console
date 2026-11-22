use crate::{End, Error, IsCurrByteEqAnswer, PrefixMatch};
use alloc::borrow::ToOwned;
use alloc::vec;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Stream<'a> {
  text: &'a str,
  pos: u64,
}

impl<'a> From<&'a str> for Stream<'a> {
  #[inline]
  fn from(text: &'a str) -> Self {
    Stream { text, pos: 0 }
  }
}

impl<'a> Stream<'a> {
  #[inline]
  pub fn pos(&self) -> Result<u64, Never> {
    Ok(self.pos)
  }

  pub fn calc_char_pos(&self) -> Result<u64, Never> {
    self.calc_char_pos_at(self.pos)
  }

  pub fn calc_char_pos_at(&self, byte_pos: u64) -> Result<u64, Never> {
    let Ok(wanted) = index(byte_pos);
    let Ok(position) = fixed_index(
      self
        .text
        .char_indices()
        .take_while(|(offset, _)| *offset < wanted)
        .count(),
    );

    Ok(position.saturating_add(1))
  }

  #[inline]
  fn last(&self) -> Result<u64, Never> {
    fixed_index(self.text.len())
  }

  pub(crate) fn remaining(&self) -> Result<u64, Never> {
    let Ok(last) = self.last();

    Ok(last.saturating_sub(self.pos))
  }

  #[inline]
  pub fn jump_to_end(&mut self) -> Result<(), Never> {
    let Ok(last) = self.last();
    self.pos = last;

    Ok(())
  }

  #[inline]
  pub fn at_end(&self) -> Result<End, Never> {
    let Ok(last) = self.last();

    Ok(match (self.pos >= last).then_some(()) {
      Some(()) => End::Reached,
      None => End::Pending,
    })
  }

  #[inline]
  pub fn curr_byte(&self) -> Result<u8, Error> {
    let Ok(native) = index(self.pos);

    self
      .text
      .as_bytes()
      .get(native)
      .copied()
      .ok_or(Error::UnexpectedEndOfStream)
  }

  #[inline]
  pub(crate) fn chars(&self) -> Result<core::str::Chars<'a>, Never> {
    let Ok(text) = self.slice_tail();

    Ok(text.chars())
  }

  #[inline]
  pub fn is_curr_byte_eq(&self, byte: u8) -> Result<IsCurrByteEqAnswer, Never> {
    Ok(match (self.curr_byte() == Ok(byte)).then_some(()) {
      Some(()) => IsCurrByteEqAnswer::Yes,
      None => IsCurrByteEqAnswer::No,
    })
  }

  #[inline]
  pub fn next_byte(&self) -> Result<u8, Error> {
    let Ok(native) = index(self.pos.saturating_add(1));

    self
      .text
      .as_bytes()
      .get(native)
      .copied()
      .ok_or(Error::UnexpectedEndOfStream)
  }

  #[inline]
  pub fn advance(&mut self, count: u64) -> Result<(), Never> {
    let Ok(last) = self.last();
    self.pos = self.pos.saturating_add(count).min(last);

    Ok(())
  }

  pub fn skip_spaces(&mut self) -> Result<(), Never> {
    self.skip_bytes(|_, byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
  }

  #[inline]
  pub fn starts_with(&self, wanted: &[u8]) -> Result<PrefixMatch, Never> {
    let Ok(native) = index(self.pos);

    Ok(match self.text.as_bytes().get(native..) {
      Some(tail) => match tail.starts_with(wanted).then_some(()) {
        Some(()) => PrefixMatch::Matches,
        None => PrefixMatch::Different,
      },
      None => PrefixMatch::Different,
    })
  }

  pub(crate) fn take_prefix<T: Copy>(
    &mut self,
    prefixes: &[(&[u8], T)],
  ) -> Result<Option<T>, Never> {
    for (prefix, value) in prefixes {
      let Ok(matched) = self.starts_with(prefix);

      match matched {
        PrefixMatch::Matches => {
          let Ok(length) = fixed_index(prefix.len());
          let Ok(()) = self.advance(length);

          return Ok(Some(*value));
        }
        PrefixMatch::Different => {}
      }
    }

    Ok(None)
  }

  pub(crate) fn require_end(&self) -> Result<(), Error> {
    let Ok(end) = self.at_end();

    match end {
      End::Reached => Ok(()),
      End::Pending => {
        let Ok(position) = self.calc_char_pos();

        Err(Error::UnexpectedData(position))
      }
    }
  }

  pub fn consume_byte(&mut self, wanted: u8) -> Result<(), Error> {
    let byte = self.curr_byte()?;

    match (byte == wanted).then_some(()) {
      Some(()) => {
        let Ok(()) = self.advance(1);

        Ok(())
      }
      None => {
        let Ok(position) = self.calc_char_pos();

        Err(Error::InvalidChar(vec![byte, wanted], position))
      }
    }
  }

  pub fn parse_ident(&mut self) -> Result<&'a str, Error> {
    let start = self.pos;

    match (self.curr_byte() == Ok(b'-')).then_some(()) {
      Some(()) => {
        let Ok(()) = self.advance(1);
      }
      None => {}
    }

    let Ok(mut chars) = self.chars();

    match chars.next() {
      Some(c @ ('_' | 'a'..='z' | 'A'..='Z' | '\u{00ee}'..)) => {
        let Ok(length) = fixed_index(c.len_utf8());
        let Ok(()) = self.advance(length);
      }
      Some(_) => return Err(Error::InvalidIdent),
      None => {}
    }

    for c in chars {
      match c {
        '_' | 'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '\u{00ee}'.. => {
          let Ok(length) = fixed_index(c.len_utf8());
          let Ok(()) = self.advance(length);
        }
        _ => break,
      }
    }

    match (start == self.pos).then_some(()) {
      Some(()) => Err(Error::InvalidIdent),
      None => {
        let Ok(name) = self.slice_back(start);

        Ok(name)
      }
    }
  }

  pub fn consume_ascii_ident(&mut self) -> Result<&'a str, Never> {
    self.consume_bytes(
      |_, byte| matches!(byte, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-' | b'_'),
    )
  }

  pub fn parse_quoted_string(&mut self) -> Result<&'a str, Error> {
    let opening = self.curr_byte()?;
    let quote = match opening {
      byte @ (b'\'' | b'"') => byte,
      _ => return Err(Error::InvalidValue),
    };
    let mut previous = quote;
    let Ok(()) = self.advance(1);
    let start = self.pos;
    let Ok(last) = self.last();

    for _ in start..last {
      let byte = self.curr_byte()?;

      match (byte == quote && previous != b'\\').then_some(()) {
        Some(()) => break,
        None => {
          previous = byte;
          let Ok(()) = self.advance(1);
        }
      }
    }

    let Ok(value) = self.slice_back(start);
    self.consume_byte(quote)?;

    Ok(value)
  }

  pub fn consume_string(&mut self, wanted: &[u8]) -> Result<(), Error> {
    let Ok(end) = self.at_end();

    match end {
      End::Reached => return Err(Error::UnexpectedEndOfStream),
      End::Pending => {}
    }

    let Ok(matched) = self.starts_with(wanted);

    match matched {
      PrefixMatch::Matches => {
        let Ok(length) = fixed_index(wanted.len());
        let Ok(()) = self.advance(length);

        Ok(())
      }
      PrefixMatch::Different => {
        let Ok(tail) = self.slice_tail();
        let actual = tail.chars().take(wanted.len()).collect();
        let expected = core::str::from_utf8(wanted).map_err(|_not_utf8| Error::InvalidValue)?;
        let Ok(position) = self.calc_char_pos();

        Err(Error::InvalidString(
          vec![actual, expected.to_owned()],
          position,
        ))
      }
    }
  }

  pub fn consume_bytes<F>(&mut self, predicate: F) -> Result<&'a str, Never>
  where
    F: Fn(&Stream<'_>, u8) -> bool,
  {
    let start = self.pos;
    let Ok(()) = self.skip_bytes(predicate);

    self.slice_back(start)
  }

  pub fn skip_bytes<F>(&mut self, predicate: F) -> Result<(), Never>
  where
    F: Fn(&Stream<'_>, u8) -> bool,
  {
    let Ok(last) = self.last();

    for _ in self.pos..last {
      let byte = match self.curr_byte() {
        Ok(byte) => byte,
        Err(_end_of_text) => break,
      };

      match predicate(self, byte).then_some(()) {
        Some(()) => {
          let Ok(()) = self.advance(1);
        }
        None => break,
      }
    }

    Ok(())
  }

  #[inline]
  pub fn slice_back(&self, start: u64) -> Result<&'a str, Never> {
    let Ok(from) = index(start);
    let Ok(to) = index(self.pos);

    Ok(match self.text.get(from..to) {
      Some(text) => text,
      None => "",
    })
  }

  #[inline]
  pub fn slice_tail(&self) -> Result<&'a str, Never> {
    let Ok(native) = index(self.pos);

    Ok(match self.text.get(native..) {
      Some(text) => text,
      None => "",
    })
  }

  pub fn parse_number_or_percent(&mut self) -> Result<f64, Error> {
    let Ok(()) = self.skip_spaces();

    let n = self.parse_number()?;
    let Ok(starts_with) = self.starts_with(b"%");

    match starts_with {
      crate::PrefixMatch::Matches => {
        let Ok(()) = self.advance(1);
        Ok(n / 100.0)
      }
      crate::PrefixMatch::Different => Ok(n),
    }
  }

  pub fn parse_list_number_or_percent(&mut self) -> Result<f64, Error> {
    let Ok(at_end) = self.at_end();

    match at_end {
      crate::End::Reached => {
        return Err(Error::UnexpectedEndOfStream);
      }
      crate::End::Pending => {}
    }

    let l = self.parse_number_or_percent()?;
    let Ok(()) = self.skip_spaces();
    let Ok(()) = self.parse_list_separator();
    Ok(l)
  }

  pub fn skip_digits(&mut self) -> core::result::Result<(), console_core_never::Never> {
    let Ok(()) = self.skip_bytes(|_, byte| byte.is_ascii_digit());

    Ok(())
  }

  #[inline]
  pub(crate) fn parse_list_separator(
    &mut self,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(is_curr_byte_eq) = self.is_curr_byte_eq(b',');

    match is_curr_byte_eq {
      crate::IsCurrByteEqAnswer::Yes => {
        let Ok(()) = self.advance(1);
      }
      crate::IsCurrByteEqAnswer::No => {}
    }

    Ok(())
  }
}
