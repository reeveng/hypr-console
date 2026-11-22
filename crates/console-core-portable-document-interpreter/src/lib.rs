//! A page of a PDF read as a list of drawing calls, made on a [`Device`] the caller writes.
//!
//! This began as hayro-interpret and keeps its shape: a content stream is read operator by
//! operator against a graphics state, and what would be drawn is handed to the device as a path,
//! a glyph, an image, a clip or a group. Rasterizing, extracting text and anything else a page is
//! for are the device's business, and none of them is decided here.
//!
//! A form, a tiling pattern, a soft mask and a Type 3 glyph are each a content stream of their
//! own drawn from inside another one, which is the one place this crate reaches itself. The depth
//! of that is counted on the context and refused past a fixed bound, and the number of nested
//! paints a page may make is counted beside it, because a document can name the same form from
//! inside itself and a depth alone does not bound the work.
//!
//! What a question here answers is a word rather than a `bool`, and those words are in
//! `answers`. With `embed-fonts`, the fourteen standard fonts a PDF may use without embedding
//! are carried as permissively licensed substitutes, so a caller need not find them.

#![forbid(unsafe_code)]

#[macro_use]
mod log;

mod answers;
mod cache;
mod context;
mod convert;
mod device;
mod interpret;
mod ocg;
mod soft_mask;
mod types;
mod x_object;

pub use x_object::image_size_ok;

pub mod color;
pub mod encode;
pub mod font;
pub mod pattern;
pub mod shading;
pub mod util;

pub use answers::*;
pub use cache::CacheKey;
pub use console_core_character_maps;
pub use console_core_portable_document_functions::Function;
pub use console_core_portable_document_syntax;
pub use context::*;
pub use device::*;
pub use interpret::*;
pub use soft_mask::*;
pub use types::*;
pub use util::{RectExt, TransformExt};
