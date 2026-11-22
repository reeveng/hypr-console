use core::ops::Range;
use core::str;

use memchr::memchr2;

use crate::{Error, TextPos};

type Result<T> = core::result::Result<T, Error>;

enum TextEncoding {
  Ascii,
  Unicode,
}

impl From<bool> for TextEncoding {
  fn from(ascii: bool) -> Self {
    match bool::then_some(ascii, ()) {
      Some(()) => Self::Ascii,
      None => Self::Unicode,
    }
  }
}

enum XmlCharacter {
  Permitted,
  Forbidden,
}

enum NameCharacter {
  Valid,
  Invalid,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Scan {
  Advance,
  Stop,
}

impl From<bool> for Scan {
  fn from(keep: bool) -> Self {
    match bool::then_some(keep, ()) {
      Some(()) => Self::Advance,
      None => Self::Stop,
    }
  }
}

const XML_SPACE: fn(u8) -> Scan = |byte| match byte {
  b' ' | b'\t' | b'\n' | b'\r' => Scan::Advance,
  _ => Scan::Stop,
};

const BYTE_NAME_START: fn(u8) -> crate::NameStart = |byte| match byte {
  b'A'..=b'Z' | b'a'..=b'z' | b':' | b'_' => crate::NameStart::Valid,
  _ => crate::NameStart::Invalid,
};

const BYTE_NAME: fn(u8) -> NameCharacter = |byte| match (BYTE_NAME_START(byte), byte) {
  (crate::NameStart::Valid, _) | (_, b'0'..=b'9' | b'-' | b'.') => NameCharacter::Valid,
  (crate::NameStart::Invalid, _) => NameCharacter::Invalid,
};

const BYTE_XML_CHAR: fn(u8) -> XmlCharacter = |byte| match byte {
  b' ' | b'\t' | b'\n' | b'\r' | 0x21.. => XmlCharacter::Permitted,
  _ => XmlCharacter::Forbidden,
};

const CHAR_NAME_START: fn(char) -> crate::NameStart = |ch| match ch {
  'A'..='Z'
  | 'a'..='z'
  | ':'
  | '_'
  | '\u{c0}'..='\u{d6}'
  | '\u{d8}'..='\u{f6}'
  | '\u{f8}'..='\u{2ff}'
  | '\u{370}'..='\u{37d}'
  | '\u{37f}'..='\u{1fff}'
  | '\u{200c}'..='\u{200d}'
  | '\u{2070}'..='\u{218f}'
  | '\u{2c00}'..='\u{2fef}'
  | '\u{3001}'..='\u{d7ff}'
  | '\u{f900}'..='\u{fdcf}'
  | '\u{fdf0}'..='\u{fffd}'
  | '\u{10000}'..='\u{effff}' => crate::NameStart::Valid,
  _ => crate::NameStart::Invalid,
};

const CHAR_NAME: fn(char) -> NameCharacter = |ch| match (CHAR_NAME_START(ch), ch) {
  (crate::NameStart::Valid, _)
  | (_, '0'..='9' | '-' | '.' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}') => {
    NameCharacter::Valid
  }
  (crate::NameStart::Invalid, _) => NameCharacter::Invalid,
};

const CHAR_XML_CHAR: fn(char) -> XmlCharacter = |ch| match ch {
  '\u{fffe}' | '\u{ffff}' => XmlCharacter::Forbidden,
  '\t' | '\n' | '\r' | ' '.. => XmlCharacter::Permitted,
  _ => XmlCharacter::Forbidden,
};

#[inline]
fn is_xml_str(s: &str, value_start: u64, stream: &mut Stream<'_>) -> Result<()> {
  match TextEncoding::from(s.as_bytes().is_ascii()) {
    TextEncoding::Ascii => {}
    TextEncoding::Unicode => return is_xml_str_unicode(s, value_start, stream),
  }

  for (i, &byte) in s.as_bytes().iter().enumerate() {
    match BYTE_XML_CHAR(byte) {
      XmlCharacter::Forbidden => {
        let Ok(offset) = console_core_number_conversion::fitted::<_, u64>(i);
        let Ok(position) = stream.gen_text_pos_from(value_start.wrapping_add(offset));

        return Err(Error::NonXmlChar(char::from(byte), position));
      }
      XmlCharacter::Permitted => {}
    }
  }

  Ok(())
}

#[cold]
#[inline(never)]
fn is_xml_str_unicode(s: &str, value_start: u64, stream: &mut Stream<'_>) -> Result<()> {
  for (i, ch) in s.char_indices() {
    match CHAR_XML_CHAR(ch) {
      XmlCharacter::Forbidden => {
        let Ok(gen_text_pos_from) = stream.gen_text_pos_from(u64::wrapping_add(
          {
            let Ok(converted) = console_core_number_conversion::fitted::<_, u64>(value_start);
            converted
          },
          {
            let Ok(converted) = console_core_number_conversion::fitted::<_, u64>(i);
            converted
          },
        ));
        return Err(Error::NonXmlChar(ch, gen_text_pos_from));
      }
      XmlCharacter::Permitted => {}
    }
  }

  Ok(())
}

#[must_use]
#[derive(Clone, Copy)]
pub struct StrSpan<'input> {
  text: &'input str,
  start: u64,
}

impl<'input> From<&'input str> for StrSpan<'input> {
  #[inline]
  fn from(text: &'input str) -> Self {
    StrSpan { text, start: 0 }
  }
}

impl<'input> StrSpan<'input> {
  pub fn from_substr(text: &'input str, fragment: Range<u64>) -> Result<Self> {
    let start = fragment.start;
    let Ok(indices) = console_core_number_conversion::indices(fragment);
    let text_2 = text.get(indices).ok_or(Error::UnexpectedEndOfStream)?;

    Ok(Self {
      text: text_2,
      start,
    })
  }

  #[inline]
  pub fn range(&self) -> core::result::Result<Range<u64>, console_core_never::Never> {
    Ok(
      self.start
        ..u64::wrapping_add(
          {
            let Ok(converted) = console_core_number_conversion::fitted::<_, u64>(self.start);
            converted
          },
          {
            let Ok(converted) = console_core_number_conversion::fitted::<_, u64>(self.text.len());
            converted
          },
        ),
    )
  }

