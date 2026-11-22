//! A cmap written out as the PostScript it is, read off its objects.
//!
//! The reader runs nothing: it looks at each name and, where the name begins a
//! section, reads the entries up to the name that ends it. The metadata is
//! read leniently, because generators write a name where a string belongs and
//! write the section a cmap is not asked for; whatever cannot be read as what
//! it should be is left out, and the cmap is still made. An entry that cannot
//! be read as one is not left out, because the codes after it would then be
//! read as the wrong thing.

use console_core_never::Never;
use console_core_postscript_tokens::{Name, NameKind, Object, Remaining, Scanner};
use core::cmp::Ordering;

use crate::layer::{Layer, Span};
use crate::{CharacterCollection, CidFamily, Layered, Metadata, ParseError, WritingMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marker {
  End,
  Entry,
}

impl From<bool> for Marker {
  fn from(ends_section: bool) -> Self {
    match bool::then_some(ends_section, ()) {
      Some(()) => Self::End,
      None => Self::Entry,
    }
  }
}

#[derive(Debug, Clone, Copy)]
enum Table {
  Cid,
  Notdef,
}

#[derive(Debug, Clone, Copy)]
enum Entries {
  Characters,
  Ranges,
}

#[derive(Debug, Default)]
struct Facts {
  registry: Option<Vec<u8>>,
  ordering: Option<Vec<u8>>,
  supplement: Option<i32>,
  writing_mode: Option<WritingMode>,
  last_name: Option<Vec<u8>>,
  base: Option<Vec<u8>>,
}

struct Reading<'a> {
  scanner: Scanner<'a>,
  bound: &'a [u8],
  buffer: Vec<u8>,
  layer: Layer,
}

fn present<T, E>(result: Result<T, E>) -> Result<Option<T>, Never> {
  Ok(match result {
    Ok(found) => Some(found),
    Err(_unreadable) => None,
  })
}

fn marker(object: &Object<'_>, expected: &str) -> Result<Marker, Never> {
  Ok(match object {
    Object::Name(name) => match (name.kind(), name.as_str()) {
      (Ok(NameKind::Executable), Ok(word)) => Marker::from(word == expected),
      (Ok(NameKind::Executable), Err(_not_text)) => Marker::Entry,
      (Ok(NameKind::Literal), _word) => Marker::Entry,
    },
    Object::Number(_) | Object::String(_) | Object::Array(_) => Marker::Entry,
  })
}

fn string_or_name(scanner: &mut Scanner<'_>) -> Result<Option<Vec<u8>>, Never> {
  match scanner.parse_object() {
    Ok(Object::String(string)) => present(string.decode()),
    Ok(Object::Name(name)) => present(name.decode()),
    Ok(Object::Number(_) | Object::Array(_)) => Ok(None),
    Err(_unreadable) => Ok(None),
  }
}

fn supplement(scanner: &mut Scanner<'_>) -> Result<Option<i32>, Never> {
  Ok(match scanner.parse_number() {
    Ok(number) => {
      let Ok(supplement) = number.as_i32();

      Some(supplement)
    }
    Err(_unreadable) => None,
  })
}

fn writing_mode(scanner: &mut Scanner<'_>) -> Result<Option<WritingMode>, Never> {
  Ok(match scanner.parse_number() {
    Ok(number) => {
      let Ok(mode) = number.as_i32();

      match mode {
        0 => Some(WritingMode::Horizontal),
        1 => Some(WritingMode::Vertical),
        _other => None,
      }
    }
    Err(_unreadable) => None,
  })
}

fn code_of(bytes: &[u8]) -> Result<u32, ParseError> {
  match bytes {
    [a] => Ok(u32::from_be_bytes([0, 0, 0, *a])),
    [a, b] => Ok(u32::from_be_bytes([0, 0, *a, *b])),
    [a, b, c] => Ok(u32::from_be_bytes([0, *a, *b, *c])),
    [a, b, c, d] => Ok(u32::from_be_bytes([*a, *b, *c, *d])),
    _empty_or_wider => Err(ParseError::BadCode),
  }
}

