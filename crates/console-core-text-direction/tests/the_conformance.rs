//! Unicode's own conformance cases for the algorithm, a sample of them, held to the levels and the
//! order Unicode gives for each.
//!
//! `fixtures/BidiTest.txt` names a case by the classes of its characters and the paragraph
//! directions it is asked in, and a character standing for each class is put in its place;
//! `fixtures/BidiCharacterTest.txt` names the characters themselves, brackets among them, and the
//! level the paragraph has to come out at. Both are Unicode 16.0's files cut down to a stride
//! through them, and the head of each says which; the whole of both passed when this crate was
//! brought under the workspace's rules. A character rule X9 removes has no level and no place in
//! the order Unicode gives, so it is left out of what was seen before the two are compared, and a
//! character's level is the level of its first byte.
//!
//! The order is read twice, once from the runs `visual_runs` answers and once from
//! `reorder_visual` over the line's levels, and the two have to agree on every character, the
//! removed ones as well.

use std::num::ParseIntError;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_text_direction::{
  BidiClass, BidiInfo, Direction, Level, LevelFault, LevelRun, LineFault, bidi_class,
};

const BY_CLASS: &str = include_str!("fixtures/BidiTest.txt");
const BY_CHARACTER: &str = include_str!("fixtures/BidiCharacterTest.txt");

struct Asked {
  bit: u8,
  level: Option<Level>,
}

const ASKED: [Asked; 3] = [
  Asked {
    bit: 1,
    level: None,
  },
  Asked {
    bit: 2,
    level: Some(Level::LEFT_TO_RIGHT),
  },
  Asked {
    bit: 4,
    level: Some(Level::RIGHT_TO_LEFT),
  },
];

struct Sample {
  name: &'static str,
  character: char,
  class: BidiClass,
}

const SAMPLES: [Sample; 23] = [
  Sample {
    name: "AL",
    character: '\u{060B}',
    class: BidiClass::AL,
  },
  Sample {
    name: "AN",
    character: '\u{0605}',
    class: BidiClass::AN,
  },
  Sample {
    name: "B",
    character: '\u{000A}',
    class: BidiClass::B,
  },
  Sample {
    name: "BN",
    character: '\u{2060}',
    class: BidiClass::BN,
  },
  Sample {
    name: "CS",
    character: '\u{2044}',
    class: BidiClass::CS,
  },
  Sample {
    name: "EN",
    character: '\u{06F9}',
    class: BidiClass::EN,
  },
  Sample {
    name: "ES",
    character: '\u{208B}',
    class: BidiClass::ES,
  },
  Sample {
    name: "ET",
    character: '\u{20CF}',
    class: BidiClass::ET,
  },
  Sample {
    name: "FSI",
    character: '\u{2068}',
    class: BidiClass::FSI,
  },
  Sample {
    name: "L",
    character: '\u{02B8}',
    class: BidiClass::L,
  },
  Sample {
    name: "LRE",
    character: '\u{202A}',
    class: BidiClass::LRE,
  },
  Sample {
    name: "LRI",
    character: '\u{2066}',
    class: BidiClass::LRI,
  },
  Sample {
    name: "LRO",
    character: '\u{202D}',
    class: BidiClass::LRO,
  },
  Sample {
    name: "NSM",
    character: '\u{0300}',
    class: BidiClass::NSM,
  },
  Sample {
    name: "ON",
    character: '\u{03F6}',
    class: BidiClass::ON,
  },
  Sample {
    name: "PDF",
    character: '\u{202C}',
    class: BidiClass::PDF,
  },
  Sample {
    name: "PDI",
    character: '\u{2069}',
    class: BidiClass::PDI,
  },
  Sample {
    name: "R",
    character: '\u{0590}',
    class: BidiClass::R,
  },
  Sample {
    name: "RLE",
    character: '\u{202B}',
    class: BidiClass::RLE,
  },
  Sample {
    name: "RLI",
    character: '\u{2067}',
    class: BidiClass::RLI,
  },
  Sample {
    name: "RLO",
    character: '\u{202E}',
    class: BidiClass::RLO,
  },
  Sample {
    name: "S",
    character: '\u{001F}',
    class: BidiClass::S,
  },
  Sample {
    name: "WS",
    character: '\u{200A}',
    class: BidiClass::WS,
  },
];

