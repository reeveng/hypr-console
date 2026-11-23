use console_core_never::Never;
pub(crate) mod error;

use crate::config::DEFAULT_INCLUDE_DIR;
use crate::config::EXTRA_INCLUDE_DIR;
use crate::xkb::code::Code;
use crate::xkb::code_map::CodeMap;
use crate::xkb::diagnostic::DiagnosticKind;
use crate::xkb::diagnostic::DiagnosticSink;
use crate::xkb::group::GroupIdx;
use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use crate::xkb::meaning::Meaning;
use crate::xkb::meaning::MeaningCache;
use crate::xkb::rmlvo::parser::error::AFTER_EXCLAM;
use crate::xkb::rmlvo::parser::error::ANY_IDENT;
use crate::xkb::rmlvo::parser::error::Expected;
use crate::xkb::rmlvo::parser::error::MAPPING_KEY;
use crate::xkb::rmlvo::parser::error::MAPPING_VALUE;
use crate::xkb::rmlvo::parser::error::MLVO;
use crate::xkb::rmlvo::parser::error::ParserError;
use crate::xkb::rmlvo::parser::error::RULE_KEY;
use crate::xkb::rmlvo::parser::error::START_OF_LINE;
use crate::xkb::rmlvo::token::Punctuation;
use crate::xkb::rmlvo::token::Token;
use crate::xkb::span::Span;
use crate::xkb::span::SpanExt;
use crate::xkb::span::SpanUnit;
use crate::xkb::span::Spanned;
use console_core_number_conversion::FromBytes;
use console_core_number_conversion::fitted;
use console_input_keymaps_derive::ad_hoc_display;

#[derive(Default)]
pub(crate) struct ParserCache {
  group_assignments: Vec<Interned>,
  mapping_keys: Vec<MappingKey>,
  mapping_values: Vec<MappingValue>,
  rule_keys: Vec<RuleKey>,
  rule_values: Vec<RuleValue>,
}

struct Parser<'a, 'b, 'c> {
  map: &'a mut CodeMap,
  diagnostics: &'a mut DiagnosticSink<'b, 'c>,
  rest: &'a [Spanned<Token>],
  end: Span,
  interner: &'a mut Interner,
  meaning_cache: &'a mut MeaningCache,
}