  #[inline]
  pub fn as_str(&self) -> core::result::Result<&'input str, console_core_never::Never> {
    Ok(self.text)
  }

  fn slice_region(&self, region: Range<u64>) -> Result<&'input str> {
    let Ok(indices) = console_core_number_conversion::indices(region);

    self.text.get(indices).ok_or(Error::UnexpectedEndOfStream)
  }
}

pub enum Token<'input> {
  ProcessingInstruction(&'input str, Option<&'input str>, Range<u64>),

  Comment(&'input str, Range<u64>),

  EntityDeclaration(&'input str, StrSpan<'input>),

  ElementStart(&'input str, &'input str, u64),

  Attribute(
    Range<u64>,
    u16,
    u8,
    &'input str,
    &'input str,
    StrSpan<'input>,
  ),

  ElementEnd(ElementEnd<'input>, Range<u64>),

  Text(&'input str, Range<u64>),

  Cdata(&'input str, Range<u64>),
}

#[derive(Clone, Copy)]
pub enum ElementEnd<'input> {
  Open,

  Close(&'input str, &'input str),

  Empty,
}

pub trait XmlEvents<'input> {
  fn token(&mut self, token: Token<'input>) -> Result<()>;

  fn resolve_entity(
    &mut self,
    _pub_id: Option<&str>,
    _uri: &str,
  ) -> core::result::Result<Option<&'input str>, crate::ResolutionError> {
    Ok(None)
  }
}

pub fn parse<'input>(
  text: &'input str,
  dtd: crate::Dtd,
  events: &mut impl XmlEvents<'input>,
) -> Result<()> {
  let Ok(mut stream) = Stream::new(text);
  let s = &mut stream;

  parse_declaration(s)?;

  parse_misc(s, events)?;

  let Ok(()) = s.skip_spaces();

  let Ok(starts_with) = s.starts_with(b"<!DOCTYPE");

  match starts_with {
    crate::PrefixMatch::Matches => {
      match dtd {
        crate::Dtd::Deny => return Err(Error::DtdDetected),
        crate::Dtd::Allow => {}
      }

      parse_doctype(s, events)?;
      parse_misc(s, events)?;
    }
    crate::PrefixMatch::Different => {}
  }

  let Ok(()) = s.skip_spaces();

  let Ok(peek) = s.peek();

  match peek {
    Some(b'<') => {
      parse_element(s, events)?;
    }
    Some(_) | None => {}
  }

  parse_misc(s, events)?;

  let Ok(at_end) = s.at_end();

  match at_end {
    crate::End::Pending => {
      let Ok(gen_text_pos) = s.gen_text_pos();
      return Err(Error::UnknownToken(gen_text_pos));
    }
    crate::End::Reached => {}
  }

  Ok(())
}

fn parse_misc<'input>(s: &mut Stream<'input>, events: &mut impl XmlEvents<'input>) -> Result<()> {
  let Ok(budget) = s.as_bytes();

  for _step in budget {
    let Ok(end) = s.at_end();

    match end {
      crate::End::Reached => break,
      crate::End::Pending => {}
    }

    let Ok(()) = s.skip_spaces();

    let Ok(starts_with) = s.starts_with(b"<!--");

    match starts_with {
      crate::PrefixMatch::Matches => {
        parse_comment(s, events)?;
      }
      crate::PrefixMatch::Different => {
        let Ok(s_starts_with) = s.starts_with(b"<?");

        match s_starts_with {
          crate::PrefixMatch::Matches => {
            parse_pi(s, events)?;
          }
          crate::PrefixMatch::Different => {
            break;
          }
        }
      }
    }
  }

  Ok(())
}

fn parse_declaration(s: &mut Stream) -> Result<()> {
  let Ok(starts_with) = s.starts_with(&[0xEF, 0xBB, 0xBF]);

  match starts_with {
    crate::PrefixMatch::Matches => {
      let Ok(()) = s.advance(3);
    }
    crate::PrefixMatch::Different => {}
  }

  let Ok(s_starts_with) = s.starts_with(b"<?xml ");

  match s_starts_with {
    crate::PrefixMatch::Different => return Ok(()),
    crate::PrefixMatch::Matches => {}
  }

  fn consume_spaces(s: &mut Stream) -> Result<()> {
    let Ok(starts_with_space) = s.starts_with_space();

    match starts_with_space {
      crate::PrefixMatch::Matches => {
        let Ok(()) = s.skip_spaces();
      }
      crate::PrefixMatch::Different => {
        let Ok(starts_with) = s.starts_with(b"?>");

        let Ok(at_end) = s.at_end();

        match (starts_with, at_end) {
          (crate::PrefixMatch::Different, crate::End::Pending) => {
            let explicit_value_0 = s.curr_byte()?;
            let Ok(gen_text_pos) = s.gen_text_pos();
            return Err(Error::InvalidChar2(
              "a whitespace",
              explicit_value_0,
              gen_text_pos,
            ));
          }
          (crate::PrefixMatch::Matches, _) | (_, crate::End::Reached) => {}
        }
      }
    }

    Ok(())
  }

  let Ok(()) = s.advance(5);
  consume_spaces(s)?;

  let Ok(version_prefix) = s.starts_with(b"version");

  match version_prefix {
    crate::PrefixMatch::Different => {
      return s.skip_string("version");
    }
    crate::PrefixMatch::Matches => {}
  }

  let _version = parse_attribute(s)?;
  consume_spaces(s)?;

  let Ok(encoding_prefix) = s.starts_with(b"encoding");

  match encoding_prefix {
    crate::PrefixMatch::Matches => {
      let _encoding = parse_attribute(s)?;
      consume_spaces(s)?;
    }
    crate::PrefixMatch::Different => {}
  }

  let Ok(standalone_prefix) = s.starts_with(b"standalone");

  match standalone_prefix {
    crate::PrefixMatch::Matches => {
      let _standalone = parse_attribute(s)?;
    }
    crate::PrefixMatch::Different => {}
  }

  let Ok(()) = s.skip_spaces();
  s.skip_string("?>")?;

  Ok(())
}

