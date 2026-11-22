//! Font data sources.

use super::{FontBlob, FontTableFunction};
use alloc::sync::Arc;
use types::Tag;

#[derive(Clone)]
pub enum FontSource {
  Blob(FontBlob),
  TableFunction(FontTableFunction),
}

impl<T: Into<FontBlob>> From<T> for FontSource {
  fn from(value: T) -> Self {
    Self::Blob(<_ as ::core::convert::Into<
      crate::model::font::blob::FontBlob,
    >>::into(value))
  }
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(explicit081_no_shared_ownership, reason = "the published conversion accepts a callback already shared with the font provider")
)]
impl From<Arc<dyn Fn(Tag) -> Option<FontBlob> + Send + Sync>> for FontSource {
  fn from(value: Arc<dyn Fn(Tag) -> Option<FontBlob> + Send + Sync>) -> Self {
    Self::TableFunction(FontTableFunction::new(value))
  }
}

impl From<FontTableFunction> for FontSource {
  fn from(value: FontTableFunction) -> Self {
    Self::TableFunction(value)
  }
}