#[derive(Debug)]
pub(crate) enum Line<'a> {
  Macro(Interned, &'a [Interned]),
  Include(Interned),
  Mapping(&'a [MappingKey], &'a [MappingValue]),
  Rule(&'a [RuleKey], &'a [RuleValue]),
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum MappingKey {
  Model,
  Option,
  Layout(Option<MappingKeyIndex>),
  Variant(Option<MappingKeyIndex>),
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum MappingValue {
  Keycodes,
  Types,
  Compat,
  Symbols,
  Geometry,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum MappingKeyIndex {
  Single,
  First,
  Later,
  Any,
  Value(GroupIdx),
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum RuleKey {
  Star,
  Any,
  Some,
  None,
  Macro(Interned),
  Ident(Interned),
}

#[derive(Debug)]
pub(crate) struct RuleValue {
  pub(crate) ident: Spanned<Interned>,
}

const LAYOUT: &[u8] = b"layout";
const VARIANT: &[u8] = b"variant";

const MAPPING_VALUES: [(Meaning, MappingValue); 5] = [
  (Meaning::Keycodes, MappingValue::Keycodes),
  (Meaning::Symbols, MappingValue::Symbols),
  (Meaning::Types, MappingValue::Types),
  (Meaning::Compat, MappingValue::Compat),
  (Meaning::Geometry, MappingValue::Geometry),
];

const RULE_KEYWORDS: [(Meaning, RuleKey); 3] = [
  (Meaning::LessThan_Any_GreaterThan, RuleKey::Any),
  (Meaning::LessThan_None_GreaterThan, RuleKey::None),
  (Meaning::LessThan_Some_GreaterThan, RuleKey::Some),
];

pub(crate) fn parse_line<'a>(
  map: &mut CodeMap,
  diagnostics: &mut DiagnosticSink,
  interner: &mut Interner,
  cache: &'a mut ParserCache,
  meaning_cache: &mut MeaningCache,
  tokens: &[Spanned<Token>],
  end: Span,
) -> Result<Spanned<Line<'a>>, Spanned<ParserError>> {
  Parser {
    map,
    diagnostics,
    rest: tokens,
    end,
    interner,
    meaning_cache,
  }
  .parse_line(cache)
}

fn mapping_value(meaning: Meaning) -> Result<Option<MappingValue>, Never> {
  Ok(
    MAPPING_VALUES
      .iter()
      .find(|(named, _value)| meaning == *named)
      .map(|(_named, value)| *value),
  )
}

fn rule_keyword(meaning: Meaning, ident: Interned) -> Result<RuleKey, Never> {
  let keyword = RULE_KEYWORDS
    .iter()
    .find(|(named, _key)| meaning == *named)
    .map(|(_named, key)| *key);

  Ok(match keyword {
    Some(key) => key,
    None => RuleKey::Ident(ident),
  })
}

fn expand_escapes(
  map: &mut CodeMap,
  diagnostics: &mut DiagnosticSink,
  ident: Spanned<Interned>,
  bytes: &[u8],
) -> Result<Vec<u8>, Never> {
  let mut res = vec![];
  let mut last_was_percent = false;

  for (offset, c) in (0_u32..).zip(bytes.iter().copied()) {
    match ::core::primitive::bool::then_some(last_was_percent, ()) {
      ::core::option::Option::Some(()) => {
        last_was_percent = false;
        let lo = ident.span.lo.saturating_add(offset).saturating_sub(1);
        let Ok(()) = expand_escape(map, diagnostics, lo, c, &mut res);
      }
      ::core::option::Option::None => match ::core::primitive::bool::then_some(c == b'%', ()) {
        ::core::option::Option::Some(()) => {
          last_was_percent = true;
        }
        ::core::option::Option::None => {
          res.push(c);
        }
      },
    }
  }

  Ok(res)
}

fn expand_escape(
  map: &mut CodeMap,
  diagnostics: &mut DiagnosticSink,
  lo: SpanUnit,
  c: u8,
  res: &mut Vec<u8>,
) -> Result<(), Never> {
  let hi = lo.saturating_add(2);

  match c {
    b'%' => res.extend_from_slice(b"%"),
    b'H' => {
      let Ok(()) = diagnostics.push(
        map,
        DiagnosticKind::HomeNotSet,
        ad_hoc_display!("$HOME is not set").spanned(lo, hi),
      );
      res.push(b'%');
      res.push(c);
    }
    b'S' => {
      res.extend_from_slice(DEFAULT_INCLUDE_DIR.as_bytes());
      res.extend_from_slice(b"/rules");
    }
    b'E' => {
      res.extend_from_slice(EXTRA_INCLUDE_DIR.as_bytes());
      res.extend_from_slice(b"/rules");
    }
    _ => {
      let Ok(()) = diagnostics.push(
        map,
        DiagnosticKind::UnknownRulesEscapeSequence,
        ad_hoc_display!("unknown escape sequence %{}", char::from(c) => char).spanned(lo, hi),
      );
      res.push(b'%');
      res.push(c);
    }
  }

  Ok(())
}

impl Parser<'_, '_, '_> {
  fn next(
    &mut self,
    expected: &'static [Expected],
  ) -> Result<Spanned<Token>, Spanned<ParserError>> {
    match self.rest.split_first() {
      Some((first, rest)) => {
        self.rest = rest;
        Ok(*first)
      }
      None => {
        let Ok(expected_but_eof) = self.expected_but_eof(self.end, expected);
        Err(expected_but_eof)
      }
    }
  }

  fn skip(&mut self) -> Result<(), Never> {
    self.rest = match self.rest.split_first() {
      Some((_, rest)) => rest,
      None => &[],
    };

    Ok(())
  }

  fn peek(&self, expected: &'static [Expected]) -> Result<Spanned<Token>, Spanned<ParserError>> {
    match self.rest.first() {
      Some(first) => Ok(*first),
      None => {
        let Ok(expected_but_eof) = self.expected_but_eof(self.end, expected);
        Err(expected_but_eof)
      }
    }
  }

  fn ident(
    &self,
    token: Spanned<Token>,
    expected: &'static [Expected],
  ) -> Result<Spanned<Interned>, Spanned<ParserError>> {
    match token.val {
      Token::Ident(i) => Ok(i.spanned2(token.span)),
      Token::GroupName(_) | Token::Punctuation(_) => {
        let Ok(unexpected_token) = self.unexpected_token(expected, token);
        Err(unexpected_token)
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

  fn parse_line<'a>(
    &mut self,
    cache: &'a mut ParserCache,
  ) -> Result<Spanned<Line<'a>>, Spanned<ParserError>> {
    let t = self.peek(START_OF_LINE)?;

    match ::core::primitive::bool::then_some(t.val == Token::Punctuation(Punctuation::Exclam), ())
    {
      ::core::option::Option::Some(()) => self.parse_exclam_line(cache),
      ::core::option::Option::None => self.parse_rule(cache),
    }
  }

  fn parse_exclam_line<'a>(
    &mut self,
    cache: &'a mut ParserCache,
  ) -> Result<Spanned<Line<'a>>, Spanned<ParserError>> {
    let Ok(()) = self.skip();
    let t = self.peek(AFTER_EXCLAM)?;

    match t.val {
      Token::Ident(i) => {
        let Ok(meaning) = self.meaning_cache.get_case_sensitive(self.interner, i);

        match ::core::primitive::bool::then_some(meaning == Meaning::Include, ()) {
          ::core::option::Option::Some(()) => self.parse_include(),
          ::core::option::Option::None => self.parse_mapping(cache),
        }
      }
      Token::Punctuation(Punctuation::Equals) => self.parse_mapping(cache),
      Token::GroupName(g) => self.parse_group_assignment(cache, g.spanned2(t.span)),
      Token::Punctuation(Punctuation::Times | Punctuation::Exclam) => {
        let Ok(unexpected_token) = self.unexpected_token(AFTER_EXCLAM, t);
        Err(unexpected_token)
      }
    }
  }

  fn parse_group_assignment<'a>(
    &mut self,
    cache: &'a mut ParserCache,
    g: Spanned<Interned>,
  ) -> Result<Spanned<Line<'a>>, Spanned<ParserError>> {
    let lo = g.span.lo;
    let Ok(()) = self.skip();
    let equals = self.consume_token(Punctuation::Equals)?;
    let mut hi = equals.hi;
    cache.group_assignments.clear();
    let names = self.rest;
    self.rest = &[];

    for t in names {
      let ident = self.ident(*t, ANY_IDENT)?;
      hi = ident.span.hi;
      cache.group_assignments.push(ident.val);
    }

    let line: Line<'a> = Line::Macro(g.val, &cache.group_assignments);
    Ok(line.spanned(lo, hi))
  }

  fn parse_include<'a>(&mut self) -> Result<Spanned<Line<'a>>, Spanned<ParserError>> {
    let Ok(()) = self.skip();
    let t = self.next(ANY_IDENT)?;
    let ident = self.ident(t, ANY_IDENT)?;

    match self.rest.first() {
      Some(after) => {
        let Ok(expected_eol) = self.expected_eol(*after);
        return Err(expected_eol);
      }
      None => {}
    }

    let Ok(val) = self.interner.get(ident.val);

    match ::core::primitive::bool::then_some(val.contains(&b'%'), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        let Ok(map) = ident.map(Line::Include);
        return Ok(map);
      }
    }

    let Ok(bytes) = val.as_bytes();
    let Ok(res) = expand_escapes(self.map, self.diagnostics, ident, bytes);
    let Ok(code) = Code::owning(res);
    let Ok(slice) = code.to_slice();
    let Ok(interned) = self.interner.intern(&slice);
    Ok(Line::Include(interned).spanned2(ident.span))
  }

  fn parse_mapping<'a>(
    &mut self,
    cache: &'a mut ParserCache,
  ) -> Result<Spanned<Line<'a>>, Spanned<ParserError>> {
    let first = self.peek(RULE_KEY)?;
    let lo = first.span.lo;
    cache.mapping_keys.clear();
    cache.mapping_values.clear();
    let mut tokens = self.rest.iter();
    let mut equals = None;

    for t in tokens.by_ref() {
      let i = match t.val {
        Token::Punctuation(Punctuation::Equals) => {
          equals = Some(t.span);
          break;
        }
        Token::Ident(i) => i,
        Token::GroupName(_) | Token::Punctuation(Punctuation::Times | Punctuation::Exclam) => {
          let Ok(unexpected_token) = self.unexpected_token(MLVO, *t);
          return Err(unexpected_token);
        }
      };
      let key = self.mapping_key(i.spanned2(t.span), *t)?;
      cache.mapping_keys.push(key);
    }

    let mut hi = match equals {
      Some(span) => span.hi,
      None => {
        let Ok(expected_but_eof) = self.expected_but_eof(self.end, MAPPING_KEY);
        return Err(expected_but_eof);
      }
    };
    let values = tokens.as_slice();
    self.rest = &[];

    for t in values {
      let ident = self.ident(*t, MAPPING_VALUE)?;
      hi = ident.span.hi;
      let Ok(meaning) = self
        .meaning_cache
        .get_case_sensitive(self.interner, ident.val);
      let Ok(found) = mapping_value(meaning);

      match found {
        Some(value) => cache.mapping_values.push(value),
        None => {
          let Ok(unexpected_token) = self.unexpected_token(MAPPING_VALUE, *t);
          return Err(unexpected_token);
        }
      }
    }

    Ok(Line::Mapping(&cache.mapping_keys, &cache.mapping_values).spanned(lo, hi))
  }

  fn mapping_key(
    &self,
    ident: Spanned<Interned>,
    token: Spanned<Token>,
  ) -> Result<MappingKey, Spanned<ParserError>> {
    let Ok(interned) = self.interner.get(ident.val);
    let Ok(name) = interned.as_bytes();

    match name {
      b"model" => Ok(MappingKey::Model),
      b"option" => Ok(MappingKey::Option),
      named => match (named.strip_prefix(LAYOUT), named.strip_prefix(VARIANT)) {
        (Some(suffix), _) => {
          let Ok(offset) = fitted::<_, u32>(LAYOUT.len());
          let index = self.mapping_key_index(ident, offset, suffix)?;
          Ok(MappingKey::Layout(index))
        }
        (None, Some(suffix)) => {
          let Ok(offset) = fitted::<_, u32>(VARIANT.len());
          let index = self.mapping_key_index(ident, offset, suffix)?;
          Ok(MappingKey::Variant(index))
        }
        (None, None) => {
          let Ok(unexpected_token) = self.unexpected_token(MAPPING_KEY, token);
          Err(unexpected_token)
        }
      },
    }
  }

  fn mapping_key_index(
    &self,
    ident: Spanned<Interned>,
    offset: u32,
    suffix: &[u8],
  ) -> Result<Option<MappingKeyIndex>, Spanned<ParserError>> {
    let inside = match suffix {
      [] => return Ok(None),
      [b'[', inside @ .., b']'] => inside,
      [b'[', .., last] | [last @ b'['] => {
        let Ok(index_end) = self.index_end(ident, *last);
        return Err(index_end);
      }
      [found, ..] => {
        let Ok(index_start) = self.index_start(ident, offset, *found);
        return Err(index_start);
      }
    };

    let named = match inside {
      b"single" => MappingKeyIndex::Single,
      b"first" => MappingKeyIndex::First,
      b"later" => MappingKeyIndex::Later,
      b"any" => MappingKeyIndex::Any,
      number => {
        let group = match u32::from_bytes_dec(number) {
          Some(value) => {
            let Ok(group) = GroupIdx::new(value);
            group
          }
          None => None,
        };

        match group {
          Some(idx) => MappingKeyIndex::Value(idx),
          None => {
            let Ok(index) = self.index(ident, offset);
            return Err(index);
          }
        }
      }
    };

    Ok(Some(named))
  }

  fn parse_rule<'a>(
    &mut self,
    cache: &'a mut ParserCache,
  ) -> Result<Spanned<Line<'a>>, Spanned<ParserError>> {
    cache.rule_keys.clear();
    cache.rule_values.clear();
    let first = self.peek(RULE_KEY)?;
    let lo = first.span.lo;
    let mut tokens = self.rest.iter();
    let mut equals = None;

    for t in tokens.by_ref() {
      let key = match t.val {
        Token::Punctuation(Punctuation::Equals) => {
          equals = Some(t.span);
          break;
        }
        Token::Punctuation(Punctuation::Times) => RuleKey::Star,
        Token::Ident(i) => {
          let Ok(meaning) = self.meaning_cache.get_case_insensitive(self.interner, i);
          let Ok(keyword) = rule_keyword(meaning, i);
          keyword
        }
        Token::GroupName(i) => RuleKey::Macro(i),
        Token::Punctuation(Punctuation::Exclam) => {
          let Ok(unexpected_token) = self.unexpected_token(RULE_KEY, *t);
          return Err(unexpected_token);
        }
      };
      cache.rule_keys.push(key);
    }

    let mut hi = match equals {
      Some(span) => span.hi,
      None => {
        let Ok(expected_but_eof) = self.expected_but_eof(self.end, RULE_KEY);
        return Err(expected_but_eof);
      }
    };
    let values = tokens.as_slice();
    self.rest = &[];

    for t in values {
      let ident = self.ident(*t, ANY_IDENT)?;
      hi = ident.span.hi;
      cache.rule_values.push(RuleValue { ident });
    }

    Ok(Line::Rule(&cache.rule_keys, &cache.rule_values).spanned(lo, hi))
  }
}
