use crate::xkb::code::Code;
use crate::xkb::code_slice::CodeSlice;
use crate::xkb::diagnostic::DiagnosticKind;
use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use crate::xkb::rmlvo::token::Punctuation;
use crate::xkb::rmlvo::token::Token;
use crate::xkb::span::SpanExt;
use crate::xkb::span::SpanUnit;
use crate::xkb::span::Spanned;
use crate::xkb::whitespace::Newline;
use crate::xkb::whitespace::consume_whitespace;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) struct Lexer {
  path: PathBuf,
  code: Code,
  span_lo: SpanUnit,
  pos: u32,
}

struct LineLexer<'a> {
  code: CodeSlice<'a>,
  interner: &'a mut Interner,
  span_lo: SpanUnit,
  pos: u32,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum LexerError {
  EmptyMacroName,
  UnexpectedByte(u8),
}

impl std::fmt::Display for LexerError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      LexerError::EmptyMacroName => f.write_str("empty macro name"),
      LexerError::UnexpectedByte(byte) => write!(f, "Unexpected byte {:?}", char::from(*byte)),
    }
  }
}

impl std::error::Error for LexerError {}

impl LexerError {
  pub(crate) fn diagnostic_kind(&self) -> Result<DiagnosticKind, Never> {
    Ok(match self {
      LexerError::EmptyMacroName => DiagnosticKind::EmptyMacroName,
      LexerError::UnexpectedByte(_) => DiagnosticKind::UnlexableByte,
    })
  }
}

enum One {
  Eof,
  Eol,
  Token(Spanned<Token>),
}

enum Ahead {
  End,
  Comment,
  Byte(u8),
}

impl Lexer {
  pub(crate) fn new(path: PathBuf, code: Code, span_lo: SpanUnit) -> Result<Self, Never> {
    Ok(Self {
      path,
      code,
      span_lo,
      pos: 0,
    })
  }

  pub(crate) fn lex_line(
    &mut self,
    interner: &mut Interner,
    output: &mut Vec<Spanned<Token>>,
  ) -> Result<(), Spanned<LexerError>> {
    let Ok(to_slice) = self.code.to_slice();
    let mut lexer = LineLexer {
      code: to_slice,
      interner,
      span_lo: self.span_lo,
      pos: self.pos,
    };
    lexer.lex_line(output)?;
    self.pos = lexer.pos;
    Ok(())
  }

  pub(crate) fn path(&self) -> Result<&Path, Never> {
    Ok(&self.path)
  }

  pub(crate) fn steps_left(&self) -> Result<u32, Never> {
    let Ok(length) = fitted::<_, u32>(self.code.len());
    Ok(length.saturating_sub(self.pos).saturating_add(1))
  }
}

