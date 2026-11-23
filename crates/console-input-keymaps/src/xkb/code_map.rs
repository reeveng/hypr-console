use crate::xkb::code::Code;
use crate::xkb::span::Span;
use crate::xkb::span::SpanUnit;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use std::cell::OnceCell;
use std::iter;
use std::path::Path;
use std::path::PathBuf;

#[derive(Default)]
pub(crate) struct CodeMap {
  ranges: Vec<CodeRange>,
}

struct CodeRange {
  span: Span,
  source: Source,
  include_span: Option<Span>,
}

enum Source {
  Original {
    code: Code,
    file: Option<PathBuf>,
    lines: OnceCell<Vec<SpanUnit>>,
  },
  Alias(u32),
}

pub(crate) struct CodeInfo<'a> {
  pub(crate) span: Span,
  pub(crate) file: Option<&'a PathBuf>,
  pub(crate) lines: &'a [SpanUnit],
  pub(crate) lines_offset: SpanUnit,
  pub(crate) include_span: Option<Span>,
}

fn line_starts(lo: SpanUnit, code: &Code) -> Result<Vec<SpanUnit>, Never> {
  let Ok(bytes) = code.as_bytes();
  let after_newlines = (1_u32..)
    .zip(bytes)
    .filter(|(_, byte)| **byte == b'\n')
    .map(|(after, _)| lo.saturating_add(after));

  Ok(iter::once(lo).chain(after_newlines).collect())
}

impl CodeMap {
  pub(crate) fn add(
    &mut self,
    file: Option<&Path>,
    include_span: Option<Span>,
    code: &Code,
  ) -> Result<Span, Never> {
    let Ok(length) = fitted::<_, SpanUnit>(code.len());
    let source = Source::Original {
      code: code.clone(),
      file: file.map(Path::to_path_buf),
      lines: OnceCell::new(),
    };

    self.add_range(length, source, include_span)
  }

  pub(crate) fn repeat(&mut self, original: Span, include_span: Span) -> Result<Span, Never> {
    let ::core::result::Result::Ok(found) = ::console_core_number_conversion::fixed_index(
      match self
        .ranges
        .binary_search_by_key(&original.lo, |range| range.span.lo)
      {
        Ok(found) => found,
        Err(_not_registered) => return Ok(original),
      },
    );
    let Ok(at) = fitted::<_, u32>(match ::console_core_number_conversion::index(found) {
      ::core::result::Result::Ok(native_position) => native_position,
    });
    let source = match self
      .ranges
      .get(match ::console_core_number_conversion::index(found) {
        ::core::result::Result::Ok(native_position) => native_position,
      })
      .map(|range| &range.source)
    {
      Some(Source::Original { .. }) => Source::Alias(at),
      Some(Source::Alias(first)) => Source::Alias(*first),
      None => return Ok(original),
    };
    let length = original.hi.saturating_sub(original.lo);

    self.add_range(length, source, Some(include_span))
  }

  fn add_range(
    &mut self,
    length: SpanUnit,
    source: Source,
    include_span: Option<Span>,
  ) -> Result<Span, Never> {
    let lo = match self.ranges.last() {
      Some(last) => last.span.hi.saturating_add(1),
      None => 0,
    };
    let span = Span {
      lo,
      hi: lo.saturating_add(length),
    };
    self.ranges.push(CodeRange {
      span,
      source,
      include_span,
    });
    Ok(span)
  }

  pub(crate) fn codes(&self) -> Result<u32, Never> {
    fitted::<_, u32>(self.ranges.len())
  }

  pub(crate) fn get(&self, span: Span) -> Result<Option<CodeInfo<'_>>, Never> {
    let Ok(found) =
      fitted::<_, u32>(self.ranges.partition_point(|range| range.span.hi < span.lo));
    let Ok(count) = self.codes();
    let at = match ::core::primitive::bool::then_some(found == count, ()) {
      ::core::option::Option::Some(()) => found.saturating_sub(1),
      ::core::option::Option::None => found,
    };
    let Ok(at_index) = index(at);
    let range = match self.ranges.get(at_index) {
      Some(range) => range,
      None => {
        return Ok(None);
      }
    };
    let owner = match range.source {
      Source::Alias(first) => {
        let Ok(first_index) = index(first);

        match self.ranges.get(first_index) {
          Some(owner) => owner,
          None => {
            return Ok(None);
          }
        }
      }
      Source::Original { .. } => range,
    };

    Ok(match &owner.source {
      Source::Original { code, file, lines } => {
        let lines_2 = lines.get_or_init(|| {
          let Ok(starts) = line_starts(owner.span.lo, code);
          starts
        });

        Some(CodeInfo {
          span: range.span,
          file: file.as_ref(),
          lines: lines_2,
          lines_offset: range.span.lo.saturating_sub(owner.span.lo),
          include_span: range.include_span,
        })
      }
      Source::Alias(_) => None,
    })
  }
}
