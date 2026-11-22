use crate::xkb::interner::Interned;
use crate::xkb::kccgst::parser::error::Expected;
use console_core_never::Never;
use console_input_keymaps_derive::CloneWithDelta;
use std::slice;

#[derive(Copy, Clone, Debug, PartialEq, CloneWithDelta)]
pub(crate) enum Token {
  Ident(Interned),
  String(Interned),
  KeyName(Interned),
  Integer(Interned, i64),
  Float(Interned, f64),
  Punctuation(Punctuation),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, CloneWithDelta)]
pub(crate) enum Punctuation {
  Semicolon,
  Obrace,
  Cbrace,
  Equals,
  Obracket,
  Cbracket,
  Oparen,
  Cparen,
  Dot,
  Comma,
  Plus,
  Minus,
  Times,
  Divide,
  Exclam,
  Invert,
}

impl PartialEq<Punctuation> for Token {
  fn eq(&self, other: &Punctuation) -> bool {
    *self == Token::Punctuation(*other)
  }
}

impl Punctuation {
  pub(crate) fn expected(self) -> Result<&'static [Expected], Never> {
    let expected: &'static Expected = match self {
      Self::Semicolon => &Expected::Punctuation(Self::Semicolon),
      Self::Obrace => &Expected::Punctuation(Self::Obrace),
      Self::Cbrace => &Expected::Punctuation(Self::Cbrace),
      Self::Equals => &Expected::Punctuation(Self::Equals),
      Self::Obracket => &Expected::Punctuation(Self::Obracket),
      Self::Cbracket => &Expected::Punctuation(Self::Cbracket),
      Self::Oparen => &Expected::Punctuation(Self::Oparen),
      Self::Cparen => &Expected::Punctuation(Self::Cparen),
      Self::Dot => &Expected::Punctuation(Self::Dot),
      Self::Comma => &Expected::Punctuation(Self::Comma),
      Self::Plus => &Expected::Punctuation(Self::Plus),
      Self::Minus => &Expected::Punctuation(Self::Minus),
      Self::Times => &Expected::Punctuation(Self::Times),
      Self::Divide => &Expected::Punctuation(Self::Divide),
      Self::Exclam => &Expected::Punctuation(Self::Exclam),
      Self::Invert => &Expected::Punctuation(Self::Invert),
    };

    Ok(slice::from_ref(expected))
  }
}

impl From<Punctuation> for Token {
  fn from(value: Punctuation) -> Self {
    Self::Punctuation(value)
  }
}
