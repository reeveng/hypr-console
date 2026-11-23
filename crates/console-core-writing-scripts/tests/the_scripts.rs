//! Every character's script and every script's short name, held to the Character Database's own
//! files for Unicode 17.0.
//!
//! `fixtures/Scripts.txt` is the whole of the file the table was made from, so every code point is
//! asked, and one the file gives no script has to come back `Unknown`, as its head says. Its names
//! are the variants' names, so a variant is found by how it prints. `fixtures/PropertyValueAliases.txt`
//! is that file's Script section and nothing else; every variant has to be named there with the
//! short name it answers, and the one script it names that no character has, Katakana_Or_Hiragana,
//! is the one with no variant.

use std::collections::HashSet;
use std::num::ParseIntError;

use console_core_never::Never;
use console_core_number_conversion::indices_inclusive;
use console_core_writing_scripts::{Script, UnicodeScript};

const SCRIPTS: &str = include_str!("fixtures/Scripts.txt");
const ALIASES: &str = include_str!("fixtures/PropertyValueAliases.txt");
const LAST_CODE_POINT: u32 = 0x10FFFF;

enum Error {
  Read { line: u32, reading: Reading },
  Misread(Vec<Misread>),
  Misnamed(Vec<Misnamed>),
  Unnamed(Vec<Script>),
  Unclaimed(Vec<&'static str>),
}

enum FixtureAgreement {
  Matches,
  Differs,
}

impl From<bool> for FixtureAgreement {
  fn from(equal: bool) -> Self {
    match bool::then_some(equal, ()) {
      Some(()) => Self::Matches,
      None => Self::Differs,
    }
  }
}

enum Reading {
  Fields,
  Number(ParseIntError),
  NoSuchScript,
}

struct Misread {
  code: u32,
  wanted: Script,
  seen: Script,
}

struct Misnamed {
  script: Script,
  wanted: &'static str,
  seen: &'static str,
}

impl std::fmt::Debug for Error {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Error::Read { line, reading } => {
        write!(to, "line {line} cannot be read: {reading:?}")
      }
      Error::Misread(wrong) => {
        for one in wrong {
          writeln!(
            to,
            "U+{:04X} wanted {:?}, seen {:?}",
            one.code, one.wanted, one.seen
          )?;
        }

        Ok(())
      }
      Error::Misnamed(wrong) => {
        for one in wrong {
          writeln!(
            to,
            "{:?} wanted {}, seen {}",
            one.script, one.wanted, one.seen
          )?;
        }

        Ok(())
      }
      Error::Unnamed(scripts) => write!(to, "no alias names {scripts:?}"),
      Error::Unclaimed(names) => write!(to, "no script answers to {names:?}"),
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
      Reading::Number(fault) => write!(to, "a code point in it is not one: {fault}"),
      Reading::NoSuchScript => write!(to, "it names a script no variant prints as"),
    }
  }
}

fn named(name: &str) -> Result<Option<Script>, Never> {
  Ok(
    Script::VARIANTS
      .iter()
      .copied()
      .find(|script| format!("{script:?}") == name),
  )
}

fn said(text: &str) -> Result<Option<&str>, Never> {
  let body = match text.split_once('#') {
    Some((body, _comment)) => body,
    None => text,
  };
  let trimmed = body.trim();

  Ok(match trimmed {
    "" => None,
    content => Some(content),
  })
}

fn code_points(span: &str) -> Result<std::ops::RangeInclusive<u32>, Reading> {
  let (first, last) = match span.split_once("..") {
    Some(both) => both,
    None => (span, span),
  };
  let start = u32::from_str_radix(first, 16).map_err(Reading::Number)?;
  let end = u32::from_str_radix(last, 16).map_err(Reading::Number)?;

  Ok(start..=end)
}

