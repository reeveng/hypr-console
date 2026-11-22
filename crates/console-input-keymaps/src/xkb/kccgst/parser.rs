#![expect(
  clippy::type_complexity,
  reason = "a parser's tables of token, span and node are spelled out where they are kept"
)]

use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;

pub(crate) mod error;

use crate::Keycode;
use crate::xkb::code_loader::CodeType;
use crate::xkb::code_map::CodeMap;
use crate::xkb::diagnostic::DiagnosticSink;
use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use crate::xkb::kccgst::ast::Call;
use crate::xkb::kccgst::ast::CallArg;
use crate::xkb::kccgst::ast::Compat;
use crate::xkb::kccgst::ast::CompatmapDecl;
use crate::xkb::kccgst::ast::CompositeMap;
use crate::xkb::kccgst::ast::ConfigItem;
use crate::xkb::kccgst::ast::ConfigItemType;
use crate::xkb::kccgst::ast::Coord;
use crate::xkb::kccgst::ast::CoordAssignment;
use crate::xkb::kccgst::ast::Decl;
use crate::xkb::kccgst::ast::Decls;
use crate::xkb::kccgst::ast::DirectOrIncluded;
use crate::xkb::kccgst::ast::DoodadDecl;
use crate::xkb::kccgst::ast::DoodadType;
use crate::xkb::kccgst::ast::Expr;
use crate::xkb::kccgst::ast::ExprAssignment;
use crate::xkb::kccgst::ast::Flag;
use crate::xkb::kccgst::ast::FlagWrapper;
use crate::xkb::kccgst::ast::Flags;
use crate::xkb::kccgst::ast::Geometry;
use crate::xkb::kccgst::ast::GeometryDecl;
use crate::xkb::kccgst::ast::GroupCompatDecl;
use crate::xkb::kccgst::ast::Include;
use crate::xkb::kccgst::ast::IndicatorMapDecl;
use crate::xkb::kccgst::ast::IndicatorNameDecl;
use crate::xkb::kccgst::ast::InterpretDecl;
use crate::xkb::kccgst::ast::InterpretMatch;
use crate::xkb::kccgst::ast::InterpretSym;
use crate::xkb::kccgst::ast::Item;
use crate::xkb::kccgst::ast::ItemType;
use crate::xkb::kccgst::ast::Key;
use crate::xkb::kccgst::ast::KeyAliasDecl;
use crate::xkb::kccgst::ast::KeyExprs;
use crate::xkb::kccgst::ast::KeyNameDecl;
use crate::xkb::kccgst::ast::KeySymbolsDecl;
use crate::xkb::kccgst::ast::KeyTypeDecl;
use crate::xkb::kccgst::ast::KeycodeDecl;
use crate::xkb::kccgst::ast::Keycodes;
use crate::xkb::kccgst::ast::Keys;
use crate::xkb::kccgst::ast::MergeMode;
use crate::xkb::kccgst::ast::ModMapDecl;
use crate::xkb::kccgst::ast::NamedParam;
use crate::xkb::kccgst::ast::NestedConfigItem;
use crate::xkb::kccgst::ast::Outline;
use crate::xkb::kccgst::ast::OverlayDecl;
use crate::xkb::kccgst::ast::OverlayItem;
use crate::xkb::kccgst::ast::Path;
use crate::xkb::kccgst::ast::PathComponent;
use crate::xkb::kccgst::ast::PathIndex;
use crate::xkb::kccgst::ast::RowBody;
use crate::xkb::kccgst::ast::RowBodyItem;
use crate::xkb::kccgst::ast::SectionDecl;
use crate::xkb::kccgst::ast::SectionItem;
use crate::xkb::kccgst::ast::ShapeDecl;
use crate::xkb::kccgst::ast::ShapeDeclType;
use crate::xkb::kccgst::ast::Symbols;
use crate::xkb::kccgst::ast::SymbolsDecl;
use crate::xkb::kccgst::ast::Types;
use crate::xkb::kccgst::ast::TypesDecl;
use crate::xkb::kccgst::ast::VModDecl;
use crate::xkb::kccgst::ast::VModDef;
use crate::xkb::kccgst::ast::Var;
use crate::xkb::kccgst::ast::VarDecl;
use crate::xkb::kccgst::ast::VarOrExpr;
use crate::xkb::kccgst::parser::error::CONFIG_ITEM_EXPECTED;
use crate::xkb::kccgst::parser::error::CompatmapDeclExpectation;
use crate::xkb::kccgst::parser::error::EXPR_TOKENS;
use crate::xkb::kccgst::parser::error::Expected;
use crate::xkb::kccgst::parser::error::FLAGS_EXPECTED;
use crate::xkb::kccgst::parser::error::GeometryDeclExpectation;
use crate::xkb::kccgst::parser::error::KEY_EXPECTED;
use crate::xkb::kccgst::parser::error::KeycodeDeclExpectation;
use crate::xkb::kccgst::parser::error::OUTLINE_TOKENS;
use crate::xkb::kccgst::parser::error::ParseDeclExpectation;
use crate::xkb::kccgst::parser::error::ParserError;
use crate::xkb::kccgst::parser::error::SECTION_ITEM_EXPECTED;
use crate::xkb::kccgst::parser::error::SymbolsDeclExpectation;
use crate::xkb::kccgst::parser::error::TypesDeclExpectation;
use crate::xkb::kccgst::token::Punctuation;
use crate::xkb::kccgst::token::Punctuation::Cbracket;
use crate::xkb::kccgst::token::Punctuation::Cparen;
use crate::xkb::kccgst::token::Punctuation::Oparen;
use crate::xkb::kccgst::token::Token;
use crate::xkb::meaning::Meaning;
use crate::xkb::meaning::MeaningCache;
use crate::xkb::span::Span;
use crate::xkb::span::SpanExt;
use crate::xkb::span::SpanResult1;
use crate::xkb::span::SpanUnit;
use crate::xkb::span::Spanned;
use Punctuation::Cbrace;
use Punctuation::Obrace;
use Punctuation::Obracket;
use std::fmt::Debug;
use std::ops::ControlFlow;

const DEEPEST_EXPRESSION: u32 = 16;

const STEPS_PER_TOKEN: u32 = 16;

struct Diag<'a, 'b, 'c> {
  map: &'a mut CodeMap,
  diagnostics: &'a mut DiagnosticSink<'b, 'c>,
}

struct Parser<'a, 'b, 'c> {
  diag: Option<Diag<'a, 'b, 'c>>,
  tokens: &'a [Spanned<Token>],
  interner: &'a Interner,
  meaning_cache: &'a mut MeaningCache,
  pos: u32,
  diagnostic_delta: SpanUnit,
  expr_depth: u32,
  remaining_runtime: &'a mut u64,
}

#[derive(Debug, Copy, Clone)]
enum DeclCandidate {
  None,
  Doodad(DoodadType),
  GroupCompat,
  Include(Spanned<MergeMode>, Interned),
  Interpret,
  KeyAlias,
  KeyName(Interned),
  Keys,
  KeyType,
  IndicatorMap,
  IndicatorName(Option<Span>),
  ModMap,
  Overlay,
  Row,
  Section,
  Shape,
  Symbols,
  Var(Token),
  Vmod,
}

#[derive(Copy, Clone)]
struct Listing {
  terminator: Punctuation,
  expected: &'static [Expected],
  terminator_plus_comma: &'static [Expected],
}

const BRACE_OR_COMMA: &[Expected] = &[
  Expected::Punctuation(Cbrace),
  Expected::Punctuation(punctuation![,]),
];

const BRACKET_OR_COMMA: &[Expected] = &[
  Expected::Punctuation(Cbracket),
  Expected::Punctuation(punctuation![,]),
];

const PAREN_OR_COMMA: &[Expected] = &[
  Expected::Punctuation(Cparen),
  Expected::Punctuation(punctuation![,]),
];

const SEMICOLON_OR_COMMA: &[Expected] = &[
  Expected::Punctuation(punctuation![;]),
  Expected::Punctuation(punctuation![,]),
];

const EXPRS_IN_BRACES: Listing = Listing {
  terminator: Cbrace,
  expected: &[Expected::Punctuation(Cbrace), Expected::Nested(EXPR_TOKENS)],
  terminator_plus_comma: BRACE_OR_COMMA,
};

const EXPRS_IN_BRACKETS: Listing = Listing {
  terminator: Cbracket,
  expected: &[
    Expected::Punctuation(Cbracket),
    Expected::Nested(EXPR_TOKENS),
  ],
  terminator_plus_comma: BRACKET_OR_COMMA,
};

const ARGS_IN_PARENS: Listing = Listing {
  terminator: Cparen,
  expected: &[Expected::Punctuation(Cparen), Expected::Nested(EXPR_TOKENS)],
  terminator_plus_comma: PAREN_OR_COMMA,
};

const OUTLINES: Listing = Listing {
  terminator: Cbrace,
  expected: &[
    Expected::Punctuation(Cbrace),
    Expected::Nested(OUTLINE_TOKENS),
  ],
  terminator_plus_comma: BRACE_OR_COMMA,
};

const COORDS: Listing = Listing {
  terminator: Cbrace,
  expected: &[
    Expected::Punctuation(Cbrace),
    Expected::Nested(&[Expected::Punctuation(Obracket)]),
  ],
  terminator_plus_comma: BRACE_OR_COMMA,
};

const KEYS: Listing = Listing {
  terminator: Cbrace,
  expected: &[
    Expected::Punctuation(Cbrace),
    Expected::Nested(KEY_EXPECTED),
  ],
  terminator_plus_comma: BRACE_OR_COMMA,
};

const OVERLAY_ITEMS: Listing = Listing {
  terminator: Cbrace,
  expected: &[
    Expected::Punctuation(Cbrace),
    Expected::Nested(&[Expected::KeyName]),
  ],
  terminator_plus_comma: BRACE_OR_COMMA,
};

const VMOD_DEFS: Listing = Listing {
  terminator: punctuation![;],
  expected: &[
    Expected::Punctuation(punctuation![;]),
    Expected::Nested(&[Expected::AnyIdent]),
  ],
  terminator_plus_comma: SEMICOLON_OR_COMMA,
};

const ITEM_TYPES: &[(Meaning, CodeType)] = &[
  (Meaning::XkbKeymap, CodeType::Keymap),
  (Meaning::XkbSemantics, CodeType::Keymap),
  (Meaning::XkbLayout, CodeType::Keymap),
  (Meaning::XkbKeycodes, CodeType::Keycodes),
  (Meaning::XkbTypes, CodeType::Types),
  (Meaning::XkbCompatibilityMap, CodeType::Compat),
  (Meaning::XkbCompatibility, CodeType::Compat),
  (Meaning::XkbCompatMap, CodeType::Compat),
  (Meaning::XkbCompat, CodeType::Compat),
  (Meaning::XkbSymbols, CodeType::Symbols),
  (Meaning::XkbGeometry, CodeType::Geometry),
];

