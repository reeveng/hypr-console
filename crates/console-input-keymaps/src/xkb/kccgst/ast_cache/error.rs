use crate::xkb::code::display_text;
use crate::xkb::code_slice::CodeSlice;
use crate::xkb::diagnostic::DiagnosticKind;
use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use crate::xkb::span::Span;
use crate::xkb::span::SpanExt;
use crate::xkb::span::Spanned;
use console_core_never::Never;
use std::fmt;
use std::fmt::Formatter;

#[derive(Clone, Debug)]
pub(crate) enum AstCacheError {
  NotFound {
    file: CodeSlice<'static>,
    map: Option<CodeSlice<'static>>,
  },
}

impl fmt::Display for AstCacheError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      AstCacheError::NotFound { file, map } => {
        let Ok(text) = display_text(file);
        write!(f, "not found: `{}{}`", text, fmt::from_fn(|f_2| show_map(f_2, map)))
      }
    }
  }
}

impl std::error::Error for AstCacheError {}

impl AstCacheError {
  pub(crate) fn diagnostic_kind(&self) -> Result<DiagnosticKind, Never> {
    Ok(DiagnosticKind::FileNotFound)
  }
}

fn show_map(f: &mut Formatter<'_>, map: &Option<CodeSlice<'_>>) -> fmt::Result {
  match map {
    Some(m) => {
      let Ok(bytes) = m.as_bytes();
      let Ok(text) = display_text(bytes);
      write!(f, "({})", text)?;
    }
    None => {}
  }

  Ok(())
}

pub(super) fn not_found(
  interner: &Interner,
  name: Interned,
  map: Option<Interned>,
  span: Span,
) -> Result<Spanned<AstCacheError>, Never> {
  let Ok(interned) = interner.get(name);
  let Ok(file) = interned.to_owned();
  let owned_map = map.map(|m| {
    let Ok(interned_map) = interner.get(m);
    let Ok(owned) = interned_map.to_owned();
    owned
  });
  Ok(
    AstCacheError::NotFound {
      file,
      map: owned_map,
    }
    .spanned2(span),
  )
}