fn units_of(bytes: &[u8]) -> Result<Vec<u16>, ParseError> {
  let (lead, body) = match bytes.split_first() {
    None => return Err(ParseError::BadCode),
    Some((first, rest)) => match bytes.as_chunks::<2>().1 {
      [] => (None, bytes),
      [_, ..] => (Some(u16::from(*first)), rest),
    },
  };
  let (pairs, _remainder) = body.as_chunks::<2>();

  Ok(
    lead
      .into_iter()
      .chain(pairs.iter().map(|pair| u16::from_be_bytes(*pair)))
      .collect(),
  )
}

impl Reading<'_> {
  fn code(&mut self, object: &Object<'_>) -> Result<u32, ParseError> {
    match object {
      Object::String(string) => {
        string.decode_into(&mut self.buffer)?;

        code_of(&self.buffer)
      }
      Object::Number(_) | Object::Name(_) | Object::Array(_) => Err(ParseError::BadCode),
    }
  }

  fn next_code(&mut self) -> Result<u32, ParseError> {
    let object = self.scanner.parse_object()?;

    self.code(&object)
  }

  fn width(&self) -> Result<u8, ParseError> {
    u8::try_from(self.buffer.len()).map_err(|_wide| ParseError::BadCode)
  }

  fn cid(&mut self) -> Result<u32, ParseError> {
    let number = self.scanner.parse_number()?;
    let Ok(value) = number.as_i32();

    u32::try_from(value).map_err(|_negative| ParseError::BadNumber)
  }

  fn string_units(&mut self) -> Result<Vec<u16>, ParseError> {
    let string = self.scanner.parse_string()?;

    string.decode_into(&mut self.buffer)?;

    units_of(&self.buffer)
  }

  fn codespace(&mut self) -> Result<(), ParseError> {
    for _entry in self.bound {
      let object = self.scanner.parse_object()?;
      let Ok(found) = marker(&object, "endcodespacerange");

      match found {
        Marker::End => return Ok(()),
        Marker::Entry => {}
      }

      self.code(&object)?;

      let width = self.width()?;

      self.next_code()?;

      let after = self.width()?;

      match after.cmp(&width) {
        Ordering::Equal => {}
        Ordering::Less | Ordering::Greater => return Err(ParseError::BadCode),
      }
    }

    Err(ParseError::Truncated)
  }

  fn add(&mut self, table: Table, width: u8, span: Span<u32>) -> Result<(), ParseError> {
    let Ok(()) = match table {
      Table::Cid => self.layer.add_cid(width, span),
      Table::Notdef => self.layer.add_notdef(width, span),
    };

    Ok(())
  }

  fn entries(&mut self, table: Table, written: Entries, ends: &str) -> Result<(), ParseError> {
    for _entry in self.bound {
      let object = self.scanner.parse_object()?;
      let Ok(found) = marker(&object, ends);

      match found {
        Marker::End => return Ok(()),
        Marker::Entry => {}
      }

      let start = self.code(&object)?;
      let width = self.width()?;

      let end = match written {
        Entries::Ranges => self.next_code(),
        Entries::Characters => Ok(start),
      }?;

      let value = self.cid()?;

      self.add(table, width, Span { start, end, value })?;
    }

    Err(ParseError::Truncated)
  }

  fn bf_characters(&mut self) -> Result<(), ParseError> {
    for _entry in self.bound {
      let object = self.scanner.parse_object()?;
      let Ok(found) = marker(&object, "endbfchar");

      match found {
        Marker::End => return Ok(()),
        Marker::Entry => {}
      }

      let code = self.code(&object)?;
      let value = self.string_units()?;
      let Ok(()) = self.layer.add_bf(Span {
        start: code,
        end: code,
        value,
      });
    }

    Err(ParseError::Truncated)
  }

  fn bf_ranges(&mut self) -> Result<(), ParseError> {
    for _entry in self.bound {
      let object = self.scanner.parse_object()?;
      let Ok(found) = marker(&object, "endbfrange");

      match found {
        Marker::End => return Ok(()),
        Marker::Entry => {}
      }

      let start = self.code(&object)?;
      let end = self.next_code()?;
      let next = self.scanner.parse_object()?;

      match next {
        Object::String(string) => {
          string.decode_into(&mut self.buffer)?;

          let value = units_of(&self.buffer)?;
          let Ok(()) = self.layer.add_bf(Span { start, end, value });
        }
        Object::Array(array) => {
          let Ok(mut inner) = array.objects();

          for code in start..=end {
            let string = inner.parse_string()?;

            string.decode_into(&mut self.buffer)?;

            let value = units_of(&self.buffer)?;
            let Ok(()) = self.layer.add_bf(Span {
              start: code,
              end: code,
              value,
            });
          }
        }
        Object::Number(_) | Object::Name(_) => return Err(ParseError::BadCode),
      }
    }

    Err(ParseError::Truncated)
  }

  fn literal(&mut self, name: &Name<'_>, facts: &mut Facts) -> Result<(), ParseError> {
    match name.as_str() {
      Ok("Registry") => {
        let Ok(found) = string_or_name(&mut self.scanner);

        facts.registry = found;
      }
      Ok("Ordering") => {
        let Ok(found) = string_or_name(&mut self.scanner);

        facts.ordering = found;
      }
      Ok("Supplement") => {
        let Ok(found) = supplement(&mut self.scanner);

        facts.supplement = found;
      }
      Ok("CMapName") => {
        let Ok(_unused_name) = present(self.scanner.parse_name());
      }
      Ok("WMode") => {
        let Ok(found) = writing_mode(&mut self.scanner);

        facts.writing_mode = found;
      }
      Ok(_other) => {
        let Ok(found) = present(name.decode());

        facts.last_name = found;
      }
      Err(_not_text) => {
        let Ok(found) = present(name.decode());

        facts.last_name = found;
      }
    }

    Ok(())
  }

  fn executable(&mut self, name: &Name<'_>, facts: &mut Facts) -> Result<(), ParseError> {
    match name.as_str() {
      Ok("begincodespacerange") => self.codespace(),
      Ok("begincidrange") => self.entries(Table::Cid, Entries::Ranges, "endcidrange"),
      Ok("begincidchar") => self.entries(Table::Cid, Entries::Characters, "endcidchar"),
      Ok("beginnotdefrange") => self.entries(Table::Notdef, Entries::Ranges, "endnotdefrange"),
      Ok("beginnotdefchar") => self.entries(Table::Notdef, Entries::Characters, "endnotdefchar"),
      Ok("beginbfchar") => self.bf_characters(),
      Ok("beginbfrange") => self.bf_ranges(),
      Ok("usecmap") => match &facts.last_name {
        Some(last) => {
          facts.base = Some(last.clone());

          Ok(())
        }
        None => Err(ParseError::Unresolved),
      },
      Ok(_other) => Ok(()),
      Err(_not_text) => Ok(()),
    }
  }

  fn visit(&mut self, object: Object<'_>, facts: &mut Facts) -> Result<(), ParseError> {
    let name = match object {
      Object::Name(name) => name,
      Object::Number(_) | Object::String(_) | Object::Array(_) => return Ok(()),
    };
    let Ok(kind) = name.kind();

    match kind {
      NameKind::Literal => self.literal(&name, facts),
      NameKind::Executable => self.executable(&name, facts),
    }
  }
}

pub(crate) fn read(data: &[u8]) -> Result<Layered, ParseError> {
  let Ok(scanner) = Scanner::new(data);
  let mut reading = Reading {
    scanner,
    bound: data,
    buffer: Vec::new(),
    layer: Layer::default(),
  };
  let mut facts = Facts::default();

  for _object in data {
    let Ok(remaining) = reading.scanner.remaining();

    match remaining {
      Remaining::Nothing => break,
      Remaining::Objects => {}
    }

    let object = reading.scanner.parse_object()?;

    reading.visit(object, &mut facts)?;
  }

  let Ok(()) = reading.layer.sort();
  let character_collection = match (facts.registry, facts.ordering, facts.supplement) {
    (Some(registry), Some(ordering), Some(supplement)) => {
      let Ok(family) = CidFamily::from_registry_ordering(&registry, &ordering);

      Some(CharacterCollection { family, supplement })
    }
    (None, _, _) | (Some(_), None, _) | (Some(_), Some(_), None) => None,
  };

  Ok(Layered {
    metadata: Metadata {
      character_collection,
      writing_mode: facts.writing_mode,
    },
    layer: reading.layer,
    base: facts.base,
  })
}
