use crate::xkb::code::display_text;
use crate::xkb::code_slice::CodeSlice;
use crate::xkb::diagnostic::DiagnosticKind;
use crate::xkb::interner::Interned;
use crate::xkb::rmlvo::parser::Parser;
use crate::xkb::rmlvo::token::Punctuation;
use crate::xkb::rmlvo::token::Token;
use crate::xkb::span::Span;
use crate::xkb::span::SpanExt;
use crate::xkb::span::Spanned;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use std::fmt;
use std::fmt::Formatter;

#[derive(Debug, Clone)]
pub(crate) enum ParserError {
  ExpectedButEof(ExpectedButEof),
  UnexpectedToken(UnexpectedToken),
  ExpectedEol(ActualToken),
  IndexStart(u8),
  IndexEnd(u8),
  Index(CodeSlice<'static>),
}

impl fmt::Display for ParserError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      ParserError::ExpectedButEof(eof) => write!(
        f,
        "expected {}, but encountered EOF",
        fmt::from_fn(|f_2| write_expected(f_2, eof.expected))
      ),
      ParserError::UnexpectedToken(unexpected) => write!(
        f,
        "expected {}, but found {}",
        fmt::from_fn(|f_3| write_expected(f_3, unexpected.expected)),
        fmt::from_fn(|f_4| write_actual(f_4, &unexpected.actual)),
      ),
      ParserError::ExpectedEol(actual) => write!(
        f,
        "expected end of line, but found {}",
        fmt::from_fn(|f_5| write_actual(f_5, actual))
      ),
      ParserError::IndexStart(byte) => {
        write!(f, "expected `[` but found `{}`", char::from(*byte))
      }
      ParserError::IndexEnd(byte) => write!(f, "expected `]` but found `{}`", char::from(*byte)),
      ParserError::Index(written) => {
        let Ok(text) = display_text(written);
        write!(f, "expected `single`, `first`, `later`, `any`, or a group index, but found `{}`", text)
      }
    }
  }
}

impl std::error::Error for ParserError {}

impl ParserError {
  pub(crate) fn diagnostic_kind(&self) -> Result<DiagnosticKind, Never> {
    Ok(match self {
      ParserError::ExpectedButEof(_) => DiagnosticKind::UnexpectedEof,
      ParserError::UnexpectedToken(_) => DiagnosticKind::UnexpectedToken,
      ParserError::ExpectedEol(_) => DiagnosticKind::ExpectedEol,
      ParserError::IndexStart(_) => DiagnosticKind::ExpectedIndexStart,
      ParserError::IndexEnd(_) => DiagnosticKind::ExpectedIndexEnd,
      ParserError::Index(_) => DiagnosticKind::InvalidMatcherIndex,
    })
  }
}

fn write_actual(f: &mut Formatter<'_>, actual: &ActualToken) -> fmt::Result {
  match actual {
    ActualToken::Ident(i) => {
      let Ok(bytes) = i.as_bytes();
      let Ok(text) = display_text(bytes);
      write!(f, "`{}`", text)
    }
    ActualToken::GroupName(i) => {
      let Ok(bytes) = i.as_bytes();
      let Ok(text) = display_text(bytes);
      write!(f, "`${}`", text)
    }
    ActualToken::Token(t) => match t {
      Token::Ident(_) => f.write_str("an identifier"),
      Token::GroupName(_) => f.write_str("a group name"),
      Token::Punctuation(p) => {
        let Ok(punctuation) = punctuation_string(*p);
        write!(f, "`{}`", punctuation)
      }
    },
  }
}

fn write_expected(f: &mut Formatter<'_>, expected: &[Expected]) -> fmt::Result {
  let (last, before) = match expected.split_last() {
    Some(split) => split,
    None => return Ok(()),
  };

  for one in before {
    write_single_expected(f, one)?;
    f.write_str(", ")?;
  }

  match ::core::primitive::bool::then_some(before.is_empty(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => f.write_str("or ")?,
  }

  write_single_expected(f, last)
}

fn write_single_expected(f: &mut Formatter<'_>, expected: &Expected) -> fmt::Result {
  match expected {
    Expected::Ident(i) => write!(f, "`{}`", i),
    Expected::AnyIdent => f.write_str("an identifier"),
    Expected::AnyGroupName => f.write_str("a group name"),
    Expected::Punctuation(p) => {
      let Ok(punctuation) = punctuation_string(*p);
      write!(f, "`{}`", punctuation)
    }
  }
}

fn punctuation_string(p: Punctuation) -> Result<&'static str, Never> {
  Ok(match p {
    Punctuation::Equals => "=",
    Punctuation::Times => "*",
    Punctuation::Exclam => "!",
  })
}

#[derive(Debug, Clone)]
pub(crate) struct ExpectedButEof {
  expected: &'static [Expected],
}