fn parse_comment<'input>(
  s: &mut Stream<'input>,
  events: &mut impl XmlEvents<'input>,
) -> Result<()> {
  let Ok(pos) = s.pos();
  let Ok(start) = console_core_number_conversion::fitted::<_, u64>(pos);
  let Ok(()) = s.advance(4);
  let text = s.consume_chars(|s_2, c| {
    Scan::from(
      !(c == '-'
        && (({
          let Ok(explicit_value) = s_2.starts_with(b"-->");
          explicit_value
        }) == crate::PrefixMatch::Matches)),
    )
  })?;
  s.skip_string("-->")?;

  match text.find("--") {
    Some(_double_dash) => {
      let Ok(gen_text_pos_from) = s.gen_text_pos_from(start);
      return Err(Error::InvalidComment(gen_text_pos_from));
    }
    None => {}
  }

  match text.strip_suffix('-') {
    Some(_before_dash) => {
      let Ok(gen_text_pos_from) = s.gen_text_pos_from(start);
      return Err(Error::InvalidComment(gen_text_pos_from));
    }
    None => {}
  }

  let Ok(range) = s.range_from(start);
  events.token(Token::Comment(text, range))?;

  Ok(())
}

fn parse_pi<'input>(s: &mut Stream<'input>, events: &mut impl XmlEvents<'input>) -> Result<()> {
  let Ok(starts_with) = s.starts_with(b"<?xml ");

  match starts_with {
    crate::PrefixMatch::Matches => {
      let Ok(gen_text_pos) = s.gen_text_pos();
      return Err(Error::UnexpectedDeclaration(gen_text_pos));
    }
    crate::PrefixMatch::Different => {}
  }

  let Ok(pos) = s.pos();
  let Ok(start) = console_core_number_conversion::fitted::<_, u64>(pos);
  let Ok(()) = s.advance(2);
  let target = s.consume_name()?;
  let Ok(()) = s.skip_spaces();
  let content = s.consume_chars(|s_2, c| {
    Scan::from(
      !(c == '?'
        && (({
          let Ok(explicit_value) = s_2.starts_with(b"?>");
          explicit_value
        }) == crate::PrefixMatch::Matches)),
    )
  })?;
  let content_2 = match content {
    "" => None,
    nonempty => Some(nonempty),
  };

  s.skip_string("?>")?;

  let Ok(range) = s.range_from(start);
  events.token(Token::ProcessingInstruction(target, content_2, range))?;
  Ok(())
}

fn parse_doctype<'input>(
  s: &mut Stream<'input>,
  events: &mut impl XmlEvents<'input>,
) -> Result<()> {
  let Ok(s_pos) = s.pos();
  let Ok(start) = console_core_number_conversion::fitted::<_, u64>(s_pos);
  parse_doctype_start(s)?;
  let Ok(()) = s.skip_spaces();

  let Ok(peek) = s.peek();

  match peek {
    Some(b'>') => {
      let Ok(()) = s.advance(1);
      return Ok(());
    }
    Some(_) | None => {}
  }

  let Ok(()) = s.advance(1);
  let Ok(budget) = s.as_bytes();

  for _step in budget {
    let Ok(end) = s.at_end();

    match end {
      crate::End::Reached => break,
      crate::End::Pending => {}
    }

    let Ok(()) = s.skip_spaces();

    match s.remaining {
      [b'<', b'!', b'E', b'N', b'T', b'I', b'T', b'Y', ..] => parse_entity_decl(s, events)?,
      [b'<', b'!', b'-', b'-', ..] => parse_comment(s, events)?,
      [b'<', b'?', ..] => parse_pi(s, events)?,
      [b']', ..] => {
        let Ok(()) = s.advance(1);
        let Ok(()) = s.skip_spaces();

        match s.curr_byte() {
          Ok(b'>') => {
            let Ok(()) = s.advance(1);
            break;
          }
          Ok(c) => {
            let Ok(gen_text_pos) = s.gen_text_pos();
            return Err(Error::InvalidChar2("'>'", c, gen_text_pos));
          }
          Err(_unexpected_end) => return Err(Error::UnexpectedEndOfStream),
        }
      }
      [b'<', b'!', b'E', b'L', b'E', b'M', b'E', b'N', b'T', ..]
      | [b'<', b'!', b'A', b'T', b'T', b'L', b'I', b'S', b'T', ..]
      | [
        b'<',
        b'!',
        b'N',
        b'O',
        b'T',
        b'A',
        b'T',
        b'I',
        b'O',
        b'N',
        ..,
      ] => match consume_decl(s) {
        Err(_invalid_declaration) => {
          let Ok(pos) = s.gen_text_pos_from(start);
          return Err(Error::UnknownToken(pos));
        }
        Ok(()) => {}
      },
      _ => {
        let Ok(gen_text_pos) = s.gen_text_pos();
        return Err(Error::UnknownToken(gen_text_pos));
      }
    }
  }

  Ok(())
}

fn parse_doctype_start(s: &mut Stream) -> Result<()> {
  let Ok(()) = s.advance(9);

  s.consume_spaces()?;
  s.skip_name()?;
  let Ok(()) = s.skip_spaces();

  let _ = parse_external_id(s)?;
  let Ok(()) = s.skip_spaces();

  let c = s.curr_byte()?;

  match c {
    b'[' | b'>' => {}
    _ => {
      let Ok(gen_text_pos) = s.gen_text_pos();
      return Err(Error::InvalidChar2("'[' or '>'", c, gen_text_pos));
    }
  }

  Ok(())
}

