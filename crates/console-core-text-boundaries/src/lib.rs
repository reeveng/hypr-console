//! Extended and legacy grapheme clusters under Unicode 17, UAX #29.
//!
//! The keyboard and text layout consume contiguous UTF-8 strings. This crate retains the
//! grapheme properties and conformance vectors from unicode-segmentation 1.13.3, with a
//! forward iterator that carries Indic, emoji and regional-indicator context as it walks.
//! Each character is inspected at most twice. Byte positions are held in sixty-four bits.

#![no_std]
#![forbid(unsafe_code)]

mod properties;

use console_core_never::Never;
use console_core_number_conversion::fitted;
use core::cmp::Ordering;

pub const UNICODE_VERSION: (u8, u8, u8) = (17, 0, 0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterKind {
  Extended,
  Legacy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Category {
  Any,
  CarriageReturn,
  Control,
  Extend,
  Pictograph,
  Consonant,
  Leading,
  LineFeed,
  Syllable,
  CompleteSyllable,
  Prepend,
  Regional,
  SpacingMark,
  Trailing,
  Vowel,
  Joiner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Indic {
  Other,
  Extend,
  Linker,
  Consonant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Conjunct {
  None,
  Consonant,
  Linked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Emoji {
  None,
  Pictograph,
  Joined,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Parity {
  Even,
  Odd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Boundary {
  Break,
  Continue,
}

struct Properties {
  category: Category,
  indic: Indic,
}

struct Context {
  previous: Category,
  conjunct: Conjunct,
  emoji: Emoji,
  regional: Parity,
}

fn category(character: char) -> Result<Category, Never> {
  match character {
    '\r' => return Ok(Category::CarriageReturn),
    '\n' => return Ok(Category::LineFeed),
    '\0'..='\u{1F}' | '\u{7F}' => return Ok(Category::Control),
    ' '..='~' => return Ok(Category::Any),
    '\u{80}'.. => {}
  }

  let range = properties::GRAPHEME
    .partition_point(|&(first, _, _)| first <= character)
    .checked_sub(1)
    .and_then(|last| properties::GRAPHEME.get(last));

  Ok(match range {
    Some(&(_, last, category)) => match character.cmp(&last) {
      Ordering::Less | Ordering::Equal => category,
      Ordering::Greater => Category::Any,
    },
    None => Category::Any,
  })
}

fn indic(character: char, category: Category) -> Result<Indic, Never> {
  match character {
    '\0'..='\u{7F}' => return Ok(Indic::Other),
    '\u{80}'.. => {}
  }

  match category {
    Category::Consonant => return Ok(Indic::Consonant),
    Category::Any
    | Category::CarriageReturn
    | Category::Control
    | Category::Extend
    | Category::Pictograph
    | Category::Leading
    | Category::LineFeed
    | Category::Syllable
    | Category::CompleteSyllable
    | Category::Prepend
    | Category::Regional
    | Category::SpacingMark
    | Category::Trailing
    | Category::Vowel
    | Category::Joiner => {}
  }

  match character {
    '\u{94D}' | '\u{9CD}' | '\u{ACD}' | '\u{B4D}' | '\u{C4D}' | '\u{D4D}' | '\u{1039}'
    | '\u{17D2}' | '\u{1A60}' | '\u{1B44}' | '\u{1BAB}' | '\u{A9C0}' | '\u{AAF6}'
    | '\u{10A3F}' | '\u{11133}' | '\u{113D0}' | '\u{1193E}' | '\u{11A47}' | '\u{11A99}'
    | '\u{11F42}' => return Ok(Indic::Linker),
    _ => {}
  }

  let range = properties::INDIC_EXTEND
    .partition_point(|&(first, _)| first <= character)
    .checked_sub(1)
    .and_then(|last| properties::INDIC_EXTEND.get(last));

  Ok(match range {
    Some(&(_, last)) => match character.cmp(&last) {
      Ordering::Less | Ordering::Equal => Indic::Extend,
      Ordering::Greater => Indic::Other,
    },
    None => Indic::Other,
  })
}

fn properties(character: char) -> Result<Properties, Never> {
  let Ok(category) = category(character);
  let Ok(indic) = indic(character, category);

  Ok(Properties { category, indic })
}

impl Context {
  fn new(first: &Properties) -> Result<Self, Never> {
    let mut context = Self {
      previous: Category::Any,
      conjunct: Conjunct::None,
      emoji: Emoji::None,
      regional: Parity::Even,
    };
    let Ok(()) = context.push(first);

    Ok(context)
  }

  fn push(&mut self, next: &Properties) -> Result<(), Never> {
    self.conjunct = match (next.indic, self.conjunct) {
      (Indic::Consonant, _) => Conjunct::Consonant,
      (Indic::Linker, Conjunct::Consonant | Conjunct::Linked) => Conjunct::Linked,
      (Indic::Extend, held) => held,
      (Indic::Linker, Conjunct::None) | (Indic::Other, _) => Conjunct::None,
    };
    self.emoji = match (next.category, self.emoji) {
      (Category::Pictograph, _) => Emoji::Pictograph,
      (Category::Extend, Emoji::Pictograph) => Emoji::Pictograph,
      (Category::Joiner, Emoji::Pictograph) => Emoji::Joined,
      (_, Emoji::None | Emoji::Joined | Emoji::Pictograph) => Emoji::None,
    };
    self.regional = match (next.category, self.regional) {
      (Category::Regional, Parity::Even) => Parity::Odd,
      (Category::Regional, Parity::Odd) | (_, Parity::Even | Parity::Odd) => Parity::Even,
    };
    self.previous = next.category;

    Ok(())
  }

  fn boundary(&self, next: &Properties, kind: ClusterKind) -> Result<Boundary, Never> {
    use Category::*;

    Ok(match (self.previous, next.category) {
      (CarriageReturn, LineFeed) => Boundary::Continue,
      (Control | CarriageReturn | LineFeed, _) | (_, Control | CarriageReturn | LineFeed) => {
        Boundary::Break
      }
      (Leading, Leading | Vowel | Syllable | CompleteSyllable)
      | (Syllable | Vowel, Vowel | Trailing)
      | (CompleteSyllable | Trailing, Trailing)
      | (_, Extend | Joiner) => Boundary::Continue,
      (_, SpacingMark) | (Prepend, _) => match kind {
        ClusterKind::Extended => Boundary::Continue,
        ClusterKind::Legacy => Boundary::Break,
      },
      (_, Consonant) => match (kind, self.conjunct) {
        (ClusterKind::Extended, Conjunct::Linked) => Boundary::Continue,
        (ClusterKind::Legacy, _)
        | (ClusterKind::Extended, Conjunct::None | Conjunct::Consonant) => Boundary::Break,
      },
      (Joiner, Pictograph) => match self.emoji {
        Emoji::Joined => Boundary::Continue,
        Emoji::None | Emoji::Pictograph => Boundary::Break,
      },
      (Regional, Regional) => match self.regional {
        Parity::Odd => Boundary::Continue,
        Parity::Even => Boundary::Break,
      },
      (_, _) => Boundary::Break,
    })
  }
}

#[derive(Clone, Debug)]
pub struct Graphemes<'text> {
  remaining: &'text str,
  kind: ClusterKind,
}

impl<'text> Graphemes<'text> {
  pub fn as_str(&self) -> Result<&'text str, Never> {
    Ok(self.remaining)
  }
}

impl<'text> Iterator for Graphemes<'text> {
  type Item = &'text str;

  fn next(&mut self) -> Option<Self::Item> {
    let text = self.remaining;
    let mut characters = text.char_indices();
    let (_, first) = characters.next()?;
    let Ok(first_properties) = properties(first);
    let Ok(mut context) = Context::new(&first_properties);

    for (at, character) in characters {
      let Ok(next) = properties(character);
      let Ok(boundary) = context.boundary(&next, self.kind);

      match boundary {
        Boundary::Break => {
          let (cluster, remaining) = text.split_at_checked(at)?;

          self.remaining = remaining;

          return Some(cluster);
        }
        Boundary::Continue => {
          let Ok(()) = context.push(&next);
        }
      }
    }

    self.remaining = "";

    Some(text)
  }
}

impl core::iter::FusedIterator for Graphemes<'_> {}

#[derive(Clone, Debug)]
pub struct GraphemeIndices<'text> {
  clusters: Graphemes<'text>,
  position: u64,
}

impl<'text> Iterator for GraphemeIndices<'text> {
  type Item = (u64, &'text str);

  fn next(&mut self) -> Option<Self::Item> {
    let cluster = self.clusters.next()?;
    let start = self.position;
    let Ok(bytes) = fitted::<_, u64>(cluster.len());

    self.position = self.position.saturating_add(bytes);

    Some((start, cluster))
  }
}

impl core::iter::FusedIterator for GraphemeIndices<'_> {}

pub trait UnicodeSegmentation {
  fn graphemes(&self, kind: ClusterKind) -> Result<Graphemes<'_>, Never>;

  fn grapheme_indices(&self, kind: ClusterKind) -> Result<GraphemeIndices<'_>, Never>;
}

impl UnicodeSegmentation for str {
  fn graphemes(&self, kind: ClusterKind) -> Result<Graphemes<'_>, Never> {
    Ok(Graphemes {
      remaining: self,
      kind,
    })
  }

  fn grapheme_indices(&self, kind: ClusterKind) -> Result<GraphemeIndices<'_>, Never> {
    let Ok(clusters) = self.graphemes(kind);

    Ok(GraphemeIndices {
      clusters,
      position: 0,
    })
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WordPart {
  Space,
  Word,
  Symbol,
}

fn word_part(cluster: &str) -> Result<WordPart, Never> {
  Ok(
    match (
      cluster.chars().all(char::is_whitespace).then_some(()),
      cluster
        .chars()
        .any(|part| part.is_alphanumeric() || part == '_')
        .then_some(()),
    ) {
      (Some(()), _) => WordPart::Space,
      (None, Some(())) => WordPart::Word,
      (None, None) => WordPart::Symbol,
    },
  )
}

#[doc = "Byte offset of the last word or symbol run, skipping trailing whitespace."]
pub fn previous_word_start(text: &str) -> Result<u64, Never> {
  let Ok(graphemes) = text.grapheme_indices(ClusterKind::Extended);
  let mut previous = WordPart::Space;
  let mut start = 0;

  for (position, cluster) in graphemes {
    let Ok(part) = word_part(cluster);

    match (part, (part != previous).then_some(())) {
      (WordPart::Word | WordPart::Symbol, Some(())) => start = position,
      (WordPart::Space, _) | (_, None) => {}
    }

    previous = part;
  }

  Ok(start)
}

pub fn next_word_end(text: &str) -> Result<u64, Never> {
  let Ok(graphemes) = text.grapheme_indices(ClusterKind::Extended);
  let mut previous = WordPart::Space;

  for (position, cluster) in graphemes {
    let Ok(part) = word_part(cluster);

    match (previous, part) {
      (WordPart::Word, WordPart::Space | WordPart::Symbol)
      | (WordPart::Symbol, WordPart::Space | WordPart::Word) => return Ok(position),
      _ => previous = part,
    }
  }

  console_core_number_conversion::fixed_index(text.len())
}

pub fn word_at(text: &str, caret: u64) -> Result<core::ops::Range<u64>, Never> {
  let Ok(graphemes) = text.grapheme_indices(ClusterKind::Extended);
  let mut previous = WordPart::Space;
  let mut start = 0;

  for (position, cluster) in graphemes {
    let Ok(part) = word_part(cluster);

    match (part != previous, position > caret) {
      (true, true) => return Ok(start..position),
      (true, false) => start = position,
      (false, _) => {}
    }

    previous = part;
  }

  let Ok(end) = console_core_number_conversion::fixed_index(text.len());
  Ok(start..end)
}