enum Fault {
  Unreadable { line: u32, reading: Reading },
  NoCases,
  NoParagraph { line: u32 },
  Refused { line: u32, fault: LineFault },
  TwoOrders { line: u32 },
  Misclassified(Vec<&'static str>),
  Wrong(Vec<Mismatch>),
}

enum Reading {
  Fields,
  Number(ParseIntError),
  Class,
  Character,
  Direction,
  Level(LevelFault),
  NoLevelsYet,
}

impl std::fmt::Debug for Fault {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Fault::Unreadable { line, reading } => {
        write!(to, "line {line} cannot be read: {reading:?}")
      }
      Fault::NoCases => write!(to, "the fixture holds no cases"),
      Fault::NoParagraph { line } => write!(to, "line {line} came out with no paragraph"),
      Fault::Refused { line, fault } => write!(to, "line {line}: {fault}"),
      Fault::TwoOrders { line } => write!(
        to,
        "line {line}: visual_runs and reorder_visual disagree on the order"
      ),
      Fault::Misclassified(names) => write!(
        to,
        "the characters standing for {names:?} have other classes"
      ),
      Fault::Wrong(wrong) => {
        for one in wrong {
          writeln!(
            to,
            "line {} asked at {:?}: wanted {:?}, seen {:?} in a paragraph at {:?}",
            one.line, one.asked, one.wanted, one.seen, one.paragraph
          )?;
        }

        Ok(())
      }
    }
  }
}

impl std::fmt::Debug for Reading {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Reading::Fields => write!(
        to,
        "its fields are not the ones the head of the file describes"
      ),
      Reading::Number(fault) => write!(to, "a number in it is not one: {fault}"),
      Reading::Class => write!(to, "it names a class no character here stands for"),
      Reading::Character => write!(to, "it names a code point that is not a character"),
      Reading::Direction => write!(to, "its paragraph direction is not 0, 1 or 2"),
      Reading::Level(fault) => write!(to, "{fault}"),
      Reading::NoLevelsYet => write!(to, "it comes before any @Levels and @Reorder line"),
    }
  }
}