fn parse_external_id<'input>(
  s: &mut Stream<'input>,
) -> Result<Option<(Option<&'input str>, &'input str)>> {
  enum ExternalId {
    System,
    Public,
  }

  let kind = match s.remaining {
    [b'S', b'Y', b'S', b'T', b'E', b'M', ..] => ExternalId::System,
    [b'P', b'U', b'B', b'L', b'I', b'C', ..] => ExternalId::Public,
    _ => return Ok(None),
  };

  let Ok(()) = s.advance(6);
  s.consume_spaces()?;
  let quote = s.consume_quote()?;
  let first = s.consume_bytes(|c| Scan::from(c != quote))?;
  s.consume_byte(quote)?;

  Ok(Some(match kind {
    ExternalId::System => (None, first),
    ExternalId::Public => {
      s.consume_spaces()?;
      let quote_2 = s.consume_quote()?;
      let second = s.consume_bytes(|c| Scan::from(c != quote_2))?;
      s.consume_byte(quote_2)?;

      (Some(first), second)
    }
  }))
}

fn parse_entity_decl<'input>(
  s: &mut Stream<'input>,
  events: &mut impl XmlEvents<'input>,
) -> Result<()> {
  let Ok(()) = s.advance(8);
  s.consume_spaces()?;

  let Ok(consume_byte) = s.try_consume_byte(b'%');
  let kind = match consume_byte {
    crate::Consumption::Consumed => {
      s.consume_spaces()?;
      EntityKind::Parameter
    }
    crate::Consumption::Missing => EntityKind::General,
  };

  let name = s.consume_name()?;
  s.consume_spaces()?;
  let explicit_value_1 = parse_entity_def(s, events, kind)?;

  match explicit_value_1 {
    Some(definition) => {
      events.token(Token::EntityDeclaration(name, definition))?;
    }
    None => {}
  }

  let Ok(()) = s.skip_spaces();
  s.consume_byte(b'>')?;

  Ok(())
}

enum EntityKind {
  General,
  Parameter,
}

fn parse_entity_def<'input>(
  s: &mut Stream<'input>,
  events: &mut impl XmlEvents<'input>,
  kind: EntityKind,
) -> Result<Option<StrSpan<'input>>> {
  let c = s.curr_byte()?;

  match c {
    b'"' | b'\'' => {
      let quote = s.consume_quote()?;
      let Ok(s_pos) = s.pos();
      let Ok(start) = console_core_number_conversion::fitted::<_, u64>(s_pos);
      let Ok(()) = s.skip_bytes(|c_2| Scan::from(c_2 != quote));
      let value = s.slice_back_span(start)?;
      s.consume_byte(quote)?;
      Ok(Some(value))
    }
    b'S' | b'P' => parse_external_entity(s, events, kind),
    _ => {
      let Ok(pos) = s.gen_text_pos();
      Err(Error::InvalidChar2("a quote, SYSTEM or PUBLIC", c, pos))
    }
  }
}

fn parse_external_entity<'input>(
  s: &mut Stream<'input>,
  events: &mut impl XmlEvents<'input>,
  kind: EntityKind,
) -> Result<Option<StrSpan<'input>>> {
  let external_id = parse_external_id(s)?;
  let (pub_id, uri) = match external_id {
    Some(pair) => pair,
    None => {
      let Ok(position) = s.gen_text_pos();

      return Err(Error::InvalidExternalID(position));
    }
  };

  match kind {
    EntityKind::General => {
      let Ok(()) = s.skip_spaces();
      let Ok(ndata) = s.starts_with(b"NDATA");

      match ndata {
        crate::PrefixMatch::Matches => {
          let Ok(()) = s.advance(5);
          s.consume_spaces()?;
          s.skip_name()?;
        }
        crate::PrefixMatch::Different => {}
      }
    }
    EntityKind::Parameter => {}
  }

  let resolved = events.resolve_entity(pub_id, uri).map_err(|message| {
    let Ok(position) = s.gen_text_pos();

    Error::EntityResolver(position, message)
  })?;

  match resolved {
    Some(text) => {
      let Ok(mut stream) = Stream::new(text);
      parse_declaration(&mut stream)?;
      let Ok(start) = stream.pos();
      let Ok(end) = console_core_number_conversion::fitted::<_, u64>(text.len());
      let value = StrSpan::from_substr(text, start..end)?;

      Ok(Some(value))
    }
    None => Ok(None),
  }
}

fn consume_decl(s: &mut Stream) -> Result<()> {
  let Ok(()) = s.skip_bytes(|c| Scan::from(c != b'>'));
  s.consume_byte(b'>')?;
  Ok(())
}

fn parse_element<'input>(
  s: &mut Stream<'input>,
  events: &mut impl XmlEvents<'input>,
) -> Result<()> {
  let explicit_value_3 = parse_opening(s, events)?;

  match explicit_value_3 {
    Opening::Open => walk_content(s, events, Boundary::Element),
    Opening::Empty | Opening::Incomplete => Ok(()),
  }
}

enum Opening {
  Open,
  Empty,
  Incomplete,
}

#[derive(Clone, Copy)]
enum Boundary {
  Element,
  Fragment,
}

enum DepthChange {
  Open,
  Close,
  Same,
}

fn parse_opening<'input>(
  s: &mut Stream<'input>,
  events: &mut impl XmlEvents<'input>,
) -> Result<Opening> {
  let Ok(start) = s.pos();
  let Ok(()) = s.advance(1);
  let (prefix, local) = s.consume_qname()?;

  events.token(Token::ElementStart(prefix, local, start))?;

  let Ok(budget) = s.as_bytes();

  for _step in budget {
    let Ok(end) = s.at_end();

    match end {
      crate::End::Reached => break,
      crate::End::Pending => {}
    }

    let Ok(space) = s.starts_with_space();
    let Ok(()) = s.skip_spaces();
    let Ok(start_2) = s.pos();
    let byte = s.curr_byte()?;

    match byte {
      b'/' => {
        let Ok(()) = s.advance(1);

        s.consume_byte(b'>')?;

        let Ok(range) = s.range_from(start_2);

        events.token(Token::ElementEnd(ElementEnd::Empty, range))?;

        return Ok(Opening::Empty);
      }
      b'>' => {
        let Ok(()) = s.advance(1);
        let Ok(range) = s.range_from(start_2);

        events.token(Token::ElementEnd(ElementEnd::Open, range))?;

        return Ok(Opening::Open);
      }
      _attribute => {
        match space {
          crate::PrefixMatch::Matches => {}
          crate::PrefixMatch::Different => s.consume_spaces()?,
        }

        parse_element_attribute(s, events, start_2)?;
      }
    }
  }

  Ok(Opening::Incomplete)
}

