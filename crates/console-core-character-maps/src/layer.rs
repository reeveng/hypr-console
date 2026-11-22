//! What one cmap file says, and the lookups a code is asked of it.
//!
//! A file's ranges are kept by the width of the codes they cover and sorted
//! by where they start, so a code is found by a binary search over the ranges
//! of its own width. A code a file does not decide is left for the file it
//! names with `usecmap`, which is a [`Lookup::Unsettled`] rather than a
//! missing answer: a file that decides a code and has nothing to say about it
//! is not asked again below.

use std::cmp::Ordering;

use console_core_never::Never;
use console_core_number_conversion::index;

use crate::{BfString, Cid};

#[derive(Debug, Clone)]
pub(crate) struct Span<T> {
  pub(crate) start: u32,
  pub(crate) end: u32,
  pub(crate) value: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lookup<T> {
  Settled(Option<T>),
  Unsettled,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Layer {
  cid: [Vec<Span<Cid>>; 4],
  notdef: [Vec<Span<Cid>>; 4],
  bf: Vec<Span<Vec<u16>>>,
}

fn bucket(buckets: &[Vec<Span<Cid>>; 4], width: u8) -> Result<Option<&[Span<Cid>]>, Never> {
  let Ok(at) = index(width.saturating_sub(1));

  Ok(match width {
    0 => None,
    _wide => buckets.get(at).map(Vec::as_slice),
  })
}

fn spans_at(
  buckets: &[Vec<Span<Cid>>; 4],
  width: u8,
  code: u32,
) -> Result<Option<&Span<Cid>>, Never> {
  let Ok(found) = bucket(buckets, width);

  match found {
    Some(spans) => find(spans, code),
    None => Ok(None),
  }
}

fn bucket_mut(
  buckets: &mut [Vec<Span<Cid>>; 4],
  width: u8,
) -> Result<Option<&mut Vec<Span<Cid>>>, Never> {
  let Ok(at) = index(width.saturating_sub(1));

  Ok(match width {
    0 => None,
    _wide => buckets.get_mut(at),
  })
}

fn find<T>(spans: &[Span<T>], code: u32) -> Result<Option<&Span<T>>, Never> {
  Ok(
    match spans.binary_search_by(|span| match (code.cmp(&span.start), code.cmp(&span.end)) {
      (Ordering::Less, _past_the_start) => Ordering::Greater,
      (Ordering::Equal | Ordering::Greater, Ordering::Greater) => Ordering::Less,
      (Ordering::Equal | Ordering::Greater, Ordering::Less | Ordering::Equal) => Ordering::Equal,
    }) {
      Ok(at) => spans.get(at),
      Err(_missing) => None,
    },
  )
}

fn decoded(units: &[u16]) -> Result<Option<BfString>, Never> {
  let mut chars = char::decode_utf16(units.iter().copied());
  let first = match chars.next() {
    Some(Ok(first)) => first,
    Some(Err(_unpaired)) => return Ok(None),
    None => return Ok(None),
  };

  Ok(match chars.next() {
    None => Some(BfString::Char(first)),
    Some(_more) => match String::from_utf16(units) {
      Ok(whole) => Some(BfString::String(whole)),
      Err(_unpaired) => None,
    },
  })
}

fn string_at(span: &Span<Vec<u16>>, code: u32) -> Result<Option<BfString>, Never> {
  let offset = match code.checked_sub(span.start).map(u16::try_from) {
    Some(Ok(offset)) => offset,
    Some(Err(_too_far)) => return Ok(None),
    None => return Ok(None),
  };

  match offset {
    0 => decoded(&span.value),
    _past_the_start => {
      let mut units = span.value.clone();

      match units.pop().and_then(|last| last.checked_add(offset)) {
        Some(last) => {
          units.push(last);

          decoded(&units)
        }
        None => Ok(None),
      }
    }
  }
}

impl Layer {
  pub(crate) fn add_cid(&mut self, width: u8, span: Span<Cid>) -> Result<(), Never> {
    let Ok(found) = bucket_mut(&mut self.cid, width);

    match found {
      Some(spans) => spans.push(span),
      None => {}
    }

    Ok(())
  }

  pub(crate) fn add_notdef(&mut self, width: u8, span: Span<Cid>) -> Result<(), Never> {
    let Ok(found) = bucket_mut(&mut self.notdef, width);

    match found {
      Some(spans) => spans.push(span),
      None => {}
    }

    Ok(())
  }

  pub(crate) fn add_bf(&mut self, span: Span<Vec<u16>>) -> Result<(), Never> {
    self.bf.push(span);

    Ok(())
  }

  pub(crate) fn sort(&mut self) -> Result<(), Never> {
    for spans in self.cid.iter_mut().chain(self.notdef.iter_mut()) {
      spans.sort_by_key(|span| span.start);
    }

    self.bf.sort_by_key(|span| span.start);

    Ok(())
  }

  pub(crate) fn cid_code(&self, code: u32, width: u8) -> Result<Lookup<Cid>, Never> {
    let Ok(found) = spans_at(&self.cid, width, code);

    match found {
      Some(span) => {
        return Ok(Lookup::Settled(
          code
            .checked_sub(span.start)
            .and_then(|offset| span.value.checked_add(offset)),
        ));
      }
      None => {}
    }

    let Ok(notdef) = spans_at(&self.notdef, width, code);

    match notdef {
      Some(span) => return Ok(Lookup::Settled(Some(span.value))),
      None => {}
    }

    let Ok(string) = self.bf_string(code);

    Ok(match string {
      Lookup::Settled(Some(BfString::Char(character))) => {
        Lookup::Settled(Some(u32::from(character)))
      }
      Lookup::Settled(Some(BfString::String(_))) | Lookup::Settled(None) | Lookup::Unsettled => {
        Lookup::Unsettled
      }
    })
  }

  pub(crate) fn bf_string(&self, code: u32) -> Result<Lookup<BfString>, Never> {
    let Ok(found) = find(&self.bf, code);

    match found {
      Some(span) => {
        let Ok(string) = string_at(span, code);

        Ok(Lookup::Settled(string))
      }
      None => Ok(Lookup::Unsettled),
    }
  }
}
