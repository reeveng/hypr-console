//! Reading OpenType tables
//!
//! This crate provides memory safe zero-allocation parsing of font files.
//! It is unopinionated, and attempts to provide raw access to the underlying
//! font data as it is described in the [OpenType specification][spec].
//!
//! This crate is intended for use by other parts of a font stack, such as a
//! shaping engine or a glyph rasterizer.
//!
//! In addition to raw data access, this crate may also provide reference
//! implementations of algorithms for interpreting that data, where such an
//! implementation is required for the data to be useful. For instance, we
//! provide functions for [mapping codepoints to glyph identifiers][cmap-impl]
//! using the `cmap` table, or for [decoding entries in the `name` table][NameString].
//!
//! For higher level/more ergonomic access to font data, you may want to look
//! into using [`skrifa`] instead.
//!
//! ## Structure & codegen
//!
//! The root [`tables`] module contains a submodule for each supported
//! [table][table-directory], and that submodule contains items for each table,
//! record, flagset or enum described in the relevant portion of the spec.
//!
//! The majority of the code in the tables module is auto-generated. For more
//! information on our use of codegen, see the [codegen tour].
//!
//! # Related projects
//!
//! - [`write-fonts`] is a companion crate for creating/modifying font files
//! - [`skrifa`] provides access to glyph outlines and metadata (in the same vein
//!   as [freetype])
//!
//! # Example
//!
//! ```no_run
//! # let path_to_my_font_file = std::path::Path::new("");
//! use console_core_font_reading::{FontRef, TableProvider};
//! let font_bytes = std::fs::read(path_to_my_font_file).unwrap();
//! // Single fonts only. for font collections (.ttc) use FontRef::from_index
//! let font = FontRef::new(&font_bytes).expect("failed to read font data");
//! let head = font.head().expect("missing 'head' table");
//! let maxp = font.maxp().expect("missing 'maxp' table");
//!
//! let Ok(revision) = head.font_revision();
//! let Ok(glyphs) = maxp.num_glyphs();
//! println!("font version {revision} containing {glyphs} glyphs");
//! ```
//!
//!
//! [spec]: https://learn.microsoft.com/en-us/typography/opentype/spec/
//! [codegen-tour]: https://github.com/googlefonts/fontations/blob/main/docs/codegen-tour.md
//! [cmap-impl]: tables::cmap::Cmap::map_codepoint
//! [`write-fonts`]: https://docs.rs/write-fonts/
//! [`skrifa`]: https://docs.rs/skrifa/
//! [freetype]: http://freetype.org
//! [codegen tour]: https://github.com/googlefonts/fontations/blob/main/docs/codegen-tour.md
//! [NameString]: tables::name::NameString
//! [table-directory]: https://learn.microsoft.com/en-us/typography/opentype/spec/otff#table-directory

#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![deny(rustdoc::broken_intra_doc_links)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(any(feature = "std", test))]
#[macro_use]
extern crate std;

#[cfg(all(not(feature = "std"), not(test)))]
#[macro_use]
extern crate core as std;

extern crate alloc;

pub mod array;
pub mod collections;
mod font_data;
mod layout;
pub mod model;
mod offset;
mod offset_array;
pub mod ps;
mod read;
mod table_provider;
mod table_ref;
pub mod tables;
#[cfg(feature = "experimental_traverse")]
pub mod traversal;

#[cfg(any(test, feature = "codegen_test"))]
pub mod codegen_test;

#[cfg(test)]
mod test_support;

pub use console_core_integer_sets::{Emptiness, Overlap, Presence};
use font_data::Cursor;
pub use font_data::FontData;
pub use offset::{Offset, ResolveNullableOffset, ResolveOffset};
pub use offset_array::{ArrayOfNullableOffsets, ArrayOfOffsets};
pub use read::{ComputeSize, FontRead, FontReadWithArgs, ReadArgs, ReadError, VarSize};
pub use table_provider::{TableProvider, TopLevelTable};
pub use table_ref::MinByteRange;
use types::{GlyphId, Tag};

pub extern crate font_types as types;

#[doc(hidden)]
pub(crate) mod codegen_prelude {
  pub use crate::array::{ComputedArray, VarLenArray};
  pub use crate::font_data::{Cursor, FontData};
  pub use crate::offset::{Offset, ResolveNullableOffset, ResolveOffset};
  pub use crate::offset_array::{ArrayOfNullableOffsets, ArrayOfOffsets};

  pub use crate::read::{
    ComputeSize, Discriminant, FontRead, FontReadWithArgs, Format, ReadArgs, ReadError, VarSize,
  };
  pub use crate::table_provider::TopLevelTable;
  pub use crate::table_ref::MinByteRange;
  pub use console_core_integer_sets::{Emptiness, Overlap, Presence};
  pub use console_core_never::Never;
  pub use std::ops::Range;

  pub use types::*;

  #[cfg(feature = "experimental_traverse")]
  pub use crate::traversal::{self, Field, FieldType, RecordResolver, SomeRecord, SomeTable};

  pub(crate) use crate::layout as transforms;

  #[cfg_attr(
    dylint_lib = "explicit080_macro_budget",
    expect(
      explicit080_macro_budget,
      reason = "generated table views publish the same three inherent offset and shape methods from this definition"
    )
  )]
  #[macro_export]
  macro_rules! basic_table_impls {
    (impl_the_methods) => {
      pub fn resolve_offset<O: Offset, R: FontRead<'a>>(
        &self,
        offset: O,
      ) -> Result<R, ReadError> {
        offset.resolve(self.data)
      }

      pub fn offset_data(&self) -> Result<FontData<'a>, console_core_never::Never> {
        Ok(self.data)
      }

      #[deprecated(note = "just use the base type directly")]
      pub fn shape(&self) -> Result<&Self, console_core_never::Never> {
        Ok(self)
      }
    };
  }

  pub(crate) use crate::basic_table_impls;
}

mod directory;
mod font;

pub use directory::{TTCHeader, TableDirectory, TableRecord};
pub use font::{CollectionRef, FileRef, FontRef};

impl<'a> TableProvider<'a> for FontRef<'a> {
  fn data_for_tag(&self, tag: types::Tag) -> Result<Option<FontData<'a>>, ReadError> {
    self.table_data(tag)
  }
}
