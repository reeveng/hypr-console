use crate::xkb::interner::Interned;
use crate::xkb::rmlvo::parser::error::Expected;
use console_core_never::Never;
use console_input_keymaps_derive::CloneWithDelta;
use std::slice;

#[derive(Copy, Clone, Debug, PartialEq, CloneWithDelta)]
pub(crate) enum Token {
  Ident(Interned),
  GroupName(Interned),
  Punctuation(Punctuation),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, CloneWithDelta)]
pub(crate) enum Punctuation {
  Equals,
  Times,
  Exclam,
}

impl Punctuation {
  pub(crate) fn expected(self) -> Result<&'static [Expected], Never> {
    let expected: &'static Expected = match self {
      Self::Equals => &Expected::Punctuation(Self::Equals),
      Self::Times => &Expected::Punctuation(Self::Times),
      Self::Exclam => &Expected::Punctuation(Self::Exclam),
    };

    Ok(slice::from_ref(expected))
  }
}