const FLAG_NAMES: &[(Meaning, Flag)] = &[
  (Meaning::Partial, Flag::Partial),
  (Meaning::Default, Flag::Default),
  (Meaning::Hidden, Flag::Hidden),
  (Meaning::AlphanumericKeys, Flag::AlphanumericKeys),
  (Meaning::ModifierKeys, Flag::ModifierKeys),
  (Meaning::KeypadKeys, Flag::KeypadKeys),
  (Meaning::FunctionKeys, Flag::FunctionKeys),
  (Meaning::AlternateGroup, Flag::AlternateGroup),
];

const MERGE_MODES: &[(Meaning, MergeMode)] = &[
  (Meaning::Include, MergeMode::Include),
  (Meaning::Augment, MergeMode::Augment),
  (Meaning::Override, MergeMode::Override),
  (Meaning::Replace, MergeMode::Replace),
  (Meaning::Alternate, MergeMode::Alternate),
];

const DECL_WORDS: &[(Meaning, DeclCandidate)] = &[
  (Meaning::Interpret, DeclCandidate::Interpret),
  (Meaning::Alias, DeclCandidate::KeyAlias),
  (Meaning::Type, DeclCandidate::KeyType),
  (Meaning::Key, DeclCandidate::Symbols),
  (Meaning::Keys, DeclCandidate::Keys),
  (Meaning::ModifierMap, DeclCandidate::ModMap),
  (Meaning::ModMap, DeclCandidate::ModMap),
  (Meaning::Modmap, DeclCandidate::ModMap),
  (Meaning::Group, DeclCandidate::GroupCompat),
  (Meaning::Shape, DeclCandidate::Shape),
  (Meaning::Section, DeclCandidate::Section),
  (Meaning::Text, DeclCandidate::Doodad(DoodadType::Text)),
  (Meaning::Outline, DeclCandidate::Doodad(DoodadType::Outline)),
  (Meaning::Solid, DeclCandidate::Doodad(DoodadType::Solid)),
  (Meaning::Logo, DeclCandidate::Doodad(DoodadType::Logo)),
  (Meaning::VirtualModifiers, DeclCandidate::Vmod),
  (Meaning::Row, DeclCandidate::Row),
  (Meaning::Overlay, DeclCandidate::Overlay),
];

const ITEM_EXPECTED: &[Expected] = &[
  Expected::Nested(FLAGS_EXPECTED),
  Expected::Ident(Meaning::XkbKeymap),
  Expected::Ident(Meaning::XkbSemantics),
  Expected::Ident(Meaning::XkbLayout),
  Expected::Nested(CONFIG_ITEM_EXPECTED),
];

#[derive(Copy, Clone)]
enum Comma {
  Due,
  NotDue,
}

enum ListStep {
  Ended(SpanUnit),
  Item,
}

enum Shape {
  Var,
  Expr,
}

enum Argument {
  Named,
  Plain,
}

#[derive(Copy, Clone)]
enum Sign {
  Plus,
  Minus,
}

#[derive(Copy, Clone)]
enum Product {
  Times,
  Divide,
}

#[derive(Copy, Clone)]
enum Unary {
  Plus,
  Minus,
  Not,
  Inverse,
}

#[derive(Copy, Clone)]
enum Bracketing {
  Brackets,
  Braces,
}

struct ExprList {
  lo: SpanUnit,
  bracketing: Bracketing,
  items: Vec<Spanned<Expr>>,
  comma: Comma,
}

struct NamedSettings {
  name: Spanned<Interned>,
  decls: Box<[Spanned<VarDecl>]>,
  hi: SpanUnit,
}

struct ArgList {
  callee: Spanned<Path>,
  items: Vec<Spanned<CallArg>>,
  comma: Comma,
}

#[derive(Default)]
struct PathSoFar {
  components: Vec<PathComponent>,
  span: Option<Span>,
}

enum Grown {
  More(PathSoFar, Spanned<Interned>),
  Finished(Spanned<Path>),
}

enum Purpose {
  Term,
  Named(ArgList),
}

