use crate::xkb::code::Code;
use crate::xkb::code_slice::CodeSlice;
use crate::xkb::diagnostic::DiagnosticKind;
use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use crate::xkb::kccgst::token::Punctuation;
use crate::xkb::kccgst::token::Token;
use crate::xkb::span::Span;
use crate::xkb::span::SpanExt;
use crate::xkb::span::SpanUnit;
use crate::xkb::span::Spanned;
use crate::xkb::whitespace::Newline;
use crate::xkb::whitespace::consume_whitespace;
use console_core_never::Never;
use console_core_number_conversion::FromBytes;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use std::num::ParseFloatError;
use std::ops::ControlFlow;
use std::ops::Range;
use std::str::FromStr;

#[derive(Debug)]
pub(crate) struct Lexer {
  code: Code,
  span_lo: SpanUnit,
  pos: SpanUnit,
}

struct ItemLexer<'a> {
  code: CodeSlice<'a>,
  interner: &'a mut Interner,
  span_lo: SpanUnit,
  pos: SpanUnit,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum LexerError {
  UnterminatedKeyName,
  UnterminatedString,
  InvalidFloatLiteral(ParseFloatError),
  InvalidIntegerLiteral,
  UnexpectedByte(u8),
}

impl std::fmt::Display for LexerError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      LexerError::UnterminatedKeyName => f.write_str("unterminated key name"),
      LexerError::UnterminatedString => f.write_str("unterminated string"),
      LexerError::InvalidFloatLiteral(_) => f.write_str("invalid float literal"),
      LexerError::InvalidIntegerLiteral => f.write_str("invalid integer literal"),
      LexerError::UnexpectedByte(byte) => write!(f, "unexpected byte {:?}", char::from(*byte)),
    }
  }
}

impl std::error::Error for LexerError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      LexerError::InvalidFloatLiteral(float) => Some(float),
      LexerError::UnterminatedKeyName
      | LexerError::UnterminatedString
      | LexerError::InvalidIntegerLiteral
      | LexerError::UnexpectedByte(_) => None,
    }
  }
}

impl LexerError {
  pub(crate) fn diagnostic_kind(&self) -> Result<DiagnosticKind, Never> {
    Ok(match self {
      LexerError::UnterminatedKeyName => DiagnosticKind::UnterminatedKeyName,
      LexerError::UnterminatedString => DiagnosticKind::UnterminatedString,
      LexerError::InvalidFloatLiteral(_) => DiagnosticKind::InvalidFloatLiteral,
      LexerError::InvalidIntegerLiteral => DiagnosticKind::InvalidIntegerLiteral,
      LexerError::UnexpectedByte(_) => DiagnosticKind::UnlexableByte,
    })
  }
}

impl Lexer {
  pub(crate) fn new(code: Code, span_lo: SpanUnit) -> Result<Self, Never> {
    Ok(Self {
      code,
      span_lo,
      pos: 0,
    })
  }

  pub(crate) fn lex_item(
    &mut self,
    interner: &mut Interner,
    output: &mut Vec<Spanned<Token>>,
  ) -> Result<(), Spanned<LexerError>> {
    let Ok(to_slice) = self.code.to_slice();
    let mut lexer = ItemLexer {
      code: to_slice,
      interner,
      span_lo: self.span_lo,
      pos: self.pos,
    };
    lexer.lex_item(output)?;
    self.pos = lexer.pos;
    Ok(())
  }

  pub(crate) fn code(&self) -> Result<&Code, Never> {
    Ok(&self.code)
  }

  pub(crate) fn span(&self) -> Result<Span, Never> {
    let Ok(length) = fitted::<_, SpanUnit>(self.code.len());
    Ok(Span {
      lo: self.span_lo,
      hi: self.span_lo.saturating_add(length),
    })
  }
}

#[derive(Copy, Clone)]
enum Nesting {
  Open,
  Close,
  End,
  Within,
}