fn parse_element_attribute<'input>(
  s: &mut Stream<'input>,
  events: &mut impl XmlEvents<'input>,
  start: u64,
) -> Result<()> {
  let (prefix, local) = s.consume_qname()?;
  let Ok(qname_end) = s.pos();
  let Ok(qname_len) =
    console_core_number_conversion::fitted::<_, u16>(qname_end.saturating_sub(start));

  s.consume_eq()?;

  let Ok(after_equal) = s.pos();
  let Ok(eq_len) =
    console_core_number_conversion::fitted::<_, u8>(after_equal.saturating_sub(qname_end));
  let quote = s.consume_quote()?;
  let Ok(value_start) = s.pos();

  s.advance_until2([quote, b'<'])?;

  let value = s.slice_back_span(value_start)?;
  let Ok(text) = value.as_str();

  is_xml_str(text, value_start, s)?;
  s.consume_byte(quote)?;

  let Ok(end) = s.pos();

  events.token(Token::Attribute(
    start..end,
    qname_len,
    eq_len,
    prefix,
    local,
    value,
  ))
}

fn parse_attribute<'input>(
  s: &mut Stream<'input>,
) -> Result<(&'input str, &'input str, StrSpan<'input>)> {
  let (prefix, local) = s.consume_qname()?;
  s.consume_eq()?;
  let quote = s.consume_quote()?;
  let quote_c = char::from(quote);

  let Ok(pos) = s.pos();
  let Ok(value_start) = console_core_number_conversion::fitted::<_, u64>(pos);
  s.skip_chars(|_, c| Scan::from(c != quote_c && c != '<'))?;
  let value = s.slice_back_span(value_start)?;
  s.consume_byte(quote)?;
  Ok((prefix, local, value))
}

pub fn parse_content<'input>(
  s: &mut Stream<'input>,
  events: &mut impl XmlEvents<'input>,
) -> Result<()> {
  walk_content(s, events, Boundary::Fragment)
}

fn walk_content<'input>(
  s: &mut Stream<'input>,
  events: &mut impl XmlEvents<'input>,
  boundary: Boundary,
) -> Result<()> {
  let mut depth = match boundary {
    Boundary::Element => 1u64,
    Boundary::Fragment => 0u64,
  };
  let Ok(budget) = s.as_bytes();

  for _step in budget {
    let Ok(end) = s.at_end();

    match end {
      crate::End::Reached => break,
      crate::End::Pending => {}
    }

    let explicit_value_4 = content_step(s, events)?;

    match explicit_value_4 {
      DepthChange::Open => depth = depth.saturating_add(1),
      DepthChange::Same => {}
      DepthChange::Close => match (boundary, depth.checked_sub(1)) {
        (_, None) | (Boundary::Element, Some(0)) => return Ok(()),
        (_, Some(remaining)) => depth = remaining,
      },
    }
  }

  Ok(())
}

fn content_step<'input>(
  s: &mut Stream<'input>,
  events: &mut impl XmlEvents<'input>,
) -> Result<DepthChange> {
  let byte = s.curr_byte()?;

  match byte {
    b'<' => {}
    _text => {
      parse_text(s, events)?;

      return Ok(DepthChange::Same);
    }
  }

  let Ok(position) = s.gen_text_pos();
  let next = s
    .next_byte()
    .map_err(|_end_after_open| Error::UnknownToken(position))?;

  match next {
    b'!' => {
      let Ok(comment) = s.starts_with(b"<!--");
      let Ok(cdata) = s.starts_with(b"<![CDATA[");

      match (comment, cdata) {
        (crate::PrefixMatch::Matches, _) => parse_comment(s, events)?,
        (_, crate::PrefixMatch::Matches) => parse_cdata(s, events)?,
        (crate::PrefixMatch::Different, crate::PrefixMatch::Different) => {
          return Err(Error::UnknownToken(position));
        }
      }

      Ok(DepthChange::Same)
    }
    b'?' => {
      parse_pi(s, events)?;

      Ok(DepthChange::Same)
    }
    b'/' => {
      parse_close_element(s, events)?;

      Ok(DepthChange::Close)
    }
    _element => {
      let opening = parse_opening(s, events)?;

      Ok(match opening {
        Opening::Open => DepthChange::Open,
        Opening::Empty | Opening::Incomplete => DepthChange::Same,
      })
    }
  }
}

fn parse_cdata<'input>(
  s: &mut Stream<'input>,
  events: &mut impl XmlEvents<'input>,
) -> Result<()> {
  let Ok(pos) = s.pos();
  let Ok(start) = console_core_number_conversion::fitted::<_, u64>(pos);
  let Ok(()) = s.advance(9);
  let text = s.consume_chars(|s_2, c| {
    Scan::from(
      !(c == ']'
        && (({
          let Ok(explicit_value) = s_2.starts_with(b"]]>");
          explicit_value
        }) == crate::PrefixMatch::Matches)),
    )
  })?;
  s.skip_string("]]>")?;
  let Ok(range) = s.range_from(start);
  events.token(Token::Cdata(text, range))?;
  Ok(())
}

fn parse_close_element<'input>(
  s: &mut Stream<'input>,
  events: &mut impl XmlEvents<'input>,
) -> Result<()> {
  let Ok(pos) = s.pos();
  let Ok(start) = console_core_number_conversion::fitted::<_, u64>(pos);
  let Ok(()) = s.advance(2);

  let (prefix, tag_name) = s.consume_qname()?;
  let Ok(()) = s.skip_spaces();
  s.consume_byte(b'>')?;

  let Ok(range) = s.range_from(start);
  events.token(Token::ElementEnd(
    ElementEnd::Close(prefix, tag_name),
    range,
  ))?;
  Ok(())
}

