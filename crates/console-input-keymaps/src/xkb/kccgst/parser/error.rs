use crate::xkb::code::display_text;
use crate::xkb::code_slice::CodeSlice;
use crate::xkb::diagnostic::DiagnosticKind;
use crate::xkb::interner::Interned;
use crate::xkb::kccgst::parser::DeclCandidate;
use crate::xkb::kccgst::parser::Parser;
use crate::xkb::kccgst::token::Punctuation;
use crate::xkb::kccgst::token::Punctuation::Cbrace;
use crate::xkb::kccgst::token::Punctuation::Obrace;
use crate::xkb::kccgst::token::Punctuation::Obracket;
use crate::xkb::kccgst::token::Token;
use crate::xkb::meaning::Meaning;
use crate::xkb::span::Span;
use crate::xkb::span::SpanExt;
use crate::xkb::span::Spanned;
use Punctuation::Oparen;
use console_core_never::Never;
use std::fmt;
use std::fmt::Formatter;
use std::ops::ControlFlow;

#[derive(Debug, Clone)]
pub(crate) enum ParserError {
  TooDeeplyNested,
  ExpectedButEof(ExpectedButEof),
  UnexpectedDeclCandidate(UnexpectedDeclCandidate),
  UnexpectedToken(UnexpectedToken),
  InvalidU32(InvalidU32),
  MaxRuntimeReached,
}

impl fmt::Display for ParserError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      ParserError::TooDeeplyNested => f.write_str("expression is too deeply nested"),
      ParserError::ExpectedButEof(eof) => write!(
        f,
        "expected {}, but encountered EOF",
        fmt::from_fn(|f_2| write_expected(f_2, eof.expected))
      ),
      ParserError::UnexpectedDeclCandidate(unexpected) => write!(
        f,
        "expected {}, but found {}",
        fmt::from_fn(|f_3| write_expected(f_3, unexpected.expected)),
        fmt::from_fn(|f_4| write_decl_candidate(f_4, unexpected.actual)),
      ),
      ParserError::UnexpectedToken(unexpected) => write!(
        f,
        "expected {}, but found {}",
        fmt::from_fn(|f_5| write_expected(f_5, unexpected.expected)),
        fmt::from_fn(|f_6| write_actual(f_6, &unexpected.actual)),
      ),
      ParserError::InvalidU32(invalid) => {
        let Ok(text) = display_text(&invalid.slice);
        write!(f, "could not parse integer as u32: `{}`", text)
      }
      ParserError::MaxRuntimeReached => f.write_str("reached maximum runtime"),
    }
  }
}

impl std::error::Error for ParserError {}

impl ParserError {
  pub(crate) fn diagnostic_kind(&self) -> Result<DiagnosticKind, Never> {
    Ok(match self {
      ParserError::TooDeeplyNested => DiagnosticKind::TooDeeplyNested,
      ParserError::ExpectedButEof(_) => DiagnosticKind::UnexpectedEof,
      ParserError::UnexpectedDeclCandidate(_) => DiagnosticKind::UnexpectedDeclaration,
      ParserError::UnexpectedToken(_) => DiagnosticKind::UnexpectedToken,
      ParserError::InvalidU32(_) => DiagnosticKind::U32Overflow,
      ParserError::MaxRuntimeReached => DiagnosticKind::MaxRuntimeReached,
    })
  }
}

fn write_decl_candidate(f: &mut Formatter<'_>, actual: DeclCandidate) -> fmt::Result {
  let name = match actual {
    DeclCandidate::None => "an unrecognized statement",
    DeclCandidate::Doodad(_) => "a doodad declaration",
    DeclCandidate::GroupCompat => "a group compat statement",
    DeclCandidate::Include(..) => "an include statement",
    DeclCandidate::Interpret => "an interpret declaration",
    DeclCandidate::KeyAlias => "a key alias declaration",
    DeclCandidate::KeyName(_) => "a key name",
    DeclCandidate::Keys => "a keys declaration",
    DeclCandidate::KeyType => "a key type declaration",
    DeclCandidate::IndicatorMap => "an indicator mapping",
    DeclCandidate::IndicatorName(_) => "an indicator declaration",
    DeclCandidate::ModMap => "a mod-map declaration",
    DeclCandidate::Overlay => "an overlay declaration",
    DeclCandidate::Row => "a row declaration",
    DeclCandidate::Section => "a section declaration",
    DeclCandidate::Shape => "a shape declaration",
    DeclCandidate::Symbols => "a key-symbol mapping",
    DeclCandidate::Var(_) => "an assignment",
    DeclCandidate::Vmod => "a virtual modifier declaration",
  };
  f.write_str(name)
}

