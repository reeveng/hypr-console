use crate::xkb::code::display_text;
use crate::xkb::code_slice::CodeSlice;
use crate::xkb::diagnostic::DiagnosticKind;
use crate::xkb::span::Span;
use crate::xkb::span::SpanExt;
use crate::xkb::span::Spanned;
use console_core_never::Never;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ParseIncludeError {
  MissingFileName,
  UnterminatedMapName,
  MissingMergeMode,
  InvalidGroupIndex(CodeSlice<'static>),
}

impl std::fmt::Display for ParseIncludeError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ParseIncludeError::MissingFileName => {
        f.write_str("include component does not contain a file name")
      }
      ParseIncludeError::UnterminatedMapName => {
        f.write_str("map name is not terminated with a `)`")
      }
      ParseIncludeError::MissingMergeMode => {
        f.write_str("include component does not start with a merge mode")
      }
      ParseIncludeError::InvalidGroupIndex(written) => {
        let Ok(text) = display_text(written);
        write!(f, "invalid group index `{}`", text)
      }
    }
  }
}

impl std::error::Error for ParseIncludeError {}

impl ParseIncludeError {
  pub(crate) fn diagnostic_kind(&self) -> Result<DiagnosticKind, Never> {
    Ok(match self {
      ParseIncludeError::MissingFileName => DiagnosticKind::MissingIncludeFileName,
      ParseIncludeError::UnterminatedMapName => DiagnosticKind::UnterminatedIncludeMapName,
      ParseIncludeError::MissingMergeMode => DiagnosticKind::MissingIncludeMergeMode,
      ParseIncludeError::InvalidGroupIndex(_) => DiagnosticKind::InvalidIncludeGroupIndex,
    })
  }
}

pub(super) fn missing_file_name(span: Span) -> Result<Spanned<ParseIncludeError>, Never> {
  Ok(ParseIncludeError::MissingFileName.spanned2(span))
}

pub(super) fn unterminated_map_name(span: Span) -> Result<Spanned<ParseIncludeError>, Never> {
  Ok(ParseIncludeError::UnterminatedMapName.spanned2(span))
}

pub(super) fn missing_merge_mode(span: Span) -> Result<Spanned<ParseIncludeError>, Never> {
  Ok(ParseIncludeError::MissingMergeMode.spanned2(span))
}

pub(super) fn invalid_group(
  s: &CodeSlice<'_>,
  span: Span,
) -> Result<Spanned<ParseIncludeError>, Never> {
  let Ok(to_owned) = s.to_owned();
  Ok(ParseIncludeError::InvalidGroupIndex(to_owned).spanned2(span))
}