fn nesting(token: Token) -> Result<Nesting, Never> {
  Ok(match token {
    Token::Punctuation(Punctuation::Obrace) => Nesting::Open,
    Token::Punctuation(Punctuation::Cbrace) => Nesting::Close,
    Token::Punctuation(Punctuation::Semicolon) => Nesting::End,
    Token::Punctuation(
      Punctuation::Equals
      | Punctuation::Obracket
      | Punctuation::Cbracket
      | Punctuation::Oparen
      | Punctuation::Cparen
      | Punctuation::Dot
      | Punctuation::Comma
      | Punctuation::Plus
      | Punctuation::Minus
      | Punctuation::Times
      | Punctuation::Divide
      | Punctuation::Exclam
      | Punctuation::Invert,
    )
    | Token::Ident(_)
    | Token::String(_)
    | Token::KeyName(_)
    | Token::Integer(..)
    | Token::Float(..) => Nesting::Within,
  })
}

struct Gaps<'c> {
  code: &'c [u8],
  at: SpanUnit,
}

impl Iterator for Gaps<'_> {
  type Item = SpanUnit;

  fn next(&mut self) -> Option<SpanUnit> {
    let mut position = self.at;
    let Ok(()) = consume_whitespace(&mut position, self.code, Newline::Space);
    let Ok(from) = index(position);
    let rest = self.code.get(from..)?;
    let after = match rest {
      [b'#', ..] | [b'/', b'/', ..] => match rest.iter().position(|byte| *byte == b'\n') {
        Some(newline) => rest.get(newline..),
        None => rest.get(rest.len()..),
      },
      [..] => Some(rest),
    }?;
    let Ok(reached) = fitted::<_, SpanUnit>(self.code.len().saturating_sub(after.len()));

    match ::core::primitive::bool::then_some(reached == self.at, ()) {
      ::core::option::Option::Some(()) => None,
      ::core::option::Option::None => {
        self.at = reached;
        Some(reached)
      }
    }
  }
}

fn gap_end(code: &[u8], from: SpanUnit) -> Result<SpanUnit, Never> {
  let gaps = Gaps { code, at: from };

  Ok(match gaps.last() {
    Some(end) => end,
    None => from,
  })
}

struct At<'c, 'k> {
  code: &'c CodeSlice<'k>,
  span_lo: SpanUnit,
  start: SpanUnit,
}

impl At<'_, '_> {
  fn spanned(&self, token: Token, length: SpanUnit) -> Result<Spanned<Token>, Never> {
    let lo = self.span_lo.saturating_add(self.start);
    Ok(token.spanned(lo, lo.saturating_add(length)))
  }

  fn fault(&self, fault: LexerError, length: SpanUnit) -> Result<Spanned<LexerError>, Never> {
    let lo = self.span_lo.saturating_add(self.start);
    Ok(fault.spanned(lo, lo.saturating_add(length)))
  }

  fn bytes_from(&self, offset: SpanUnit) -> Result<&[u8], Never> {
    let Ok(lo) = index(self.start.saturating_add(offset));

    Ok(match self.code.get(lo..) {
      Some(bytes) => bytes,
      None => &[],
    })
  }

  fn bytes_within(&self, within: Range<SpanUnit>) -> Result<&[u8], Never> {
    let Ok(lo) = index(self.start.saturating_add(within.start));
    let Ok(hi) = index(self.start.saturating_add(within.end));

    Ok(match self.code.get(lo..hi) {
      Some(bytes) => bytes,
      None => &[],
    })
  }

  fn intern(&self, interner: &mut Interner, within: Range<SpanUnit>) -> Result<Interned, Never> {
    let lo = self.start.saturating_add(within.start);
    let hi = self.start.saturating_add(within.end);
    let Ok(slice) = self.code.slice(lo..hi);
    interner.intern(&slice)
  }
}

struct Lexed {
  token: Spanned<Token>,
  length: SpanUnit,
}

fn run_length(bytes: &[u8], continues: impl Fn(&u8) -> bool) -> Result<SpanUnit, Never> {
  fitted(match bytes.iter().position(|byte| !continues(byte)) {
    Some(length) => length,
    None => bytes.len(),
  })
}