fn write_actual(f: &mut Formatter<'_>, actual: &ActualToken) -> fmt::Result {
  match actual {
    ActualToken::Ident(i) => {
      let Ok(bytes) = i.as_bytes();
      let Ok(text) = display_text(bytes);
      write!(f, "`{}`", text)
    }
    ActualToken::Token(t) => match t {
      Token::Ident(_) => f.write_str("an identifier"),
      Token::String(_) => f.write_str("a string"),
      Token::KeyName(_) => f.write_str("a key name"),
      Token::Integer(_, _) => f.write_str("an integer"),
      Token::Float(_, _) => f.write_str("a float"),
      Token::Punctuation(p) => {
        let Ok(punctuation) = punctuation_string(*p);
        write!(f, "`{}`", punctuation)
      }
    },
  }
}

const LONGEST_EXPECTATION: u32 = 1024;

#[derive(Copy, Clone)]
enum Ending {
  Or,
  Plain,
}

#[derive(Copy, Clone)]
enum Place {
  First,
  After,
}

struct Waiting {
  rest: &'static [Expected],
  ending: Ending,
  place: Place,
}

enum Part {
  List(&'static [Expected]),
  Quoted(&'static str),
  Plain(&'static str),
}

fn write_expected(f: &mut Formatter<'_>, expected: &'static [Expected]) -> fmt::Result {
  let Ok(ending) = ending_of(expected);
  let mut waiting = vec![Waiting {
    rest: expected,
    ending,
    place: Place::First,
  }];

  for _step in 0..LONGEST_EXPECTATION {
    let next = match waiting.pop() {
      Some(next) => next,
      None => break,
    };
    let (element, after) = match next.rest.split_first() {
      Some(split) => split,
      None => continue,
    };

    match next.place {
      Place::First => {}
      Place::After => {
        f.write_str(", ")?;
      }
    }

    let here = match after {
      [] => next.ending,
      [_, ..] => {
        waiting.push(Waiting {
          rest: after,
          ending: next.ending,
          place: Place::After,
        });
        Ending::Plain
      }
    };
    let Ok(part) = part_of(element);

    match part {
      Part::List(inner) => waiting.push(Waiting {
        rest: inner,
        ending: here,
        place: Place::First,
      }),
      Part::Quoted(said) => {
        let Ok(lead) = lead_of(here);
        write!(f, "{}`{}`", lead, said)?;
      }
      Part::Plain(said) => {
        let Ok(lead) = lead_of(here);
        write!(f, "{}{}", lead, said)?;
      }
    }
  }

  Ok(())
}

fn ending_of(expected: &'static [Expected]) -> Result<Ending, Never> {
  let walked = (0..LONGEST_EXPECTATION).try_fold(expected, |list, _step| match list {
    [Expected::Nested(inner)] => ControlFlow::Continue(*inner),
    [_alone] => ControlFlow::Break(Ending::Plain),
    [] | [_, _, ..] => ControlFlow::Break(Ending::Or),
  });

  Ok(match walked {
    ControlFlow::Break(ending) => ending,
    ControlFlow::Continue(_too_deep) => Ending::Or,
  })
}

fn lead_of(ending: Ending) -> Result<&'static str, Never> {
  Ok(match ending {
    Ending::Or => "or ",
    Ending::Plain => "",
  })
}

fn part_of(expected: &'static Expected) -> Result<Part, Never> {
  Ok(match expected {
    Expected::Nested(inner) => Part::List(inner),
    Expected::Ident(i) => {
      let Ok(name) = i.name();
      Part::Quoted(name)
    }
    Expected::VarDecl => Part::Plain("an assignment"),
    Expected::AnyIdent => Part::Plain("an identifier"),
    Expected::String => Part::Plain("a string"),
    Expected::KeyName => Part::Plain("a key name"),
    Expected::Integer => Part::Plain("an integer"),
    Expected::Float => Part::Plain("a float"),
    Expected::Punctuation(p) => {
      let Ok(punctuation) = punctuation_string(*p);
      Part::Quoted(punctuation)
    }
  })
}

fn punctuation_string(p: Punctuation) -> Result<&'static str, Never> {
  Ok(match p {
    Punctuation::Semicolon => ";",
    Obrace => "{",
    Cbrace => "}",
    Punctuation::Equals => "=",
    Obracket => "[",
    Punctuation::Cbracket => "]",
    Oparen => "(",
    Punctuation::Cparen => ")",
    Punctuation::Dot => ".",
    Punctuation::Comma => ",",
    Punctuation::Plus => "+",
    Punctuation::Minus => "-",
    Punctuation::Times => "*",
    Punctuation::Divide => "/",
    Punctuation::Exclam => "!",
    Punctuation::Invert => "~",
  })
}

#[derive(Debug, Clone)]
pub(crate) struct InvalidU32 {
  slice: CodeSlice<'static>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExpectedButEof {
  expected: &'static [Expected],
}

#[derive(Debug, Clone)]
pub(crate) struct UnexpectedDeclCandidate {
  expected: &'static [Expected],
  actual: DeclCandidate,
}

#[derive(Debug, Clone)]
pub(crate) struct UnexpectedToken {
  expected: &'static [Expected],
  actual: ActualToken,
}

#[derive(Debug)]
pub(crate) enum Expected {
  Nested(&'static [Expected]),
  Ident(Meaning),
  VarDecl,
  AnyIdent,
  String,
  KeyName,
  Integer,
  Float,
  Punctuation(Punctuation),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ActualToken {
  Ident(CodeSlice<'static>),
  Token(Token),
}

impl Parser<'_, '_, '_> {
  pub(super) fn too_deeply_nested(&self, span: Span) -> Result<Spanned<ParserError>, Never> {
    Ok(ParserError::TooDeeplyNested.spanned2(span + self.diagnostic_delta))
  }

  pub(super) fn expected_but_eof(
    &self,
    expected: &'static [Expected],
  ) -> Result<Spanned<ParserError>, Never> {
    let span = match self.tokens.last() {
      Some(t) => t.span,
      None => Span { lo: 0, hi: 0 },
    };
    Ok(
      ParserError::ExpectedButEof(ExpectedButEof { expected })
        .spanned2(span + self.diagnostic_delta),
    )
  }

  pub(super) fn unexpected_decl_candidate(
    &self,
    expected: &'static [Expected],
    actual: DeclCandidate,
    span: Span,
  ) -> Result<Spanned<ParserError>, Never> {
    Ok(
      ParserError::UnexpectedDeclCandidate(UnexpectedDeclCandidate { expected, actual })
        .spanned2(span + self.diagnostic_delta),
    )
  }

  pub(super) fn unexpected_token(
    &self,
    expected: &'static [Expected],
    token: Spanned<Token>,
  ) -> Result<Spanned<ParserError>, Never> {
    let actual = match token.val {
      Token::Ident(i) => {
        let Ok(interned) = self.interner.get(i);
        let Ok(owned) = interned.to_owned();
        ActualToken::Ident(owned)
      }
      Token::String(_)
      | Token::KeyName(_)
      | Token::Integer(_, _)
      | Token::Float(_, _)
      | Token::Punctuation(_) => ActualToken::Token(token.val),
    };
    Ok(
      ParserError::UnexpectedToken(UnexpectedToken { expected, actual })
        .spanned2(token.span + self.diagnostic_delta),
    )
  }

  pub(super) fn invalid_u32(
    &self,
    name: Spanned<Interned>,
  ) -> Result<Spanned<ParserError>, Never> {
    let Ok(interned) = self.interner.get(name.val);
    let Ok(slice) = interned.to_owned();
    Ok(ParserError::InvalidU32(InvalidU32 { slice }).spanned2(name.span + self.diagnostic_delta))
  }
}

pub(super) const EXPR_TOKENS: &[Expected] = &[
  Expected::AnyIdent,
  Expected::String,
  Expected::KeyName,
  Expected::Integer,
  Expected::Float,
  Expected::Punctuation(punctuation![+]),
  Expected::Punctuation(punctuation![-]),
  Expected::Punctuation(punctuation![!]),
  Expected::Punctuation(punctuation![~]),
  Expected::Punctuation(Oparen),
  Expected::Punctuation(Obracket),
  Expected::Punctuation(Obrace),
];

pub(super) const OUTLINE_TOKENS: &[Expected] =
  &[Expected::Punctuation(Obrace), Expected::VarDecl];

pub(super) struct KeycodeDeclExpectation;

impl ParseDeclExpectationBase for KeycodeDeclExpectation {
  const EXPECTED: &'static [Expected] = &[
    Expected::KeyName,
    Expected::Ident(Meaning::Alias),
    Expected::VarDecl,
    Expected::Ident(Meaning::Indicator),
    Expected::Ident(Meaning::Virtual),
  ];
}

pub(super) struct TypesDeclExpectation;

impl ParseDeclExpectationBase for TypesDeclExpectation {
  const EXPECTED: &'static [Expected] = &[
    Expected::Ident(Meaning::Type),
    Expected::VarDecl,
    Expected::Ident(Meaning::VirtualModifiers),
  ];
}

pub(super) struct CompatmapDeclExpectation;

impl ParseDeclExpectationBase for CompatmapDeclExpectation {
  const EXPECTED: &'static [Expected] = &[
    Expected::Ident(Meaning::Interpret),
    Expected::Ident(Meaning::Group),
    Expected::Ident(Meaning::Indicator),
    Expected::VarDecl,
    Expected::Ident(Meaning::VirtualModifiers),
  ];
}

