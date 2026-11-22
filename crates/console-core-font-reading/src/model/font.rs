//! Font representation.

mod blob;
mod format;
mod instance;
mod source;
mod tables;

pub use blob::FontBlob;
pub use format::{FontContainer, FontFormat};
pub use instance::{
  FontFeatureVariations, FontInstance, FontInstanceBuilder, FontVariation, NormalizedCoord,
};
pub use source::FontSource;
pub use tables::{FontTableFunction, FontTables};

#[doc(hidden)]
#[rust_analyzer::completions(hidden_from_completion)]
pub mod interop;

use super::once::Once;
use crate::{ReadError, ps::type1::Type1Font};
use alloc::{boxed::Box, sync::Arc};
use core::any::Any;

#[derive(Clone)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(explicit081_no_shared_ownership, reason = "cloned fonts share their parsed tables, immutable source, and once-initialized shaping data across threads")
)]
pub struct Font(Arc<FontRepr>);

impl Font {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(explicit081_no_shared_ownership, reason = "initialize the single parsed font representation shared by its cloneable handles")
  )]
  pub fn new(source: impl Into<FontSource>, index: u32) -> Result<Self, ReadError> {
    let source_2 =
      <_ as ::core::convert::Into<crate::model::font::source::FontSource>>::into(source);
    let kind = match FontTables::new(source_2.clone(), index) {
      Ok(tables) => Some(FontKindRepr::Sfnt(tables, index)),
      Err(_unreadable_sfnt_directory) => match &source_2 {
        FontSource::Blob(blob) => match FontFormat::new(blob) {
          Some(FontFormat::Type1) => (match Type1Font::new(blob) { Ok(value) => Some(value), Err(_invalid_type1_font) => None }).map(FontKindRepr::Type1),
          Some(FontFormat::Sfnt(_) | FontFormat::Cff(_)) | None => None,
        },
        FontSource::TableFunction(_) => None,
      },
    };
    let kind_2 = kind.ok_or(ReadError::MalformedData("Data isn't a font"))?;
    let repr = FontRepr {
      source: source_2,
      kind: kind_2,
      shaping_data: Once::new(),
    };
    Ok(Self(Arc::new(repr)))
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Font::source` method returns a borrow tied to the source lifetime directly")
  )]
  pub fn source(&self) -> &FontSource {
    &self.0.source
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Font::kind` method returns `FontKind` directly in the existing parser interface")
  )]
  pub fn kind(&self) -> FontKind<'_> {
    match &self.0.kind {
      FontKindRepr::Sfnt(tables, index) => FontKind::Sfnt(tables, *index),
      FontKindRepr::Type1(font) => FontKind::Type1(font),
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the published `Font::tables` method returns a borrow tied to the source lifetime directly")
  )]
  pub fn tables(&self) -> &FontTables {
    match &self.0.kind {
      FontKindRepr::Sfnt(tables, _) => tables,
      FontKindRepr::Type1(_) => &tables::EMPTY_FONT_TABLES,
    }
  }
}

struct FontRepr {
  source: FontSource,
  kind: FontKindRepr,

  shaping_data: Once<Box<dyn Any + Send + Sync>>,
}

#[derive(Clone)]
pub enum FontKind<'a> {
  Sfnt(&'a FontTables, u32),
  Type1(&'a Type1Font),
}

#[expect(
  clippy::large_enum_variant,
  reason = "font representations stay inline so choosing a kind does not allocate another container"
)]
enum FontKindRepr {
  Sfnt(FontTables, u32),
  Type1(Type1Font),
}