fn punctuation(byte: u8) -> Result<Option<Punctuation>, Never> {
  Ok(match byte {
    b';' => Some(Punctuation::Semicolon),
    b'{' => Some(Punctuation::Obrace),
    b'}' => Some(Punctuation::Cbrace),
    b'=' => Some(Punctuation::Equals),
    b'[' => Some(Punctuation::Obracket),
    b']' => Some(Punctuation::Cbracket),
    b'(' => Some(Punctuation::Oparen),
    b')' => Some(Punctuation::Cparen),
    b'.' => Some(Punctuation::Dot),
    b',' => Some(Punctuation::Comma),
    b'+' => Some(Punctuation::Plus),
    b'-' => Some(Punctuation::Minus),
    b'*' => Some(Punctuation::Times),
    b'/' => Some(Punctuation::Divide),
    b'!' => Some(Punctuation::Exclam),
    b'~' => Some(Punctuation::Invert),
    _ => None,
  })
}

fn key_name(at: &At<'_, '_>, interner: &mut Interner) -> Result<Lexed, Spanned<LexerError>> {
  let Ok(body) = at.bytes_from(1);

  match body
    .iter()
    .position(|byte| *byte == b'>' || !(b'!'..=b'~').contains(byte))
  {
    Some(contents) => {
      let Ok(length) = fitted::<_, SpanUnit>(contents);

      match body.get(contents) {
        Some(b'>') => {
          let Ok(name) = at.intern(interner, 1..length.saturating_add(1));
          let total = length.saturating_add(2);
          let Ok(token) = at.spanned(Token::KeyName(name), total);
          Ok(Lexed {
            token,
            length: total,
          })
        }
        Some(_) | None => {
          let Ok(fault) = at.fault(LexerError::UnterminatedKeyName, length.saturating_add(1));
          Err(fault)
        }
      }
    }
    None => {
      let Ok(length) = fitted::<_, SpanUnit>(body.len());
      let Ok(fault) = at.fault(LexerError::UnterminatedKeyName, length.saturating_add(1));
      Err(fault)
    }
  }
}

#[derive(Copy, Clone)]
enum Escape {
  Plain,
  Escaped,
}

fn string(at: &At<'_, '_>, interner: &mut Interner) -> Result<Lexed, Spanned<LexerError>> {
  let Ok(body) = at.bytes_from(1);
  let closing = body
    .iter()
    .enumerate()
    .try_fold(Escape::Plain, |escape, (offset, byte)| {
      match (escape, *byte) {
        (Escape::Plain, b'"') => {
          let Ok(length) = fitted::<_, SpanUnit>(offset);
          ControlFlow::Break(length)
        }
        (Escape::Plain, b'\\') => ControlFlow::Continue(Escape::Escaped),
        (Escape::Plain | Escape::Escaped, _) => ControlFlow::Continue(Escape::Plain),
      }
    });

  match closing {
    ControlFlow::Break(length) => {
      let Ok(text) = at.intern(interner, 1..length.saturating_add(1));
      let total = length.saturating_add(2);
      let Ok(token) = at.spanned(Token::String(text), total);
      Ok(Lexed {
        token,
        length: total,
      })
    }
    ControlFlow::Continue(_) => {
      let Ok(length) = fitted::<_, SpanUnit>(body.len());
      let Ok(fault) = at.fault(LexerError::UnterminatedString, length.saturating_add(1));
      Err(fault)
    }
  }
}

