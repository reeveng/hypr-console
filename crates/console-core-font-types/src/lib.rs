//! The scalar types an OpenType font is written in: fixed-point numbers, 24-bit integers, glyph
//! and name identifiers, tags, offsets, versions and dates, and the big-endian bytes each is
//! stored as.
//!
//! This is fontations' `font-types` 0.12.6 (revision `0e93993`), MIT or Apache-2.0 at the
//! reader's choice, and it is what the font reader, metrics and hinting crates read a font
//! through. Every value is its own type rather than the integer it is stored in, so a glyph
//! identifier cannot be passed where a name identifier was meant; [`Scalar`] is how each is read
//! from and written to its bytes, and [`BigEndian`] is a value left as the bytes until it is asked
//! for, which is what lets a table be laid over a font without being copied out of it.
//!
//! Upstream's `std` feature is gone: nothing here needs more than `core`, and the two error types
//! implement `core::error::Error` with or without it. A length is a `u64` rather than a `usize`,
//! because how many bytes a value takes in a file is a fact about the format and not about the
//! machine reading it, and a glyph identifier no longer converts to a `usize` for the same reason.
//! The data types are the specification's, at
//! <https://learn.microsoft.com/en-us/typography/opentype/spec/otff#data-types>.

#![no_std]
#![cfg_attr(not(feature = "bytemuck"), forbid(unsafe_code))]

mod bbox;
mod fixed;
mod fword;
mod glyph_id;
mod int24;
mod longdatetime;
mod matrix;
mod name_id;
mod offset;
mod point;
mod raw;
mod tag;
mod uint24;
mod version;

pub use bbox::{BoundingBox, CornerOrder};
pub use fixed::{F2Dot14, F4Dot12, F6Dot10, F26Dot6, F48Dot16, Fixed};
pub use fword::{FWord, UfWord};
pub use glyph_id::{GlyphId, GlyphId16, GlyphId24, TryFromGlyphIdError};
pub use int24::Int24;
pub use longdatetime::LongDateTime;
pub use matrix::{Matrix, MatrixElement};
pub use name_id::{NameId, NameIdKind};
pub use offset::{Nullable, Nullness, Offset16, Offset24, Offset32};
pub use point::Point;
pub use raw::{BigEndian, FixedSize, Scalar};
pub use tag::{InvalidTag, Tag};
pub use uint24::Uint24;
pub use version::{Compatibility, Compatible, MajorMinor, Version16Dot16};

pub const TTC_HEADER_TAG: Tag = match Tag::new(b"ttcf") {
  Ok(tag) => tag,
};

pub const TT_SFNT_VERSION: u32 = 0x0001_0000;

pub const TRUE_SFNT_VERSION: u32 = 0x7472_7565;

pub const CFF_SFNT_VERSION: u32 = 0x4f54_544f;