enum Begin {
  Expr(&'static [Expected]),
  Product(&'static [Expected]),
  Term(&'static [Expected]),
  DeepTerm,
  Component(Box<Component>),
  ExprList(ExprList),
  ArgList(ArgList),
}

struct Component {
  so_far: PathSoFar,
  ident: Spanned<Interned>,
  purpose: Purpose,
}

struct Indexing {
  component: Component,
  obracket: Spanned<Token>,
}

enum Frame {
  LeaveDepth,
  Sum,
  SumRight(Box<Spanned<Expr>>, Sign),
  Product,
  ProductRight(Box<Spanned<Expr>>, Product, Span),
  Paren(SpanUnit),
  Unary(SpanUnit, Unary),
  ExprItem(ExprList),
  PathIndex(Box<Indexing>),
  ArgExpr(ArgList),
  NamedValue(ArgList, Spanned<Path>),
}

enum Next {
  Begin(Begin),
  Deliver(Spanned<Expr>),
}

fn named<T: Copy>(table: &[(Meaning, T)], meaning: Meaning) -> Result<Option<T>, Never> {
  Ok(
    table
      .iter()
      .find(|(name, _)| *name == meaning)
      .map(|(_, found)| *found),
  )
}

fn path_of(components: Vec<PathComponent>) -> Result<Path, Never> {
  let alone = match components.as_slice() {
    [only] => match &only.index {
      None => Some(only.ident.val),
      Some(_) => None,
    },
    [] | [_, _, ..] => None,
  };

  Ok(match alone {
    Some(ident) => Path::One(ident),
    None => Path::Any(components.into_boxed_slice()),
  })
}

fn statement_end(tokens: &[Spanned<Token>], from: u32) -> Result<u32, Never> {
  let Ok(start) = index(from);
  let rest = match tokens.get(start..) {
    Some(rest) => rest,
    None => &[],
  };
  let walked = (from..)
    .zip(rest)
    .try_fold(0_u32, |depth, (at, token)| match token.val {
      token![;] => match depth {
        0 => ControlFlow::Break(at.saturating_add(1)),
        _ => ControlFlow::Continue(depth),
      },
      Token::Punctuation(Obrace) => ControlFlow::Continue(depth.saturating_add(1)),
      Token::Punctuation(Cbrace) => ControlFlow::Continue(depth.saturating_sub(1)),
      Token::Punctuation(_)
      | Token::Ident(_)
      | Token::String(_)
      | Token::KeyName(_)
      | Token::Integer(_, _)
      | Token::Float(_, _) => ControlFlow::Continue(depth),
    });

  Ok(match walked {
    ControlFlow::Break(after) => after,
    ControlFlow::Continue(_depth) => {
      let Ok(end) = fitted::<_, u32>(tokens.len());
      end
    }
  })
}

fn statement_shape(ahead: &[Spanned<Token>]) -> Result<Shape, Never> {
  let after_not = match ahead {
    [Spanned { val: token![!], .. }, rest @ ..] => rest,
    _ => ahead,
  };

  Ok(match after_not {
    [
      Spanned { val: Token::Ident(_), .. },
      Spanned { val: token![.] | token![=] | token![,] | Token::Punctuation(Cbrace | Obracket), .. },
      ..,
    ] => Shape::Var,
    _ => Shape::Expr,
  })
}

pub(crate) type TyAndName = (Option<Span>, Spanned<CodeType>, Option<Spanned<Interned>>);

pub(crate) fn snoop_ty_and_name(
  tokens: &[Spanned<Token>],
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
) -> Result<TyAndName, Spanned<ParserError>> {
  let mut parser = Parser {
    diag: None,
    tokens,
    interner,
    meaning_cache,
    pos: 0,
    diagnostic_delta: 0,
    expr_depth: 0,
    remaining_runtime: &mut { u64::MAX },
  };
  let flags = parser.parse_flags()?;
  let default = flags
    .flags
    .iter()
    .find(|f| f.flag == Flag::Default)
    .map(|f| f.name.span);
  let i = parser.parse_ident()?;
  let Ok(meaning) = parser.meaning_cache.get_case_insensitive(interner, i.val);
  let Ok(item_type) = named(ITEM_TYPES, meaning);
  let ty = match item_type {
    Some(ty) => ty,
    None => {
      let Ok(map) = i.map(Token::Ident);
      let Ok(unexpected_token) = parser.unexpected_token(ITEM_EXPECTED, map);
      return Err(unexpected_token);
    }
  };
  let (name, _) = parser.parse_map_name()?;
  Ok((default, ty.spanned2(i.span), name))
}

pub(crate) fn parse_item(
  map: &mut CodeMap,
  diagnostics: &mut DiagnosticSink,
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  tokens: &[Spanned<Token>],
  diagnostic_delta: SpanUnit,
  remaining_runtime: &mut u64,
) -> Result<Spanned<Item>, Spanned<ParserError>> {
  Parser {
    diag: Some(Diag { map, diagnostics }),
    tokens,
    interner,
    meaning_cache,
    pos: 0,
    diagnostic_delta,
    expr_depth: 0,
    remaining_runtime,
  }
  .parse_item()
}

impl Parser<'_, '_, '_> {
  fn next(
    &mut self,
    expected: &'static [Expected],
  ) -> Result<Spanned<Token>, Spanned<ParserError>> {
    let Ok(at) = index(self.pos);

    match self.tokens.get(at) {
      Some(token) => {
        self.pos = self.pos.saturating_add(1);
        Ok(*token)
      }
      None => {
        let Ok(expected_but_eof) = self.expected_but_eof(expected);
        Err(expected_but_eof)
      }
    }
  }

  fn try_peek(&mut self) -> Result<Option<Spanned<Token>>, Never> {
    let Ok(at) = index(self.pos);
    Ok(self.tokens.get(at).copied())
  }

  fn try_peek2(&mut self) -> Result<Option<Spanned<Token>>, Never> {
    let Ok(at) = index(self.pos.saturating_add(1));
    Ok(self.tokens.get(at).copied())
  }

  fn ahead(&self) -> Result<&[Spanned<Token>], Never> {
    let Ok(at) = index(self.pos);

    Ok(match self.tokens.get(at..) {
      Some(ahead) => ahead,
      None => &[],
    })
  }

  fn peek(
    &mut self,
    expected: &'static [Expected],
  ) -> Result<Spanned<Token>, Spanned<ParserError>> {
    let Ok(peeked) = self.try_peek();

    match peeked {
      Some(token) => Ok(token),
      None => {
        let Ok(expected_but_eof) = self.expected_but_eof(expected);
        Err(expected_but_eof)
      }
    }
  }

  fn eat(&mut self, wanted: Punctuation) -> Result<Option<Spanned<Token>>, Never> {
    let Ok(peeked) = self.try_peek();

    Ok(match peeked {
      Some(token) => match ::core::primitive::bool::then_some(token.val == Token::Punctuation(wanted), ()) {
        ::core::option::Option::Some(()) => {
          self.pos = self.pos.saturating_add(1);
          Some(token)
        }
        ::core::option::Option::None => None,
      },
      None => None,
    })
  }

  fn rounds(&self) -> Result<u32, Never> {
    let Ok(count) = fitted::<_, u32>(self.tokens.len());
    Ok(count.saturating_sub(self.pos).saturating_add(1))
  }

  fn here(&self) -> Result<Span, Never> {
    let Ok(at) = index(self.pos);

    Ok(match self.tokens.get(at) {
      Some(token) => token.span,
      None => match self.tokens.last() {
        Some(token) => token.span,
        None => Span { lo: 0, hi: 0 },
      },
    })
  }

  fn consume_runtime(&mut self, span: Span) -> Result<(), Spanned<ParserError>> {
    match self.remaining_runtime.checked_sub(1) {
      Some(left) => {
        *self.remaining_runtime = left;
        Ok(())
      }
      None => Err(ParserError::MaxRuntimeReached.spanned2(span)),
    }
  }

  fn parse_item(&mut self) -> Result<Spanned<Item>, Spanned<ParserError>> {
    let flags = self.parse_flags()?;
    let i = self.parse_ident()?;
    let Ok(meaning) = self
      .meaning_cache
      .get_case_insensitive(self.interner, i.val);
    let Ok(item_type) = named(ITEM_TYPES, meaning);
    let map = match item_type {
      Some(CodeType::Keymap) => {
        let composite = self.parse_composite_map(i)?;
        let Ok(map) = composite.map(ItemType::Composite);
        map
      }
      Some(
        CodeType::Keycodes
        | CodeType::Types
        | CodeType::Compat
        | CodeType::Symbols
        | CodeType::Geometry,
      ) => {
        let config = self.parse_config_item_type(i, item_type)?;
        let Ok(map) = config.map(ItemType::Configuration);
        map
      }
      Some(CodeType::Rules) | None => {
        let Ok(token) = i.map(Token::Ident);
        let Ok(unexpected_token) = self.unexpected_token(ITEM_EXPECTED, token);
        return Err(unexpected_token);
      }
    };
    let lo = match flags.flags.first() {
      Some(f) => f.name.span.lo,
      None => i.span.lo,
    };
    let item = Item { flags, ty: map.val };
    let span = Span {
      lo,
      hi: map.span.hi,
    };
    Ok(item.spanned2(span))
  }

  fn parse_config_item_type(
    &mut self,
    ty: Spanned<Interned>,
    item_type: Option<CodeType>,
  ) -> Result<Spanned<ConfigItemType>, Spanned<ParserError>> {
    match item_type {
      Some(CodeType::Keycodes) => self.parse_config_item(
        &KeycodeDeclExpectation,
        ty.span.lo,
        |slf| slf.parse_keycode_decl(),
        |name, decls| ConfigItemType::Keycodes(Keycodes { name, decls }),
      ),
      Some(CodeType::Types) => self.parse_config_item(
        &TypesDeclExpectation,
        ty.span.lo,
        |slf| slf.parse_types_decl(),
        |name, decls| ConfigItemType::Types(Types { name, decls }),
      ),
      Some(CodeType::Compat) => self.parse_config_item(
        &CompatmapDeclExpectation,
        ty.span.lo,
        |slf| slf.parse_compatmap_decl(),
        |name, decls| ConfigItemType::Compat(Compat { name, decls }),
      ),
      Some(CodeType::Symbols) => self.parse_config_item(
        &SymbolsDeclExpectation,
        ty.span.lo,
        |slf| slf.parse_symbols_decl(),
        |name, decls| ConfigItemType::Symbols(Symbols { name, decls }),
      ),
      Some(CodeType::Geometry) => self.parse_config_item(
        &GeometryDeclExpectation,
        ty.span.lo,
        |slf| slf.parse_geometry_decl(),
        |name, decls| ConfigItemType::Geometry(Geometry { name, decls }),
      ),
      Some(CodeType::Keymap | CodeType::Rules) | None => {
        const EXPECTED: &[Expected] = &[
          Expected::Nested(FLAGS_EXPECTED),
          Expected::Nested(CONFIG_ITEM_EXPECTED),
        ];
        let Ok(map) = ty.map(Token::Ident);
        let Ok(unexpected_token) = self.unexpected_token(EXPECTED, map);
        Err(unexpected_token)
      }
    }
  }

  fn parse_composite_map(
    &mut self,
    ty: Spanned<Interned>,
  ) -> Result<Spanned<CompositeMap>, Spanned<ParserError>> {
    const EXPECTED: &[Expected] = &[Expected::Punctuation(Cbrace), Expected::AnyIdent];
    let (name, obrace) = self.parse_map_name()?;
    let items =
      self.parse_item_list(obrace.lo, Cbrace, EXPECTED, |slf| slf.parse_map_config())?;
    let semicolon = self.consume_token(punctuation![;])?;
    let map = CompositeMap {
      name,
      config_items: items.val,
    };
    Ok(map.spanned(ty.span.lo, semicolon.hi))
  }

  fn parse_map_config(&mut self) -> Result<Spanned<NestedConfigItem>, Spanned<ParserError>> {
    let flags = self.parse_flags()?;
    let ty = self.parse_ident()?;
    let Ok(meaning) = self
      .meaning_cache
      .get_case_insensitive(self.interner, ty.val);
    let Ok(item_type) = named(ITEM_TYPES, meaning);
    let ct = self.parse_config_item_type(ty, item_type)?;
    let lo = match flags.flags.first() {
      Some(f) => f.name.span.lo,
      None => ty.span.lo,
    };
    let item = NestedConfigItem {
      flags,
      item: ConfigItem { specific: ct.val },
    };
    Ok(item.spanned(lo, ct.span.hi))
  }

  fn parse_keycode_decl(&mut self) -> Result<Spanned<Decl<KeycodeDecl>>, Spanned<ParserError>> {
    self.parse_decl(
      &KeycodeDeclExpectation,
      |slf, candidate, span, expected| match candidate {
        DeclCandidate::Include(mm, i) => slf
          .parse_include(mm, i.spanned2(span))
          .span_map(KeycodeDecl::Include),
        DeclCandidate::KeyName(i) => slf
          .parse_keyname_decl(i.spanned2(span))
          .span_map(KeycodeDecl::KeyName),
        DeclCandidate::KeyAlias => slf
          .parse_key_alias_decl(span.lo)
          .span_map(KeycodeDecl::KeyAlias),
        DeclCandidate::Var(t) => slf
          .parse_var_decl(Some(t.spanned2(span)))
          .span_map(KeycodeDecl::Var),
        DeclCandidate::IndicatorName(virt) => slf
          .parse_indicator_name_decl(virt, span.lo)
          .span_map(KeycodeDecl::IndicatorName),
        DeclCandidate::None
        | DeclCandidate::Doodad(_)
        | DeclCandidate::GroupCompat
        | DeclCandidate::Interpret
        | DeclCandidate::Keys
        | DeclCandidate::KeyType
        | DeclCandidate::IndicatorMap
        | DeclCandidate::ModMap
        | DeclCandidate::Overlay
        | DeclCandidate::Row
        | DeclCandidate::Section
        | DeclCandidate::Shape
        | DeclCandidate::Symbols
        | DeclCandidate::Vmod => {
          let Ok(unexpected) = slf.unexpected_decl_candidate(expected, candidate, span);
          Err(unexpected)
        }
      },
    )
  }

  fn parse_types_decl(&mut self) -> Result<Spanned<Decl<TypesDecl>>, Spanned<ParserError>> {
    self.parse_decl(
      &TypesDeclExpectation,
      |slf, candidate, span, expected| match candidate {
        DeclCandidate::Include(mm, i) => slf
          .parse_include(mm, i.spanned2(span))
          .span_map(TypesDecl::Include),
        DeclCandidate::KeyType => slf
          .parse_key_type_decl(span.lo)
          .span_map(TypesDecl::KeyType),
        DeclCandidate::Var(t) => slf
          .parse_var_decl(Some(t.spanned2(span)))
          .span_map(TypesDecl::Var),
        DeclCandidate::Vmod => slf.parse_vmod_decl(span.lo).span_map(TypesDecl::VMod),
        DeclCandidate::None
        | DeclCandidate::Doodad(_)
        | DeclCandidate::GroupCompat
        | DeclCandidate::Interpret
        | DeclCandidate::KeyAlias
        | DeclCandidate::KeyName(_)
        | DeclCandidate::Keys
        | DeclCandidate::IndicatorMap
        | DeclCandidate::IndicatorName(_)
        | DeclCandidate::ModMap
        | DeclCandidate::Overlay
        | DeclCandidate::Row
        | DeclCandidate::Section
        | DeclCandidate::Shape
        | DeclCandidate::Symbols => {
          let Ok(unexpected) = slf.unexpected_decl_candidate(expected, candidate, span);
          Err(unexpected)
        }
      },
    )
  }

  fn parse_compatmap_decl(
    &mut self,
  ) -> Result<Spanned<Decl<CompatmapDecl>>, Spanned<ParserError>> {
    self.parse_decl(
      &CompatmapDeclExpectation,
      |slf, candidate, span, expected| match candidate {
        DeclCandidate::Include(mm, i) => slf
          .parse_include(mm, i.spanned2(span))
          .span_map(CompatmapDecl::Include),
        DeclCandidate::Interpret => slf
          .parse_interpret_decl(span.lo)
          .span_map(CompatmapDecl::Interpret),
        DeclCandidate::GroupCompat => slf
          .parse_group_compat_decl(span.lo)
          .span_map(CompatmapDecl::GroupCompat),
        DeclCandidate::IndicatorMap => slf
          .parse_indicator_map_decl(span.lo)
          .span_map(CompatmapDecl::IndicatorMap),
        DeclCandidate::Var(t) => slf
          .parse_var_decl(Some(t.spanned2(span)))
          .span_map(CompatmapDecl::Var),
        DeclCandidate::Vmod => slf.parse_vmod_decl(span.lo).span_map(CompatmapDecl::VMod),
        DeclCandidate::None
        | DeclCandidate::Doodad(_)
        | DeclCandidate::KeyAlias
        | DeclCandidate::KeyName(_)
        | DeclCandidate::Keys
        | DeclCandidate::KeyType
        | DeclCandidate::IndicatorName(_)
        | DeclCandidate::ModMap
        | DeclCandidate::Overlay
        | DeclCandidate::Row
        | DeclCandidate::Section
        | DeclCandidate::Shape
        | DeclCandidate::Symbols => {
          let Ok(unexpected) = slf.unexpected_decl_candidate(expected, candidate, span);
          Err(unexpected)
        }
      },
    )
  }

  fn parse_symbols_decl(&mut self) -> Result<Spanned<Decl<SymbolsDecl>>, Spanned<ParserError>> {
    self.parse_decl(
      &SymbolsDeclExpectation,
      |slf, candidate, span, expected| match candidate {
        DeclCandidate::Include(mm, i) => slf
          .parse_include(mm, i.spanned2(span))
          .span_map(SymbolsDecl::Include),
        DeclCandidate::Symbols => slf
          .parse_key_symbols_decl(span.lo)
          .span_map(SymbolsDecl::Symbols),
        DeclCandidate::Var(t) => slf
          .parse_var_decl(Some(t.spanned2(span)))
          .span_map(SymbolsDecl::Var),
        DeclCandidate::Vmod => slf.parse_vmod_decl(span.lo).span_map(SymbolsDecl::VMod),
        DeclCandidate::ModMap => slf
          .parse_mod_map_decl(span.lo)
          .span_map(SymbolsDecl::ModMap),
        DeclCandidate::None
        | DeclCandidate::Doodad(_)
        | DeclCandidate::GroupCompat
        | DeclCandidate::Interpret
        | DeclCandidate::KeyAlias
        | DeclCandidate::KeyName(_)
        | DeclCandidate::Keys
        | DeclCandidate::KeyType
        | DeclCandidate::IndicatorMap
        | DeclCandidate::IndicatorName(_)
        | DeclCandidate::Overlay
        | DeclCandidate::Row
        | DeclCandidate::Section
        | DeclCandidate::Shape => {
          let Ok(unexpected) = slf.unexpected_decl_candidate(expected, candidate, span);
          Err(unexpected)
        }
      },
    )
  }

  fn parse_geometry_decl(&mut self) -> Result<Spanned<Decl<GeometryDecl>>, Spanned<ParserError>> {
    self.parse_decl(
      &GeometryDeclExpectation,
      |slf, candidate, span, expected| match candidate {
        DeclCandidate::Include(mm, i) => slf
          .parse_include(mm, i.spanned2(span))
          .span_map(GeometryDecl::Include),
        DeclCandidate::KeyAlias => slf
          .parse_key_alias_decl(span.lo)
          .span_map(GeometryDecl::KeyAlias),
        DeclCandidate::Var(t) => slf
          .parse_var_decl(Some(t.spanned2(span)))
          .span_map(GeometryDecl::Var),
        DeclCandidate::Shape => slf.parse_shape_decl(span.lo).span_map(GeometryDecl::Shape),
        DeclCandidate::Section => slf
          .parse_section_decl(span.lo)
          .span_map(GeometryDecl::Section),
        DeclCandidate::IndicatorMap => slf
          .parse_indicator_map_decl(span.lo)
          .span_map(GeometryDecl::IndicatorMap),
        DeclCandidate::Doodad(doodad) => slf
          .parse_doodad_decl(doodad.spanned2(span))
          .span_map(GeometryDecl::Doodad),
        DeclCandidate::None
        | DeclCandidate::GroupCompat
        | DeclCandidate::Interpret
        | DeclCandidate::KeyName(_)
        | DeclCandidate::Keys
        | DeclCandidate::KeyType
        | DeclCandidate::IndicatorName(_)
        | DeclCandidate::ModMap
        | DeclCandidate::Overlay
        | DeclCandidate::Row
        | DeclCandidate::Symbols
        | DeclCandidate::Vmod => {
          let Ok(unexpected) = slf.unexpected_decl_candidate(expected, candidate, span);
          Err(unexpected)
        }
      },
    )
  }

  fn parse_include(
    &mut self,
    mm: Spanned<MergeMode>,
    path: Spanned<Interned>,
  ) -> Result<Spanned<Include>, Spanned<ParserError>> {
    let ty = Include {
      mm,
      path,
      loaded: None,
    };
    Ok(ty.spanned2(path.span))
  }

  fn parse_config_item<T: Debug, U>(
    &mut self,
    e: &impl ParseDeclExpectation,
    lo: SpanUnit,
    f: impl FnMut(&mut Self) -> Result<Spanned<Decl<T>>, Spanned<ParserError>>,
    g: impl FnOnce(Option<Spanned<Interned>>, Decls<T>) -> U,
  ) -> Result<Spanned<U>, Spanned<ParserError>> {
    let (name, obrace) = self.parse_map_name()?;
    let items = self.parse_item_list(
      obrace.lo,
      Cbrace,
      e.expected_plus_merge_mode_plus_cbrace(),
      f,
    )?;
    let semicolon = self.consume_token(punctuation![;])?;
    let decls = Decls { decls: items.val };
    let codes = g(name, decls);
    Ok(codes.spanned(lo, semicolon.hi))
  }

  fn parse_decl<T>(
    &mut self,
    e: &impl ParseDeclExpectation,
    f: impl FnOnce(
      &mut Self,
      DeclCandidate,
      Span,
      &'static [Expected],
    ) -> Result<Spanned<T>, Spanned<ParserError>>,
  ) -> Result<Spanned<Decl<T>>, Spanned<ParserError>> {
    let Ok(merge_mode) = self.parse_merge_mode();
    let expected = match merge_mode {
      Some(_) => e.expected_plus_include(),
      None => e.expected_plus_merge_mode(),
    };
    let t = self.next(expected)?;
    let Ok(candidate) = self.detect_decl_candidate(merge_mode, t);
    let ty = f(self, candidate, t.span, expected)?;
    let span = match merge_mode {
      Some(mm) => Span {
        lo: mm.span.lo,
        hi: ty.span.hi,
      },
      None => ty.span,
    };
    let decl = Decl {
      merge_mode,
      ty: DirectOrIncluded::Direct(ty.val),
    };
    Ok(decl.spanned2(span))
  }

  fn parse_map_name(
    &mut self,
  ) -> Result<(Option<Spanned<Interned>>, Span), Spanned<ParserError>> {
    let Ok(peeked) = self.try_peek();
    let map_name = match peeked {
      Some(Spanned {
        val: Token::String(i),
        span,
      }) => {
        self.pos = self.pos.saturating_add(1);
        Some(i.spanned2(span))
      }
      Some(_) | None => None,
    };
    let expected: &'static [Expected] = match map_name {
      Some(_) => &[Expected::Punctuation(Obrace)],
      None => &[Expected::String, Expected::Punctuation(Obrace)],
    };
    let t = self.next(expected)?;

    match ::core::primitive::bool::then_some(t.val == Obrace, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        let Ok(unexpected_token) = self.unexpected_token(expected, t);
        return Err(unexpected_token);
      }
    }

    Ok((map_name, t.span))
  }

