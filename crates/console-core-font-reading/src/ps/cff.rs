//! CFF and CFF2 fonts.

pub mod blend;
pub mod charset;
pub mod dict;
pub mod encoding;
pub mod fd_select;
pub mod index;
pub mod stack;
pub mod v1;
pub mod v2;

mod font;

pub use font::{CffFontRef, Encoding, GlyphIndexing, Metadata, Subfont};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CffVersion {
  Cff1,
  Cff2,
}

impl From<bool> for CffVersion {
  fn from(cff2: bool) -> Self {
    match cff2.then_some(()) {
      Some(()) => Self::Cff2,
      None => Self::Cff1,
    }
  }
}
