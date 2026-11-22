use console_core_never::Never;
mod error;

use crate::xkb::code_slice::CodeSlice;
use crate::xkb::group::GroupIdx;
use crate::xkb::include::error::ParseIncludeError;
use crate::xkb::include::error::invalid_group;
use crate::xkb::include::error::missing_file_name;
use crate::xkb::include::error::missing_merge_mode;
use crate::xkb::include::error::unterminated_map_name;
use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use crate::xkb::kccgst::MergeMode;
use crate::xkb::span::Span;
use crate::xkb::span::SpanExt;
use crate::xkb::span::SpanUnit;
use crate::xkb::span::Spanned;
use console_core_number_conversion::FromBytes;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use console_input_keymaps_derive::CloneWithDelta;
use std::ops::Range;

#[derive(Debug)]
pub(crate) struct Include {
  pub(crate) merge_mode: Option<Spanned<MergeMode>>,
  pub(crate) file: Spanned<Interned>,
  pub(crate) map: Option<Spanned<Interned>>,
  pub(crate) group: Option<IncludeGroup>,
}

#[derive(Copy, Clone, Debug, CloneWithDelta)]
pub(crate) struct IncludeGroup {
  pub(crate) group_name: Spanned<Interned>,
  pub(crate) group: GroupIdx,
}

pub(crate) struct IncludeIter<'a> {
  interner: &'a mut Interner,
  s: CodeSlice<'static>,
  pos: u32,
  span_lo: SpanUnit,
  first: bool,
}

pub(crate) fn parse_include(
  interner: &mut Interner,
  include: Spanned<Interned>,
) -> Result<IncludeIter<'_>, Never> {
  let Ok(interned) = interner.get(include.val);
  let Ok(s) = interned.to_owned();
  Ok(IncludeIter {
    s,
    interner,
    pos: 0,
    span_lo: include.span.lo.saturating_add(1),
    first: true,
  })
}

impl Iterator for IncludeIter<'_> {
  type Item = Result<Include, Spanned<ParseIncludeError>>;

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(bytes) = self.s.as_bytes();
    let Ok(end) = fitted::<_, u32>(bytes.len());
    let Ok(start) = first_from(bytes, self.pos, |byte| !byte.is_ascii_whitespace());
    self.pos = start;

    match ::core::primitive::bool::then_some(self.pos >= end, ()) {
      ::core::option::Option::Some(()) => {
        return None;
      }
      ::core::option::Option::None => {}
    }

    let written = Written {
      slice: &self.s,
      lo: self.span_lo,
    };
    let captures = match capture(written, self.pos) {
      Ok(c) => c,
      Err(e) => {
        self.pos = end;
        return Some(Err(e));
      }
    };
    self.pos = captures.pos;
    let written_file = captures.file;
    let mm = captures.mm;

    match ::core::primitive::bool::then_some(mm.is_none() && !self.first, ()) {
      ::core::option::Option::Some(()) => {
        let span = Span {
          lo: written_file.span.lo,
          hi: written_file.span.lo.saturating_add(1),
        };
        let Ok(missing_merge_mode) = missing_merge_mode(span);
        return Some(Err(missing_merge_mode));
      }
      ::core::option::Option::None => {}
    }

    let Ok(file_name) = self.interner.intern(&written_file.val);
    let file = file_name.spanned2(written_file.span);
    let map = match captures.map {
      Some(written_map) => {
        let Ok(map_name) = self.interner.intern(&written_map.val);
        Some(map_name.spanned2(written_map.span))
      }
      None => None,
    };
    let group = match include_group(self.interner, captures.group) {
      Ok(group) => group,
      Err(invalid) => {
        return Some(Err(invalid));
      }
    };
    self.first = false;
    Some(Ok(Include {
      merge_mode: mm,
      file,
      map,
      group,
    }))
  }
}

fn include_group(
  interner: &mut Interner,
  written: Option<Spanned<CodeSlice<'_>>>,
) -> Result<Option<IncludeGroup>, Spanned<ParseIncludeError>> {
  let g = match written {
    Some(g) => g,
    None => {
      return Ok(None);
    }
  };
  let Ok(interned_group) = interner.intern(&g.val);
  let group_name = interned_group.spanned2(g.span);
  let Ok(as_bytes) = g.val.as_bytes();
  let number = match u32::from_bytes_dec(as_bytes) {
    Some(number) => number,
    None => {
      let Ok(invalid) = invalid_group(&g.val, g.span);
      return Err(invalid);
    }
  };
  let Ok(checked) = GroupIdx::new(number);
  let group = match checked {
    Some(group) => group,
    None => {
      let Ok(invalid) = invalid_group(&g.val, g.span);
      return Err(invalid);
    }
  };
  Ok(Some(IncludeGroup { group_name, group }))
}

struct Capture<'a> {
  mm: Option<Spanned<MergeMode>>,
  file: Spanned<CodeSlice<'a>>,
  map: Option<Spanned<CodeSlice<'a>>>,
  group: Option<Spanned<CodeSlice<'a>>>,
  pos: u32,
}

struct Part<'a> {
  written: Option<Spanned<CodeSlice<'a>>>,
  after: u32,
}

#[derive(Copy, Clone)]
struct Written<'a> {
  slice: &'a CodeSlice<'static>,
  lo: SpanUnit,
}