  fn flag_ahead(&mut self) -> Result<Option<(Spanned<Interned>, Flag)>, Never> {
    let Ok(peeked) = self.try_peek();
    let name = match peeked {
      Some(Spanned {
        val: Token::Ident(i),
        span,
      }) => i.spanned2(span),
      Some(_) | None => {
        return Ok(None);
      }
    };
    let Ok(meaning) = self
      .meaning_cache
      .get_case_insensitive(self.interner, name.val);
    let Ok(flag) = named(FLAG_NAMES, meaning);

    Ok(flag.map(|found| (name, found)))
  }

  fn parse_flags(&mut self) -> Result<Flags, Spanned<ParserError>> {
    let mut flags = vec![];
    let Ok(rounds) = self.rounds();

    for _round in 0..rounds {
      let Ok(ahead) = self.flag_ahead();
      let (name, flag) = match ahead {
        Some(found) => found,
        None => break,
      };
      self.consume_runtime(name.span)?;
      flags.push(FlagWrapper { name, flag });
      self.pos = self.pos.saturating_add(1);
    }

    Ok(Flags {
      flags: flags.into_boxed_slice(),
    })
  }

  fn parse_interpret_decl(
    &mut self,
    lo: SpanUnit,
  ) -> Result<Spanned<InterpretDecl>, Spanned<ParserError>> {
    let match_ = self.parse_interpret_match()?;
    let obrace = self.consume_token(Obrace)?;
    let vars = self.parse_item_list(
      obrace.lo,
      Cbrace,
      &[Expected::Punctuation(Cbrace), Expected::VarDecl],
      |slf| slf.parse_var_decl(None),
    )?;
    let decl = InterpretDecl {
      match_,
      vars: vars.val,
    };
    let semicolon = self.consume_token(punctuation![;])?;
    Ok(decl.spanned(lo, semicolon.hi))
  }

  fn parse_interpret_match(&mut self) -> Result<Spanned<InterpretMatch>, Spanned<ParserError>> {
    let expected = &[Expected::AnyIdent, Expected::String];
    let token = self.next(expected)?;
    let sym = match token.val {
      Token::Ident(i) => InterpretSym::Ident(i),
      Token::String(i) => InterpretSym::String(i),
      Token::Integer(i, v) => InterpretSym::Integer(i, v),
      Token::KeyName(_) | Token::Float(_, _) | Token::Punctuation(_) => {
        let Ok(unexpected) = self.unexpected_token(expected, token);
        return Err(unexpected);
      }
    };
    let symbol = sym.spanned2(token.span);
    let Ok(plus) = self.eat(punctuation![+]);
    let (filter, hi) = match plus {
      Some(_) => {
        let e = self.parse_expr(EXPR_TOKENS)?;
        let hi = e.span.hi;
        (Some(e), hi)
      }
      None => (None, symbol.span.hi),
    };
    let span = Span {
      lo: symbol.span.lo,
      hi,
    };
    let m = InterpretMatch {
      sym: symbol,
      filter,
    };
    Ok(m.spanned2(span))
  }

  fn parse_key_alias_decl(
    &mut self,
    lo: SpanUnit,
  ) -> Result<Spanned<KeyAliasDecl>, Spanned<ParserError>> {
    let name = self.parse_keyname()?;
    self.consume_token(punctuation![=])?;
    let alias_for = self.parse_keyname()?;
    let semicolon = self.consume_token(punctuation![;])?;
    let ty = KeyAliasDecl { name, alias_for };
    Ok(ty.spanned(lo, semicolon.hi))
  }

  fn parse_key_type_decl(
    &mut self,
    lo: SpanUnit,
  ) -> Result<Spanned<KeyTypeDecl>, Spanned<ParserError>> {
    let named = self.parse_named_settings()?;
    let decl = KeyTypeDecl {
      name: named.name,
      decls: named.decls,
    };
    Ok(decl.spanned(lo, named.hi))
  }

  fn parse_named_settings(&mut self) -> Result<NamedSettings, Spanned<ParserError>> {
    let name = self.parse_string()?;
    let obrace = self.consume_token(Obrace)?;
    let decls = self.parse_item_list(
      obrace.lo,
      Cbrace,
      &[Expected::Punctuation(Cbrace), Expected::VarDecl],
      |slf| slf.parse_var_decl(None),
    )?;
    let semicolon = self.consume_token(punctuation![;])?;

    Ok(NamedSettings {
      name,
      decls: decls.val,
      hi: semicolon.hi,
    })
  }

  fn parse_key_symbols_decl(
    &mut self,
    lo: SpanUnit,
  ) -> Result<Spanned<KeySymbolsDecl>, Spanned<ParserError>> {
    let key = self.parse_keyname()?;
    let obrace = self.consume_token(Obrace)?;
    let vars = self.parse_var_decl_or_expr_list(obrace.lo)?;
    let semicolon = self.consume_token(punctuation![;])?;
    let decl = KeySymbolsDecl {
      key,
      vars: vars.val,
    };
    Ok(decl.spanned(lo, semicolon.hi))
  }