fn wanted() -> Result<Vec<Script>, Error> {
  let mut each = vec![Script::Unknown; 0x110000];

  for (line, text) in (1_u32..).zip(SCRIPTS.lines()) {
    let Ok(found) = said(text);

    match found {
      Some(body) => {
        let (span, name) = body.split_once(';').ok_or(Error::Read {
          line,
          reading: Reading::Fields,
        })?;
        let codes =
          code_points(span.trim()).map_err(|reading| Error::Read { line, reading })?;
        let Ok(script) = named(name.trim());
        let claimed = script.ok_or(Error::Read {
          line,
          reading: Reading::NoSuchScript,
        })?;
        let part = match indices_inclusive(codes) {
          Ok(span_2) => each.get_mut(span_2),
        };

        match part {
          Some(part_2) => part_2.fill(claimed),
          None => {
            return Err(Error::Read {
              line,
              reading: Reading::Fields,
            });
          }
        }
      }
      None => {}
    }
  }

  Ok(each)
}

#[test]
fn every_character_has_the_script_scripts_txt_gives_it() -> Result<(), Error> {
  let each = wanted()?;
  let mut wrong = Vec::new();

  for (code, &script) in (0_u32..=LAST_CODE_POINT).zip(&each) {
    match char::from_u32(code) {
      Some(character) => {
        let Ok(seen) = character.script();

        match FixtureAgreement::from(seen == script) {
          FixtureAgreement::Matches => {}
          FixtureAgreement::Differs => wrong.push(Misread {
            code,
            wanted: script,
            seen,
          }),
        }
      }
      None => {}
    }
  }

  match wrong.as_slice() {
    [] => Ok(()),
    [_, ..] => Err(Error::Misread(wrong)),
  }
}

#[test]
fn every_script_has_the_short_name_its_alias_gives_it() -> Result<(), Error> {
  let mut wrong = Vec::new();
  let mut seen = HashSet::new();
  let mut unclaimed = Vec::new();

  for (line, text) in (1_u32..).zip(ALIASES.lines()) {
    let Ok(found) = said(text);
    let fields: Vec<&'static str> = match found {
      Some(body) => body.split(';').map(str::trim).collect(),
      None => Vec::new(),
    };

    let (short, long) = match fields.as_slice() {
      [] => continue,
      ["sc", short, long, ..] => (*short, *long),
      _ => {
        return Err(Error::Read {
          line,
          reading: Reading::Fields,
        });
      }
    };
    let script = match named(long) {
      Ok(Some(script)) => script,
      Ok(None) => {
        unclaimed.push(long);

        continue;
      }
    };
    let Ok(answered) = script.short_name();

    seen.insert(script);

    match FixtureAgreement::from(answered == short) {
      FixtureAgreement::Matches => {}
      FixtureAgreement::Differs => wrong.push(Misnamed {
        script,
        wanted: short,
        seen: answered,
      }),
    }
  }

  let unnamed: Vec<Script> = Script::VARIANTS
    .iter()
    .copied()
    .filter(|script| !seen.contains(script))
    .collect();

  match (wrong.as_slice(), unnamed.as_slice(), unclaimed.as_slice()) {
    ([], [], ["Katakana_Or_Hiragana"]) => Ok(()),
    ([_, ..], _, _) => Err(Error::Misnamed(wrong)),
    ([], [_, ..], _) => Err(Error::Unnamed(unnamed)),
    ([], [], _) => Err(Error::Unclaimed(unclaimed)),
  }
}

#[test]
fn a_few_characters_a_reader_knows_the_scripts_of() {
  let scripts: Vec<Result<Script, Never>> = [
    'a', 'א', 'ب', 'あ', 'ア', '中', '한', '1', '\u{300}', '\u{378}',
  ]
  .iter()
  .map(UnicodeScript::script)
  .collect();

  assert_eq!(
    scripts,
    vec![
      Ok(Script::Latin),
      Ok(Script::Hebrew),
      Ok(Script::Arabic),
      Ok(Script::Hiragana),
      Ok(Script::Katakana),
      Ok(Script::Han),
      Ok(Script::Hangul),
      Ok(Script::Common),
      Ok(Script::Inherited),
      Ok(Script::Unknown),
    ]
  );
  assert_eq!(Script::Latin.short_name(), Ok("Latn"));
  assert_eq!(Script::Unknown.short_name(), Ok("Zzzz"));
}