impl<'a> Written<'a> {
  fn bytes(self) -> Result<&'a [u8], Never> {
    self.slice.as_bytes()
  }

  fn at(self, offset: u32) -> Result<SpanUnit, Never> {
    Ok(self.lo.saturating_add(offset))
  }

  fn part(self, range: Range<u32>) -> Result<Spanned<CodeSlice<'a>>, Never> {
    let Ok(lo) = self.at(range.start);
    let Ok(hi) = self.at(range.end);
    let Ok(part) = part_of(self.slice, range);
    Ok(part.spanned(lo, hi))
  }
}

fn capture(written: Written<'_>, from: u32) -> Result<Capture<'_>, Spanned<ParseIncludeError>> {
  let Ok(bytes) = written.bytes();
  let Ok((mm, file_start)) = merge_mode_at(written, from);
  let Ok(first) = byte_at(bytes, file_start);

  match first {
    None | Some(b'(' | b'|' | b'+' | b'^' | b':') => {
      let Ok(at) = written.at(file_start);
      let Ok(missing_file_name) = missing_file_name(Span {
        lo: at,
        hi: at.saturating_add(1),
      });
      return Err(missing_file_name);
    }
    Some(_) => {}
  }

  let Ok(file_end) = first_from(bytes, file_start.saturating_add(1), |byte| {
    matches!(byte, b'(' | b'|' | b'+' | b'^' | b':')
  });
  let Ok(after_file) = byte_at(bytes, file_end);
  let map = match ::core::primitive::bool::then_some(after_file == Some(b'('), ()) {
    ::core::option::Option::Some(()) => map_at(written, file_end)?,
    ::core::option::Option::None => Part {
      written: None,
      after: file_end,
    },
  };
  let Ok(after_map) = byte_at(bytes, map.after);
  let group = match ::core::primitive::bool::then_some(after_map == Some(b':'), ()) {
    ::core::option::Option::Some(()) => group_at(written, map.after)?,
    ::core::option::Option::None => Part {
      written: None,
      after: map.after,
    },
  };
  let Ok(file) = written.part(file_start..file_end);
  Ok(Capture {
    mm,
    file,
    map: map.written,
    group: group.written,
    pos: group.after,
  })
}

fn merge_mode_at(
  written: Written<'_>,
  at: u32,
) -> Result<(Option<Spanned<MergeMode>>, u32), Never> {
  let Ok(bytes) = written.bytes();
  let Ok(byte) = byte_at(bytes, at);
  let found = match byte {
    Some(b'|') => Some(MergeMode::Augment),
    Some(b'+') => Some(MergeMode::Override),
    Some(b'^') => Some(MergeMode::Replace),
    Some(_) | None => None,
  };
  let Ok(mm_lo) = written.at(at);

  Ok(match found {
    Some(mode) => (
      Some(mode.spanned(mm_lo, mm_lo.saturating_add(1))),
      at.saturating_add(1),
    ),
    None => (None, at),
  })
}

fn map_at(written: Written<'_>, open: u32) -> Result<Part<'_>, Spanned<ParseIncludeError>> {
  let Ok(bytes) = written.bytes();
  let Ok(end) = fitted::<_, u32>(bytes.len());
  let map_start = open.saturating_add(1);
  let Ok(map_end) = first_from(bytes, map_start, |byte| byte == b')');

  match ::core::primitive::bool::then_some(map_end >= end, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(at) = written.at(map_end);
      let Ok(unterminated_map_name) = unterminated_map_name(Span {
        lo: at,
        hi: at.saturating_add(1),
      });
      return Err(unterminated_map_name);
    }
    ::core::option::Option::None => {}
  }

  let Ok(map) = written.part(map_start..map_end);
  Ok(Part {
    written: Some(map),
    after: map_end.saturating_add(1),
  })
}

fn group_at(written: Written<'_>, colon: u32) -> Result<Part<'_>, Spanned<ParseIncludeError>> {
  let Ok(bytes) = written.bytes();
  let group_start = colon.saturating_add(1);
  let Ok(first) = byte_at(bytes, group_start);

  match first {
    Some(b'0'..=b'9') => {}
    Some(_) | None => {
      let Ok(digit) = part_of(written.slice, group_start..group_start.saturating_add(1));
      let Ok(at) = written.at(group_start);
      let Ok(invalid_group) = invalid_group(
        &digit,
        Span {
          lo: at,
          hi: at.saturating_add(1),
        },
      );
      return Err(invalid_group);
    }
  }

  let Ok(group_end) = first_from(bytes, group_start.saturating_add(1), |byte| {
    !byte.is_ascii_digit()
  });
  let Ok(group) = written.part(group_start..group_end);
  Ok(Part {
    written: Some(group),
    after: group_end,
  })
}

fn part_of<'a>(slice: &'a CodeSlice<'static>, range: Range<u32>) -> Result<CodeSlice<'a>, Never> {
  slice.slice(range)
}

fn byte_at(bytes: &[u8], at: u32) -> Result<Option<u8>, Never> {
  let Ok(position) = index(at);
  Ok(bytes.get(position).copied())
}

fn first_from(bytes: &[u8], from: u32, stops: impl Fn(u8) -> bool) -> Result<u32, Never> {
  let Ok(start) = index(from);
  let Ok(end) = fitted::<_, u32>(bytes.len());
  let rest = match bytes.get(start..) {
    Some(rest) => rest,
    None => &[],
  };
  let found = (from..).zip(rest).find(|(_, byte)| stops(**byte));

  Ok(match found {
    Some((at, _)) => at,
    None => end.max(from),
  })
}