fn parse_text<'input>(s: &mut Stream<'input>, events: &mut impl XmlEvents<'input>) -> Result<()> {
  let Ok(pos) = s.pos();
  let Ok(start) = console_core_number_conversion::fitted::<_, u64>(pos);
  let text = s.consume_chars(|_, c| Scan::from(c != '<'))?;

  match text.find("]]>") {
    Some(_cdata_end) => {
      let Ok(gen_text_pos) = s.gen_text_pos();
      return Err(Error::InvalidCharacterData(gen_text_pos));
    }
    None => {}
  }

  let Ok(range) = s.range_from(start);
  events.token(Token::Text(text, range))?;
  Ok(())
}

#[derive(Clone, Copy)]
pub enum Reference<'input> {
  Entity(&'input str),

  Char(char),
}

#[derive(Clone)]
pub struct Stream<'input> {
  remaining: &'input [u8],
  end: u64,
  span: StrSpan<'input>,
}

impl<'input> Stream<'input> {
  pub fn new(text: &'input str) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(end) = console_core_number_conversion::fitted::<_, u64>(text.len());

    Ok(Self {
      remaining: text.as_bytes(),
      end,
      span: StrSpan::from(text),
    })
  }

  pub fn from_substr(text: &'input str, fragment: Range<u64>) -> Result<Self> {
    let end = fragment.end;
    let Ok(indices) = console_core_number_conversion::indices(fragment);
    let explicit_value_5 = text.get(indices).ok_or(Error::UnexpectedEndOfStream)?;
    let remaining = explicit_value_5.as_bytes();

    Ok(Self {
      remaining,
      end,
      span: StrSpan::from(text),
    })
  }

  pub fn pos(&self) -> core::result::Result<u64, console_core_never::Never> {
    let Ok(remaining) = console_core_number_conversion::fitted::<_, u64>(self.remaining.len());

    Ok(self.end.saturating_sub(remaining))
  }

  pub fn at_end(&self) -> core::result::Result<crate::End, console_core_never::Never> {
    Ok(match self.remaining {
      [] => crate::End::Reached,
      [_, ..] => crate::End::Pending,
    })
  }

  pub fn curr_byte(&self) -> Result<u8> {
    self
      .remaining
      .first()
      .copied()
      .ok_or(Error::UnexpectedEndOfStream)
  }

  pub fn peek(&self) -> core::result::Result<Option<u8>, console_core_never::Never> {
    Ok(self.remaining.first().copied())
  }

  fn next_byte(&self) -> Result<u8> {
    self
      .remaining
      .get(1)
      .copied()
      .ok_or(Error::UnexpectedEndOfStream)
  }