  fn parse_mod_map_decl(
    &mut self,
    lo: SpanUnit,
  ) -> Result<Spanned<ModMapDecl>, Spanned<ParserError>> {
    let modifier = self.parse_ident()?;
    let obrace = self.consume_token(Obrace)?;
    let keys = self.parse_expr_list(obrace.lo)?;
    let semicolon = self.consume_token(punctuation![;])?;
    let ty = ModMapDecl {
      modifier,
      keys: keys.val,
    };
    Ok(ty.spanned(lo, semicolon.hi))
  }

  fn parse_expr_list(
    &mut self,
    lo: SpanUnit,
  ) -> Result<Spanned<Box<[Spanned<Expr>]>>, Spanned<ParserError>> {
    self.parse_inline_list(lo, EXPRS_IN_BRACES, |slf| slf.parse_expr(EXPR_TOKENS))
  }

  fn parse_var_decl_or_expr_list(
    &mut self,
    lo: SpanUnit,
  ) -> Result<Spanned<Box<[Spanned<VarOrExpr>]>>, Spanned<ParserError>> {
    self.parse_inline_list(lo, EXPRS_IN_BRACES, |slf| slf.parse_var_decl_or_expr())
  }

  fn parse_var_decl_or_expr(&mut self) -> Result<Spanned<VarOrExpr>, Spanned<ParserError>> {
    let Ok(ahead) = self.ahead();
    let Ok(shape) = statement_shape(ahead);

    match shape {
      Shape::Var => self.parse_var(None).span_map(VarOrExpr::Var),
      Shape::Expr => self.parse_expr(EXPR_TOKENS).span_map(VarOrExpr::Expr),
    }
  }

  fn parse_group_compat_decl(
    &mut self,
    lo: SpanUnit,
  ) -> Result<Spanned<GroupCompatDecl>, Spanned<ParserError>> {
    let (group_name, group) = self.parse_u32()?;
    self.consume_token(punctuation![=])?;
    let modifiers = self.parse_expr(EXPR_TOKENS)?;
    let semicolon = self.consume_token(punctuation![;])?;
    let ty = GroupCompatDecl {
      group_name,
      group,
      val: modifiers,
    };
    Ok(ty.spanned(lo, semicolon.hi))
  }

  fn parse_indicator_map_decl(
    &mut self,
    lo: SpanUnit,
  ) -> Result<Spanned<IndicatorMapDecl>, Spanned<ParserError>> {
    let named = self.parse_named_settings()?;
    let decl = IndicatorMapDecl {
      name: named.name,
      decls: named.decls,
    };
    Ok(decl.spanned(lo, named.hi))
  }

  fn parse_indicator_name_decl(
    &mut self,
    virt: Option<Span>,
    lo: SpanUnit,
  ) -> Result<Spanned<IndicatorNameDecl>, Spanned<ParserError>> {
    match virt {
      Some(_) => {
        let indicator = self.parse_ident()?;
        let Ok(meaning) = self
          .meaning_cache
          .get_case_insensitive(self.interner, indicator.val);

        match ::core::primitive::bool::then_some(meaning == Meaning::Indicator, ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => {
            let Ok(map) = indicator.map(Token::Ident);
            let Ok(unexpected_token) =
              self.unexpected_token(&[Expected::Ident(Meaning::Indicator)], map);
            return Err(unexpected_token);
          }
        }
      }
      None => {}
    }

    let (idx_name, idx) = self.parse_u32()?;
    self.consume_token(punctuation![=])?;
    let name = self.parse_expr(EXPR_TOKENS)?;
    let semicolon = self.consume_token(punctuation![;])?;
    let decl = IndicatorNameDecl {
      virt,
      idx_name,
      idx,
      val: name,
    };
    Ok(decl.spanned(lo, semicolon.hi))
  }

  fn parse_shape_decl(
    &mut self,
    lo: SpanUnit,
  ) -> Result<Spanned<ShapeDecl>, Spanned<ParserError>> {
    const EXPECTED: &[Expected] = &[
      Expected::Punctuation(Obracket),
      Expected::Nested(OUTLINE_TOKENS),
    ];
    let name = self.parse_string()?;
    let ob = self.consume_token(Obrace)?;
    let first = self.peek(EXPECTED)?;
    let shape_type = match ::core::primitive::bool::then_some(first.val == Obracket, ()) {
      ::core::option::Option::Some(()) => {
        let coords = self.parse_coord_list(ob.lo)?;
        let Ok(listed) = coords.map(ShapeDeclType::CoordList);
        listed
      }
      ::core::option::Option::None => {
        let outlines = self.parse_inline_list(ob.lo, OUTLINES, |slf| slf.parse_outline())?;
        let Ok(listed) = outlines.map(ShapeDeclType::OutlineList);
        listed
      }
    };
    let semicolon = self.consume_token(punctuation![;])?;
    let shape = ShapeDecl {
      name,
      ty: shape_type,
    };
    Ok(shape.spanned(lo, semicolon.hi))
  }

  fn parse_coord_list(
    &mut self,
    lo: SpanUnit,
  ) -> Result<Spanned<Box<[Spanned<Coord>]>>, Spanned<ParserError>> {
    self.parse_inline_list(lo, COORDS, |slf| slf.parse_coord())
  }

  fn parse_outline(&mut self) -> Result<Spanned<Outline>, Spanned<ParserError>> {
    const EXPECTED2: &[Expected] =
      &[Expected::Punctuation(Obrace), Expected::Nested(EXPR_TOKENS)];
    let t = self.next(OUTLINE_TOKENS)?;

    match ::core::primitive::bool::then_some(t.val == Obrace, ()) {
      ::core::option::Option::Some(()) => {
        return self
          .parse_coord_list(t.span.lo)
          .span_map(Outline::CoordList);
      }
      ::core::option::Option::None => {}
    }

    let name = match t.val {
      Token::Ident(name) => name,
      Token::String(_)
      | Token::KeyName(_)
      | Token::Integer(_, _)
      | Token::Float(_, _)
      | Token::Punctuation(_) => {
        let Ok(unexpected_token) = self.unexpected_token(OUTLINE_TOKENS, t);
        return Err(unexpected_token);
      }
    };
    self.consume_token(punctuation![=])?;
    let maybe_brace = self.peek(EXPECTED2)?;

    match ::core::primitive::bool::then_some(maybe_brace.val == Obrace, ()) {
      ::core::option::Option::Some(()) => {
        let obrace = self.consume_token(Obrace)?;
        let el = self.parse_coord_list(obrace.lo)?;
        let span = Span {
          lo: t.span.lo,
          hi: el.span.hi,
        };
        return Ok(
          Outline::CoordAssignment(CoordAssignment {
            name: name.spanned2(t.span),
            coords: el,
          })
          .spanned2(span),
        );
      }
      ::core::option::Option::None => {}
    }

    let expr = self.parse_expr(EXPECTED2)?;
    let span = Span {
      lo: t.span.lo,
      hi: expr.span.hi,
    };
    Ok(
      Outline::ExprAssignment(ExprAssignment {
        name: name.spanned2(t.span),
        expr,
      })
      .spanned2(span),
    )
  }

  fn parse_coord(&mut self) -> Result<Spanned<Coord>, Spanned<ParserError>> {
    let obracket = self.consume_token(Obracket)?;
    let x = self.parse_expr(EXPR_TOKENS)?;
    self.consume_token(punctuation![,])?;
    let y = self.parse_expr(EXPR_TOKENS)?;
    let cbracket = self.consume_token(Cbracket)?;
    let ty = Coord { x, y };
    Ok(ty.spanned(obracket.lo, cbracket.lo))
  }

  fn parse_section_decl(
    &mut self,
    lo: SpanUnit,
  ) -> Result<Spanned<SectionDecl>, Spanned<ParserError>> {
    let name = self.parse_string()?;
    let obrace = self.consume_token(Obrace)?;
    let items = self.parse_item_list(
      obrace.lo,
      Cbrace,
      &[
        Expected::Punctuation(Cbrace),
        Expected::Nested(SECTION_ITEM_EXPECTED),
      ],
      |slf| slf.parse_section_item(),
    )?;
    let semicolon = self.consume_token(punctuation![;])?;
    let ty = SectionDecl {
      name,
      items: items.val,
    };
    Ok(ty.spanned(lo, semicolon.hi))
  }

  fn parse_section_item(&mut self) -> Result<Spanned<SectionItem>, Spanned<ParserError>> {
    let t = self.next(SECTION_ITEM_EXPECTED)?;
    let Ok(detected) = self.detect_decl_candidate(None, t);

    match detected {
      DeclCandidate::Row => self.parse_row_decl(t.span.lo).span_map(SectionItem::Row),
      DeclCandidate::Overlay => self
        .parse_overlay_decl(t.span.lo)
        .span_map(SectionItem::Overlay),
      DeclCandidate::IndicatorMap => self
        .parse_indicator_map_decl(t.span.lo)
        .span_map(SectionItem::IndicatorMap),
      DeclCandidate::Doodad(doodad) => self
        .parse_doodad_decl(doodad.spanned2(t.span))
        .span_map(SectionItem::Doodad),
      DeclCandidate::Var(_) => self.parse_var_decl(Some(t)).span_map(SectionItem::Var),
      candidate @ (DeclCandidate::None
      | DeclCandidate::GroupCompat
      | DeclCandidate::Include(_, _)
      | DeclCandidate::Interpret
      | DeclCandidate::KeyAlias
      | DeclCandidate::KeyName(_)
      | DeclCandidate::Keys
      | DeclCandidate::KeyType
      | DeclCandidate::IndicatorName(_)
      | DeclCandidate::ModMap
      | DeclCandidate::Section
      | DeclCandidate::Shape
      | DeclCandidate::Symbols
      | DeclCandidate::Vmod) => {
        let Ok(unexpected) =
          self.unexpected_decl_candidate(SECTION_ITEM_EXPECTED, candidate, t.span);
        Err(unexpected)
      }
    }
  }

  fn parse_row_decl(&mut self, lo: SpanUnit) -> Result<Spanned<RowBody>, Spanned<ParserError>> {
    let obrace = self.consume_token(Obrace)?;
    let items =
      self.parse_item_list(obrace.lo, Cbrace, &[Expected::Punctuation(Cbrace)], |slf| {
        slf.parse_row_body_item()
      })?;
    let semicolon = self.consume_token(punctuation![;])?;
    Ok(RowBody { items: items.val }.spanned(lo, semicolon.hi))
  }

