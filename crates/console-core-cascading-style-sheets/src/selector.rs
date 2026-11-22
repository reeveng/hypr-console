//! A selector matches one element by following its ancestors and siblings.
//! Implementations yield ancestors nearest first and compare elements by node identity.

use std::collections::BTreeSet;

use console_core_never::Never;

use crate::Fault;
use crate::stream::Stream;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Matching {
  Yes,
  No,
}

impl From<bool> for Matching {
  fn from(answer: bool) -> Matching {
    match ::core::primitive::bool::then_some(answer, ()) {
      ::core::option::Option::Some(()) => Matching::Yes,
      ::core::option::Option::None => Matching::No,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttributeOperator<'a> {
  Exists,
  Matches(&'a str),
  Contains(&'a str),
  StartsWith(&'a str),
}

impl AttributeOperator<'_> {
  pub fn matches(&self, value: &str) -> Result<Matching, Never> {
    Ok(match *self {
      AttributeOperator::Exists => Matching::Yes,
      AttributeOperator::Matches(wanted) => Matching::from(value == wanted),
      AttributeOperator::Contains(wanted) => {
        Matching::from(value.split(' ').any(|word| word == wanted))
      }
      AttributeOperator::StartsWith(wanted) => match value.strip_prefix(wanted) {
        Some(tail) => Matching::from(tail.is_empty() || tail.starts_with('-')),
        None => Matching::No,
      },
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PseudoClass<'a> {
  FirstChild,
  Link,
  Visited,
  Hover,
  Active,
  Focus,
  Lang(&'a str),
}

pub trait Element: Clone + Ord {
  fn ancestor_elements(&self) -> impl Iterator<Item = Self>;

  fn prev_sibling_element(&self) -> Option<Self>;

  fn has_local_name(&self, name: &str) -> Matching;

  fn attribute_matches(&self, local_name: &str, operator: AttributeOperator<'_>) -> Matching;

  fn pseudo_class_matches(&self, class: PseudoClass<'_>) -> Matching;
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind<'a> {
  Type(&'a str),
  Universal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SubSelector<'a> {
  Attribute(&'a str, AttributeOperator<'a>),
  PseudoClass(PseudoClass<'a>),
}

#[derive(Debug, Clone)]
struct SimpleSelector<'a> {
  kind: Kind<'a>,
  subselectors: Vec<SubSelector<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Combinator {
  None,
  Descendant,
  Child,
  AdjacentSibling,
}

#[derive(Debug, Clone)]
struct Component<'a> {
  combinator: Combinator,
  selector: SimpleSelector<'a>,
}

#[derive(Debug, Clone)]
pub struct Selector<'a> {
  components: Vec<Component<'a>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Specificity {
  ids: u8,
  others: u8,
  types: u8,
}

impl<'a> Selector<'a> {
  pub(crate) fn specificity(&self) -> Result<Specificity, Never> {
    Ok(
      self
        .components
        .iter()
        .fold(Specificity::default(), |so_far, component| {
          let named = match component.selector.kind {
            Kind::Type(_name) => Specificity {
              types: so_far.types.saturating_add(1),
              ..so_far
            },
            Kind::Universal => so_far,
          };

          component
            .selector
            .subselectors
            .iter()
            .fold(named, |counted, sub| match sub {
              SubSelector::Attribute("id", _operator) => Specificity {
                ids: counted.ids.saturating_add(1),
                ..counted
              },
              SubSelector::Attribute(_, _) | SubSelector::PseudoClass(_) => Specificity {
                others: counted.others.saturating_add(1),
                ..counted
              },
            })
        }),
    )
  }

  pub fn matches<E: Element>(&self, element: &E) -> Result<Matching, Never> {
    let mut candidates = BTreeSet::from([element.clone()]);

    for component in self.components.iter().rev() {
      let mut accepted = candidates.iter().filter(|candidate| {
        let Ok(matched) = accepts(&component.selector, *candidate);

        matched == Matching::Yes
      });

      let Ok(next) = match component.combinator {
        Combinator::None => return Ok(Matching::from(accepted.next().is_some())),
        Combinator::Descendant => ancestors_of(accepted),
        Combinator::Child => Ok(
          accepted
            .filter_map(|candidate| candidate.ancestor_elements().next())
            .collect(),
        ),
        Combinator::AdjacentSibling => Ok(
          accepted
            .filter_map(|candidate| candidate.prev_sibling_element())
            .collect(),
        ),
      };

      candidates = next;
    }

    Ok(Matching::No)
  }
}

fn ancestors_of<'e, E: Element + 'e>(
  elements: impl Iterator<Item = &'e E>,
) -> Result<BTreeSet<E>, Never> {
  let mut ancestors = BTreeSet::new();

  for element in elements {
    'parents: for ancestor in element.ancestor_elements() {
      match ::core::primitive::bool::then_some(ancestors.insert(ancestor), ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => break 'parents,
      }
    }
  }

  Ok(ancestors)
}

fn accepts<E: Element>(simple: &SimpleSelector<'_>, element: &E) -> Result<Matching, Never> {
  let named = match simple.kind {
    Kind::Type(name) => element.has_local_name(name),
    Kind::Universal => Matching::Yes,
  };

  Ok(match named {
    Matching::No => Matching::No,
    Matching::Yes => Matching::from(simple.subselectors.iter().all(|sub| {
      let matched = match *sub {
        SubSelector::Attribute(name, operator) => element.attribute_matches(name, operator),
        SubSelector::PseudoClass(class) => element.pseudo_class_matches(class),
      };

      matched == Matching::Yes
    })),
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expecting {
  Selector,
  Continuation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
  Continue,
  Finished,
}

struct Parsing<'a> {
  components: Vec<Component<'a>>,
  combinator: Combinator,
  expecting: Expecting,
}

pub(crate) fn parse<'a>(stream: &mut Stream<'a>) -> Result<Selector<'a>, Fault> {
  let mut parsing = Parsing {
    components: Vec::new(),
    combinator: Combinator::None,
    expecting: Expecting::Selector,
  };
  let bound = stream.budget()?;

  for _step in 0..=bound {
    let flow = parsing.step(stream)?;

    match flow {
      Flow::Continue => {}
      Flow::Finished => return parsing.finish(),
    }
  }

  parsing.finish()
}

impl<'a> Parsing<'a> {
  fn finish(self) -> Result<Selector<'a>, Fault> {
    match self.expecting {
      Expecting::Selector => Err(Fault::SelectorMissing),
      Expecting::Continuation => match self.components.first().map(|first| first.combinator) {
        Some(Combinator::None) => Ok(Selector {
          components: self.components,
        }),
        Some(Combinator::Descendant | Combinator::Child | Combinator::AdjacentSibling) => {
          Err(Fault::UnexpectedCombinator)
        }
        None => Err(Fault::SelectorMissing),
      },
    }
  }

  fn push(&mut self, kind: Kind<'a>, subselectors: Vec<SubSelector<'a>>) -> Result<(), Never> {
    self.components.push(Component {
      combinator: self.combinator,
      selector: SimpleSelector { kind, subselectors },
    });
    self.combinator = Combinator::None;

    Ok(())
  }

  fn refine(&mut self, sub: SubSelector<'a>) -> Result<(), Never> {
    match self.combinator {
      Combinator::None => match self.components.last_mut() {
        Some(component) => {
          component.selector.subselectors.push(sub);
          Ok(())
        }
        None => self.push(Kind::Universal, vec![sub]),
      },
      Combinator::Descendant | Combinator::Child | Combinator::AdjacentSibling => {
        self.push(Kind::Universal, vec![sub])
      }
    }
  }

  fn step(&mut self, s: &mut Stream<'a>) -> Result<Flow, Fault> {
    let Ok(first) = s.first();

    match first {
      Some(byte) => self.dispatch(s, byte),
      None => Ok(Flow::Finished),
    }
  }

  fn dispatch(&mut self, s: &mut Stream<'a>, byte: u8) -> Result<Flow, Fault> {
    match byte {
      b'*' => self.universal(s),
      b'#' => self.attached(s, "id", AttributeOperator::Matches),
      b'.' => self.attached(s, "class", AttributeOperator::Contains),
      b'[' => self.bracketed(s),
      b':' => self.pseudo(s),
      b'>' => self.combine(s, Combinator::Child),
      b'+' => self.combine(s, Combinator::AdjacentSibling),
      b' ' | b'\t' | b'\n' | b'\r' | b'\x0C' => self.spaced(s),
      b'/' => self.slashed(s),
      b',' | b'{' => Ok(Flow::Finished),
      _other => self.named(s),
    }
  }

  fn universal(&mut self, s: &mut Stream<'a>) -> Result<Flow, Fault> {
    match self.expecting {
      Expecting::Continuation => Err(Fault::UnexpectedSelector),
      Expecting::Selector => {
        s.bump()?;
        self.expecting = Expecting::Continuation;
        self.push(Kind::Universal, Vec::new())?;

        Ok(Flow::Continue)
      }
    }
  }

  fn attached(
    &mut self,
    s: &mut Stream<'a>,
    attribute: &'static str,
    operator: fn(&'a str) -> AttributeOperator<'a>,
  ) -> Result<Flow, Fault> {
    self.expecting = Expecting::Continuation;
    s.bump()?;

    let name = s.consume_ident()?;

    self.refine(SubSelector::Attribute(attribute, operator(name)))?;

    Ok(Flow::Continue)
  }

  fn bracketed(&mut self, s: &mut Stream<'a>) -> Result<Flow, Fault> {
    self.expecting = Expecting::Continuation;
    s.bump()?;

    let name = s.consume_ident()?;
    let operator = attribute_operator(s)?;

    s.consume("]")?;
    self.refine(SubSelector::Attribute(name, operator))?;

    Ok(Flow::Continue)
  }

  fn pseudo(&mut self, s: &mut Stream<'a>) -> Result<Flow, Fault> {
    self.expecting = Expecting::Continuation;
    s.bump()?;

    let name = s.consume_ident()?;
    let class = match name {
      "lang" => {
        s.consume("(")?;

        let taken = s.take_until(')')?;
        let language = taken.trim();

        s.consume(")")?;

        match ::core::primitive::bool::then_some(language.is_empty(), ()) {
          ::core::option::Option::Some(()) => return Err(Fault::InvalidLanguagePseudoClass),
          ::core::option::Option::None => PseudoClass::Lang(language),
        }
      }
      "first-child" => PseudoClass::FirstChild,
      "link" => PseudoClass::Link,
      "visited" => PseudoClass::Visited,
      "hover" => PseudoClass::Hover,
      "active" => PseudoClass::Active,
      "focus" => PseudoClass::Focus,
      _unsupported => return Err(Fault::UnsupportedPseudoClass),
    };

    self.refine(SubSelector::PseudoClass(class))?;

    Ok(Flow::Continue)
  }

  fn combine(&mut self, s: &mut Stream<'a>, combinator: Combinator) -> Result<Flow, Fault> {
    match self.expecting {
      Expecting::Selector => Err(Fault::UnexpectedCombinator),
      Expecting::Continuation => {
        s.bump()?;
        self.expecting = Expecting::Selector;
        self.combinator = combinator;

        Ok(Flow::Continue)
      }
    }
  }

  fn spaced(&mut self, s: &mut Stream<'a>) -> Result<Flow, Fault> {
    s.skip_spaces()?;

    match self.expecting {
      Expecting::Selector => Ok(Flow::Continue),
      Expecting::Continuation => {
        let bound = s.budget()?;

        for _comment in 0..=bound {
          let Ok(first) = s.first();

          match first {
            Some(b'/') => {
              s.skip_comment()?;
              s.skip_spaces()?;
            }
            Some(_) | None => break,
          }
        }

        let Ok(first) = s.first();

        match first {
          None | Some(b'>' | b'+' | b',' | b'{') => {}
          Some(_other) => {
            self.expecting = Expecting::Selector;
            self.combinator = Combinator::Descendant;
          }
        }

        Ok(Flow::Continue)
      }
    }
  }

  fn slashed(&mut self, s: &mut Stream<'a>) -> Result<Flow, Fault> {
    let Ok(second) = s.second();

    match second {
      Some(b'*') => {
        s.skip_comment()?;

        Ok(Flow::Continue)
      }
      Some(_) | None => Ok(Flow::Finished),
    }
  }

  fn named(&mut self, s: &mut Stream<'a>) -> Result<Flow, Fault> {
    let name = s.consume_ident()?;

    match self.expecting {
      Expecting::Continuation => Err(Fault::UnexpectedSelector),
      Expecting::Selector => {
        self.expecting = Expecting::Continuation;
        self.push(Kind::Type(name), Vec::new())?;

        Ok(Flow::Continue)
      }
    }
  }
}

fn attribute_operator<'a>(s: &mut Stream<'a>) -> Result<AttributeOperator<'a>, Fault> {
  let Ok(first) = s.first();
  let byte = first.ok_or(Fault::UnexpectedEnd)?;

  match byte {
    b']' => Ok(AttributeOperator::Exists),
    b'=' => {
      s.bump()?;

      let wanted = s.consume_string()?;

      Ok(AttributeOperator::Matches(wanted))
    }
    b'~' => {
      s.bump()?;
      s.consume("=")?;

      let wanted = s.consume_string()?;

      Ok(AttributeOperator::Contains(wanted))
    }
    b'|' => {
      s.bump()?;
      s.consume("=")?;

      let wanted = s.consume_string()?;

      Ok(AttributeOperator::StartsWith(wanted))
    }
    _other => Err(Fault::InvalidAttributeSelector),
  }
}
