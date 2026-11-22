//! A CSS 2.1 style sheet read as far as an SVG needs: rules, selectors and
//! declarations, with the selectors matched against whatever tree the caller
//! hands in.
//!
//! The crate began as simplecss, by the Linebender authors under Apache-2.0 or
//! MIT, carried in `forks/` for the SVG reader, and it came into the tree
//! under the rules. It is as simple as it was: at-rules are skipped, a value
//! is the text between its colon and its semicolon rather than anything
//! parsed, a keyword is case sensitive and a unicode escape is not read. Rules
//! come out sorted by specificity, which is what lets a caller apply them in
//! order and have the more specific one win.
//!
//! The text is read off the front of what is left of it rather than at an
//! offset into it, so nothing is indexed and no position can pass the end.
//! A selector's ancestors are walked with a stack of candidates rather than
//! by calling itself, so a deeply nested document asks for no more stack than
//! a shallow one. What nobody read was left behind: a selector or a
//! declaration tokenized one token at a time, the text position every fault
//! used to carry, a sheet written back out as text, and the build without the
//! standard library.

mod selector;
mod stream;

use std::fmt;

use console_core_logging::{debug, warn};
use console_core_never::Never;
use console_core_number_conversion::fitted;

pub use selector::{AttributeOperator, Element, Matching, PseudoClass, Selector};
use stream::{Stream, Taken};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
  UnexpectedEnd,
  InvalidIdent,
  InvalidComment,
  InvalidValue,
  InvalidByte,
  Misaligned,
  SelectorMissing,
  UnexpectedSelector,
  UnexpectedCombinator,
  InvalidAttributeSelector,
  InvalidLanguagePseudoClass,
  UnsupportedPseudoClass,
}

impl fmt::Display for Fault {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Fault::UnexpectedEnd => write!(to, "unexpected end of stream"),
      Fault::InvalidIdent => write!(to, "invalid ident"),
      Fault::InvalidComment => write!(to, "unclosed comment"),
      Fault::InvalidValue => write!(to, "invalid value"),
      Fault::InvalidByte => write!(to, "an unexpected byte"),
      Fault::Misaligned => write!(to, "a span that does not belong to the text"),
      Fault::SelectorMissing => write!(to, "selector missing"),
      Fault::UnexpectedSelector => write!(to, "unexpected selector"),
      Fault::UnexpectedCombinator => write!(to, "unexpected combinator"),
      Fault::InvalidAttributeSelector => write!(to, "invalid or unsupported attribute selector"),
      Fault::InvalidLanguagePseudoClass => write!(to, "invalid language pseudo-class"),
      Fault::UnsupportedPseudoClass => write!(to, "a pseudo-class that is not supported"),
    }
  }
}

impl std::error::Error for Fault {}

impl From<Never> for Fault {
  fn from(never: Never) -> Fault {
    match never {}
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Declaration<'a> {
  pub name: &'a str,
  pub value: &'a str,
  pub important: bool,
}

#[derive(Debug, Clone)]
pub struct Rule<'a> {
  pub selector: Selector<'a>,
  pub declarations: Vec<Declaration<'a>>,
}

#[derive(Debug, Clone, Default)]
pub struct StyleSheet<'a> {
  pub rules: Vec<Rule<'a>>,
}

impl<'a> StyleSheet<'a> {
  pub fn parse_more(&mut self, text: &'a str) -> Result<(), Never> {
    let mut s = Stream::from(text);

    match s.skip_spaces_and_comments() {
      Ok(()) => {}
      Err(_unclosed) => return Ok(()),
    }

    let Ok(bound) = fitted::<_, u32>(text.len());

    for _statement in 0..=bound {
      let Ok(first) = s.first();

      match first {
        Some(_byte) => {}
        None => break,
      }

      match s.skip_spaces_and_comments() {
        Ok(()) => {}
        Err(_unclosed) => break,
      }

      match consume_statement(&mut s, &mut self.rules) {
        Ok(()) => {}
        Err(fault) => debug!("a statement was skipped: {}", fault),
      }
    }

    let Ok(left) = s.budget();

    match left {
      0 => {}
      bytes => warn!("{} bytes were left.", bytes),
    }

    self.rules.retain(|rule| !rule.declarations.is_empty());
    self.rules.sort_by_cached_key(|rule| {
      let Ok(specificity) = rule.selector.specificity();

      specificity
    });

    Ok(())
  }
}

fn consume_statement<'a>(s: &mut Stream<'a>, rules: &mut Vec<Rule<'a>>) -> Result<(), Fault> {
  let Ok(first) = s.first();

  match first {
    Some(b'@') => {
      s.bump()?;
      consume_at_rule(s)
    }
    Some(_) | None => consume_rule_set(s, rules),
  }
}

fn consume_at_rule(s: &mut Stream<'_>) -> Result<(), Fault> {
  let ident = s.consume_ident()?;

  warn!("The @{} rule is not supported. Skipped.", ident);

  s.skip_while(|c| c != ';' && c != '{')?;

  let Ok(first) = s.first();
  let byte = first.ok_or(Fault::UnexpectedEnd)?;

  match byte {
    b';' => {
      s.bump()?;
    }
    b'{' => {
      s.bump()?;
      s.skip_block()?;
    }
    _other => {}
  }

  Ok(())
}