  fn parse_row_body_item(&mut self) -> Result<Spanned<RowBodyItem>, Spanned<ParserError>> {
    const EXPECTED: &[Expected] = &[Expected::Ident(Meaning::Keys), Expected::VarDecl];
    let t = self.next(EXPECTED)?;
    let Ok(detected) = self.detect_decl_candidate(None, t);

    match detected {
      DeclCandidate::Keys => self.parse_keys(t.span.lo).span_map(RowBodyItem::Keys),
      DeclCandidate::Var(_) => self.parse_var_decl(Some(t)).span_map(RowBodyItem::Var),
      candidate @ (DeclCandidate::None
      | DeclCandidate::Doodad(_)
      | DeclCandidate::GroupCompat
      | DeclCandidate::Include(_, _)
      | DeclCandidate::Interpret
      | DeclCandidate::KeyAlias
      | DeclCandidate::KeyName(_)
      | DeclCandidate::KeyType
      | DeclCandidate::IndicatorMap
      | DeclCandidate::IndicatorName(_)
      | DeclCandidate::ModMap
      | DeclCandidate::Overlay
      | DeclCandidate::Row
      | DeclCandidate::Section
      | DeclCandidate::Shape
      | DeclCandidate::Symbols
      | DeclCandidate::Vmod) => {
        let Ok(unexpected) = self.unexpected_decl_candidate(EXPECTED, candidate, t.span);
        Err(unexpected)
      }
    }
  }

  fn parse_keys(&mut self, lo: SpanUnit) -> Result<Spanned<Keys>, Spanned<ParserError>> {
    let obrace = self.consume_token(Obrace)?;
    let keys = self.parse_inline_list(obrace.lo, KEYS, |slf| slf.parse_key())?;
    let semicolon = self.consume_token(punctuation![;])?;
    let k = Keys { keys: keys.val };
    Ok(k.spanned(lo, semicolon.hi))
  }

  fn parse_key(&mut self) -> Result<Spanned<Key>, Spanned<ParserError>> {
    let t = self.next(KEY_EXPECTED)?;

    match t.val {
      Token::KeyName(i) => Ok(Key::Name(i).spanned2(t.span)),
      Token::Punctuation(Obrace) => {
        let expr = self.parse_var_decl_or_expr_list(t.span.lo)?;
        Ok(Key::Exprs(KeyExprs { exprs: expr.val }).spanned(t.span.lo, expr.span.hi))
      }
      Token::Ident(_)
      | Token::String(_)
      | Token::Integer(_, _)
      | Token::Float(_, _)
      | Token::Punctuation(_) => {
        let Ok(unexpected) = self.unexpected_token(KEY_EXPECTED, t);
        Err(unexpected)
      }
    }
  }

  fn parse_overlay_decl(
    &mut self,
    lo: SpanUnit,
  ) -> Result<Spanned<OverlayDecl>, Spanned<ParserError>> {
    let name = self.parse_string()?;
    let obrace = self.consume_token(Obrace)?;
    let items = self.parse_inline_list(obrace.lo, OVERLAY_ITEMS, |slf| {
      let key = slf.parse_keyname()?;
      slf.consume_token(punctuation![=])?;
      let alias = slf.parse_keyname()?;
      Ok(OverlayItem { name: key, alias }.spanned(key.span.lo, alias.span.hi))
    })?;
    let semicolon = self.consume_token(punctuation![;])?;
    let item = OverlayDecl {
      name,
      items: items.val,
    };
    Ok(item.spanned(lo, semicolon.hi))
  }

  fn detect_decl_candidate(
    &mut self,
    mm: Option<Spanned<MergeMode>>,
    t: Spanned<Token>,
  ) -> Result<DeclCandidate, Never> {
    Ok(match t.val {
      Token::KeyName(i) => DeclCandidate::KeyName(i),
      token![!] => DeclCandidate::Var(t.val),
      Token::String(i) => match mm {
        Some(mode) => DeclCandidate::Include(mode, i),
        None => DeclCandidate::None,
      },
      Token::Ident(i) => {
        let Ok(candidate) = self.word_candidate(i, t);
        candidate
      }
      Token::Integer(_, _) | Token::Float(_, _) | Token::Punctuation(_) => DeclCandidate::None,
    })
  }

  fn word_candidate(&mut self, i: Interned, t: Spanned<Token>) -> Result<DeclCandidate, Never> {
    let Ok(peeked) = self.try_peek();
    let following = peeked.map(|next| next.val);

    match following {
      Some(token![.] | token![=] | token![;] | Token::Punctuation(Obracket)) => {
        return Ok(DeclCandidate::Var(t.val));
      }
      Some(_) | None => {}
    }

    let Ok(meaning) = self.meaning_cache.get_case_insensitive(self.interner, i);
    let Ok(word) = named(DECL_WORDS, meaning);

    Ok(match word {
      Some(candidate) => candidate,
      None => match ::core::primitive::bool::then_some(meaning == Meaning::Indicator, ()) {
        ::core::option::Option::Some(()) => match following {
          Some(Token::String(_)) => DeclCandidate::IndicatorMap,
          Some(_) | None => DeclCandidate::IndicatorName(None),
        },
        ::core::option::Option::None => match ::core::primitive::bool::then_some(meaning == Meaning::Virtual, ()) {
          ::core::option::Option::Some(()) => DeclCandidate::IndicatorName(Some(t.span)),
          ::core::option::Option::None => DeclCandidate::None,
        },
      },
    })
  }

  fn parse_doodad_decl(
    &mut self,
    doodad: Spanned<DoodadType>,
  ) -> Result<Spanned<DoodadDecl>, Spanned<ParserError>> {
    let named = self.parse_named_settings()?;
    let decl = DoodadDecl {
      ty: doodad,
      name: named.name,
      decls: named.decls,
    };
    Ok(decl.spanned(doodad.span.lo, named.hi))
  }

  fn parse_item_list<E: Debug>(
    &mut self,
    lo: SpanUnit,
    terminator: Punctuation,
    expected: &'static [Expected],
    mut f: impl FnMut(&mut Self) -> Result<Spanned<E>, Spanned<ParserError>>,
  ) -> Result<Spanned<Box<[Spanned<E>]>>, Spanned<ParserError>> {
    let mut res = vec![];
    let Ok(rounds) = self.rounds();

    for _round in 0..rounds {
      let t = self.peek(expected)?;

      match ::core::primitive::bool::then_some(t.val == Token::Punctuation(terminator), ()) {
        ::core::option::Option::Some(()) => {
          let end = self.consume_token(terminator)?;
          return Ok(res.into_boxed_slice().spanned(lo, end.hi));
        }
        ::core::option::Option::None => {}
      }

      let saved_pos = self.pos;
      let parsed = f(self);

      match parsed {
        Ok(l) => {
          self.consume_runtime(l.span)?;
          res.push(l);
        }
        Err(err) => {
          self.consume_runtime(err.span)?;
          self.recovered(saved_pos, err)?;
        }
      }
    }

    let Ok(expected_but_eof) = self.expected_but_eof(expected);
    Err(expected_but_eof)
  }

  fn recovered(
    &mut self,
    saved_pos: u32,
    err: Spanned<ParserError>,
  ) -> Result<(), Spanned<ParserError>> {
    let diag = match &mut self.diag {
      Some(diag) => diag,
      None => {
        return Err(err);
      }
    };
    let Ok(after) = statement_end(self.tokens, saved_pos);
    self.pos = after;
    let Ok(diagnostic_kind) = err.val.diagnostic_kind();
    let Ok(()) = diag.diagnostics.push(diag.map, diagnostic_kind, err);

    Ok(())
  }

  fn list_open(&mut self, listing: Listing) -> Result<ListStep, Spanned<ParserError>> {
    let t = self.peek(listing.expected)?;

    match ::core::primitive::bool::then_some(t.val == Token::Punctuation(listing.terminator), ()) {
      ::core::option::Option::Some(()) => {
        let end = self.consume_token(listing.terminator)?;
        Ok(ListStep::Ended(end.hi))
      }
      ::core::option::Option::None => Ok(ListStep::Item),
    }
  }