#[derive(Debug, Clone)]
pub(crate) struct UnexpectedToken {
  expected: &'static [Expected],
  actual: ActualToken,
}

#[derive(Debug)]
pub(crate) enum Expected {
  Ident(&'static str),
  AnyIdent,
  AnyGroupName,
  Punctuation(Punctuation),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ActualToken {
  Ident(CodeSlice<'static>),
  GroupName(CodeSlice<'static>),
  Token(Token),
}

impl Parser<'_, '_, '_> {
  pub(super) fn expected_but_eof(
    &self,
    span: Span,
    expected: &'static [Expected],
  ) -> Result<Spanned<ParserError>, Never> {
    Ok(ParserError::ExpectedButEof(ExpectedButEof { expected }).spanned2(span))
  }

  fn token_to_actual(&self, token: Spanned<Token>) -> Result<ActualToken, Never> {
    Ok(match token.val {
      Token::Ident(i) => {
        let Ok(interned) = self.interner.get(i);
        let Ok(owned) = interned.to_owned();
        ActualToken::Ident(owned)
      }
      Token::GroupName(i) => {
        let Ok(interned) = self.interner.get(i);
        let Ok(owned) = interned.to_owned();
        ActualToken::GroupName(owned)
      }
      Token::Punctuation(_) => ActualToken::Token(token.val),
    })
  }

  pub(super) fn expected_eol(
    &self,
    token: Spanned<Token>,
  ) -> Result<Spanned<ParserError>, Never> {
    let Ok(actual) = self.token_to_actual(token);
    Ok(ParserError::ExpectedEol(actual).spanned2(token.span))
  }

  pub(super) fn unexpected_token(
    &self,
    expected: &'static [Expected],
    token: Spanned<Token>,
  ) -> Result<Spanned<ParserError>, Never> {
    let Ok(actual) = self.token_to_actual(token);
    Ok(ParserError::UnexpectedToken(UnexpectedToken { expected, actual }).spanned2(token.span))
  }

  pub(super) fn index_start(
    &self,
    ident: Spanned<Interned>,
    offset: u32,
    found: u8,
  ) -> Result<Spanned<ParserError>, Never> {
    let lo = ident.span.lo.saturating_add(offset);
    Ok(ParserError::IndexStart(found).spanned(lo, lo.saturating_add(1)))
  }

  pub(super) fn index_end(
    &self,
    ident: Spanned<Interned>,
    last: u8,
  ) -> Result<Spanned<ParserError>, Never> {
    let hi = ident.span.hi;
    Ok(ParserError::IndexEnd(last).spanned(hi.saturating_sub(1), hi))
  }

  pub(super) fn index(
    &self,
    ident: Spanned<Interned>,
    offset: u32,
  ) -> Result<Spanned<ParserError>, Never> {
    let Ok(interned) = self.interner.get(ident.val);
    let Ok(length) = fitted::<_, u32>(interned.len());
    let Ok(actual) = interned.slice(offset.saturating_add(1)..length.saturating_sub(1));
    let lo = ident.span.lo.saturating_add(offset).saturating_add(1);
    let hi = ident.span.hi.saturating_sub(1);
    let Ok(owned) = actual.to_owned();
    Ok(ParserError::Index(owned).spanned(lo, hi))
  }
}

pub(super) const ANY_IDENT: &[Expected] = &[Expected::AnyIdent];

pub(super) const START_OF_LINE: &[Expected] = &[
  Expected::Punctuation(Punctuation::Exclam),
  Expected::Punctuation(Punctuation::Times),
  Expected::AnyIdent,
];

pub(super) const AFTER_EXCLAM: &[Expected] = &[
  Expected::Ident("include"),
  Expected::Punctuation(Punctuation::Times),
  Expected::Ident("model"),
  Expected::Ident("layout"),
  Expected::Ident("variant"),
  Expected::Ident("option"),
];

pub(super) const RULE_KEY: &[Expected] = &[
  Expected::Punctuation(Punctuation::Times),
  Expected::Punctuation(Punctuation::Equals),
  Expected::AnyIdent,
  Expected::AnyGroupName,
];

pub(super) const MLVO: &[Expected] = &[
  Expected::Ident("model"),
  Expected::Ident("layout"),
  Expected::Ident("variant"),
  Expected::Ident("option"),
];

pub(super) const MAPPING_KEY: &[Expected] = &[
  Expected::Ident("model"),
  Expected::Ident("layout"),
  Expected::Ident("variant"),
  Expected::Ident("option"),
  Expected::Punctuation(Punctuation::Equals),
];

pub(super) const MAPPING_VALUE: &[Expected] = &[
  Expected::Ident("keycodes"),
  Expected::Ident("symbols"),
  Expected::Ident("types"),
  Expected::Ident("compat"),
  Expected::Ident("geometry"),
];