impl LineLexer<'_> {
  fn lex_line(&mut self, output: &mut Vec<Spanned<Token>>) -> Result<(), Spanned<LexerError>> {
    let mut lexed_any = false;
    let Ok(steps) = self.steps_left();

    for _ in 0..steps {
      let one = self.lex_one()?;

      match one {
        One::Eof => return Ok(()),
        One::Eol => match ::core::primitive::bool::then_some(lexed_any, ()) {
          ::core::option::Option::Some(()) => return Ok(()),
          ::core::option::Option::None => {}
        },
        One::Token(t) => {
          lexed_any = true;
          output.push(t);
        }
      }
    }

    Ok(())
  }

  fn lex_one(&mut self) -> Result<One, Spanned<LexerError>> {
    let ahead = self.skip_blanks()?;

    let b = match ahead {
      Ahead::End => return Ok(One::Eof),
      Ahead::Comment => return Ok(One::Eol),
      Ahead::Byte(b) => b,
    };
    let start = self.pos;
    let lo = self.span_lo.saturating_add(start);
    self.pos = start.saturating_add(1);

    let punctuation = match b {
      b'\n' => return Ok(One::Eol),
      b'=' => Punctuation::Equals,
      b'*' => Punctuation::Times,
      b'!' => Punctuation::Exclam,
      _ => return self.lex_name(b, start),
    };

    Ok(One::Token(
      Token::Punctuation(punctuation).spanned(lo, lo.saturating_add(1)),
    ))
  }

  fn lex_name(&mut self, first: u8, start: u32) -> Result<One, Spanned<LexerError>> {
    let lo = self.span_lo.saturating_add(start);
    let (from, token): (u32, fn(Interned) -> Token) =
      match ::core::primitive::bool::then_some(first == b'$', ()) {
        ::core::option::Option::Some(()) => (start.saturating_add(1), Token::GroupName),
        ::core::option::Option::None => (start, Token::Ident),
      };
    let Ok(rest) = self.rest(from);

    let found = match rest.first() {
      Some(found) => *found,
      None => return Err(LexerError::EmptyMacroName.spanned(lo, lo.saturating_add(1))),
    };
    let Ok(run) = fitted::<_, u32>(
      rest
        .iter()
        .take_while(|b| matches!(**b, b'!'..=b'[' | b']'..=b'~'))
        .count(),
    );

    match ::core::primitive::bool::then_some(run == 0, ()) {
      ::core::option::Option::Some(()) => {
        let at = self.span_lo.saturating_add(from);
        return Err(LexerError::UnexpectedByte(found).spanned(at, at.saturating_add(1)));
      }
      ::core::option::Option::None => {}
    }

    let end = from.saturating_add(run);
    self.pos = end;
    let Ok(slice) = self.code.slice(from..end);
    let Ok(value) = self.interner.intern(&slice);
    let hi = self.span_lo.saturating_add(end);
    Ok(One::Token(token(value).spanned(lo, hi)))
  }

  fn skip_blanks(&mut self) -> Result<Ahead, Spanned<LexerError>> {
    let Ok(steps) = self.steps_left();

    for _ in 0..steps {
      let Ok(()) = self.skip_whitespace();
      let Ok(here) = self.byte_at(self.pos);

      let b = match here {
        Some(b) => b,
        None => return Ok(Ahead::End),
      };

      match b {
        b'/' => {
          let Ok(after_slash) = self.after_slash();
          return Ok(after_slash);
        }
        b'\\' => self.continue_line()?,
        _ => return Ok(Ahead::Byte(b)),
      }
    }

    Ok(Ahead::End)
  }

  fn after_slash(&mut self) -> Result<Ahead, Never> {
    let Ok(next) = self.byte_at(self.pos.saturating_add(1));

    match ::core::primitive::bool::then_some(next == Some(b'/'), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(Ahead::Byte(b'/')),
    }

    let comment = self.pos.saturating_add(2);
    let Ok(rest) = self.rest(comment);

    let Ok(skipped) = match rest.iter().position(|b| *b == b'\n') {
      Some(newline) => fitted::<_, u32>(newline.saturating_add(1)),
      None => fitted::<_, u32>(rest.len()),
    };
    self.pos = comment.saturating_add(skipped);
    Ok(Ahead::Comment)
  }

  fn continue_line(&mut self) -> Result<(), Spanned<LexerError>> {
    self.pos = self.pos.saturating_add(1);
    let Ok(here) = self.byte_at(self.pos);

    match ::core::primitive::bool::then_some(here == Some(b'\r'), ()) {
      ::core::option::Option::Some(()) => self.pos = self.pos.saturating_add(1),
      ::core::option::Option::None => {}
    }

    let Ok(after) = self.byte_at(self.pos);

    match after {
      Some(b'\n') => self.pos = self.pos.saturating_add(1),
      Some(b) => {
        let lo = self.span_lo.saturating_add(self.pos);
        return Err(LexerError::UnexpectedByte(b).spanned(lo, lo.saturating_add(1)));
      }
      None => {}
    }

    Ok(())
  }

  fn skip_whitespace(&mut self) -> Result<(), Never> {
    let Ok(()) = consume_whitespace(&mut self.pos, &self.code, Newline::Significant);
    Ok(())
  }

  fn byte_at(&self, at: u32) -> Result<Option<u8>, Never> {
    let Ok(position) = index(at);
    Ok(self.code.get(position).copied())
  }

  fn rest(&self, from: u32) -> Result<&[u8], Never> {
    let Ok(position) = index(from);

    Ok(match self.code.get(position..) {
      Some(rest) => rest,
      None => &[],
    })
  }

  fn steps_left(&self) -> Result<u32, Never> {
    let Ok(length) = fitted::<_, u32>(self.code.len());
    Ok(length.saturating_sub(self.pos).saturating_add(1))
  }
}