  fn list_step(
    &mut self,
    listing: Listing,
    comma: Comma,
  ) -> Result<ListStep, Spanned<ParserError>> {
    let expected = match comma {
      Comma::NotDue => {
        return self.list_open(listing);
      }
      Comma::Due => listing.terminator_plus_comma,
    };
    let t = self.peek(expected)?;

    match ::core::primitive::bool::then_some(t.val == Token::Punctuation(listing.terminator), ()) {
      ::core::option::Option::Some(()) => {
        let end = self.consume_token(listing.terminator)?;
        return Ok(ListStep::Ended(end.hi));
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(t.val == token![,], ()) {
      ::core::option::Option::Some(()) => {
        self.pos = self.pos.saturating_add(1);
        self.list_open(listing)
      }
      ::core::option::Option::None => {
        let Ok(unexpected_token) = self.unexpected_token(expected, t);
        Err(unexpected_token)
      }
    }
  }

  fn parse_inline_list<T>(
    &mut self,
    lo: SpanUnit,
    listing: Listing,
    mut f: impl FnMut(&mut Self) -> Result<Spanned<T>, Spanned<ParserError>>,
  ) -> Result<Spanned<Box<[Spanned<T>]>>, Spanned<ParserError>> {
    let mut res = vec![];
    let mut comma = Comma::NotDue;
    let Ok(rounds) = self.rounds();

    for _round in 0..rounds {
      let step = self.list_step(listing, comma)?;

      match step {
        ListStep::Ended(hi) => {
          return Ok(res.into_boxed_slice().spanned(lo, hi));
        }
        ListStep::Item => {}
      }

      let v = f(self)?;
      self.consume_runtime(v.span)?;
      res.push(v);
      comma = Comma::Due;
    }

    let expected = match comma {
      Comma::Due => listing.terminator_plus_comma,
      Comma::NotDue => listing.expected,
    };
    let Ok(expected_but_eof) = self.expected_but_eof(expected);
    Err(expected_but_eof)
  }

  fn parse_string(&mut self) -> Result<Spanned<Interned>, Spanned<ParserError>> {
    let t = self.next(&[Expected::String])?;

    match t.val {
      Token::String(i) => Ok(i.spanned2(t.span)),
      Token::Ident(_)
      | Token::KeyName(_)
      | Token::Integer(_, _)
      | Token::Float(_, _)
      | Token::Punctuation(_) => {
        let Ok(unexpected) = self.unexpected_token(&[Expected::String], t);
        Err(unexpected)
      }
    }
  }

  fn parse_keyname(&mut self) -> Result<Spanned<Interned>, Spanned<ParserError>> {
    let t = self.next(&[Expected::KeyName])?;

    match t.val {
      Token::KeyName(i) => Ok(i.spanned2(t.span)),
      Token::Ident(_)
      | Token::String(_)
      | Token::Integer(_, _)
      | Token::Float(_, _)
      | Token::Punctuation(_) => {
        let Ok(unexpected) = self.unexpected_token(&[Expected::KeyName], t);
        Err(unexpected)
      }
    }
  }

  fn parse_vmod_decl(&mut self, lo: SpanUnit) -> Result<Spanned<VModDecl>, Spanned<ParserError>> {
    let defs = self.parse_inline_list(lo, VMOD_DEFS, |slf| {
      let name = slf.parse_ident()?;
      let Ok(equals) = slf.eat(punctuation![=]);
      let (val, hi) = match equals {
        Some(_) => {
          let e = slf.parse_expr(EXPR_TOKENS)?;
          let hi = e.span.hi;
          (Some(e), hi)
        }
        None => (None, name.span.hi),
      };

      Ok(VModDef { name, val }.spanned(name.span.lo, hi))
    })?;
    let ty = VModDecl { defs: defs.val };
    Ok(ty.spanned(lo, defs.span.hi))
  }

  fn parse_var(
    &mut self,
    first: Option<Spanned<Token>>,
  ) -> Result<Spanned<Var>, Spanned<ParserError>> {
    let t = match first {
      Some(given) => given,
      None => self.next(&[Expected::AnyIdent, Expected::Punctuation(punctuation![!])])?,
    };

    match ::core::primitive::bool::then_some(t.val == token![!], ()) {
      ::core::option::Option::Some(()) => {
        let path = self.parse_path(None)?;
        let hi = path.span.hi;
        let ty = Var {
          not: Some(t.span),
          path,
          expr: None,
        };
        return Ok(ty.spanned(t.span.lo, hi));
      }
      ::core::option::Option::None => {}
    }

    let i = match t.val {
      Token::Ident(i) => i,
      Token::String(_)
      | Token::KeyName(_)
      | Token::Integer(_, _)
      | Token::Float(_, _)
      | Token::Punctuation(_) => {
        const EXPECTED: &[Expected] =
          &[Expected::Punctuation(punctuation![!]), Expected::AnyIdent];
        let Ok(unexpected_token) = self.unexpected_token(EXPECTED, t);
        return Err(unexpected_token);
      }
    };
    let path = self.parse_path(Some(i.spanned2(t.span)))?;
    let Ok(equals) = self.eat(punctuation![=]);
    let (expr, hi) = match equals {
      Some(_) => {
        let e = self.parse_expr(EXPR_TOKENS)?;
        let hi = e.span.hi;
        (Some(e), hi)
      }
      None => (None, path.span.hi),
    };
    let ty = Var {
      not: None,
      path,
      expr,
    };
    Ok(ty.spanned(t.span.lo, hi))
  }

  fn parse_var_decl(
    &mut self,
    first: Option<Spanned<Token>>,
  ) -> Result<Spanned<VarDecl>, Spanned<ParserError>> {
    let mut res = self.parse_var(first)?;
    let semicolon = self.consume_token(punctuation![;])?;
    res.span.hi = semicolon.hi;
    let Ok(map) = res.map(|var| VarDecl { var });
    Ok(map)
  }

  fn parse_keyname_decl(
    &mut self,
    keyname: Spanned<Interned>,
  ) -> Result<Spanned<KeyNameDecl>, Spanned<ParserError>> {
    self.consume_token(punctuation![=])?;
    let (code_str, code) = self.parse_u32()?;
    let semicolon = self.consume_token(punctuation![;])?;
    let ty = KeyNameDecl {
      name: keyname,
      code_str,
      code: Keycode(code),
    };
    Ok(ty.spanned(keyname.span.lo, semicolon.hi))
  }

  fn parse_u32(&mut self) -> Result<(Spanned<Interned>, u32), Spanned<ParserError>> {
    let t = self.next(&[Expected::Integer])?;
    let (i, v) = match t.val {
      Token::Integer(i, v) => (i, v),
      Token::Ident(_)
      | Token::String(_)
      | Token::KeyName(_)
      | Token::Float(_, _)
      | Token::Punctuation(_) => {
        let Ok(unexpected_token) = self.unexpected_token(&[Expected::Integer], t);
        return Err(unexpected_token);
      }
    };
    let value = match u32::try_from(v) {
      Ok(value) => value,
      Err(_past_u32) => {
        let Ok(invalid_u32) = self.invalid_u32(i.spanned2(t.span));
        return Err(invalid_u32);
      }
    };

    Ok((i.spanned2(t.span), value))
  }

  fn parse_merge_mode(&mut self) -> Result<Option<Spanned<MergeMode>>, Never> {
    let Ok(peeked) = self.try_peek();
    let name = match peeked {
      Some(Spanned {
        val: Token::Ident(i),
        span,
      }) => i.spanned2(span),
      Some(_) | None => {
        return Ok(None);
      }
    };
    let Ok(meaning) = self
      .meaning_cache
      .get_case_insensitive(self.interner, name.val);
    let Ok(found) = named(MERGE_MODES, meaning);

    Ok(match found {
      Some(mode) => {
        self.pos = self.pos.saturating_add(1);
        Some(mode.spanned2(name.span))
      }
      None => None,
    })
  }

  fn parse_ident(&mut self) -> Result<Spanned<Interned>, Spanned<ParserError>> {
    let t = self.next(&[Expected::AnyIdent])?;

    match t.val {
      Token::Ident(i) => Ok(i.spanned2(t.span)),
      Token::String(_)
      | Token::KeyName(_)
      | Token::Integer(_, _)
      | Token::Float(_, _)
      | Token::Punctuation(_) => {
        let Ok(unexpected) = self.unexpected_token(&[Expected::AnyIdent], t);
        Err(unexpected)
      }
    }
  }

  fn consume_token(&mut self, token: Punctuation) -> Result<Span, Spanned<ParserError>> {
    let Ok(expected) = token.expected();
    let t = self.next(expected)?;

    match ::core::primitive::bool::then_some(t.val == Token::Punctuation(token), ()) {
      ::core::option::Option::Some(()) => Ok(t.span),
      ::core::option::Option::None => {
        let Ok(unexpected_token) = self.unexpected_token(expected, t);
        Err(unexpected_token)
      }
    }
  }

  fn parse_path(
    &mut self,
    first: Option<Spanned<Interned>>,
  ) -> Result<Spanned<Path>, Spanned<ParserError>> {
    let mut ident = match first {
      Some(given) => given,
      None => self.parse_ident()?,
    };
    let mut so_far = PathSoFar::default();
    let Ok(rounds) = self.rounds();

    for _round in 0..rounds {
      let component = self.path_component(ident)?;
      let grown = self.path_grown(so_far, component)?;

      match grown {
        Grown::More(longer, following) => {
          so_far = longer;
          ident = following;
        }
        Grown::Finished(path) => {
          return Ok(path);
        }
      }
    }

    let Ok(expected_but_eof) = self.expected_but_eof(&[Expected::AnyIdent]);
    Err(expected_but_eof)
  }

  fn path_component(
    &mut self,
    ident: Spanned<Interned>,
  ) -> Result<Spanned<PathComponent>, Spanned<ParserError>> {
    let Ok(opened) = self.eat(Obracket);

    match opened {
      Some(obracket) => {
        let index = self.parse_expr(EXPR_TOKENS)?;
        self.index_closed(ident, obracket, index)
      }
      None => Ok(PathComponent { ident, index: None }.spanned2(ident.span)),
    }
  }

  fn index_closed(
    &mut self,
    ident: Spanned<Interned>,
    obracket: Spanned<Token>,
    index: Spanned<Expr>,
  ) -> Result<Spanned<PathComponent>, Spanned<ParserError>> {
    let cbracket = self.consume_token(Cbracket)?;
    let component = PathComponent {
      ident,
      index: Some(Box::new(PathIndex { obracket, index })),
    };
    Ok(component.spanned(ident.span.lo, cbracket.hi))
  }

  fn path_grown(
    &mut self,
    mut so_far: PathSoFar,
    component: Spanned<PathComponent>,
  ) -> Result<Grown, Spanned<ParserError>> {
    let span = match so_far.span {
      None => component.span,
      Some(before) => {
        self.consume_runtime(component.span)?;
        Span {
          lo: before.lo,
          hi: component.span.hi,
        }
      }
    };
    so_far.components.push(component.val);
    so_far.span = Some(span);
    let Ok(dot) = self.eat(punctuation![.]);

    match dot {
      Some(_) => {
        let ident = self.parse_ident()?;
        Ok(Grown::More(so_far, ident))
      }
      None => {
        let Ok(path) = path_of(so_far.components);
        Ok(Grown::Finished(path.spanned2(span)))
      }
    }
  }

  fn named_arg_ahead(&mut self) -> Result<Argument, Never> {
    let Ok(first) = self.try_peek();
    let Ok(second) = self.try_peek2();

    Ok(
      match (first.map(|token| token.val), second.map(|token| token.val)) {
        (Some(Token::Ident(_)), Some(token![.] | token![=] | Token::Punctuation(Obracket))) => {
          Argument::Named
        }
        _ => Argument::Plain,
      },
    )
  }

  fn parse_expr(
    &mut self,
    expected: &'static [Expected],
  ) -> Result<Spanned<Expr>, Spanned<ParserError>> {
    let depth = self.expr_depth;
    let parsed = self.run_expr(expected);

    match parsed {
      Ok(expr) => Ok(expr),
      Err(e) => {
        self.expr_depth = depth;
        Err(e)
      }
    }
  }

  fn run_expr(
    &mut self,
    expected: &'static [Expected],
  ) -> Result<Spanned<Expr>, Spanned<ParserError>> {
    let mut frames = vec![];
    let mut next = Next::Begin(Begin::Expr(expected));
    let Ok(rounds) = self.rounds();

    for _step in 0..rounds.saturating_mul(STEPS_PER_TOKEN) {
      let stepped = match next {
        Next::Begin(begin) => self.begin(begin, &mut frames),
        Next::Deliver(value) => match frames.pop() {
          Some(frame) => self.reduce(frame, value, &mut frames),
          None => {
            return Ok(value);
          }
        },
      };
      let advanced = stepped?;
      next = advanced;
    }

    let Ok(span) = self.here();
    Err(ParserError::MaxRuntimeReached.spanned2(span))
  }

  fn deeper(&mut self, expected: &'static [Expected]) -> Result<(), Spanned<ParserError>> {
    match ::core::primitive::bool::then_some(self.expr_depth >= DEEPEST_EXPRESSION, ()) {
      ::core::option::Option::Some(()) => {
        let next = self.peek(expected)?;
        let Ok(too_deeply_nested) = self.too_deeply_nested(next.span);
        Err(too_deeply_nested)
      }
      ::core::option::Option::None => {
        self.expr_depth = self.expr_depth.saturating_add(1);
        Ok(())
      }
    }
  }

  fn begin(
    &mut self,
    begin: Begin,
    frames: &mut Vec<Frame>,
  ) -> Result<Next, Spanned<ParserError>> {
    match begin {
      Begin::Expr(expected) => {
        self.deeper(expected)?;
        frames.push(Frame::LeaveDepth);
        frames.push(Frame::Sum);
        Ok(Next::Begin(Begin::Product(expected)))
      }
      Begin::Product(expected) => {
        frames.push(Frame::Product);
        Ok(Next::Begin(Begin::Term(expected)))
      }
      Begin::DeepTerm => {
        self.deeper(EXPR_TOKENS)?;
        frames.push(Frame::LeaveDepth);
        Ok(Next::Begin(Begin::Term(EXPR_TOKENS)))
      }
      Begin::Term(expected) => self.term(expected, frames),
      Begin::Component(component) => self.component(*component, frames),
      Begin::ExprList(list) => self.expr_list(list, frames),
      Begin::ArgList(list) => self.arg_list(list, frames),
    }
  }

  fn term(
    &mut self,
    expected: &'static [Expected],
    frames: &mut Vec<Frame>,
  ) -> Result<Next, Spanned<ParserError>> {
    let t = self.next(expected)?;
    let lo = t.span.lo;
    let unary = match t.val {
      Token::Ident(i) => {
        return Ok(Next::Begin(Begin::Component(Box::new(Component {
          so_far: PathSoFar::default(),
          ident: i.spanned2(t.span),
          purpose: Purpose::Term,
        }))));
      }
      Token::String(s) => {
        return Ok(Next::Deliver(Expr::String(s).spanned2(t.span)));
      }
      Token::KeyName(k) => {
        return Ok(Next::Deliver(Expr::KeyName(k).spanned2(t.span)));
      }
      Token::Integer(i, v) => {
        return Ok(Next::Deliver(Expr::Integer(i, v).spanned2(t.span)));
      }
      Token::Float(i, v) => {
        return Ok(Next::Deliver(Expr::Float(i, v).spanned2(t.span)));
      }
      Token::Punctuation(Oparen) => {
        frames.push(Frame::Paren(lo));
        return Ok(Next::Begin(Begin::Expr(EXPR_TOKENS)));
      }
      Token::Punctuation(Obracket) => {
        return Ok(Next::Begin(Begin::ExprList(ExprList {
          lo,
          bracketing: Bracketing::Brackets,
          items: vec![],
          comma: Comma::NotDue,
        })));
      }
      Token::Punctuation(Obrace) => {
        return Ok(Next::Begin(Begin::ExprList(ExprList {
          lo,
          bracketing: Bracketing::Braces,
          items: vec![],
          comma: Comma::NotDue,
        })));
      }
      token![+] => Unary::Plus,
      token![-] => Unary::Minus,
      token![!] => Unary::Not,
      token![~] => Unary::Inverse,
      Token::Punctuation(_) => {
        let Ok(unexpected) = self.unexpected_token(expected, t);
        return Err(unexpected);
      }
    };
    frames.push(Frame::Unary(lo, unary));

    Ok(Next::Begin(Begin::DeepTerm))
  }

  fn component(
    &mut self,
    component: Component,
    frames: &mut Vec<Frame>,
  ) -> Result<Next, Spanned<ParserError>> {
    let Ok(opened) = self.eat(Obracket);

    match opened {
      Some(obracket) => {
        frames.push(Frame::PathIndex(Box::new(Indexing {
          component,
          obracket,
        })));
        Ok(Next::Begin(Begin::Expr(EXPR_TOKENS)))
      }
      None => {
        let plain = PathComponent {
          ident: component.ident,
          index: None,
        }
        .spanned2(component.ident.span);
        self.component_done(component.so_far, plain, component.purpose, frames)
      }
    }
  }

  fn component_done(
    &mut self,
    so_far: PathSoFar,
    done: Spanned<PathComponent>,
    purpose: Purpose,
    frames: &mut Vec<Frame>,
  ) -> Result<Next, Spanned<ParserError>> {
    let grown = self.path_grown(so_far, done)?;

    match grown {
      Grown::More(longer, ident) => Ok(Next::Begin(Begin::Component(Box::new(Component {
        so_far: longer,
        ident,
        purpose,
      })))),
      Grown::Finished(path) => self.path_finished(path, purpose, frames),
    }
  }

  fn path_finished(
    &mut self,
    path: Spanned<Path>,
    purpose: Purpose,
    frames: &mut Vec<Frame>,
  ) -> Result<Next, Spanned<ParserError>> {
    match purpose {
      Purpose::Term => {
        let Ok(called) = self.eat(Oparen);

        Ok(match called {
          Some(_) => Next::Begin(Begin::ArgList(ArgList {
            callee: path,
            items: vec![],
            comma: Comma::NotDue,
          })),
          None => Next::Deliver(Expr::Path(path.val).spanned2(path.span)),
        })
      }
      Purpose::Named(list) => {
        self.consume_token(punctuation![=])?;
        frames.push(Frame::NamedValue(list, path));
        Ok(Next::Begin(Begin::Expr(EXPR_TOKENS)))
      }
    }
  }

  fn expr_list(
    &mut self,
    list: ExprList,
    frames: &mut Vec<Frame>,
  ) -> Result<Next, Spanned<ParserError>> {
    let listing = match list.bracketing {
      Bracketing::Brackets => EXPRS_IN_BRACKETS,
      Bracketing::Braces => EXPRS_IN_BRACES,
    };
    let step = self.list_step(listing, list.comma)?;

    Ok(match step {
      ListStep::Ended(hi) => {
        let items = list.items.into_boxed_slice();
        let expr = match list.bracketing {
          Bracketing::Brackets => Expr::BracketList(items),
          Bracketing::Braces => Expr::BraceList(items),
        };
        Next::Deliver(expr.spanned(list.lo, hi))
      }
      ListStep::Item => {
        frames.push(Frame::ExprItem(list));
        Next::Begin(Begin::Expr(EXPR_TOKENS))
      }
    })
  }

  fn arg_list(
    &mut self,
    list: ArgList,
    frames: &mut Vec<Frame>,
  ) -> Result<Next, Spanned<ParserError>> {
    let step = self.list_step(ARGS_IN_PARENS, list.comma)?;
    let hi = match step {
      ListStep::Ended(hi) => hi,
      ListStep::Item => {
        let Ok(arg) = self.named_arg_ahead();

        return match arg {
          Argument::Named => {
            let ident = self.parse_ident()?;
            Ok(Next::Begin(Begin::Component(Box::new(Component {
              so_far: PathSoFar::default(),
              ident,
              purpose: Purpose::Named(list),
            }))))
          }
          Argument::Plain => {
            frames.push(Frame::ArgExpr(list));
            Ok(Next::Begin(Begin::Expr(EXPR_TOKENS)))
          }
        };
      }
    };
    let span = Span {
      lo: list.callee.span.lo,
      hi,
    };
    let call = Call {
      path: list.callee,
      args: list.items.into_boxed_slice(),
    };

    Ok(Next::Deliver(Expr::Call(Box::new(call)).spanned2(span)))
  }

  fn sum_more(&mut self, lhs: Spanned<Expr>, frames: &mut Vec<Frame>) -> Result<Next, Never> {
    let Ok(peeked) = self.try_peek();
    let sign = match peeked.map(|token| token.val) {
      Some(token![+]) => Sign::Plus,
      Some(token![-]) => Sign::Minus,
      Some(_) | None => {
        return Ok(Next::Deliver(lhs));
      }
    };
    self.pos = self.pos.saturating_add(1);
    frames.push(Frame::SumRight(Box::new(lhs), sign));

    Ok(Next::Begin(Begin::Product(EXPR_TOKENS)))
  }

  fn product_more(&mut self, lhs: Spanned<Expr>, frames: &mut Vec<Frame>) -> Result<Next, Never> {
    let Ok(peeked) = self.try_peek();
    let (product, op) = match peeked {
      Some(op) => match op.val {
        token![*] => (Product::Times, op),
        token![/] => (Product::Divide, op),
        Token::Punctuation(_)
        | Token::Ident(_)
        | Token::String(_)
        | Token::KeyName(_)
        | Token::Integer(_, _)
        | Token::Float(_, _) => {
          return Ok(Next::Deliver(lhs));
        }
      },
      None => {
        return Ok(Next::Deliver(lhs));
      }
    };
    self.pos = self.pos.saturating_add(1);
    frames.push(Frame::ProductRight(Box::new(lhs), product, op.span));

    Ok(Next::Begin(Begin::Term(EXPR_TOKENS)))
  }

  fn reduce(
    &mut self,
    frame: Frame,
    value: Spanned<Expr>,
    frames: &mut Vec<Frame>,
  ) -> Result<Next, Spanned<ParserError>> {
    match frame {
      Frame::LeaveDepth => {
        self.expr_depth = self.expr_depth.saturating_sub(1);
        Ok(Next::Deliver(value))
      }
      Frame::Sum => {
        let Ok(more) = self.sum_more(value, frames);
        Ok(more)
      }
      Frame::SumRight(lhs, sign) => {
        self.consume_runtime(value.span)?;
        let span = Span {
          lo: lhs.span.lo,
          hi: value.span.hi,
        };
        let sum = match sign {
          Sign::Plus => Expr::Add(lhs, Box::new(value)),
          Sign::Minus => Expr::Sub(lhs, Box::new(value)),
        };
        let Ok(more) = self.sum_more(sum.spanned2(span), frames);
        Ok(more)
      }
      Frame::Product => {
        let Ok(more) = self.product_more(value, frames);
        Ok(more)
      }
      Frame::ProductRight(lhs, product, op_span) => {
        self.consume_runtime(op_span)?;
        let span = Span {
          lo: lhs.span.lo,
          hi: value.span.hi,
        };
        let multiplied = match product {
          Product::Times => Expr::Mul(lhs, Box::new(value)),
          Product::Divide => Expr::Div(lhs, Box::new(value)),
        };
        let Ok(more) = self.product_more(multiplied.spanned2(span), frames);
        Ok(more)
      }
      Frame::Paren(lo) => {
        let cparen = self.consume_token(Cparen)?;
        Ok(Next::Deliver(
          Expr::Parenthesized(Box::new(value)).spanned(lo, cparen.hi),
        ))
      }
      Frame::Unary(lo, unary) => {
        let span = Span {
          lo,
          hi: value.span.hi,
        };
        let arg = Box::new(value);
        let expr = match unary {
          Unary::Plus => Expr::UnPlus(arg),
          Unary::Minus => Expr::UnMinus(arg),
          Unary::Not => Expr::UnNot(arg),
          Unary::Inverse => Expr::UnInverse(arg),
        };
        Ok(Next::Deliver(expr.spanned2(span)))
      }
      Frame::ExprItem(mut list) => {
        self.consume_runtime(value.span)?;
        list.items.push(value);
        list.comma = Comma::Due;
        Ok(Next::Begin(Begin::ExprList(list)))
      }
      Frame::PathIndex(indexing) => {
        let Indexing {
          component,
          obracket,
        } = *indexing;
        let done = self.index_closed(component.ident, obracket, value)?;
        self.component_done(component.so_far, done, component.purpose, frames)
      }
      Frame::ArgExpr(mut list) => {
        let arg = CallArg::Expr(value.val).spanned2(value.span);
        self.consume_runtime(arg.span)?;
        list.items.push(arg);
        list.comma = Comma::Due;
        Ok(Next::Begin(Begin::ArgList(list)))
      }
      Frame::NamedValue(mut list, name) => {
        let span = Span {
          lo: name.span.lo,
          hi: value.span.hi,
        };
        let arg = CallArg::NamedParam(NamedParam { name, expr: value }).spanned2(span);
        self.consume_runtime(arg.span)?;
        list.items.push(arg);
        list.comma = Comma::Due;
        Ok(Next::Begin(Begin::ArgList(list)))
      }
    }
  }
}