enum Line<'text> {
  Nothing,
  Levels(&'text str),
  Order(&'text str),
  Case(&'text str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Shown {
  levels: Vec<Option<u8>>,
  order: Vec<u32>,
}

#[derive(Clone, Copy)]
enum Paragraph {
  Said(Level),
  Unsaid,
}

struct Case {
  line: u32,
  text: String,
  asked: Option<Level>,
  paragraph: Paragraph,
  wanted: Shown,
}

struct Seen {
  shown: Shown,
  paragraph: Level,
}

struct Mismatch {
  line: u32,
  asked: Option<Level>,
  wanted: Shown,
  seen: Shown,
  paragraph: Level,
}

struct Character {
  number: u32,
  start: u32,
}

enum FixtureAgreement {
  Met,
  Different,
}

impl From<bool> for FixtureAgreement {
  fn from(agrees: bool) -> Self {
    match agrees.then_some(()) {
      Some(()) => Self::Met,
      None => Self::Different,
    }
  }
}

fn line_of(text: &str) -> Result<Line<'_>, Never> {
  let trimmed = text.trim();

  Ok(
    match (
      trimmed.chars().next(),
      trimmed.strip_prefix("@Levels:"),
      trimmed.strip_prefix("@Reorder:"),
    ) {
      (None | Some('#'), _, _) => Line::Nothing,
      (Some(_), Some(levels), _) => Line::Levels(levels),
      (Some(_), None, Some(order)) => Line::Order(order),
      (Some(_), None, None) => Line::Case(trimmed),
    },
  )
}

fn levels_in(said: &str) -> Result<Vec<Option<u8>>, Reading> {
  said
    .split_whitespace()
    .map(|token| match token {
      "x" => Ok(None),
      number => number.parse::<u8>().map(Some).map_err(Reading::Number),
    })
    .collect()
}

fn order_in(said: &str) -> Result<Vec<u32>, Reading> {
  said
    .split_whitespace()
    .map(|token| token.parse::<u32>().map_err(Reading::Number))
    .collect()
}

fn sample(name: &str) -> Result<char, Reading> {
  SAMPLES
    .iter()
    .find(|sample| sample.name == name)
    .map(|sample| sample.character)
    .ok_or(Reading::Class)
}

fn character(hexadecimal: &str) -> Result<char, Reading> {
  let code = u32::from_str_radix(hexadecimal, 16).map_err(Reading::Number)?;

  char::from_u32(code).ok_or(Reading::Character)
}

fn by_class() -> Result<Vec<Case>, Fault> {
  let mut levels: Option<Vec<Option<u8>>> = None;
  let mut order: Option<Vec<u32>> = None;
  let mut cases = Vec::new();

  for (line, text) in (1_u32..).zip(BY_CLASS.lines()) {
    let Ok(found) = line_of(text);

    match found {
      Line::Nothing => {}
      Line::Levels(said) => {
        let read = levels_in(said).map_err(|reading| Fault::Unreadable { line, reading })?;

        levels = Some(read);
      }
      Line::Order(said) => {
        let read = order_in(said).map_err(|reading| Fault::Unreadable { line, reading })?;

        order = Some(read);
      }
      Line::Case(said) => {
        let wanted = match (&levels, &order) {
          (Some(levels_2), Some(order_2)) => Shown {
            levels: levels_2.clone(),
            order: order_2.clone(),
          },
          (None, _) | (_, None) => {
            return Err(Fault::Unreadable {
              line,
              reading: Reading::NoLevelsYet,
            });
          }
        };
        let asked = by_class_at(line, said, &wanted)
          .map_err(|reading| Fault::Unreadable { line, reading })?;

        cases.extend(asked);
      }
    }
  }

  some_of(cases)
}

fn by_class_at(line: u32, said: &str, wanted: &Shown) -> Result<Vec<Case>, Reading> {
  let (classes, bits) = said.split_once(';').ok_or(Reading::Fields)?;
  let bitset = u8::from_str_radix(bits.trim(), 16).map_err(Reading::Number)?;
  let text = classes
    .split_whitespace()
    .map(sample)
    .collect::<Result<String, Reading>>()?;

  Ok(
    ASKED
      .iter()
      .filter(|asked| bitset & asked.bit != 0)
      .map(|asked| Case {
        line,
        text: text.clone(),
        asked: asked.level,
        paragraph: Paragraph::Unsaid,
        wanted: wanted.clone(),
      })
      .collect(),
  )
}

fn by_character() -> Result<Vec<Case>, Fault> {
  let mut cases = Vec::new();

  for (line, text) in (1_u32..).zip(BY_CHARACTER.lines()) {
    let Ok(found) = line_of(text);

    match found {
      Line::Nothing => {}
      Line::Levels(_) | Line::Order(_) => {
        return Err(Fault::Unreadable {
          line,
          reading: Reading::Fields,
        });
      }
      Line::Case(said) => {
        let case =
          by_character_at(line, said).map_err(|reading| Fault::Unreadable { line, reading })?;

        cases.push(case);
      }
    }
  }

  some_of(cases)
}

fn by_character_at(line: u32, said: &str) -> Result<Case, Reading> {
  let fields: Vec<&str> = said.split(';').collect();

  match fields.as_slice() {
    [characters, direction, paragraph, levels, order] => {
      let text = characters
        .split_whitespace()
        .map(character)
        .collect::<Result<String, Reading>>()?;
      let asked = match direction.trim() {
        "0" => Some(Level::LEFT_TO_RIGHT),
        "1" => Some(Level::RIGHT_TO_LEFT),
        "2" => None,
        _ => return Err(Reading::Direction),
      };
      let number = paragraph.trim().parse::<u8>().map_err(Reading::Number)?;
      let level = Level::new(number).map_err(Reading::Level)?;
      let levels_2 = levels_in(levels)?;
      let order_2 = order_in(order)?;

      Ok(Case {
        line,
        text,
        asked,
        paragraph: Paragraph::Said(level),
        wanted: Shown {
          levels: levels_2,
          order: order_2,
        },
      })
    }
    _ => Err(Reading::Fields),
  }
}

fn some_of(cases: Vec<Case>) -> Result<Vec<Case>, Fault> {
  match cases.as_slice() {
    [] => Err(Fault::NoCases),
    [_, ..] => Ok(cases),
  }
}

fn level_at(levels: &[Level], start: u32) -> Result<Option<Level>, Never> {
  let Ok(at) = index(start);

  Ok(levels.get(at).copied())
}

fn seen(case: &Case) -> Result<Seen, Fault> {
  let Ok(info) = BidiInfo::new(&case.text, case.asked);
  let Ok(paragraphs) = info.paragraphs();

  let paragraph = match paragraphs.first() {
    Some(paragraph) => paragraph,
    None => return Err(Fault::NoParagraph { line: case.line }),
  };
  let (levels, runs) = info
    .visual_runs(paragraph, paragraph.range.clone())
    .map_err(|fault| Fault::Refused {
      line: case.line,
      fault,
    })?;
  let characters: Vec<Character> = (0_u32..)
    .zip(case.text.char_indices())
    .map(|(number, (offset, _))| match fitted::<_, u32>(offset) {
      Ok(start) => Character { number, start },
    })
    .collect();
  let each: Vec<Level> = characters
    .iter()
    .filter_map(|character| match level_at(&levels, character.start) {
      Ok(level) => level,
    })
    .collect();
  let Ok(reordered) = BidiInfo::reorder_visual(&each);
  let Ok(walked) = walked(&characters, &levels, &runs);

  match FixtureAgreement::from(reordered == walked) {
    FixtureAgreement::Met => {}
    FixtureAgreement::Different => return Err(Fault::TwoOrders { line: case.line }),
  }

  let Ok(shown) = shown(&case.wanted, &each, &reordered);

  Ok(Seen {
    shown,
    paragraph: paragraph.level,
  })
}

fn walked(
  characters: &[Character],
  levels: &[Level],
  runs: &[LevelRun],
) -> Result<Vec<u32>, Never> {
  let mut order = Vec::new();

  for run in runs {
    let inside = characters
      .iter()
      .filter(|character| run.contains(&character.start))
      .map(|character| character.number);
    let Ok(first) = level_at(levels, run.start);

    match first.map(Level::direction) {
      Some(Ok(Direction::RightToLeft)) => order.extend(inside.rev()),
      Some(Ok(Direction::LeftToRight)) => order.extend(inside),
      None => {}
    }
  }

  Ok(order)
}

fn shown(wanted: &Shown, each: &[Level], reordered: &[u32]) -> Result<Shown, Never> {
  let levels = wanted
    .levels
    .iter()
    .zip(each)
    .map(|(said, level)| match (said, level.number()) {
      (Some(_), Ok(number)) => Some(number),
      (None, Ok(_)) => None,
    })
    .collect();
  let order = reordered
    .iter()
    .copied()
    .filter(|&number| match index(number) {
      Ok(at) => matches!(wanted.levels.get(at), Some(Some(_))),
    })
    .collect();

  Ok(Shown { levels, order })
}

fn mismatches(cases: &[Case]) -> Result<Vec<Mismatch>, Fault> {
  let mut wrong = Vec::new();

  for case in cases {
    let found = seen(case)?;
    let paragraph = match case.paragraph {
      Paragraph::Said(level) => FixtureAgreement::from(level == found.paragraph),
      Paragraph::Unsaid => FixtureAgreement::Met,
    };

    match (
      paragraph,
      FixtureAgreement::from(found.shown == case.wanted),
    ) {
      (FixtureAgreement::Met, FixtureAgreement::Met) => {}
      (FixtureAgreement::Different, _) | (_, FixtureAgreement::Different) => {
        wrong.push(Mismatch {
          line: case.line,
          asked: case.asked,
          wanted: case.wanted.clone(),
          seen: found.shown,
          paragraph: found.paragraph,
        })
      }
    }
  }

  Ok(wrong)
}

fn none_of(wrong: Vec<Mismatch>) -> Result<(), Fault> {
  match wrong.as_slice() {
    [] => Ok(()),
    [_, ..] => Err(Fault::Wrong(wrong)),
  }
}

#[test]
fn every_case_named_by_its_classes_comes_out_as_unicode_says() -> Result<(), Fault> {
  let cases = by_class()?;
  let wrong = mismatches(&cases)?;

  none_of(wrong)
}

#[test]
fn every_case_named_by_its_characters_comes_out_as_unicode_says() -> Result<(), Fault> {
  let cases = by_character()?;
  let wrong = mismatches(&cases)?;

  none_of(wrong)
}

#[test]
fn every_character_standing_for_a_class_has_that_class() -> Result<(), Fault> {
  let misread: Vec<&'static str> = SAMPLES
    .iter()
    .filter(|sample| match bidi_class(sample.character) {
      Ok(class) => class != sample.class,
    })
    .map(|sample| sample.name)
    .collect();

  match misread.as_slice() {
    [] => Ok(()),
    [_, ..] => Err(Fault::Misclassified(misread)),
  }
}