fn consume_rule_set<'a>(s: &mut Stream<'a>, rules: &mut Vec<Rule<'a>>) -> Result<(), Fault> {
  let mut selectors = Vec::new();
  let bound = s.budget()?;

  for _selector in 0..=bound {
    let Ok(first) = s.first();
    let byte = first.ok_or(Fault::UnexpectedEnd)?;

    match (byte, selectors.is_empty()) {
      (b',', _) | (_, true) => {}
      (_, false) => break,
    }

    s.try_consume(",")?;

    match selector::parse(s) {
      Ok(selector) => selectors.push(selector),
      Err(fault) => warn!("Selector parsing failed cause {}.", fault),
    }

    s.skip_spaces()?;

    let Ok(next) = s.first();
    let delimiter = next.ok_or(Fault::UnexpectedEnd)?;

    match delimiter {
      b'{' => break,
      b',' => {}
      _other => {
        s.skip_while(|c| c != '{')?;
        break;
      }
    }
  }

  s.try_consume("{")?;

  let declarations = consume_declarations(s)?;

  rules.extend(selectors.into_iter().map(|selector| Rule {
    selector,
    declarations: declarations.clone(),
  }));

  s.try_consume("}")?;

  Ok(())
}

fn consume_declarations<'a>(s: &mut Stream<'a>) -> Result<Vec<Declaration<'a>>, Fault> {
  let mut declarations = Vec::new();
  let bound = s.budget()?;

  for _declaration in 0..=bound {
    let Ok(first) = s.first();

    match first {
      None | Some(b'}') => break,
      Some(_byte) => {}
    }

    match consume_declaration(s) {
      Ok(declaration) => declarations.push(declaration),
      Err(_invalid) => {
        s.skip_block()?;
        break;
      }
    }
  }

  Ok(declarations)
}

pub struct DeclarationTokenizer<'a> {
  stream: Stream<'a>,
}

impl<'a> From<&'a str> for DeclarationTokenizer<'a> {
  fn from(text: &'a str) -> Self {
    DeclarationTokenizer {
      stream: Stream::from(text),
    }
  }
}

impl<'a> Iterator for DeclarationTokenizer<'a> {
  type Item = Declaration<'a>;

  fn next(&mut self) -> Option<Self::Item> {
    match self.stream.skip_spaces_and_comments() {
      Ok(()) => {}
      Err(_unclosed) => return None,
    }

    let Ok(first) = self.stream.first();

    match first {
      None => None,
      Some(_byte) => match consume_declaration(&mut self.stream) {
        Ok(declaration) => Some(declaration),
        Err(_invalid) => {
          let Ok(()) = self.stream.exhaust();

          None
        }
      },
    }
  }
}

fn consume_declaration<'a>(s: &mut Stream<'a>) -> Result<Declaration<'a>, Fault> {
  s.skip_spaces_and_comments()?;
  s.try_consume("*")?;

  let name = s.consume_ident()?;

  s.skip_spaces_and_comments()?;
  s.consume(":")?;
  s.skip_spaces_and_comments()?;

  let start = *s;
  let mut end = *s;
  let bound = s.budget()?;

  for _term in 0..=bound {
    match consume_term(s) {
      Ok(()) => {
        end = *s;
        s.skip_spaces_and_comments()?;
      }
      Err(_unterminated) => break,
    }
  }

  let walked = end.since(start)?;
  let value = walked.trim();

  s.skip_spaces_and_comments()?;

  let Ok(bang) = s.try_consume("!");

  let important = match bang {
    Taken::Yes => {
      s.skip_spaces_and_comments()?;

      let Ok(word) = s.try_consume("important");

      match word {
        Taken::Yes => true,
        Taken::No => false,
      }
    }
    Taken::No => false,
  };

  s.skip_spaces_and_comments()?;

  let semicolons = s.budget()?;

  for _semicolon in 0..=semicolons {
    let Ok(semicolon) = s.try_consume(";");

    match semicolon {
      Taken::Yes => {
        s.skip_spaces_and_comments()?;
      }
      Taken::No => break,
    }
  }

  s.skip_spaces_and_comments()?;

  match ::core::primitive::bool::then_some(value.is_empty(), ()) {
    ::core::option::Option::Some(()) => Err(Fault::InvalidValue),
    ::core::option::Option::None => Ok(Declaration {
      name,
      value,
      important,
    }),
  }
}

fn consume_term(s: &mut Stream<'_>) -> Result<(), Fault> {
  let Ok(first) = s.first();
  let byte = first.ok_or(Fault::UnexpectedEnd)?;

  match byte {
    b'#' => {
      s.bump()?;

      match s.consume_ident() {
        Ok(_name) => {}
        Err(_not_a_name) => {
          s.skip_while(|c| c.is_ascii_hexdigit())?;
        }
      }
    }
    b'+' | b'-' | b'0'..=b'9' | b'.' => return consume_number(s),
    b'\'' | b'"' => {
      s.consume_string()?;
    }
    b',' => {
      s.bump()?;
    }
    _other => {
      s.consume_ident()?;

      let Ok(parenthesis) = s.try_consume("(");

      match parenthesis {
        Taken::Yes => {
          s.skip_while(|c| c != ')')?;
          s.consume(")")?;
        }
        Taken::No => {}
      }
    }
  }

  Ok(())
}

fn consume_number(s: &mut Stream<'_>) -> Result<(), Fault> {
  s.bump()?;
  s.skip_while(|c| c.is_ascii_digit())?;

  let Ok(point) = s.try_consume(".");

  match point {
    Taken::Yes => {
      s.skip_while(|c| c.is_ascii_digit())?;
    }
    Taken::No => {}
  }

  let Ok(percent) = s.try_consume("%");

  match percent {
    Taken::Yes => Ok(()),
    Taken::No => match s.consume_ident() {
      Ok(_suffix) => Ok(()),
      Err(_no_optional_unit) => Ok(()),
    },
  }
}