  pub(crate) fn as_bytes(&self) -> core::result::Result<&'input [u8], console_core_never::Never> {
    Ok(self.remaining)
  }

  pub fn advance(&mut self, count: u64) -> core::result::Result<(), console_core_never::Never> {
    match count {
      0 => {}
      other => {
        let Ok(last) = console_core_number_conversion::index(other.saturating_sub(1));
        let mut bytes = self.remaining.iter();
        let _last_taken = bytes.nth(last);

        self.remaining = bytes.as_slice();
      }
    }

    Ok(())
  }

  fn starts_with(
    &self,
    text: &[u8],
  ) -> core::result::Result<crate::PrefixMatch, console_core_never::Never> {
    Ok(match self.remaining.strip_prefix(text) {
      Some(_remaining) => crate::PrefixMatch::Matches,
      None => crate::PrefixMatch::Different,
    })
  }

  fn consume_byte(&mut self, c: u8) -> Result<()> {
    let curr = self.curr_byte()?;

    match curr.cmp(&c) {
      core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {
        let Ok(gen_text_pos) = self.gen_text_pos();
        return Err(Error::InvalidChar(c, curr, gen_text_pos));
      }
      core::cmp::Ordering::Equal => {}
    }

    let Ok(()) = self.advance(1);
    Ok(())
  }

  fn try_consume_byte(
    &mut self,
    expected: u8,
  ) -> core::result::Result<crate::Consumption, console_core_never::Never> {
    Ok(match self.remaining.first().copied().cmp(&Some(expected)) {
      core::cmp::Ordering::Equal => {
        let Ok(()) = self.advance(1);

        crate::Consumption::Consumed
      }
      core::cmp::Ordering::Less | core::cmp::Ordering::Greater => crate::Consumption::Missing,
    })
  }

  fn skip_string(&mut self, text: &'static str) -> Result<()> {
    match self.remaining.strip_prefix(text.as_bytes()) {
      Some(_remaining) => {}
      None => {
        let Ok(position) = self.gen_text_pos();

        return Err(Error::InvalidString(text, position));
      }
    }

    let Ok(length) = console_core_number_conversion::fitted::<_, u64>(text.len());
    let Ok(()) = self.advance(length);

    Ok(())
  }

  fn consume_bytes<F: Fn(u8) -> Scan>(&mut self, keep: F) -> Result<&'input str> {
    let Ok(start) = self.pos();
    let Ok(()) = self.skip_bytes(keep);

    self.slice_back(start)
  }

  fn skip_bytes<F: Fn(u8) -> Scan>(
    &mut self,
    keep: F,
  ) -> core::result::Result<(), console_core_never::Never> {
    let until = self
      .remaining
      .iter()
      .position(|byte| keep(*byte) == Scan::Stop)
      .map(|native| {
        let Ok(position) = console_core_number_conversion::fitted::<_, u64>(native);

        position
      });
    let count = match until {
      Some(count) => count,
      None => {
        let Ok(own_length) =
          console_core_number_conversion::fitted::<_, u64>(self.remaining.len());

        own_length
      }
    };

    self.advance(count)
  }

  #[inline]
  fn consume_chars<F>(&mut self, f: F) -> Result<&'input str>
  where
    F: Fn(&Stream, char) -> Scan,
  {
    let Ok(pos) = self.pos();
    let Ok(start) = console_core_number_conversion::fitted::<_, u64>(pos);
    self.skip_chars(f)?;
    let explicit_value_6 = self.slice_back(start)?;
    Ok(explicit_value_6)
  }

  #[inline]
  fn skip_chars<F>(&mut self, f: F) -> Result<()>
  where
    F: Fn(&Stream, char) -> Scan,
  {
    let chars = self.chars()?;

    for c in chars {
      match CHAR_XML_CHAR(c) {
        XmlCharacter::Forbidden => {
          let Ok(gen_text_pos) = self.gen_text_pos();
          return Err(Error::NonXmlChar(c, gen_text_pos));
        }
        XmlCharacter::Permitted => match f(self, c) {
          Scan::Advance => {
            let Ok(len_utf8) = console_core_number_conversion::fitted::<_, u64>(c.len_utf8());
            let Ok(()) = self.advance(len_utf8);
          }
          Scan::Stop => {
            break;
          }
        },
      }
    }

    Ok(())
  }

  #[inline]
  fn advance_until2(&mut self, needles: [u8; 2]) -> Result<()> {
    let [needle1, needle2] = needles;
    let Ok(bytes) = self.as_bytes();

    match memchr2(needle1, needle2, bytes) {
      Some(pos) => {
        let Ok(()) = self.advance({
          let Ok(position) = console_core_number_conversion::fitted::<_, u64>(pos);
          position
        });
        Ok(())
      }
      None => Err(Error::UnexpectedEndOfStream),
    }
  }

  fn chars(&self) -> Result<str::Chars<'input>> {
    let text = str::from_utf8(self.remaining)
      .map_err(|_not_a_character_boundary| Error::UnexpectedEndOfStream)?;

    Ok(text.chars())
  }

  fn slice_back(&self, start: u64) -> Result<&'input str> {
    let Ok(end) = self.pos();

    self.span.slice_region(start..end)
  }

  fn slice_back_span(&self, start: u64) -> Result<StrSpan<'input>> {
    let Ok(end) = self.pos();

    StrSpan::from_substr(self.span.text, start..end)
  }

  fn range_from(
    &self,
    start: u64,
  ) -> core::result::Result<Range<u64>, console_core_never::Never> {
    let Ok(end) = self.pos();

    Ok(start..end)
  }

  fn skip_spaces(&mut self) -> core::result::Result<(), console_core_never::Never> {
    self.skip_bytes(XML_SPACE)
  }

  fn starts_with_space(
    &self,
  ) -> core::result::Result<crate::PrefixMatch, console_core_never::Never> {
    Ok(match self.remaining.first().copied().map(XML_SPACE) {
      Some(Scan::Advance) => crate::PrefixMatch::Matches,
      Some(Scan::Stop) | None => crate::PrefixMatch::Different,
    })
  }

  fn consume_spaces(&mut self) -> Result<()> {
    let Ok(at_end) = self.at_end();

    match at_end {
      crate::End::Reached => {
        return Err(Error::UnexpectedEndOfStream);
      }
      crate::End::Pending => {}
    }

    let Ok(starts_with_space) = self.starts_with_space();

    match starts_with_space {
      crate::PrefixMatch::Different => {
        let explicit_value_7 = self.curr_byte()?;
        let Ok(gen_text_pos) = self.gen_text_pos();
        return Err(Error::InvalidChar2(
          "a whitespace",
          explicit_value_7,
          gen_text_pos,
        ));
      }
      crate::PrefixMatch::Matches => {}
    }

    let Ok(()) = self.skip_spaces();
    Ok(())
  }

  pub fn consume_reference(&mut self) -> Result<Option<Reference<'input>>> {
    let Ok(position) = self.gen_text_pos();
    let Ok(ampersand) = self.try_consume_byte(b'&');

    match ampersand {
      crate::Consumption::Missing => return Ok(None),
      crate::Consumption::Consumed => {}
    }

    let Ok(numeric) = self.try_consume_byte(b'#');
    let reference = match numeric {
      crate::Consumption::Consumed => {
        let Ok(hexadecimal) = self.try_consume_byte(b'x');
        let (digits, radix) = match hexadecimal {
          crate::Consumption::Consumed => {
            let digits = self.consume_bytes(|byte| Scan::from(byte.is_ascii_hexdigit()))?;

            (digits, 16)
          }
          crate::Consumption::Missing => {
            let digits = self.consume_bytes(|byte| Scan::from(byte.is_ascii_digit()))?;

            (digits, 10)
          }
        };
        let value = u32::from_str_radix(digits, radix)
          .map_err(|_not_digits| Error::MalformedEntityReference(position))?;
        let ch = match char::from_u32(value) {
          Some(ch) => ch,
          None => char::REPLACEMENT_CHARACTER,
        };

        match CHAR_XML_CHAR(ch) {
          XmlCharacter::Permitted => Reference::Char(ch),
          XmlCharacter::Forbidden => return Err(Error::MalformedEntityReference(position)),
        }
      }
      crate::Consumption::Missing => {
        let name = self
          .consume_name()
          .map_err(|_invalid_name| Error::MalformedEntityReference(position))?;

        match name {
          "quot" => Reference::Char('"'),
          "amp" => Reference::Char('&'),
          "apos" => Reference::Char('\''),
          "lt" => Reference::Char('<'),
          "gt" => Reference::Char('>'),
          other => Reference::Entity(other),
        }
      }
    };

    self
      .consume_byte(b';')
      .map_err(|_missing_end| Error::MalformedEntityReference(position))?;

    Ok(Some(reference))
  }

  fn consume_name(&mut self) -> Result<&'input str> {
    let Ok(pos) = self.pos();
    let Ok(start) = console_core_number_conversion::fitted::<_, u64>(pos);
    self.skip_name()?;

    let name = self.slice_back(start)?;

    match name {
      "" => {
        let Ok(gen_text_pos_from) = self.gen_text_pos_from(start);
        return Err(Error::InvalidName(gen_text_pos_from));
      }
      _ => {}
    }

    Ok(name)
  }

  fn skip_name(&mut self) -> Result<()> {
    let Ok(pos) = self.pos();
    let Ok(start) = console_core_number_conversion::fitted::<_, u64>(pos);
    let mut iter = self.chars()?;

    match iter.next() {
      Some(c) => match CHAR_NAME_START(c) {
        crate::NameStart::Valid => {
          let Ok(len_utf8) = console_core_number_conversion::fitted::<_, u64>(c.len_utf8());
          let Ok(()) = self.advance(len_utf8);
        }
        crate::NameStart::Invalid => {
          let Ok(gen_text_pos_from) = self.gen_text_pos_from(start);
          return Err(Error::InvalidName(gen_text_pos_from));
        }
      },
      None => {}
    }

    for c in iter {
      match CHAR_NAME(c) {
        NameCharacter::Valid => {
          let Ok(len_utf8) = console_core_number_conversion::fitted::<_, u64>(c.len_utf8());
          let Ok(()) = self.advance(len_utf8);
        }
        NameCharacter::Invalid => {
          break;
        }
      }
    }

    Ok(())
  }

  #[inline(never)]
  fn consume_qname(&mut self) -> Result<(&'input str, &'input str)> {
    let Ok(pos) = self.pos();
    let Ok(start) = console_core_number_conversion::fitted::<_, u64>(pos);

    let mut splitter = None;

    let Ok(budget) = self.as_bytes();

    for _step in budget {
      let Ok(end) = self.at_end();

      match end {
        crate::End::Reached => break,
        crate::End::Pending => {}
      }

      let b = self.curr_byte()?;

      match b {
        b':' => match splitter {
          None => {
            let Ok(own_pos) = self.pos();
            splitter = Some(own_pos);
            let Ok(()) = self.advance(1);
          }
          Some(_) => {
            let Ok(gen_text_pos_from) = self.gen_text_pos_from(start);
            return Err(Error::InvalidName(gen_text_pos_from));
          }
        },
        ..128 => match BYTE_NAME(b) {
          NameCharacter::Valid => {
            let Ok(()) = self.advance(1);
          }
          NameCharacter::Invalid => break,
        },
        128.. => {
          let scanned = self.consume_unicode_name_character()?;

          match scanned {
            Scan::Advance => {}
            Scan::Stop => break,
          }
        }
      }
    }

    let (prefix, local) = match splitter {
      Some(splitter_2) => {
        let prefix = self.span.slice_region(start..splitter_2)?;
        let local = self.slice_back(u64::wrapping_add(
          {
            let Ok(converted) = console_core_number_conversion::fitted::<_, u64>(splitter_2);
            converted
          },
          {
            let Ok(converted) = console_core_number_conversion::fitted::<_, u64>(1);
            converted
          },
        ))?;
        (prefix, local)
      }
      None => {
        let local = self.slice_back(start)?;

        let prefix = self.span.slice_region(start..start)?;

        (prefix, local)
      }
    };

    fn is_xml_name_start(
      name: &str,
    ) -> core::result::Result<crate::NameStart, console_core_never::Never> {
      Ok(match name.as_bytes().first() {
        Some(byte @ ..128) => BYTE_NAME_START(*byte),
        Some(128..) => match name.chars().next() {
          Some(character) => CHAR_NAME_START(character),
          None => crate::NameStart::Invalid,
        },
        None => crate::NameStart::Invalid,
      })
    }

    match prefix {
      "" => {}
      _ => {
        let Ok(prefix_start) = is_xml_name_start(prefix);

        match prefix_start {
          crate::NameStart::Invalid => {
            let Ok(gen_text_pos_from) = self.gen_text_pos_from(start);
            return Err(Error::InvalidName(gen_text_pos_from));
          }
          crate::NameStart::Valid => {}
        }
      }
    }

    let Ok(local_start) = is_xml_name_start(local);

    match local_start {
      crate::NameStart::Invalid => {
        let Ok(gen_text_pos_from) = self.gen_text_pos_from(start);
        return Err(Error::InvalidName(gen_text_pos_from));
      }
      crate::NameStart::Valid => {}
    }

    Ok((prefix, local))
  }

  fn consume_unicode_name_character(&mut self) -> Result<Scan> {
    let mut chars = self.chars()?;
    let character = match chars.next() {
      Some(character) => character,
      None => return Ok(Scan::Stop),
    };

    match CHAR_NAME(character) {
      NameCharacter::Valid => {
        let Ok(width) = console_core_number_conversion::fitted::<_, u64>(character.len_utf8());
        let Ok(()) = self.advance(width);

        Ok(Scan::Advance)
      }
      NameCharacter::Invalid => Ok(Scan::Stop),
    }
  }

  fn consume_eq(&mut self) -> Result<()> {
    let Ok(()) = self.skip_spaces();
    self.consume_byte(b'=')?;
    let Ok(()) = self.skip_spaces();

    Ok(())
  }

  fn consume_quote(&mut self) -> Result<u8> {
    let c = self.curr_byte()?;

    match c {
      b'\'' | b'"' => {
        let Ok(()) = self.advance(1);
        Ok(c)
      }
      _ => {
        let Ok(gen_text_pos) = self.gen_text_pos();

        Err(Error::InvalidChar2("a quote", c, gen_text_pos))
      }
    }
  }

  pub fn gen_text_pos(&self) -> core::result::Result<TextPos, console_core_never::Never> {
    let Ok(position) = self.pos();

    self.gen_text_pos_from(position)
  }

  pub fn gen_text_pos_from(
    &self,
    position: u64,
  ) -> core::result::Result<TextPos, console_core_never::Never> {
    let Ok(row) = Self::calc_curr_row(self.span.text, position);
    let Ok(col) = Self::calc_curr_col(self.span.text, position);

    TextPos::new(crate::Row(row), crate::Column(col))
  }

  fn calc_curr_row(text: &str, end: u64) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(limit) = console_core_number_conversion::index(end);
    let Ok(lines) = console_core_number_conversion::fitted::<_, u32>(
      text
        .bytes()
        .take(limit)
        .filter(|byte| *byte == b'\n')
        .count(),
    );

    Ok(lines.saturating_add(1))
  }

  fn calc_curr_col(text: &str, end: u64) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(limit) = console_core_number_conversion::index(end);
    let Ok(columns) = console_core_number_conversion::fitted::<_, u32>(
      text
        .bytes()
        .take(limit)
        .rev()
        .take_while(|byte| *byte != b'\n')
        .filter(|byte| *byte & 0xc0 != 0x80)
        .count(),
    );

    Ok(columns.saturating_add(1))
  }
}