pub(super) struct SymbolsDeclExpectation;

impl ParseDeclExpectationBase for SymbolsDeclExpectation {
  const EXPECTED: &'static [Expected] = &[
    Expected::Ident(Meaning::Key),
    Expected::VarDecl,
    Expected::Ident(Meaning::VirtualModifiers),
    Expected::Ident(Meaning::ModifierMap),
    Expected::Ident(Meaning::ModMap),
    Expected::Ident(Meaning::Modmap),
  ];
}

pub(super) struct GeometryDeclExpectation;

impl ParseDeclExpectationBase for GeometryDeclExpectation {
  const EXPECTED: &'static [Expected] = &[
    Expected::Ident(Meaning::Alias),
    Expected::VarDecl,
    Expected::Ident(Meaning::Shape),
    Expected::Ident(Meaning::Section),
    Expected::Ident(Meaning::Indicator),
    Expected::Ident(Meaning::Modmap),
    Expected::Nested(DOODAD_EXPECTED),
  ];
}

pub(super) trait ParseDeclExpectationBase {
  const EXPECTED: &'static [Expected];
}

pub(super) trait ParseDeclExpectation {
  fn expected_plus_merge_mode(&self) -> &'static [Expected];

  fn expected_plus_merge_mode_plus_cbrace(&self) -> &'static [Expected];