fn ident(at: &At<'_, '_>, interner: &mut Interner) -> Result<Lexed, Spanned<LexerError>> {
  let Ok(after_first) = at.bytes_from(1);
  let Ok(tail) = run_length(
    after_first,
    |byte| matches!(byte, b'_' | b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9'),
  );
  let length = tail.saturating_add(1);
  let Ok(name) = at.intern(interner, 0..length);
  let Ok(token) = at.spanned(Token::Ident(name), length);
  Ok(Lexed { token, length })
}

#[derive(Copy, Clone)]
enum NumberShape {
  Decimal,
  Hexadecimal,
  Float,
}

struct Measured {
  shape: NumberShape,
  length: SpanUnit,
}

fn measure_number(at: &At<'_, '_>) -> Result<Measured, Never> {
  let Ok(written) = at.bytes_from(0);

  match written {
    [b'0', b'x', hexadecimal @ ..] => {
      let Ok(digits) = run_length(hexadecimal, u8::is_ascii_hexdigit);
      Ok(Measured {
        shape: NumberShape::Hexadecimal,
        length: digits.saturating_add(2),
      })
    }
    [_, decimal @ ..] => {
      let Ok(integer_digits) = run_length(decimal, u8::is_ascii_digit);
      let whole = integer_digits.saturating_add(1);
      let Ok(after_whole) = at.bytes_from(whole);

      match after_whole {
        [b'.', fraction @ ..] => {
          let Ok(fraction_digits) = run_length(fraction, u8::is_ascii_digit);
          Ok(Measured {
            shape: NumberShape::Float,
            length: whole.saturating_add(1).saturating_add(fraction_digits),
          })
        }
        [..] => Ok(Measured {
          shape: NumberShape::Decimal,
          length: whole,
        }),
      }
    }
    [] => Ok(Measured {
      shape: NumberShape::Decimal,
      length: 0,
    }),
  }
}

fn number(at: &At<'_, '_>, interner: &mut Interner) -> Result<Lexed, Spanned<LexerError>> {
  let Ok(measured) = measure_number(at);
  let Ok(text) = at.intern(interner, 0..measured.length);
  let Ok(written) = at.bytes_within(0..measured.length);
  let parsed = match measured.shape {
    NumberShape::Float => {
      let digits: String = written.iter().map(|byte| char::from(*byte)).collect();
      f64::from_str(&digits)
        .map(|value| Token::Float(text, value))
        .map_err(LexerError::InvalidFloatLiteral)
    }
    NumberShape::Decimal => i64::from_bytes_dec(written)
      .map(|value| Token::Integer(text, value))
      .ok_or(LexerError::InvalidIntegerLiteral),
    NumberShape::Hexadecimal => {
      let Ok(digits) = at.bytes_within(2..measured.length);
      i64::from_bytes_hex(digits)
        .map(|value| Token::Integer(text, value))
        .ok_or(LexerError::InvalidIntegerLiteral)
    }
  };

  match parsed {
    Ok(token) => {
      let Ok(spanned) = at.spanned(token, measured.length);
      Ok(Lexed {
        token: spanned,
        length: measured.length,
      })
    }
    Err(fault) => {
      let Ok(spanned) = at.fault(fault, measured.length);
      Err(spanned)
    }
  }
}

fn lex_token(
  at: &At<'_, '_>,
  interner: &mut Interner,
  first: u8,
) -> Result<Lexed, Spanned<LexerError>> {
  let Ok(single) = punctuation(first);

  match (single, first) {
    (Some(mark), _) => {
      let Ok(token) = at.spanned(Token::Punctuation(mark), 1);
      Ok(Lexed { token, length: 1 })
    }
    (None, b'<') => key_name(at, interner),
    (None, b'"') => string(at, interner),
    (None, b'_' | b'a'..=b'z' | b'A'..=b'Z') => ident(at, interner),
    (None, b'0'..=b'9') => number(at, interner),
    (None, unexpected) => {
      let Ok(fault) = at.fault(LexerError::UnexpectedByte(unexpected), 1);
      Err(fault)
    }
  }
}

impl ItemLexer<'_> {
  fn lex_item(&mut self, output: &mut Vec<Spanned<Token>>) -> Result<(), Spanned<LexerError>> {
    let mut depth = 0_u64;

    for lexing in self.by_ref() {
      let token = lexing?;
      output.push(token);
      let Ok(nested) = nesting(token.val);

      match (nested, depth) {
        (Nesting::Open, _) => depth = depth.saturating_add(1),
        (Nesting::Close, _) => depth = depth.saturating_sub(1),
        (Nesting::End, 0) => break,
        (Nesting::End | Nesting::Within, _) => {}
      }
    }

    Ok(())
  }

  fn lex_one(&mut self) -> Result<Option<Spanned<Token>>, Spanned<LexerError>> {
    let Ok(start) = gap_end(&self.code, self.pos);
    self.pos = start;
    let at = At {
      code: &self.code,
      span_lo: self.span_lo,
      start,
    };
    let Ok(rest) = at.bytes_from(0);
    let scanned = match rest {
      [] => return Ok(None),
      [first, ..] => lex_token(&at, self.interner, *first)?,
    };
    self.pos = start.saturating_add(scanned.length);
    Ok(Some(scanned.token))
  }
}

impl Iterator for ItemLexer<'_> {
  type Item = Result<Spanned<Token>, Spanned<LexerError>>;

  fn next(&mut self) -> Option<Self::Item> {
    self.lex_one().transpose()
  }
}