  fn expected_plus_include(&self) -> &'static [Expected];
}

impl<T> ParseDeclExpectation for T
where
  T: ParseDeclExpectationBase,
{
  fn expected_plus_merge_mode(&self) -> &'static [Expected] {
    &[
      Expected::Nested(MERGE_MODE_EXPECTED),
      Expected::Nested(Self::EXPECTED),
    ]
  }

  fn expected_plus_merge_mode_plus_cbrace(&self) -> &'static [Expected] {
    &[
      Expected::Punctuation(Cbrace),
      Expected::Nested(MERGE_MODE_EXPECTED),
      Expected::Nested(Self::EXPECTED),
    ]
  }

  fn expected_plus_include(&self) -> &'static [Expected] {
    &[Expected::String, Expected::Nested(Self::EXPECTED)]
  }
}

pub(super) const MERGE_MODE_EXPECTED: &[Expected] = &[
  Expected::Ident(Meaning::Include),
  Expected::Ident(Meaning::Augment),
  Expected::Ident(Meaning::Override),
  Expected::Ident(Meaning::Replace),
  Expected::Ident(Meaning::Alternate),
];

pub(super) const DOODAD_EXPECTED: &[Expected] = &[
  Expected::Ident(Meaning::Text),
  Expected::Ident(Meaning::Outline),
  Expected::Ident(Meaning::Solid),
  Expected::Ident(Meaning::Logo),
];

pub(super) const SECTION_ITEM_EXPECTED: &[Expected] = &[
  Expected::Ident(Meaning::Row),
  Expected::Ident(Meaning::Overlay),
  Expected::Ident(Meaning::Indicator),
  Expected::Nested(DOODAD_EXPECTED),
  Expected::VarDecl,
];

pub(super) const KEY_EXPECTED: &[Expected] = &[Expected::KeyName, Expected::Punctuation(Obrace)];

pub(super) const FLAGS_EXPECTED: &[Expected] = &[
  Expected::Ident(Meaning::Partial),
  Expected::Ident(Meaning::Default),
  Expected::Ident(Meaning::Hidden),
  Expected::Ident(Meaning::AlphanumericKeys),
  Expected::Ident(Meaning::ModifierKeys),
  Expected::Ident(Meaning::KeypadKeys),
  Expected::Ident(Meaning::FunctionKeys),
  Expected::Ident(Meaning::AlternateGroup),
];

pub(super) const CONFIG_ITEM_EXPECTED: &[Expected] = &[
  Expected::Ident(Meaning::XkbKeycodes),
  Expected::Ident(Meaning::XkbTypes),
  Expected::Ident(Meaning::XkbCompatibilityMap),
  Expected::Ident(Meaning::XkbCompatibility),
  Expected::Ident(Meaning::XkbCompatMap),
  Expected::Ident(Meaning::XkbCompat),
  Expected::Ident(Meaning::XkbSymbols),
  Expected::Ident(Meaning::XkbGeometry),
];
